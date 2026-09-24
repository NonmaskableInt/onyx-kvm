//! Fixed-size, fail-closed protocol and frame validator for the privileged video broker.

use cleankvm_core::{PolicyError, Scope, authorize};
use cleankvm_media_protocol::{
    Operation, SNAPSHOT_REQUEST_BYTES, SnapshotFormat, SnapshotRequest, ValidationError,
    VideoSnapshot, WIRE_HEADER_BYTES, WireHeader,
};
use core::fmt;
use sha2::{Digest, Sha256};

pub const FRAME_LEN: usize = WIRE_HEADER_BYTES + SNAPSHOT_REQUEST_BYTES;
pub const EXPECTED_YUV422P_1080P_BYTES: usize = 1920 * 1080 * 2;

#[derive(Debug, Eq, PartialEq)]
pub enum BrokerError {
    VideoDisabled,
    Validation(ValidationError),
    Policy(PolicyError),
    UnsupportedFormat,
    UnsupportedDimensions,
}

impl fmt::Display for BrokerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::VideoDisabled => formatter.write_str("video snapshot capture is disabled"),
            Self::Validation(error) => write!(formatter, "wire validation failed: {error:?}"),
            Self::Policy(error) => write!(formatter, "policy rejected request: {error}"),
            Self::UnsupportedFormat => formatter.write_str("unsupported snapshot pixel format"),
            Self::UnsupportedDimensions => formatter.write_str("unsupported snapshot dimensions"),
        }
    }
}

/// Decode a fixed 20-byte snapshot wire frame and enforce the broker's enablement gate.
///
/// # Errors
/// Rejects disabled state, malformed headers/payloads, non-snapshot operations,
/// and unsupported dimensions/formats.
pub fn decode_and_authorize(
    frame: &[u8; FRAME_LEN],
    video_enabled: bool,
) -> Result<SnapshotRequest, BrokerError> {
    if !video_enabled {
        return Err(BrokerError::VideoDisabled);
    }
    authorize("kvm.video.snapshot", &[Scope::Video], false).map_err(BrokerError::Policy)?;
    let mut header_bytes = [0_u8; WIRE_HEADER_BYTES];
    header_bytes.copy_from_slice(&frame[..WIRE_HEADER_BYTES]);
    let header = WireHeader::decode(header_bytes).map_err(BrokerError::Validation)?;
    if header.operation != Operation::Snapshot {
        return Err(BrokerError::Validation(ValidationError::UnknownOperation));
    }
    if usize::try_from(header.payload_bytes).ok() != Some(SNAPSHOT_REQUEST_BYTES) {
        return Err(BrokerError::Validation(ValidationError::InvalidLength));
    }
    let mut payload_bytes = [0_u8; SNAPSHOT_REQUEST_BYTES];
    payload_bytes.copy_from_slice(&frame[WIRE_HEADER_BYTES..]);
    let request = SnapshotRequest::decode(payload_bytes).map_err(BrokerError::Validation)?;
    if request.format != SnapshotFormat::Yuv422p {
        return Err(BrokerError::UnsupportedFormat);
    }
    if request.width != 1920 || request.height != 1080 {
        return Err(BrokerError::UnsupportedDimensions);
    }
    Ok(request)
}

/// Validate a captured 1920x1080 YUV422P frame buffer and construct its wire response.
///
/// # Errors
/// Rejects buffers whose length does not exactly equal `4,147,200` bytes.
pub fn summarize_yuv422p_frame(bytes: &[u8]) -> Result<VideoSnapshot, BrokerError> {
    if bytes.len() != EXPECTED_YUV422P_1080P_BYTES {
        return Err(BrokerError::Validation(ValidationError::InvalidLength));
    }
    let byte_length = u32::try_from(bytes.len())
        .map_err(|_| BrokerError::Validation(ValidationError::InvalidLength))?;
    let digest = Sha256::digest(bytes);
    let mut sha256 = [0_u8; 32];
    sha256.copy_from_slice(&digest);
    VideoSnapshot::new(1920, 1080, SnapshotFormat::Yuv422p, byte_length, sha256)
        .map_err(BrokerError::Validation)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_wire_frame(format: SnapshotFormat, width: u16, height: u16) -> [u8; FRAME_LEN] {
        let header = WireHeader {
            operation: Operation::Snapshot,
            payload_bytes: 8,
        };
        let request = SnapshotRequest::new(width, height, format, 2_000)
            .unwrap_or_else(|error| panic!("unexpected request error: {error:?}"));
        let mut frame = [0_u8; FRAME_LEN];
        frame[..WIRE_HEADER_BYTES].copy_from_slice(&header.encode());
        frame[WIRE_HEADER_BYTES..].copy_from_slice(&request.encode());
        frame
    }

    #[test]
    fn video_gate_is_fail_closed() {
        let frame = valid_wire_frame(SnapshotFormat::Yuv422p, 1920, 1080);
        assert_eq!(
            decode_and_authorize(&frame, false),
            Err(BrokerError::VideoDisabled)
        );
    }

    #[test]
    fn decodes_only_1080p_yuv422p_requests() {
        let valid = valid_wire_frame(SnapshotFormat::Yuv422p, 1920, 1080);
        let request = decode_and_authorize(&valid, true)
            .unwrap_or_else(|error| panic!("valid frame rejected: {error}"));
        assert_eq!(request.width, 1920);
        assert_eq!(request.height, 1080);
        assert_eq!(request.format, SnapshotFormat::Yuv422p);

        let wrong_format = valid_wire_frame(SnapshotFormat::Jpeg, 1920, 1080);
        assert_eq!(
            decode_and_authorize(&wrong_format, true),
            Err(BrokerError::UnsupportedFormat)
        );

        let wrong_dims = valid_wire_frame(SnapshotFormat::Yuv422p, 1280, 720);
        assert_eq!(
            decode_and_authorize(&wrong_dims, true),
            Err(BrokerError::UnsupportedDimensions)
        );
    }

    #[test]
    fn summarizes_exact_frame_and_rejects_truncated_buffers() {
        let buffer = vec![0x42_u8; EXPECTED_YUV422P_1080P_BYTES];
        let summary = summarize_yuv422p_frame(&buffer)
            .unwrap_or_else(|error| panic!("valid 1080p buffer failed: {error}"));
        assert_eq!(summary.width, 1920);
        assert_eq!(summary.height, 1080);
        assert_eq!(summary.byte_length, 4_147_200);
        assert_eq!(summary.sha256_hex().len(), 64);

        assert_eq!(
            summarize_yuv422p_frame(&[0_u8; 1024]),
            Err(BrokerError::Validation(ValidationError::InvalidLength))
        );
    }
}
