//! Cognitive parameter actuator — L6-only cognitive self-modification.

use super::types::{ActionResult, Actuator, ActuatorContext, ActuatorError, SideEffect};
use crate::autonomy::AutonomyLevel;
use crate::cognitive::algorithms::WorldModel;
use crate::cognitive::types::PlannedAction;
use crate::manifest::AgentManifest;
use crate::orchestration::orchestrator::Orchestrator;
use crate::orchestration::roles::canonical_pipeline_order;
use crate::time_machine::{Checkpoint, TimeMachine};
use nexus_persistence::{NexusDatabase, StateStore};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::path::PathBuf;
use uuid::Uuid;

const STATE_DIR: &str = ".nexus/l6";
const CHECKPOINT_FILE: &str = "checkpoints.json";
const SUPPORTED_PHASES: &[&str] = &["perceive", "reason", "plan", "act", "reflect", "learn"];
const SUPPORTED_ALGORITHMS: &[&str] = &["evolutionary", "swarm", "world_model", "adversarial"];
const SUPPORTED_PROVIDERS: &[&str] = &[
    "anthropic",
    "claude",
    "cohere",
    "fireworks",
    "gemini",
    "groq",
    "mistral",
    "mock",
    "ollama",
    "openai",
    "openrouter",
    "perplexity",
    "together",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CheckpointLedger {
    checkpoints: Vec<Checkpoint>,
}

#[derive(Debug, Clone, Deserialize)]
struct EcosystemAgentDefinition {
    #[serde(flatten)]
    manifest: AgentManifest,
}

#[derive(Debug, Clone)]
pub struct CognitiveParamActuator;

impl CognitiveParamActuator {
    fn ensure_l6(context: &ActuatorContext) -> Result<(), ActuatorError> {
        if context.autonomy_level < AutonomyLevel::L6 {
            return Err(ActuatorError::CapabilityDenied(
                "cognitive parameter modification requires L6 (Transcendent)".to_string(),
            ));
        }
        Ok(())
    }

    fn db_path(context: &ActuatorContext) -> PathBuf {
        context.working_dir.join(".nexus").join("nexus.db")
    }

    fn state_root(context: &ActuatorContext) -> Result<PathBuf, ActuatorError> {
        let root = context.working_dir.join(STATE_DIR);
        std::fs::create_dir_all(&root)
            .map_err(|e| ActuatorError::IoError(format!("create l6 state dir: {e}")))?;
        Ok(root)
    }

    fn checkpoint_path(context: &ActuatorContext) -> Result<PathBuf, ActuatorError> {
        Ok(Self::state_root(context)?.join(CHECKPOINT_FILE))
    }

    fn append_checkpoint(
        context: &ActuatorContext,
        checkpoint: Checkpoint,
    ) -> Result<(), ActuatorError> {
        let path = Self::checkpoint_path(context)?;
        let mut ledger = if path.exists() {
            let raw = std::fs::read_to_string(&path)
                .map_err(|e| ActuatorError::IoError(format!("read l6 checkpoint ledger: {e}")))?;
            serde_json::from_str::<CheckpointLedger>(&raw)
                .map_err(|e| ActuatorError::IoError(format!("parse l6 checkpoint ledger: {e}")))?
        } else {
            CheckpointLedger {
                checkpoints: Vec::new(),
            }
        };
        ledger.checkpoints.push(checkpoint);
        let raw = serde_json::to_string_pretty(&ledger)
            .map_err(|e| ActuatorError::IoError(format!("serialize l6 checkpoint ledger: {e}")))?;
        std::fs::write(path, raw)
            .map_err(|e| ActuatorError::IoError(format!("write l6 checkpoint ledger: {e}")))
    }

    fn db(context: &ActuatorContext) -> Result<NexusDatabase, ActuatorError> {
        NexusDatabase::open(&Self::db_path(context))
            .map_err(|e| ActuatorError::IoError(format!("open l6 db: {e}")))
    }

    fn latest_memory_json(
        db: &NexusDatabase,
        agent_id: &str,
        memory_type: &str,
    ) -> Result<Option<(i64, Value)>, ActuatorError> {
        let mut rows = db
            .load_memories(agent_id, Some(memory_type), 100)
            .map_err(|e| ActuatorError::IoError(format!("load {memory_type} memory: {e}")))?;
        rows.sort_by_key(|row| row.id);
        let Some(row) = rows.last() else {
            return Ok(None);
        };
        let parsed = serde_json::from_str::<Value>(&row.value_json)
            .map_err(|e| ActuatorError::IoError(format!("parse {memory_type} memory json: {e}")))?;
        Ok(Some((row.id, parsed)))
    }

    fn upsert_json_memory(
        db: &NexusDatabase,
        agent_id: &str,
        memory_type: &str,
        value: &Value,
    ) -> Result<(), ActuatorError> {
        db.save_memory(agent_id, memory_type, "active", &value.to_string())
            .map_err(|e| ActuatorError::IoError(format!("save {memory_type} memory: {e}")))
    }

    fn validate_param(param_key: &str, param_value: &str) -> Result<f64, ActuatorError> {
        let parsed = param_value.parse::<f64>().map_err(|_| {
            ActuatorError::IoError(format!(
                "cognitive_param: '{param_key}' value '{param_value}' is not numeric"
            ))
        })?;
        let valid = match param_key {
            "reflection_interval" => (1.0..=20.0).contains(&parsed),
            "max_cycles" => (1.0..=500.0).contains(&parsed),
            "cycle_delay_ms" => parsed >= 100.0,
            "fuel_reserve_threshold" => (0.01..=0.5).contains(&parsed),
            "planning_depth" => (1.0..=10.0).contains(&parsed),
            _ => {
                return Err(ActuatorError::IoError(format!(
                    "cognitive_param: invalid param_key '{param_key}'"
                )));
            }
        };
        if !valid {
            return Err(ActuatorError::IoError(format!(
                "cognitive_param: value '{param_value}' out of bounds for '{param_key}'"
            )));
        }
        Ok(parsed)
    }

    fn checkpoint_config(
        context: &ActuatorContext,
        key: &str,
        before: Value,
        after: Value,
    ) -> Result<(), ActuatorError> {
        let mut tm = TimeMachine::default();
        let mut builder = tm.begin_checkpoint("l6_cognitive", Some(context.agent_id.clone()));
        builder.record_config_change(key, before, after);
        let checkpoint = builder.build();
        tm.commit_checkpoint(checkpoint.clone())
            .map_err(|e| ActuatorError::IoError(format!("commit l6 checkpoint: {e}")))?;
        Self::append_checkpoint(context, checkpoint)
    }

    fn parse_ecosystem(
        ecosystem_json: &str,
    ) -> Result<Vec<EcosystemAgentDefinition>, ActuatorError> {
        serde_json::from_str::<Vec<EcosystemAgentDefinition>>(ecosystem_json).map_err(|e| {
            ActuatorError::IoError(format!(
                "design_agent_ecosystem: invalid ecosystem_json: {e}"
            ))
        })
    }

    fn provider_allowed(provider: &str) -> bool {
        SUPPORTED_PROVIDERS
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(provider))
    }
}

