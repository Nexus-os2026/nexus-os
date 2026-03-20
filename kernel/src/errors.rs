use crate::audit::AuditError;
use crate::fuel_hardening::FuelViolation;
use crate::lifecycle::AgentState;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AgentError {
    #[error("fuel budget exhausted")]
    FuelExhausted,
    #[error("fuel violation '{violation:?}': {reason}")]
    FuelViolation {
        violation: FuelViolation,
        reason: String,
    },
    #[error("invalid state transition from '{from}' to '{to}'")]
    InvalidTransition { from: AgentState, to: AgentState },
    #[error("manifest error: {0}")]
    ManifestError(String),
    #[error("capability denied: '{0}' is not allowed")]
    CapabilityDenied(String),
    #[error("approval required: request_id='{request_id}'")]
    ApprovalRequired { request_id: String },
    #[error("supervisor error: {0}")]
    SupervisorError(String),
    #[error("key '{0}' has been destroyed")]
    KeyDestroyed(String),
    #[error("adversarial challenge failed: {0}")]
    AdversarialBlock(String),
    #[error("audit failure: {0}")]
    AuditFailure(#[from] AuditError),
}

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
        | AgentError::AdversarialBlock(_)
        | AgentError::ApprovalRequired { .. }
        | AgentError::KeyDestroyed(_)
        | AgentError::AuditFailure(_) => ErrorStrategy::Escalate,
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
