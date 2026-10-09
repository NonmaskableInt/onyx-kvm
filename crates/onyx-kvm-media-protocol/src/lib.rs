//! Fixed limits and validated request types shared by Phase 3 media brokers.
//!
//! This crate intentionally performs no device I/O and contains no vendor FFI.
//! Network-facing code must construct these types before contacting a broker;
//! each broker must validate its wire request again before touching hardware.

use std::collections::VecDeque;

pub const WIRE_MAGIC: [u8; 4] = *b"CKM3";
pub const WIRE_VERSION: u8 = 1;
pub const WIRE_HEADER_BYTES: usize = 12;
pub const MAX_WIRE_PAYLOAD_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_WIDTH: u16 = 1920;
pub const MAX_HEIGHT: u16 = 1080;
pub const MAX_SNAPSHOT_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_PCM_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_AUDIO_DURATION_MS: u32 = 10_000;
pub const MAX_PLAYBACK_VOLUME_PERCENT: u8 = 70;
pub const MAX_READ_ONLY_MEDIA_BYTES: u64 = 8 * 1024 * 1024 * 1024;

pub const STREAM_MAGIC: [u8; 4] = *b"OXFR";
pub const STREAM_MAGIC_LEGACY: [u8; 4] = *b"CKFR";
pub const STREAM_FRAME_HEADER_BYTES: usize = 64;
pub const MAX_STREAM_LEASE_MS: u32 = 60_000;
pub const DEFAULT_STREAM_LEASE_MS: u32 = 10_000;
pub const MAX_RING_BUFFER_FRAMES: usize = 4;

pub const STREAM_FLAG_KEYFRAME: u8 = 0x01;
pub const STREAM_FLAG_DELTA: u8 = 0x02;

pub const TILE_WIDTH: u16 = 64;
pub const TILE_HEIGHT: u16 = 40;
pub const TILE_COLS: u16 = 30; // 1920 / 64
pub const TILE_ROWS: u16 = 27; // 1080 / 40
pub const TOTAL_TILES: u16 = 810; // 30 * 27
pub const TILE_BYTES: usize = (TILE_WIDTH as usize) * (TILE_HEIGHT as usize) * 2; // 5120
pub const TILE_RECORD_BYTES: usize = 2 + TILE_BYTES; // 5122
pub const TILE_DELTA_HEADER_BYTES: usize = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Operation {
    Snapshot = 1,
    AttachReadOnlyMedia = 2,
    DetachMedia = 3,
    SnapshotFrame = 4,
    StreamFrame = 5,
    CapturePcm = 16,
    PlayPcm = 17,
    StopAudio = 18,
}

impl TryFrom<u8> for Operation {
    type Error = ValidationError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Snapshot),
            2 => Ok(Self::AttachReadOnlyMedia),
            3 => Ok(Self::DetachMedia),
            4 => Ok(Self::SnapshotFrame),
            5 => Ok(Self::StreamFrame),
            16 => Ok(Self::CapturePcm),
            17 => Ok(Self::PlayPcm),
            18 => Ok(Self::StopAudio),
            _ => Err(ValidationError::UnknownOperation),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValidationError {
    BadMagic,
    UnsupportedVersion,
    UnknownOperation,
    NonZeroFlags,
    InvalidLength,
    InvalidDimensions,
    InvalidDeadline,
    InvalidAudioParameters,
    AudioTooLarge,
    VolumeTooHigh,
    WritableMediaForbidden,
    MediaTooLarge,
    VideoDisabled,
    PolicyRejected,
}

pub const SNAPSHOT_REQUEST_BYTES: usize = 8;
pub const SNAPSHOT_RESPONSE_BYTES: usize = 42;
pub const AUDIO_REQUEST_BYTES: usize = 8;
pub const CAPTURE_METRICS_BYTES: usize = 32;
pub const PLAYBACK_RECEIPT_BYTES: usize = 20;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WireHeader {
    pub operation: Operation,
    pub payload_bytes: u32,
}

impl WireHeader {
    /// Encode a fixed-size wire header with zero flags.
    #[must_use]
    pub const fn encode(self) -> [u8; WIRE_HEADER_BYTES] {
        let len = self.payload_bytes.to_be_bytes();
        [
            WIRE_MAGIC[0],
            WIRE_MAGIC[1],
            WIRE_MAGIC[2],
            WIRE_MAGIC[3],
            WIRE_VERSION,
            self.operation as u8,
            0,
            0,
            len[0],
            len[1],
            len[2],
            len[3],
        ]
    }

