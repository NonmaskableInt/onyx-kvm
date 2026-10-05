use std::{
    env, fs,
    io::{self, Read, Write},
    net::{TcpListener, TcpStream},
    os::unix::net::UnixStream,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use cleankvm_mcp::{
    Ed25519JwtValidator, HidTransport, HttpAuthError, HttpRequestAuthError, MAX_MESSAGE_BYTES,
    Server, VideoTransport, authenticate_http_request, bearer_challenge,
    protected_resource_metadata,
};
use cleankvm_media_protocol::{
    Operation, SNAPSHOT_RESPONSE_BYTES, SnapshotRequest, VideoSnapshot, WireHeader,
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
const HID_SOCKET: &str = "/run/cleankvm/hid.sock";
const VIDEO_SOCKET: &str = "/run/cleankvm/video.sock";
const VIDEO_ENABLED_PATH: &str = "/run/cleankvm/video-enabled";

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
        let Some(bind) = env::var_os("CLEANKVM_HTTPS_BIND") else {
            return Ok(None);
        };
        Ok(Some(Self {
            bind: bind
                .into_string()
                .map_err(|_| invalid("invalid bind address"))?,
            origin: required("CLEANKVM_HTTPS_ORIGIN")?,
            resource: required("CLEANKVM_MCP_RESOURCE")?,
            authorization_server: required("CLEANKVM_AUTHORIZATION_SERVER")?,
            issuer: required("CLEANKVM_TOKEN_ISSUER")?,
            certificate: required("CLEANKVM_TLS_CERTIFICATE_DER")?.into(),
            private_key: required("CLEANKVM_TLS_PRIVATE_KEY_DER")?.into(),
            issuer_public_key: required("CLEANKVM_ISSUER_PUBLIC_KEY_RAW")?.into(),
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
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let server = ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(io::Error::other)?
        .with_no_client_auth()
        .with_single_cert(vec![certificate], private_key)
        .map_err(io::Error::other)?;
    Ok(Arc::new(server))
}

fn serve(config: &Config) -> io::Result<()> {
    let tls = tls_config(config)?;
    let issuer_key = read_identity(&config.issuer_public_key, Some(32))?;
    let listener = TcpListener::bind(&config.bind)?;
    let mut initialized = false;
    for socket in listener.incoming() {
        let socket = socket?;
        socket.set_read_timeout(Some(TIMEOUT))?;
        socket.set_write_timeout(Some(TIMEOUT))?;
        let session = ServerConnection::new(Arc::clone(&tls)).map_err(io::Error::other)?;
        let mut stream = StreamOwned::new(session, socket);
        if let Err(error) = handle(&mut stream, config, &issuer_key, &mut initialized) {
            let _ = respond(&mut stream, 400, "text/plain", b"bad request", None);
            eprintln!("rejected HTTPS request: {error}");
        }
    }
    Ok(())
}

fn handle(
    stream: &mut StreamOwned<ServerConnection, TcpStream>,
    config: &Config,
    issuer_key: &[u8],
    initialized: &mut bool,
) -> io::Result<()> {
    let request = read_request(stream)?;
    if request.method == "GET" && request.path == METADATA_PATH {
        let body =
            protected_resource_metadata(&config.resource, &config.authorization_server).to_string();
        return respond(stream, 200, "application/json", body.as_bytes(), None);
    }
    if request.method == "GET" {
        return match request.path.as_str() {
            "/" => respond(stream, 200, "text/html; charset=utf-8", INDEX_HTML, None),
            "/app.css" => respond(stream, 200, "text/css; charset=utf-8", APP_CSS, None),
            "/app.js" => respond(stream, 200, "text/javascript; charset=utf-8", APP_JS, None),
            _ => respond(stream, 404, "text/plain", b"not found", None),
        };
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
    if claims.audience != config.resource || claims.expires_at_unix_seconds < now {
        return unauthorized(stream);
    }

    let message: serde_json::Value = serde_json::from_slice(&request.body)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let method = message.get("method").and_then(serde_json::Value::as_str);
    if method == Some("notifications/initialized") {
        *initialized = true;
        return respond(stream, 202, "application/json", b"", None);
    }
    let grant = claims.as_grant();
    let hid = BrokerHid;
    let video = BrokerVideo;
    let mut server = if method == Some("initialize") || !*initialized {
        Server::remote(&config.resource, now, grant)
    } else {
        Server::remote_ready_with_hid(&config.resource, now, grant, &hid)
    };
    if Path::new(VIDEO_ENABLED_PATH).exists() {
        server = server.with_video(&video);
    }
    let Some(body) = server.handle_line(&request.body) else {
        return respond(stream, 202, "application/json", b"", None);
    };
    respond(stream, 200, "application/json", body.as_bytes(), None)
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
}