impl Actuator for CognitiveParamActuator {
    fn name(&self) -> &str {
        "cognitive_param"
    }

    fn required_capabilities(&self) -> Vec<String> {
        vec!["self.modify".to_string()]
    }

    fn execute(
        &self,
        action: &PlannedAction,
        context: &ActuatorContext,
    ) -> Result<ActionResult, ActuatorError> {
        Self::ensure_l6(context)?;
        let db = Self::db(context)?;

        match action {
            PlannedAction::ModifyCognitiveParams {
                param_key,
                param_value,
            } => {
                Self::validate_param(param_key, param_value)?;
                let before = Self::latest_memory_json(&db, &context.agent_id, "cognitive_params")?
                    .map(|(_, v)| v)
                    .unwrap_or_else(|| Value::Object(Map::new()));
                let mut after = before.clone();
                if let Some(map) = after.as_object_mut() {
                    map.insert(param_key.clone(), Value::String(param_value.clone()));
                }
                Self::checkpoint_config(
                    context,
                    "cognitive_params",
                    before.clone(),
                    after.clone(),
                )?;
                Self::upsert_json_memory(&db, &context.agent_id, "cognitive_params", &after)?;
                db.save_evolution_history(
                    &context.agent_id,
                    chrono::Utc::now().timestamp_millis(),
                    &before.to_string(),
                    &after.to_string(),
                    "l6.cognitive_param_override",
                    1.0,
                    1.0,
                    true,
                )
                .map_err(|e| ActuatorError::IoError(format!("save evolution history: {e}")))?;

                Ok(ActionResult {
                    success: true,
                    output: format!(
                        "Cognitive parameter '{param_key}' updated to '{param_value}' with checkpoint and audit trail."
                    ),
                    fuel_cost: 8.0,
                    side_effects: vec![SideEffect::FileModified {
                        path: Self::checkpoint_path(context)?,
                    }],
                })
            }
            PlannedAction::SelectLlmProvider {
                phase,
                provider,
                model,
            } => {
                if !SUPPORTED_PHASES
                    .iter()
                    .any(|p| p.eq_ignore_ascii_case(phase))
                {
                    return Err(ActuatorError::IoError(format!(
                        "select_llm_provider: invalid phase '{phase}'"
                    )));
                }
                if provider.trim().is_empty() || model.trim().is_empty() {
                    return Err(ActuatorError::IoError(
                        "select_llm_provider: provider and model must be non-empty".to_string(),
                    ));
                }
                if !Self::provider_allowed(provider) {
                    return Err(ActuatorError::IoError(format!(
                        "select_llm_provider: unsupported provider '{provider}'"
                    )));
                }

                let mut mapping =
                    Self::latest_memory_json(&db, &context.agent_id, "model_mapping")?
                        .map(|(_, v)| v)
                        .unwrap_or_else(|| Value::Object(Map::new()));
                if let Some(map) = mapping.as_object_mut() {
                    map.insert(
                        phase.to_ascii_lowercase(),
                        json!({
                            "provider": provider,
                            "model": model,
                        }),
                    );
                }
                Self::upsert_json_memory(&db, &context.agent_id, "model_mapping", &mapping)?;

                Ok(ActionResult {
                    success: true,
                    output: format!(
                        "Phase '{phase}' now uses provider '{provider}' with model '{model}'."
                    ),
                    fuel_cost: 5.0,
                    side_effects: vec![],
                })
            }
            PlannedAction::SelectAlgorithm {
                algorithm,
                config_json,
            } => {
                if !SUPPORTED_ALGORITHMS
                    .iter()
                    .any(|candidate| candidate == algorithm)
                {
                    return Err(ActuatorError::IoError(format!(
                        "select_algorithm: invalid algorithm '{algorithm}'"
                    )));
                }
                let parsed_config = serde_json::from_str::<Value>(config_json).map_err(|e| {
                    ActuatorError::IoError(format!("select_algorithm: invalid config_json: {e}"))
                })?;
                let value = json!({
                    "algorithm": algorithm,
                    "config": parsed_config,
                });
                Self::upsert_json_memory(&db, &context.agent_id, "algorithm_selection", &value)?;
                let _ = db.save_algorithm_selection(
                    &context.agent_id,
                    "next-task",
                    algorithm,
                    config_json,
                    None,
                );

                Ok(ActionResult {
                    success: true,
                    output: format!("Algorithm '{algorithm}' selected for the next task."),
                    fuel_cost: 4.0,
                    side_effects: vec![],
                })
            }
            PlannedAction::DesignAgentEcosystem { ecosystem_json } => {
                let definitions = Self::parse_ecosystem(ecosystem_json)?;
                let parent_level = context.autonomy_level as u8;
                let total_fuel: u64 = definitions.iter().map(|def| def.manifest.fuel_budget).sum();
                if total_fuel as f64 > context.fuel_remaining {
                    return Err(ActuatorError::InsufficientFuel {
                        needed: total_fuel as f64,
                        available: context.fuel_remaining,
                    });
                }
                for def in &definitions {
                    let child_level = def.manifest.autonomy_level.unwrap_or(parent_level);
                    if child_level > parent_level {
                        return Err(ActuatorError::CapabilityDenied(format!(
                            "design_agent_ecosystem: sub-agent autonomy L{child_level} exceeds parent autonomy L{parent_level}"
                        )));
                    }
                }

                let ecosystem_id = Uuid::new_v4().to_string();
                let mut child_ids = Vec::new();
                for def in &definitions {
                    let child_id = Uuid::new_v4().to_string();
                    let manifest_json =
                        serde_json::to_string_pretty(&def.manifest).map_err(|e| {
                            ActuatorError::IoError(format!("serialize child manifest: {e}"))
                        })?;
                    db.save_agent_with_parent(
                        &child_id,
                        &manifest_json,
                        "created",
                        def.manifest.autonomy_level.unwrap_or(parent_level),
                        "native",
                        Some(&context.agent_id),
                    )
                    .map_err(|e| ActuatorError::IoError(format!("persist ecosystem child: {e}")))?;
                    child_ids.push(child_id);
                }

                let mut orchestrator = Orchestrator::with_autonomy_level(context.autonomy_level);
                let roles = canonical_pipeline_order()
                    .into_iter()
                    .take(definitions.len().min(5))
                    .collect::<Vec<_>>();
                let team_id = if roles.is_empty() {
                    None
                } else {
                    orchestrator.create_team(&roles).ok()
                };

                let mut delegation_engine = crate::delegation::DelegationEngine::new();
                let creator_uuid =
                    Uuid::parse_str(&context.agent_id).unwrap_or_else(|_| Uuid::new_v4());
                let mut creator_caps = context.capabilities.iter().cloned().collect::<Vec<_>>();
                for def in &definitions {
                    for capability in &def.manifest.capabilities {
                        if !creator_caps.contains(capability) {
                            creator_caps.push(capability.clone());
                        }
                    }
                }
                delegation_engine.register_agent(creator_uuid, creator_caps);
                let mut delegation_chain_ids = Vec::new();
                for (child_id, def) in child_ids.iter().zip(definitions.iter()) {
                    let child_uuid = Uuid::parse_str(child_id).unwrap_or_else(|_| Uuid::new_v4());
                    delegation_engine.register_agent(child_uuid, def.manifest.capabilities.clone());
                    let grant = delegation_engine
                        .delegate(
                            creator_uuid,
                            child_uuid,
                            def.manifest.capabilities.clone(),
                            crate::delegation::DelegationConstraints {
                                max_fuel: def.manifest.fuel_budget,
                                ..Default::default()
                            },
                        )
                        .map_err(|e| {
                            ActuatorError::IoError(format!("delegate ecosystem capability: {e}"))
                        })?;
                    delegation_chain_ids.push(grant.id.to_string());
                }

                db.save_agent_ecosystem(
                    &ecosystem_id,
                    &context.agent_id,
                    ecosystem_json,
                    definitions.len() as i64,
                    total_fuel as f64,
                    "active",
                )
                .map_err(|e| ActuatorError::IoError(format!("persist ecosystem: {e}")))?;

                Ok(ActionResult {
                    success: true,
                    output: json!({
                        "ecosystem_id": ecosystem_id,
                        "agent_ids": child_ids,
                        "team_id": team_id.map(|id| id.to_string()),
                        "delegation_chain_ids": delegation_chain_ids,
                    })
                    .to_string(),
                    fuel_cost: 20.0,
                    side_effects: vec![],
                })
            }
            PlannedAction::RunCounterfactual {
                decision_id,
                alternatives,
            } => {
                let world_model = Self::latest_memory_json(&db, &context.agent_id, "world_model")?
                    .and_then(|(_, value)| serde_json::from_value::<WorldModel>(value).ok())
                    .unwrap_or_default();
                let simulations = alternatives
                    .iter()
                    .map(|alternative| world_model.simulate_action(decision_id, alternative))
                    .collect::<Vec<_>>();
                db.save_memory(
                    &context.agent_id,
                    "episodic",
                    &format!("counterfactual:{decision_id}"),
                    &json!({
                        "decision_id": decision_id,
                        "actual": "pending",
                        "alternatives": simulations,
                    })
                    .to_string(),
                )
                .map_err(|e| ActuatorError::IoError(format!("save counterfactual memory: {e}")))?;

                Ok(ActionResult {
                    success: true,
                    output: format!(
                        "Stored {} counterfactual outcomes for decision '{}'.",
                        alternatives.len(),
                        decision_id
                    ),
                    fuel_cost: 6.0,
                    side_effects: vec![],
                })
            }
            PlannedAction::TemporalPlan {
                immediate,
                short_term,
                medium_term,
                long_term,
            } => {
                let now = chrono::Utc::now();
                let review_schedule = json!({
                    "immediate": {"goal": immediate, "review_at": now.to_rfc3339()},
                    "short_term": {"goal": short_term, "review_at": (now + chrono::Duration::days(7)).to_rfc3339()},
                    "medium_term": {"goal": medium_term, "review_at": (now + chrono::Duration::days(30)).to_rfc3339()},
                    "long_term": {"goal": long_term, "review_at": (now + chrono::Duration::days(90)).to_rfc3339()},
                });
                db.save_memory(
                    &context.agent_id,
                    "temporal_goals",
                    "active",
                    &review_schedule.to_string(),
                )
                .map_err(|e| ActuatorError::IoError(format!("save temporal plan: {e}")))?;

                Ok(ActionResult {
                    success: true,
                    output: "Temporal plan stored across immediate, short-term, medium-term, and long-term horizons.".to_string(),
                    fuel_cost: 4.0,
                    side_effects: vec![],
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

    fn make_context(workspace: &std::path::Path) -> ActuatorContext {
        let mut caps = HashSet::new();
        caps.insert("self.modify".to_string());
        caps.insert("cognitive_modify".to_string());
        ActuatorContext {
            agent_id: Uuid::new_v4().to_string(),
            working_dir: workspace.to_path_buf(),
            autonomy_level: AutonomyLevel::L6,
            capabilities: caps,
            fuel_remaining: 200_000.0,
            egress_allowlist: vec![],
        }
    }

    #[test]
    fn modify_cognitive_params_rejects_invalid_key() {
        let tmp = TempDir::new().unwrap();
        let ctx = make_context(tmp.path());
        let actuator = CognitiveParamActuator;
        let err = actuator
            .execute(
                &PlannedAction::ModifyCognitiveParams {
                    param_key: "bad_key".to_string(),
                    param_value: "3".to_string(),
                },
                &ctx,
            )
            .unwrap_err();
        assert!(err.to_string().contains("invalid param_key"));
    }

    #[test]
    fn modify_cognitive_params_rejects_out_of_bounds_value() {
        let tmp = TempDir::new().unwrap();
        let ctx = make_context(tmp.path());
        let actuator = CognitiveParamActuator;
        let err = actuator
            .execute(
                &PlannedAction::ModifyCognitiveParams {
                    param_key: "max_cycles".to_string(),
                    param_value: "501".to_string(),
                },
                &ctx,
            )
            .unwrap_err();
        assert!(err.to_string().contains("out of bounds"));
    }

    #[test]
    fn modify_cognitive_params_persists_memory_and_checkpoint() {
        let tmp = TempDir::new().unwrap();
        let ctx = make_context(tmp.path());
        let actuator = CognitiveParamActuator;
        let result = actuator
            .execute(
                &PlannedAction::ModifyCognitiveParams {
                    param_key: "planning_depth".to_string(),
                    param_value: "6".to_string(),
                },
                &ctx,
            )
            .unwrap();
        assert!(result.success);

        let db = CognitiveParamActuator::db(&ctx).unwrap();
        let memories = db
            .load_memories(&ctx.agent_id, Some("cognitive_params"), 10)
            .unwrap();
        assert!(!memories.is_empty());
        assert!(memories[0].value_json.contains("planning_depth"));

        let checkpoint_path = CognitiveParamActuator::checkpoint_path(&ctx).unwrap();
        let raw = std::fs::read_to_string(checkpoint_path).unwrap();
        assert!(raw.contains("cognitive_params"));
    }

    #[test]
    fn select_llm_provider_rejects_invalid_phase() {
        let tmp = TempDir::new().unwrap();
        let ctx = make_context(tmp.path());
        let actuator = CognitiveParamActuator;
        let err = actuator
            .execute(
                &PlannedAction::SelectLlmProvider {
                    phase: "unknown".to_string(),
                    provider: "mock".to_string(),
                    model: "mock-model".to_string(),
                },
                &ctx,
            )
            .unwrap_err();
        assert!(err.to_string().contains("invalid phase"));
    }

    #[test]
    fn select_algorithm_rejects_invalid_name() {
        let tmp = TempDir::new().unwrap();
        let ctx = make_context(tmp.path());
        let actuator = CognitiveParamActuator;
        let err = actuator
            .execute(
                &PlannedAction::SelectAlgorithm {
                    algorithm: "bad".to_string(),
                    config_json: "{}".to_string(),
                },
                &ctx,
            )
            .unwrap_err();
        assert!(err.to_string().contains("invalid algorithm"));
    }

    #[test]
    fn design_agent_ecosystem_rejects_sub_agent_above_parent() {
        let tmp = TempDir::new().unwrap();
        let ctx = make_context(tmp.path());
        let actuator = CognitiveParamActuator;
        let err = actuator
            .execute(
                &PlannedAction::DesignAgentEcosystem {
                    ecosystem_json: r#"[{
                        "name":"child-a",
                        "version":"1.0.0",
                        "capabilities":["fs.read"],
                        "fuel_budget":1000,
                        "autonomy_level":7
                    }]"#
                    .to_string(),
                },
                &ctx,
            )
            .unwrap_err();
        assert!(err.to_string().contains("exceeds parent autonomy"));
    }

    #[test]
    fn design_agent_ecosystem_rejects_excess_fuel() {
        let tmp = TempDir::new().unwrap();
        let mut ctx = make_context(tmp.path());
        ctx.fuel_remaining = 500.0;
        let actuator = CognitiveParamActuator;
        let err = actuator
            .execute(
                &PlannedAction::DesignAgentEcosystem {
                    ecosystem_json: r#"[{
                        "name":"child-a",
                        "version":"1.0.0",
                        "capabilities":["fs.read"],
                        "fuel_budget":600,
                        "autonomy_level":6
                    }]"#
                    .to_string(),
                },
                &ctx,
            )
            .unwrap_err();
        assert!(matches!(err, ActuatorError::InsufficientFuel { .. }));
    }
}
