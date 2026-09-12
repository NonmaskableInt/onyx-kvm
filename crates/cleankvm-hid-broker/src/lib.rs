//! Fixed-size, fail-closed protocol for the privileged HID writer.

use cleankvm_core::{PolicyError, Scope, authorize};
use core::fmt;

const MAGIC: [u8; 2] = *b"CK";
const VERSION: u8 = 1;
const HEADER_LEN: usize = 5;
pub const MAX_FRAME_LEN: usize = HEADER_LEN + 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Report {
    Keyboard([u8; 8]),
    Pointer([u8; 4]),
}

impl Report {
    #[must_use]
    pub const fn operation_name(self) -> &'static str {
        match self {
            Self::Keyboard(_) => "kvm.keyboard.send",
            Self::Pointer(_) => "kvm.pointer.send",
        }
    }

    #[must_use]
    pub const fn bytes(self) -> ReportBytes {
        match self {
            Self::Keyboard(bytes) => ReportBytes::Keyboard(bytes),
            Self::Pointer(bytes) => ReportBytes::Pointer(bytes),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReportBytes {
    Keyboard([u8; 8]),
    Pointer([u8; 4]),
}

#[derive(Debug, Eq, PartialEq)]
pub enum ProtocolError {
    InvalidLength,
    InvalidMagic,
    UnsupportedVersion,
    UnknownOperation,
    InvalidPayloadLength,
    Policy(PolicyError),
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLength => formatter.write_str("invalid frame length"),
            Self::InvalidMagic => formatter.write_str("invalid frame magic"),
            Self::UnsupportedVersion => formatter.write_str("unsupported protocol version"),
            Self::UnknownOperation => formatter.write_str("unknown HID operation"),
            Self::InvalidPayloadLength => formatter.write_str("invalid HID payload length"),
            Self::Policy(error) => write!(formatter, "policy rejected request: {error}"),
        }
    }
}

/// Parse a complete frame and independently apply the broker's mutation gate.
///
/// # Errors
///
/// Rejects malformed frames and every request while `mutations_enabled` is false.
pub fn decode_and_authorize(
    frame: &[u8],
    mutations_enabled: bool,
) -> Result<Report, ProtocolError> {
    if !(HEADER_LEN..=MAX_FRAME_LEN).contains(&frame.len()) {
        return Err(ProtocolError::InvalidLength);
    }
    if frame[..2] != MAGIC {
        return Err(ProtocolError::InvalidMagic);
    }
    if frame[2] != VERSION {
        return Err(ProtocolError::UnsupportedVersion);
    }
    let payload_len = usize::from(frame[4]);
    if frame.len() != HEADER_LEN + payload_len {
        return Err(ProtocolError::InvalidPayloadLength);
    }
    let report = match (frame[3], payload_len) {
        (1, 8) => {
            let mut report = [0_u8; 8];
            report.copy_from_slice(&frame[HEADER_LEN..]);
            Report::Keyboard(report)
        }
        (2, 4) => {
            let mut report = [0_u8; 4];
            report.copy_from_slice(&frame[HEADER_LEN..]);
            Report::Pointer(report)
        }
        (1 | 2, _) => return Err(ProtocolError::InvalidPayloadLength),
        _ => return Err(ProtocolError::UnknownOperation),
    };
    authorize(
        report.operation_name(),
        &[Scope::Control],
        mutations_enabled,
    )
    .map_err(ProtocolError::Policy)?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mutation_gate_is_fail_closed() {
        let frame = [b'C', b'K', 1, 1, 8, 0, 0, 4, 0, 0, 0, 0, 0];
        assert_eq!(
            decode_and_authorize(&frame, false),
            Err(ProtocolError::Policy(
                PolicyError::MutatingOperationsDisabled
            ))
        );
    }

    #[test]
    fn decodes_only_exact_keyboard_and_pointer_reports() {
        let keyboard = [b'C', b'K', 1, 1, 8, 2, 0, 4, 0, 0, 0, 0, 0];
        let pointer = [b'C', b'K', 1, 2, 4, 1, 127, 128, 255];
        assert_eq!(
            decode_and_authorize(&keyboard, true),
            Ok(Report::Keyboard([2, 0, 4, 0, 0, 0, 0, 0]))
        );
        assert_eq!(
            decode_and_authorize(&pointer, true),
            Ok(Report::Pointer([1, 127, 128, 255]))
        );
    }

    #[test]
    fn rejects_malformed_or_extensible_frames() {
        assert_eq!(
            decode_and_authorize(b"", true),
            Err(ProtocolError::InvalidLength)
        );
        assert_eq!(
            decode_and_authorize(&[b'X', b'K', 1, 2, 4, 0, 0, 0, 0], true),
            Err(ProtocolError::InvalidMagic)
        );
        assert_eq!(
            decode_and_authorize(&[b'C', b'K', 2, 2, 4, 0, 0, 0, 0], true),
            Err(ProtocolError::UnsupportedVersion)
        );
        assert_eq!(
            decode_and_authorize(&[b'C', b'K', 1, 3, 4, 0, 0, 0, 0], true),
            Err(ProtocolError::UnknownOperation)
        );
        assert_eq!(
            decode_and_authorize(&[b'C', b'K', 1, 1, 7, 0, 0, 0, 0, 0, 0, 0], true),
            Err(ProtocolError::InvalidPayloadLength)
        );
    }
}
