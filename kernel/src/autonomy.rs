use crate::audit::{AuditTrail, EventType};
use crate::errors::AgentError;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::error::Error;
use std::fmt::{Display, Formatter};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
pub enum AutonomyLevel {
    #[default]
    L0,
    L1,
    L2,
    L3,
    L4,
    L5,
}

impl AutonomyLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            AutonomyLevel::L0 => "L0",
            AutonomyLevel::L1 => "L1",
            AutonomyLevel::L2 => "L2",
            AutonomyLevel::L3 => "L3",
            AutonomyLevel::L4 => "L4",
            AutonomyLevel::L5 => "L5",
        }
    }

    pub fn from_numeric(value: u8) -> Option<Self> {
        match value {
            0 => Some(AutonomyLevel::L0),
            1 => Some(AutonomyLevel::L1),
            2 => Some(AutonomyLevel::L2),
            3 => Some(AutonomyLevel::L3),
            4 => Some(AutonomyLevel::L4),
            5 => Some(AutonomyLevel::L5),
            _ => None,
        }
    }

    pub fn from_manifest(value: Option<u8>) -> Self {
        match value.and_then(Self::from_numeric) {
            Some(level) => level,
            None => AutonomyLevel::L0,
        }
    }

    pub fn previous(self) -> Self {
        match self {
            AutonomyLevel::L0 => AutonomyLevel::L0,
            AutonomyLevel::L1 => AutonomyLevel::L0,
            AutonomyLevel::L2 => AutonomyLevel::L1,
            AutonomyLevel::L3 => AutonomyLevel::L2,
            AutonomyLevel::L4 => AutonomyLevel::L3,
            AutonomyLevel::L5 => AutonomyLevel::L4,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutonomyError {
    Denied {
        required: AutonomyLevel,
        current: AutonomyLevel,
        action: &'static str,
        downgraded_to: Option<AutonomyLevel>,
    },
}

impl Display for AutonomyError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            AutonomyError::Denied {
                required,
                current,
                action,
                downgraded_to,
            } => match downgraded_to {
                Some(level) => write!(
                    f,
                    "autonomy denied action '{}' (required={}, current={}) downgraded_to={}",
                    action,
                    required.as_str(),
                    current.as_str(),
                    level.as_str()
                ),
                None => write!(
                    f,
                    "autonomy denied action '{}' (required={}, current={})",
                    action,
                    required.as_str(),
                    current.as_str()
                ),
            },
        }
    }
}

impl Error for AutonomyError {}

