//! Fixed limits and validated request types shared by Phase 3 media brokers.
//!
//! This crate intentionally performs no device I/O and contains no vendor FFI.
//! Network-facing code must construct these types before contacting a broker;
//! each broker must validate its wire request again before touching hardware.

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Operation {
    Snapshot = 1,
    AttachReadOnlyMedia = 2,
    DetachMedia = 3,
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
}

impl SnapshotFormat {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Jpeg => "jpeg",
            Self::Rgb565 => "rgb565",
            Self::Yuv422p => "yuv422p",
            Self::Yuv420p => "yuv420p",
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
}
