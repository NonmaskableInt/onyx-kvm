//! Bounded MCP stdio protocol adapter.

use std::io::{self, BufRead, Write};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
pub use onyx_kvm_core::Scope;
use onyx_kvm_core::{Effect, Grant, Operation, PolicyError, authorize, authorize_grant};
use onyx_kvm_media_protocol::{
    CaptureMetrics, PcmFormat, PcmShape, PlaybackReceipt, SnapshotFormat, SnapshotRequest,
    VideoSnapshot,
};
use serde::Deserialize;
use serde_json::{Value, json};

pub const PROTOCOL_VERSION: &str = "2025-11-25";
pub const MAX_MESSAGE_BYTES: usize = 64 * 1024;
pub const MAX_BEARER_TOKEN_BYTES: usize = 4096;
pub const MAX_GRANT_SCOPES: usize = 7;

/// Narrow transport boundary for already-authorized, fixed-size HID reports.
pub trait HidTransport {
    /// Send one boot-keyboard report.
    ///
    /// # Errors
    /// Returns an I/O error when the privileged broker rejects or cannot write it.
    fn send_keyboard(&self, report: [u8; 8]) -> io::Result<()>;
    /// Send one relative-pointer report.
    ///
    /// # Errors
    /// Returns an I/O error when the privileged broker rejects or cannot write it.
    fn send_pointer(&self, report: [u8; 4]) -> io::Result<()>;
}

/// Narrow transport boundary for bounded, read-only video frame snapshots.
pub trait VideoTransport {
    /// Request one bounded video frame snapshot from the privileged broker.
    ///
    /// # Errors
    /// Returns an I/O error when the privileged broker rejects or cannot fulfill it.
    fn capture_snapshot(&self, request: SnapshotRequest) -> io::Result<VideoSnapshot>;
}

/// Narrow transport boundary for metrics-only microphone capture.
pub trait AudioCaptureTransport {
    /// Capture one admitted PCM interval and return aggregate metrics only.
    ///
    /// # Errors
    /// Returns an I/O error when the privileged broker rejects or cannot complete capture.
    fn capture_metrics(&self, shape: PcmShape) -> io::Result<CaptureMetrics>;
}

/// Narrow transport boundary for the admitted conservative playback tone.
pub trait AudioOutputTransport {
    /// Play one admitted bounded tone and return its completion receipt.
    ///
    /// # Errors
    /// Returns an I/O error when the privileged broker rejects or cannot complete playback.
    fn play_bounded_tone(&self, shape: PcmShape) -> io::Result<PlaybackReceipt>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HttpAuthError {
    InvalidOrigin,
    MissingAuthorization,
    InvalidAuthorization,
}

/// Owned claims emitted by a cryptographic OAuth access-token validator.
/// Raw bearer credentials are deliberately not retained.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedClaims {
    pub audience: String,
    pub expires_at_unix_seconds: u64,
    pub scopes: Vec<Scope>,
    pub mutation_enabled: bool,
}

impl ValidatedClaims {
    #[must_use]
    pub fn as_grant(&self) -> Grant<'_> {
        Grant {
            audience: &self.audience,
            expires_at_unix_seconds: self.expires_at_unix_seconds,
            scopes: &self.scopes,
            mutation_enabled: self.mutation_enabled,
        }
    }
}

/// Adapter for a local cryptographic validator or OAuth introspection client.
pub trait BearerTokenValidator {
    type Error;

    /// Validate integrity, issuer, audience, expiry, and authorized scopes.
    ///
    /// # Errors
    /// Returns an implementation-specific error for any invalid token.
    fn validate(&self, token: &str) -> Result<ValidatedClaims, Self::Error>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HttpRequestAuthError<E> {
    Boundary(HttpAuthError),
    InvalidToken(E),
    InvalidClaims,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JwtValidationError {
    Malformed,
    InvalidHeader,
    InvalidSignature,
    InvalidClaims,
    WrongIssuer,
    UnknownScope,
}

/// Offline validator for compact Ed25519-signed JWT access tokens.
///
/// The public key is raw 32-byte Ed25519 key material provisioned during local
/// enrollment. This validator never accepts unsigned or symmetric JWTs.
pub struct Ed25519JwtValidator<'a> {
    issuer: &'a str,
    public_key: &'a [u8],
}

impl<'a> Ed25519JwtValidator<'a> {
    #[must_use]
    pub const fn new(issuer: &'a str, public_key: &'a [u8]) -> Self {
        Self { issuer, public_key }
    }
}

#[derive(Deserialize)]
struct JwtHeader<'a> {
    alg: &'a str,
    #[serde(default)]
    typ: Option<&'a str>,
}

#[derive(Deserialize)]
struct JwtClaims<'a> {
    iss: &'a str,
    aud: &'a str,
    exp: u64,
    #[serde(default)]
    scope: &'a str,
    #[serde(default, alias = "cleankvm_mutation", alias = "onyx_kvm_mutation")]
    onyx_mutation: bool,
}

impl BearerTokenValidator for Ed25519JwtValidator<'_> {
    type Error = JwtValidationError;

    fn validate(&self, token: &str) -> Result<ValidatedClaims, Self::Error> {
        let mut segments = token.split('.');
        let encoded_header = segments.next().ok_or(JwtValidationError::Malformed)?;
        let encoded_claims = segments.next().ok_or(JwtValidationError::Malformed)?;
        let encoded_signature = segments.next().ok_or(JwtValidationError::Malformed)?;
        if segments.next().is_some()
            || encoded_header.is_empty()
            || encoded_claims.is_empty()
            || encoded_signature.is_empty()
        {
            return Err(JwtValidationError::Malformed);
        }

        let header_bytes = URL_SAFE_NO_PAD
            .decode(encoded_header)
            .map_err(|_| JwtValidationError::Malformed)?;
        let header: JwtHeader<'_> =
            serde_json::from_slice(&header_bytes).map_err(|_| JwtValidationError::InvalidHeader)?;
        if header.alg != "EdDSA" || header.typ.is_some_and(|value| value != "at+jwt") {
            return Err(JwtValidationError::InvalidHeader);
        }

        let signature = URL_SAFE_NO_PAD
            .decode(encoded_signature)
            .map_err(|_| JwtValidationError::Malformed)?;
        let signing_input = format!("{encoded_header}.{encoded_claims}");
        let public_key: &[u8; 32] = self
            .public_key
            .try_into()
            .map_err(|_| JwtValidationError::InvalidSignature)?;
        let public_key = VerifyingKey::from_bytes(public_key)
            .map_err(|_| JwtValidationError::InvalidSignature)?;
        let signature =
            Signature::from_slice(&signature).map_err(|_| JwtValidationError::InvalidSignature)?;
        public_key
            .verify(signing_input.as_bytes(), &signature)
            .map_err(|_| JwtValidationError::InvalidSignature)?;

        let claim_bytes = URL_SAFE_NO_PAD
            .decode(encoded_claims)
            .map_err(|_| JwtValidationError::Malformed)?;
        let claims: JwtClaims<'_> =
            serde_json::from_slice(&claim_bytes).map_err(|_| JwtValidationError::InvalidClaims)?;
        if claims.iss != self.issuer && !self.issuer.split(',').any(|i| i.trim() == claims.iss) {
            return Err(JwtValidationError::WrongIssuer);
        }
        if claims.aud.is_empty() {
            return Err(JwtValidationError::InvalidClaims);
        }
        let scopes = claims
            .scope
            .split_ascii_whitespace()
            .map(parse_scope)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ValidatedClaims {
            audience: claims.aud.to_owned(),
            expires_at_unix_seconds: claims.exp,
            scopes,
            mutation_enabled: claims.onyx_mutation,
        })
    }
}

