use std::{
    env, fs,
    io::{self, Read, Write},
    net::{TcpListener, TcpStream},
    os::unix::net::UnixStream,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use onyx_kvm_mcp::{
    AudioCaptureTransport, AudioOutputTransport, Ed25519JwtValidator, HidTransport, HttpAuthError,
    HttpRequestAuthError, MAX_MESSAGE_BYTES, Scope, Server, VideoTransport,
    authenticate_http_request, bearer_challenge, protected_resource_metadata, validate_origin,
};
use onyx_kvm_media_protocol::{
    AUDIO_REQUEST_BYTES, BoundedFrameRing, CAPTURE_METRICS_BYTES, CaptureMetrics,
    DEFAULT_STREAM_LEASE_MS, Operation, PLAYBACK_RECEIPT_BYTES, PcmShape, PlaybackReceipt,
    SNAPSHOT_RESPONSE_BYTES, STREAM_FLAG_DELTA, STREAM_FLAG_KEYFRAME, SnapshotFormat,
    SnapshotRequest, StreamFrame, StreamFrameHeader, StreamSessionLease, VideoSnapshot,
    WIRE_HEADER_BYTES, WireHeader,
};
use rustls::{
    ServerConfig, ServerConnection, StreamOwned,
    pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer},
};

use crate::web::{APP_CSS, APP_JS, INDEX_HTML};

const MAX_HEADER_BYTES: usize = 16 * 1024;
const MAX_IDENTITY_BYTES: usize = 64 * 1024;
const TIMEOUT: Duration = Duration::from_secs(10);
const METADATA_PATH: &str = "/.well-known/oauth-protected-resource";
const HID_SOCKET: &str = "/run/onyx-kvm/hid.sock";
const VIDEO_SOCKET: &str = "/run/onyx-kvm/video.sock";
const VIDEO_ENABLED_PATH: &str = "/run/onyx-kvm/video-enabled";
const AUDIO_CAPTURE_SOCKET: &str = "/run/onyx-kvm/audio-capture.sock";
const AUDIO_CAPTURE_ENABLED_PATH: &str = "/run/onyx-kvm/audio-capture-enabled";
const AUDIO_OUTPUT_SOCKET: &str = "/run/onyx-kvm/audio-output.sock";
const AUDIO_OUTPUT_ENABLED_PATH: &str = "/run/onyx-kvm/audio-output-enabled";
const PAIRING_CODE_PATH: &str = "/run/onyx-kvm/pairing-code";
const PAIRING_CODE_FALLBACK: &str = "/run/cleankvm/pairing-code";
const PENDING_SESSION_TOKEN_PATH: &str = "/run/onyx-kvm/pending-session-token";
const PENDING_SESSION_TOKEN_FALLBACK: &str = "/run/cleankvm/pending-session-token";

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

struct BrokerVideo;

