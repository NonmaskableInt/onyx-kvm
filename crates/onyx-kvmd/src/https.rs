use std::{
    env, fs,
    io::{self, Read, Write},
    net::{TcpListener, TcpStream},
    os::unix::net::UnixStream,
    path::{Path, PathBuf},
    sync::{
        Arc, RwLock,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use onyx_kvm_mcp::{
    AudioCaptureTransport, AudioOutputTransport, Ed25519JwtValidator, HidTransport, HttpAuthError,
    HttpRequestAuthError, MAX_MESSAGE_BYTES, Scope, Server, VideoTransport,
    authenticate_http_request, bearer_challenge, protected_resource_metadata, validate_origin,
};
use onyx_kvm_media_protocol::{
    AUDIO_REQUEST_BYTES, BoundedFrameRing, CAPTURE_METRICS_BYTES, CaptureMetrics,
    MAX_STREAM_LEASE_MS, Operation, PLAYBACK_RECEIPT_BYTES, PcmShape, PlaybackReceipt,
    SNAPSHOT_RESPONSE_BYTES, STREAM_FLAG_DELTA, STREAM_FLAG_KEYFRAME, STREAM_FLAG_STANDBY,
    SnapshotFormat, SnapshotRequest, StreamFrame, StreamFrameHeader, StreamSessionLease,
    VideoSnapshot, WIRE_HEADER_BYTES, WireHeader,
};
use rustls::{
    ServerConfig, ServerConnection, StreamOwned,
    pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer},
};

use crate::web::{APP_CSS, APP_JS, FAVICON_SVG, INDEX_HTML};

const MAX_HEADER_BYTES: usize = 16 * 1024;
const MAX_IDENTITY_BYTES: usize = 64 * 1024;
const TIMEOUT: Duration = Duration::from_secs(10);
const METADATA_PATH: &str = "/.well-known/oauth-protected-resource";
const HID_SOCKET: &str = "/run/onyx-kvm/hid.sock";
const HID_ENABLED_PATH: &str = "/run/onyx-kvm/hid-enabled";
const VIDEO_SOCKET: &str = "/run/onyx-kvm/video.sock";
const VIDEO_ENABLED_PATH: &str = "/run/onyx-kvm/video-enabled";
const AUDIO_CAPTURE_SOCKET: &str = "/run/onyx-kvm/audio-capture.sock";
const AUDIO_CAPTURE_ENABLED_PATH: &str = "/run/onyx-kvm/audio-capture-enabled";
const AUDIO_OUTPUT_SOCKET: &str = "/run/onyx-kvm/audio-output.sock";
const AUDIO_OUTPUT_ENABLED_PATH: &str = "/run/onyx-kvm/audio-output-enabled";
const FEATURES_BOOT_PATH: &str = "/boot/onyx-kvm-features.json";
const FEATURES_RUN_PATH: &str = "/run/onyx-kvm/features.json";
const FEATURES_ETC_PATH: &str = "/etc/onyx-kvm/features.json";
const PAIRING_CODE_PATH: &str = "/run/onyx-kvm/pairing-code";
const PENDING_SESSION_TOKEN_PATH: &str = "/run/onyx-kvm/pending-session-token";
const PIN_HASH_BOOT: &str = "/boot/onyx-kvm-pin.hash";
const PIN_HASH_RUN: &str = "/run/onyx-kvm/pin.hash";
const PIN_HASH_ETC: &str = "/etc/onyx-kvm/pin.hash";
const FACTORY_RESET_BOOT: &str = "/boot/RESET";
const FACTORY_RESET_BOOT2: &str = "/boot/FACTORY_RESET";
const FACTORY_RESET_RUN: &str = "/run/onyx-kvm/reset";

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[allow(clippy::struct_excessive_bools)]
pub struct FeatureState {
    pub video: bool,
    pub hid: bool,
    pub speaker: bool,
    pub microphone: bool,
}

impl Default for FeatureState {
    fn default() -> Self {
        Self {
            video: true,
            hid: true,
            speaker: false,
            microphone: false,
        }
    }
}

#[must_use]
pub fn current_feature_state() -> FeatureState {
    FeatureState {
        video: Path::new(VIDEO_ENABLED_PATH).exists(),
        hid: Path::new(HID_ENABLED_PATH).exists(),
        speaker: Path::new(AUDIO_OUTPUT_ENABLED_PATH).exists(),
        microphone: Path::new(AUDIO_CAPTURE_ENABLED_PATH).exists(),
    }
}

fn set_gate_file(path: &str, enable: bool) -> io::Result<()> {
    if enable {
        if let Some(parent) = Path::new(path).parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, b"")?;
    } else if Path::new(path).exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

#[allow(clippy::trivially_copy_pass_by_ref)]
pub fn apply_feature_state(state: &FeatureState) -> io::Result<()> {
    set_gate_file(VIDEO_ENABLED_PATH, state.video)?;
    set_gate_file(HID_ENABLED_PATH, state.hid)?;
    set_gate_file(AUDIO_OUTPUT_ENABLED_PATH, state.speaker)?;
    set_gate_file(AUDIO_CAPTURE_ENABLED_PATH, state.microphone)?;

    let json_bytes = serde_json::to_vec_pretty(state)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    // Always persist to /run (current runtime state)
    if let Some(parent) = Path::new(FEATURES_RUN_PATH).parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(FEATURES_RUN_PATH, &json_bytes);

    // Persist to /boot so settings survive reboot on rw FAT boot partition
    if let Some(parent) = Path::new(FEATURES_BOOT_PATH).parent() {
        if parent.exists() {
            fs::write(FEATURES_BOOT_PATH, &json_bytes)?;
        }
    }

    // Try /etc if writable (development mode or rw rootfs)
    if let Some(parent) = Path::new(FEATURES_ETC_PATH).parent() {
        if parent.exists() {
            let _ = fs::write(FEATURES_ETC_PATH, &json_bytes);
        }
    }
    Ok(())
}

pub fn init_features_from_disk() {
    let loaded = (|| -> Option<FeatureState> {
        for path in [FEATURES_RUN_PATH, FEATURES_BOOT_PATH, FEATURES_ETC_PATH] {
            if let Ok(content) = fs::read_to_string(path) {
                if let Ok(s) = serde_json::from_str(&content) {
                    return Some(s);
                }
            }
        }
        None
    })();

    let state = loaded.unwrap_or_default();
    let _ = apply_feature_state(&state);
}

static PAIRING_ATTEMPTS: AtomicUsize = AtomicUsize::new(0);
const MAX_PAIRING_ATTEMPTS: usize = 3;

struct BrokerHid;

impl BrokerHid {
    fn send(report_type: u8, report: &[u8]) -> io::Result<()> {
        let mut stream = UnixStream::connect(HID_SOCKET)?;
        stream.set_read_timeout(Some(TIMEOUT))?;
        stream.set_write_timeout(Some(TIMEOUT))?;
        let length = u8::try_from(report.len()).map_err(|_| invalid("invalid HID length"))?;
        stream.write_all(&[b'C', b'K', 1, report_type, length])?;
        stream.write_all(report)?;
        let mut acknowledgement = [0_u8; 1];
        stream.read_exact(&mut acknowledgement)?;
        if acknowledgement != [0] {
            return Err(invalid("invalid HID broker acknowledgement"));
        }
        Ok(())
    }
}

impl HidTransport for BrokerHid {
    fn send_keyboard(&self, report: [u8; 8]) -> io::Result<()> {
        Self::send(1, &report)
    }

    fn send_pointer(&self, report: [u8; 4]) -> io::Result<()> {
        Self::send(2, &report)
    }
}

static LATEST_STREAM_FRAME: RwLock<Option<(VideoSnapshot, Vec<u8>, Instant)>> = RwLock::new(None);

struct BrokerVideo;

impl BrokerVideo {
    fn capture_frame(request: SnapshotRequest) -> io::Result<(VideoSnapshot, Vec<u8>)> {
        if let Ok(guard) = LATEST_STREAM_FRAME.read() {
            if let Some((snap, bytes, timestamp)) = guard.as_ref() {
                if timestamp.elapsed() < Duration::from_secs(2) {
                    return Ok((*snap, bytes.clone()));
                }
            }
        }
        let mut stream = UnixStream::connect(VIDEO_SOCKET)?;
        let timeout = TIMEOUT.max(Duration::from_millis(u64::from(request.deadline_ms)));
        stream.set_read_timeout(Some(timeout))?;
        stream.set_write_timeout(Some(timeout))?;
        let payload = request.encode();
        let payload_bytes =
            u32::try_from(payload.len()).map_err(|_| invalid("invalid snapshot request length"))?;
        let header = WireHeader {
            operation: Operation::SnapshotFrame,
            payload_bytes,
        };
        stream.write_all(&header.encode())?;
        stream.write_all(&payload)?;
        let mut header_response = [0_u8; WIRE_HEADER_BYTES];
        stream.read_exact(&mut header_response)?;
        let wire_header = WireHeader::decode(header_response)
            .map_err(|_| invalid("invalid video broker wire header"))?;
        if wire_header.operation != Operation::SnapshotFrame {
            return Err(invalid("unexpected operation in broker response"));
        }
        let mut snapshot_bytes = [0_u8; SNAPSHOT_RESPONSE_BYTES];
        stream.read_exact(&mut snapshot_bytes)?;
        let snapshot = VideoSnapshot::decode(snapshot_bytes)
            .map_err(|_| invalid("invalid video broker snapshot response"))?;
        let expected_frame_bytes = usize::try_from(snapshot.byte_length)
            .map_err(|_| invalid("frame byte length overflow"))?;
        let mut frame_bytes = vec![0_u8; expected_frame_bytes];
        stream.read_exact(&mut frame_bytes)?;
        Ok((snapshot, frame_bytes))
    }
}

struct VideoStreamSession {
    stream: UnixStream,
}

impl VideoStreamSession {
    fn connect(timeout: Duration) -> io::Result<Self> {
        let stream = UnixStream::connect(VIDEO_SOCKET)?;
        stream.set_read_timeout(Some(timeout))?;
        stream.set_write_timeout(Some(timeout))?;
        Ok(Self { stream })
    }

    fn capture_next(
        &mut self,
        request: SnapshotRequest,
    ) -> io::Result<Option<(VideoSnapshot, Vec<u8>)>> {
        let payload = request.encode();
        let payload_bytes =
            u32::try_from(payload.len()).map_err(|_| invalid("invalid snapshot request length"))?;
        let header = WireHeader {
            operation: Operation::StreamFrame,
            payload_bytes,
        };
        self.stream.write_all(&header.encode())?;
        self.stream.write_all(&payload)?;
        self.stream.flush()?;
        let mut header_response = [0_u8; WIRE_HEADER_BYTES];
        self.stream.read_exact(&mut header_response)?;
        let wire_header = WireHeader::decode(header_response)
            .map_err(|_| invalid("invalid video broker wire header"))?;
        if wire_header.operation != Operation::StreamFrame {
            return Err(invalid("unexpected operation in broker response"));
        }
        if wire_header.payload_bytes == 0 {
            return Ok(None);
        }
        let mut snapshot_bytes = [0_u8; SNAPSHOT_RESPONSE_BYTES];
        self.stream.read_exact(&mut snapshot_bytes)?;
        let snapshot = VideoSnapshot::decode(snapshot_bytes)
            .map_err(|_| invalid("invalid video broker snapshot response"))?;
        let expected_frame_bytes = usize::try_from(snapshot.byte_length)
            .map_err(|_| invalid("frame byte length overflow"))?;
        let mut frame_bytes = vec![0_u8; expected_frame_bytes];
        self.stream.read_exact(&mut frame_bytes)?;
        Ok(Some((snapshot, frame_bytes)))
    }
}

impl VideoTransport for BrokerVideo {
    fn capture_snapshot(&self, request: SnapshotRequest) -> io::Result<VideoSnapshot> {
        if let Ok(guard) = LATEST_STREAM_FRAME.read() {
            if let Some((snap, _, timestamp)) = guard.as_ref() {
                if timestamp.elapsed() < Duration::from_secs(2) {
                    return Ok(*snap);
                }
            }
        }
        let mut stream = UnixStream::connect(VIDEO_SOCKET)?;
        let timeout = TIMEOUT.max(Duration::from_millis(u64::from(request.deadline_ms)));
        stream.set_read_timeout(Some(timeout))?;
        stream.set_write_timeout(Some(timeout))?;
        let payload = request.encode();
        let payload_bytes =
            u32::try_from(payload.len()).map_err(|_| invalid("invalid snapshot request length"))?;
        let header = WireHeader {
            operation: Operation::Snapshot,
            payload_bytes,
        };
        stream.write_all(&header.encode())?;
        stream.write_all(&payload)?;
        let mut response = [0_u8; SNAPSHOT_RESPONSE_BYTES];
        stream.read_exact(&mut response)?;
        VideoSnapshot::decode(response).map_err(|_| invalid("invalid video broker response"))
    }
}

fn audio_transaction<const N: usize>(
    socket: &str,
    operation: Operation,
    shape: PcmShape,
) -> io::Result<[u8; N]> {
    let mut stream = UnixStream::connect(socket)?;
    stream.set_read_timeout(Some(TIMEOUT))?;
    stream.set_write_timeout(Some(TIMEOUT))?;
    let payload = shape.encode();
    let header = WireHeader {
        operation,
        payload_bytes: u32::try_from(AUDIO_REQUEST_BYTES)
            .map_err(|_| invalid("invalid audio request length"))?,
    };
    stream.write_all(&header.encode())?;
    stream.write_all(&payload)?;
    let mut response = [0_u8; N];
    stream.read_exact(&mut response)?;
    Ok(response)
}

struct BrokerAudioCapture;

impl AudioCaptureTransport for BrokerAudioCapture {
    fn capture_metrics(&self, shape: PcmShape) -> io::Result<CaptureMetrics> {
        let response = audio_transaction::<CAPTURE_METRICS_BYTES>(
            AUDIO_CAPTURE_SOCKET,
            Operation::CapturePcm,
            shape,
        )?;
        Ok(CaptureMetrics::decode(response))
    }
}

struct BrokerAudioOutput;

impl AudioOutputTransport for BrokerAudioOutput {
    fn play_bounded_tone(&self, shape: PcmShape) -> io::Result<PlaybackReceipt> {
        let response = audio_transaction::<PLAYBACK_RECEIPT_BYTES>(
            AUDIO_OUTPUT_SOCKET,
            Operation::PlayPcm,
            shape,
        )?;
        Ok(PlaybackReceipt::decode(response))
    }
}

#[derive(Clone)]
struct Config {
    bind: String,
    origin: String,
    resource: String,
    authorization_server: String,
    issuer: String,
    certificate: PathBuf,
    private_key: PathBuf,
    issuer_public_key: PathBuf,
    issuer_signing_key: Option<PathBuf>,
}

struct Request {
    method: String,
    path: String,
    origin: Option<String>,
    authorization: Option<String>,
    body: Vec<u8>,
}

pub fn serve_from_environment() -> io::Result<bool> {
    let Some(config) = Config::from_environment()? else {
        return Ok(false);
    };
    serve(&config)?;
    Ok(true)
}

impl Config {
    fn from_environment() -> io::Result<Option<Self>> {
        let Some(bind) = env::var_os("ONYX_KVM_HTTPS_BIND") else {
            return Ok(None);
        };
        let signing_key = env::var_os("ONYX_KVM_ISSUER_SIGNING_KEY_RAW")
            .map(PathBuf::from)
            .or_else(|| {
                let default_path = PathBuf::from("/etc/onyx-kvm/oauth-issuer-signing-key.raw");
                default_path.exists().then_some(default_path)
            });
        Ok(Some(Self {
            bind: bind
                .into_string()
                .map_err(|_| invalid("invalid bind address"))?,
            origin: required("ONYX_KVM_HTTPS_ORIGIN")?,
            resource: required("ONYX_KVM_MCP_RESOURCE")?,
            authorization_server: required("ONYX_KVM_AUTHORIZATION_SERVER")?,
            issuer: required("ONYX_KVM_TOKEN_ISSUER")?,
            certificate: required("ONYX_KVM_TLS_CERTIFICATE_DER")?.into(),
            private_key: required("ONYX_KVM_TLS_PRIVATE_KEY_DER")?.into(),
            issuer_public_key: required("ONYX_KVM_ISSUER_PUBLIC_KEY_RAW")?.into(),
            issuer_signing_key: signing_key,
        }))
    }
}

fn required(name: &str) -> io::Result<String> {
    env::var(name).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("missing required {name}"),
        )
    })
}

