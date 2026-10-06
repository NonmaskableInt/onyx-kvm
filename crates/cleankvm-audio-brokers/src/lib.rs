//! Separate, default-deny brokers for the admitted local audio probes.

use std::collections::BTreeMap;
use std::env;
use std::fs::{self, File, Permissions};
use std::io::{self, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::time::{Duration, Instant};

use cleankvm_media_protocol::{
    AUDIO_REQUEST_BYTES, CaptureMetrics, Operation, PcmFormat, PcmShape, PlaybackReceipt,
    WIRE_HEADER_BYTES, WireHeader,
};

const CLEANKVM_GID: u32 = 900;
#[cfg(target_os = "linux")]
const CLEANKVM_UID: u32 = 900;
const IO_TIMEOUT: Duration = Duration::from_secs(6);
const MAX_PROBE_OUTPUT: usize = 4096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    Capture,
    Playback,
}

impl Kind {
    const fn operation(self) -> Operation {
        match self {
            Self::Capture => Operation::CapturePcm,
            Self::Playback => Operation::PlayPcm,
        }
    }

    const fn defaults(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Self::Capture => (
                "/run/cleankvm/audio-capture.sock",
                "/run/cleankvm/audio-capture-enabled",
                "/usr/libexec/cleankvm-audio-capture-probe",
            ),
            Self::Playback => (
                "/run/cleankvm/audio-output.sock",
                "/run/cleankvm/audio-output-enabled",
                "/usr/libexec/cleankvm-audio-playback-probe",
            ),
        }
    }
}

struct Config {
    kind: Kind,
    socket: PathBuf,
    gate: PathBuf,
    probe: PathBuf,
}

#[must_use]
pub fn run(kind: Kind) -> ExitCode {
    let (socket, gate, probe) = kind.defaults();
    let config = Config {
        kind,
        socket: env_path("CLEANKVM_AUDIO_SOCKET", socket),
        gate: env_path("CLEANKVM_AUDIO_GATE", gate),
        probe: env_path("CLEANKVM_AUDIO_PROBE", probe),
    };
    match serve_forever(&config) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("cleankvm audio {kind:?} broker: {error}");
            ExitCode::FAILURE
        }
    }
}

fn env_path(name: &str, default: &str) -> PathBuf {
    env::var_os(name).map_or_else(|| PathBuf::from(default), PathBuf::from)
}

fn serve_forever(config: &Config) -> io::Result<()> {
    if let Some(parent) = config.socket.parent() {
        fs::create_dir_all(parent)?;
        fs::set_permissions(parent, Permissions::from_mode(0o2750))?;
    }
    remove_if_exists(&config.socket)?;
    let listener = UnixListener::bind(&config.socket)?;
    rustix::fs::chown(
        &config.socket,
        Some(rustix::fs::Uid::from_raw(0)),
        Some(rustix::fs::Gid::from_raw(CLEANKVM_GID)),
    )
    .map_err(io::Error::from)?;
    fs::set_permissions(&config.socket, Permissions::from_mode(0o660))?;
    for connection in listener.incoming() {
        match connection {
            Ok(stream) => {
                if let Err(error) = serve_one(stream, config) {
                    eprintln!("cleankvm audio {:?} client rejected: {error}", config.kind);
                }
            }
            Err(error) => eprintln!("cleankvm audio {:?} accept failed: {error}", config.kind),
        }
    }
    Ok(())
}

fn serve_one(mut stream: UnixStream, config: &Config) -> io::Result<()> {
    authenticate_peer(&stream)?;
    stream.set_read_timeout(Some(IO_TIMEOUT))?;
    stream.set_write_timeout(Some(IO_TIMEOUT))?;
    let mut header = [0_u8; WIRE_HEADER_BYTES];
    let mut payload = [0_u8; AUDIO_REQUEST_BYTES];
    stream.read_exact(&mut header)?;
    stream.read_exact(&mut payload)?;
    authorize_request(header, payload, config.kind, &config.gate)?;
    let started = Instant::now();
    let output = Command::new(&config.probe)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()?;
    if !output.status.success() || output.stdout.len() > MAX_PROBE_OUTPUT {
        return Err(io::Error::other("bounded audio probe failed"));
    }
    let elapsed_ms = u32::try_from(started.elapsed().as_millis())
        .map_err(|_| invalid("invalid probe elapsed time"))?;
    let fields = parse_fields(&output.stdout)?;
    match config.kind {
        Kind::Capture => stream.write_all(&capture_metrics(&fields, elapsed_ms)?.encode())?,
        Kind::Playback => stream.write_all(&playback_receipt(&fields, elapsed_ms)?.encode())?,
    }
    stream.flush()
}

fn authorize_request(
    header: [u8; WIRE_HEADER_BYTES],
    payload: [u8; AUDIO_REQUEST_BYTES],
    kind: Kind,
    gate: &Path,
) -> io::Result<PcmShape> {
    let header = WireHeader::decode(header).map_err(|_| invalid("invalid wire header"))?;
    if header.operation != kind.operation()
        || usize::try_from(header.payload_bytes).ok() != Some(AUDIO_REQUEST_BYTES)
        || File::open(gate).is_err()
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "audio capability is disabled",
        ));
    }
    let shape = PcmShape::decode(payload).map_err(|_| invalid("invalid PCM shape"))?;
    let admitted = PcmShape::new(PcmFormat::Signed16LittleEndian, 16_000, 1, 1_000)
        .map_err(|_| invalid("invalid admitted PCM shape"))?;
    if shape != admitted {
        return Err(invalid("PCM shape is not admitted"));
    }
    Ok(shape)
}

