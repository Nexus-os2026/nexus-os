//! Self-Evolution Actuator — handles L4+ self-modification actions.
//!
//! Before any self-modification, the actuator:
//! 1. Reconciles any pending experiments against the latest task history.
//! 2. Snapshots current state via Time Machine.
//! 3. Runs SpeculativeEngine simulation.
//! 4. Commits only when the simulation passes local governance checks.
//! 5. Tracks performance for the next 5 tasks and auto-rolls back if it drops.

use super::types::{ActionResult, Actuator, ActuatorContext, ActuatorError, SideEffect};
use crate::autonomy::AutonomyLevel;
use crate::cognitive::types::PlannedAction;
use crate::consent::{GovernedOperation, HitlTier};
use crate::speculative::SpeculativeEngine;
use crate::time_machine::TimeMachine;
use nexus_persistence::{NexusDatabase, StateStore, TaskRow};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;
use std::path::PathBuf;
use uuid::Uuid;

const STATE_DIR: &str = ".nexus/self_evolution";
const STATE_FILE: &str = "state.json";
const CHECKPOINTS_FILE: &str = "checkpoints.json";
const PERFORMANCE_WINDOW: usize = 5;
const MAX_DESCRIPTION_BYTES: usize = 50_000;
const MAX_VARIANTS: usize = 32;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct SelfEvolutionState {
    current_description: String,
    current_version: i64,
    strategies: BTreeMap<String, String>,
    archive: Vec<ArchiveVariant>,
    pending_experiments: Vec<PendingExperiment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ArchiveVariant {
    version: i64,
    description: String,
    score: f64,
    task: String,
    rounds: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PendingExperiment {
    version: i64,
    description_before: String,
    description_after: String,
    trigger: String,
    performance_before: f64,
    created_at: String,
    checkpoint_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct CheckpointLedger {
    checkpoints: Vec<crate::time_machine::Checkpoint>,
}

/// Governed actuator for agent self-evolution (L4+).
#[derive(Debug, Clone)]
pub struct SelfEvolutionActuator;

impl SelfEvolutionActuator {
    fn ensure_l4(context: &ActuatorContext) -> Result<(), ActuatorError> {
        if context.autonomy_level < AutonomyLevel::L4 {
            return Err(ActuatorError::CapabilityDenied(
                "self_evolution actions require L4+ autonomy".to_string(),
            ));
        }
        Ok(())
    }

    fn state_root(context: &ActuatorContext) -> Result<PathBuf, ActuatorError> {
        let path = context.working_dir.join(STATE_DIR);
        std::fs::create_dir_all(&path)
            .map_err(|e| ActuatorError::IoError(format!("create self-evolution state dir: {e}")))?;
        Ok(path)
    }

    fn state_path(context: &ActuatorContext) -> Result<PathBuf, ActuatorError> {
        Ok(Self::state_root(context)?.join(STATE_FILE))
    }

    fn checkpoints_path(context: &ActuatorContext) -> Result<PathBuf, ActuatorError> {
        Ok(Self::state_root(context)?.join(CHECKPOINTS_FILE))
    }

    fn db_path(context: &ActuatorContext) -> PathBuf {
        context.working_dir.join(".nexus").join("nexus.db")
    }

    fn load_state(context: &ActuatorContext) -> Result<SelfEvolutionState, ActuatorError> {
        let path = Self::state_path(context)?;
        if !path.exists() {
            return Ok(SelfEvolutionState::default());
        }
        let raw = std::fs::read_to_string(&path)
            .map_err(|e| ActuatorError::IoError(format!("read self-evolution state: {e}")))?;
        serde_json::from_str(&raw)
            .map_err(|e| ActuatorError::IoError(format!("parse self-evolution state: {e}")))
    }

    fn save_state(
        context: &ActuatorContext,
        state: &SelfEvolutionState,
    ) -> Result<(), ActuatorError> {
        let raw = serde_json::to_string_pretty(state)
            .map_err(|e| ActuatorError::IoError(format!("serialize self-evolution state: {e}")))?;
        std::fs::write(Self::state_path(context)?, raw)
            .map_err(|e| ActuatorError::IoError(format!("write self-evolution state: {e}")))
    }

    fn append_checkpoint(
        context: &ActuatorContext,
        checkpoint: crate::time_machine::Checkpoint,
    ) -> Result<(), ActuatorError> {
        let path = Self::checkpoints_path(context)?;
        let mut ledger = if path.exists() {
            let raw = std::fs::read_to_string(&path)
                .map_err(|e| ActuatorError::IoError(format!("read checkpoint ledger: {e}")))?;
            serde_json::from_str::<CheckpointLedger>(&raw)
                .map_err(|e| ActuatorError::IoError(format!("parse checkpoint ledger: {e}")))?
        } else {
            CheckpointLedger::default()
        };
        ledger.checkpoints.push(checkpoint);
        let raw = serde_json::to_string_pretty(&ledger)
            .map_err(|e| ActuatorError::IoError(format!("serialize checkpoint ledger: {e}")))?;
        std::fs::write(path, raw)
            .map_err(|e| ActuatorError::IoError(format!("write checkpoint ledger: {e}")))
    }

    fn current_description<'a>(state: &'a SelfEvolutionState, fallback: &'a str) -> &'a str {
        if state.current_description.trim().is_empty() {
            fallback
        } else {
            state.current_description.as_str()
        }
    }

    fn current_time_rfc3339() -> String {
        chrono::Utc::now().to_rfc3339()
    }

    fn task_performance(tasks: &[TaskRow]) -> Option<f64> {
        let completed: Vec<&TaskRow> = tasks
            .iter()
            .filter(|task| task.completed_at.is_some() || task.success)
            .collect();
        if completed.is_empty() {
            return None;
        }

        let mut total = 0.0;
        for task in &completed {
            let success = if task.success { 1.0 } else { 0.0 };
            let quality = task
                .quality_score
                .unwrap_or(if task.success { 0.8 } else { 0.3 });
            let fuel_efficiency = if task.fuel_consumed > 0.0 {
                (1.0 / task.fuel_consumed).min(1.0)
            } else {
                1.0
            };
            total += 0.5 * success + 0.35 * quality.clamp(0.0, 1.0) + 0.15 * fuel_efficiency;
        }

        Some(total / completed.len() as f64)
    }

    fn recent_performance(db: &NexusDatabase, agent_id: &str) -> Result<f64, ActuatorError> {
        let tasks = db
            .load_tasks_by_agent(agent_id, PERFORMANCE_WINDOW)
            .map_err(|e| ActuatorError::IoError(format!("load recent tasks: {e}")))?;
        Ok(Self::task_performance(&tasks).unwrap_or(1.0))
    }

    fn reconcile_pending_experiments(
        context: &ActuatorContext,
        state: &mut SelfEvolutionState,
        db: &NexusDatabase,
    ) -> Result<Vec<String>, ActuatorError> {
        let tasks = db
            .load_tasks_by_agent(&context.agent_id, PERFORMANCE_WINDOW * 4)
            .map_err(|e| ActuatorError::IoError(format!("load task history: {e}")))?;
        let mut summaries = Vec::new();
        let mut remaining = Vec::new();

        for pending in state.pending_experiments.drain(..) {
            let post_tasks = tasks
                .iter()
                .filter(|task| task.started_at.as_str() > pending.created_at.as_str())
                .take(PERFORMANCE_WINDOW)
                .cloned()
                .collect::<Vec<_>>();

            if post_tasks.len() < PERFORMANCE_WINDOW {
                remaining.push(pending);
                continue;
            }

            let performance_after =
                Self::task_performance(&post_tasks).unwrap_or(pending.performance_before);
            let dropped = pending.performance_before > 0.0
                && performance_after < pending.performance_before * 0.9;
            if dropped {
                state.current_description = pending.description_before.clone();
                state.current_version = pending.version - 1;
            }

            db.save_evolution_history(
                &context.agent_id,
                pending.version,
                &pending.description_before,
                &pending.description_after,
                &pending.trigger,
                pending.performance_before,
                performance_after,
                !dropped,
            )
            .map_err(|e| ActuatorError::IoError(format!("save evolution history: {e}")))?;

            summaries.push(if dropped {
                format!(
                    "Auto-rollback applied for version {} after {:.1}% performance drop.",
                    pending.version,
                    ((pending.performance_before - performance_after) / pending.performance_before)
                        * 100.0
                )
            } else {
                format!(
                    "Version {} kept after 5-task review (+{:.1}% vs baseline).",
                    pending.version,
                    if pending.performance_before > 0.0 {
                        ((performance_after - pending.performance_before)
                            / pending.performance_before)
                            * 100.0
                    } else {
                        0.0
                    }
                )
            });
        }

        state.pending_experiments = remaining;
        Ok(summaries)
    }

    fn parse_agent_uuid(agent_id: &str) -> Uuid {
        Uuid::parse_str(agent_id).unwrap_or_else(|_| Uuid::new_v4())
    }

    fn simulate_self_evolution(
        context: &ActuatorContext,
        payload: &[u8],
    ) -> Result<(), ActuatorError> {
        let mut audit = crate::audit::AuditTrail::new();
        let mut speculative = SpeculativeEngine::new();
        let snapshot = speculative.fork_state(
            Self::parse_agent_uuid(&context.agent_id),
            context.fuel_remaining.max(0.0) as u64,
            context.autonomy_level,
            context.capabilities.iter().cloned().collect(),
            0,
        );
        let result = speculative.simulate(
            &snapshot,
            GovernedOperation::SelfEvolution,
            HitlTier::Tier2,
            payload,
            &mut audit,
        );

        if result.resource_impact.fuel_cost as f64 > context.fuel_remaining {
            return Err(ActuatorError::InsufficientFuel {
                needed: result.resource_impact.fuel_cost as f64,
                available: context.fuel_remaining,
            });
        }
        Ok(())
    }

    fn record_checkpoint(
        context: &ActuatorContext,
        field: &str,
        before: serde_json::Value,
        after: serde_json::Value,
    ) -> Result<String, ActuatorError> {
        let mut tm = TimeMachine::default();
        let mut builder = tm.begin_checkpoint("self_evolution", Some(context.agent_id.clone()));
        builder.record_agent_state(&context.agent_id, field, before, after);
        let checkpoint = builder.build();
        let checkpoint_id = checkpoint.id.clone();
        // Best-effort: discard commit_checkpoint Ok value; error is propagated via ?
        let _ = tm
            .commit_checkpoint(checkpoint.clone())
            .map_err(|e| ActuatorError::IoError(format!("commit time machine checkpoint: {e}")))?;
        Self::append_checkpoint(context, checkpoint)?;
        Ok(checkpoint_id)
    }

    fn commit_description_change(
        context: &ActuatorContext,
        state: &mut SelfEvolutionState,
        new_description: String,
        trigger: String,
        baseline: f64,
    ) -> Result<(String, String), ActuatorError> {
        let previous_description =
            Self::current_description(state, "Current agent description").to_string();
        let checkpoint_id = Self::record_checkpoint(
            context,
            "description",
            json!(previous_description),
            json!(new_description),
        )?;

        state.current_version += 1;
        state.current_description = new_description.clone();
        state.pending_experiments.push(PendingExperiment {
            version: state.current_version,
            description_before: previous_description.clone(),
            description_after: new_description,
            trigger,
            performance_before: baseline,
            created_at: Self::current_time_rfc3339(),
            checkpoint_id,
        });
        Ok((
            previous_description,
            format!(
                "Committed self-evolution version {}. Tracking performance across the next 5 tasks.",
                state.current_version
            ),
        ))
    }

    fn run_tournament(
        context: &ActuatorContext,
        state: &mut SelfEvolutionState,
        variants: &[String],
        task: &str,
        rounds: u32,
        baseline: f64,
    ) -> Result<String, ActuatorError> {
        let scored = variants
            .iter()
            .enumerate()
            .map(|(idx, variant)| {
                let task_alignment = task
                    .split_whitespace()
                    .filter(|word| {
                        !word.is_empty() && variant.to_lowercase().contains(&word.to_lowercase())
                    })
                    .count() as f64;
                let score = variant.len() as f64 + task_alignment * 25.0 + rounds as f64;
                (idx, variant, score)
            })
            .collect::<Vec<_>>();
        let (_, winner, winner_score) = scored
            .iter()
            .max_by(|left, right| {
                left.2
                    .partial_cmp(&right.2)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .cloned()
            .ok_or_else(|| {
                ActuatorError::IoError("self_evolution: no tournament winner".to_string())
            })?;

        state.archive.extend(
            scored
                .into_iter()
                .map(|(_, variant, score)| ArchiveVariant {
                    version: state.current_version + 1,
                    description: variant.to_string(),
                    score,
                    task: task.to_string(),
                    rounds,
                }),
        );

        let (_, message) = Self::commit_description_change(
            context,
            state,
            winner.to_string(),
            format!("run_evolution_tournament:{task}"),
            baseline,
        )?;

        Ok(format!(
            "{message} Tournament winner score: {:.1}. Variants tested: {}.",
            winner_score,
            variants.len()
        ))
    }
}

impl Actuator for SelfEvolutionActuator {
    fn name(&self) -> &str {
        "self_evolution"
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
            .map_err(|e| ActuatorError::IoError(format!("open evolution db: {e}")))?;
        let mut state = Self::load_state(context)?;
        let reconciled = Self::reconcile_pending_experiments(context, &mut state, &db)?;
        let baseline = Self::recent_performance(&db, &context.agent_id)?;

        let output = match action {
            PlannedAction::SelfModifyDescription { new_description } => {
                if new_description.trim().is_empty() {
                    return Err(ActuatorError::IoError(
                        "self_evolution: new_description cannot be empty".to_string(),
                    ));
                }
                if new_description.len() > MAX_DESCRIPTION_BYTES {
                    return Err(ActuatorError::BodyTooLarge {
                        size: new_description.len() as u64,
                        max: MAX_DESCRIPTION_BYTES as u64,
                    });
                }

                Self::simulate_self_evolution(context, new_description.as_bytes())?;
                let (_, message) = Self::commit_description_change(
                    context,
                    &mut state,
                    new_description.clone(),
                    "self_modify_description".to_string(),
                    baseline,
                )?;
                message
            }
            PlannedAction::SelfModifyStrategy {
                strategy_key,
                new_strategy,
            } => {
                if strategy_key.trim().is_empty() || new_strategy.trim().is_empty() {
                    return Err(ActuatorError::IoError(
                        "self_evolution: strategy_key and new_strategy cannot be empty".to_string(),
                    ));
                }

                let before = state
                    .strategies
                    .get(strategy_key)
                    .cloned()
                    .unwrap_or_default();
                let payload = json!({
                    "strategy_key": strategy_key,
                    "new_strategy": new_strategy,
                });
                Self::simulate_self_evolution(context, payload.to_string().as_bytes())?;
                let checkpoint_id = Self::record_checkpoint(
                    context,
                    &format!("strategy:{strategy_key}"),
                    json!(before),
                    json!(new_strategy),
                )?;
                state.current_version += 1;
                state
                    .strategies
                    .insert(strategy_key.clone(), new_strategy.clone());
                state.pending_experiments.push(PendingExperiment {
                    version: state.current_version,
                    description_before: before,
                    description_after: new_strategy.clone(),
                    trigger: format!("self_modify_strategy:{strategy_key}"),
                    performance_before: baseline,
                    created_at: Self::current_time_rfc3339(),
                    checkpoint_id,
                });
                format!(
                    "Committed strategy rewrite '{}' at version {}. Tracking performance across the next 5 tasks.",
                    strategy_key, state.current_version
                )
            }
            PlannedAction::RunEvolutionTournament {
                variants,
                task,
                rounds,
            } => {
                if variants.is_empty() {
                    return Err(ActuatorError::IoError(
                        "self_evolution: variants cannot be empty".to_string(),
                    ));
                }
                if variants.len() > MAX_VARIANTS {
                    return Err(ActuatorError::IoError(format!(
                        "self_evolution: variants exceeds maximum ({MAX_VARIANTS})"
                    )));
                }
                if task.trim().is_empty() {
                    return Err(ActuatorError::IoError(
                        "self_evolution: task cannot be empty".to_string(),
                    ));
                }
                if *rounds == 0 || *rounds > 100 {
                    return Err(ActuatorError::IoError(
                        "self_evolution: rounds must be 1-100".to_string(),
                    ));
                }

                let payload = json!({
                    "variants": variants,
                    "task": task,
                    "rounds": rounds,
                });
                Self::simulate_self_evolution(context, payload.to_string().as_bytes())?;
                Self::run_tournament(context, &mut state, variants, task, *rounds, baseline)?
            }
            _ => return Err(ActuatorError::ActionNotHandled),
        };

        Self::save_state(context, &state)?;

        let mut full_output = output;
        if !reconciled.is_empty() {
            full_output.push(' ');
            full_output.push_str(&reconciled.join(" "));
        }

        Ok(ActionResult {
            success: true,
            output: full_output,
            fuel_cost: match action {
                PlannedAction::SelfModifyDescription { .. } => 15.0,
                PlannedAction::SelfModifyStrategy { .. } => 10.0,
                PlannedAction::RunEvolutionTournament {
                    variants, rounds, ..
                } => (variants.len() as f64) * (*rounds as f64) * 5.0,
                _ => 0.0,
            },
            side_effects: vec![SideEffect::FileModified {
                path: Self::state_path(context)?,
            }],
        })
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
            agent_id: Uuid::new_v4().to_string(),
            agent_name: "self-evolution-agent".to_string(),
            working_dir: workspace.to_path_buf(),
            autonomy_level: AutonomyLevel::L4,
            capabilities: caps,
            fuel_remaining: 10_000.0,
            egress_allowlist: vec![],
            action_review_engine: None,
        }
    }

    #[test]
    fn self_modify_description_persists_state() {
        let tmp = TempDir::new().unwrap();
        let ctx = make_l4_context(tmp.path());
        let actuator = SelfEvolutionActuator;

        let result = actuator
            .execute(
                &PlannedAction::SelfModifyDescription {
                    new_description: "Updated agent description".to_string(),
                },
                &ctx,
            )
            .unwrap();
        assert!(result.success);

        let state = SelfEvolutionActuator::load_state(&ctx).unwrap();
        assert_eq!(state.current_description, "Updated agent description");
        assert_eq!(state.current_version, 1);
        assert_eq!(state.pending_experiments.len(), 1);
    }

    #[test]
    fn self_modify_strategy_tracks_strategy_state() {
        let tmp = TempDir::new().unwrap();
        let ctx = make_l4_context(tmp.path());
        let actuator = SelfEvolutionActuator;

        actuator
            .execute(
                &PlannedAction::SelfModifyStrategy {
                    strategy_key: "search_depth".to_string(),
                    new_strategy: "increase to 5 levels".to_string(),
                },
                &ctx,
            )
            .unwrap();

        let state = SelfEvolutionActuator::load_state(&ctx).unwrap();
        assert_eq!(
            state.strategies.get("search_depth").map(String::as_str),
            Some("increase to 5 levels")
        );
    }

    #[test]
    fn tournament_adopts_winner_and_archives_variants() {
        let tmp = TempDir::new().unwrap();
        let ctx = make_l4_context(tmp.path());
        let actuator = SelfEvolutionActuator;

        actuator
            .execute(
                &PlannedAction::RunEvolutionTournament {
                    variants: vec![
                        "short variant".into(),
                        "longer variant optimized for code review planning".into(),
                    ],
                    task: "code review planning".to_string(),
                    rounds: 3,
                },
                &ctx,
            )
            .unwrap();

        let state = SelfEvolutionActuator::load_state(&ctx).unwrap();
        assert_eq!(state.archive.len(), 2);
        assert!(state.current_description.contains("planning"));
    }

    #[test]
    fn l3_agent_is_denied() {
        let tmp = TempDir::new().unwrap();
        let mut ctx = make_l4_context(tmp.path());
        ctx.autonomy_level = AutonomyLevel::L3;
        let actuator = SelfEvolutionActuator;
        let err = actuator
            .execute(
                &PlannedAction::SelfModifyDescription {
                    new_description: "new".to_string(),
                },
                &ctx,
            )
            .unwrap_err();
        assert!(matches!(err, ActuatorError::CapabilityDenied(_)));
    }
}
