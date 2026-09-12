//! Hardware-neutral types shared by all `CleanKVM` processes.

use core::fmt;

/// Authority required for an operation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Scope {
    Read,
    Video,
    AudioCapture,
    AudioOutput,
    Control,
    Media,
    Admin,
}

impl Scope {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Read => "kvm:read",
            Self::Video => "kvm:video",
            Self::AudioCapture => "kvm:audio-capture",
            Self::AudioOutput => "kvm:audio-output",
            Self::Control => "kvm:control",
            Self::Media => "kvm:media",
            Self::Admin => "kvm:admin",
        }
    }
}

/// Whether an operation can affect the KVM or attached host.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Effect {
    ReadOnly,
    Mutating,
    Disruptive,
}

/// Stable names used at protocol and audit boundaries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Operation {
    pub name: &'static str,
    pub scope: Scope,
    pub effect: Effect,
}

pub const OPERATIONS: &[Operation] = &[
    Operation {
        name: "kvm.status",
        scope: Scope::Read,
        effect: Effect::ReadOnly,
    },
    Operation {
        name: "kvm.video.snapshot",
        scope: Scope::Video,
        effect: Effect::ReadOnly,
    },
    Operation {
        name: "kvm.audio.capture",
        scope: Scope::AudioCapture,
        effect: Effect::ReadOnly,
    },
    Operation {
        name: "kvm.audio.play",
        scope: Scope::AudioOutput,
        effect: Effect::Mutating,
    },
    Operation {
        name: "kvm.keyboard.send",
        scope: Scope::Control,
        effect: Effect::Mutating,
    },
    Operation {
        name: "kvm.pointer.send",
        scope: Scope::Control,
        effect: Effect::Mutating,
    },
    Operation {
        name: "kvm.media.attach",
        scope: Scope::Media,
        effect: Effect::Mutating,
    },
    Operation {
        name: "kvm.device.reboot",
        scope: Scope::Admin,
        effect: Effect::Disruptive,
    },
];

#[derive(Debug, Eq, PartialEq)]
pub enum PolicyError {
    UnknownOperation,
    WrongAudience,
    ExpiredGrant,
    MissingScope(Scope),
    MutatingOperationsDisabled,
}

impl fmt::Display for PolicyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownOperation => formatter.write_str("unknown operation"),
            Self::WrongAudience => formatter.write_str("grant has the wrong audience"),
            Self::ExpiredGrant => formatter.write_str("grant is expired"),
            Self::MissingScope(scope) => write!(formatter, "missing scope {}", scope.as_str()),
            Self::MutatingOperationsDisabled => {
                formatter.write_str("mutating operations are disabled")
            }
        }
    }
}

/// Validated authorization facts supplied by a transport adapter.
///
/// This type deliberately contains no bearer-token material. HTTPS/OAuth code
/// must validate token integrity first, then pass only these bounded claims to
/// the shared policy layer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Grant<'a> {
    pub audience: &'a str,
    pub expires_at_unix_seconds: u64,
    pub scopes: &'a [Scope],
    pub mutation_enabled: bool,
}

/// Authorize an operation using audience-bound, expiring claims.
///
/// # Errors
///
/// Returns [`PolicyError`] when the audience differs, the grant is expired, or
/// the operation fails the default-deny scope and mutation policy.
pub fn authorize_grant(
    name: &str,
    expected_audience: &str,
    now_unix_seconds: u64,
    grant: Grant<'_>,
) -> Result<Operation, PolicyError> {
    if grant.audience != expected_audience {
        return Err(PolicyError::WrongAudience);
    }
    if grant.expires_at_unix_seconds <= now_unix_seconds {
        return Err(PolicyError::ExpiredGrant);
    }
    authorize(name, grant.scopes, grant.mutation_enabled)
}

/// Default-deny authorization policy shared by every protocol adapter.
///
/// # Errors
///
/// Returns [`PolicyError`] when the operation is unknown, its scope is absent,
/// or a mutating operation is requested while mutations are disabled.
pub fn authorize(
    name: &str,
    scopes: &[Scope],
    mutation_enabled: bool,
) -> Result<Operation, PolicyError> {
    let operation = OPERATIONS
        .iter()
        .find(|operation| operation.name == name)
        .copied()
        .ok_or(PolicyError::UnknownOperation)?;
    if !scopes.contains(&operation.scope) {
        return Err(PolicyError::MissingScope(operation.scope));
    }
    if operation.effect != Effect::ReadOnly && !mutation_enabled {
        return Err(PolicyError::MutatingOperationsDisabled);
    }
    Ok(operation)
}

#[cfg(test)]
mod tests {
    use super::{Effect, Grant, PolicyError, Scope, authorize, authorize_grant};

    #[test]
    fn read_operation_requires_its_scope() {
        assert_eq!(
            authorize("kvm.status", &[], false),
            Err(PolicyError::MissingScope(Scope::Read))
        );
        assert_eq!(
            authorize("kvm.status", &[Scope::Read], false).map(|op| op.effect),
            Ok(Effect::ReadOnly)
        );
    }

    #[test]
    fn mutations_are_default_deny() {
        assert_eq!(
            authorize("kvm.keyboard.send", &[Scope::Control], false),
            Err(PolicyError::MutatingOperationsDisabled)
        );
    }

    #[test]
    fn grants_are_audience_bound_and_expire_fail_closed() {
        let grant = Grant {
            audience: "https://kvm.example/mcp",
            expires_at_unix_seconds: 101,
            scopes: &[Scope::Read],
            mutation_enabled: false,
        };
        assert!(authorize_grant("kvm.status", "https://kvm.example/mcp", 100, grant).is_ok());
        assert_eq!(
            authorize_grant("kvm.status", "https://other.example/mcp", 100, grant),
            Err(PolicyError::WrongAudience)
        );
        assert_eq!(
            authorize_grant("kvm.status", "https://kvm.example/mcp", 101, grant),
            Err(PolicyError::ExpiredGrant)
        );
    }

    #[test]
    fn a_valid_grant_still_cannot_enable_mutation_implicitly() {
        let grant = Grant {
            audience: "https://kvm.example/mcp",
            expires_at_unix_seconds: 200,
            scopes: &[Scope::Control],
            mutation_enabled: false,
        };
        assert_eq!(
            authorize_grant("kvm.keyboard.send", "https://kvm.example/mcp", 100, grant),
            Err(PolicyError::MutatingOperationsDisabled)
        );
    }
}
