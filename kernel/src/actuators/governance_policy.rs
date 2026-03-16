//! Governance Policy Actuator — handles L5-only governance actions.
//!
//! All changes are simulated first, written with a reversible file diff, and
//! protected by immutable system constraints.

use super::types::{ActionResult, Actuator, ActuatorContext, ActuatorError, SideEffect};
use crate::autonomy::AutonomyLevel;
use crate::cognitive::types::PlannedAction;
use crate::consent::{GovernedOperation, HitlTier};
use crate::speculative::SpeculativeEngine;
use crate::time_machine::TimeMachine;
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};
use uuid::Uuid;

const GOVERNANCE_DIR: &str = ".nexus/governance";
const POLICIES_FILE: &str = "policies.json";
const FUEL_FILE: &str = "ecosystem_fuel.json";

/// Governed actuator for system-wide governance policy modification (L5 only).
#[derive(Debug, Clone)]
pub struct GovernancePolicyActuator;

impl GovernancePolicyActuator {
    fn ensure_l5(context: &ActuatorContext) -> Result<(), ActuatorError> {
        if context.autonomy_level < AutonomyLevel::L5 {
            return Err(ActuatorError::CapabilityDenied(
                "governance_policy actions require L5 (Sovereign) autonomy".to_string(),
            ));
        }
        Ok(())
    }

    fn governance_root(context: &ActuatorContext) -> Result<PathBuf, ActuatorError> {
        let path = context.working_dir.join(GOVERNANCE_DIR);
        std::fs::create_dir_all(&path)
            .map_err(|e| ActuatorError::IoError(format!("create governance dir: {e}")))?;
        Ok(path)
    }

    fn policies_path(context: &ActuatorContext) -> Result<PathBuf, ActuatorError> {
        Ok(Self::governance_root(context)?.join(POLICIES_FILE))
    }

    fn fuel_path(context: &ActuatorContext) -> Result<PathBuf, ActuatorError> {
        Ok(Self::governance_root(context)?.join(FUEL_FILE))
    }

    fn immutable_key(policy_key: &str) -> bool {
        matches!(
            policy_key,
            "audit.chain.enabled"
                | "fuel.metering.enabled"
                | "hitl.override.other_users"
                | "kernel.code.modify"
        )
    }

    fn load_json_object(path: &Path) -> Result<Map<String, Value>, ActuatorError> {
        if !path.exists() {
            return Ok(Map::new());
        }
        let raw = std::fs::read_to_string(path)
            .map_err(|e| ActuatorError::IoError(format!("read {}: {e}", path.display())))?;
        serde_json::from_str::<Map<String, Value>>(&raw)
            .map_err(|e| ActuatorError::IoError(format!("parse {}: {e}", path.display())))
    }

    fn write_json_object(path: &Path, data: &Map<String, Value>) -> Result<(), ActuatorError> {
        let raw = serde_json::to_string_pretty(data)
            .map_err(|e| ActuatorError::IoError(format!("serialize {}: {e}", path.display())))?;
        std::fs::write(path, raw)
            .map_err(|e| ActuatorError::IoError(format!("write {}: {e}", path.display())))
    }

    fn simulate(
        context: &ActuatorContext,
        operation: GovernedOperation,
        tier: HitlTier,
        payload: &[u8],
    ) -> Result<(), ActuatorError> {
        let mut audit = crate::audit::AuditTrail::new();
        let mut speculative = SpeculativeEngine::new();
        let snapshot = speculative.fork_state(
            Uuid::parse_str(&context.agent_id).unwrap_or_else(|_| Uuid::new_v4()),
            context.fuel_remaining.max(0.0) as u64,
            context.autonomy_level,
            context.capabilities.iter().cloned().collect(),
            0,
        );
        let result = speculative.simulate(&snapshot, operation, tier, payload, &mut audit);
        if result.resource_impact.fuel_cost as f64 > context.fuel_remaining {
            return Err(ActuatorError::InsufficientFuel {
                needed: result.resource_impact.fuel_cost as f64,
                available: context.fuel_remaining,
            });
        }
        Ok(())
    }

    fn checkpoint_config_change(
        context: &ActuatorContext,
        key: &str,
        before: Value,
        after: Value,
    ) -> Result<(), ActuatorError> {
        let mut tm = TimeMachine::default();
        let mut builder = tm.begin_checkpoint("governance_policy", Some(context.agent_id.clone()));
        builder.record_config_change(key, before, after);
        let checkpoint = builder.build();
        tm.commit_checkpoint(checkpoint)
            .map_err(|e| ActuatorError::IoError(format!("commit governance checkpoint: {e}")))?;
        Ok(())
    }
}

impl Actuator for GovernancePolicyActuator {
    fn name(&self) -> &str {
        "governance_policy"
    }

    fn required_capabilities(&self) -> Vec<String> {
        vec!["self.modify".to_string()]
    }

