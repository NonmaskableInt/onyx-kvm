//! Fixed-size, fail-closed protocol and frame validator for the privileged video broker.

use core::fmt;
use onyx_kvm_core::{PolicyError, Scope, authorize};
use onyx_kvm_media_protocol::{
    Operation, SNAPSHOT_REQUEST_BYTES, SnapshotFormat, SnapshotRequest, ValidationError,
    VideoSnapshot, WIRE_HEADER_BYTES, WireHeader,
};
use sha2::{Digest, Sha256};

pub const FRAME_LEN: usize = WIRE_HEADER_BYTES + SNAPSHOT_REQUEST_BYTES;
pub const EXPECTED_YUV420P_1080P_BYTES: usize = 1920 * 1080 * 3 / 2;
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
) -> Result<(Operation, SnapshotRequest), BrokerError> {
    if !video_enabled {
        return Err(BrokerError::VideoDisabled);
    }
    authorize("kvm.video.snapshot", &[Scope::Video], false).map_err(BrokerError::Policy)?;
    let mut header_bytes = [0_u8; WIRE_HEADER_BYTES];
    header_bytes.copy_from_slice(&frame[..WIRE_HEADER_BYTES]);
    let header = WireHeader::decode(header_bytes).map_err(BrokerError::Validation)?;
    if header.operation != Operation::Snapshot && header.operation != Operation::SnapshotFrame {
        return Err(BrokerError::Validation(ValidationError::UnknownOperation));
    }
    if usize::try_from(header.payload_bytes).ok() != Some(SNAPSHOT_REQUEST_BYTES) {
        return Err(BrokerError::Validation(ValidationError::InvalidLength));
    }
    let mut payload_bytes = [0_u8; SNAPSHOT_REQUEST_BYTES];
    payload_bytes.copy_from_slice(&frame[WIRE_HEADER_BYTES..]);
    let request = SnapshotRequest::decode(payload_bytes).map_err(BrokerError::Validation)?;
    if !matches!(
        request.format,
        SnapshotFormat::Yuv420p | SnapshotFormat::Yuv422p
    ) {
        return Err(BrokerError::UnsupportedFormat);
    }
    if request.width != 1920 || request.height != 1080 {
        return Err(BrokerError::UnsupportedDimensions);
    }
    Ok((header.operation, request))
}

/// Validate a captured 1920x1080 planar YUV frame buffer and construct its wire response.
///
/// # Errors
/// Rejects a buffer unless its length exactly matches the requested pixel format.
pub fn summarize_frame(bytes: &[u8], format: SnapshotFormat) -> Result<VideoSnapshot, BrokerError> {
    let expected = match format {
        SnapshotFormat::Yuv420p => EXPECTED_YUV420P_1080P_BYTES,
        SnapshotFormat::Yuv422p => EXPECTED_YUV422P_1080P_BYTES,
        SnapshotFormat::Jpeg | SnapshotFormat::Rgb565 => {
            return Err(BrokerError::UnsupportedFormat);
        }
    };
    if bytes.len() != expected {
        return Err(BrokerError::Validation(ValidationError::InvalidLength));
    }
    let byte_length = u32::try_from(bytes.len())
        .map_err(|_| BrokerError::Validation(ValidationError::InvalidLength))?;
    let digest = Sha256::digest(bytes);
    let mut sha256 = [0_u8; 32];
    sha256.copy_from_slice(&digest);
    VideoSnapshot::new(1920, 1080, format, byte_length, sha256).map_err(BrokerError::Validation)
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
    fn decodes_only_1080p_planar_yuv_requests() {
        let valid = valid_wire_frame(SnapshotFormat::Yuv420p, 1920, 1080);
        let (op, request) = decode_and_authorize(&valid, true)
            .unwrap_or_else(|error| panic!("valid frame rejected: {error}"));
        assert_eq!(op, Operation::Snapshot);
        assert_eq!(request.width, 1920);
        assert_eq!(request.height, 1080);
        assert_eq!(request.format, SnapshotFormat::Yuv420p);

        let yuv422 = valid_wire_frame(SnapshotFormat::Yuv422p, 1920, 1080);
        assert!(decode_and_authorize(&yuv422, true).is_ok());

        let mut frame_op = valid;
        let frame_header = WireHeader {
            operation: Operation::SnapshotFrame,
            payload_bytes: 8,
        };
        frame_op[..WIRE_HEADER_BYTES].copy_from_slice(&frame_header.encode());
        let (op2, req2) = decode_and_authorize(&frame_op, true)
            .unwrap_or_else(|error| panic!("valid snapshot frame rejected: {error}"));
        assert_eq!(op2, Operation::SnapshotFrame);
        assert_eq!(req2.format, SnapshotFormat::Yuv420p);

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
        let summary = summarize_frame(&buffer, SnapshotFormat::Yuv422p)
            .unwrap_or_else(|error| panic!("valid 1080p buffer failed: {error}"));
        assert_eq!(summary.width, 1920);
        assert_eq!(summary.height, 1080);
        assert_eq!(summary.byte_length, 4_147_200);
        assert_eq!(summary.sha256_hex().len(), 64);

        let two_plane_buffer = vec![0x24_u8; EXPECTED_YUV420P_1080P_BYTES];
        let two_plane_summary = summarize_frame(&two_plane_buffer, SnapshotFormat::Yuv420p)
            .unwrap_or_else(|error| panic!("valid 2-plane 1080p buffer failed: {error}"));
        assert_eq!(two_plane_summary.byte_length, 3_110_400);
        assert_eq!(two_plane_summary.format, SnapshotFormat::Yuv420p);

        assert_eq!(
            summarize_frame(&two_plane_buffer, SnapshotFormat::Yuv422p),
            Err(BrokerError::Validation(ValidationError::InvalidLength))
        );

        assert_eq!(
            summarize_frame(&[0_u8; 1024], SnapshotFormat::Yuv420p),
            Err(BrokerError::Validation(ValidationError::InvalidLength))
        );
    }
}