impl From<AutonomyError> for AgentError {
    fn from(value: AutonomyError) -> Self {
        AgentError::SupervisorError(value.to_string())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutonomyPolicyHooks {
    pub tool_call_min: AutonomyLevel,
    pub multi_agent_min: AutonomyLevel,
    pub self_modification_min: AutonomyLevel,
    pub distributed_min: AutonomyLevel,
}

impl Default for AutonomyPolicyHooks {
    fn default() -> Self {
        Self {
            tool_call_min: AutonomyLevel::L1,
            multi_agent_min: AutonomyLevel::L2,
            self_modification_min: AutonomyLevel::L4,
            distributed_min: AutonomyLevel::L5,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutonomyGuard {
    level: AutonomyLevel,
    hooks: AutonomyPolicyHooks,
}

impl Default for AutonomyGuard {
    fn default() -> Self {
        Self::new(AutonomyLevel::L0)
    }
}

impl AutonomyGuard {
    pub fn new(level: AutonomyLevel) -> Self {
        Self {
            level,
            hooks: AutonomyPolicyHooks::default(),
        }
    }

    pub fn with_hooks(level: AutonomyLevel, hooks: AutonomyPolicyHooks) -> Self {
        Self { level, hooks }
    }

    pub fn level(&self) -> AutonomyLevel {
        self.level
    }

    pub fn require_tool_call(
        &mut self,
        actor_id: Uuid,
        audit_trail: &mut AuditTrail,
    ) -> Result<(), AutonomyError> {
        self.require_level(actor_id, audit_trail, self.hooks.tool_call_min, "tool_call")
    }

    pub fn require_multi_agent(
        &mut self,
        actor_id: Uuid,
        audit_trail: &mut AuditTrail,
    ) -> Result<(), AutonomyError> {
        self.require_level(
            actor_id,
            audit_trail,
            self.hooks.multi_agent_min,
            "multi_agent",
        )
    }

    pub fn require_self_modification(
        &mut self,
        actor_id: Uuid,
        audit_trail: &mut AuditTrail,
    ) -> Result<(), AutonomyError> {
        self.require_level(
            actor_id,
            audit_trail,
            self.hooks.self_modification_min,
            "self_modification",
        )
    }

    pub fn require_distributed(
        &mut self,
        actor_id: Uuid,
        audit_trail: &mut AuditTrail,
    ) -> Result<(), AutonomyError> {
        self.require_level(
            actor_id,
            audit_trail,
            self.hooks.distributed_min,
            "distributed",
        )
    }

    pub fn downgrade(
        &mut self,
        actor_id: Uuid,
        new_level: AutonomyLevel,
        action: &'static str,
        reason: &str,
        audit_trail: &mut AuditTrail,
    ) {
        let previous = self.level;
        self.level = new_level.min(previous);
        let _ = audit_trail.append_event(
            actor_id,
            EventType::StateChange,
            json!({
                "event": "autonomy.level_changed",
                "action": action,
                "previous_level": previous.as_str(),
                "new_level": self.level.as_str(),
                "reason": reason,
            }),
        );
    }

    fn require_level(
        &mut self,
        actor_id: Uuid,
        audit_trail: &mut AuditTrail,
        required: AutonomyLevel,
        action: &'static str,
    ) -> Result<(), AutonomyError> {
        if self.level >= required {
            return Ok(());
        }

        let current = self.level;
        let downgrade_target = current.previous();
        let reason = format!(
            "action '{}' requires {} while current level is {}",
            action,
            required.as_str(),
            current.as_str(),
        );
        self.downgrade(
            actor_id,
            downgrade_target,
            action,
            reason.as_str(),
            audit_trail,
        );
        Err(AutonomyError::Denied {
            required,
            current,
            action,
            downgraded_to: Some(self.level),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{AutonomyError, AutonomyGuard, AutonomyLevel};
    use crate::audit::AuditTrail;
    use uuid::Uuid;

    #[test]
    fn denied_tool_call_at_l0() {
        let mut guard = AutonomyGuard::new(AutonomyLevel::L0);
        let mut audit = AuditTrail::new();
        let result = guard.require_tool_call(Uuid::new_v4(), &mut audit);
        assert!(matches!(result, Err(AutonomyError::Denied { .. })));
    }

    #[test]
    fn allowed_tool_call_at_l1() {
        let mut guard = AutonomyGuard::new(AutonomyLevel::L1);
        let mut audit = AuditTrail::new();
        let result = guard.require_tool_call(Uuid::new_v4(), &mut audit);
        assert!(result.is_ok());
    }

    #[test]
    fn denied_self_modification_at_l3() {
        let mut guard = AutonomyGuard::new(AutonomyLevel::L3);
        let mut audit = AuditTrail::new();
        let result = guard.require_self_modification(Uuid::new_v4(), &mut audit);
        assert!(matches!(result, Err(AutonomyError::Denied { .. })));
        assert_eq!(guard.level(), AutonomyLevel::L2);
    }

    #[test]
    fn downgrade_on_violation_emits_audit_event() {
        let actor_id = Uuid::new_v4();
        let mut guard = AutonomyGuard::new(AutonomyLevel::L3);
        let mut audit = AuditTrail::new();
        let _ = guard.require_self_modification(actor_id, &mut audit);

        let changed = audit
            .events()
            .iter()
            .find(|event| {
                event.payload.get("event").and_then(|value| value.as_str())
                    == Some("autonomy.level_changed")
            })
            .expect("expected autonomy level change audit event");
        assert_eq!(
            changed
                .payload
                .get("action")
                .and_then(|value| value.as_str()),
            Some("self_modification")
        );
        assert_eq!(
            changed
                .payload
                .get("previous_level")
                .and_then(|value| value.as_str()),
            Some("L3")
        );
        assert_eq!(
            changed
                .payload
                .get("new_level")
                .and_then(|value| value.as_str()),
            Some("L2")
        );
        assert!(changed
            .payload
            .get("reason")
            .and_then(|value| value.as_str())
            .is_some_and(|reason| reason.contains("requires L4")));
    }
}
