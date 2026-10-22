//! L6 actuators for cognitive tuning, model routing, algorithm selection,
//! ecosystem design, counterfactual analysis, and temporal planning.

use super::types::{
    ActionResult, Actuator, ActuatorContext, ActuatorError, ActuatorScope, SideEffect,
};
use crate::autonomy::AutonomyLevel;
use crate::cognitive::algorithms::{
    AdversarialArena, EvolutionEngine, SwarmCoordinator, WorldModel,
};
use crate::cognitive::types::PlannedAction;
use crate::hm::{ControlRecord, NewMemory, Parent, RecallSpec, SourceKind};
use crate::manifest::AgentManifest;
use crate::orchestration::orchestrator::Orchestrator;
use crate::orchestration::roles::canonical_pipeline_order;
use crate::time_machine::{Checkpoint, TimeMachine};
use nexus_persistence::{NexusDatabase, StateStore};

/// The refusal of an L6 action outside a unit: these actuators open no
/// database of their own (P3-HM-CONTRACT-02 §6.5; mission R7).
const NO_UNIT: &str =
    "l6 actuator: refused: it runs only as a step of a unit (P3-HM-CONTRACT-02 §6.1)";
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

fn ensure_l6(context: &ActuatorContext) -> Result<(), ActuatorError> {
    if context.autonomy_level < AutonomyLevel::L6 {
        return Err(ActuatorError::CapabilityDenied(
            "cognitive parameter modification requires L6 (Transcendent)".to_string(),
        ));
    }
    Ok(())
}

fn state_root(context: &ActuatorContext) -> Result<PathBuf, ActuatorError> {
    let root = context.working_dir.join(STATE_DIR);
    std::fs::create_dir_all(&root)
        .map_err(|e| ActuatorError::IoError(format!("create l6 state dir: {e}")))?;
    Ok(root)
}

fn checkpoint_path(context: &ActuatorContext) -> Result<PathBuf, ActuatorError> {
    Ok(state_root(context)?.join(CHECKPOINT_FILE))
}

