//! Bounded MCP stdio protocol adapter.

use std::io::{self, BufRead, Write};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
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

/// Mint an Ed25519-signed JWT bearer token using a 32-byte private signing key seed.
#[must_use]
pub fn mint_token(
    signing_key_bytes: &[u8; 32],
    issuer: &str,
    audience: &str,
    scope: &str,
    mutation: bool,
    expires_at_unix_seconds: u64,
) -> String {
    let signing_key = SigningKey::from_bytes(signing_key_bytes);
    let header_json = r#"{"alg":"EdDSA","typ":"at+jwt"}"#;
    let claims_json = json!({
        "iss": issuer,
        "aud": audience,
        "exp": expires_at_unix_seconds,
        "scope": scope,
        "onyx_kvm_mutation": mutation,
    })
    .to_string();

    let header_b64 = URL_SAFE_NO_PAD.encode(header_json.as_bytes());
    let claims_b64 = URL_SAFE_NO_PAD.encode(claims_json.as_bytes());
    let signing_input = format!("{header_b64}.{claims_b64}");
    let signature = signing_key.sign(signing_input.as_bytes());
    let signature_b64 = URL_SAFE_NO_PAD.encode(signature.to_bytes());
    format!("{signing_input}.{signature_b64}")
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
    #[serde(default)]
    onyx_mutation: Option<bool>,
    #[serde(default)]
    onyx_kvm_mutation: Option<bool>,
    #[serde(default)]
    cleankvm_mutation: Option<bool>,
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
        let mutation_enabled = claims
            .onyx_kvm_mutation
            .or(claims.cleankvm_mutation)
            .or(claims.onyx_mutation)
            .unwrap_or(false);
        Ok(ValidatedClaims {
            audience: claims.aud.to_owned(),
            expires_at_unix_seconds: claims.exp,
            scopes,
            mutation_enabled,
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
        if matches!(name, "kvm.device.reboot" | "kvm.system.reboot") {
            return self.call_reboot_tool(id, name, params.get("arguments"));
        }
        if name == "kvm.diag.video" {
            return self.call_diag_video(id);
        }
        if name == "kvm.diag.dmesg" {
            return self.call_diag_dmesg(id, params.get("arguments"));
        }
        if name == "kvm.diag.system" {
            return self.call_diag_system(id);
        }
        if name == "kvm.dev.exec" {
            return self.call_dev_exec(id, params.get("arguments"));
        }
        if name == "kvm.system.dev_mode" {
            return self.call_dev_mode(id, params.get("arguments"));
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
        let request = match parse_snapshot_arguments(arguments) {
            Ok(req) => req,
            Err(err) => return error(id, -32602, err),
        };
        // If hardware LT6911 reports no carrier lock (target sleeping / no HDMI signal),
        // return early without timing out inside the hardware driver.
        let (hdmi_locked, sig_output, _, _, _) = query_lt6911_i2c();
        if !hdmi_locked && sig_output.is_some() {
            let sig_str = sig_output.as_deref().unwrap_or("unknown");
            return success(
                id,
                &json!({
                    "content": [{
                        "type": "text",
                        "text": format!("Target device is sleeping or HDMI carrier is not locked (signal: {sig_str}). No video frame available while device is sleeping.")
                    }],
                    "structuredContent": {
                        "connected": false,
                        "standby": true,
                        "signalA3": sig_str,
                        "message": "Target device is in standby or HDMI signal is not present. Video stream and snapshot will resume when target displays video."
                    },
                    "isError": false
                }),
            );
        }
        let Ok(snapshot) = video.capture_snapshot(request) else {
            return success(
                id,
                &json!({
                    "content": [{
                        "type": "text",
                        "text": "Video snapshot failed: No active HDMI video input signal detected or video broker capture timed out (verify HDMI cable is connected and transmitting video)"
                    }],
                    "structuredContent": {
                        "error": "No active HDMI video input signal detected or video broker capture timed out",
                        "connected": false
                    },
                    "isError": true
                }),
            );
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

    fn call_reboot_tool(&self, id: &Value, name: &str, arguments: Option<&Value>) -> String {
        let arguments_are_empty =
            arguments.is_none_or(|value| value.as_object().is_some_and(serde_json::Map::is_empty));
        if !arguments_are_empty {
            return error(id, -32602, "Reboot tool accepts no arguments");
        }
        if self.authorize(name).is_err() {
            return error(id, -32603, "Policy denied device reboot");
        }
        let _ = std::fs::write("/run/onyx-kvm/reboot-trigger", b"reboot\n");
        let _ = std::fs::write("/run/cleankvm/reboot-trigger", b"reboot\n");
        let _ = std::process::Command::new("/sbin/reboot").spawn();

        let result = json!({
            "status": "rebooting",
            "message": "Device reboot initiated"
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

    fn call_diag_video(&self, id: &Value) -> String {
        if self.authorize("kvm.diag.video").is_err() {
            return error(id, -32603, "Policy denied diagnostic video status");
        }
        let mipi_rx = std::fs::read_to_string("/proc/mipi-rx")
            .unwrap_or_else(|e| format!("unavailable: {e}"));
        let vi_dbg = std::fs::read_to_string("/proc/cvitek/vi_dbg")
            .unwrap_or_else(|e| format!("unavailable: {e}"));
        let (hdmi_locked, signal_a3, csi_w, csi_h, i2c_note) = query_lt6911_i2c();

        let result = json!({
            "hdmiCarrierLocked": hdmi_locked,
            "signalA3": signal_a3,
            "csiWidth": csi_w,
            "csiHeight": csi_h,
            "i2cStatus": i2c_note,
            "mipiRx": mipi_rx,
            "viDbg": vi_dbg
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

    fn call_diag_dmesg(&self, id: &Value, arguments: Option<&Value>) -> String {
        if self.authorize("kvm.diag.dmesg").is_err() {
            return error(id, -32603, "Policy denied diagnostic dmesg");
        }
        let lines_limit = arguments
            .and_then(|args| args.get("lines"))
            .and_then(Value::as_u64)
            .unwrap_or(60)
            .clamp(1, 200) as usize;

        let output = std::process::Command::new("dmesg")
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_else(|e| format!("dmesg execution failed: {e}"));

        let all_lines: Vec<&str> = output.lines().collect();
        let total = all_lines.len();
        let start = total.saturating_sub(lines_limit);
        let selected = all_lines[start..].join("\n");

        let result = json!({
            "totalLines": total,
            "returnedLines": all_lines[start..].len(),
            "messages": selected
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

    fn call_diag_system(&self, id: &Value) -> String {
        if self.authorize("kvm.diag.system").is_err() {
            return error(id, -32603, "Policy denied diagnostic system status");
        }
        let uptime = std::fs::read_to_string("/proc/uptime").unwrap_or_default();
        let meminfo = std::fs::read_to_string("/proc/meminfo").unwrap_or_default();
        let loadavg = std::fs::read_to_string("/proc/loadavg").unwrap_or_default();

        let usb_state = (|| -> Option<String> {
            for entry in std::fs::read_dir("/sys/class/udc").ok()? {
                let entry = entry.ok()?;
                let state_path = entry.path().join("state");
                if let Ok(content) = std::fs::read_to_string(state_path) {
                    return Some(format!(
                        "{}: {}",
                        entry.file_name().to_string_lossy(),
                        content.trim()
                    ));
                }
            }
            None
        })()
        .unwrap_or_else(|| "unknown".into());

        let dev_mode_active = std::path::Path::new("/run/onyx-kvm/dev-mode").exists()
            || std::path::Path::new("/run/cleankvm/dev-mode").exists();
        let dev_mode_boot = std::path::Path::new("/boot/DEV_MODE").exists()
            || std::path::Path::new("/boot/dev_mode").exists();

        let ps_output = std::process::Command::new("ps")
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_default();

        let result = json!({
            "uptime": uptime.trim(),
            "loadavg": loadavg.trim(),
            "usbUdcState": usb_state,
            "devModeActive": dev_mode_active,
            "devModeBootConfig": dev_mode_boot,
            "meminfo": meminfo.lines().take(5).collect::<Vec<_>>().join("; "),
            "processes": ps_output.lines().take(30).collect::<Vec<_>>().join("\n")
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

    fn call_dev_exec(&self, id: &Value, arguments: Option<&Value>) -> String {
        if self.authorize("kvm.dev.exec").is_err() {
            return error(id, -32603, "Policy denied dev execution");
        }
        let dev_mode_active = std::path::Path::new("/run/onyx-kvm/dev-mode").exists()
            || std::path::Path::new("/boot/DEV_MODE").exists()
            || std::path::Path::new("/boot/dev_mode").exists();
        if !dev_mode_active {
            return success(
                id,
                &json!({
                    "isError": true,
                    "content": [{ "type": "text", "text": "DEV_MODE is disabled. Create /boot/DEV_MODE or call kvm.system.dev_mode to enable." }],
                    "structuredContent": { "error": "DEV_MODE is disabled" }
                }),
            );
        }
        let Some(cmd) = arguments
            .and_then(|args| args.get("command"))
            .and_then(Value::as_str)
        else {
            return error(id, -32602, "Missing required 'command' argument");
        };

        let req_path = "/run/onyx-kvm/dev-exec-request";
        let res_path = "/run/onyx-kvm/dev-exec-response";
        let (exit_code, output) = if std::path::Path::new("/run/onyx-kvm").exists() {
            let _ = std::fs::remove_file(res_path);
            if std::fs::write(req_path, cmd).is_ok() {
                let start = std::time::Instant::now();
                let mut response = None;
                while start.elapsed() < std::time::Duration::from_secs(10) {
                    if let Ok(content) = std::fs::read_to_string(res_path) {
                        response = Some(content);
                        let _ = std::fs::remove_file(res_path);
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                if let Some(resp) = response {
                    let mut lines = resp.lines();
                    let rc = lines.next().and_then(|l| l.parse::<i32>().ok()).unwrap_or(-1);
                    let out = lines.collect::<Vec<_>>().join("\n");
                    (rc, out)
                } else {
                    run_command_fallback(cmd)
                }
            } else {
                run_command_fallback(cmd)
            }
        } else {
            run_command_fallback(cmd)
        };

        let result = json!({
            "command": cmd,
            "exitCode": exit_code,
            "output": output
        });

        success(
            id,
            &json!({
                "content": [{ "type": "text", "text": output }],
                "structuredContent": result,
                "isError": exit_code != 0
            }),
        )
    }

    fn call_dev_mode(&self, id: &Value, arguments: Option<&Value>) -> String {
        if self.authorize("kvm.system.dev_mode").is_err() {
            return error(id, -32603, "Policy denied dev_mode control");
        }
        let action = arguments
            .and_then(|args| args.get("action"))
            .and_then(Value::as_str)
            .unwrap_or("status");

        match action {
            "status" => {
                let boot_flag = std::path::Path::new("/boot/DEV_MODE").exists()
                    || std::path::Path::new("/boot/dev_mode").exists();
                let run_flag = std::path::Path::new("/run/onyx-kvm/dev-mode").exists()
                    || std::path::Path::new("/run/cleankvm/dev-mode").exists();
                let ssh_keys = std::fs::read_to_string("/boot/ssh_authorized_keys").ok();
                let result = json!({
                    "devModeBootConfig": boot_flag,
                    "devModeRuntimeActive": run_flag,
                    "authorizedKeysPresent": ssh_keys.is_some()
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
            "enable" => {
                let _ = std::process::Command::new("mount").arg("/boot").status();
                let _ = std::fs::write("/boot/DEV_MODE", b"");
                if let Some(keys) = arguments
                    .and_then(|a| a.get("authorizedKeys"))
                    .and_then(Value::as_str)
                {
                    let _ = std::fs::write("/boot/ssh_authorized_keys", keys);
                }
                let _ = std::fs::create_dir_all("/run/onyx-kvm");
                let _ = std::fs::write("/run/onyx-kvm/dev-mode", b"");
                let _ = std::process::Command::new("sync").status();
                if std::path::Path::new("/etc/init.d/S35onyx-kvm-devmode").exists() {
                    let _ = std::process::Command::new("/etc/init.d/S35onyx-kvm-devmode")
                        .arg("start")
                        .status();
                }
                let result = json!({
                    "status": "enabled",
                    "devModeBootConfig": true,
                    "devModeRuntimeActive": true,
                    "message": "DEV_MODE enabled. SSH and dev tools are active."
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
            "disable" => {
                let _ = std::fs::remove_file("/boot/DEV_MODE");
                let _ = std::fs::remove_file("/boot/dev_mode");
                let _ = std::fs::remove_file("/run/onyx-kvm/dev-mode");
                let _ = std::fs::remove_file("/run/cleankvm/dev-mode");
                let _ = std::process::Command::new("sync").status();
                if std::path::Path::new("/etc/init.d/S35onyx-kvm-devmode").exists() {
                    let _ = std::process::Command::new("/etc/init.d/S35onyx-kvm-devmode")
                        .arg("stop")
                        .status();
                }
                let result = json!({
                    "status": "disabled",
                    "devModeBootConfig": false,
                    "devModeRuntimeActive": false,
                    "message": "DEV_MODE disabled. SSH stopped and dev tools disabled."
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
            _ => error(
                id,
                -32602,
                "Invalid action: must be 'status', 'enable', or 'disable'",
            ),
        }
    }

    fn call_hid_tool(&self, id: &Value, name: &str, arguments: Option<&Value>) -> String {
        let Some(hid) = self.hid else {
            return error(id, -32602, "Unknown or disabled tool");
        };
        if self.authorize(name).is_err() {
            return error(id, -32603, "Policy denied HID report");
        }
        match name {
            "kvm.keyboard.send" => {
                let reports = match parse_keyboard_action(arguments) {
                    Ok(r) => r,
                    Err(err) => return error(id, -32602, err),
                };
                for rep in reports {
                    if hid.send_keyboard(rep).is_err() {
                        return success(
                            id,
                            &json!({
                                "content": [{
                                    "type": "text",
                                    "text": "HID keyboard report failed: USB HID device not connected or not enumerated by target computer (verify USB-C OTG cable is connected to target host)"
                                }],
                                "structuredContent": {
                                    "error": "USB HID device not connected or not enumerated by target computer",
                                    "connected": false
                                },
                                "isError": true
                            }),
                        );
                    }
                }
            }
            "kvm.pointer.send" => {
                let reports = match parse_pointer_action(arguments) {
                    Ok(r) => r,
                    Err(err) => return error(id, -32602, err),
                };
                for rep in reports {
                    if hid.send_pointer(rep).is_err() {
                        return success(
                            id,
                            &json!({
                                "content": [{
                                    "type": "text",
                                    "text": "HID pointer report failed: USB HID device not connected or not enumerated by target computer (verify USB-C OTG cable is connected to target host)"
                                }],
                                "structuredContent": {
                                    "error": "USB HID device not connected or not enumerated by target computer",
                                    "connected": false
                                },
                                "isError": true
                            }),
                        );
                    }
                }
            }
            _ => return error(id, -32602, "Unknown or disabled tool"),
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
        if self.authorize("kvm.device.reboot").is_ok() {
            tools.push(reboot_tool("kvm.device.reboot", "Reboot Onyx KVM device"));
            tools.push(reboot_tool("kvm.system.reboot", "Reboot Onyx KVM system"));
        }
        if self.authorize("kvm.diag.video").is_ok() {
            tools.push(diag_video_tool());
        }
        if self.authorize("kvm.diag.dmesg").is_ok() {
            tools.push(diag_dmesg_tool());
        }
        if self.authorize("kvm.diag.system").is_ok() {
            tools.push(diag_system_tool());
        }
        if self.authorize("kvm.dev.exec").is_ok() {
            tools.push(dev_exec_tool());
        }
        if self.authorize("kvm.system.dev_mode").is_ok() {
            tools.push(dev_mode_tool());
        }
        json!({ "tools": tools })
    }
}

fn diag_video_tool() -> Value {
    json!({
        "name": "kvm.diag.video",
        "title": "Hardware Video Pipeline Diagnostics",
        "description": "Query hardware video pipeline state: LT6911 I2C lock & CSI dimensions, MIPI-RX driver stats, and CVITEK VI debug buffer",
        "inputSchema": {
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "type": "object",
            "additionalProperties": false
        },
        "outputSchema": {
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "type": "object",
            "properties": {
                "hdmiCarrierLocked": { "type": "boolean" },
                "signalA3": { "type": ["string", "null"] },
                "csiWidth": { "type": ["integer", "null"] },
                "csiHeight": { "type": ["integer", "null"] },
                "i2cStatus": { "type": "string" },
                "mipiRx": { "type": "string" },
                "viDbg": { "type": "string" }
            },
            "required": ["hdmiCarrierLocked", "i2cStatus", "mipiRx", "viDbg"],
            "additionalProperties": false
        },
        "annotations": {
            "readOnlyHint": true,
            "destructiveHint": false,
            "idempotentHint": true,
            "openWorldHint": false
        },
        "execution": { "taskSupport": "forbidden" }
    })
}

fn diag_dmesg_tool() -> Value {
    json!({
        "name": "kvm.diag.dmesg",
        "title": "Kernel Ring Buffer (dmesg)",
        "description": "Fetch the most recent kernel ring buffer messages (dmesg tail)",
        "inputSchema": {
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "type": "object",
            "properties": {
                "lines": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 200,
                    "default": 60,
                    "description": "Number of trailing dmesg lines to return (1-200, default 60)"
                }
            },
            "additionalProperties": false
        },
        "outputSchema": {
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "type": "object",
            "properties": {
                "totalLines": { "type": "integer" },
                "returnedLines": { "type": "integer" },
                "messages": { "type": "string" }
            },
            "required": ["totalLines", "returnedLines", "messages"],
            "additionalProperties": false
        },
        "annotations": {
            "readOnlyHint": true,
            "destructiveHint": false,
            "idempotentHint": true,
            "openWorldHint": false
        },
        "execution": { "taskSupport": "forbidden" }
    })
}

fn diag_system_tool() -> Value {
    json!({
        "name": "kvm.diag.system",
        "title": "System Hardware & Runtime Diagnostics",
        "description": "Query system diagnostics: uptime, memory, CPU load, USB UDC endpoint state, DEV_MODE status, and process table",
        "inputSchema": {
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "type": "object",
            "additionalProperties": false
        },
        "outputSchema": {
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "type": "object",
            "properties": {
                "uptime": { "type": "string" },
                "loadavg": { "type": "string" },
                "usbUdcState": { "type": "string" },
                "devModeActive": { "type": "boolean" },
                "devModeBootConfig": { "type": "boolean" },
                "meminfo": { "type": "string" },
                "processes": { "type": "string" }
            },
            "required": ["uptime", "loadavg", "usbUdcState", "devModeActive", "devModeBootConfig", "meminfo", "processes"],
            "additionalProperties": false
        },
        "annotations": {
            "readOnlyHint": true,
            "destructiveHint": false,
            "idempotentHint": true,
            "openWorldHint": false
        },
        "execution": { "taskSupport": "forbidden" }
    })
}

fn dev_exec_tool() -> Value {
    json!({
        "name": "kvm.dev.exec",
        "title": "Execute Diagnostic Command in DEV_MODE",
        "description": "Execute a diagnostic command in DEV_MODE (only available when /boot/DEV_MODE is present and Admin scope + mutation is granted)",
        "inputSchema": {
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "Shell command to run"
                }
            },
            "required": ["command"],
            "additionalProperties": false
        },
        "outputSchema": {
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "type": "object",
            "properties": {
                "command": { "type": "string" },
                "exitCode": { "type": "integer" },
                "output": { "type": "string" }
            },
            "required": ["command", "exitCode", "output"],
            "additionalProperties": false
        },
        "annotations": {
            "readOnlyHint": false,
            "destructiveHint": true,
            "idempotentHint": false,
            "openWorldHint": false
        },
        "execution": { "taskSupport": "forbidden" }
    })
}

fn dev_mode_tool() -> Value {
    json!({
        "name": "kvm.system.dev_mode",
        "title": "DEV_MODE Runtime & Boot Control",
        "description": "Control DEV_MODE (status, enable, disable) and configure SSH authorized keys on /boot",
        "inputSchema": {
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["status", "enable", "disable"],
                    "default": "status",
                    "description": "Dev mode action: 'status' to inspect, 'enable' to enable and optionally install keys, 'disable' to turn off"
                },
                "authorizedKeys": {
                    "type": "string",
                    "description": "Optional OpenSSH public key line(s) to store in /boot/ssh_authorized_keys when enabling"
                }
            },
            "additionalProperties": false
        },
        "outputSchema": {
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "type": "object"
        },
        "annotations": {
            "readOnlyHint": false,
            "destructiveHint": false,
            "idempotentHint": false,
            "openWorldHint": false
        },
        "execution": { "taskSupport": "forbidden" }
    })
}

fn run_command_fallback(cmd: &str) -> (i32, String) {
    match std::process::Command::new("/bin/sh").arg("-c").arg(cmd).output() {
        Ok(output) => {
            let rc = output.status.code().unwrap_or(-1);
            let mut out = String::from_utf8_lossy(&output.stdout).to_string();
            let err = String::from_utf8_lossy(&output.stderr);
            if !err.is_empty() {
                if !out.is_empty() {
                    out.push('\n');
                }
                out.push_str(&err);
            }
            (rc, out)
        }
        Err(e) => (-1, format!("Execution failed: {e}")),
    }
}

fn query_lt6911_i2c() -> (bool, Option<String>, Option<u16>, Option<u16>, String) {
    if !std::path::Path::new("/dev/i2c-4").exists() {
        return (false, None, None, None, "/dev/i2c-4 unavailable".into());
    }

    // 1. Halt LT6911 MCU: bank 0x80, reg 0xee = 0x01
    let halt_status = std::process::Command::new("i2cset")
        .args(["-y", "4", "0x2b", "0xff", "0x80"])
        .status();
    if halt_status.is_err() || !halt_status.unwrap().success() {
        return (false, None, None, None, "i2cset bank 0x80 failed".into());
    }
    let _ = std::process::Command::new("i2cset")
        .args(["-y", "4", "0x2b", "0xee", "0x01"])
        .status();

    // 2. Query signal state from bank 0x86, reg 0xa3
    let _ = std::process::Command::new("i2cset")
        .args(["-y", "4", "0x2b", "0xff", "0x86"])
        .status();
    let sig_output = std::process::Command::new("i2cget")
        .args(["-y", "4", "0x2b", "0xa3"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());

    // 3. Query CSI geometry from bank 0x85: HACTIVE at 0xea, VACTIVE at 0xf0
    let _ = std::process::Command::new("i2cset")
        .args(["-y", "4", "0x2b", "0xff", "0x85"])
        .status();

    let parse_i2ctransfer_u16 = |reg: &str| -> Option<u16> {
        let out = std::process::Command::new("i2ctransfer")
            .args(["-y", "4", "w1@0x2b", reg, "r2"])
            .output()
            .ok()?;
        let text = String::from_utf8_lossy(&out.stdout);
        let bytes: Vec<u8> = text
            .split_whitespace()
            .filter_map(|s| {
                let clean = s.strip_prefix("0x").unwrap_or(s);
                u8::from_str_radix(clean, 16).ok()
            })
            .collect();
        if bytes.len() >= 2 {
            Some(((bytes[0] as u16) << 8) | (bytes[1] as u16))
        } else {
            None
        }
    };

    let csi_w = parse_i2ctransfer_u16("0xea");
    let csi_h = parse_i2ctransfer_u16("0xf0");

    // 4. Resume LT6911 MCU unconditionally: bank 0x80, reg 0xee = 0x00
    let _ = std::process::Command::new("i2cset")
        .args(["-y", "4", "0x2b", "0xff", "0x80"])
        .status();
    let _ = std::process::Command::new("i2cset")
        .args(["-y", "4", "0x2b", "0xee", "0x00"])
        .status();

    let hdmi_locked = sig_output
        .as_deref()
        .map(|s| s.eq_ignore_ascii_case("0x55") || s == "85")
        .unwrap_or(false);

    (hdmi_locked, sig_output, csi_w, csi_h, "ok".into())
}

fn reboot_tool(name: &str, title: &str) -> Value {
    json!({
        "name": name,
        "title": title,
        "description": "Trigger an authenticated, graceful system reboot of the Onyx KVM device",
        "inputSchema": {
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "type": "object",
            "additionalProperties": false
        },
        "outputSchema": {
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "type": "object",
            "properties": {
                "status": { "type": "string" },
                "message": { "type": "string" }
            },
            "required": ["status", "message"],
            "additionalProperties": false
        },
        "annotations": {
            "readOnlyHint": false,
            "destructiveHint": true,
            "idempotentHint": false,
            "openWorldHint": false
        },
        "execution": { "taskSupport": "forbidden" }
    })
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

fn parse_snapshot_arguments(arguments: Option<&Value>) -> Result<SnapshotRequest, &'static str> {
    let Some(arguments) = arguments else {
        return SnapshotRequest::new(1920, 1080, SnapshotFormat::Yuv422p, 5_000)
            .map_err(|_| "Invalid default snapshot arguments");
    };
    let Some(object) = arguments.as_object() else {
        return Err("Snapshot arguments must be a JSON object");
    };
    for key in object.keys() {
        if !matches!(key.as_str(), "width" | "height" | "format" | "deadlineMs") {
            return Err("Unknown snapshot argument key");
        }
    }
    let width = match object.get("width") {
        Some(value) => {
            let Some(val) = value.as_u64() else {
                return Err("width must be an integer");
            };
            let Ok(w) = u16::try_from(val) else {
                return Err("width must be between 1 and 1920");
            };
            if w == 0 || w > 1920 {
                return Err("width must be between 1 and 1920");
            }
            w
        }
        None => 1920,
    };
    let height = match object.get("height") {
        Some(value) => {
            let Some(val) = value.as_u64() else {
                return Err("height must be an integer");
            };
            let Ok(h) = u16::try_from(val) else {
                return Err("height must be between 1 and 1080");
            };
            if h == 0 || h > 1080 {
                return Err("height must be between 1 and 1080");
            }
            h
        }
        None => 1080,
    };
    let format = match object.get("format") {
        Some(value) => {
            let Some(s) = value.as_str() else {
                return Err("format must be a string");
            };
            match s {
                "jpeg" => SnapshotFormat::Jpeg,
                "rgb565" => SnapshotFormat::Rgb565,
                "yuv422p" => SnapshotFormat::Yuv422p,
                "yuv420p" => SnapshotFormat::Yuv420p,
                "tile-delta-yuv422" | "tile_delta" => SnapshotFormat::TileDeltaYuv422,
                "h264" => SnapshotFormat::H264,
                _ => return Err("format must be 'yuv422p', 'yuv420p', or 'h264'"),
            }
        }
        None => SnapshotFormat::Yuv422p,
    };
    let deadline_ms = match object.get("deadlineMs") {
        Some(value) => {
            let Some(val) = value.as_u64() else {
                return Err("deadlineMs must be an integer");
            };
            u16::try_from(val.clamp(100, 5000)).unwrap_or(5000)
        }
        None => 5_000,
    };
    SnapshotRequest::new(width, height, format, deadline_ms)
        .map_err(|_| "Invalid snapshot dimensions or deadline")
}

fn char_to_hid(c: char) -> Option<(u8, u8)> {
    match c {
        'a'..='z' => Some((0, 4 + (c as u8 - b'a'))),
        'A'..='Z' => Some((0x02, 4 + (c as u8 - b'A'))),
        '1'..='9' => Some((0, 30 + (c as u8 - b'1'))),
        '0' => Some((0, 39)),
        '!' => Some((0x02, 30)),
        '@' => Some((0x02, 31)),
        '#' => Some((0x02, 32)),
        '$' => Some((0x02, 33)),
        '%' => Some((0x02, 34)),
        '^' => Some((0x02, 35)),
        '&' => Some((0x02, 36)),
        '*' => Some((0x02, 37)),
        '(' => Some((0x02, 38)),
        ')' => Some((0x02, 39)),
        '\n' | '\r' => Some((0, 40)),
        '\t' => Some((0, 43)),
        ' ' => Some((0, 44)),
        '-' => Some((0, 45)),
        '_' => Some((0x02, 45)),
        '=' => Some((0, 46)),
        '+' => Some((0x02, 46)),
        '[' => Some((0, 47)),
        '{' => Some((0x02, 47)),
        ']' => Some((0, 48)),
        '}' => Some((0x02, 48)),
        '\\' => Some((0, 49)),
        '|' => Some((0x02, 49)),
        ';' => Some((0, 51)),
        ':' => Some((0x02, 51)),
        '\'' => Some((0, 52)),
        '"' => Some((0x02, 52)),
        '`' => Some((0, 53)),
        '~' => Some((0x02, 53)),
        ',' => Some((0, 54)),
        '<' => Some((0x02, 54)),
        '.' => Some((0, 55)),
        '>' => Some((0x02, 55)),
        '/' => Some((0, 56)),
        '?' => Some((0x02, 56)),
        _ => None,
    }
}

fn key_name_to_hid(name: &str) -> Option<(u8, u8)> {
    let lower = name.trim().to_ascii_lowercase();
    match lower.as_str() {
        "enter" | "return" => Some((0, 40)),
        "esc" | "escape" => Some((0, 41)),
        "backspace" => Some((0, 42)),
        "tab" => Some((0, 43)),
        "space" | "spacebar" => Some((0, 44)),
        "capslock" => Some((0, 57)),
        "f1" => Some((0, 58)),
        "f2" => Some((0, 59)),
        "f3" => Some((0, 60)),
        "f4" => Some((0, 61)),
        "f5" => Some((0, 62)),
        "f6" => Some((0, 63)),
        "f7" => Some((0, 64)),
        "f8" => Some((0, 65)),
        "f9" => Some((0, 66)),
        "f10" => Some((0, 67)),
        "f11" => Some((0, 68)),
        "f12" => Some((0, 69)),
        "printscreen" => Some((0, 70)),
        "scrolllock" => Some((0, 71)),
        "pause" => Some((0, 72)),
        "insert" => Some((0, 73)),
        "home" => Some((0, 74)),
        "pageup" => Some((0, 75)),
        "delete" => Some((0, 76)),
        "end" => Some((0, 77)),
        "pagedown" => Some((0, 78)),
        "right" | "arrowright" => Some((0, 79)),
        "left" | "arrowleft" => Some((0, 80)),
        "down" | "arrowdown" => Some((0, 81)),
        "up" | "arrowup" => Some((0, 82)),
        _ => {
            if lower.chars().count() == 1 {
                char_to_hid(lower.chars().next()?)
            } else {
                None
            }
        }
    }
}

fn parse_modifiers(value: Option<&Value>) -> u8 {
    let mut mask = 0u8;
    let Some(val) = value else { return 0; };
    if let Some(arr) = val.as_array() {
        for item in arr {
            if let Some(s) = item.as_str() {
                match s.to_ascii_lowercase().as_str() {
                    "ctrl" | "leftctrl" => mask |= 0x01,
                    "shift" | "leftshift" => mask |= 0x02,
                    "alt" | "leftalt" => mask |= 0x04,
                    "gui" | "meta" | "win" | "cmd" | "leftgui" => mask |= 0x08,
                    "rctrl" | "rightctrl" => mask |= 0x10,
                    "rshift" | "rightshift" => mask |= 0x20,
                    "ralt" | "rightalt" => mask |= 0x40,
                    "rgui" | "rightgui" => mask |= 0x80,
                    _ => {}
                }
            }
        }
    } else if let Some(n) = val.as_u64() {
        mask = (n & 0xFF) as u8;
    }
    mask
}

fn parse_keyboard_action(arguments: Option<&Value>) -> Result<Vec<[u8; 8]>, &'static str> {
    let Some(arguments) = arguments else {
        return Err("Keyboard arguments must be provided");
    };
    let Some(object) = arguments.as_object() else {
        return Err("Keyboard arguments must be a JSON object");
    };
    if let Some(report) = object.get("report") {
        let Some(arr) = report.as_array() else {
            return Err("HID report must be one byte array");
        };
        let Some(bytes) = arr
            .iter()
            .map(|value| value.as_u64().and_then(|byte| u8::try_from(byte).ok()))
            .collect::<Option<Vec<_>>>()
        else {
            return Err("HID report contains an invalid byte");
        };
        let Ok(report) = <[u8; 8]>::try_from(bytes.as_slice()) else {
            return Err("Keyboard report must contain 8 bytes");
        };
        return Ok(vec![report]);
    }
    if let Some(key_val) = object.get("key") {
        let Some(key_str) = key_val.as_str() else {
            return Err("key must be a string");
        };
        let Some((char_mod, code)) = key_name_to_hid(key_str) else {
            return Err("Unknown key name");
        };
        let mod_mask = parse_modifiers(object.get("modifiers")) | char_mod;
        return Ok(vec![
            [mod_mask, 0, code, 0, 0, 0, 0, 0],
            [0, 0, 0, 0, 0, 0, 0, 0],
        ]);
    }
    if let Some(text_val) = object.get("text") {
        let Some(text_str) = text_val.as_str() else {
            return Err("text must be a string");
        };
        if text_str.len() > 128 {
            return Err("text cannot exceed 128 characters");
        }
        let mut reports = Vec::new();
        for ch in text_str.chars() {
            let Some((char_mod, code)) = char_to_hid(ch) else {
                return Err("Unsupported character in text");
            };
            reports.push([char_mod, 0, code, 0, 0, 0, 0, 0]);
            reports.push([0, 0, 0, 0, 0, 0, 0, 0]);
        }
        return Ok(reports);
    }
    Err("Keyboard arguments must specify 'key', 'text', or 'report'")
}

fn parse_pointer_action(arguments: Option<&Value>) -> Result<Vec<[u8; 4]>, &'static str> {
    let Some(arguments) = arguments else {
        return Err("Pointer arguments must be provided");
    };
    let Some(object) = arguments.as_object() else {
        return Err("Pointer arguments must be a JSON object");
    };
    if let Some(report) = object.get("report") {
        let Some(arr) = report.as_array() else {
            return Err("HID report must be one byte array");
        };
        let Some(bytes) = arr
            .iter()
            .map(|value| {
                if let Some(byte) = value.as_u64() {
                    u8::try_from(byte).ok()
                } else if let Some(signed) = value.as_i64() {
                    i8::try_from(signed).ok().map(|s| s as u8)
                } else {
                    None
                }
            })
            .collect::<Option<Vec<_>>>()
        else {
            return Err("HID report contains an invalid byte");
        };
        let Ok(report) = <[u8; 4]>::try_from(bytes.as_slice()) else {
            return Err("Pointer report must contain 4 bytes");
        };
        return Ok(vec![report]);
    }
    if let Some(click_val) = object.get("click") {
        let Some(btn_str) = click_val.as_str() else {
            return Err("click must be a string");
        };
        let button = match btn_str.to_ascii_lowercase().as_str() {
            "left" => 1,
            "right" => 2,
            "middle" => 4,
            _ => return Err("click must be 'left', 'right', or 'middle'"),
        };
        return Ok(vec![
            [button, 0, 0, 0],
            [0, 0, 0, 0],
        ]);
    }
    if object.contains_key("dx")
        || object.contains_key("dy")
        || object.contains_key("wheel")
        || object.contains_key("buttons")
    {
        let buttons = object.get("buttons").and_then(Value::as_u64).unwrap_or(0) as u8;
        let dx = object.get("dx").and_then(Value::as_i64).unwrap_or(0).clamp(-127, 127) as i8 as u8;
        let dy = object.get("dy").and_then(Value::as_i64).unwrap_or(0).clamp(-127, 127) as i8 as u8;
        let wheel = object.get("wheel").and_then(Value::as_i64).unwrap_or(0).clamp(-127, 127) as i8 as u8;
        return Ok(vec![[buttons, dx, dy, wheel]]);
    }
    Err("Pointer arguments must specify 'click', 'dx'/'dy', or 'report'")
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
                "width": { "type": "integer", "minimum": 1, "maximum": 1920, "default": 1920, "description": "Horizontal resolution (1 to 1920, default 1920)" },
                "height": { "type": "integer", "minimum": 1, "maximum": 1080, "default": 1080, "description": "Vertical resolution (1 to 1080, default 1080)" },
                "format": { "type": "string", "enum": ["yuv420p", "yuv422p", "h264"], "default": "yuv422p", "description": "Video frame pixel format: 'yuv422p' (lossless UYVY raw), 'yuv420p', or 'h264'" },
                "deadlineMs": { "type": "integer", "minimum": 100, "maximum": 5000, "default": 5000, "description": "Snapshot timeout in milliseconds between 100 and 5000 (default 5000)" }
            },
            "additionalProperties": false
        },
        "outputSchema": {
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "type": "object",
            "properties": {
                "width": { "type": "integer", "minimum": 1, "maximum": 1920 },
                "height": { "type": "integer", "minimum": 1, "maximum": 1080 },
                "format": { "type": "string", "enum": ["yuv420p", "yuv422p", "h264"] },
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

fn hid_tool(name: &str, title: &str, _length: usize) -> Value {
    if name == "kvm.keyboard.send" {
        json!({
            "name": name,
            "title": title,
            "description": "Send keyboard keystrokes or raw reports to the target computer over USB HID. Provide either 'key' (named key, e.g. 'Return', 'Escape', 'Tab', 'Space', 'Backspace', 'ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight', or single char 'a'-'z'), 'text' (a string to type sequentially), or 'report' (raw 8-byte array).",
            "inputSchema": {
                "$schema": "https://json-schema.org/draft/2020-12/schema",
                "type": "object",
                "properties": {
                    "key": {
                        "type": "string",
                        "description": "Named key or character to press and release, e.g. 'Return', 'Enter', 'Escape', 'Tab', 'Space', 'Backspace', 'ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight', 'Delete', 'Home', 'End', 'PageUp', 'PageDown', 'F1'-'F12', or single character 'a'-'z', '0'-'9'."
                    },
                    "modifiers": {
                        "type": "array",
                        "items": { "type": "string", "enum": ["ctrl", "shift", "alt", "gui"] },
                        "description": "Optional list of modifier keys to hold during the keypress, e.g. ['ctrl', 'alt']"
                    },
                    "text": {
                        "type": "string",
                        "description": "Plain ASCII text string to type sequentially on the target machine (max 128 characters)"
                    },
                    "report": {
                        "type": "array",
                        "items": { "type": "integer", "minimum": 0, "maximum": 255 },
                        "minItems": 8, "maxItems": 8,
                        "description": "Exact 8-byte array representing the raw USB HID boot keyboard report: [modifiers, 0, key1, key2, key3, key4, key5, key6]"
                    }
                },
                "additionalProperties": false
            },
            "annotations": {
                "readOnlyHint": false, "destructiveHint": false,
                "idempotentHint": false, "openWorldHint": false
            },
            "execution": { "taskSupport": "forbidden" }
        })
    } else {
        json!({
            "name": name,
            "title": title,
            "description": "Send mouse pointer movements, clicks, or raw reports to the target computer over USB HID. Provide 'dx'/'dy' for relative movement, 'click' ('left', 'right', 'middle'), 'wheel' for scrolling, or 'report' (raw 4-byte array).",
            "inputSchema": {
                "$schema": "https://json-schema.org/draft/2020-12/schema",
                "type": "object",
                "properties": {
                    "click": {
                        "type": "string",
                        "enum": ["left", "right", "middle"],
                        "description": "Mouse button to click (press and release)"
                    },
                    "dx": {
                        "type": "integer",
                        "minimum": -127, "maximum": 127,
                        "description": "Relative horizontal movement in pixels (-127 to 127, positive = right, negative = left)"
                    },
                    "dy": {
                        "type": "integer",
                        "minimum": -127, "maximum": 127,
                        "description": "Relative vertical movement in pixels (-127 to 127, positive = down, negative = up)"
                    },
                    "wheel": {
                        "type": "integer",
                        "minimum": -127, "maximum": 127,
                        "description": "Relative vertical scroll wheel movement (positive = scroll up, negative = scroll down)"
                    },
                    "buttons": {
                        "type": "integer",
                        "minimum": 0, "maximum": 7,
                        "description": "Button bitmask: bit 0 = Left (1), bit 1 = Right (2), bit 2 = Middle (4)"
                    },
                    "report": {
                        "type": "array",
                        "items": { "type": "integer", "minimum": 0, "maximum": 255 },
                        "minItems": 4, "maxItems": 4,
                        "description": "Exact 4-byte array representing the raw USB HID relative mouse report: [buttons, dx, dy, wheel]"
                    }
                },
                "additionalProperties": false
            },
            "annotations": {
                "readOnlyHint": false, "destructiveHint": false,
                "idempotentHint": false, "openWorldHint": false
            },
            "execution": { "taskSupport": "forbidden" }
        })
    }
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

        let key_named = response(
            &mut server,
            r#"{"jsonrpc":"2.0","id":16,"method":"tools/call","params":{"name":"kvm.keyboard.send","arguments":{"key":"Return"}}}"#,
        );
        assert_eq!(key_named["result"]["structuredContent"]["accepted"], true);
        assert_eq!(hid.keyboard.get(), Some([0, 0, 0, 0, 0, 0, 0, 0]));

        let pointer_move = response(
            &mut server,
            r#"{"jsonrpc":"2.0","id":17,"method":"tools/call","params":{"name":"kvm.pointer.send","arguments":{"dx":10,"dy":5}}}"#,
        );
        assert_eq!(pointer_move["result"]["structuredContent"]["accepted"], true);
        assert_eq!(hid.pointer.get(), Some([0, 10, 5, 0]));

        let pointer_click = response(
            &mut server,
            r#"{"jsonrpc":"2.0","id":18,"method":"tools/call","params":{"name":"kvm.pointer.send","arguments":{"click":"left"}}}"#,
        );
        assert_eq!(pointer_click["result"]["structuredContent"]["accepted"], true);
        assert_eq!(hid.pointer.get(), Some([0, 0, 0, 0]));

        let pointer_signed = response(
            &mut server,
            r#"{"jsonrpc":"2.0","id":19,"method":"tools/call","params":{"name":"kvm.pointer.send","arguments":{"report":[0,-1,0,0]}}}"#,
        );
        assert_eq!(pointer_signed["result"]["structuredContent"]["accepted"], true);
        assert_eq!(hid.pointer.get(), Some([0, 255, 0, 0]));

        let malformed = response(
            &mut server,
            r#"{"jsonrpc":"2.0","id":13,"method":"tools/call","params":{"name":"kvm.pointer.send","arguments":{"report":[0,0,0,256]}}}"#,
        );
        assert_eq!(malformed["error"]["code"], -32602);
        assert_eq!(hid.pointer.get(), Some([0, 255, 0, 0]));

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
    fn mint_token_produces_verifiable_jwt() {
        let seed = [77u8; 32];
        let signing_key = SigningKey::from_bytes(&seed);
        let public_key = signing_key.verifying_key().to_bytes();
        let token = super::mint_token(
            &seed,
            "https://test-issuer",
            "https://test-kvm/mcp",
            "kvm:read kvm:video kvm:control",
            true,
            1234567890,
        );
        let validator = Ed25519JwtValidator::new("https://test-issuer", &public_key);
        let claims = validator.validate(&token).expect("valid mint_token JWT");
        assert_eq!(claims.audience, "https://test-kvm/mcp");
        assert_eq!(claims.expires_at_unix_seconds, 1234567890);
        assert_eq!(claims.scopes, vec![Scope::Read, Scope::Video, Scope::Control]);
        assert!(claims.mutation_enabled);
    }

    #[test]
    fn validate_actual_v59_token() {
        if let (Ok(token), Ok(pub_key)) = (
            std::fs::read_to_string("../../out/product-v59-secrets/operator-token.jwt"),
            std::fs::read("../../out/product-v59-secrets/identity/oauth-issuer-public-key.raw"),
        ) {
            let issuer = "https://onyx-kvm-owner,https://cleankvm-owner";
            let validator = Ed25519JwtValidator::new(issuer, &pub_key);
            let Ok(claims) = validator.validate(token.trim()) else {
                panic!("token validation failed");
            };
            assert_eq!(claims.audience, "https://onyx-kvm-recovery:8443/mcp");
            assert!(claims.mutation_enabled);
        }
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
                SnapshotFormat::H264 => 100_000,
                SnapshotFormat::Yuv420p => 3_110_400,
                SnapshotFormat::Yuv422p | SnapshotFormat::TileDeltaYuv422 => 4_147_200,
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
            "yuv422p"
        );
        assert_eq!(
            default_snapshot["result"]["structuredContent"]["byteLength"],
            4_147_200
        );
        assert_eq!(
            video.last_request.get().map(|request| request.format),
            Some(SnapshotFormat::Yuv422p)
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

    #[test]
    fn reboot_tools_require_admin_scope_and_mutation() {
        let read_scopes = [Scope::Read];
        let read_grant = Grant {
            audience: "https://kvm.example/mcp",
            expires_at_unix_seconds: 200,
            scopes: &read_scopes,
            mutation_enabled: false,
        };
        let mut server = Server::remote_ready("https://kvm.example/mcp", 100, read_grant);
        let list = response(&mut server, r#"{"jsonrpc":"2.0","id":40,"method":"tools/list"}"#);
        let tool_names: Vec<&str> = list["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        assert!(!tool_names.contains(&"kvm.device.reboot"));
        assert!(!tool_names.contains(&"kvm.system.reboot"));

        let denied = response(
            &mut server,
            r#"{"jsonrpc":"2.0","id":41,"method":"tools/call","params":{"name":"kvm.device.reboot","arguments":{}}}"#,
        );
        assert_eq!(denied["error"]["code"], -32603);

        let admin_scopes = [Scope::Read, Scope::Admin];
        let admin_no_mut_grant = Grant {
            audience: "https://kvm.example/mcp",
            expires_at_unix_seconds: 200,
            scopes: &admin_scopes,
            mutation_enabled: false,
        };
        let mut server = Server::remote_ready("https://kvm.example/mcp", 100, admin_no_mut_grant);
        let denied = response(
            &mut server,
            r#"{"jsonrpc":"2.0","id":42,"method":"tools/call","params":{"name":"kvm.device.reboot","arguments":{}}}"#,
        );
        assert_eq!(denied["error"]["code"], -32603);

        let admin_mut_grant = Grant {
            audience: "https://kvm.example/mcp",
            expires_at_unix_seconds: 200,
            scopes: &admin_scopes,
            mutation_enabled: true,
        };
        let mut server = Server::remote_ready("https://kvm.example/mcp", 100, admin_mut_grant);
        let list = response(&mut server, r#"{"jsonrpc":"2.0","id":43,"method":"tools/list"}"#);
        let tool_names: Vec<&str> = list["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        assert!(tool_names.contains(&"kvm.device.reboot"));
        assert!(tool_names.contains(&"kvm.system.reboot"));

        let res = response(
            &mut server,
            r#"{"jsonrpc":"2.0","id":44,"method":"tools/call","params":{"name":"kvm.device.reboot","arguments":{}}}"#,
        );
        assert_eq!(res["result"]["structuredContent"]["status"], "rebooting");

        let res = response(
            &mut server,
            r#"{"jsonrpc":"2.0","id":45,"method":"tools/call","params":{"name":"kvm.system.reboot","arguments":{}}}"#,
        );
        assert_eq!(res["result"]["structuredContent"]["status"], "rebooting");
    }

    #[test]
    fn diagnostic_tools_require_admin_scope() {
        let read_scopes = [Scope::Read];
        let read_grant = Grant {
            audience: "https://kvm.example/mcp",
            expires_at_unix_seconds: 200,
            scopes: &read_scopes,
            mutation_enabled: false,
        };
        let mut server = Server::remote_ready("https://kvm.example/mcp", 100, read_grant);
        let list = response(&mut server, r#"{"jsonrpc":"2.0","id":50,"method":"tools/list"}"#);
        let tool_names: Vec<&str> = list["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        assert!(!tool_names.contains(&"kvm.diag.video"));
        assert!(!tool_names.contains(&"kvm.diag.dmesg"));
        assert!(!tool_names.contains(&"kvm.diag.system"));

        let denied_video = response(
            &mut server,
            r#"{"jsonrpc":"2.0","id":51,"method":"tools/call","params":{"name":"kvm.diag.video"}}"#,
        );
        assert_eq!(denied_video["error"]["code"], -32603);

        let admin_scopes = [Scope::Read, Scope::Admin];
        let admin_grant = Grant {
            audience: "https://kvm.example/mcp",
            expires_at_unix_seconds: 200,
            scopes: &admin_scopes,
            mutation_enabled: false,
        };
        let mut server = Server::remote_ready("https://kvm.example/mcp", 100, admin_grant);
        let list = response(&mut server, r#"{"jsonrpc":"2.0","id":52,"method":"tools/list"}"#);
        let tool_names: Vec<&str> = list["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        assert!(tool_names.contains(&"kvm.diag.video"));
        assert!(tool_names.contains(&"kvm.diag.dmesg"));
        assert!(tool_names.contains(&"kvm.diag.system"));

        let diag_vid = response(
            &mut server,
            r#"{"jsonrpc":"2.0","id":53,"method":"tools/call","params":{"name":"kvm.diag.video"}}"#,
        );
        assert!(diag_vid["result"]["structuredContent"]["hdmiCarrierLocked"].is_boolean());
        assert!(diag_vid["result"]["structuredContent"]["i2cStatus"].is_string());

        let diag_dmsg = response(
            &mut server,
            r#"{"jsonrpc":"2.0","id":54,"method":"tools/call","params":{"name":"kvm.diag.dmesg","arguments":{"lines":10}}}"#,
        );
        assert!(diag_dmsg["result"]["structuredContent"]["totalLines"].is_number());
        assert!(diag_dmsg["result"]["structuredContent"]["returnedLines"].is_number());

        let diag_sys = response(
            &mut server,
            r#"{"jsonrpc":"2.0","id":55,"method":"tools/call","params":{"name":"kvm.diag.system"}}"#,
        );
        assert!(diag_sys["result"]["structuredContent"]["uptime"].is_string());
        assert!(diag_sys["result"]["structuredContent"]["usbUdcState"].is_string());
    }

    #[test]
    fn dev_exec_and_dev_mode_require_admin_scope_and_mutation() {
        let admin_scopes = [Scope::Read, Scope::Admin];
        let admin_no_mut = Grant {
            audience: "https://kvm.example/mcp",
            expires_at_unix_seconds: 200,
            scopes: &admin_scopes,
            mutation_enabled: false,
        };
        let mut server = Server::remote_ready("https://kvm.example/mcp", 100, admin_no_mut);
        let list = response(&mut server, r#"{"jsonrpc":"2.0","id":60,"method":"tools/list"}"#);
        let tool_names: Vec<&str> = list["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        assert!(!tool_names.contains(&"kvm.dev.exec"));
        assert!(!tool_names.contains(&"kvm.system.dev_mode"));

        let denied_exec = response(
            &mut server,
            r#"{"jsonrpc":"2.0","id":61,"method":"tools/call","params":{"name":"kvm.dev.exec","arguments":{"command":"id"}}}"#,
        );
        assert_eq!(denied_exec["error"]["code"], -32603);

        let admin_mut = Grant {
            audience: "https://kvm.example/mcp",
            expires_at_unix_seconds: 200,
            scopes: &admin_scopes,
            mutation_enabled: true,
        };
        let mut server = Server::remote_ready("https://kvm.example/mcp", 100, admin_mut);
        let list = response(&mut server, r#"{"jsonrpc":"2.0","id":62,"method":"tools/list"}"#);
        let tool_names: Vec<&str> = list["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        assert!(tool_names.contains(&"kvm.dev.exec"));
        assert!(tool_names.contains(&"kvm.system.dev_mode"));

        let dev_status = response(
            &mut server,
            r#"{"jsonrpc":"2.0","id":63,"method":"tools/call","params":{"name":"kvm.system.dev_mode","arguments":{"action":"status"}}}"#,
        );
        assert!(dev_status["result"]["structuredContent"]["devModeBootConfig"].is_boolean());

        // dev.exec when DEV_MODE is disabled reports failure closed
        let dev_exec_disabled = response(
            &mut server,
            r#"{"jsonrpc":"2.0","id":64,"method":"tools/call","params":{"name":"kvm.dev.exec","arguments":{"command":"echo hello"}}}"#,
        );
        assert_eq!(dev_exec_disabled["result"]["isError"], true);
    }
}

