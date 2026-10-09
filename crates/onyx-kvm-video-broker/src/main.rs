use onyx_kvm_media_protocol::{Operation, SNAPSHOT_RESPONSE_BYTES, WireHeader};
use onyx_kvm_video_broker::{
    EXPECTED_YUV422P_1080P_BYTES, FRAME_LEN, decode_and_authorize, summarize_frame_fast,
};
use std::env;
use std::fs::{self, File, Permissions};
use std::io::{self, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, ExitCode, Stdio};
use std::time::Duration;

const DEFAULT_SOCKET: &str = "/run/onyx-kvm/video.sock";
const DEFAULT_ENABLE: &str = "/run/onyx-kvm/video-enabled";
const DEFAULT_MODULE_DIR: &str = "/usr/lib/onyx-kvm/video-quarantine";
const DEFAULT_PROBE_BIN: &str = "/usr/libexec/onyx-kvm-raw-frame-probe";
const DEFAULT_FRAME_PATH: &str = "/run/onyx-kvm-frame.yuv422p";
const DEVMEM_PATH: &str = "/dev/mem";
#[cfg(target_os = "linux")]
const ONYX_KVM_UID: u32 = 900;
const ONYX_KVM_GID: u32 = 900;

const LOAD_ORDER: &[(&str, &[&str])] = &[
    ("soph_sys", &[]),
    ("soph_base", &[]),
    (
        "soph_mipi_rx",
        &["lane_phase=0,8,8,3,3", "csi_ck_phase=0", "hs_settle=8"],
    ),
    ("soph_snsr_i2c", &[]),
    ("soph_vi", &[]),
    ("soph_vpss", &[]),
];

const UNLOAD_ORDER: &[&str] = &[
    "soph_vpss",
    "soph_vi",
    "soph_snsr_i2c",
    "soph_mipi_rx",
    "soph_base",
    "soph_sys",
];

struct Config {
    socket: PathBuf,
    enable: PathBuf,
    module_dir: PathBuf,
    probe_bin: PathBuf,
    frame_path: PathBuf,
}

fn main() -> ExitCode {
    let config = Config {
        socket: env_path_any(
            "ONYX_KVM_VIDEO_SOCKET",
            "CLEANKVM_VIDEO_SOCKET",
            DEFAULT_SOCKET,
        ),
        enable: env_path_any(
            "ONYX_KVM_VIDEO_ENABLE",
            "CLEANKVM_VIDEO_ENABLE",
            DEFAULT_ENABLE,
        ),
        module_dir: env_path_any(
            "ONYX_KVM_VIDEO_MODULE_DIR",
            "CLEANKVM_VIDEO_MODULE_DIR",
            DEFAULT_MODULE_DIR,
        ),
        probe_bin: env_path_any(
            "ONYX_KVM_VIDEO_PROBE_BIN",
            "CLEANKVM_VIDEO_PROBE_BIN",
            DEFAULT_PROBE_BIN,
        ),
        frame_path: env_path_any(
            "ONYX_KVM_VIDEO_FRAME_PATH",
            "CLEANKVM_VIDEO_FRAME_PATH",
            DEFAULT_FRAME_PATH,
        ),
    };
    match run(&config) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("onyx-kvm-video-broker: {error}");
            ExitCode::FAILURE
        }
    }
}

fn env_path_any(primary: &str, fallback: &str, default: &str) -> PathBuf {
    env::var_os(primary)
        .or_else(|| env::var_os(fallback))
        .map_or_else(|| PathBuf::from(default), PathBuf::from)
}