fn parse_scope(value: &str) -> Result<Scope, JwtValidationError> {
    match value {
        "kvm:read" => Ok(Scope::Read),
        "kvm:video" => Ok(Scope::Video),
        "kvm:audio-capture" => Ok(Scope::AudioCapture),
        "kvm:audio-output" => Ok(Scope::AudioOutput),
        "kvm:control" => Ok(Scope::Control),
        "kvm:media" => Ok(Scope::Media),
        "kvm:admin" => Ok(Scope::Admin),
        _ => Err(JwtValidationError::UnknownScope),
    }
}

/// Authenticate one remote HTTP request and discard its raw credential.
///
/// Origin is checked before token validation. Returned claims are bounded and
/// contain no bearer-token material; the MCP session independently enforces
/// audience, expiry, per-tool scopes, and the mutation gate.
///
/// # Errors
/// Returns a boundary, validator, or bounded-claims error.
pub fn authenticate_http_request<V: BearerTokenValidator>(
    origin: Option<&str>,
    authorization: Option<&str>,
    expected_origin: &str,
    validator: &V,
) -> Result<ValidatedClaims, HttpRequestAuthError<V::Error>> {
    validate_origin(origin, expected_origin).map_err(HttpRequestAuthError::Boundary)?;
    let token = extract_bearer(authorization).map_err(HttpRequestAuthError::Boundary)?;
    let claims = validator
        .validate(token)
        .map_err(HttpRequestAuthError::InvalidToken)?;
    if claims.audience.is_empty()
        || claims.scopes.len() > MAX_GRANT_SCOPES
        || claims.scopes.iter().enumerate().any(|(index, scope)| {
            claims
                .scopes
                .get(index + 1..)
                .is_some_and(|tail| tail.contains(scope))
        })
    {
        return Err(HttpRequestAuthError::InvalidClaims);
    }
    Ok(claims)
}

/// Produce RFC 9728 protected-resource metadata for the remote MCP endpoint.
#[must_use]
pub fn protected_resource_metadata(resource: &str, authorization_server: &str) -> Value {
    json!({
        "resource": resource,
        "authorization_servers": [authorization_server],
        "scopes_supported": [
            "kvm:read",
            "kvm:video",
            "kvm:audio-capture",
            "kvm:audio-output",
            "kvm:control",
            "kvm:media",
            "kvm:admin"
        ],
        "bearer_methods_supported": ["header"]
    })
}

/// Construct the Bearer challenge used by unauthorized MCP HTTP requests.
#[must_use]
pub fn bearer_challenge(resource_metadata_uri: &str, required_scope: &str) -> String {
    format!("Bearer resource_metadata=\"{resource_metadata_uri}\", scope=\"{required_scope}\"")
}

/// Reject cross-origin browser requests before parsing MCP messages.
///
/// Non-browser clients commonly omit `Origin`; when present it must exactly
/// match the configured HTTPS origin.
///
/// # Errors
///
/// Returns [`HttpAuthError::InvalidOrigin`] for a non-matching origin.
pub fn validate_origin(origin: Option<&str>, expected_origin: &str) -> Result<(), HttpAuthError> {
    if origin.is_none_or(|value| {
        expected_origin
            .split(',')
            .any(|expected| expected.trim() == value)
    }) {
        Ok(())
    } else {
        Err(HttpAuthError::InvalidOrigin)
    }
}

/// Extract a bounded Bearer credential from one Authorization header.
///
/// Integrity, issuer, audience, expiry, and scope validation remain the OAuth
/// validator's responsibility. Query-string tokens are never accepted here.
///
/// # Errors
///
/// Returns [`HttpAuthError`] when the header is absent, is not a Bearer
/// credential, or contains an empty, oversized, or malformed token.
pub fn extract_bearer(header: Option<&str>) -> Result<&str, HttpAuthError> {
    let header = header.ok_or(HttpAuthError::MissingAuthorization)?;
    let token = header
        .strip_prefix("Bearer ")
        .ok_or(HttpAuthError::InvalidAuthorization)?;
    if token.is_empty()
        || token.len() > MAX_BEARER_TOKEN_BYTES
        || token
            .bytes()
            .any(|byte| byte.is_ascii_whitespace() || byte.is_ascii_control())
    {
        return Err(HttpAuthError::InvalidAuthorization);
    }
    Ok(token)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum State {
    AwaitInitialize,
    AwaitInitialized,
    Ready,
}

#[derive(Clone, Copy, Debug)]
enum Authority<'a> {
    LocalRecovery,
    Remote {
        expected_audience: &'a str,
        now_unix_seconds: u64,
        grant: Grant<'a>,
    },
}

pub struct Server<'a> {
    state: State,
    authority: Authority<'a>,
    hid: Option<&'a dyn HidTransport>,
    video: Option<&'a dyn VideoTransport>,
    audio_capture: Option<&'a dyn AudioCaptureTransport>,
    audio_output: Option<&'a dyn AudioOutputTransport>,
}

impl Default for Server<'static> {
    fn default() -> Self {
        Self {
            state: State::AwaitInitialize,
            authority: Authority::LocalRecovery,
            hid: None,
            video: None,
            audio_capture: None,
            audio_output: None,
        }
    }
}

impl<'a> Server<'a> {
    /// Construct an MCP session from cryptographically validated remote claims.
    /// Bearer-token material must never be passed into this type.
    #[must_use]
    pub const fn remote(
        expected_audience: &'a str,
        now_unix_seconds: u64,
        grant: Grant<'a>,
    ) -> Self {
        Self {
            state: State::AwaitInitialize,
            authority: Authority::Remote {
                expected_audience,
                now_unix_seconds,
                grant,
            },
            hid: None,
            video: None,
            audio_capture: None,
            audio_output: None,
        }
    }

    /// Construct an already-initialized stateless HTTP request context.
    /// Transport code must authenticate every request and must not retain raw
    /// bearer material between calls.
    #[must_use]
    pub const fn remote_ready(
        expected_audience: &'a str,
        now_unix_seconds: u64,
        grant: Grant<'a>,
    ) -> Self {
        Self {
            state: State::Ready,
            authority: Authority::Remote {
                expected_audience,
                now_unix_seconds,
                grant,
            },
            hid: None,
            video: None,
            audio_capture: None,
            audio_output: None,
        }
    }

