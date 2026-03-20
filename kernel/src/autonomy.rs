use crate::audit::{AuditTrail, EventType};
use crate::errors::AgentError;
use crate::policy_engine::{EvaluationContext, PolicyDecision, PolicyEngine};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::str::FromStr;
use thiserror::Error;
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
    L6,
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
            AutonomyLevel::L6 => "L6",
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
            6 => Some(AutonomyLevel::L6),
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
            AutonomyLevel::L6 => AutonomyLevel::L5,
        }
    }
}

impl std::fmt::Display for AutonomyLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for AutonomyLevel {
    type Err = AgentError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_uppercase().as_str() {
            "L0" | "0" => Ok(AutonomyLevel::L0),
            "L1" | "1" => Ok(AutonomyLevel::L1),
            "L2" | "2" => Ok(AutonomyLevel::L2),
            "L3" | "3" => Ok(AutonomyLevel::L3),
            "L4" | "4" => Ok(AutonomyLevel::L4),
            "L5" | "5" => Ok(AutonomyLevel::L5),
            "L6" | "6" => Ok(AutonomyLevel::L6),
            other => Err(AgentError::ManifestError(format!(
                "unknown autonomy level '{other}'"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum AutonomyError {
    #[error("autonomy denied action '{action}' (required={}, current={}){}", required.as_str(), current.as_str(), downgraded_to.map(|l| format!(" downgraded_to={}", l.as_str())).unwrap_or_default())]
    Denied {
        required: AutonomyLevel,
        current: AutonomyLevel,
        action: &'static str,
        downgraded_to: Option<AutonomyLevel>,
    },
}

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

    /// Require L4+ for self-evolution actions (modify own description/strategy,
    /// run evolution tournaments).
    pub fn require_self_evolution(
        &mut self,
        actor_id: Uuid,
        audit_trail: &mut AuditTrail,
    ) -> Result<(), AutonomyError> {
        self.require_level(actor_id, audit_trail, AutonomyLevel::L4, "self_evolution")
    }

    /// Require L4+ for agent creation/destruction by another agent.
    pub fn require_agent_creation(
        &mut self,
        actor_id: Uuid,
        audit_trail: &mut AuditTrail,
    ) -> Result<(), AutonomyError> {
        self.require_level(actor_id, audit_trail, AutonomyLevel::L4, "agent_creation")
    }

    /// Require L5 for modifying system-wide governance policies.
    pub fn require_governance_modification(
        &mut self,
        actor_id: Uuid,
        audit_trail: &mut AuditTrail,
    ) -> Result<(), AutonomyError> {
        self.require_level(
            actor_id,
            audit_trail,
            AutonomyLevel::L5,
            "governance_modification",
        )
    }

    /// Require L5 for sovereign-only actions (ecosystem fuel allocation,
    /// agent lifecycle management across the system).
    pub fn require_sovereign_action(
        &mut self,
        actor_id: Uuid,
        audit_trail: &mut AuditTrail,
    ) -> Result<(), AutonomyError> {
        self.require_level(actor_id, audit_trail, AutonomyLevel::L5, "sovereign_action")
    }

    /// Require L6 for changing the agent's cognitive loop parameters.
    pub fn require_cognitive_modification(
        &mut self,
        actor_id: Uuid,
        audit_trail: &mut AuditTrail,
    ) -> Result<(), AutonomyError> {
        self.require_level(
            actor_id,
            audit_trail,
            AutonomyLevel::L6,
            "cognitive_modification",
        )
    }

    /// Require L6 for phase-specific multi-model orchestration.
    pub fn require_multi_model_orchestration(
        &mut self,
        actor_id: Uuid,
        audit_trail: &mut AuditTrail,
    ) -> Result<(), AutonomyError> {
        self.require_level(
            actor_id,
            audit_trail,
            AutonomyLevel::L6,
            "multi_model_orchestration",
        )
    }

    /// Require L6 for dynamic algorithm selection.
    pub fn require_algorithm_selection(
        &mut self,
        actor_id: Uuid,
        audit_trail: &mut AuditTrail,
    ) -> Result<(), AutonomyError> {
        self.require_level(
            actor_id,
            audit_trail,
            AutonomyLevel::L6,
            "algorithm_selection",
        )
    }

    /// Require L6 for designing and deploying agent ecosystems.
    pub fn require_ecosystem_design(
        &mut self,
        actor_id: Uuid,
        audit_trail: &mut AuditTrail,
    ) -> Result<(), AutonomyError> {
        self.require_level(actor_id, audit_trail, AutonomyLevel::L6, "ecosystem_design")
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
        if let Err(e) = audit_trail.append_event(
            actor_id,
            EventType::StateChange,
            json!({
                "event": "autonomy.level_changed",
                "action": action,
                "previous_level": previous.as_str(),
                "new_level": self.level.as_str(),
                "reason": reason,
            }),
        ) {
            eprintln!("audit write failed: {e}");
        }
    }

    /// Check the policy engine for an autonomy override before falling back
    /// to the hardcoded minimum level.  Policies can raise the required
    /// autonomy level (enterprises restricting specific operations) but the
    /// result is always at least `default_required`.
    pub fn require_level_with_policy(
        &mut self,
        actor_id: Uuid,
        audit_trail: &mut AuditTrail,
        default_required: AutonomyLevel,
        action: &'static str,
        policy_engine: Option<&PolicyEngine>,
    ) -> Result<(), AutonomyError> {
        let mut effective = default_required;

        if let Some(engine) = policy_engine {
            if engine.has_policies() {
                let ctx = EvaluationContext {
                    autonomy_level: self.level,
                    fuel_cost: None,
                };
                let decision = engine.evaluate(&actor_id.to_string(), action, "*", &ctx);
                match decision {
                    PolicyDecision::Deny { .. } => {
                        // Policy explicitly denies — treat as requiring L5+
                        // which is unreachable, so it always fails.
                        effective = AutonomyLevel::L5;
                        if self.level < effective {
                            return self.require_level(actor_id, audit_trail, effective, action);
                        }
                    }
                    PolicyDecision::Allow => {
                        // Policy explicitly allows — still enforce the
                        // default minimum (policies can loosen consent
                        // tiers but not bypass autonomy minimums).
                    }
                    PolicyDecision::RequireApproval { .. } => {
                        // RequireApproval in autonomy context: bump required
                        // level by one above current default (stricter).
                        effective = match default_required {
                            AutonomyLevel::L0 => AutonomyLevel::L1,
                            AutonomyLevel::L1 => AutonomyLevel::L2,
                            AutonomyLevel::L2 => AutonomyLevel::L3,
                            AutonomyLevel::L3 => AutonomyLevel::L4,
                            AutonomyLevel::L4 => AutonomyLevel::L5,
                            AutonomyLevel::L5 | AutonomyLevel::L6 => AutonomyLevel::L6,
                        };
                    }
                }
            }
        }

        self.require_level(actor_id, audit_trail, effective, action)
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
    use std::str::FromStr;
    use uuid::Uuid;

    #[test]
    fn l6_orders_above_l5() {
        assert!(AutonomyLevel::L6 > AutonomyLevel::L5);
        assert_eq!(AutonomyLevel::from_numeric(6), Some(AutonomyLevel::L6));
        assert_eq!(AutonomyLevel::L6.previous(), AutonomyLevel::L5);
        assert_eq!(AutonomyLevel::from_str("L6").unwrap(), AutonomyLevel::L6);
    }

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

    #[test]
    fn self_evolution_requires_l4() {
        let mut guard = AutonomyGuard::new(AutonomyLevel::L3);
        let mut audit = AuditTrail::new();
        let result = guard.require_self_evolution(Uuid::new_v4(), &mut audit);
        assert!(matches!(result, Err(AutonomyError::Denied { .. })));
    }

    #[test]
    fn governance_modification_requires_l5() {
        let mut guard = AutonomyGuard::new(AutonomyLevel::L4);
        let mut audit = AuditTrail::new();
        let result = guard.require_governance_modification(Uuid::new_v4(), &mut audit);
        assert!(matches!(result, Err(AutonomyError::Denied { .. })));
    }

    #[test]
    fn sovereign_action_allowed_at_l5() {
        let mut guard = AutonomyGuard::new(AutonomyLevel::L5);
        let mut audit = AuditTrail::new();
        assert!(guard
            .require_sovereign_action(Uuid::new_v4(), &mut audit)
            .is_ok());
    }

    #[test]
    fn cognitive_modification_requires_l6() {
        let mut l5 = AutonomyGuard::new(AutonomyLevel::L5);
        let mut audit = AuditTrail::new();
        assert!(matches!(
            l5.require_cognitive_modification(Uuid::new_v4(), &mut audit),
            Err(AutonomyError::Denied { .. })
        ));

        let mut l6 = AutonomyGuard::new(AutonomyLevel::L6);
        let mut audit = AuditTrail::new();
        assert!(l6
            .require_cognitive_modification(Uuid::new_v4(), &mut audit)
            .is_ok());
    }

    #[test]
    fn multi_model_orchestration_requires_l6() {
        let mut l5 = AutonomyGuard::new(AutonomyLevel::L5);
        let mut audit = AuditTrail::new();
        assert!(matches!(
            l5.require_multi_model_orchestration(Uuid::new_v4(), &mut audit),
            Err(AutonomyError::Denied { .. })
        ));

        let mut l6 = AutonomyGuard::new(AutonomyLevel::L6);
        let mut audit = AuditTrail::new();
        assert!(l6
            .require_multi_model_orchestration(Uuid::new_v4(), &mut audit)
            .is_ok());
    }

    #[test]
    fn algorithm_selection_requires_l6() {
        let mut l5 = AutonomyGuard::new(AutonomyLevel::L5);
        let mut audit = AuditTrail::new();
        assert!(matches!(
            l5.require_algorithm_selection(Uuid::new_v4(), &mut audit),
            Err(AutonomyError::Denied { .. })
        ));

        let mut l6 = AutonomyGuard::new(AutonomyLevel::L6);
        let mut audit = AuditTrail::new();
        assert!(l6
            .require_algorithm_selection(Uuid::new_v4(), &mut audit)
            .is_ok());
    }

    #[test]
    fn ecosystem_design_requires_l6() {
        let mut l5 = AutonomyGuard::new(AutonomyLevel::L5);
        let mut audit = AuditTrail::new();
        assert!(matches!(
            l5.require_ecosystem_design(Uuid::new_v4(), &mut audit),
            Err(AutonomyError::Denied { .. })
        ));

        let mut l6 = AutonomyGuard::new(AutonomyLevel::L6);
        let mut audit = AuditTrail::new();
        assert!(l6
            .require_ecosystem_design(Uuid::new_v4(), &mut audit)
            .is_ok());
    }
}