fn parse_fields(output: &[u8]) -> io::Result<BTreeMap<&str, &str>> {
    let text = std::str::from_utf8(output).map_err(|_| invalid("probe output is not UTF-8"))?;
    let mut fields = BTreeMap::new();
    for line in text.lines() {
        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| invalid("malformed probe output"))?;
        if fields.insert(key, value).is_some() {
            return Err(invalid("duplicate probe output field"));
        }
    }
    Ok(fields)
}

fn number(fields: &BTreeMap<&str, &str>, key: &str) -> io::Result<u32> {
    fields
        .get(key)
        .ok_or_else(|| invalid("missing probe output field"))?
        .parse()
        .map_err(|_| invalid("invalid probe output number"))
}

fn require_common(fields: &BTreeMap<&str, &str>) -> io::Result<()> {
    if number(fields, "rate_hz")? != 16_000
        || number(fields, "channels")? != 1
        || fields.get("format") != Some(&"s16_le")
    {
        return Err(invalid("probe reported unexpected PCM shape"));
    }
    Ok(())
}

fn capture_metrics(fields: &BTreeMap<&str, &str>, elapsed_ms: u32) -> io::Result<CaptureMetrics> {
    require_common(fields)?;
    let result = CaptureMetrics {
        frames: number(fields, "capture_frames")?,
        bytes: number(fields, "capture_bytes")?,
        peak_absolute: number(fields, "peak_abs")?,
        mean_absolute: number(fields, "mean_abs")?,
        rms: number(fields, "rms")?,
        clipped_samples: number(fields, "clipped_samples")?,
        zero_samples: number(fields, "zero_samples")?,
        elapsed_ms,
    };
    if result.frames != 16_000 || result.bytes != 32_000 {
        return Err(invalid("probe reported unexpected capture length"));
    }
    Ok(result)
}

fn playback_receipt(fields: &BTreeMap<&str, &str>, elapsed_ms: u32) -> io::Result<PlaybackReceipt> {
    require_common(fields)?;
    let result = PlaybackReceipt {
        frames: number(fields, "playback_frames")?,
        bytes: number(fields, "playback_bytes")?,
        tone_hz: number(fields, "tone_hz")?,
        peak: number(fields, "tone_peak")?,
        elapsed_ms,
    };
    if result.frames != 16_000
        || result.bytes != 32_000
        || result.tone_hz != 1_000
        || result.peak != 512
    {
        return Err(invalid("probe reported unexpected playback receipt"));
    }
    Ok(result)
}

#[cfg(target_os = "linux")]
fn authenticate_peer(stream: &UnixStream) -> io::Result<()> {
    let credentials = rustix::net::sockopt::socket_peercred(stream).map_err(io::Error::from)?;
    if credentials.uid.as_raw() != CLEANKVM_UID {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "audio broker peer is not authorized",
        ));
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn authenticate_peer(_stream: &UnixStream) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "peer credential authentication requires Linux",
    ))
}

fn remove_if_exists(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strict_capture_output_becomes_metrics_only() {
        let output = b"capture_frames=16000\ncapture_bytes=32000\nrate_hz=16000\nchannels=1\nformat=s16_le\npeak_abs=32767\nmean_abs=10990\nrms=13098\nclipped_samples=0\nzero_samples=0\n";
        let fields = parse_fields(output).unwrap_or_else(|error| panic!("valid fields: {error}"));
        let metrics = capture_metrics(&fields, 1_010)
            .unwrap_or_else(|error| panic!("valid metrics: {error}"));
        assert_eq!(metrics.frames, 16_000);
        assert_eq!(metrics.bytes, 32_000);
        assert_eq!(metrics.elapsed_ms, 1_010);
    }

    #[test]
    fn playback_receipt_enforces_the_admitted_tone() {
        let output = b"playback_frames=16000\nplayback_bytes=32000\nrate_hz=16000\nchannels=1\nformat=s16_le\ntone_hz=1000\ntone_peak=512\ndac_control=31\n";
        let fields = parse_fields(output).unwrap_or_else(|error| panic!("valid fields: {error}"));
        assert!(playback_receipt(&fields, 1_005).is_ok());
        let changed = b"playback_frames=16000\nplayback_bytes=32000\nrate_hz=16000\nchannels=1\nformat=s16_le\ntone_hz=1000\ntone_peak=513\ndac_control=31\n";
        let changed_fields =
            parse_fields(changed).unwrap_or_else(|error| panic!("valid fields: {error}"));
        assert!(playback_receipt(&changed_fields, 1_005).is_err());
    }

    #[test]
    fn malformed_or_duplicate_probe_output_is_rejected() {
        assert!(parse_fields(b"not-a-field\n").is_err());
        assert!(parse_fields(b"rate_hz=16000\nrate_hz=8000\n").is_err());
        assert_eq!(cleankvm_media_protocol::CAPTURE_METRICS_BYTES, 32);
        assert_eq!(cleankvm_media_protocol::PLAYBACK_RECEIPT_BYTES, 20);
    }
}
