//! Agent Lifecycle Actuator — handles L4+ agent creation/destruction.
//!
//! Sub-agents inherit parent governance, cannot exceed parent autonomy level,
//! and are persisted with `parent_agent_id` lineage metadata.

use super::types::{ActionResult, Actuator, ActuatorContext, ActuatorError, SideEffect};
use crate::autonomy::AutonomyLevel;
use crate::cognitive::types::PlannedAction;
use crate::manifest::AgentManifest;
use nexus_persistence::{NexusDatabase, StateStore};
use serde_json::Value;
use std::path::PathBuf;
use uuid::Uuid;

const MAX_MANIFEST_BYTES: usize = 100_000;

/// Governed actuator for agent lifecycle management (L4+).
#[derive(Debug, Clone)]
pub struct AgentLifecycleActuator;

impl AgentLifecycleActuator {
    fn ensure_l4(context: &ActuatorContext) -> Result<(), ActuatorError> {
        if context.autonomy_level < AutonomyLevel::L4 {
            return Err(ActuatorError::CapabilityDenied(
                "agent lifecycle actions require L4+ autonomy".to_string(),
            ));
        }
        Ok(())
    }

    fn db_path(context: &ActuatorContext) -> PathBuf {
        context.working_dir.join(".nexus").join("nexus.db")
    }

    fn parse_manifest(manifest_json: &str) -> Result<(AgentManifest, Value), ActuatorError> {
        let value = serde_json::from_str::<Value>(manifest_json).map_err(|_| {
            ActuatorError::IoError("agent_lifecycle: manifest_json is not valid JSON".to_string())
        })?;
        let manifest = serde_json::from_value::<AgentManifest>(value.clone()).map_err(|e| {
            ActuatorError::IoError(format!("agent_lifecycle: manifest_json is invalid: {e}"))
        })?;
        Ok((manifest, value))
    }
}

impl Actuator for AgentLifecycleActuator {
    fn name(&self) -> &str {
        "agent_lifecycle"
    }

    fn required_capabilities(&self) -> Vec<String> {
        vec!["self.modify".to_string()]
    }