    fn execute(
        &self,
        action: &PlannedAction,
        context: &ActuatorContext,
    ) -> Result<ActionResult, ActuatorError> {
        Self::ensure_l5(context)?;

        match action {
            PlannedAction::ModifyGovernancePolicy {
                policy_key,
                policy_value,
            } => {
                if policy_key.trim().is_empty() {
                    return Err(ActuatorError::IoError(
                        "governance_policy: policy_key cannot be empty".to_string(),
                    ));
                }
                if policy_value.trim().is_empty() {
                    return Err(ActuatorError::IoError(
                        "governance_policy: policy_value cannot be empty".to_string(),
                    ));
                }
                if Self::immutable_key(policy_key) {
                    return Err(ActuatorError::CommandBlocked(format!(
                        "governance_policy: '{}' is an immutable constraint and cannot be modified",
                        policy_key
                    )));
                }

                let payload = json!({
                    "policy_key": policy_key,
                    "policy_value": policy_value,
                });
                Self::simulate(
                    context,
                    GovernedOperation::GovernancePolicyModify,
                    HitlTier::Tier3,
                    payload.to_string().as_bytes(),
                )?;

                let path = Self::policies_path(context)?;
                let mut policies = Self::load_json_object(&path)?;
                let before = policies.get(policy_key).cloned().unwrap_or(Value::Null);
                policies.insert(policy_key.clone(), Value::String(policy_value.clone()));
                let after = policies.get(policy_key).cloned().unwrap_or(Value::Null);
                Self::checkpoint_config_change(context, policy_key, before.clone(), after.clone())?;
                Self::write_json_object(&path, &policies)?;

                Ok(ActionResult {
                    success: true,
                    output: format!(
                        "Governance policy updated with full diff and reversible checkpoint: {}: {} -> {}",
                        policy_key,
                        before,
                        after
                    ),
                    fuel_cost: 10.0,
                    side_effects: vec![SideEffect::FileModified { path }],
                })
            }
            PlannedAction::AllocateEcosystemFuel { agent_id, amount } => {
                if agent_id.trim().is_empty() {
                    return Err(ActuatorError::IoError(
                        "governance_policy: agent_id cannot be empty".to_string(),
                    ));
                }
                if *amount <= 0.0 {
                    return Err(ActuatorError::IoError(
                        "governance_policy: fuel amount must be positive".to_string(),
                    ));
                }
                if *amount > 1_000_000.0 {
                    return Err(ActuatorError::IoError(
                        "governance_policy: fuel amount exceeds maximum (1,000,000)".to_string(),
                    ));
                }

                let payload = json!({
                    "agent_id": agent_id,
                    "amount": amount,
                });
                Self::simulate(
                    context,
                    GovernedOperation::EcosystemFuelAllocate,
                    HitlTier::Tier2,
                    payload.to_string().as_bytes(),
                )?;

                let path = Self::fuel_path(context)?;
                let mut allocations = Self::load_json_object(&path)?;
                let before = allocations.get(agent_id).cloned().unwrap_or(json!(0.0));
                allocations.insert(agent_id.clone(), json!(amount));
                let after = allocations.get(agent_id).cloned().unwrap_or(json!(0.0));
                Self::checkpoint_config_change(
                    context,
                    &format!("ecosystem_fuel:{agent_id}"),
                    before.clone(),
                    after.clone(),
                )?;
                Self::write_json_object(&path, &allocations)?;

                Ok(ActionResult {
                    success: true,
                    output: format!(
                        "Ecosystem fuel allocation updated with reversible diff: {}: {} -> {}",
                        agent_id, before, after
                    ),
                    fuel_cost: 5.0,
                    side_effects: vec![SideEffect::FileModified { path }],
                })
            }
            _ => Err(ActuatorError::ActionNotHandled),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use tempfile::TempDir;

    fn make_context(workspace: &std::path::Path, level: AutonomyLevel) -> ActuatorContext {
        let mut caps = HashSet::new();
        caps.insert("self.modify".to_string());
        ActuatorContext {
            agent_id: Uuid::new_v4().to_string(),
            agent_name: "governance-policy-agent".to_string(),
            working_dir: workspace.to_path_buf(),
            autonomy_level: level,
            capabilities: caps,
            fuel_remaining: 100_000.0,
            egress_allowlist: vec![],
            action_review_engine: None,
        }
    }

    #[test]
    fn modify_governance_policy_writes_reversible_diff() {
        let tmp = TempDir::new().unwrap();
        let ctx = make_context(tmp.path(), AutonomyLevel::L5);
        let actuator = GovernancePolicyActuator;
        let result = actuator
            .execute(
                &PlannedAction::ModifyGovernancePolicy {
                    policy_key: "agent.auto_approve_threshold".to_string(),
                    policy_value: "0.95".to_string(),
                },
                &ctx,
            )
            .unwrap();
        assert!(result.success);

        let raw = std::fs::read_to_string(GovernancePolicyActuator::policies_path(&ctx).unwrap())
            .unwrap();
        assert!(raw.contains("agent.auto_approve_threshold"));
    }

    #[test]
    fn immutable_constraints_are_blocked() {
        let tmp = TempDir::new().unwrap();
        let ctx = make_context(tmp.path(), AutonomyLevel::L5);
        let actuator = GovernancePolicyActuator;
        let err = actuator
            .execute(
                &PlannedAction::ModifyGovernancePolicy {
                    policy_key: "audit.chain.enabled".to_string(),
                    policy_value: "false".to_string(),
                },
                &ctx,
            )
            .unwrap_err();
        assert!(matches!(err, ActuatorError::CommandBlocked(_)));
    }

    #[test]
    fn l4_agent_cannot_modify_governance() {
        let tmp = TempDir::new().unwrap();
        let ctx = make_context(tmp.path(), AutonomyLevel::L4);
        let actuator = GovernancePolicyActuator;
        let err = actuator
            .execute(
                &PlannedAction::AllocateEcosystemFuel {
                    agent_id: "target-agent".to_string(),
                    amount: 500.0,
                },
                &ctx,
            )
            .unwrap_err();
        assert!(matches!(err, ActuatorError::CapabilityDenied(_)));
    }
}