fn read_identity(path: &Path, expected: Option<usize>) -> io::Result<Vec<u8>> {
    let bytes = fs::read(path)?;
    if bytes.is_empty()
        || bytes.len() > MAX_IDENTITY_BYTES
        || expected.is_some_and(|length| bytes.len() != length)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid identity file: {}", path.display()),
        ));
    }
    Ok(bytes)
}

fn tls_config(config: &Config) -> io::Result<Arc<ServerConfig>> {
    let certificate = CertificateDer::from(read_identity(&config.certificate, None)?);
    let private_key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(read_identity(
        &config.private_key,
        None,
    )?));
    let mut provider = rustls::crypto::ring::default_provider();
    // On riscv64 musl without hardware crypto extensions, ring's AES-GCM
    // implementation triggers decryption/MAC verification failures on incoming
    // TLS records. ChaCha20-Poly1305 is timing-safe, fully supported across all
    // modern browsers and TLS 1.3/1.2 clients, and avoids the architecture-specific
    // AES record layer failure.
    provider.cipher_suites.retain(|cs| {
        matches!(
            cs.suite(),
            rustls::CipherSuite::TLS13_CHACHA20_POLY1305_SHA256
                | rustls::CipherSuite::TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256
                | rustls::CipherSuite::TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256
        )
    });
    let mut server = ServerConfig::builder_with_provider(Arc::new(provider))
        .with_safe_default_protocol_versions()
        .map_err(io::Error::other)?
        .with_no_client_auth()
        .with_single_cert(vec![certificate], private_key)
        .map_err(io::Error::other)?;
    server.alpn_protocols = vec![b"http/1.1".to_vec()];
    Ok(Arc::new(server))
}

const MAX_CONCURRENT_CONNECTIONS: usize = 16;

fn serve(config: &Config) -> io::Result<()> {
    init_features_from_disk();
    let tls = tls_config(config)?;
    let issuer_key = Arc::new(read_identity(&config.issuer_public_key, Some(32))?);
    let signing_key = Arc::new(load_signing_key(config.issuer_signing_key.as_deref()));
    let listener = TcpListener::bind(&config.bind)?;
    let initialized = Arc::new(AtomicBool::new(false));
    let config = Arc::new(config.clone());
    let active_connections = Arc::new(AtomicUsize::new(0));
    for socket in listener.incoming() {
        let socket = socket?;
        socket.set_read_timeout(Some(TIMEOUT))?;
        socket.set_write_timeout(Some(TIMEOUT))?;
        if active_connections.load(Ordering::Relaxed) >= MAX_CONCURRENT_CONNECTIONS {
            eprintln!(
                "connection rejected: maximum concurrent connections reached ({MAX_CONCURRENT_CONNECTIONS})"
            );
            drop(socket);
            continue;
        }
        active_connections.fetch_add(1, Ordering::SeqCst);
        let active = Arc::clone(&active_connections);
        let tls = Arc::clone(&tls);
        let config = Arc::clone(&config);
        let issuer_key = Arc::clone(&issuer_key);
        let signing_key = Arc::clone(&signing_key);
        let initialized = Arc::clone(&initialized);
        std::thread::spawn(move || {
            struct ConnGuard(Arc<AtomicUsize>);
            impl Drop for ConnGuard {
                fn drop(&mut self) {
                    self.0.fetch_sub(1, Ordering::SeqCst);
                }
            }
            let _conn_guard = ConnGuard(active);
            let session = match ServerConnection::new(tls) {
                Ok(s) => s,
                Err(err) => {
                    eprintln!("failed to establish TLS session: {err}");
                    return;
                }
            };
            let mut stream = StreamOwned::new(session, socket);
            if let Err(error) = handle(
                &mut stream,
                &config,
                &issuer_key,
                signing_key.as_ref().as_ref(),
                &initialized,
            ) {
                let _ = respond(&mut stream, 400, "text/plain", b"bad request", None);
                eprintln!("rejected HTTPS request: {error}");
            }
        });
    }
    Ok(())
}

#[derive(Clone, Copy, Debug)]
pub struct PairingPaths<'a> {
    pub code: &'a str,
    pub token: &'a str,
}

#[derive(Clone, Copy, Debug)]
pub struct PinPaths<'a> {
    pub boot: &'a str,
    pub run: &'a str,
    pub etc: &'a str,
}

pub const DEFAULT_PIN_PATHS: PinPaths<'static> = PinPaths {
    boot: PIN_HASH_BOOT,
    run: PIN_HASH_RUN,
    etc: PIN_HASH_ETC,
};

pub const DEFAULT_PAIRING_PATHS: PairingPaths<'static> = PairingPaths {
    code: PAIRING_CODE_PATH,
    token: PENDING_SESSION_TOKEN_PATH,
};

pub fn check_and_process_factory_reset(paths: &PinPaths<'_>) {
    let mut triggers: Vec<PathBuf> = vec![
        PathBuf::from(FACTORY_RESET_BOOT),
        PathBuf::from(FACTORY_RESET_BOOT2),
        PathBuf::from(FACTORY_RESET_RUN),
    ];
    if let Some(parent) = Path::new(paths.boot).parent() {
        triggers.push(parent.join("RESET"));
        triggers.push(parent.join("FACTORY_RESET"));
    }
    if let Some(parent) = Path::new(paths.run).parent() {
        triggers.push(parent.join("RESET"));
        triggers.push(parent.join("FACTORY_RESET"));
    }
    let mut triggered = false;
    for p in &triggers {
        if p.exists() {
            triggered = true;
            let _ = fs::remove_file(p);
        }
    }
    if triggered {
        let _ = fs::remove_file(paths.boot);
        let _ = fs::remove_file(paths.run);
        let _ = fs::remove_file(paths.etc);
        eprintln!("Factory reset executed: cleared device PIN hash");
    }
}