    /// Construct a remote stateless request with the privileged HID transport.
    #[must_use]
    pub const fn remote_ready_with_hid(
        expected_audience: &'a str,
        now_unix_seconds: u64,
        grant: Grant<'a>,
        hid: &'a dyn HidTransport,
    ) -> Self {
        Self {
            state: State::Ready,
            authority: Authority::Remote {
                expected_audience,
                now_unix_seconds,
                grant,
            },
            hid: Some(hid),
            video: None,
            audio_capture: None,
            audio_output: None,
        }
    }

    /// Attach the privileged video snapshot transport to this session.
    #[must_use]
    pub const fn with_video(mut self, video: &'a dyn VideoTransport) -> Self {
        self.video = Some(video);
        self
    }

    /// Attach the independently gated microphone-capture transport.
    #[must_use]
    pub const fn with_audio_capture(mut self, audio: &'a dyn AudioCaptureTransport) -> Self {
        self.audio_capture = Some(audio);
        self
    }

    /// Attach the independently gated audio-output transport.
    #[must_use]
    pub const fn with_audio_output(mut self, audio: &'a dyn AudioOutputTransport) -> Self {
        self.audio_output = Some(audio);
        self
    }

    fn authorize(&self, name: &str) -> Result<Operation, PolicyError> {
        match self.authority {
            Authority::LocalRecovery => authorize(name, &[Scope::Read], false),
            Authority::Remote {
                expected_audience,
                now_unix_seconds,
                grant,
            } => authorize_grant(name, expected_audience, now_unix_seconds, grant),
        }
    }

    #[must_use]
    pub fn handle_line(&mut self, line: &[u8]) -> Option<String> {
        let message: Value = match serde_json::from_slice(line) {
            Ok(message) => message,
            Err(_) => return Some(error(&Value::Null, -32700, "Parse error")),
        };
        let Some(object) = message.as_object() else {
            return Some(error(&Value::Null, -32600, "Invalid Request"));
        };
        let id = object.get("id").cloned();
        let method = object.get("method").and_then(Value::as_str);
        if object.get("jsonrpc").and_then(Value::as_str) != Some("2.0") || method.is_none() {
            return Some(error(&id.unwrap_or(Value::Null), -32600, "Invalid Request"));
        }

        let method = method.unwrap_or_default();
        if id.is_none() {
            self.handle_notification(method);
            return None;
        }
        let id = id.unwrap_or(Value::Null);
        if !matches!(id, Value::String(_) | Value::Number(_)) {
            return Some(error(&Value::Null, -32600, "Invalid Request"));
        }

        Some(match method {
            "initialize" => self.initialize(&id, object.get("params")),
            "ping" if self.state != State::AwaitInitialize => success(&id, &json!({})),
            "tools/list" if self.state == State::Ready => success(&id, &self.list_tools()),
            "tools/call" if self.state == State::Ready => self.call_tool(&id, object.get("params")),
            "tools/list" | "tools/call" | "ping" => error(&id, -32002, "Server is not initialized"),
            _ => error(&id, -32601, "Method not found"),
        })
    }

    fn handle_notification(&mut self, method: &str) {
        if method == "notifications/initialized" && self.state == State::AwaitInitialized {
            self.state = State::Ready;
        }
    }