fn run(config: &Config) -> io::Result<()> {
    if let Some(parent) = config.socket.parent() {
        fs::create_dir_all(parent)?;
        fs::set_permissions(parent, Permissions::from_mode(0o2750))?;
    }
    remove_if_exists(&config.socket)?;
    remove_if_exists(&config.frame_path)?;
    let listener = UnixListener::bind(&config.socket)?;
    rustix::fs::chown(
        &config.socket,
        Some(rustix::fs::Uid::from_raw(0)),
        Some(rustix::fs::Gid::from_raw(ONYX_KVM_GID)),
    )
    .map_err(io::Error::from)?;
    fs::set_permissions(&config.socket, Permissions::from_mode(0o660))?;
    for connection in listener.incoming() {
        match connection {
            Ok(stream) => {
                if let Err(error) = serve(stream, config) {
                    eprintln!("onyx-kvm-video-broker: client rejected: {error}");
                }
            }
            Err(error) => eprintln!("onyx-kvm-video-broker: accept failed: {error}"),
        }
    }
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn serve(mut stream: UnixStream, config: &Config) -> io::Result<()> {
    authenticate_peer(&stream)?;
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    stream.set_write_timeout(Some(Duration::from_secs(10)))?;
    if Path::new(DEVMEM_PATH).exists() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "/dev/mem must be absent before video capture",
        ));
    }
    if File::open(&config.enable).is_err() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "video capture is disabled",
        ));
    }
    remove_if_exists(&config.frame_path)?;
    remove_if_exists(Path::new("/run/cleankvm-frame.yuv422p"))?;
    let _guard = ModuleCleanupGuard {
        frame_path: &config.frame_path,
    };
    load_quarantine_modules(&config.module_dir)?;

    let probe_bin = resolve_probe_bin(config);
    let mut streaming_probe = match StreamingProbe::spawn(probe_bin) {
        Ok(p) => Some(p),
        Err(err) => {
            eprintln!("onyx-kvm-video-broker: streaming probe spawn note: {err}");
            None
        }
    };

    let mut is_first = true;
    loop {
        let mut frame = [0_u8; FRAME_LEN];
        match stream.read_exact(&mut frame) {
            Ok(()) => {}
            Err(err) if err.kind() == io::ErrorKind::UnexpectedEof => {
                break;
            }
            Err(err) => {
                if !is_first {
                    break;
                }
                return Err(err);
            }
        }
        is_first = false;
        if Path::new(DEVMEM_PATH).exists() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "/dev/mem must be absent before video capture",
            ));
        }
        let enabled = File::open(&config.enable).is_ok();
        let (operation, request) = decode_and_authorize(&frame, enabled)
            .map_err(|error| io::Error::new(io::ErrorKind::PermissionDenied, error.to_string()))?;
        let (snapshot, mut bytes) = if let Some(ref mut probe) = streaming_probe {
            match probe.capture_frame(request.format) {
                Ok(bytes) => {
                    let snapshot = summarize_frame_fast(
                        &bytes,
                        request.format,
                        operation != Operation::StreamFrame,
                    )
                    .map_err(|error| {
                        eprintln!(
                            "onyx-kvm-video-broker: summarize_frame_fast failed: {error:?}"
                        );
                        io::Error::new(io::ErrorKind::InvalidData, error.to_string())
                    })?;
                    (snapshot, bytes)
                }
                Err(err) => {
                    eprintln!(
                        "onyx-kvm-video-broker: streaming probe error: {err}, falling back to oneshot"
                    );
                    streaming_probe = None;
                    capture_frame_payload(
                        config,
                        request.format,
                        operation != Operation::StreamFrame,
                    )?
                }
            }
        } else {
            capture_frame_payload(
                config,
                request.format,
                operation != Operation::StreamFrame,
            )?
        };
        match operation {
            Operation::Snapshot => {
                stream.write_all(&snapshot.encode())?;
            }
            Operation::SnapshotFrame | Operation::StreamFrame => {
                let payload_bytes = u32::try_from(SNAPSHOT_RESPONSE_BYTES + bytes.len())
                    .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "payload too large"))?;
                let header = WireHeader {
                    operation,
                    payload_bytes,
                };
                stream.write_all(&header.encode())?;
                stream.write_all(&snapshot.encode())?;
                stream.write_all(&bytes)?;
            }
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "unsupported operation",
                ));
            }
        }
        bytes.fill(0);
        stream.flush()?;
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn authenticate_peer(stream: &UnixStream) -> io::Result<()> {
    let credentials = rustix::net::sockopt::socket_peercred(stream).map_err(io::Error::from)?;
    if credentials.uid.as_raw() != ONYX_KVM_UID {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("peer uid {} is not authorized", credentials.uid.as_raw()),
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

fn resolve_probe_bin(config: &Config) -> &Path {
    if config.probe_bin.exists() {
        config.probe_bin.as_path()
    } else if Path::new("/usr/libexec/cleankvm-raw-frame-probe").exists() {
        Path::new("/usr/libexec/cleankvm-raw-frame-probe")
    } else {
        config.probe_bin.as_path()
    }
}

struct StreamingProbe {
    child: Child,
    stdin: ChildStdin,
    stdout: ChildStdout,
}

impl StreamingProbe {
    fn spawn(probe_bin: &Path) -> io::Result<Self> {
        let mut child = Command::new(probe_bin)
            .args(["0", "1080", "8", "0,0,0,0,0", "2,4,3,1,0", "stream"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| io::Error::other("no stdin for probe"))?;
        let mut stdout = child
            .stdout
            .take()
            .ok_or_else(|| io::Error::other("no stdout for probe"))?;

        let mut ready = [0_u8; 1];
        stdout.read_exact(&mut ready)?;
        if ready[0] != 0 {
            let _ = child.kill();
            let _ = child.wait();
            return Err(io::Error::other("probe returned non-zero ready byte"));
        }
        Ok(Self {
            child,
            stdin,
            stdout,
        })
    }

    fn capture_frame(&mut self, format: onyx_kvm_media_protocol::SnapshotFormat) -> io::Result<Vec<u8>> {
        let cmd = match format {
            onyx_kvm_media_protocol::SnapshotFormat::TileDeltaYuv422 => 2_u8,
            _ => 1_u8,
        };
        self.stdin.write_all(&[cmd])?;
        self.stdin.flush()?;
        let mut header = [0_u8; 5];
        self.stdout.read_exact(&mut header)?;
        if header[0] != 0 {
            return Err(io::Error::other("probe capture error"));
        }
        let length = u32::from_be_bytes([header[1], header[2], header[3], header[4]]) as usize;
        if length > 5 * 1024 * 1024 {
            return Err(io::Error::other("probe returned oversized frame"));
        }
        let mut bytes = vec![0_u8; length];
        self.stdout.read_exact(&mut bytes)?;
        Ok(bytes)
    }
}

impl Drop for StreamingProbe {
    fn drop(&mut self) {
        let _ = self.stdin.write_all(b"Q");
        let _ = self.stdin.flush();
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn capture_frame_payload(
    config: &Config,
    format: onyx_kvm_media_protocol::SnapshotFormat,
    compute_digest: bool,
) -> io::Result<(onyx_kvm_media_protocol::VideoSnapshot, Vec<u8>)> {
    remove_if_exists(&config.frame_path)?;
    remove_if_exists(Path::new("/run/cleankvm-frame.yuv422p"))?;
    let probe_bin = resolve_probe_bin(config);
    let out_path = config
        .frame_path
        .to_str()
        .unwrap_or("/run/cleankvm-frame.yuv422p");
    let status = Command::new(probe_bin)
        .args([
            "0",
            "1080",
            "8",
            "0,0,0,0,0",
            "2,4,3,1,0",
            "oneshot",
            out_path,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    if !status.success() {
        return Err(io::Error::other("raw frame probe exited with failure"));
    }
    let actual_frame_path = if config.frame_path.exists() {
        config.frame_path.as_path()
    } else if Path::new("/run/cleankvm-frame.yuv422p").exists() {
        Path::new("/run/cleankvm-frame.yuv422p")
    } else {
        config.frame_path.as_path()
    };
    let file = File::open(actual_frame_path)?;
    let mut bytes = Vec::with_capacity(EXPECTED_YUV422P_1080P_BYTES);
    file.take((EXPECTED_YUV422P_1080P_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    remove_if_exists(&config.frame_path)?;
    remove_if_exists(Path::new("/run/cleankvm-frame.yuv422p"))?;
    let snapshot = summarize_frame_fast(&bytes, format, compute_digest)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;
    Ok((snapshot, bytes))
}

fn load_quarantine_modules(module_dir: &Path) -> io::Result<()> {
    for (name, params) in LOAD_ORDER {
        let ko_path = module_dir.join(format!("{name}.ko"));
        let status = Command::new("/sbin/insmod")
            .arg(&ko_path)
            .args(*params)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?;
        if !status.success() {
            return Err(io::Error::other(format!("failed to insmod {name}")));
        }
    }
    Ok(())
}

struct ModuleCleanupGuard<'a> {
    frame_path: &'a Path,
}

impl Drop for ModuleCleanupGuard<'_> {
    fn drop(&mut self) {
        let _ = remove_if_exists(self.frame_path);
        for name in UNLOAD_ORDER {
            let _ = Command::new("/sbin/rmmod")
                .arg(name)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
    }
}

fn remove_if_exists(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}
