use crate::fuel_hardening::FuelViolation;
use crate::lifecycle::AgentState;
use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentError {
    FuelExhausted,
    FuelViolation {
        violation: FuelViolation,
        reason: String,
    },
    InvalidTransition {
        from: AgentState,
        to: AgentState,
    },
    ManifestError(String),
    CapabilityDenied(String),
    ApprovalRequired { request_id: String },
    SupervisorError(String),
    KeyDestroyed(String),
}

impl Display for AgentError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentError::FuelExhausted => write!(f, "fuel budget exhausted"),
            AgentError::FuelViolation { violation, reason } => {
                write!(f, "fuel violation '{violation:?}': {reason}")
            }
            AgentError::InvalidTransition { from, to } => {
                write!(f, "invalid state transition from '{from}' to '{to}'")
            }
            AgentError::ManifestError(reason) => write!(f, "manifest error: {reason}"),
            AgentError::CapabilityDenied(capability) => {
                write!(f, "capability denied: '{capability}' is not allowed")
            }
            AgentError::ApprovalRequired { request_id } => {
                write!(f, "approval required: request_id='{request_id}'")
            }
            AgentError::SupervisorError(reason) => write!(f, "supervisor error: {reason}"),
            AgentError::KeyDestroyed(key_id) => write!(f, "key '{key_id}' has been destroyed"),
        }
    }
}

impl Error for AgentError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorStrategy {
    Retry { max_attempts: u8 },
    Skip,
    Escalate,
}

pub fn on_error(error: &AgentError) -> ErrorStrategy {
    match error {
        AgentError::FuelExhausted
        | AgentError::FuelViolation { .. }
        | AgentError::CapabilityDenied(_)
        | AgentError::ApprovalRequired { .. }
        | AgentError::KeyDestroyed(_) => ErrorStrategy::Escalate,
        AgentError::InvalidTransition { .. } | AgentError::ManifestError(_) => ErrorStrategy::Skip,
        AgentError::SupervisorError(_) => ErrorStrategy::Retry { max_attempts: 3 },
    }
}

#[cfg(test)]
mod tests {
    use super::{on_error, AgentError, ErrorStrategy};
    use crate::fuel_hardening::FuelViolation;
    use crate::lifecycle::AgentState;

    #[test]
    fn test_error_display() {
        let fuel = AgentError::FuelExhausted;
        let violation = AgentError::FuelViolation {
            violation: FuelViolation::OverMonthlyCap,
            reason: "cap exceeded".to_string(),
        };
        let transition = AgentError::InvalidTransition {
            from: AgentState::Created,
            to: AgentState::Paused,
        };
        let manifest = AgentError::ManifestError("missing field: name".to_string());
        let capability = AgentError::CapabilityDenied("web.search".to_string());
        let approval = AgentError::ApprovalRequired {
            request_id: "req-123".to_string(),
        };
        let supervisor = AgentError::SupervisorError("agent not found".to_string());

        assert_eq!(fuel.to_string(), "fuel budget exhausted");
        assert_eq!(
            violation.to_string(),
            "fuel violation 'OverMonthlyCap': cap exceeded"
        );
        assert_eq!(
            transition.to_string(),
            "invalid state transition from 'Created' to 'Paused'"
        );
        assert_eq!(manifest.to_string(), "manifest error: missing field: name");
        assert_eq!(
            capability.to_string(),
            "capability denied: 'web.search' is not allowed"
        );
        assert_eq!(
            approval.to_string(),
            "approval required: request_id='req-123'"
        );
        assert_eq!(supervisor.to_string(), "supervisor error: agent not found");
    }

    #[test]
    fn test_on_error_strategy_defaults() {
        let retry = on_error(&AgentError::SupervisorError(
            "temporary failure".to_string(),
        ));
        let escalate = on_error(&AgentError::FuelExhausted);
        let skip = on_error(&AgentError::ManifestError("bad config".to_string()));

        assert_eq!(retry, ErrorStrategy::Retry { max_attempts: 3 });
        assert_eq!(escalate, ErrorStrategy::Escalate);
        assert_eq!(skip, ErrorStrategy::Skip);
    }
}