    fn initialize(&mut self, id: &Value, params: Option<&Value>) -> String {
        if self.state != State::AwaitInitialize {
            return error(id, -32600, "Already initialized");
        }
        let version = params
            .and_then(Value::as_object)
            .and_then(|params| params.get("protocolVersion"))
            .and_then(Value::as_str);
        let capabilities = params
            .and_then(Value::as_object)
            .and_then(|params| params.get("capabilities"))
            .and_then(Value::as_object);
        let client_info = params
            .and_then(Value::as_object)
            .and_then(|params| params.get("clientInfo"))
            .and_then(Value::as_object);
        let client_info_is_valid = client_info.is_some_and(|info| {
            info.get("name").and_then(Value::as_str).is_some()
                && info.get("version").and_then(Value::as_str).is_some()
        });
        if version != Some(PROTOCOL_VERSION) {
            return error(id, -32602, "Unsupported protocol version");
        }
        if capabilities.is_none() || !client_info_is_valid {
            return error(id, -32602, "Invalid initialization parameters");
        }
        self.state = State::AwaitInitialized;
        success(
            id,
            &json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": { "tools": { "listChanged": false } },
                "serverInfo": {
                    "name": "onyx-kvm",
                    "title": "Onyx KVM local recovery",
                    "version": env!("CARGO_PKG_VERSION"),
                    "description": "Local, read-only Onyx KVM recovery interface"
                },
                "instructions": "Only read-only local recovery status is enabled."
            }),
        )
    }

    fn call_tool(&self, id: &Value, params: Option<&Value>) -> String {
        let Some(params) = params.and_then(Value::as_object) else {
            return error(id, -32602, "Invalid tool arguments");
        };
        if params.contains_key("task") {
            return error(id, -32601, "Task-augmented execution is not supported");
        }
        let Some(name) = params.get("name").and_then(Value::as_str) else {
            return error(id, -32602, "Unknown or disabled tool");
        };
        if matches!(name, "kvm.keyboard.send" | "kvm.pointer.send") {
            return self.call_hid_tool(id, name, params.get("arguments"));
        }
        if name == "kvm.video.snapshot" {
            return self.call_video_snapshot(id, params.get("arguments"));
        }
        if matches!(name, "kvm.audio.capture" | "kvm.audio.play") {
            return self.call_audio_tool(id, name, params.get("arguments"));
        }
        if name != "kvm.status" {
            return error(id, -32602, "Unknown or disabled tool");
        }
        let arguments = params.get("arguments");
        let arguments_are_empty =
            arguments.is_none_or(|value| value.as_object().is_some_and(serde_json::Map::is_empty));
        if !arguments_are_empty {
            return error(id, -32602, "kvm.status accepts no arguments");
        }
        if self.authorize("kvm.status").is_err() {
            return error(id, -32603, "Policy denied read-only status");
        }
        let (mode, network_listeners) = match self.authority {
            Authority::LocalRecovery => ("development-recovery", 0),
            Authority::Remote { .. } => ("development-remote", 1),
        };
        let video_enabled = self.video.is_some() && self.authorize("kvm.video.snapshot").is_ok();
        let audio_capture_enabled =
            self.audio_capture.is_some() && self.authorize("kvm.audio.capture").is_ok();
        let audio_output_enabled =
            self.audio_output.is_some() && self.authorize("kvm.audio.play").is_ok();
        let mutating_operations_enabled = match self.authority {
            Authority::LocalRecovery => false,
            Authority::Remote { grant, .. } => {
                grant.mutation_enabled
                    && self.hid.is_some()
                    && self.authorize("kvm.keyboard.send").is_ok()
            }
        };
        let status = json!({
            "mode": mode,
            "networkListeners": network_listeners,
            "mutatingOperationsEnabled": mutating_operations_enabled,
            "videoEnabled": video_enabled,
            "audioCaptureEnabled": audio_capture_enabled,
            "audioOutputEnabled": audio_output_enabled
        });
        success(
            id,
            &json!({
                "content": [{ "type": "text", "text": status.to_string() }],
                "structuredContent": status,
                "isError": false
            }),
        )
    }

    fn call_audio_tool(&self, id: &Value, name: &str, arguments: Option<&Value>) -> String {
        let arguments_are_empty =
            arguments.is_none_or(|value| value.as_object().is_some_and(serde_json::Map::is_empty));
        if !arguments_are_empty {
            return error(id, -32602, "Audio operation accepts no arguments");
        }
        if self.authorize(name).is_err() {
            return error(id, -32603, "Policy denied audio operation");
        }
        let Ok(shape) = PcmShape::new(PcmFormat::Signed16LittleEndian, 16_000, 1, 1_000) else {
            return error(id, -32603, "Internal audio shape rejected");
        };
        let result = match name {
            "kvm.audio.capture" => {
                let Some(audio) = self.audio_capture else {
                    return error(id, -32602, "Unknown or disabled tool");
                };
                let Ok(metrics) = audio.capture_metrics(shape) else {
                    return error(id, -32603, "Audio capture broker rejected request");
                };
                json!({
                    "frames": metrics.frames, "byteLength": metrics.bytes,
                    "sampleRateHz": 16000, "channels": 1, "format": "s16le",
                    "peakAbsolute": metrics.peak_absolute,
                    "meanAbsolute": metrics.mean_absolute, "rms": metrics.rms,
                    "clippedSamples": metrics.clipped_samples,
                    "zeroSamples": metrics.zero_samples, "elapsedMs": metrics.elapsed_ms
                })
            }
            "kvm.audio.play" => {
                let Some(audio) = self.audio_output else {
                    return error(id, -32602, "Unknown or disabled tool");
                };
                let Ok(receipt) = audio.play_bounded_tone(shape) else {
                    return error(id, -32603, "Audio output broker rejected request");
                };
                json!({
                    "frames": receipt.frames, "byteLength": receipt.bytes,
                    "sampleRateHz": 16000, "channels": 1, "format": "s16le",
                    "toneHz": receipt.tone_hz, "peak": receipt.peak,
                    "elapsedMs": receipt.elapsed_ms, "completed": true
                })
            }
            _ => return error(id, -32602, "Unknown or disabled tool"),
        };
        success(
            id,
            &json!({
                "content": [{ "type": "text", "text": result.to_string() }],
                "structuredContent": result,
                "isError": false
            }),
        )
    }

    fn call_video_snapshot(&self, id: &Value, arguments: Option<&Value>) -> String {
        let Some(video) = self.video else {
            return error(id, -32602, "Unknown or disabled tool");
        };
        if self.authorize("kvm.video.snapshot").is_err() {
            return error(id, -32603, "Policy denied video snapshot");
        }
        let Some(request) = parse_snapshot_arguments(arguments) else {
            return error(id, -32602, "Invalid video snapshot arguments");
        };
        let Ok(snapshot) = video.capture_snapshot(request) else {
            return error(id, -32603, "Video broker rejected snapshot");
        };
        let result = json!({
            "width": snapshot.width,
            "height": snapshot.height,
            "format": snapshot.format.as_str(),
            "byteLength": snapshot.byte_length,
            "sha256": snapshot.sha256_hex(),
        });
        success(
            id,
            &json!({
                "content": [{ "type": "text", "text": result.to_string() }],
                "structuredContent": result,
                "isError": false
            }),
        )
    }

    fn call_hid_tool(&self, id: &Value, name: &str, arguments: Option<&Value>) -> String {
        let Some(hid) = self.hid else {
            return error(id, -32602, "Unknown or disabled tool");
        };
        if self.authorize(name).is_err() {
            return error(id, -32603, "Policy denied HID report");
        }
        let Some(report) = arguments
            .and_then(Value::as_object)
            .filter(|object| object.len() == 1)
            .and_then(|object| object.get("report"))
            .and_then(Value::as_array)
        else {
            return error(id, -32602, "HID report must be one byte array");
        };
        let Some(bytes) = report
            .iter()
            .map(|value| value.as_u64().and_then(|byte| u8::try_from(byte).ok()))
            .collect::<Option<Vec<_>>>()
        else {
            return error(id, -32602, "HID report contains an invalid byte");
        };
        let result = match name {
            "kvm.keyboard.send" => match <[u8; 8]>::try_from(bytes.as_slice()) {
                Ok(report) => hid.send_keyboard(report),
                Err(_) => return error(id, -32602, "Keyboard report must contain 8 bytes"),
            },
            "kvm.pointer.send" => match <[u8; 4]>::try_from(bytes.as_slice()) {
                Ok(report) => hid.send_pointer(report),
                Err(_) => return error(id, -32602, "Pointer report must contain 4 bytes"),
            },
            _ => return error(id, -32602, "Unknown or disabled tool"),
        };
        if result.is_err() {
            return error(id, -32603, "HID broker rejected report");
        }
        success(
            id,
            &json!({
                "content": [{ "type": "text", "text": "HID report accepted" }],
                "structuredContent": { "accepted": true },
                "isError": false
            }),
        )
    }

    fn list_tools(&self) -> Value {
        let enabled = self
            .authorize("kvm.status")
            .ok()
            .is_some_and(|operation| operation.effect == Effect::ReadOnly);
        let mut tools = Vec::new();
        if enabled {
            tools.push(json!({
                "name": "kvm.status",
                "title": "Onyx KVM status",
                "description": "Read the local recovery-mode security status",
                "inputSchema": {
                    "$schema": "https://json-schema.org/draft/2020-12/schema",
                    "type": "object",
                    "additionalProperties": false
                },
                "outputSchema": {
                    "$schema": "https://json-schema.org/draft/2020-12/schema",
                    "type": "object",
                    "properties": {
                        "mode": { "type": "string" },
                        "networkListeners": { "type": "integer", "minimum": 0 },
                        "mutatingOperationsEnabled": { "type": "boolean" },
                        "videoEnabled": { "type": "boolean" },
                        "audioCaptureEnabled": { "type": "boolean" },
                        "audioOutputEnabled": { "type": "boolean" }
                    },
                    "required": ["mode", "networkListeners", "mutatingOperationsEnabled",
                        "videoEnabled", "audioCaptureEnabled", "audioOutputEnabled"],
                    "additionalProperties": false
                },
                "annotations": {
                    "readOnlyHint": true,
                    "destructiveHint": false,
                    "idempotentHint": true,
                    "openWorldHint": false
                },
                "execution": { "taskSupport": "forbidden" }
            }));
        }
        if self.video.is_some() && self.authorize("kvm.video.snapshot").is_ok() {
            tools.push(video_snapshot_tool());
        }
        if self.hid.is_some() && self.authorize("kvm.keyboard.send").is_ok() {
            tools.push(hid_tool("kvm.keyboard.send", "Send keyboard report", 8));
            tools.push(hid_tool("kvm.pointer.send", "Send pointer report", 4));
        }
        if self.audio_capture.is_some() && self.authorize("kvm.audio.capture").is_ok() {
            tools.push(audio_tool(
                "kvm.audio.capture",
                "Capture microphone metrics",
                true,
            ));
        }
        if self.audio_output.is_some() && self.authorize("kvm.audio.play").is_ok() {
            tools.push(audio_tool(
                "kvm.audio.play",
                "Play bounded test tone",
                false,
            ));
        }
        json!({ "tools": tools })
    }
}