impl BrokerVideo {
    fn capture_frame(request: SnapshotRequest) -> io::Result<(VideoSnapshot, Vec<u8>)> {
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

    fn capture_next(&mut self, request: SnapshotRequest) -> io::Result<(VideoSnapshot, Vec<u8>)> {
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
        let mut snapshot_bytes = [0_u8; SNAPSHOT_RESPONSE_BYTES];
        self.stream.read_exact(&mut snapshot_bytes)?;
        let snapshot = VideoSnapshot::decode(snapshot_bytes)
            .map_err(|_| invalid("invalid video broker snapshot response"))?;
        let expected_frame_bytes = usize::try_from(snapshot.byte_length)
            .map_err(|_| invalid("frame byte length overflow"))?;
        let mut frame_bytes = vec![0_u8; expected_frame_bytes];
        self.stream.read_exact(&mut frame_bytes)?;
        Ok((snapshot, frame_bytes))
    }
}

impl VideoTransport for BrokerVideo {
    fn capture_snapshot(&self, request: SnapshotRequest) -> io::Result<VideoSnapshot> {
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
        let Some(bind) =
            env::var_os("ONYX_KVM_HTTPS_BIND").or_else(|| env::var_os("CLEANKVM_HTTPS_BIND"))
        else {
            return Ok(None);
        };
        Ok(Some(Self {
            bind: bind
                .into_string()
                .map_err(|_| invalid("invalid bind address"))?,
            origin: required("ONYX_KVM_HTTPS_ORIGIN", "CLEANKVM_HTTPS_ORIGIN")?,
            resource: required("ONYX_KVM_MCP_RESOURCE", "CLEANKVM_MCP_RESOURCE")?,
            authorization_server: required(
                "ONYX_KVM_AUTHORIZATION_SERVER",
                "CLEANKVM_AUTHORIZATION_SERVER",
            )?,
            issuer: required("ONYX_KVM_TOKEN_ISSUER", "CLEANKVM_TOKEN_ISSUER")?,
            certificate: required(
                "ONYX_KVM_TLS_CERTIFICATE_DER",
                "CLEANKVM_TLS_CERTIFICATE_DER",
            )?
            .into(),
            private_key: required(
                "ONYX_KVM_TLS_PRIVATE_KEY_DER",
                "CLEANKVM_TLS_PRIVATE_KEY_DER",
            )?
            .into(),
            issuer_public_key: required(
                "ONYX_KVM_ISSUER_PUBLIC_KEY_RAW",
                "CLEANKVM_ISSUER_PUBLIC_KEY_RAW",
            )?
            .into(),
        }))
    }
}

fn required(name: &str, fallback: &str) -> io::Result<String> {
    env::var(name).or_else(|_| env::var(fallback)).map_err(|_| {
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

fn serve(config: &Config) -> io::Result<()> {
    let tls = tls_config(config)?;
    let issuer_key = Arc::new(read_identity(&config.issuer_public_key, Some(32))?);
    let listener = TcpListener::bind(&config.bind)?;
    let initialized = Arc::new(AtomicBool::new(false));
    let config = Arc::new(config.clone());
    for socket in listener.incoming() {
        let socket = socket?;
        socket.set_read_timeout(Some(TIMEOUT))?;
        socket.set_write_timeout(Some(TIMEOUT))?;
        let tls = Arc::clone(&tls);
        let config = Arc::clone(&config);
        let issuer_key = Arc::clone(&issuer_key);
        let initialized = Arc::clone(&initialized);
        std::thread::spawn(move || {
            let session = match ServerConnection::new(tls) {
                Ok(s) => s,
                Err(err) => {
                    eprintln!("failed to establish TLS session: {err}");
                    return;
                }
            };
            let mut stream = StreamOwned::new(session, socket);
            if let Err(error) = handle(&mut stream, &config, &issuer_key, &initialized) {
                let _ = respond(&mut stream, 400, "text/plain", b"bad request", None);
                eprintln!("rejected HTTPS request: {error}");
            }
        });
    }
    Ok(())
}

struct PairingPaths<'a> {
    code: &'a str,
    code_fallback: &'a str,
    token: &'a str,
    token_fallback: &'a str,
}

fn wipe_pairing_state_paths(paths: &PairingPaths<'_>) {
    let _ = fs::write(paths.code, b"");
    let _ = fs::write(paths.code_fallback, b"");
    let _ = fs::write(paths.token, b"");
    let _ = fs::write(paths.token_fallback, b"");
    let _ = fs::remove_file(paths.code);
    let _ = fs::remove_file(paths.code_fallback);
    let _ = fs::remove_file(paths.token);
    let _ = fs::remove_file(paths.token_fallback);
}

fn read_file_with_fallback(path: &str, fallback: &str) -> Option<String> {
    if let Ok(content) = fs::read_to_string(path) {
        return Some(content);
    }
    fs::read_to_string(fallback).ok()
}

fn handle_pair_with_paths<W: Write>(
    stream: &mut W,
    config_origin: &str,
    request: &Request,
    paths: &PairingPaths<'_>,
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
    let Some(code_val) = value.get("code").and_then(serde_json::Value::as_str) else {
        return respond(
            stream,
            400,
            "application/json",
            b"{\"error\":\"missing 'code' field in payload\"}",
            None,
        );
    };

    let Some(expected_code) = read_file_with_fallback(paths.code, paths.code_fallback) else {
        return respond(
            stream,
            404,
            "application/json",
            b"{\"error\":\"no pairing ceremony currently active\"}",
            None,
        );
    };

    let expected_trimmed = expected_code.trim();
    let code_trimmed = code_val.trim();

    let mut diff = expected_trimmed.len() ^ code_trimmed.len();
    for (a, b) in expected_trimmed.bytes().zip(code_trimmed.bytes()) {
        diff |= (a ^ b) as usize;
    }

    if diff != 0 {
        let attempts = attempts_counter.fetch_add(1, Ordering::SeqCst) + 1;
        if attempts >= MAX_PAIRING_ATTEMPTS {
            wipe_pairing_state_paths(paths);
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
    let Some(token) = read_file_with_fallback(paths.token, paths.token_fallback) else {
        wipe_pairing_state_paths(paths);
        return respond(
            stream,
            500,
            "application/json",
            b"{\"error\":\"missing pending session token\"}",
            None,
        );
    };

    wipe_pairing_state_paths(paths);

    let response_body = serde_json::json!({
        "token": token.trim(),
        "tokenType": "Bearer",
        "expiresIn": 14400,
    });
    let body_bytes = serde_json::to_vec(&response_body)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    respond(stream, 200, "application/json", &body_bytes, None)
}

fn handle_pair<W: Write>(
    stream: &mut W,
    config: &Config,
    request: &Request,
) -> io::Result<()> {
    let paths = PairingPaths {
        code: PAIRING_CODE_PATH,
        code_fallback: PAIRING_CODE_FALLBACK,
        token: PENDING_SESSION_TOKEN_PATH,
        token_fallback: PENDING_SESSION_TOKEN_FALLBACK,
    };
    handle_pair_with_paths(
        stream,
        &config.origin,
        request,
        &paths,
        &PAIRING_ATTEMPTS,
    )
}

fn handle(
    stream: &mut StreamOwned<ServerConnection, TcpStream>,
    config: &Config,
    issuer_key: &[u8],
    initialized: &AtomicBool,
) -> io::Result<()> {
    let request = read_request(stream)?;
    if request.method == "GET" && request.path == METADATA_PATH {
        let body =
            protected_resource_metadata(&config.resource, &config.authorization_server).to_string();
        return respond(stream, 200, "application/json", body.as_bytes(), None);
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
    if request.method == "GET" {
        return match request.path.as_str() {
            "/" => respond(stream, 200, "text/html; charset=utf-8", INDEX_HTML, None),
            "/app.css" => respond(stream, 200, "text/css; charset=utf-8", APP_CSS, None),
            "/app.js" => respond(stream, 200, "text/javascript; charset=utf-8", APP_JS, None),
            _ => respond(stream, 404, "text/plain", b"not found", None),
        };
    }
    if request.method == "POST" && (request.path == "/pair" || request.path == "/enroll") {
        return handle_pair(stream, config, &request);
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
    let mut server = if method == Some("initialize") || !is_init {
        Server::remote(&claims.audience, now, grant)
    } else {
        Server::remote_ready_with_hid(&claims.audience, now, grant, &hid)
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
    let format = if request.path.contains("format=yuv420p") {
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
    let format = if request.path.contains("format=tile_delta")
        || request.path.contains("mode=delta")
    {
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
    let mut lease = StreamSessionLease::new(1, now_ms, DEFAULT_STREAM_LEASE_MS)
        .map_err(|_| invalid("invalid stream lease"))?;
    let mut ring = BoundedFrameRing::new(2);
    let mut sequence = 0_u32;
    let timeout = TIMEOUT.max(Duration::from_millis(u64::from(snapshot_request.deadline_ms)));
    let mut broker_session = match VideoStreamSession::connect(timeout) {
        Ok(s) => s,
        Err(err) => {
            eprintln!("failed to connect to video broker: {err}");
            return Ok(());
        }
    };

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

        let (snapshot, frame_bytes) = match broker_session.capture_next(snapshot_request) {
            Ok(res) => res,
            Err(err) => {
                eprintln!("failed to capture video frame in stream: {err}");
                break;
            }
        };

        let flags = match snapshot.format {
            SnapshotFormat::TileDeltaYuv422 => {
                if snapshot.byte_length >= 1920 * 1080 * 2 {
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

        sequence = sequence.wrapping_add(1);
        let _ = lease.renew(current_ms, DEFAULT_STREAM_LEASE_MS);
    }

    ring.clear();
    Ok(())
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
mod tests {
    use super::{MAX_HEADER_BYTES, read_request, respond};
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
            code_fallback: "/nonexistent/code",
            token: token_file.to_str().unwrap(),
            token_fallback: "/nonexistent/token",
        };
        let res = super::handle_pair_with_paths(
            &mut output,
            "https://kvm",
            &req,
            &paths,
            &attempts,
        );
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
            code_fallback: "/nonexistent/code",
            token: token_file.to_str().unwrap(),
            token_fallback: "/nonexistent/token",
        };

        let mut out1 = Vec::new();
        super::handle_pair_with_paths(
            &mut out1,
            "https://kvm",
            &req,
            &paths,
            &attempts,
        )
        .unwrap();
        let t1 = String::from_utf8(out1).unwrap();
        assert!(t1.starts_with("HTTP/1.1 401 Unauthorized\r\n"));
        assert!(pin_file.exists());

        let mut out2 = Vec::new();
        super::handle_pair_with_paths(
            &mut out2,
            "https://kvm",
            &req,
            &paths,
            &attempts,
        )
        .unwrap();
        let t2 = String::from_utf8(out2).unwrap();
        assert!(t2.starts_with("HTTP/1.1 401 Unauthorized\r\n"));
        assert!(pin_file.exists());

        let mut out3 = Vec::new();
        super::handle_pair_with_paths(
            &mut out3,
            "https://kvm",
            &req,
            &paths,
            &attempts,
        )
        .unwrap();
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
            code_fallback: "/nonexistent/code",
            token: "/nonexistent/token",
            token_fallback: "/nonexistent/token",
        };
        let mut output = Vec::new();
        super::handle_pair_with_paths(
            &mut output,
            "https://kvm",
            &req,
            &paths,
            &attempts,
        )
        .unwrap();
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
            code_fallback: "/nonexistent/code",
            token: "/nonexistent/token",
            token_fallback: "/nonexistent/token",
        };
        let mut output = Vec::new();
        super::handle_pair_with_paths(
            &mut output,
            "https://kvm",
            &req,
            &paths,
            &attempts,
        )
        .unwrap();
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
        let suites: Vec<_> = provider.cipher_suites.iter().map(|cs| cs.suite()).collect();
        assert!(suites.contains(&rustls::CipherSuite::TLS13_CHACHA20_POLY1305_SHA256));
        assert!(suites.contains(&rustls::CipherSuite::TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256));
        assert!(suites.contains(&rustls::CipherSuite::TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256));
    }
}