    fn execute(
        &self,
        action: &PlannedAction,
        context: &ActuatorContext,
    ) -> Result<ActionResult, ActuatorError> {
        Self::ensure_l4(context)?;
        let db = NexusDatabase::open(&Self::db_path(context))
            .map_err(|e| ActuatorError::IoError(format!("open lifecycle db: {e}")))?;

        match action {
            PlannedAction::CreateSubAgent { manifest_json } => {
                if manifest_json.trim().is_empty() {
                    return Err(ActuatorError::IoError(
                        "agent_lifecycle: manifest_json cannot be empty".to_string(),
                    ));
                }
                if manifest_json.len() > MAX_MANIFEST_BYTES {
                    return Err(ActuatorError::BodyTooLarge {
                        size: manifest_json.len() as u64,
                        max: MAX_MANIFEST_BYTES as u64,
                    });
                }

                let (manifest, mut value) = Self::parse_manifest(manifest_json)?;
                let requested = manifest
                    .autonomy_level
                    .unwrap_or(context.autonomy_level as u8);
                let parent_level = context.autonomy_level as u8;
                if requested > parent_level {
                    return Err(ActuatorError::CapabilityDenied(format!(
                        "sub-agent autonomy L{requested} exceeds parent autonomy L{parent_level}"
                    )));
                }

                if value.get("autonomy_level").is_none() {
                    value["autonomy_level"] = Value::from(parent_level);
                }

                let child_id = Uuid::new_v4().to_string();
                let normalized_manifest = serde_json::to_string_pretty(&value).map_err(|e| {
                    ActuatorError::IoError(format!("serialize normalized sub-agent manifest: {e}"))
                })?;

                db.save_agent_with_parent(
                    &child_id,
                    &normalized_manifest,
                    "created",
                    requested,
                    "native",
                    Some(&context.agent_id),
                )
                .map_err(|e| ActuatorError::IoError(format!("persist sub-agent: {e}")))?;

                Ok(ActionResult {
                    success: true,
                    output: format!(
                        "Created sub-agent '{}' ({}) under parent {} with inherited governance and parent-funded fuel.",
                        manifest.name, child_id, context.agent_id
                    ),
                    fuel_cost: 20.0,
                    side_effects: vec![SideEffect::MessageSent {
                        target: format!("sub-agent:{child_id}"),
                    }],
                })
            }
            PlannedAction::DestroySubAgent { agent_id } => {
                if agent_id.trim().is_empty() {
                    return Err(ActuatorError::IoError(
                        "agent_lifecycle: agent_id cannot be empty".to_string(),
                    ));
                }

                let row = db
                    .load_agent(agent_id)
                    .map_err(|e| ActuatorError::IoError(format!("load sub-agent: {e}")))?
                    .ok_or_else(|| {
                        ActuatorError::IoError(format!(
                            "agent_lifecycle: sub-agent '{}' not found",
                            agent_id
                        ))
                    })?;
                if row.parent_agent_id.as_deref() != Some(context.agent_id.as_str()) {
                    return Err(ActuatorError::CapabilityDenied(format!(
                        "sub-agent '{}' is not owned by parent {}",
                        agent_id, context.agent_id
                    )));
                }

                db.delete_agent(agent_id)
                    .map_err(|e| ActuatorError::IoError(format!("delete sub-agent: {e}")))?;

                Ok(ActionResult {
                    success: true,
                    output: format!(
                        "Destroyed sub-agent '{}' owned by parent {}.",
                        agent_id, context.agent_id
                    ),
                    fuel_cost: 5.0,
                    side_effects: vec![SideEffect::MessageSent {
                        target: format!("sub-agent:{agent_id}"),
                    }],
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

    fn make_l4_context(workspace: &std::path::Path) -> ActuatorContext {
        let mut caps = HashSet::new();
        caps.insert("self.modify".to_string());
        ActuatorContext {
            agent_id: "parent-agent".to_string(),
            working_dir: workspace.to_path_buf(),
            autonomy_level: AutonomyLevel::L4,
            capabilities: caps,
            fuel_remaining: 10_000.0,
            egress_allowlist: vec![],
        }
    }

    #[test]
    fn create_sub_agent_persists_parent_lineage() {
        let tmp = TempDir::new().unwrap();
        let ctx = make_l4_context(tmp.path());
        let actuator = AgentLifecycleActuator;

        let result = actuator
            .execute(
                &PlannedAction::CreateSubAgent {
                    manifest_json: r#"{
                        "name":"sub-agent",
                        "version":"1.0.0",
                        "capabilities":["fs.read"],
                        "fuel_budget":1000,
                        "autonomy_level":4
                    }"#
                    .to_string(),
                },
                &ctx,
            )
            .unwrap();
        assert!(result.success);

        let db = NexusDatabase::open(&AgentLifecycleActuator::db_path(&ctx)).unwrap();
        let agents = db.list_agents().unwrap();
        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0].parent_agent_id.as_deref(), Some("parent-agent"));
    }

    #[test]
    fn create_sub_agent_above_parent_autonomy_is_rejected() {
        let tmp = TempDir::new().unwrap();
        let ctx = make_l4_context(tmp.path());
        let actuator = AgentLifecycleActuator;
        let err = actuator
            .execute(
                &PlannedAction::CreateSubAgent {
                    manifest_json: r#"{
                        "name":"sub-agent",
                        "version":"1.0.0",
                        "capabilities":["fs.read"],
                        "fuel_budget":1000,
                        "autonomy_level":5
                    }"#
                    .to_string(),
                },
                &ctx,
            )
            .unwrap_err();
        assert!(matches!(err, ActuatorError::CapabilityDenied(_)));
    }

    #[test]
    fn destroy_sub_agent_requires_parent_ownership() {
        let tmp = TempDir::new().unwrap();
        let ctx = make_l4_context(tmp.path());
        let actuator = AgentLifecycleActuator;
        let db = NexusDatabase::open(&AgentLifecycleActuator::db_path(&ctx)).unwrap();
        db.save_agent_with_parent(
            "child-1",
            "{}",
            "created",
            2,
            "native",
            Some("other-parent"),
        )
        .unwrap();

        let err = actuator
            .execute(
                &PlannedAction::DestroySubAgent {
                    agent_id: "child-1".to_string(),
                },
                &ctx,
            )
            .unwrap_err();
        assert!(matches!(err, ActuatorError::CapabilityDenied(_)));
    }
}