fn append_checkpoint(
    context: &ActuatorContext,
    checkpoint: Checkpoint,
) -> Result<(), ActuatorError> {
    let path = checkpoint_path(context)?;
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

/// The source kind of an L6 memory row (§6.5): `model_mapping` rows are
/// `model_mapping`; every other row these actuators write is
/// `cognitive_param`.
fn kind_of(memory_type: &str) -> SourceKind {
    if memory_type == "model_mapping" {
        SourceKind::ModelMapping
    } else {
        SourceKind::CognitiveParam
    }
}

/// The newest eligible row of `memory_type` (route-first: a non-local unit
/// reads only rows it may disclose), with its JSON value.
fn latest_memory_json(
    scope: &ActuatorScope<'_>,
    memory_type: &str,
) -> Result<Option<(ControlRecord, Value)>, ActuatorError> {
    let Some(row) = scope
        .issuer
        .control_records(&*scope.ctx, &RecallSpec::memory_type(memory_type, 1))
        .map_err(|e| ActuatorError::IoError(format!("load {memory_type} memory: {e}")))?
        .into_iter()
        .next()
    else {
        return Ok(None);
    };
    let parsed = serde_json::from_str::<Value>(row.field("value_json").unwrap_or_default())
        .map_err(|e| ActuatorError::IoError(format!("parse {memory_type} memory json: {e}")))?;
    Ok(Some((row, parsed)))
}

/// A labelled `agent_memory` write of the unit (§6.1): the plan and the
/// row it was derived from, if any, are its parents.
fn save_labelled_memory(
    scope: &ActuatorScope<'_>,
    memory_type: &str,
    key: &str,
    value_json: &str,
    base: Option<&ControlRecord>,
) -> Result<(), ActuatorError> {
    let mut parents = vec![Parent::Derived(scope.plan)];
    if let Some(base) = base {
        parents.push(Parent::Record(base.record_ref()));
    }
    scope
        .issuer
        .write_memory(
            &*scope.ctx,
            &NewMemory {
                source_kind: kind_of(memory_type),
                memory_type,
                key,
                value_json,
                relevance_score: 1.0,
            },
            &parents,
        )
        .map(|_| ())
        .map_err(|e| ActuatorError::IoError(format!("save {memory_type} memory: {e}")))
}

fn upsert_json_memory(
    scope: &ActuatorScope<'_>,
    memory_type: &str,
    value: &Value,
    base: Option<&ControlRecord>,
) -> Result<(), ActuatorError> {
    save_labelled_memory(scope, memory_type, "active", &value.to_string(), base)
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
    append_checkpoint(context, checkpoint)
}

fn save_evolution_history(
    db: &NexusDatabase,
    context: &ActuatorContext,
    before: &Value,
    after: &Value,
    change_summary: &str,
) -> Result<(), ActuatorError> {
    db.save_evolution_history(
        &context.agent_id,
        chrono::Utc::now().timestamp_millis(),
        &before.to_string(),
        &after.to_string(),
        change_summary,
        1.0,
        1.0,
        true,
    )
    .map(|_| ())
    .map_err(|e| ActuatorError::IoError(format!("save evolution history: {e}")))
}

#[allow(clippy::too_many_arguments)]
fn triple_audit_memory_change(
    scope: &ActuatorScope<'_>,
    context: &ActuatorContext,
    memory_type: &str,
    base: Option<&ControlRecord>,
    before: Value,
    after: Value,
    change_summary: &str,
) -> Result<(), ActuatorError> {
    checkpoint_config(context, memory_type, before.clone(), after.clone())?;
    upsert_json_memory(scope, memory_type, &after, base)?;
    save_evolution_history(
        scope.issuer.state_database(),
        context,
        &before,
        &after,
        change_summary,
    )
}

fn parse_ecosystem(ecosystem_json: &str) -> Result<Vec<EcosystemAgentDefinition>, ActuatorError> {
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

fn instantiate_algorithm(algorithm: &str) -> Result<(), ActuatorError> {
    match algorithm {
        "evolutionary" => {
            // Best-effort: instantiation validates the algorithm exists; value is intentionally unused
            let _ = EvolutionEngine::default();
            Ok(())
        }
        "swarm" => {
            // Best-effort: instantiation validates the algorithm exists; value is intentionally unused
            let _ = SwarmCoordinator::default();
            Ok(())
        }
        "world_model" => {
            // Best-effort: instantiation validates the algorithm exists; value is intentionally unused
            let _ = WorldModel::default();
            Ok(())
        }
        "adversarial" => {
            // Best-effort: instantiation validates the algorithm exists; value is intentionally unused
            let _ = AdversarialArena::default();
            Ok(())
        }
        "darwin" => {
            // Best-effort: instantiation validates the algorithm exists; value is intentionally unused
            let _ = super::super::cognitive::PlanEvolutionEngine::default();
            Ok(())
        }
        other => Err(ActuatorError::IoError(format!(
            "select_algorithm: invalid algorithm '{other}'"
        ))),
    }
}

#[derive(Debug, Clone)]
pub struct CognitiveParamActuator;

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
        ensure_l6(context)?;
        match matches!(action, PlannedAction::ModifyCognitiveParams { .. }) {
            true => Err(ActuatorError::IoError(NO_UNIT.into())),
            false => Err(ActuatorError::ActionNotHandled),
        }
    }

    fn execute_in(
        &self,
        action: &PlannedAction,
        context: &ActuatorContext,
        scope: &mut ActuatorScope<'_>,
    ) -> Result<ActionResult, ActuatorError> {
        ensure_l6(context)?;
        let scope = &*scope;

        match action {
            PlannedAction::ModifyCognitiveParams {
                param_key,
                param_value,
            } => {
                validate_param(param_key, param_value)?;
                let latest = latest_memory_json(scope, "cognitive_params")?;
                let before = latest
                    .as_ref()
                    .map(|(_, v)| v.clone())
                    .unwrap_or_else(|| Value::Object(Map::new()));
                let mut after = before.clone();
                if let Some(map) = after.as_object_mut() {
                    map.insert(param_key.clone(), Value::String(param_value.clone()));
                }
                triple_audit_memory_change(
                    scope,
                    context,
                    "cognitive_params",
                    latest.as_ref().map(|(r, _)| r),
                    before,
                    after,
                    "l6.cognitive_param_override",
                )?;

                Ok(ActionResult {
                    success: true,
                    output: format!(
                        "Cognitive parameter '{param_key}' updated to '{param_value}' with checkpoint and audit trail."
                    ),
                    fuel_cost: 8.0,
                    side_effects: vec![SideEffect::FileModified {
                        path: checkpoint_path(context)?,
                    }],
                })
            }
            _ => Err(ActuatorError::ActionNotHandled),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ModelOrchestrationActuator;

impl Actuator for ModelOrchestrationActuator {
    fn name(&self) -> &str {
        "model_orchestration"
    }

    fn required_capabilities(&self) -> Vec<String> {
        vec!["self.modify".to_string()]
    }

    fn execute(
        &self,
        action: &PlannedAction,
        context: &ActuatorContext,
    ) -> Result<ActionResult, ActuatorError> {
        ensure_l6(context)?;
        match matches!(action, PlannedAction::SelectLlmProvider { .. }) {
            true => Err(ActuatorError::IoError(NO_UNIT.into())),
            false => Err(ActuatorError::ActionNotHandled),
        }
    }

    fn execute_in(
        &self,
        action: &PlannedAction,
        context: &ActuatorContext,
        scope: &mut ActuatorScope<'_>,
    ) -> Result<ActionResult, ActuatorError> {
        ensure_l6(context)?;
        let scope = &*scope;

        match action {
            PlannedAction::SelectLlmProvider {
                phase,
                provider,
                model,
            } => {
                if !SUPPORTED_PHASES
                    .iter()
                    .any(|candidate| candidate.eq_ignore_ascii_case(phase))
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
                if !provider_allowed(provider) {
                    return Err(ActuatorError::IoError(format!(
                        "select_llm_provider: unsupported provider '{provider}'"
                    )));
                }

                let latest = latest_memory_json(scope, "model_mapping")?;
                let before = latest
                    .as_ref()
                    .map(|(_, v)| v.clone())
                    .unwrap_or_else(|| Value::Object(Map::new()));
                let mut after = before.clone();
                if let Some(map) = after.as_object_mut() {
                    map.insert(
                        phase.to_ascii_lowercase(),
                        json!({
                            "provider": provider,
                            "model": model,
                        }),
                    );
                }

                triple_audit_memory_change(
                    scope,
                    context,
                    "model_mapping",
                    latest.as_ref().map(|(r, _)| r),
                    before,
                    after,
                    "l6.model_orchestration",
                )?;

                Ok(ActionResult {
                    success: true,
                    output: format!(
                        "Phase '{phase}' now uses provider '{provider}' with model '{model}'."
                    ),
                    fuel_cost: 5.0,
                    side_effects: vec![SideEffect::FileModified {
                        path: checkpoint_path(context)?,
                    }],
                })
            }
            _ => Err(ActuatorError::ActionNotHandled),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AlgorithmSelectionActuator;

impl Actuator for AlgorithmSelectionActuator {
    fn name(&self) -> &str {
        "algorithm_selection"
    }

    fn required_capabilities(&self) -> Vec<String> {
        vec!["self.modify".to_string()]
    }

    fn execute(
        &self,
        action: &PlannedAction,
        context: &ActuatorContext,
    ) -> Result<ActionResult, ActuatorError> {
        ensure_l6(context)?;
        match matches!(action, PlannedAction::SelectAlgorithm { .. }) {
            true => Err(ActuatorError::IoError(NO_UNIT.into())),
            false => Err(ActuatorError::ActionNotHandled),
        }
    }

    fn execute_in(
        &self,
        action: &PlannedAction,
        context: &ActuatorContext,
        scope: &mut ActuatorScope<'_>,
    ) -> Result<ActionResult, ActuatorError> {
        ensure_l6(context)?;
        let db = scope.issuer.state_database().clone();
        let scope = &*scope;

        match action {
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
                instantiate_algorithm(algorithm)?;

                let latest = latest_memory_json(scope, "algorithm_selection")?;
                let before = latest
                    .as_ref()
                    .map(|(_, v)| v.clone())
                    .unwrap_or_else(|| Value::Object(Map::new()));
                let after = json!({
                    "algorithm": algorithm,
                    "config": parsed_config,
                });

                triple_audit_memory_change(
                    scope,
                    context,
                    "algorithm_selection",
                    latest.as_ref().map(|(r, _)| r),
                    before,
                    after,
                    "l6.algorithm_selection",
                )?;
                db.save_algorithm_selection(
                    &context.agent_id,
                    "next-task",
                    algorithm,
                    config_json,
                    None,
                )
                .map_err(|e| ActuatorError::IoError(format!("save algorithm selection: {e}")))?;

                Ok(ActionResult {
                    success: true,
                    output: format!("Algorithm '{algorithm}' selected for the next task."),
                    fuel_cost: 4.0,
                    side_effects: vec![SideEffect::FileModified {
                        path: checkpoint_path(context)?,
                    }],
                })
            }
            _ => Err(ActuatorError::ActionNotHandled),
        }
    }
}

#[derive(Debug, Clone)]
pub struct EcosystemDesignActuator;

impl Actuator for EcosystemDesignActuator {
    fn name(&self) -> &str {
        "ecosystem_design"
    }

    fn required_capabilities(&self) -> Vec<String> {
        vec!["self.modify".to_string()]
    }

    fn execute(
        &self,
        action: &PlannedAction,
        context: &ActuatorContext,
    ) -> Result<ActionResult, ActuatorError> {
        ensure_l6(context)?;
        match matches!(action, PlannedAction::DesignAgentEcosystem { .. }) {
            true => Err(ActuatorError::IoError(NO_UNIT.into())),
            false => Err(ActuatorError::ActionNotHandled),
        }
    }

    fn execute_in(
        &self,
        action: &PlannedAction,
        context: &ActuatorContext,
        scope: &mut ActuatorScope<'_>,
    ) -> Result<ActionResult, ActuatorError> {
        ensure_l6(context)?;
        let db = scope.issuer.state_database().clone();
        let scope = &*scope;

        match action {
            PlannedAction::DesignAgentEcosystem { ecosystem_json } => {
                let definitions = parse_ecosystem(ecosystem_json)?;
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
                    if child_level == 6 {
                        return Err(ActuatorError::CapabilityDenied(
                            "design_agent_ecosystem: L6 agents require direct human authorization"
                                .to_string(),
                        ));
                    }
                    if child_level > parent_level {
                        return Err(ActuatorError::CapabilityDenied(format!(
                            "design_agent_ecosystem: sub-agent autonomy L{child_level} exceeds parent autonomy L{parent_level}"
                        )));
                    }
                }

                let latest = latest_memory_json(scope, "agent_ecosystem")?;
                let before = latest
                    .as_ref()
                    .map(|(_, v)| v.clone())
                    .unwrap_or_else(|| json!({"ecosystems": []}));

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
                    // Optional: team creation is supplementary; ecosystem agents function independently without a team
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

                let mut after = before.clone();
                if let Some(existing) = after.get_mut("ecosystems").and_then(Value::as_array_mut) {
                    existing.push(json!({
                        "ecosystem_id": ecosystem_id,
                        "agent_ids": child_ids,
                        "agent_count": definitions.len(),
                        "total_fuel": total_fuel,
                        "team_id": team_id.map(|id| id.to_string()),
                        "delegation_chain_ids": delegation_chain_ids,
                    }));
                }

                triple_audit_memory_change(
                    scope,
                    context,
                    "agent_ecosystem",
                    latest.as_ref().map(|(r, _)| r),
                    before,
                    after,
                    "l6.ecosystem_design",
                )?;

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
                    side_effects: vec![SideEffect::FileModified {
                        path: checkpoint_path(context)?,
                    }],
                })
            }
            _ => Err(ActuatorError::ActionNotHandled),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CounterfactualActuator;

impl Actuator for CounterfactualActuator {
    fn name(&self) -> &str {
        "counterfactual"
    }

    fn required_capabilities(&self) -> Vec<String> {
        vec!["self.modify".to_string()]
    }

    fn execute(
        &self,
        action: &PlannedAction,
        context: &ActuatorContext,
    ) -> Result<ActionResult, ActuatorError> {
        ensure_l6(context)?;
        match matches!(action, PlannedAction::RunCounterfactual { .. }) {
            true => Err(ActuatorError::IoError(NO_UNIT.into())),
            false => Err(ActuatorError::ActionNotHandled),
        }
    }

    fn execute_in(
        &self,
        action: &PlannedAction,
        context: &ActuatorContext,
        scope: &mut ActuatorScope<'_>,
    ) -> Result<ActionResult, ActuatorError> {
        ensure_l6(context)?;
        let scope = &*scope;

        match action {
            PlannedAction::RunCounterfactual {
                decision_id,
                alternatives,
            } => {
                let latest = latest_memory_json(scope, "world_model")?;
                let world_model = latest
                    .as_ref()
                    // Optional: deserialization failure means no prior world model; falls back to default
                    .and_then(|(_, value)| serde_json::from_value::<WorldModel>(value.clone()).ok())
                    .unwrap_or_default();
                let simulations = alternatives
                    .iter()
                    .map(|alternative| world_model.simulate_action(decision_id, alternative))
                    .collect::<Vec<_>>();
                save_labelled_memory(
                    scope,
                    "episodic",
                    &format!("counterfactual:{decision_id}"),
                    &json!({
                        "decision_id": decision_id,
                        "actual": "pending",
                        "alternatives": simulations,
                    })
                    .to_string(),
                    latest.as_ref().map(|(r, _)| r),
                )?;

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
            _ => Err(ActuatorError::ActionNotHandled),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TemporalPlanActuator;

impl Actuator for TemporalPlanActuator {
    fn name(&self) -> &str {
        "temporal_plan"
    }

    fn required_capabilities(&self) -> Vec<String> {
        vec!["self.modify".to_string()]
    }

    fn execute(
        &self,
        action: &PlannedAction,
        context: &ActuatorContext,
    ) -> Result<ActionResult, ActuatorError> {
        ensure_l6(context)?;
        match matches!(action, PlannedAction::TemporalPlan { .. }) {
            true => Err(ActuatorError::IoError(NO_UNIT.into())),
            false => Err(ActuatorError::ActionNotHandled),
        }
    }

    fn execute_in(
        &self,
        action: &PlannedAction,
        context: &ActuatorContext,
        scope: &mut ActuatorScope<'_>,
    ) -> Result<ActionResult, ActuatorError> {
        ensure_l6(context)?;
        let scope = &*scope;

        match action {
            PlannedAction::TemporalPlan {
                immediate,
                short_term,
                medium_term,
                long_term,
            } => {
                let now = chrono::Utc::now();
                let review_schedule = json!({
                    "immediate": {"goal": immediate, "review_at": now.to_rfc3339()},
                    "short_term": {"goal": short_term, "review_at": (now + chrono::Duration::hours(24)).to_rfc3339()},
                    "medium_term": {"goal": medium_term, "review_at": (now + chrono::Duration::days(7)).to_rfc3339()},
                    "long_term": {"goal": long_term, "review_at": (now + chrono::Duration::days(30)).to_rfc3339()},
                });
                save_labelled_memory(
                    scope,
                    "temporal_goals",
                    "active",
                    &review_schedule.to_string(),
                    None,
                )?;
                save_labelled_memory(scope, "temporal_goal_immediate", "active", immediate, None)?;
                save_labelled_memory(
                    scope,
                    "temporal_goal_short_term",
                    "active",
                    short_term,
                    None,
                )?;
                save_labelled_memory(
                    scope,
                    "temporal_goal_medium_term",
                    "active",
                    medium_term,
                    None,
                )?;
                save_labelled_memory(scope, "temporal_goal_long_term", "active", long_term, None)?;

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
    use crate::hm::testing::{label_source, rows, TestHm};
    use std::collections::HashSet;
    use tempfile::TempDir;

    /// Runs `action` as a step of a unit of the context's agent (registered
    /// in `hm`, whose state database is the primary one), with a plan.
    fn run_in(
        hm: &TestHm,
        actuator: &dyn Actuator,
        action: &PlannedAction,
        ctx: &ActuatorContext,
    ) -> Result<ActionResult, ActuatorError> {
        let agent = Uuid::parse_str(&ctx.agent_id).unwrap();
        hm.registry.add(agent, &ctx.agent_name);
        let mut unit = hm.goal(agent);
        let plan = unit.produce("the plan");
        let mut scope = ActuatorScope {
            issuer: &hm.issuer,
            ctx: &mut unit,
            plan: &plan,
            recalled: Vec::new(),
        };
        actuator.execute_in(action, ctx, &mut scope)
    }

    fn run(
        actuator: &dyn Actuator,
        action: &PlannedAction,
        ctx: &ActuatorContext,
    ) -> Result<ActionResult, ActuatorError> {
        run_in(&TestHm::new(), actuator, action, ctx)
    }

    fn make_context(workspace: &std::path::Path) -> ActuatorContext {
        let mut caps = HashSet::new();
        caps.insert("self.modify".to_string());
        caps.insert("cognitive_modify".to_string());
        ActuatorContext {
            agent_id: Uuid::new_v4().to_string(),
            agent_name: "cognitive-param-agent".to_string(),
            working_dir: workspace.to_path_buf(),
            autonomy_level: AutonomyLevel::L6,
            capabilities: caps,
            fuel_remaining: 200_000.0,
            egress_allowlist: vec![],
            action_review_engine: None,
            hitl_approved: false,
        }
    }

    #[test]
    fn modify_cognitive_params_rejects_invalid_key() {
        let tmp = TempDir::new().unwrap();
        let ctx = make_context(tmp.path());
        let actuator = CognitiveParamActuator;
        let err = run(
            &actuator,
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
        let err = run(
            &actuator,
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
        let hm = TestHm::new();
        let result = run_in(
            &hm,
            &actuator,
            &PlannedAction::ModifyCognitiveParams {
                param_key: "planning_depth".to_string(),
                param_value: "6".to_string(),
            },
            &ctx,
        )
        .unwrap();
        assert!(result.success);

        // In the primary state database, labelled (P3-HM-CONTRACT-02 §6.5).
        let memories = rows(
            &hm.db,
            &format!(
                "SELECT CAST(m.id AS TEXT), m.value_json FROM agent_memory m \
                 WHERE m.agent_id = '{}' AND m.memory_type = 'cognitive_params'",
                ctx.agent_id
            ),
        );
        assert!(!memories.is_empty());
        assert!(memories[0][1].contains("planning_depth"));
        assert_eq!(
            label_source(&hm.db, "agent_memory", &memories[0][0]),
            "cognitive_param"
        );

        let raw = std::fs::read_to_string(super::checkpoint_path(&ctx).unwrap()).unwrap();
        assert!(raw.contains("cognitive_params"));
    }

    #[test]
    fn select_llm_provider_rejects_invalid_phase() {
        let tmp = TempDir::new().unwrap();
        let ctx = make_context(tmp.path());
        let actuator = ModelOrchestrationActuator;
        let err = run(
            &actuator,
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
        let actuator = AlgorithmSelectionActuator;
        let err = run(
            &actuator,
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
        let actuator = EcosystemDesignActuator;
        let err = run(
            &actuator,
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
    fn design_agent_ecosystem_rejects_l6_children() {
        let tmp = TempDir::new().unwrap();
        let ctx = make_context(tmp.path());
        let actuator = EcosystemDesignActuator;
        let err = run(
            &actuator,
            &PlannedAction::DesignAgentEcosystem {
                ecosystem_json: r#"[{
                        "name":"child-a",
                        "version":"1.0.0",
                        "capabilities":["fs.read"],
                        "fuel_budget":1000,
                        "autonomy_level":6
                    }]"#
                .to_string(),
            },
            &ctx,
        )
        .unwrap_err();
        assert!(err.to_string().contains("direct human authorization"));
    }

    #[test]
    fn design_agent_ecosystem_rejects_excess_fuel() {
        let tmp = TempDir::new().unwrap();
        let mut ctx = make_context(tmp.path());
        ctx.fuel_remaining = 500.0;
        let actuator = EcosystemDesignActuator;
        let err = run(
            &actuator,
            &PlannedAction::DesignAgentEcosystem {
                ecosystem_json: r#"[{
                        "name":"child-a",
                        "version":"1.0.0",
                        "capabilities":["fs.read"],
                        "fuel_budget":600,
                        "autonomy_level":3
                    }]"#
                .to_string(),
            },
            &ctx,
        )
        .unwrap_err();
        assert!(matches!(err, ActuatorError::InsufficientFuel { .. }));
    }

    /// Mission R7: the L6 actuators open no database of their own; outside
    /// a unit every action that reads or writes memory is refused, and no
    /// database file appears in the workspace.
    #[test]
    fn l6_actuators_open_no_database_of_their_own() {
        let tmp = TempDir::new().unwrap();
        let ctx = make_context(tmp.path());
        let actions: [(&dyn Actuator, PlannedAction); 3] = [
            (
                &CognitiveParamActuator,
                PlannedAction::ModifyCognitiveParams {
                    param_key: "planning_depth".to_string(),
                    param_value: "6".to_string(),
                },
            ),
            (
                &CounterfactualActuator,
                PlannedAction::RunCounterfactual {
                    decision_id: "d1".to_string(),
                    alternatives: vec!["a".to_string()],
                },
            ),
            (
                &TemporalPlanActuator,
                PlannedAction::TemporalPlan {
                    immediate: "i".to_string(),
                    short_term: "s".to_string(),
                    medium_term: "m".to_string(),
                    long_term: "l".to_string(),
                },
            ),
        ];
        for (actuator, action) in actions {
            let err = actuator.execute(&action, &ctx).unwrap_err();
            assert!(
                err.to_string().contains("only as a step of a unit"),
                "{err}"
            );
        }
        assert!(!tmp.path().join(".nexus").join("nexus.db").exists());
    }
}