    /// Parse a complete fixed-size header and reject extensible flags.
    ///
    /// # Errors
    /// Returns a validation error for malformed or unsupported fields.
    pub fn decode(bytes: [u8; WIRE_HEADER_BYTES]) -> Result<Self, ValidationError> {
        if bytes[..4] != WIRE_MAGIC {
            return Err(ValidationError::BadMagic);
        }
        if bytes[4] != WIRE_VERSION {
            return Err(ValidationError::UnsupportedVersion);
        }
        let operation = Operation::try_from(bytes[5])?;
        if bytes[6..8] != [0, 0] {
            return Err(ValidationError::NonZeroFlags);
        }
        let payload_bytes = u32::from_be_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
        if usize::try_from(payload_bytes).map_or(true, |length| length > MAX_WIRE_PAYLOAD_BYTES) {
            return Err(ValidationError::InvalidLength);
        }
        Ok(Self {
            operation,
            payload_bytes,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SnapshotFormat {
    Jpeg = 1,
    Rgb565 = 2,
    Yuv422p = 3,
    Yuv420p = 4,
    TileDeltaYuv422 = 5,
}

impl SnapshotFormat {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Jpeg => "jpeg",
            Self::Rgb565 => "rgb565",
            Self::Yuv422p => "yuv422p",
            Self::Yuv420p => "yuv420p",
            Self::TileDeltaYuv422 => "tile-delta-yuv422",
        }
    }
}

impl TryFrom<u8> for SnapshotFormat {
    type Error = ValidationError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Jpeg),
            2 => Ok(Self::Rgb565),
            3 => Ok(Self::Yuv422p),
            4 => Ok(Self::Yuv420p),
            5 => Ok(Self::TileDeltaYuv422),
            _ => Err(ValidationError::InvalidDimensions),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SnapshotRequest {
    pub width: u16,
    pub height: u16,
    pub format: SnapshotFormat,
    pub deadline_ms: u16,
}

impl SnapshotRequest {
    /// Validate one bounded snapshot request.
    ///
    /// # Errors
    /// Rejects zero/oversized dimensions and deadlines outside 100..=5000 ms.
    pub const fn new(
        width: u16,
        height: u16,
        format: SnapshotFormat,
        deadline_ms: u16,
    ) -> Result<Self, ValidationError> {
        if width == 0 || height == 0 || width > MAX_WIDTH || height > MAX_HEIGHT {
            return Err(ValidationError::InvalidDimensions);
        }
        if deadline_ms < 100 || deadline_ms > 5_000 {
            return Err(ValidationError::InvalidDeadline);
        }
        Ok(Self {
            width,
            height,
            format,
            deadline_ms,
        })
    }

    /// Encode this snapshot request into a fixed 8-byte payload.
    #[must_use]
    pub const fn encode(self) -> [u8; SNAPSHOT_REQUEST_BYTES] {
        let w = self.width.to_be_bytes();
        let h = self.height.to_be_bytes();
        let d = self.deadline_ms.to_be_bytes();
        [w[0], w[1], h[0], h[1], self.format as u8, 0, d[0], d[1]]
    }

    /// Decode and validate a fixed 8-byte snapshot request payload.
    ///
    /// # Errors
    /// Rejects non-zero reserved bytes, unknown formats, or out-of-range fields.
    pub fn decode(bytes: [u8; SNAPSHOT_REQUEST_BYTES]) -> Result<Self, ValidationError> {
        if bytes[5] != 0 {
            return Err(ValidationError::NonZeroFlags);
        }
        let width = u16::from_be_bytes([bytes[0], bytes[1]]);
        let height = u16::from_be_bytes([bytes[2], bytes[3]]);
        let format = SnapshotFormat::try_from(bytes[4])?;
        let deadline_ms = u16::from_be_bytes([bytes[6], bytes[7]]);
        Self::new(width, height, format, deadline_ms)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VideoSnapshot {
    pub width: u16,
    pub height: u16,
    pub format: SnapshotFormat,
    pub byte_length: u32,
    pub sha256: [u8; 32],
}

impl VideoSnapshot {
    /// Validate and construct a bounded snapshot metadata record.
    ///
    /// # Errors
    /// Rejects zero/oversized dimensions or byte lengths exceeding [`MAX_SNAPSHOT_BYTES`].
    pub fn new(
        width: u16,
        height: u16,
        format: SnapshotFormat,
        byte_length: u32,
        sha256: [u8; 32],
    ) -> Result<Self, ValidationError> {
        if width == 0 || height == 0 || width > MAX_WIDTH || height > MAX_HEIGHT {
            return Err(ValidationError::InvalidDimensions);
        }
        let len = usize::try_from(byte_length).map_err(|_| ValidationError::InvalidLength)?;
        if len == 0 || len > MAX_SNAPSHOT_BYTES {
            return Err(ValidationError::InvalidLength);
        }
        Ok(Self {
            width,
            height,
            format,
            byte_length,
            sha256,
        })
    }

    #[must_use]
    pub const fn encode(self) -> [u8; SNAPSHOT_RESPONSE_BYTES] {
        let mut out = [0_u8; SNAPSHOT_RESPONSE_BYTES];
        let w = self.width.to_be_bytes();
        let h = self.height.to_be_bytes();
        let l = self.byte_length.to_be_bytes();
        out[0] = w[0];
        out[1] = w[1];
        out[2] = h[0];
        out[3] = h[1];
        out[4] = self.format as u8;
        out[5] = 0;
        out[6] = l[0];
        out[7] = l[1];
        out[8] = l[2];
        out[9] = l[3];
        let mut idx = 0;
        while idx < 32 {
            out[10 + idx] = self.sha256[idx];
            idx += 1;
        }
        out
    }

    /// Decode and validate a fixed 42-byte snapshot response record.
    ///
    /// # Errors
    /// Rejects non-zero flags, invalid dimensions, or invalid byte lengths.
    pub fn decode(bytes: [u8; SNAPSHOT_RESPONSE_BYTES]) -> Result<Self, ValidationError> {
        if bytes[5] != 0 {
            return Err(ValidationError::NonZeroFlags);
        }
        let width = u16::from_be_bytes([bytes[0], bytes[1]]);
        let height = u16::from_be_bytes([bytes[2], bytes[3]]);
        let format = SnapshotFormat::try_from(bytes[4])?;
        let byte_length = u32::from_be_bytes([bytes[6], bytes[7], bytes[8], bytes[9]]);
        let mut sha256 = [0_u8; 32];
        sha256.copy_from_slice(&bytes[10..42]);
        Self::new(width, height, format, byte_length, sha256)
    }

    #[must_use]
    pub fn sha256_hex(&self) -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut text = String::with_capacity(64);
        for byte in self.sha256 {
            text.push(char::from(HEX[usize::from(byte >> 4)]));
            text.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
        text
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StreamFrameHeader {
    pub sequence: u32,
    pub timestamp_ms: u64,
    pub width: u16,
    pub height: u16,
    pub format: SnapshotFormat,
    pub flags: u8,
    pub payload_bytes: u32,
    pub sha256: [u8; 32],
}

impl StreamFrameHeader {
    /// Create and validate a stream frame header.
    ///
    /// # Errors
    /// Rejects zero/oversized dimensions or byte lengths exceeding [`MAX_SNAPSHOT_BYTES`].
    pub fn new(
        sequence: u32,
        timestamp_ms: u64,
        width: u16,
        height: u16,
        format: SnapshotFormat,
        payload_bytes: u32,
        sha256: [u8; 32],
    ) -> Result<Self, ValidationError> {
        Self::with_flags(
            sequence,
            timestamp_ms,
            width,
            height,
            format,
            0,
            payload_bytes,
            sha256,
        )
    }

    /// Create and validate a stream frame header with explicit flags.
    ///
    /// # Errors
    /// Rejects invalid flags, zero/oversized dimensions, or byte lengths exceeding [`MAX_SNAPSHOT_BYTES`].
    pub fn with_flags(
        sequence: u32,
        timestamp_ms: u64,
        width: u16,
        height: u16,
        format: SnapshotFormat,
        flags: u8,
        payload_bytes: u32,
        sha256: [u8; 32],
    ) -> Result<Self, ValidationError> {
        if (flags & !(STREAM_FLAG_KEYFRAME | STREAM_FLAG_DELTA)) != 0 {
            return Err(ValidationError::NonZeroFlags);
        }
        if width == 0 || height == 0 || width > MAX_WIDTH || height > MAX_HEIGHT {
            return Err(ValidationError::InvalidDimensions);
        }
        let len = usize::try_from(payload_bytes).map_err(|_| ValidationError::InvalidLength)?;
        if len > MAX_SNAPSHOT_BYTES {
            return Err(ValidationError::InvalidLength);
        }
        if len == 0 && flags == 0 {
            return Err(ValidationError::InvalidLength);
        }
        Ok(Self {
            sequence,
            timestamp_ms,
            width,
            height,
            format,
            flags,
            payload_bytes,
            sha256,
        })
    }

    /// Encode into fixed 64-byte binary header.
    #[must_use]
    pub const fn encode(self) -> [u8; STREAM_FRAME_HEADER_BYTES] {
        let mut out = [0_u8; STREAM_FRAME_HEADER_BYTES];
        out[0] = STREAM_MAGIC[0];
        out[1] = STREAM_MAGIC[1];
        out[2] = STREAM_MAGIC[2];
        out[3] = STREAM_MAGIC[3];
        let seq = self.sequence.to_be_bytes();
        out[4] = seq[0];
        out[5] = seq[1];
        out[6] = seq[2];
        out[7] = seq[3];
        let ts = self.timestamp_ms.to_be_bytes();
        out[8] = ts[0];
        out[9] = ts[1];
        out[10] = ts[2];
        out[11] = ts[3];
        out[12] = ts[4];
        out[13] = ts[5];
        out[14] = ts[6];
        out[15] = ts[7];
        let w = self.width.to_be_bytes();
        out[16] = w[0];
        out[17] = w[1];
        let h = self.height.to_be_bytes();
        out[18] = h[0];
        out[19] = h[1];
        out[20] = self.format as u8;
        out[21] = self.flags;
        out[22] = 0; // reserved
        out[23] = 0; // reserved
        let len = self.payload_bytes.to_be_bytes();
        out[24] = len[0];
        out[25] = len[1];
        out[26] = len[2];
        out[27] = len[3];
        out[28] = 0; // reserved2
        out[29] = 0;
        out[30] = 0;
        out[31] = 0;
        let mut idx = 0;
        while idx < 32 {
            out[32 + idx] = self.sha256[idx];
            idx += 1;
        }
        out
    }

    /// Decode and validate a fixed 64-byte header.
    ///
    /// # Errors
    /// Rejects bad magic, non-zero flags/reserved bytes, or out-of-range dimensions/payloads.
    pub fn decode(bytes: [u8; STREAM_FRAME_HEADER_BYTES]) -> Result<Self, ValidationError> {
        if bytes[..4] != STREAM_MAGIC && bytes[..4] != STREAM_MAGIC_LEGACY {
            return Err(ValidationError::BadMagic);
        }
        let sequence = u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        let timestamp_ms = u64::from_be_bytes([
            bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15],
        ]);
        let width = u16::from_be_bytes([bytes[16], bytes[17]]);
        let height = u16::from_be_bytes([bytes[18], bytes[19]]);
        let format = SnapshotFormat::try_from(bytes[20])?;
        let flags = bytes[21];
        if (flags & !(STREAM_FLAG_KEYFRAME | STREAM_FLAG_DELTA)) != 0
            || bytes[22] != 0
            || bytes[23] != 0
            || bytes[28..32] != [0, 0, 0, 0]
        {
            return Err(ValidationError::NonZeroFlags);
        }
        let payload_bytes = u32::from_be_bytes([bytes[24], bytes[25], bytes[26], bytes[27]]);
        let mut sha256 = [0_u8; 32];
        sha256.copy_from_slice(&bytes[32..64]);
        Self::with_flags(
            sequence,
            timestamp_ms,
            width,
            height,
            format,
            flags,
            payload_bytes,
            sha256,
        )
    }

    #[must_use]
    pub fn sha256_hex(&self) -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut text = String::with_capacity(64);
        for byte in self.sha256 {
            text.push(char::from(HEX[usize::from(byte >> 4)]));
            text.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
        text
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TileDeltaHeader {
    pub dirty_count: u16,
    pub tile_width: u16,
    pub tile_height: u16,
    pub flags: u16,
}

impl TileDeltaHeader {
    #[must_use]
    pub const fn new(dirty_count: u16) -> Self {
        Self {
            dirty_count,
            tile_width: TILE_WIDTH,
            tile_height: TILE_HEIGHT,
            flags: 0,
        }
    }

    #[must_use]
    pub const fn encode(self) -> [u8; TILE_DELTA_HEADER_BYTES] {
        let dc = self.dirty_count.to_be_bytes();
        let tw = self.tile_width.to_be_bytes();
        let th = self.tile_height.to_be_bytes();
        let fl = self.flags.to_be_bytes();
        [dc[0], dc[1], tw[0], tw[1], th[0], th[1], fl[0], fl[1]]
    }

    pub fn decode(bytes: [u8; TILE_DELTA_HEADER_BYTES]) -> Result<Self, ValidationError> {
        let dirty_count = u16::from_be_bytes([bytes[0], bytes[1]]);
        let tile_width = u16::from_be_bytes([bytes[2], bytes[3]]);
        let tile_height = u16::from_be_bytes([bytes[4], bytes[5]]);
        let flags = u16::from_be_bytes([bytes[6], bytes[7]]);
        if dirty_count > TOTAL_TILES {
            return Err(ValidationError::InvalidLength);
        }
        if tile_width != TILE_WIDTH || tile_height != TILE_HEIGHT {
            return Err(ValidationError::InvalidDimensions);
        }
        if flags != 0 {
            return Err(ValidationError::NonZeroFlags);
        }
        Ok(Self {
            dirty_count,
            tile_width,
            tile_height,
            flags,
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct StreamFrame {
    pub header: StreamFrameHeader,
    pub payload: Vec<u8>,
}

impl StreamFrame {
    /// Create a validated stream frame with owned buffer.
    ///
    /// # Errors
    /// Returns [`ValidationError::InvalidLength`] if payload length does not match header.
    pub fn new(header: StreamFrameHeader, payload: Vec<u8>) -> Result<Self, ValidationError> {
        let expected =
            usize::try_from(header.payload_bytes).map_err(|_| ValidationError::InvalidLength)?;
        if payload.len() != expected {
            return Err(ValidationError::InvalidLength);
        }
        Ok(Self { header, payload })
    }

    pub fn zeroize(&mut self) {
        self.payload.fill(0);
    }
}

impl Drop for StreamFrame {
    fn drop(&mut self) {
        self.zeroize();
    }
}

#[derive(Debug)]
pub struct BoundedFrameRing {
    capacity: usize,
    frames: VecDeque<StreamFrame>,
    dropped_frames_count: u64,
}

impl BoundedFrameRing {
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        let cap = capacity.clamp(1, MAX_RING_BUFFER_FRAMES);
        Self {
            capacity: cap,
            frames: VecDeque::with_capacity(cap),
            dropped_frames_count: 0,
        }
    }

    pub fn push(&mut self, frame: StreamFrame) -> bool {
        let mut dropped = false;
        if self.frames.len() >= self.capacity {
            if let Some(mut old) = self.frames.pop_front() {
                old.payload.fill(0);
                self.dropped_frames_count = self.dropped_frames_count.saturating_add(1);
                dropped = true;
            }
        }
        self.frames.push_back(frame);
        dropped
    }

    pub fn pop(&mut self) -> Option<StreamFrame> {
        self.frames.pop_front()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.frames.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    #[must_use]
    pub fn dropped_count(&self) -> u64 {
        self.dropped_frames_count
    }

    pub fn clear(&mut self) {
        for frame in &mut self.frames {
            frame.payload.fill(0);
        }
        self.frames.clear();
    }
}

impl Drop for BoundedFrameRing {
    fn drop(&mut self) {
        self.clear();
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StreamSessionLease {
    pub session_id: u64,
    pub created_at_unix_ms: u64,
    pub expires_at_unix_ms: u64,
}

impl StreamSessionLease {
    /// Create a bounded streaming session lease.
    ///
    /// # Errors
    /// Returns [`ValidationError::InvalidDeadline`] if duration is 0 or exceeds [`MAX_STREAM_LEASE_MS`].
    pub fn new(
        session_id: u64,
        now_unix_ms: u64,
        duration_ms: u32,
    ) -> Result<Self, ValidationError> {
        if duration_ms == 0 || duration_ms > MAX_STREAM_LEASE_MS {
            return Err(ValidationError::InvalidDeadline);
        }
        let expires_at_unix_ms = now_unix_ms.saturating_add(u64::from(duration_ms));
        Ok(Self {
            session_id,
            created_at_unix_ms: now_unix_ms,
            expires_at_unix_ms,
        })
    }

    #[must_use]
    pub fn is_expired(&self, now_unix_ms: u64) -> bool {
        now_unix_ms >= self.expires_at_unix_ms
    }

    /// Renew the lease duration.
    ///
    /// # Errors
    /// Returns [`ValidationError::PolicyRejected`] if already expired,
    /// or [`ValidationError::InvalidDeadline`] if duration is out of bounds.
    pub fn renew(&mut self, now_unix_ms: u64, duration_ms: u32) -> Result<(), ValidationError> {
        if self.is_expired(now_unix_ms) {
            return Err(ValidationError::PolicyRejected);
        }
        if duration_ms == 0 || duration_ms > MAX_STREAM_LEASE_MS {
            return Err(ValidationError::InvalidDeadline);
        }
        self.expires_at_unix_ms = now_unix_ms.saturating_add(u64::from(duration_ms));
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AudioSource {
    OnboardMicrophone,
    Hdmi,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AudioSink {
    OnboardDac,
    UsbAudioClass,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PcmFormat {
    Signed16LittleEndian,
}

impl PcmFormat {
    const fn wire_value(self) -> u8 {
        match self {
            Self::Signed16LittleEndian => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PcmShape {
    pub format: PcmFormat,
    pub sample_rate_hz: u32,
    pub channels: u8,
    pub duration_ms: u32,
    pub total_bytes: usize,
}

impl PcmShape {
    /// Validate PCM shape and calculate its exact maximum byte count.
    ///
    /// # Errors
    /// Rejects non-enumerated rates/channels, zero or excessive duration, and
    /// any byte calculation beyond the fixed PCM ceiling.
    pub fn new(
        format: PcmFormat,
        sample_rate_hz: u32,
        channels: u8,
        duration_ms: u32,
    ) -> Result<Self, ValidationError> {
        if !matches!(sample_rate_hz, 8_000 | 16_000 | 48_000)
            || !matches!(channels, 1 | 2)
            || duration_ms == 0
            || duration_ms > MAX_AUDIO_DURATION_MS
        {
            return Err(ValidationError::InvalidAudioParameters);
        }
        let samples = u64::from(sample_rate_hz)
            .checked_mul(u64::from(duration_ms))
            .and_then(|value| value.checked_add(999))
            .map(|value| value / 1_000)
            .ok_or(ValidationError::AudioTooLarge)?;
        let bytes = samples
            .checked_mul(u64::from(channels))
            .and_then(|value| value.checked_mul(2))
            .and_then(|value| usize::try_from(value).ok())
            .ok_or(ValidationError::AudioTooLarge)?;
        if bytes > MAX_PCM_BYTES {
            return Err(ValidationError::AudioTooLarge);
        }
        Ok(Self {
            format,
            sample_rate_hz,
            channels,
            duration_ms,
            total_bytes: bytes,
        })
    }

    #[must_use]
    pub const fn encode(self) -> [u8; AUDIO_REQUEST_BYTES] {
        let rate = self.sample_rate_hz.to_be_bytes();
        let duration = self.duration_ms.to_be_bytes();
        [
            rate[0],
            rate[1],
            rate[2],
            rate[3],
            self.channels,
            self.format.wire_value(),
            duration[2],
            duration[3],
        ]
    }

    /// Decode one fixed-size PCM shape.
    ///
    /// # Errors
    /// Rejects unknown formats and shapes outside the global audio bounds.
    pub fn decode(bytes: [u8; AUDIO_REQUEST_BYTES]) -> Result<Self, ValidationError> {
        let sample_rate_hz = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let channels = bytes[4];
        let format = match bytes[5] {
            1 => PcmFormat::Signed16LittleEndian,
            _ => return Err(ValidationError::InvalidAudioParameters),
        };
        let duration_ms = u32::from(u16::from_be_bytes([bytes[6], bytes[7]]));
        Self::new(format, sample_rate_hz, channels, duration_ms)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CaptureMetrics {
    pub frames: u32,
    pub bytes: u32,
    pub peak_absolute: u32,
    pub mean_absolute: u32,
    pub rms: u32,
    pub clipped_samples: u32,
    pub zero_samples: u32,
    pub elapsed_ms: u32,
}

impl CaptureMetrics {
    #[must_use]
    pub fn encode(self) -> [u8; CAPTURE_METRICS_BYTES] {
        let values = [
            self.frames,
            self.bytes,
            self.peak_absolute,
            self.mean_absolute,
            self.rms,
            self.clipped_samples,
            self.zero_samples,
            self.elapsed_ms,
        ];
        let mut output = [0_u8; CAPTURE_METRICS_BYTES];
        for (index, value) in values.into_iter().enumerate() {
            output[index * 4..index * 4 + 4].copy_from_slice(&value.to_be_bytes());
        }
        output
    }

    #[must_use]
    pub fn decode(bytes: [u8; CAPTURE_METRICS_BYTES]) -> Self {
        let value = |index: usize| {
            u32::from_be_bytes([
                bytes[index * 4],
                bytes[index * 4 + 1],
                bytes[index * 4 + 2],
                bytes[index * 4 + 3],
            ])
        };
        Self {
            frames: value(0),
            bytes: value(1),
            peak_absolute: value(2),
            mean_absolute: value(3),
            rms: value(4),
            clipped_samples: value(5),
            zero_samples: value(6),
            elapsed_ms: value(7),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlaybackReceipt {
    pub frames: u32,
    pub bytes: u32,
    pub tone_hz: u32,
    pub peak: u32,
    pub elapsed_ms: u32,
}

impl PlaybackReceipt {
    #[must_use]
    pub fn encode(self) -> [u8; PLAYBACK_RECEIPT_BYTES] {
        let values = [
            self.frames,
            self.bytes,
            self.tone_hz,
            self.peak,
            self.elapsed_ms,
        ];
        let mut output = [0_u8; PLAYBACK_RECEIPT_BYTES];
        for (index, value) in values.into_iter().enumerate() {
            output[index * 4..index * 4 + 4].copy_from_slice(&value.to_be_bytes());
        }
        output
    }

    #[must_use]
    pub fn decode(bytes: [u8; PLAYBACK_RECEIPT_BYTES]) -> Self {
        let value = |index: usize| {
            u32::from_be_bytes([
                bytes[index * 4],
                bytes[index * 4 + 1],
                bytes[index * 4 + 2],
                bytes[index * 4 + 3],
            ])
        };
        Self {
            frames: value(0),
            bytes: value(1),
            tone_hz: value(2),
            peak: value(3),
            elapsed_ms: value(4),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlaybackRequest {
    pub sink: AudioSink,
    pub shape: PcmShape,
    pub volume_percent: u8,
}

impl PlaybackRequest {
    /// Validate a playback request against the hard volume ceiling.
    ///
    /// # Errors
    /// Rejects a volume above [`MAX_PLAYBACK_VOLUME_PERCENT`].
    pub const fn new(
        sink: AudioSink,
        shape: PcmShape,
        volume_percent: u8,
    ) -> Result<Self, ValidationError> {
        if volume_percent > MAX_PLAYBACK_VOLUME_PERCENT {
            return Err(ValidationError::VolumeTooHigh);
        }
        Ok(Self {
            sink,
            shape,
            volume_percent,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MediaImage {
    pub bytes: u64,
}

impl MediaImage {
    /// Admit only non-empty, explicitly read-only media within the fixed cap.
    ///
    /// # Errors
    /// Rejects writable, empty, or oversized images.
    pub const fn new(bytes: u64, read_only: bool) -> Result<Self, ValidationError> {
        if !read_only {
            return Err(ValidationError::WritableMediaForbidden);
        }
        if bytes == 0 || bytes > MAX_READ_ONLY_MEDIA_BYTES {
            return Err(ValidationError::MediaTooLarge);
        }
        Ok(Self { bytes })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_header_is_strict_and_bounded() {
        let valid = [b'C', b'K', b'M', b'3', 1, 1, 0, 0, 0, 0, 0, 16];
        assert_eq!(
            WireHeader::decode(valid),
            Ok(WireHeader {
                operation: Operation::Snapshot,
                payload_bytes: 16,
            })
        );
        let mut flagged = valid;
        flagged[7] = 1;
        assert_eq!(
            WireHeader::decode(flagged),
            Err(ValidationError::NonZeroFlags)
        );
        let mut unknown = valid;
        unknown[5] = 255;
        assert_eq!(
            WireHeader::decode(unknown),
            Err(ValidationError::UnknownOperation)
        );
    }

    #[test]
    fn snapshot_limits_are_closed() {
        assert!(SnapshotRequest::new(1920, 1080, SnapshotFormat::Jpeg, 5_000).is_ok());
        assert_eq!(
            SnapshotRequest::new(1921, 1080, SnapshotFormat::Jpeg, 5_000),
            Err(ValidationError::InvalidDimensions)
        );
        assert_eq!(
            SnapshotRequest::new(640, 480, SnapshotFormat::Jpeg, 5_001),
            Err(ValidationError::InvalidDeadline)
        );
    }

    #[test]
    fn pcm_shape_has_an_exact_ceiling() {
        let shape = PcmShape::new(PcmFormat::Signed16LittleEndian, 48_000, 2, 10_000)
            .unwrap_or_else(|error| panic!("valid maximum PCM shape failed: {error:?}"));
        assert_eq!(shape.total_bytes, 1_920_000);
        assert_eq!(
            PcmShape::new(PcmFormat::Signed16LittleEndian, 44_100, 2, 1_000),
            Err(ValidationError::InvalidAudioParameters)
        );
    }

    #[test]
    fn playback_and_media_fail_closed() {
        let shape = PcmShape::new(PcmFormat::Signed16LittleEndian, 16_000, 1, 1_000)
            .unwrap_or_else(|error| panic!("valid PCM shape failed: {error:?}"));
        assert_eq!(
            PlaybackRequest::new(AudioSink::OnboardDac, shape, 71),
            Err(ValidationError::VolumeTooHigh)
        );
        assert_eq!(
            MediaImage::new(1024, false),
            Err(ValidationError::WritableMediaForbidden)
        );
        assert!(MediaImage::new(MAX_READ_ONLY_MEDIA_BYTES, true).is_ok());
    }

    #[test]
    fn audio_wire_records_roundtrip() {
        let shape = PcmShape::new(PcmFormat::Signed16LittleEndian, 16_000, 1, 1_000)
            .unwrap_or_else(|error| panic!("valid PCM shape failed: {error:?}"));
        assert_eq!(PcmShape::decode(shape.encode()), Ok(shape));
        let metrics = CaptureMetrics {
            frames: 16_000,
            bytes: 32_000,
            peak_absolute: 512,
            mean_absolute: 128,
            rms: 180,
            clipped_samples: 0,
            zero_samples: 1,
            elapsed_ms: 1_012,
        };
        assert_eq!(CaptureMetrics::decode(metrics.encode()), metrics);
        let receipt = PlaybackReceipt {
            frames: 16_000,
            bytes: 32_000,
            tone_hz: 1_000,
            peak: 512,
            elapsed_ms: 1_008,
        };
        assert_eq!(PlaybackReceipt::decode(receipt.encode()), receipt);
    }

    #[test]
    fn snapshot_wire_roundtrip_and_bounds() {
        let request = SnapshotRequest::new(1920, 1080, SnapshotFormat::Yuv420p, 2_000)
            .unwrap_or_else(|error| panic!("valid snapshot request failed: {error:?}"));
        let encoded = request.encode();
        assert_eq!(SnapshotRequest::decode(encoded), Ok(request));

        let header = WireHeader {
            operation: Operation::Snapshot,
            payload_bytes: 8,
        };
        assert_eq!(WireHeader::decode(header.encode()), Ok(header));

        let digest = [0x50_u8; 32];
        let snapshot = VideoSnapshot::new(1920, 1080, SnapshotFormat::Yuv422p, 4_147_200, digest)
            .unwrap_or_else(|error| panic!("valid 1080p YUV422P snapshot failed: {error:?}"));
        assert_eq!(VideoSnapshot::decode(snapshot.encode()), Ok(snapshot));
        assert_eq!(snapshot.sha256_hex().len(), 64);

        let yuv420 = VideoSnapshot::new(1920, 1080, SnapshotFormat::Yuv420p, 3_110_400, digest)
            .unwrap_or_else(|error| panic!("valid 1080p YUV420P snapshot failed: {error:?}"));
        assert_eq!(VideoSnapshot::decode(yuv420.encode()), Ok(yuv420));
    }

    #[test]
    fn stream_frame_header_roundtrip_and_bounds() {
        let digest = [0x7a_u8; 32];
        let header = StreamFrameHeader::new(
            42,
            1_700_000_000_123,
            1920,
            1080,
            SnapshotFormat::Yuv420p,
            3_110_400,
            digest,
        )
        .unwrap_or_else(|error| panic!("valid stream frame header failed: {error:?}"));
        let encoded = header.encode();
        assert_eq!(encoded.len(), STREAM_FRAME_HEADER_BYTES);
        let decoded = StreamFrameHeader::decode(encoded)
            .unwrap_or_else(|error| panic!("decoding stream frame header failed: {error:?}"));
        assert_eq!(decoded, header);
        assert_eq!(decoded.sha256_hex().len(), 64);

        // Rejects out of bounds
        assert_eq!(
            StreamFrameHeader::new(0, 0, 1921, 1080, SnapshotFormat::Yuv420p, 3_110_400, digest),
            Err(ValidationError::InvalidDimensions)
        );
        assert_eq!(
            StreamFrameHeader::new(0, 0, 1920, 1080, SnapshotFormat::Yuv420p, 0, digest),
            Err(ValidationError::InvalidLength)
        );

        // Rejects bad magic
        let mut bad_magic = encoded;
        bad_magic[0] = b'X';
        assert_eq!(
            StreamFrameHeader::decode(bad_magic),
            Err(ValidationError::BadMagic)
        );

        // Rejects invalid flags
        let mut bad_flags = encoded;
        bad_flags[21] = 0x80;
        assert_eq!(
            StreamFrameHeader::decode(bad_flags),
            Err(ValidationError::NonZeroFlags)
        );

        // Accepts valid flags
        let mut delta_flags = encoded;
        delta_flags[21] = STREAM_FLAG_DELTA;
        assert!(StreamFrameHeader::decode(delta_flags).is_ok());
    }

    #[test]
    fn tile_delta_header_roundtrip_and_bounds() {
        let header = TileDeltaHeader::new(42);
        let encoded = header.encode();
        assert_eq!(encoded.len(), TILE_DELTA_HEADER_BYTES);
        let decoded = TileDeltaHeader::decode(encoded).expect("decode failed");
        assert_eq!(decoded, header);
        assert_eq!(decoded.dirty_count, 42);
        assert_eq!(decoded.tile_width, 64);
        assert_eq!(decoded.tile_height, 40);

        // Rejects oversized dirty count
        let mut bad = encoded;
        bad[0] = 0x03;
        bad[1] = 0x50; // 848 > 810
        assert_eq!(TileDeltaHeader::decode(bad), Err(ValidationError::InvalidLength));
    }

    #[test]
    fn stream_frame_drop_zeroizes_payload() {
        let digest = [0x11_u8; 32];
        let header = StreamFrameHeader::new(1, 100, 1920, 1080, SnapshotFormat::Yuv420p, 8, digest)
            .unwrap_or_else(|e| panic!("{e:?}"));
        let buf = vec![0xaa_u8; 8];
        let mut frame = StreamFrame::new(header, buf).unwrap_or_else(|e| panic!("{e:?}"));
        assert_eq!(frame.payload[0], 0xaa);
        frame.zeroize();
        assert_eq!(frame.payload, vec![0_u8; 8]);
    }

    #[test]
    fn bounded_frame_ring_backpressure_eviction() {
        let digest = [0x22_u8; 32];
        let header1 =
            StreamFrameHeader::new(1, 100, 1920, 1080, SnapshotFormat::Yuv420p, 4, digest)
                .unwrap_or_else(|e| panic!("{e:?}"));
        let header2 =
            StreamFrameHeader::new(2, 200, 1920, 1080, SnapshotFormat::Yuv420p, 4, digest)
                .unwrap_or_else(|e| panic!("{e:?}"));
        let header3 =
            StreamFrameHeader::new(3, 300, 1920, 1080, SnapshotFormat::Yuv420p, 4, digest)
                .unwrap_or_else(|e| panic!("{e:?}"));

        let mut ring = BoundedFrameRing::new(2);
        assert_eq!(ring.len(), 0);
        assert!(ring.is_empty());

        let dropped1 = ring
            .push(StreamFrame::new(header1, vec![1, 1, 1, 1]).unwrap_or_else(|e| panic!("{e:?}")));
        assert!(!dropped1);
        assert_eq!(ring.len(), 1);

        let dropped2 = ring
            .push(StreamFrame::new(header2, vec![2, 2, 2, 2]).unwrap_or_else(|e| panic!("{e:?}")));
        assert!(!dropped2);
        assert_eq!(ring.len(), 2);

        // Ring is full (capacity 2). Pushing frame 3 evicts frame 1!
        let dropped3 = ring
            .push(StreamFrame::new(header3, vec![3, 3, 3, 3]).unwrap_or_else(|e| panic!("{e:?}")));
        assert!(dropped3);
        assert_eq!(ring.len(), 2);
        assert_eq!(ring.dropped_count(), 1);

        // Popping yields frame 2, then frame 3
        let pop1 = ring.pop().unwrap_or_else(|| panic!("expected frame 2"));
        assert_eq!(pop1.header.sequence, 2);
        assert_eq!(pop1.payload, vec![2, 2, 2, 2]);

        let pop2 = ring.pop().unwrap_or_else(|| panic!("expected frame 3"));
        assert_eq!(pop2.header.sequence, 3);
        assert_eq!(pop2.payload, vec![3, 3, 3, 3]);

        assert!(ring.pop().is_none());
        assert!(ring.is_empty());
    }

    #[test]
    fn stream_session_lease_lifecycle() {
        let lease = StreamSessionLease::new(101, 1_000, 5_000).unwrap_or_else(|e| panic!("{e:?}"));
        assert_eq!(lease.session_id, 101);
        assert!(!lease.is_expired(1_000));
        assert!(!lease.is_expired(5_999));
        assert!(lease.is_expired(6_000));

        let mut renewable = lease;
        assert!(renewable.renew(4_000, 3_000).is_ok());
        assert_eq!(renewable.expires_at_unix_ms, 7_000);
        assert!(!renewable.is_expired(6_500));
        assert!(renewable.is_expired(7_000));

        // Cannot renew expired lease
        assert_eq!(
            renewable.renew(7_000, 1_000),
            Err(ValidationError::PolicyRejected)
        );

        // Cannot create zero or oversized lease
        assert_eq!(
            StreamSessionLease::new(1, 100, 0),
            Err(ValidationError::InvalidDeadline)
        );
        assert_eq!(
            StreamSessionLease::new(1, 100, MAX_STREAM_LEASE_MS + 1),
            Err(ValidationError::InvalidDeadline)
        );
    }
}