fn audio_tool(name: &str, title: &str, read_only: bool) -> Value {
    json!({
        "name": name,
        "title": title,
        "description": if read_only {
            "Capture one second of admitted microphone PCM and return aggregate metrics only"
        } else {
            "Play the admitted one-second low-amplitude diagnostic tone and restore safe mute"
        },
        "inputSchema": {
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "type": "object", "additionalProperties": false
        },
        "outputSchema": { "$schema": "https://json-schema.org/draft/2020-12/schema", "type": "object" },
        "annotations": {
            "readOnlyHint": read_only, "destructiveHint": false,
            "idempotentHint": false, "openWorldHint": false
        },
        "execution": { "taskSupport": "forbidden" }
    })
}

fn parse_snapshot_arguments(arguments: Option<&Value>) -> Option<SnapshotRequest> {
    let Some(arguments) = arguments else {
        return SnapshotRequest::new(1920, 1080, SnapshotFormat::Yuv420p, 5_000).ok();
    };
    let object = arguments.as_object()?;
    for key in object.keys() {
        if !matches!(key.as_str(), "width" | "height" | "format" | "deadlineMs") {
            return None;
        }
    }
    let width = match object.get("width") {
        Some(value) => u16::try_from(value.as_u64()?).ok()?,
        None => 1920,
    };
    let height = match object.get("height") {
        Some(value) => u16::try_from(value.as_u64()?).ok()?,
        None => 1080,
    };
    let format = match object.get("format") {
        Some(value) => match value.as_str()? {
            "jpeg" => SnapshotFormat::Jpeg,
            "rgb565" => SnapshotFormat::Rgb565,
            "yuv422p" => SnapshotFormat::Yuv422p,
            "yuv420p" => SnapshotFormat::Yuv420p,
            _ => return None,
        },
        None => SnapshotFormat::Yuv420p,
    };
    let deadline_ms = match object.get("deadlineMs") {
        Some(value) => u16::try_from(value.as_u64()?).ok()?,
        None => 5_000,
    };
    SnapshotRequest::new(width, height, format, deadline_ms).ok()
}

fn video_snapshot_tool() -> Value {
    json!({
        "name": "kvm.video.snapshot",
        "title": "Capture video frame snapshot",
        "description": "Capture one bounded video frame snapshot through the privileged video broker",
        "inputSchema": {
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "type": "object",
            "properties": {
                "width": { "type": "integer", "minimum": 1, "maximum": 1920 },
                "height": { "type": "integer", "minimum": 1, "maximum": 1080 },
                "format": { "type": "string", "enum": ["yuv420p", "yuv422p"] },
                "deadlineMs": { "type": "integer", "minimum": 100, "maximum": 5000 }
            },
            "additionalProperties": false
        },
        "outputSchema": {
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "type": "object",
            "properties": {
                "width": { "type": "integer", "minimum": 1, "maximum": 1920 },
                "height": { "type": "integer", "minimum": 1, "maximum": 1080 },
                "format": { "type": "string", "enum": ["yuv420p", "yuv422p"] },
                "byteLength": { "type": "integer", "minimum": 1, "maximum": 4_194_304 },
                "sha256": { "type": "string", "minLength": 64, "maxLength": 64 }
            },
            "required": ["width", "height", "format", "byteLength", "sha256"],
            "additionalProperties": false
        },
        "annotations": {
            "readOnlyHint": true, "destructiveHint": false,
            "idempotentHint": false, "openWorldHint": false
        },
        "execution": { "taskSupport": "forbidden" }
    })
}

fn hid_tool(name: &str, title: &str, length: usize) -> Value {
    json!({
        "name": name,
        "title": title,
        "description": "Send one fixed-size USB HID report through the local privileged broker",
        "inputSchema": {
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "type": "object",
            "properties": { "report": {
                "type": "array", "items": { "type": "integer", "minimum": 0, "maximum": 255 },
                "minItems": length, "maxItems": length
            }},
            "required": ["report"], "additionalProperties": false
        },
        "annotations": {
            "readOnlyHint": false, "destructiveHint": false,
            "idempotentHint": false, "openWorldHint": false
        },
        "execution": { "taskSupport": "forbidden" }
    })
}

fn success(id: &Value, result: &Value) -> String {
    json!({ "jsonrpc": "2.0", "id": id, "result": result }).to_string()
}

fn error(id: &Value, code: i64, message: &str) -> String {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } }).to_string()
}

/// Serve one bounded, newline-delimited MCP session.
///
/// # Errors
/// Returns an I/O error if stdin cannot be read or stdout cannot be written.
pub fn serve<R: BufRead, W: Write>(mut input: R, mut output: W) -> io::Result<()> {
    let mut server = Server::default();
    let mut line = Vec::new();
    loop {
        line.clear();
        let bytes = std::io::Read::take(&mut input, (MAX_MESSAGE_BYTES + 1) as u64)
            .read_until(b'\n', &mut line)?;
        if bytes == 0 {
            return Ok(());
        }
        let has_newline = line.ends_with(b"\n");
        if line.len() > MAX_MESSAGE_BYTES || !has_newline {
            if !has_newline {
                discard_line(&mut input)?;
            }
            let message = if line.len() > MAX_MESSAGE_BYTES {
                "Message too large"
            } else {
                "Invalid stdio framing"
            };
            writeln!(output, "{}", error(&Value::Null, -32600, message))?;
            output.flush()?;
            continue;
        }
        line.pop();
        if line.last() == Some(&b'\r') {
            line.pop();
        }
        if let Some(response) = server.handle_line(&line) {
            writeln!(output, "{response}")?;
            output.flush()?;
        }
    }
}