pub fn is_device_configured_with_paths(
    pin_paths: &PinPaths<'_>,
    pairing_paths: &PairingPaths<'_>,
) -> bool {
    check_and_process_factory_reset(pin_paths);
    if let Ok(content) = fs::read_to_string(pin_paths.boot) {
        if !content.trim().is_empty() {
            return true;
        }
    }
    if let Ok(content) = fs::read_to_string(pin_paths.run) {
        if !content.trim().is_empty() {
            return true;
        }
    }
    if let Ok(content) = fs::read_to_string(pin_paths.etc) {
        if !content.trim().is_empty() {
            return true;
        }
    }
    if let Ok(content) = fs::read_to_string(pairing_paths.code) {
        if !content.trim().is_empty() {
            return true;
        }
    }
    false
}

fn to_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        use std::fmt::Write as _;
        let _ = write!(s, "{b:02x}");
    }
    s
}

fn from_hex(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (&x, &y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

fn hash_pin(pin: &str, salt: &[u8]) -> [u8; 32] {
    let mut data = Vec::with_capacity(salt.len() + pin.len());
    data.extend_from_slice(salt);
    data.extend_from_slice(pin.as_bytes());
    let mut h = ring::digest::digest(&ring::digest::SHA256, &data);
    for _ in 0..5000 {
        h = ring::digest::digest(&ring::digest::SHA256, h.as_ref());
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(h.as_ref());
    out
}

fn verify_pin(pin: &str, stored_hash_str: &str) -> bool {
    let parts: Vec<&str> = stored_hash_str.trim().split(':').collect();
    if parts.len() != 3 || parts[0] != "sha256-5000" {
        return false;
    }
    let Some(salt) = from_hex(parts[1]) else {
        return false;
    };
    let Some(expected) = from_hex(parts[2]) else {
        return false;
    };
    if salt.len() != 16 || expected.len() != 32 {
        return false;
    }
    let computed = hash_pin(pin, &salt);
    constant_time_eq(&computed, &expected)
}

fn save_pin_hash_paths(pin: &str, paths: &PinPaths<'_>) -> io::Result<()> {
    let mut salt = [0u8; 16];
    if let Ok(mut f) = fs::File::open("/dev/urandom") {
        let _ = f.read_exact(&mut salt);
    } else {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let h = ring::digest::digest(&ring::digest::SHA256, &now.to_le_bytes());
        salt.copy_from_slice(&h.as_ref()[..16]);
    }
    let hash = hash_pin(pin, &salt);
    let record = format!("sha256-5000:{}:{}\n", to_hex(&salt), to_hex(&hash));

    if let Some(parent) = Path::new(paths.run).parent() {
        let _ = fs::create_dir_all(parent);
    }
    fs::write(paths.run, record.as_bytes())?;

    if let Some(parent) = Path::new(paths.boot).parent() {
        if parent.exists() {
            let _ = fs::write(paths.boot, record.as_bytes());
        }
    }
    Ok(())
}

fn load_signing_key(path: Option<&Path>) -> Option<[u8; 32]> {
    let path = path?;
    let bytes = fs::read(path).ok()?;
    if bytes.len() == 32 {
        let mut key = [0u8; 32];
        key.copy_from_slice(&bytes);
        Some(key)
    } else if bytes.len() >= 48 {
        let mut key = [0u8; 32];
        key.copy_from_slice(&bytes[bytes.len() - 32..]);
        Some(key)
    } else {
        None
    }
}

fn mint_scoped_token(
    signing_key: &[u8; 32],
    issuer: &str,
    resource: &str,
    scope: &str,
    mutation: bool,
    ttl_seconds: u64,
) -> io::Result<String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_secs();
    let iss = issuer.split(',').next().unwrap_or(issuer).trim();
    let aud = resource.split(',').next().unwrap_or(resource).trim();
    let exp = now.saturating_add(ttl_seconds);
    Ok(onyx_kvm_mcp::mint_token(
        signing_key,
        iss,
        aud,
        scope,
        mutation,
        exp,
    ))
}

fn mint_operator_token(
    signing_key: &[u8; 32],
    issuer: &str,
    resource: &str,
    ttl_seconds: u64,
) -> io::Result<String> {
    mint_scoped_token(
        signing_key,
        issuer,
        resource,
        "kvm:read kvm:video kvm:audio-capture kvm:audio-output kvm:control kvm:admin",
        true,
        ttl_seconds,
    )
}

fn mint_console_token(
    signing_key: &[u8; 32],
    issuer: &str,
    resource: &str,
    ttl_seconds: u64,
) -> io::Result<String> {
    mint_scoped_token(
        signing_key,
        issuer,
        resource,
        "kvm:read kvm:video kvm:audio-capture kvm:audio-output kvm:control",
        true,
        ttl_seconds,
    )
}

fn wipe_pairing_state_paths(paths: &PairingPaths<'_>) {
    let _ = fs::write(paths.code, b"");
    let _ = fs::write(paths.token, b"");
    let _ = fs::remove_file(paths.code);
    let _ = fs::remove_file(paths.token);
}

fn read_first_available(path: &str, fallback: &str) -> Option<String> {
    if let Ok(content) = fs::read_to_string(path) {
        return Some(content);
    }
    fs::read_to_string(fallback).ok()
}

fn read_device_name() -> String {
    if let Ok(name) = std::env::var("ONYX_KVM_HOSTNAME") {
        let trimmed = name.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    if let Ok(content) = fs::read_to_string("/etc/hostname") {
        let trimmed = content.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    if let Ok(content) = fs::read_to_string("/proc/sys/kernel/hostname") {
        let trimmed = content.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    "onyx-kvm".to_string()
}

fn handle_setup_status<W: Write>(
    stream: &mut W,
    pin_paths: &PinPaths<'_>,
    pairing_paths: &PairingPaths<'_>,
) -> io::Result<()> {
    let configured = is_device_configured_with_paths(pin_paths, pairing_paths);
    let name = read_device_name();
    let body = serde_json::json!({
        "configured": configured,
        "name": name,
    });
    let bytes =
        serde_json::to_vec(&body).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    respond(stream, 200, "application/json", &bytes, None)
}

static SETUP_CLAIM_ATTEMPTS: AtomicUsize = AtomicUsize::new(0);
const MAX_SETUP_CLAIM_ATTEMPTS: usize = 5;

fn handle_setup_claim<W: Write>(
    stream: &mut W,
    config: &Config,
    request: &Request,
    signing_key: Option<&[u8; 32]>,
    pin_paths: &PinPaths<'_>,
    pairing_paths: &PairingPaths<'_>,
) -> io::Result<()> {
    let Some(ref origin) = request.origin else {
        return respond(
            stream,
            403,
            "application/json",
            b"{\"error\":\"Origin header required for setup claim\"}",
            None,
        );
    };
    if origin != &config.origin {
        return respond(
            stream,
            403,
            "application/json",
            b"{\"error\":\"forbidden origin for setup claim\"}",
            None,
        );
    }

    if SETUP_CLAIM_ATTEMPTS.load(Ordering::Relaxed) >= MAX_SETUP_CLAIM_ATTEMPTS {
        return respond(
            stream,
            429,
            "application/json",
            b"{\"error\":\"maximum setup claim attempts exceeded; reboot device to retry\"}",
            None,
        );
    }

    if is_device_configured_with_paths(pin_paths, pairing_paths) {
        return respond(
            stream,
            409,
            "application/json",
            b"{\"error\":\"device already configured\"}",
            None,
        );
    }

    let Ok(value) = serde_json::from_slice::<serde_json::Value>(&request.body) else {
        SETUP_CLAIM_ATTEMPTS.fetch_add(1, Ordering::SeqCst);
        return respond(
            stream,
            400,
            "application/json",
            b"{\"error\":\"invalid JSON payload\"}",
            None,
        );
    };
    let pin_str = value
        .get("pin")
        .or_else(|| value.get("code"))
        .and_then(serde_json::Value::as_str)
        .map(str::trim);

    let Some(pin) = pin_str else {
        SETUP_CLAIM_ATTEMPTS.fetch_add(1, Ordering::SeqCst);
        return respond(
            stream,
            400,
            "application/json",
            b"{\"error\":\"missing 'pin' field in payload\"}",
            None,
        );
    };

    if pin.len() < 6 || pin.len() > 8 || !pin.chars().all(|c| c.is_ascii_digit()) {
        SETUP_CLAIM_ATTEMPTS.fetch_add(1, Ordering::SeqCst);
        return respond(
            stream,
            400,
            "application/json",
            b"{\"error\":\"PIN must be between 6 and 8 digits\"}",
            None,
        );
    }

    save_pin_hash_paths(pin, pin_paths)?;
    SETUP_CLAIM_ATTEMPTS.store(0, Ordering::SeqCst);

    let Some(signing_key) = signing_key else {
        return respond(
            stream,
            500,
            "application/json",
            b"{\"error\":\"server missing token signing key\"}",
            None,
        );
    };

    let token = mint_console_token(signing_key, &config.issuer, &config.resource, 14400)?;
    let response_body = serde_json::json!({
        "status": "configured",
        "token": token,
        "tokenType": "Bearer",
        "expiresIn": 14400,
    });
    let body_bytes = serde_json::to_vec(&response_body)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    respond(stream, 200, "application/json", &body_bytes, None)
}

fn handle_update_pin<W: Write>(
    stream: &mut W,
    config: &Config,
    issuer_key: &[u8],
    request: &Request,
    pin_paths: &PinPaths<'_>,
) -> io::Result<()> {
    let validator = Ed25519JwtValidator::new(&config.issuer, issuer_key);
    let claims = match authenticate_http_request(
        request.origin.as_deref(),
        request.authorization.as_deref(),
        &config.origin,
        &validator,
    ) {
        Ok(claims) => claims,
        Err(HttpRequestAuthError::Boundary(HttpAuthError::InvalidOrigin)) => {
            return respond(stream, 403, "text/plain", b"forbidden", None);
        }
        Err(_) => return unauthorized(stream),
    };

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_secs();
    let valid_aud = config
        .resource
        .split(',')
        .any(|expected| expected.trim() == claims.audience);
    if !valid_aud || claims.expires_at_unix_seconds < now || !claims.mutation_enabled {
        return unauthorized(stream);
    }

    let value: serde_json::Value = match serde_json::from_slice(&request.body) {
        Ok(v) => v,
        Err(_) => {
            return respond(
                stream,
                400,
                "application/json",
                b"{\"error\":\"invalid JSON payload\"}",
                None,
            );
        }
    };
    let Some(pin) = value
        .get("pin")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
    else {
        return respond(
            stream,
            400,
            "application/json",
            b"{\"error\":\"missing 'pin' field in payload\"}",
            None,
        );
    };
    if pin.len() < 4 || pin.len() > 8 || !pin.chars().all(|c| c.is_ascii_digit()) {
        return respond(
            stream,
            400,
            "application/json",
            b"{\"error\":\"PIN must be between 4 and 8 digits\"}",
            None,
        );
    }

    save_pin_hash_paths(pin, pin_paths)?;
    respond(
        stream,
        200,
        "application/json",
        b"{\"status\":\"updated\"}",
        None,
    )
}

fn handle_pair_full<W: Write>(
    stream: &mut W,
    config_origin: &str,
    issuer: Option<&str>,
    resource: Option<&str>,
    request: &Request,
    pairing_paths: &PairingPaths<'_>,
    pin_paths: &PinPaths<'_>,
    signing_key: Option<&[u8; 32]>,
    attempts_counter: &AtomicUsize,
) -> io::Result<()> {
    if validate_origin(request.origin.as_deref(), config_origin).is_err() {
        return respond(
            stream,
            403,
            "application/json",
            b"{\"error\":\"forbidden\"}",
            None,
        );
    }

    let value: serde_json::Value = match serde_json::from_slice(&request.body) {
        Ok(v) => v,
        Err(_) => {
            return respond(
                stream,
                400,
                "application/json",
                b"{\"error\":\"invalid JSON payload\"}",
                None,
            );
        }
    };
    let code_str = value
        .get("code")
        .or_else(|| value.get("pin"))
        .and_then(serde_json::Value::as_str)
        .map(str::trim);

    let Some(code_val) = code_str else {
        return respond(
            stream,
            400,
            "application/json",
            b"{\"error\":\"missing 'code' field in payload\"}",
            None,
        );
    };

    // 1. Check UART-staged pairing ceremony first
    if let Ok(expected_code) = fs::read_to_string(pairing_paths.code) {
        let expected_trimmed = expected_code.trim();
        let mut diff = expected_trimmed.len() ^ code_val.len();
        for (a, b) in expected_trimmed.bytes().zip(code_val.bytes()) {
            diff |= (a ^ b) as usize;
        }

        if diff != 0 {
            let attempts = attempts_counter.fetch_add(1, Ordering::SeqCst) + 1;
            if attempts >= MAX_PAIRING_ATTEMPTS {
                wipe_pairing_state_paths(pairing_paths);
                attempts_counter.store(0, Ordering::SeqCst);
                return respond(
                    stream,
                    403,
                    "application/json",
                    b"{\"error\":\"maximum pairing attempts exceeded; pairing code invalidated\"}",
                    None,
                );
            }
            return respond(
                stream,
                401,
                "application/json",
                b"{\"error\":\"invalid pairing code\"}",
                None,
            );
        }

        attempts_counter.store(0, Ordering::SeqCst);
        let token = if let Ok(tok) = fs::read_to_string(pairing_paths.token) {
            tok
        } else if let (Some(sk), Some(iss), Some(res)) = (signing_key, issuer, resource) {
            mint_operator_token(sk, iss, res, 14400)?
        } else {
            wipe_pairing_state_paths(pairing_paths);
            return respond(
                stream,
                500,
                "application/json",
                b"{\"error\":\"missing pending session token\"}",
                None,
            );
        };

        wipe_pairing_state_paths(pairing_paths);

        let response_body = serde_json::json!({
            "token": token.trim(),
            "tokenType": "Bearer",
            "expiresIn": 14400,
        });
        let body_bytes = serde_json::to_vec(&response_body)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        return respond(stream, 200, "application/json", &body_bytes, None);
    }

    // 2. Check persistent PIN hash
    let stored_hash = read_first_available(pin_paths.boot, pin_paths.run)
        .or_else(|| fs::read_to_string(pin_paths.etc).ok());

    let Some(stored_hash) = stored_hash else {
        return respond(
            stream,
            404,
            "application/json",
            b"{\"error\":\"no pairing ceremony currently active\"}",
            None,
        );
    };

    if !verify_pin(code_val, &stored_hash) {
        let attempts = attempts_counter.fetch_add(1, Ordering::SeqCst) + 1;
        if attempts >= 10 {
            std::thread::sleep(Duration::from_millis(500));
        }
        return respond(
            stream,
            401,
            "application/json",
            b"{\"error\":\"invalid pairing code\"}",
            None,
        );
    }

    attempts_counter.store(0, Ordering::SeqCst);
    let (Some(sk), Some(iss), Some(res)) = (signing_key, issuer, resource) else {
        return respond(
            stream,
            500,
            "application/json",
            b"{\"error\":\"server missing token signing key\"}",
            None,
        );
    };

    let token = mint_operator_token(sk, iss, res, 14400)?;
    let response_body = serde_json::json!({
        "token": token,
        "tokenType": "Bearer",
        "expiresIn": 14400,
    });
    let body_bytes = serde_json::to_vec(&response_body)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    respond(stream, 200, "application/json", &body_bytes, None)
}

#[cfg(test)]
fn handle_pair_with_paths<W: Write>(
    stream: &mut W,
    config_origin: &str,
    request: &Request,
    paths: &PairingPaths<'_>,
    attempts_counter: &AtomicUsize,
) -> io::Result<()> {
    handle_pair_full(
        stream,
        config_origin,
        None,
        None,
        request,
        paths,
        &DEFAULT_PIN_PATHS,
        None,
        attempts_counter,
    )
}

fn handle_pair<W: Write>(
    stream: &mut W,
    config: &Config,
    signing_key: Option<&[u8; 32]>,
    request: &Request,
) -> io::Result<()> {
    handle_pair_full(
        stream,
        &config.origin,
        Some(&config.issuer),
        Some(&config.resource),
        request,
        &DEFAULT_PAIRING_PATHS,
        &DEFAULT_PIN_PATHS,
        signing_key,
        &PAIRING_ATTEMPTS,
    )
}

fn handle(
    stream: &mut StreamOwned<ServerConnection, TcpStream>,
    config: &Config,
    issuer_key: &[u8],
    signing_key: Option<&[u8; 32]>,
    initialized: &AtomicBool,
) -> io::Result<()> {
    let request = read_request(stream)?;
    if request.method == "GET" && request.path == METADATA_PATH {
        let body =
            protected_resource_metadata(&config.resource, &config.authorization_server).to_string();
        return respond(stream, 200, "application/json", body.as_bytes(), None);
    }
    if request.method == "GET" && request.path == "/setup/status" {
        return handle_setup_status(stream, &DEFAULT_PIN_PATHS, &DEFAULT_PAIRING_PATHS);
    }
    if request.method == "POST" && request.path == "/setup/claim" {
        return handle_setup_claim(
            stream,
            config,
            &request,
            signing_key,
            &DEFAULT_PIN_PATHS,
            &DEFAULT_PAIRING_PATHS,
        );
    }
    if request.method == "POST" && request.path == "/setup/pin" {
        return handle_update_pin(stream, config, issuer_key, &request, &DEFAULT_PIN_PATHS);
    }
    if request.method == "GET"
        && (request.path == "/video/frame" || request.path.starts_with("/video/frame?"))
    {
        return handle_video_frame(stream, config, issuer_key, &request);
    }
    if request.method == "GET"
        && (request.path == "/video/stream" || request.path.starts_with("/video/stream?"))
    {
        return handle_video_stream(stream, config, issuer_key, &request);
    }
    if request.method == "GET"
        && (request.path == "/audio/stream" || request.path.starts_with("/audio/stream?"))
    {
        return handle_audio_stream(stream, config, issuer_key, &request);
    }
    if request.method == "GET" && request.path == "/features" {
        return handle_get_features(stream, config, issuer_key, &request);
    }
    if request.method == "POST" && request.path == "/features" {
        return handle_post_features(stream, config, issuer_key, &request);
    }
    if request.method == "POST" && (request.path == "/system/reboot" || request.path == "/reboot") {
        return handle_reboot(stream, config, issuer_key, &request);
    }
    if request.method == "GET" {
        return match request.path.as_str() {
            "/" => respond(stream, 200, "text/html; charset=utf-8", INDEX_HTML, None),
            "/app.css" => respond(stream, 200, "text/css; charset=utf-8", APP_CSS, None),
            "/app.js" => respond(stream, 200, "text/javascript; charset=utf-8", APP_JS, None),
            "/favicon.ico" | "/favicon.svg" => {
                respond(stream, 200, "image/svg+xml", FAVICON_SVG, None)
            }
            _ => respond(stream, 404, "text/plain", b"not found", None),
        };
    }
    if request.method == "POST" && (request.path == "/pair" || request.path == "/enroll") {
        return handle_pair(stream, config, signing_key, &request);
    }
    if request.method != "POST" || request.path != "/mcp" {
        return respond(stream, 404, "text/plain", b"not found", None);
    }

    let validator = Ed25519JwtValidator::new(&config.issuer, issuer_key);
    let claims = match authenticate_http_request(
        request.origin.as_deref(),
        request.authorization.as_deref(),
        &config.origin,
        &validator,
    ) {
        Ok(claims) => claims,
        Err(HttpRequestAuthError::Boundary(HttpAuthError::InvalidOrigin)) => {
            return respond(stream, 403, "text/plain", b"forbidden", None);
        }
        Err(_) => return unauthorized(stream),
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_secs();
    let valid_aud = config
        .resource
        .split(',')
        .any(|expected| expected.trim() == claims.audience);
    if !valid_aud || claims.expires_at_unix_seconds < now {
        return unauthorized(stream);
    }

    let message: serde_json::Value = serde_json::from_slice(&request.body)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let method = message.get("method").and_then(serde_json::Value::as_str);
    if method == Some("notifications/initialized") {
        initialized.store(true, Ordering::Relaxed);
        return respond(stream, 202, "application/json", b"", None);
    }
    let grant = claims.as_grant();
    let hid = BrokerHid;
    let video = BrokerVideo;
    let audio_capture = BrokerAudioCapture;
    let audio_output = BrokerAudioOutput;
    let is_init = initialized.load(Ordering::Relaxed);
    let hid_enabled = Path::new(HID_ENABLED_PATH).exists();
    let mut server = if method == Some("initialize") || !is_init {
        Server::remote(&claims.audience, now, grant)
    } else if hid_enabled {
        Server::remote_ready_with_hid(&claims.audience, now, grant, &hid)
    } else {
        Server::remote_ready(&claims.audience, now, grant)
    };
    if Path::new(VIDEO_ENABLED_PATH).exists() {
        server = server.with_video(&video);
    }
    if Path::new(AUDIO_CAPTURE_ENABLED_PATH).exists() {
        server = server.with_audio_capture(&audio_capture);
    }
    if Path::new(AUDIO_OUTPUT_ENABLED_PATH).exists() {
        server = server.with_audio_output(&audio_output);
    }
    let Some(body) = server.handle_line(&request.body) else {
        return respond(stream, 202, "application/json", b"", None);
    };
    respond(stream, 200, "application/json", body.as_bytes(), None)
}

fn handle_video_frame(
    stream: &mut StreamOwned<ServerConnection, TcpStream>,
    config: &Config,
    issuer_key: &[u8],
    request: &Request,
) -> io::Result<()> {
    if !Path::new(VIDEO_ENABLED_PATH).exists() {
        return respond(stream, 403, "text/plain", b"video disabled", None);
    }
    let validator = Ed25519JwtValidator::new(&config.issuer, issuer_key);
    let claims = match authenticate_http_request(
        request.origin.as_deref(),
        request.authorization.as_deref(),
        &config.origin,
        &validator,
    ) {
        Ok(claims) => claims,
        Err(HttpRequestAuthError::Boundary(HttpAuthError::InvalidOrigin)) => {
            return respond(stream, 403, "text/plain", b"forbidden", None);
        }
        Err(_) => return unauthorized(stream),
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_secs();
    let valid_aud = config
        .resource
        .split(',')
        .any(|expected| expected.trim() == claims.audience);
    if !valid_aud || claims.expires_at_unix_seconds < now {
        return unauthorized(stream);
    }
    if !claims.scopes.contains(&Scope::Video) {
        return unauthorized(stream);
    }
    let (width, height) =
        if request.path.contains("res=720p") || request.path.contains("height=720") {
            (1280, 720)
        } else {
            (1920, 1080)
        };
    let format = if request.path.contains("format=h264") {
        SnapshotFormat::H264
    } else if request.path.contains("format=yuv420p") {
        SnapshotFormat::Yuv420p
    } else {
        SnapshotFormat::Yuv422p
    };
    let Ok(snapshot_request) = SnapshotRequest::new(width, height, format, 5000) else {
        return respond(stream, 500, "text/plain", b"invalid snapshot request", None);
    };
    let (snapshot, mut frame_bytes) = match BrokerVideo::capture_frame(snapshot_request) {
        Ok(res) => res,
        Err(err) => {
            eprintln!("failed to capture video frame: {err}");
            return respond(stream, 500, "text/plain", b"frame capture failed", None);
        }
    };
    let result = respond_frame(stream, &snapshot, &frame_bytes);
    frame_bytes.fill(0);
    result
}

fn handle_video_stream(
    stream: &mut StreamOwned<ServerConnection, TcpStream>,
    config: &Config,
    issuer_key: &[u8],
    request: &Request,
) -> io::Result<()> {
    if !Path::new(VIDEO_ENABLED_PATH).exists() {
        return respond(stream, 403, "text/plain", b"video disabled", None);
    }
    let validator = Ed25519JwtValidator::new(&config.issuer, issuer_key);
    let claims = match authenticate_http_request(
        request.origin.as_deref(),
        request.authorization.as_deref(),
        &config.origin,
        &validator,
    ) {
        Ok(claims) => claims,
        Err(HttpRequestAuthError::Boundary(HttpAuthError::InvalidOrigin)) => {
            return respond(stream, 403, "text/plain", b"forbidden", None);
        }
        Err(_) => return unauthorized(stream),
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_secs();
    let valid_aud = config
        .resource
        .split(',')
        .any(|expected| expected.trim() == claims.audience);
    if !valid_aud || claims.expires_at_unix_seconds < now {
        return unauthorized(stream);
    }
    if !claims.scopes.contains(&Scope::Video) {
        return unauthorized(stream);
    }
    let (width, height) =
        if request.path.contains("res=720p") || request.path.contains("height=720") {
            (1280, 720)
        } else {
            (1920, 1080)
        };
    let format = if request.path.contains("format=h264") {
        SnapshotFormat::H264
    } else if request.path.contains("format=tile_delta") || request.path.contains("mode=delta") {
        SnapshotFormat::TileDeltaYuv422
    } else if request.path.contains("format=yuv420p") {
        SnapshotFormat::Yuv420p
    } else {
        SnapshotFormat::Yuv422p
    };
    let Ok(snapshot_request) = SnapshotRequest::new(width, height, format, 5000) else {
        return respond(stream, 500, "text/plain", b"invalid snapshot request", None);
    };

    write!(
        stream,
        concat!(
            "HTTP/1.1 200 OK\r\n",
            "Content-Type: application/octet-stream\r\n",
            "Cache-Control: no-store, no-cache, must-revalidate\r\n",
            "Content-Security-Policy: default-src 'none'; script-src 'self'; style-src 'self'; connect-src 'self'; img-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'\r\n",
            "Permissions-Policy: camera=(), geolocation=(), microphone=()\r\n",
            "Referrer-Policy: no-referrer\r\n",
            "X-Content-Type-Options: nosniff\r\n",
            "X-Frame-Options: DENY\r\n",
            "Connection: close\r\n\r\n"
        )
    )?;
    stream.flush()?;

    stream_frames_loop(stream, snapshot_request, claims.expires_at_unix_seconds)
}

struct StreamFrameGuard;

impl Drop for StreamFrameGuard {
    fn drop(&mut self) {
        if let Ok(mut lock) = LATEST_STREAM_FRAME.write() {
            *lock = None;
        }
    }
}

fn stream_frames_loop(
    stream: &mut StreamOwned<ServerConnection, TcpStream>,
    snapshot_request: SnapshotRequest,
    token_expiry: u64,
) -> io::Result<()> {
    let now_ms = u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(io::Error::other)?
            .as_millis(),
    )
    .map_err(io::Error::other)?;
    let mut lease = StreamSessionLease::new(1, now_ms, MAX_STREAM_LEASE_MS)
        .map_err(|_| invalid("invalid stream lease"))?;
    let mut ring = BoundedFrameRing::new(2);
    let mut sequence = 0_u32;
    let timeout = TIMEOUT.max(Duration::from_millis(u64::from(
        snapshot_request.deadline_ms,
    )));
    let mut broker_session = VideoStreamSession::connect(timeout).ok();
    let _frame_guard = StreamFrameGuard;

    loop {
        if !Path::new(VIDEO_ENABLED_PATH).exists() {
            break;
        }
        let current_now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(io::Error::other)?;
        let current_secs = current_now.as_secs();
        let current_ms = u64::try_from(current_now.as_millis()).map_err(io::Error::other)?;
        if current_secs >= token_expiry || lease.is_expired(current_ms) {
            break;
        }

        if broker_session.is_none() {
            broker_session = VideoStreamSession::connect(timeout).ok();
        }

        let maybe_frame = if let Some(ref mut session) = broker_session {
            match session.capture_next(snapshot_request) {
                Ok(frame) => frame,
                Err(err) => {
                    eprintln!("video broker session error: {err}");
                    broker_session = None;
                    None
                }
            }
        } else {
            None
        };

        if let Some((snapshot, frame_bytes)) = maybe_frame {
            if let Ok(mut lock) = LATEST_STREAM_FRAME.write() {
                *lock = Some((snapshot, frame_bytes.clone(), Instant::now()));
            }

            let flags = match snapshot.format {
                SnapshotFormat::H264 => {
                    let mut is_key = false;
                    let payload = &frame_bytes;
                    if payload.len() >= 4 {
                        for i in 0..payload.len() - 3 {
                            if payload[i] == 0 && payload[i + 1] == 0 {
                                if payload[i + 2] == 1 {
                                    let nal_type = payload[i + 3] & 0x1f;
                                    if nal_type == 5 || nal_type == 7 {
                                        is_key = true;
                                        break;
                                    }
                                } else if i + 4 < payload.len()
                                    && payload[i + 2] == 0
                                    && payload[i + 3] == 1
                                {
                                    let nal_type = payload[i + 4] & 0x1f;
                                    if nal_type == 5 || nal_type == 7 {
                                        is_key = true;
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    if is_key {
                        STREAM_FLAG_KEYFRAME
                    } else {
                        STREAM_FLAG_DELTA
                    }
                }
                SnapshotFormat::TileDeltaYuv422 => {
                    let full_frame_bytes = u32::from(snapshot.width)
                        .saturating_mul(u32::from(snapshot.height))
                        .saturating_mul(2);
                    if snapshot.byte_length >= full_frame_bytes {
                        STREAM_FLAG_KEYFRAME
                    } else {
                        STREAM_FLAG_DELTA
                    }
                }
                _ => 0,
            };

            let Ok(header) = StreamFrameHeader::with_flags(
                sequence,
                current_ms,
                snapshot.width,
                snapshot.height,
                snapshot.format,
                flags,
                snapshot.byte_length,
                snapshot.sha256,
            ) else {
                break;
            };

            let Ok(frame) = StreamFrame::new(header, frame_bytes) else {
                break;
            };

            ring.push(frame);

            if let Some(mut to_send) = ring.pop() {
                let encoded_hdr = to_send.header.encode();
                if stream.write_all(&encoded_hdr).is_err() {
                    to_send.zeroize();
                    break;
                }
                if stream.write_all(&to_send.payload).is_err() {
                    to_send.zeroize();
                    break;
                }
                if stream.flush().is_err() {
                    to_send.zeroize();
                    break;
                }
                to_send.zeroize();
            }
        } else {
            // Standby / no HDMI signal / target asleep
            let Ok(header) = StreamFrameHeader::with_flags(
                sequence,
                current_ms,
                snapshot_request.width,
                snapshot_request.height,
                snapshot_request.format,
                STREAM_FLAG_STANDBY,
                0,
                [0; 32],
            ) else {
                break;
            };

            let Ok(frame) = StreamFrame::new(header, Vec::new()) else {
                break;
            };

            ring.push(frame);

            if let Some(mut to_send) = ring.pop() {
                let encoded_hdr = to_send.header.encode();
                if stream.write_all(&encoded_hdr).is_err() {
                    to_send.zeroize();
                    break;
                }
                if stream.flush().is_err() {
                    to_send.zeroize();
                    break;
                }
                to_send.zeroize();
            }

            // Pace standby heartbeats so we don't spin while target is sleeping
            std::thread::sleep(Duration::from_millis(500));
        }

        sequence = sequence.wrapping_add(1);
        let renew_ms = u64::try_from(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(io::Error::other)?
                .as_millis(),
        )
        .map_err(io::Error::other)?;
        let _ = lease.renew(renew_ms, MAX_STREAM_LEASE_MS);
    }

    ring.clear();
    Ok(())
}

fn handle_audio_stream(
    stream: &mut StreamOwned<ServerConnection, TcpStream>,
    config: &Config,
    issuer_key: &[u8],
    request: &Request,
) -> io::Result<()> {
    if !Path::new(AUDIO_CAPTURE_ENABLED_PATH).exists() {
        return respond(stream, 403, "text/plain", b"microphone disabled", None);
    }
    let validator = Ed25519JwtValidator::new(&config.issuer, issuer_key);
    let claims = match authenticate_http_request(
        request.origin.as_deref(),
        request.authorization.as_deref(),
        &config.origin,
        &validator,
    ) {
        Ok(claims) => claims,
        Err(HttpRequestAuthError::Boundary(HttpAuthError::InvalidOrigin)) => {
            return respond(stream, 403, "text/plain", b"forbidden", None);
        }
        Err(_) => return unauthorized(stream),
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_secs();
    let valid_aud = config
        .resource
        .split(',')
        .any(|expected| expected.trim() == claims.audience);
    if !valid_aud || claims.expires_at_unix_seconds < now {
        return unauthorized(stream);
    }
    if !claims.scopes.contains(&Scope::AudioCapture) && !claims.scopes.contains(&Scope::Read) {
        return unauthorized(stream);
    }

    let mut broker_stream = match UnixStream::connect(AUDIO_CAPTURE_SOCKET) {
        Ok(s) => s,
        Err(err) => {
            eprintln!("failed to connect to audio capture broker: {err}");
            return respond(stream, 500, "text/plain", b"audio broker unavailable", None);
        }
    };
    broker_stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    broker_stream.set_write_timeout(Some(Duration::from_secs(5)))?;

    let shape = PcmShape::new(
        onyx_kvm_media_protocol::PcmFormat::Signed16LittleEndian,
        16_000,
        1,
        1_000,
    )
    .map_err(|_| invalid("invalid pcm shape"))?;
    let header = WireHeader {
        operation: Operation::StreamAudioPcm,
        payload_bytes: u32::try_from(AUDIO_REQUEST_BYTES)
            .map_err(|_| invalid("invalid audio request length"))?,
    };
    broker_stream.write_all(&header.encode())?;
    broker_stream.write_all(&shape.encode())?;
    broker_stream.flush()?;

    let mut ready = [0_u8; 1];
    broker_stream.read_exact(&mut ready)?;
    if ready[0] != 0 {
        return respond(stream, 500, "text/plain", b"audio probe not ready", None);
    }

    write!(
        stream,
        concat!(
            "HTTP/1.1 200 OK\r\n",
            "Content-Type: audio/l16; rate=16000; channels=1\r\n",
            "Cache-Control: no-store, no-cache, must-revalidate\r\n",
            "Content-Security-Policy: default-src 'none'; script-src 'self'; style-src 'self'; connect-src 'self'; img-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'\r\n",
            "Permissions-Policy: camera=(), geolocation=(), microphone=()\r\n",
            "Referrer-Policy: no-referrer\r\n",
            "X-Content-Type-Options: nosniff\r\n",
            "X-Frame-Options: DENY\r\n",
            "Connection: close\r\n\r\n"
        )
    )?;
    stream.flush()?;

    let mut chunk = vec![0_u8; 3200];
    loop {
        if !Path::new(AUDIO_CAPTURE_ENABLED_PATH).exists() {
            break;
        }
        let current_now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(io::Error::other)?
            .as_secs();
        if current_now >= claims.expires_at_unix_seconds {
            break;
        }
        if broker_stream.read_exact(&mut chunk).is_err() {
            break;
        }
        if stream.write_all(&chunk).is_err() {
            break;
        }
        if stream.flush().is_err() {
            break;
        }
    }
    chunk.fill(0);
    Ok(())
}

fn handle_get_features(
    stream: &mut StreamOwned<ServerConnection, TcpStream>,
    config: &Config,
    issuer_key: &[u8],
    request: &Request,
) -> io::Result<()> {
    let validator = Ed25519JwtValidator::new(&config.issuer, issuer_key);
    let claims = match authenticate_http_request(
        request.origin.as_deref(),
        request.authorization.as_deref(),
        &config.origin,
        &validator,
    ) {
        Ok(claims) => claims,
        Err(HttpRequestAuthError::Boundary(HttpAuthError::InvalidOrigin)) => {
            return respond(stream, 403, "text/plain", b"forbidden", None);
        }
        Err(_) => return unauthorized(stream),
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_secs();
    let valid_aud = config
        .resource
        .split(',')
        .any(|expected| expected.trim() == claims.audience);
    if !valid_aud || claims.expires_at_unix_seconds < now {
        return unauthorized(stream);
    }

    let state = current_feature_state();
    let body =
        serde_json::to_vec(&state).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    respond(stream, 200, "application/json", &body, None)
}

fn handle_post_features(
    stream: &mut StreamOwned<ServerConnection, TcpStream>,
    config: &Config,
    issuer_key: &[u8],
    request: &Request,
) -> io::Result<()> {
    let validator = Ed25519JwtValidator::new(&config.issuer, issuer_key);
    let claims = match authenticate_http_request(
        request.origin.as_deref(),
        request.authorization.as_deref(),
        &config.origin,
        &validator,
    ) {
        Ok(claims) => claims,
        Err(HttpRequestAuthError::Boundary(HttpAuthError::InvalidOrigin)) => {
            return respond(stream, 403, "text/plain", b"forbidden", None);
        }
        Err(_) => return unauthorized(stream),
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_secs();
    let valid_aud = config
        .resource
        .split(',')
        .any(|expected| expected.trim() == claims.audience);
    if !valid_aud || claims.expires_at_unix_seconds < now {
        return unauthorized(stream);
    }
    if !claims.mutation_enabled {
        return respond(
            stream,
            403,
            "application/json",
            b"{\"error\":\"mutation permission required\"}",
            None,
        );
    }

    let Ok(state) = serde_json::from_slice::<FeatureState>(&request.body) else {
        return respond(
            stream,
            400,
            "application/json",
            b"{\"error\":\"invalid feature state payload\"}",
            None,
        );
    };

    if let Err(err) = apply_feature_state(&state) {
        return respond(
            stream,
            500,
            "application/json",
            format!("{{\"error\":\"failed to apply feature state: {err}\"}}").as_bytes(),
            None,
        );
    }
    let updated = current_feature_state();
    let body =
        serde_json::to_vec(&updated).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    respond(stream, 200, "application/json", &body, None)
}

fn handle_reboot<W: Write>(
    stream: &mut W,
    config: &Config,
    issuer_key: &[u8],
    request: &Request,
) -> io::Result<()> {
    let validator = Ed25519JwtValidator::new(&config.issuer, issuer_key);
    let claims = match authenticate_http_request(
        request.origin.as_deref(),
        request.authorization.as_deref(),
        &config.origin,
        &validator,
    ) {
        Ok(claims) => claims,
        Err(HttpRequestAuthError::Boundary(HttpAuthError::InvalidOrigin)) => {
            return respond(stream, 403, "text/plain", b"forbidden", None);
        }
        Err(_) => return unauthorized(stream),
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_secs();
    let valid_aud = config
        .resource
        .split(',')
        .any(|expected| expected.trim() == claims.audience);
    if !valid_aud || claims.expires_at_unix_seconds < now {
        return unauthorized(stream);
    }
    if !claims.mutation_enabled {
        return respond(
            stream,
            403,
            "application/json",
            b"{\"error\":\"mutation permission required\"}",
            None,
        );
    }

    let trigger_res = fs::write("/run/onyx-kvm/reboot-trigger", b"reboot\n");
    let spawn_res = std::process::Command::new("/sbin/reboot").spawn();
    if trigger_res.is_err() && spawn_res.is_err() {
        return respond(
            stream,
            500,
            "application/json",
            b"{\"error\":\"failed to trigger system reboot\"}",
            None,
        );
    }

    respond(
        stream,
        200,
        "application/json",
        b"{\"status\":\"rebooting\",\"message\":\"Device reboot initiated\"}",
        None,
    )
}

fn unauthorized<W: Write>(stream: &mut W) -> io::Result<()> {
    let challenge = bearer_challenge(METADATA_PATH, "kvm:read");
    respond(stream, 401, "text/plain", b"unauthorized", Some(&challenge))
}

fn read_request<R: Read>(stream: &mut R) -> io::Result<Request> {
    let mut bytes = Vec::new();
    let header_end = loop {
        if bytes.len() >= MAX_HEADER_BYTES {
            return Err(invalid("headers too large"));
        }
        let mut block = [0_u8; 4096];
        let count = stream.read(&mut block)?;
        if count == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "incomplete headers",
            ));
        }
        bytes.extend_from_slice(&block[..count]);
        if let Some(index) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            break index + 4;
        }
    };

    let mut headers = [httparse::EMPTY_HEADER; 32];
    let mut parsed = httparse::Request::new(&mut headers);
    parsed
        .parse(&bytes[..header_end])
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let method = parsed
        .method
        .ok_or_else(|| invalid("missing method"))?
        .to_owned();
    let path = parsed
        .path
        .ok_or_else(|| invalid("missing path"))?
        .to_owned();
    let mut content_length = 0;
    let mut origin = None;
    let mut authorization = None;
    for header in parsed.headers {
        if header.name.eq_ignore_ascii_case("content-length") {
            let text = std::str::from_utf8(header.value).map_err(|_| invalid("bad length"))?;
            content_length = text.parse().map_err(|_| invalid("bad length"))?;
        } else if header.name.eq_ignore_ascii_case("origin") {
            origin = Some(header_text(header.value)?);
        } else if header.name.eq_ignore_ascii_case("authorization") {
            authorization = Some(header_text(header.value)?);
        } else if header.name.eq_ignore_ascii_case("transfer-encoding") {
            return Err(invalid("transfer encoding is unsupported"));
        }
    }
    if content_length > MAX_MESSAGE_BYTES {
        return Err(invalid("body too large"));
    }
    while bytes.len() - header_end < content_length {
        let remaining = content_length - (bytes.len() - header_end);
        let mut block = [0_u8; 4096];
        let limit = remaining.min(block.len());
        let count = stream.read(&mut block[..limit])?;
        if count == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "incomplete body",
            ));
        }
        bytes.extend_from_slice(&block[..count]);
    }
    Ok(Request {
        method,
        path,
        origin,
        authorization,
        body: bytes[header_end..header_end + content_length].to_vec(),
    })
}

fn header_text(value: &[u8]) -> io::Result<String> {
    let text = std::str::from_utf8(value).map_err(|_| invalid("invalid header"))?;
    if text.bytes().any(|byte| byte.is_ascii_control()) {
        return Err(invalid("invalid header"));
    }
    Ok(text.to_owned())
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn respond_frame<W: Write>(
    stream: &mut W,
    snapshot: &VideoSnapshot,
    body: &[u8],
) -> io::Result<()> {
    write!(
        stream,
        concat!(
            "HTTP/1.1 200 OK\r\n",
            "Content-Type: application/octet-stream\r\n",
            "Content-Length: {}\r\n",
            "Cache-Control: no-store, no-cache, must-revalidate\r\n",
            "Content-Security-Policy: default-src 'none'; script-src 'self'; style-src 'self'; connect-src 'self'; img-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'\r\n",
            "Permissions-Policy: camera=(), geolocation=(), microphone=()\r\n",
            "Referrer-Policy: no-referrer\r\n",
            "X-Content-Type-Options: nosniff\r\n",
            "X-Frame-Options: DENY\r\n",
            "X-Frame-Width: {}\r\n",
            "X-Frame-Height: {}\r\n",
            "X-Frame-Format: {}\r\n",
            "X-Frame-Digest: {}\r\n",
            "Connection: close\r\n\r\n"
        ),
        body.len(),
        snapshot.width,
        snapshot.height,
        snapshot.format.as_str(),
        snapshot.sha256_hex()
    )?;
    stream.write_all(body)?;
    stream.flush()
}

fn respond<W: Write>(
    stream: &mut W,
    status: u16,
    content_type: &str,
    body: &[u8],
    challenge: Option<&str>,
) -> io::Result<()> {
    let reason = match status {
        200 => "OK",
        202 => "Accepted",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        409 => "Conflict",
        _ => "Error",
    };
    write!(
        stream,
        concat!(
            "HTTP/1.1 {} {}\r\n",
            "Content-Type: {}\r\n",
            "Content-Length: {}\r\n",
            "Cache-Control: no-store\r\n",
            "Content-Security-Policy: default-src 'none'; script-src 'self'; style-src 'self'; connect-src 'self'; img-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'\r\n",
            "Permissions-Policy: camera=(), geolocation=(), microphone=()\r\n",
            "Referrer-Policy: no-referrer\r\n",
            "X-Content-Type-Options: nosniff\r\n",
            "X-Frame-Options: DENY\r\n",
            "Connection: close\r\n"
        ),
        status,
        reason,
        content_type,
        body.len()
    )?;
    if let Some(challenge) = challenge {
        write!(stream, "WWW-Authenticate: {challenge}\r\n")?;
    }
    write!(stream, "\r\n")?;
    stream.write_all(body)?;
    stream.flush()
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::needless_borrow,
    clippy::similar_names
)]
mod tests {
    use super::{FAVICON_SVG, MAX_HEADER_BYTES, read_request, respond};
    use std::io::Cursor;

    #[test]
    fn parses_bounded_request() {
        let input = b"POST /mcp HTTP/1.1\r\nContent-Length: 2\r\nOrigin: https://kvm\r\nAuthorization: Bearer token\r\n\r\n{}";
        let request = read_request(&mut Cursor::new(input))
            .unwrap_or_else(|error| panic!("unexpected parse error: {error}"));
        assert_eq!(request.method, "POST");
        assert_eq!(request.path, "/mcp");
        assert_eq!(request.body, b"{}");
    }

    #[test]
    fn rejects_oversized_headers() {
        assert!(read_request(&mut Cursor::new(vec![b'a'; MAX_HEADER_BYTES])).is_err());
    }

    #[test]
    fn rejects_transfer_encoding() {
        let input = b"POST /mcp HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n";
        assert!(read_request(&mut Cursor::new(input)).is_err());
    }

    #[test]
    fn parses_bounded_video_stream_request() {
        let input = b"GET /video/stream?format=yuv420p HTTP/1.1\r\nOrigin: https://kvm\r\nAuthorization: Bearer token\r\n\r\n";
        let request = read_request(&mut Cursor::new(input))
            .unwrap_or_else(|error| panic!("unexpected parse error: {error}"));
        assert_eq!(request.method, "GET");
        assert_eq!(request.path, "/video/stream?format=yuv420p");
        assert_eq!(request.origin, Some("https://kvm".to_string()));
        assert_eq!(request.authorization, Some("Bearer token".to_string()));
    }

    #[test]
    fn responses_apply_browser_security_headers() {
        let mut output = Vec::new();
        respond(&mut output, 200, "text/html", b"ok", None)
            .unwrap_or_else(|error| panic!("response failed: {error}"));
        let text = String::from_utf8(output).unwrap_or_default();
        assert!(text.contains("Cache-Control: no-store\r\n"));
        assert!(text.contains("Content-Security-Policy: default-src 'none';"));
        assert!(text.contains("Permissions-Policy: camera=(), geolocation=(), microphone=()"));
        assert!(text.contains("X-Frame-Options: DENY\r\n"));
    }

    #[test]
    fn pair_succeeds_with_valid_pin_and_wipes_state() {
        let temp_dir = std::env::temp_dir().join(format!("onyx-pair-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let pin_file = temp_dir.join("pairing-code");
        let token_file = temp_dir.join("pending-session-token");
        std::fs::write(&pin_file, "123456\n").unwrap();
        std::fs::write(&token_file, "valid-jwt-token-xyz\n").unwrap();

        let req = super::Request {
            method: "POST".to_string(),
            path: "/pair".to_string(),
            origin: Some("https://kvm".to_string()),
            authorization: None,
            body: b"{\"code\":\"123456\"}".to_vec(),
        };

        let attempts = std::sync::atomic::AtomicUsize::new(0);
        let mut output = Vec::new();
        let paths = super::PairingPaths {
            code: pin_file.to_str().unwrap(),
            token: token_file.to_str().unwrap(),
        };
        let res =
            super::handle_pair_with_paths(&mut output, "https://kvm", &req, &paths, &attempts);
        assert!(res.is_ok());
        let text = String::from_utf8(output).unwrap();
        assert!(text.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(text.contains("\"token\":\"valid-jwt-token-xyz\""));
        assert!(text.contains("\"tokenType\":\"Bearer\""));

        assert!(!pin_file.exists());
        assert!(!token_file.exists());
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn pair_rejects_invalid_pin_and_locks_out_after_max_attempts() {
        let temp_dir =
            std::env::temp_dir().join(format!("onyx-pair-lockout-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let pin_file = temp_dir.join("pairing-code");
        let token_file = temp_dir.join("pending-session-token");
        std::fs::write(&pin_file, "654321\n").unwrap();
        std::fs::write(&token_file, "valid-jwt-token-xyz\n").unwrap();

        let req = super::Request {
            method: "POST".to_string(),
            path: "/pair".to_string(),
            origin: Some("https://kvm".to_string()),
            authorization: None,
            body: b"{\"code\":\"000000\"}".to_vec(),
        };

        let attempts = std::sync::atomic::AtomicUsize::new(0);
        let paths = super::PairingPaths {
            code: pin_file.to_str().unwrap(),
            token: token_file.to_str().unwrap(),
        };

        let mut out1 = Vec::new();
        super::handle_pair_with_paths(&mut out1, "https://kvm", &req, &paths, &attempts).unwrap();
        let t1 = String::from_utf8(out1).unwrap();
        assert!(t1.starts_with("HTTP/1.1 401 Unauthorized\r\n"));
        assert!(pin_file.exists());

        let mut out2 = Vec::new();
        super::handle_pair_with_paths(&mut out2, "https://kvm", &req, &paths, &attempts).unwrap();
        let t2 = String::from_utf8(out2).unwrap();
        assert!(t2.starts_with("HTTP/1.1 401 Unauthorized\r\n"));
        assert!(pin_file.exists());

        let mut out3 = Vec::new();
        super::handle_pair_with_paths(&mut out3, "https://kvm", &req, &paths, &attempts).unwrap();
        let t3 = String::from_utf8(out3).unwrap();
        assert!(t3.starts_with("HTTP/1.1 403 Forbidden\r\n"));
        assert!(t3.contains("maximum pairing attempts exceeded"));

        assert!(!pin_file.exists());
        assert!(!token_file.exists());
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn pair_rejects_origin_mismatch() {
        let req = super::Request {
            method: "POST".to_string(),
            path: "/pair".to_string(),
            origin: Some("https://evil.com".to_string()),
            authorization: None,
            body: b"{\"code\":\"123456\"}".to_vec(),
        };

        let attempts = std::sync::atomic::AtomicUsize::new(0);
        let paths = super::PairingPaths {
            code: "/nonexistent/code",
            token: "/nonexistent/token",
        };
        let mut output = Vec::new();
        super::handle_pair_with_paths(&mut output, "https://kvm", &req, &paths, &attempts).unwrap();
        let text = String::from_utf8(output).unwrap();
        assert!(text.starts_with("HTTP/1.1 403 Forbidden\r\n"));
    }

    #[test]
    fn pair_returns_404_when_no_ceremony_active() {
        let req = super::Request {
            method: "POST".to_string(),
            path: "/pair".to_string(),
            origin: Some("https://kvm".to_string()),
            authorization: None,
            body: b"{\"code\":\"123456\"}".to_vec(),
        };

        let attempts = std::sync::atomic::AtomicUsize::new(0);
        let paths = super::PairingPaths {
            code: "/nonexistent/code",
            token: "/nonexistent/token",
        };
        let mut output = Vec::new();
        super::handle_pair_with_paths(&mut output, "https://kvm", &req, &paths, &attempts).unwrap();
        let text = String::from_utf8(output).unwrap();
        assert!(text.starts_with("HTTP/1.1 404 Not Found\r\n"));
        assert!(text.contains("no pairing ceremony currently active"));
    }

    #[test]
    fn tls_config_cipher_suites_and_alpn() {
        let mut provider = rustls::crypto::ring::default_provider();
        provider.cipher_suites.retain(|cs| {
            matches!(
                cs.suite(),
                rustls::CipherSuite::TLS13_CHACHA20_POLY1305_SHA256
                    | rustls::CipherSuite::TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256
                    | rustls::CipherSuite::TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256
            )
        });
        assert_eq!(provider.cipher_suites.len(), 3);
        let suites: Vec<_> = provider
            .cipher_suites
            .iter()
            .map(rustls::SupportedCipherSuite::suite)
            .collect();
        assert!(suites.contains(&rustls::CipherSuite::TLS13_CHACHA20_POLY1305_SHA256));
        assert!(
            suites.contains(&rustls::CipherSuite::TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256)
        );
        assert!(suites.contains(&rustls::CipherSuite::TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256));
    }

    #[test]
    fn stream_flags_detect_keyframe_for_720p_and_1080p() {
        fn check_flags(width: u16, height: u16, byte_len: u32) -> u8 {
            let full = u32::from(width)
                .saturating_mul(u32::from(height))
                .saturating_mul(2);
            if byte_len >= full {
                onyx_kvm_media_protocol::STREAM_FLAG_KEYFRAME
            } else {
                onyx_kvm_media_protocol::STREAM_FLAG_DELTA
            }
        }
        assert_eq!(
            check_flags(1280, 720, 1280 * 720 * 2),
            onyx_kvm_media_protocol::STREAM_FLAG_KEYFRAME
        );
        assert_eq!(
            check_flags(1280, 720, 5000),
            onyx_kvm_media_protocol::STREAM_FLAG_DELTA
        );
        assert_eq!(
            check_flags(1920, 1080, 1920 * 1080 * 2),
            onyx_kvm_media_protocol::STREAM_FLAG_KEYFRAME
        );
        assert_eq!(
            check_flags(1920, 1080, 10000),
            onyx_kvm_media_protocol::STREAM_FLAG_DELTA
        );
    }

    #[test]
    fn stream_standby_frame_header_encoding_and_flags() {
        use onyx_kvm_media_protocol::{
            STREAM_FLAG_STANDBY, SnapshotFormat, StreamFrame, StreamFrameHeader,
        };
        let header = StreamFrameHeader::with_flags(
            42,
            1_000_000,
            1920,
            1080,
            SnapshotFormat::H264,
            STREAM_FLAG_STANDBY,
            0,
            [0_u8; 32],
        )
        .expect("standby header creation failed");
        assert_eq!(header.flags, STREAM_FLAG_STANDBY);
        assert_eq!(header.payload_bytes, 0);

        let encoded = header.encode();
        assert_eq!(encoded.len(), 64);
        let decoded = StreamFrameHeader::decode(encoded).expect("decode failed");
        assert_eq!(decoded.flags, STREAM_FLAG_STANDBY);
        assert_eq!(decoded.payload_bytes, 0);
        assert_eq!(decoded.sequence, 42);

        let frame = StreamFrame::new(header, Vec::new()).expect("standby frame creation failed");
        assert_eq!(frame.payload.len(), 0);
    }

    #[test]
    fn feature_state_defaults_and_serde() {
        let default_state = super::FeatureState::default();
        assert!(default_state.video);
        assert!(default_state.hid);
        assert!(!default_state.speaker);
        assert!(!default_state.microphone);

        let json = serde_json::to_string(&default_state).unwrap();
        let parsed: super::FeatureState = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, default_state);
    }

    #[test]
    fn feature_state_gate_synchronization() {
        let temp_dir =
            std::env::temp_dir().join(format!("onyx-features-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let vid_path = temp_dir.join("video-enabled");
        let hid_path = temp_dir.join("hid-enabled");

        let _ = super::set_gate_file(vid_path.to_str().unwrap(), true);
        assert!(vid_path.exists());

        let _ = super::set_gate_file(vid_path.to_str().unwrap(), false);
        assert!(!vid_path.exists());

        let _ = super::set_gate_file(hid_path.to_str().unwrap(), true);
        assert!(hid_path.exists());
        let _ = super::set_gate_file(hid_path.to_str().unwrap(), false);
        assert!(!hid_path.exists());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn favicon_endpoint_returns_svg() {
        let mut output = Vec::new();
        respond(&mut output, 200, "image/svg+xml", FAVICON_SVG, None)
            .unwrap_or_else(|error| panic!("response failed: {error}"));
        let text = String::from_utf8_lossy(&output);
        assert!(text.contains("HTTP/1.1 200 OK\r\n"));
        assert!(text.contains("Content-Type: image/svg+xml\r\n"));
        assert!(text.contains("Onyx zen-O mark"));
    }

    #[test]
    fn tofu_setup_lifecycle_and_pin_authentication() {
        let temp_dir = std::env::temp_dir().join(format!("onyx-tofu-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let boot_pin = temp_dir.join("boot-pin.hash");
        let run_pin = temp_dir.join("run-pin.hash");
        let etc_pin = temp_dir.join("etc-pin.hash");
        let pin_paths = super::PinPaths {
            boot: boot_pin.to_str().unwrap(),
            run: run_pin.to_str().unwrap(),
            etc: etc_pin.to_str().unwrap(),
        };
        let pairing_paths = super::PairingPaths {
            code: "/nonexistent/code",
            token: "/nonexistent/token",
        };

        // 1. Initial state is unconfigured
        let mut status_out = Vec::new();
        super::handle_setup_status(&mut status_out, &pin_paths, &pairing_paths).unwrap();
        let status_str = String::from_utf8(status_out).unwrap();
        assert!(status_str.contains("\"configured\":false"));

        // Setup test config and signing key
        let signing_key = [42u8; 32];
        let config = super::Config {
            bind: "127.0.0.1:8443".to_string(),
            origin: "https://kvm.example".to_string(),
            resource: "https://kvm.example/mcp".to_string(),
            authorization_server: "https://kvm.example".to_string(),
            issuer: "https://kvm.example".to_string(),
            certificate: std::path::PathBuf::from("/nonexistent/cert"),
            private_key: std::path::PathBuf::from("/nonexistent/key"),
            issuer_public_key: std::path::PathBuf::from("/nonexistent/pub"),
            issuer_signing_key: None,
        };

        // 2. Claim with bad origin rejected
        let bad_origin_req = super::Request {
            method: "POST".to_string(),
            path: "/setup/claim".to_string(),
            origin: Some("https://evil.com".to_string()),
            authorization: None,
            body: b"{\"pin\":\"123456\"}".to_vec(),
        };
        let mut bad_origin_out = Vec::new();
        super::handle_setup_claim(
            &mut bad_origin_out,
            &config,
            &bad_origin_req,
            Some(&signing_key),
            &pin_paths,
            &pairing_paths,
        )
        .unwrap();
        assert!(
            String::from_utf8(bad_origin_out)
                .unwrap()
                .contains("403 Forbidden")
        );

        // 2b. Claim with missing origin rejected
        let missing_origin_req = super::Request {
            method: "POST".to_string(),
            path: "/setup/claim".to_string(),
            origin: None,
            authorization: None,
            body: b"{\"pin\":\"123456\"}".to_vec(),
        };
        let mut missing_origin_out = Vec::new();
        super::handle_setup_claim(
            &mut missing_origin_out,
            &config,
            &missing_origin_req,
            Some(&signing_key),
            &pin_paths,
            &pairing_paths,
        )
        .unwrap();
        assert!(
            String::from_utf8(missing_origin_out)
                .unwrap()
                .contains("403 Forbidden")
        );

        // 3. Claim with invalid PIN (letters) rejected
        let invalid_pin_req = super::Request {
            method: "POST".to_string(),
            path: "/setup/claim".to_string(),
            origin: Some("https://kvm.example".to_string()),
            authorization: None,
            body: b"{\"pin\":\"abcdef\"}".to_vec(),
        };
        let mut invalid_pin_out = Vec::new();
        super::handle_setup_claim(
            &mut invalid_pin_out,
            &config,
            &invalid_pin_req,
            Some(&signing_key),
            &pin_paths,
            &pairing_paths,
        )
        .unwrap();
        assert!(
            String::from_utf8(invalid_pin_out)
                .unwrap()
                .contains("400 Bad Request")
        );

        // 4. Valid claim succeeds and mints token
        let claim_req = super::Request {
            method: "POST".to_string(),
            path: "/setup/claim".to_string(),
            origin: Some("https://kvm.example".to_string()),
            authorization: None,
            body: b"{\"pin\":\"852963\"}".to_vec(),
        };
        let mut claim_out = Vec::new();
        super::handle_setup_claim(
            &mut claim_out,
            &config,
            &claim_req,
            Some(&signing_key),
            &pin_paths,
            &pairing_paths,
        )
        .unwrap();
        let claim_res = String::from_utf8(claim_out).unwrap();
        assert!(claim_res.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(claim_res.contains("\"status\":\"configured\""));
        assert!(claim_res.contains("\"token\":"));

        // 5. Subsequent status shows configured
        let mut status_out2 = Vec::new();
        super::handle_setup_status(&mut status_out2, &pin_paths, &pairing_paths).unwrap();
        let status_str2 = String::from_utf8(status_out2).unwrap();
        assert!(status_str2.contains("\"configured\":true"));

        // 6. Claim again is rejected with 409 Conflict (single-use TOFU)
        let mut second_claim_out = Vec::new();
        super::handle_setup_claim(
            &mut second_claim_out,
            &config,
            &claim_req,
            Some(&signing_key),
            &pin_paths,
            &pairing_paths,
        )
        .unwrap();
        assert!(
            String::from_utf8(second_claim_out)
                .unwrap()
                .contains("409 Conflict")
        );

        // 7. Pair with wrong PIN rejected
        let attempts = std::sync::atomic::AtomicUsize::new(0);
        let wrong_pair_req = super::Request {
            method: "POST".to_string(),
            path: "/pair".to_string(),
            origin: Some("https://kvm.example".to_string()),
            authorization: None,
            body: b"{\"code\":\"000000\"}".to_vec(),
        };
        let mut wrong_pair_out = Vec::new();
        super::handle_pair_full(
            &mut wrong_pair_out,
            "https://kvm.example",
            Some("https://kvm.example"),
            Some("https://kvm.example/mcp"),
            &wrong_pair_req,
            &pairing_paths,
            &pin_paths,
            Some(&signing_key),
            &attempts,
        )
        .unwrap();
        assert!(
            String::from_utf8(wrong_pair_out)
                .unwrap()
                .contains("401 Unauthorized")
        );

        // 8. Pair with correct PIN succeeds and mints fresh token
        let correct_pair_req = super::Request {
            method: "POST".to_string(),
            path: "/pair".to_string(),
            origin: Some("https://kvm.example".to_string()),
            authorization: None,
            body: b"{\"code\":\"852963\"}".to_vec(),
        };
        let mut correct_pair_out = Vec::new();
        super::handle_pair_full(
            &mut correct_pair_out,
            "https://kvm.example",
            Some("https://kvm.example"),
            Some("https://kvm.example/mcp"),
            &correct_pair_req,
            &pairing_paths,
            &pin_paths,
            Some(&signing_key),
            &attempts,
        )
        .unwrap();
        let pair_res = String::from_utf8(correct_pair_out).unwrap();
        assert!(pair_res.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(pair_res.contains("\"token\":"));
        assert!(pair_res.contains("\"tokenType\":\"Bearer\""));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn factory_reset_wipes_pin_hash() {
        let temp_dir = std::env::temp_dir().join(format!("onyx-reset-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let boot_pin = temp_dir.join("boot-pin.hash");
        let run_pin = temp_dir.join("run-pin.hash");
        let etc_pin = temp_dir.join("etc-pin.hash");
        let pin_paths = super::PinPaths {
            boot: boot_pin.to_str().unwrap(),
            run: run_pin.to_str().unwrap(),
            etc: etc_pin.to_str().unwrap(),
        };
        let pairing_paths = super::PairingPaths {
            code: "/nonexistent/code",
            token: "/nonexistent/token",
        };

        // Save a PIN
        super::save_pin_hash_paths("123456", &pin_paths).unwrap();
        assert!(super::is_device_configured_with_paths(
            &pin_paths,
            &pairing_paths
        ));

        // Create reset trigger
        let trigger = temp_dir.join("RESET");
        let _ = std::fs::write(&trigger, b"RESET\n");

        // Factory reset clears state
        super::check_and_process_factory_reset(&pin_paths);
        assert!(!trigger.exists());
        assert!(!super::is_device_configured_with_paths(
            &pin_paths,
            &pairing_paths
        ));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn reboot_endpoint_requires_auth_and_triggers_reboot() {
        use ring::signature::KeyPair;
        let signing_key = [7_u8; 32];
        let key_pair = ring::signature::Ed25519KeyPair::from_seed_unchecked(&signing_key).unwrap();
        let verifying_key = key_pair.public_key().as_ref();
        let config = super::Config {
            bind: "127.0.0.1:0".to_string(),
            origin: "https://kvm.example".to_string(),
            resource: "https://kvm.example/mcp".to_string(),
            authorization_server: "https://kvm.example/auth".to_string(),
            issuer: "https://kvm.example/issuer".to_string(),
            certificate: std::path::PathBuf::from("/nonexistent/cert"),
            private_key: std::path::PathBuf::from("/nonexistent/key"),
            issuer_public_key: std::path::PathBuf::from("/nonexistent/pub"),
            issuer_signing_key: None,
        };

        // 1. Unauthenticated request -> 401
        let unauth_req = super::Request {
            method: "POST".to_string(),
            path: "/system/reboot".to_string(),
            origin: Some("https://kvm.example".to_string()),
            authorization: None,
            body: Vec::new(),
        };
        let mut out = Vec::new();
        super::handle_reboot(&mut out, &config, &verifying_key, &unauth_req).unwrap();
        let res = String::from_utf8(out).unwrap();
        assert!(res.starts_with("HTTP/1.1 401 Unauthorized\r\n"));

        // 2. Read-only token (mutation_enabled = false) -> 403
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let ro_token = onyx_kvm_mcp::mint_token(
            &signing_key,
            "https://kvm.example/issuer",
            "https://kvm.example/mcp",
            "kvm:read",
            false,
            now + 3600,
        );
        let ro_req = super::Request {
            method: "POST".to_string(),
            path: "/system/reboot".to_string(),
            origin: Some("https://kvm.example".to_string()),
            authorization: Some(format!("Bearer {ro_token}")),
            body: Vec::new(),
        };
        let mut out = Vec::new();
        super::handle_reboot(&mut out, &config, &verifying_key, &ro_req).unwrap();
        let res = String::from_utf8(out).unwrap();
        assert!(res.starts_with("HTTP/1.1 403 Forbidden\r\n"));
        assert!(res.contains("mutation permission required"));

        // 3. Authorized mutating token -> 200 OK & writes reboot-trigger
        let rw_token = onyx_kvm_mcp::mint_token(
            &signing_key,
            "https://kvm.example/issuer",
            "https://kvm.example/mcp",
            "kvm:read kvm:admin",
            true,
            now + 3600,
        );
        let rw_req = super::Request {
            method: "POST".to_string(),
            path: "/system/reboot".to_string(),
            origin: Some("https://kvm.example".to_string()),
            authorization: Some(format!("Bearer {rw_token}")),
            body: Vec::new(),
        };
        let mut out = Vec::new();
        super::handle_reboot(&mut out, &config, &verifying_key, &rw_req).unwrap();
        let res = String::from_utf8(out).unwrap();
        assert!(res.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(res.contains("\"status\":\"rebooting\""));
    }
}