fn discard_line<R: BufRead>(input: &mut R) -> io::Result<()> {
    loop {
        let buffer = input.fill_buf()?;
        if buffer.is_empty() {
            return Ok(());
        }
        if let Some(index) = buffer.iter().position(|byte| *byte == b'\n') {
            input.consume(index + 1);
            return Ok(());
        }
        let length = buffer.len();
        input.consume(length);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AudioCaptureTransport, AudioOutputTransport, BearerTokenValidator, CaptureMetrics,
        Ed25519JwtValidator, HidTransport, HttpAuthError, HttpRequestAuthError, JwtValidationError,
        MAX_MESSAGE_BYTES, PcmShape, PlaybackReceipt, Server, SnapshotFormat, SnapshotRequest,
        ValidatedClaims, VideoSnapshot, VideoTransport, authenticate_http_request,
        bearer_challenge, extract_bearer, protected_resource_metadata, serve, validate_origin,
    };
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
    use ed25519_dalek::{Signer, SigningKey};
    use onyx_kvm_core::{Grant, Scope};
    use serde_json::Value;
    use std::{
        cell::Cell,
        io::{self, BufReader, Cursor},
    };

    fn response(server: &mut Server, request: &str) -> Value {
        let text = server.handle_line(request.as_bytes()).unwrap_or_default();
        serde_json::from_str(&text).unwrap_or(Value::Null)
    }

    fn initialize(server: &mut Server) {
        let result = response(
            server,
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"test","version":"1"}}}"#,
        );
        assert_eq!(result["result"]["protocolVersion"], "2025-11-25");
        assert!(
            server
                .handle_line(br#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
                .is_none()
        );
    }

    #[test]
    fn lifecycle_and_status_call() {
        let mut server = Server::default();
        let early = response(
            &mut server,
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#,
        );
        assert_eq!(early["error"]["code"], -32002);
        initialize(&mut server);
        let tools = response(
            &mut server,
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#,
        );
        assert_eq!(tools["result"]["tools"].as_array().map(Vec::len), Some(1));
        let call = response(
            &mut server,
            r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"kvm.status","arguments":{}}}"#,
        );
        assert_eq!(call["result"]["structuredContent"]["networkListeners"], 0);
    }

    #[test]
    fn disabled_and_argumented_tools_fail_closed() {
        let mut server = Server::default();
        initialize(&mut server);
        let disabled = response(
            &mut server,
            r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"kvm.keyboard.send","arguments":{}}}"#,
        );
        assert_eq!(disabled["error"]["code"], -32602);
        let arguments = response(
            &mut server,
            r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"kvm.status","arguments":{"extra":true}}}"#,
        );
        assert_eq!(arguments["error"]["code"], -32602);
    }

    #[test]
    fn oversized_message_is_rejected_and_next_message_survives() {
        let mut input = vec![b'x'; MAX_MESSAGE_BYTES + 1];
        input.extend_from_slice(b"\n{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2025-11-25\",\"capabilities\":{},\"clientInfo\":{\"name\":\"test\",\"version\":\"1\"}}}\n");
        let mut output = Vec::new();
        serve(BufReader::new(Cursor::new(input)), &mut output).unwrap_or_default();
        assert_eq!(
            output
                .split(|byte| *byte == b'\n')
                .filter(|line| !line.is_empty())
                .count(),
            2
        );
    }

    #[test]
    fn initialization_requires_client_identity_and_capabilities() {
        let mut server = Server::default();
        let result = response(
            &mut server,
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25"}}"#,
        );
        assert_eq!(result["error"]["code"], -32602);
    }

    #[test]
    fn task_augmented_tool_calls_are_forbidden() {
        let mut server = Server::default();
        initialize(&mut server);
        let result = response(
            &mut server,
            r#"{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"kvm.status","arguments":{},"task":{}}}"#,
        );
        assert_eq!(result["error"]["code"], -32601);
    }

    #[test]
    fn protected_resource_metadata_is_audience_specific() {
        let metadata =
            protected_resource_metadata("https://kvm.example/mcp", "https://kvm.example/authorize");
        assert_eq!(metadata["resource"], "https://kvm.example/mcp");
        assert_eq!(
            metadata["authorization_servers"][0],
            "https://kvm.example/authorize"
        );
        assert_eq!(metadata["bearer_methods_supported"][0], "header");
    }

    #[test]
    fn bearer_credentials_are_header_only_and_bounded() {
        assert_eq!(
            extract_bearer(Some("Bearer opaque-token")),
            Ok("opaque-token")
        );
        assert_eq!(
            extract_bearer(None),
            Err(HttpAuthError::MissingAuthorization)
        );
        assert_eq!(
            extract_bearer(Some("Basic abc")),
            Err(HttpAuthError::InvalidAuthorization)
        );
        assert_eq!(
            extract_bearer(Some("Bearer two tokens")),
            Err(HttpAuthError::InvalidAuthorization)
        );
    }

    #[test]
    fn browser_origin_is_exact_match() {
        assert_eq!(validate_origin(None, "https://kvm.example"), Ok(()));
        assert_eq!(
            validate_origin(Some("https://kvm.example"), "https://kvm.example"),
            Ok(())
        );
        assert_eq!(
            validate_origin(
                Some("https://192.168.2.106:8443"),
                "https://onyx-kvm-recovery:8443,https://192.168.2.106:8443"
            ),
            Ok(())
        );
        assert_eq!(
            validate_origin(Some("https://evil.example"), "https://kvm.example"),
            Err(HttpAuthError::InvalidOrigin)
        );
        assert_eq!(
            bearer_challenge(
                "https://kvm.example/.well-known/oauth-protected-resource",
                "kvm:read"
            ),
            "Bearer resource_metadata=\"https://kvm.example/.well-known/oauth-protected-resource\", scope=\"kvm:read\""
        );
    }

    #[test]
    fn remote_session_uses_validated_grant_claims() {
        let grant = Grant {
            audience: "https://kvm.example/mcp",
            expires_at_unix_seconds: 200,
            scopes: &[Scope::Read],
            mutation_enabled: false,
        };
        let mut server = Server::remote("https://kvm.example/mcp", 100, grant);
        initialize(&mut server);
        let tools = response(
            &mut server,
            r#"{"jsonrpc":"2.0","id":7,"method":"tools/list"}"#,
        );
        assert_eq!(tools["result"]["tools"].as_array().map(Vec::len), Some(1));
        let call = response(
            &mut server,
            r#"{"jsonrpc":"2.0","id":8,"method":"tools/call","params":{"name":"kvm.status","arguments":{}}}"#,
        );
        assert_eq!(
            call["result"]["structuredContent"]["mode"],
            "development-remote"
        );
        assert_eq!(call["result"]["structuredContent"]["networkListeners"], 1);
    }

    #[test]
    fn remote_session_fails_closed_for_invalid_claims() {
        let missing_scope = Grant {
            audience: "https://kvm.example/mcp",
            expires_at_unix_seconds: 200,
            scopes: &[],
            mutation_enabled: false,
        };
        let mut server = Server::remote("https://kvm.example/mcp", 100, missing_scope);
        initialize(&mut server);
        let tools = response(
            &mut server,
            r#"{"jsonrpc":"2.0","id":9,"method":"tools/list"}"#,
        );
        assert_eq!(tools["result"]["tools"].as_array().map(Vec::len), Some(0));

        let expired = Grant {
            audience: "https://kvm.example/mcp",
            expires_at_unix_seconds: 100,
            scopes: &[Scope::Read],
            mutation_enabled: false,
        };
        let mut server = Server::remote("https://kvm.example/mcp", 100, expired);
        initialize(&mut server);
        let call = response(
            &mut server,
            r#"{"jsonrpc":"2.0","id":10,"method":"tools/call","params":{"name":"kvm.status","arguments":{}}}"#,
        );
        assert_eq!(call["error"]["code"], -32603);
    }

    struct RecordingHid {
        keyboard: Cell<Option<[u8; 8]>>,
        pointer: Cell<Option<[u8; 4]>>,
    }

    impl HidTransport for RecordingHid {
        fn send_keyboard(&self, report: [u8; 8]) -> io::Result<()> {
            self.keyboard.set(Some(report));
            Ok(())
        }

        fn send_pointer(&self, report: [u8; 4]) -> io::Result<()> {
            self.pointer.set(Some(report));
            Ok(())
        }
    }

    #[test]
    fn authorized_hid_tools_accept_only_fixed_byte_reports() {
        let scopes = [Scope::Control];
        let grant = Grant {
            audience: "https://kvm.example/mcp",
            expires_at_unix_seconds: 200,
            scopes: &scopes,
            mutation_enabled: true,
        };
        let hid = RecordingHid {
            keyboard: Cell::new(None),
            pointer: Cell::new(None),
        };
        let mut server = Server::remote_ready_with_hid("https://kvm.example/mcp", 100, grant, &hid);
        let tools = response(
            &mut server,
            r#"{"jsonrpc":"2.0","id":11,"method":"tools/list"}"#,
        );
        assert_eq!(tools["result"]["tools"].as_array().map(Vec::len), Some(2));
        let accepted = response(
            &mut server,
            r#"{"jsonrpc":"2.0","id":12,"method":"tools/call","params":{"name":"kvm.keyboard.send","arguments":{"report":[0,0,4,0,0,0,0,0]}}}"#,
        );
        assert_eq!(accepted["result"]["structuredContent"]["accepted"], true);
        assert_eq!(hid.keyboard.get(), Some([0, 0, 4, 0, 0, 0, 0, 0]));
        let malformed = response(
            &mut server,
            r#"{"jsonrpc":"2.0","id":13,"method":"tools/call","params":{"name":"kvm.pointer.send","arguments":{"report":[0,0,0,256]}}}"#,
        );
        assert_eq!(malformed["error"]["code"], -32602);
        assert_eq!(hid.pointer.get(), None);

        let read_control = [Scope::Read, Scope::Control];
        let full_grant = Grant {
            audience: "https://kvm.example/mcp",
            expires_at_unix_seconds: 200,
            scopes: &read_control,
            mutation_enabled: true,
        };
        let mut server =
            Server::remote_ready_with_hid("https://kvm.example/mcp", 100, full_grant, &hid);
        let status = response(
            &mut server,
            r#"{"jsonrpc":"2.0","id":14,"method":"tools/call","params":{"name":"kvm.status","arguments":{}}}"#,
        );
        assert_eq!(
            status["result"]["structuredContent"]["mutatingOperationsEnabled"],
            true
        );

        let non_mutating_grant = Grant {
            audience: "https://kvm.example/mcp",
            expires_at_unix_seconds: 200,
            scopes: &read_control,
            mutation_enabled: false,
        };
        let mut server =
            Server::remote_ready_with_hid("https://kvm.example/mcp", 100, non_mutating_grant, &hid);
        let status = response(
            &mut server,
            r#"{"jsonrpc":"2.0","id":15,"method":"tools/call","params":{"name":"kvm.status","arguments":{}}}"#,
        );
        assert_eq!(
            status["result"]["structuredContent"]["mutatingOperationsEnabled"],
            false
        );
    }

    struct TestValidator;

    impl BearerTokenValidator for TestValidator {
        type Error = &'static str;

        fn validate(&self, token: &str) -> Result<ValidatedClaims, Self::Error> {
            if token != "signed-token" {
                return Err("bad signature");
            }
            Ok(ValidatedClaims {
                audience: "https://kvm.example/mcp".to_owned(),
                expires_at_unix_seconds: 200,
                scopes: vec![Scope::Read],
                mutation_enabled: false,
            })
        }
    }

    #[test]
    fn http_authentication_returns_only_owned_bounded_claims() {
        let claims = authenticate_http_request(
            Some("https://kvm.example"),
            Some("Bearer signed-token"),
            "https://kvm.example",
            &TestValidator,
        )
        .unwrap_or_else(|error| panic!("unexpected authentication error: {error:?}"));
        assert_eq!(claims.audience, "https://kvm.example/mcp");
        assert_eq!(claims.scopes, vec![Scope::Read]);
        assert!(!claims.audience.contains("signed-token"));
    }

    #[test]
    fn http_authentication_rejects_boundary_and_validator_failures() {
        assert_eq!(
            authenticate_http_request(
                Some("https://evil.example"),
                Some("Bearer signed-token"),
                "https://kvm.example",
                &TestValidator,
            ),
            Err(HttpRequestAuthError::Boundary(HttpAuthError::InvalidOrigin))
        );
        assert_eq!(
            authenticate_http_request(
                None,
                Some("Bearer forged-token"),
                "https://kvm.example",
                &TestValidator,
            ),
            Err(HttpRequestAuthError::InvalidToken("bad signature"))
        );
    }

    fn signed_token(header: &str, claims: &str) -> (String, Vec<u8>) {
        let key_pair = SigningKey::from_bytes(&[42; 32]);
        let header = URL_SAFE_NO_PAD.encode(header);
        let claims = URL_SAFE_NO_PAD.encode(claims);
        let input = format!("{header}.{claims}");
        let signature = URL_SAFE_NO_PAD.encode(key_pair.sign(input.as_bytes()).to_bytes());
        (
            format!("{input}.{signature}"),
            key_pair.verifying_key().to_bytes().to_vec(),
        )
    }

    #[test]
    fn ed25519_jwt_validation_extracts_authenticated_claims() {
        let (token, public_key) = signed_token(
            r#"{"alg":"EdDSA","typ":"at+jwt"}"#,
            r#"{"iss":"https://issuer.example","aud":"https://kvm.example/mcp","exp":200,"scope":"kvm:read kvm:video","cleankvm_mutation":false}"#,
        );
        let validator = Ed25519JwtValidator::new("https://issuer.example", &public_key);
        let claims = validator
            .validate(&token)
            .unwrap_or_else(|error| panic!("unexpected JWT error: {error:?}"));
        assert_eq!(claims.audience, "https://kvm.example/mcp");
        assert_eq!(claims.expires_at_unix_seconds, 200);
        assert_eq!(claims.scopes, vec![Scope::Read, Scope::Video]);
        assert!(!claims.mutation_enabled);
    }

    #[test]
    fn ed25519_jwt_validation_rejects_tampering_and_wrong_issuer() {
        let (token, public_key) = signed_token(
            r#"{"alg":"EdDSA","typ":"at+jwt"}"#,
            r#"{"iss":"https://issuer.example","aud":"https://kvm.example/mcp","exp":200,"scope":"kvm:read"}"#,
        );
        let validator = Ed25519JwtValidator::new("https://issuer.example", &public_key);
        let mut tampered = token.into_bytes();
        if let Some(byte) = tampered.get_mut(4) {
            *byte = if *byte == b'A' { b'B' } else { b'A' };
        }
        let tampered = String::from_utf8(tampered).unwrap_or_default();
        assert!(matches!(
            validator.validate(&tampered),
            Err(JwtValidationError::InvalidSignature | JwtValidationError::InvalidHeader)
        ));

        let (token, public_key) = signed_token(
            r#"{"alg":"EdDSA"}"#,
            r#"{"iss":"https://other.example","aud":"https://kvm.example/mcp","exp":200,"scope":"kvm:read"}"#,
        );
        let validator = Ed25519JwtValidator::new("https://issuer.example", &public_key);
        assert_eq!(
            validator.validate(&token),
            Err(JwtValidationError::WrongIssuer)
        );
    }

    struct RecordingVideo {
        last_request: Cell<Option<SnapshotRequest>>,
    }

    impl VideoTransport for RecordingVideo {
        fn capture_snapshot(&self, request: SnapshotRequest) -> io::Result<VideoSnapshot> {
            self.last_request.set(Some(request));
            let byte_length = match request.format {
                SnapshotFormat::Yuv420p => 3_110_400,
                SnapshotFormat::Yuv422p => 4_147_200,
                SnapshotFormat::Jpeg | SnapshotFormat::Rgb565 => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "unsupported snapshot format",
                    ));
                }
            };
            VideoSnapshot::new(
                request.width,
                request.height,
                request.format,
                byte_length,
                [0x5a; 32],
            )
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid snapshot"))
        }
    }

    #[test]
    fn authorized_video_snapshot_returns_bounded_metadata_and_keeps_audio_disabled() {
        let scopes = [Scope::Read, Scope::Video];
        let grant = Grant {
            audience: "https://kvm.example/mcp",
            expires_at_unix_seconds: 200,
            scopes: &scopes,
            mutation_enabled: false,
        };
        let video = RecordingVideo {
            last_request: Cell::new(None),
        };
        let mut server = Server::remote("https://kvm.example/mcp", 100, grant).with_video(&video);
        initialize(&mut server);
        let tools = response(
            &mut server,
            r#"{"jsonrpc":"2.0","id":20,"method":"tools/list"}"#,
        );
        assert_eq!(tools["result"]["tools"].as_array().map(Vec::len), Some(2));

        let status = response(
            &mut server,
            r#"{"jsonrpc":"2.0","id":21,"method":"tools/call","params":{"name":"kvm.status","arguments":{}}}"#,
        );
        assert_eq!(status["result"]["structuredContent"]["videoEnabled"], true);
        assert_eq!(
            status["result"]["structuredContent"]["audioCaptureEnabled"],
            false
        );
        assert_eq!(
            status["result"]["structuredContent"]["audioOutputEnabled"],
            false
        );

        let snap = response(
            &mut server,
            r#"{"jsonrpc":"2.0","id":22,"method":"tools/call","params":{"name":"kvm.video.snapshot","arguments":{"width":1920,"height":1080,"format":"yuv422p","deadlineMs":2000}}}"#,
        );
        assert_eq!(snap["result"]["structuredContent"]["width"], 1920);
        assert_eq!(snap["result"]["structuredContent"]["height"], 1080);
        assert_eq!(snap["result"]["structuredContent"]["format"], "yuv422p");
        assert_eq!(snap["result"]["structuredContent"]["byteLength"], 4_147_200);
        assert_eq!(
            snap["result"]["structuredContent"]["sha256"],
            "5a".repeat(32)
        );
        let recorded = video.last_request.get();
        assert_eq!(
            recorded.map(|req| req.format),
            Some(SnapshotFormat::Yuv422p)
        );

        let default_snapshot = response(
            &mut server,
            r#"{"jsonrpc":"2.0","id":24,"method":"tools/call","params":{"name":"kvm.video.snapshot"}}"#,
        );
        assert_eq!(
            default_snapshot["result"]["structuredContent"]["format"],
            "yuv420p"
        );
        assert_eq!(
            default_snapshot["result"]["structuredContent"]["byteLength"],
            3_110_400
        );
        assert_eq!(
            video.last_request.get().map(|request| request.format),
            Some(SnapshotFormat::Yuv420p)
        );

        let invalid = response(
            &mut server,
            r#"{"jsonrpc":"2.0","id":23,"method":"tools/call","params":{"name":"kvm.video.snapshot","arguments":{"width":3840}}}"#,
        );
        assert_eq!(invalid["error"]["code"], -32602);
    }

    struct RecordingAudio;

    impl AudioCaptureTransport for RecordingAudio {
        fn capture_metrics(&self, shape: PcmShape) -> io::Result<CaptureMetrics> {
            assert_eq!(shape.sample_rate_hz, 16_000);
            Ok(CaptureMetrics {
                frames: 16_000,
                bytes: 32_000,
                peak_absolute: 512,
                mean_absolute: 128,
                rms: 180,
                clipped_samples: 0,
                zero_samples: 1,
                elapsed_ms: 1_010,
            })
        }
    }

    impl AudioOutputTransport for RecordingAudio {
        fn play_bounded_tone(&self, shape: PcmShape) -> io::Result<PlaybackReceipt> {
            assert_eq!(shape.duration_ms, 1_000);
            Ok(PlaybackReceipt {
                frames: 16_000,
                bytes: 32_000,
                tone_hz: 1_000,
                peak: 512,
                elapsed_ms: 1_005,
            })
        }
    }

    #[test]
    fn independently_scoped_audio_tools_return_bounded_results() {
        let audio = RecordingAudio;
        let capture_scopes = [Scope::Read, Scope::AudioCapture];
        let capture_grant = Grant {
            audience: "https://kvm.example/mcp",
            expires_at_unix_seconds: 200,
            scopes: &capture_scopes,
            mutation_enabled: false,
        };
        let mut capture = Server::remote_ready("https://kvm.example/mcp", 100, capture_grant)
            .with_audio_capture(&audio)
            .with_audio_output(&audio);
        let status = response(
            &mut capture,
            r#"{"jsonrpc":"2.0","id":30,"method":"tools/call","params":{"name":"kvm.status","arguments":{}}}"#,
        );
        assert_eq!(
            status["result"]["structuredContent"]["audioCaptureEnabled"],
            true
        );
        assert_eq!(
            status["result"]["structuredContent"]["audioOutputEnabled"],
            false
        );
        let result = response(
            &mut capture,
            r#"{"jsonrpc":"2.0","id":31,"method":"tools/call","params":{"name":"kvm.audio.capture","arguments":{}}}"#,
        );
        assert_eq!(result["result"]["structuredContent"]["byteLength"], 32_000);
        assert_eq!(result["result"]["structuredContent"]["rms"], 180);

        let output_scopes = [Scope::Read, Scope::AudioOutput];
        let output_grant = Grant {
            audience: "https://kvm.example/mcp",
            expires_at_unix_seconds: 200,
            scopes: &output_scopes,
            mutation_enabled: true,
        };
        let mut output = Server::remote_ready("https://kvm.example/mcp", 100, output_grant)
            .with_audio_output(&audio);
        let result = response(
            &mut output,
            r#"{"jsonrpc":"2.0","id":32,"method":"tools/call","params":{"name":"kvm.audio.play","arguments":{}}}"#,
        );
        assert_eq!(result["result"]["structuredContent"]["toneHz"], 1_000);
        assert_eq!(result["result"]["structuredContent"]["completed"], true);
    }
}
