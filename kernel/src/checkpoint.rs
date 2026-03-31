//! Execution checkpoint-rollback system — extends the time machine to cover
//! full agent execution state, side effects, and recovery strategies.
//!
//! Three-level rollback:
//! - **Level 1**: Memory rollback (nexus-memory Phase 3 — already built)
//! - **Level 2**: Execution rollback (this module) — task state, plan, step outputs
//! - **Level 3**: Side effect compensation (this module) — file writes, messages, API calls

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ═══════════════════════════════════════════════════════════════════════════
// Types
// ═══════════════════════════════════════════════════════════════════════════

/// A complete checkpoint of agent execution state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionCheckpoint {
    pub id: Uuid,
    pub agent_id: String,
    pub task_id: String,
    pub label: String,
    pub step_index: usize,
    pub plan_state: serde_json::Value,
    pub step_outputs: Vec<StepOutput>,
    /// Links to nexus-memory MemoryCheckpoint.
    pub memory_checkpoint_id: Option<Uuid>,
    /// Side effects recorded since the previous checkpoint.
    pub side_effects: Vec<SideEffect>,
    pub fuel_consumed: f64,
    pub created_at: u64,
}

/// Output from a completed execution step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepOutput {
    pub step_index: usize,
    pub step_name: String,
    pub output: serde_json::Value,
    pub tool_used: Option<String>,
    pub completed_at: u64,
    pub success: bool,
}

/// A side effect produced by agent execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SideEffect {
    pub id: Uuid,
    pub effect_type: SideEffectType,
    pub description: String,
    pub reversible: bool,
    pub compensation: Option<CompensationAction>,
    pub executed_at: u64,
    pub reversed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SideEffectType {
    FileWrite {
        path: String,
    },
    FileDelete {
        path: String,
    },
    FileCreate {
        path: String,
    },
    MessageSent {
        platform: String,
        channel: String,
        message_id: String,
    },
    ApiCall {
        url: String,
        method: String,
        response_status: u16,
    },
    GitCommit {
        repo: String,
        commit_hash: String,
    },
    Delegation {
        delegate_agent_id: String,
        delegation_id: String,
    },
    DatabaseWrite {
        table: String,
        operation: String,
        record_id: String,
    },
    Custom {
        category: String,
    },
}

/// How to compensate/reverse a side effect.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CompensationAction {
    RestoreFile {
        path: String,
        content: String,
    },
    DeleteFile {
        path: String,
    },
    SendCorrection {
        platform: String,
        channel: String,
        correction: String,
    },
    GitRevert {
        repo: String,
        commit_hash: String,
    },
    CancelDelegation {
        delegation_id: String,
    },
    Custom {
        description: String,
    },
    /// No compensation possible — flag for human review.
    ManualReview {
        reason: String,
    },
}

/// Result of a rollback operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackResult {
    pub checkpoint_id: Uuid,
    pub agent_id: String,
    pub task_id: String,
    pub reason: String,
    pub steps_rolled_back: usize,
    pub memory_rolled_back: bool,
    pub effects_compensated: Vec<Uuid>,
    pub effects_irreversible: Vec<Uuid>,
    pub recovery: RecoveryStrategy,
    pub performed_at: u64,
}

/// What to do after rollback.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RecoveryStrategy {
    RetryStep {
        step_index: usize,
        attempt: u32,
        max_attempts: u32,
    },
    EscalateModel {
        step_index: usize,
        from_model: String,
        to_model: String,
    },
    SkipStep {
        step_index: usize,
        reason: String,
    },
    Replan {
        from_step: usize,
    },
    EscalateToHuman {
        reason: String,
    },
    Abort {
        reason: String,
    },
}

/// Metadata about a step, used to decide whether to checkpoint.
#[derive(Debug, Clone)]
pub struct StepInfo {
    pub step_index: usize,
    pub step_name: String,
    pub capabilities_used: Vec<String>,
    pub risk_level: u8,
}

/// Policy for when to checkpoint and how to recover.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointPolicy {
    pub checkpoint_every_step: bool,
    pub checkpoint_before_capabilities: Vec<String>,
    pub checkpoint_above_risk_level: u8,
    pub max_retries_per_step: u32,
    pub enable_model_escalation: bool,
    pub model_escalation_chain: Vec<String>,
    pub enable_replanning: bool,
    pub max_rollbacks_per_task: u32,
}

impl Default for CheckpointPolicy {
    fn default() -> Self {
        Self {
            checkpoint_every_step: false,
            checkpoint_before_capabilities: vec![
                "fs.write".into(),
                "process.exec".into(),
                "messaging.send".into(),
                "computer.use".into(),
                "agent.message".into(),
            ],
            checkpoint_above_risk_level: 3,
            max_retries_per_step: 3,
            enable_model_escalation: true,
            model_escalation_chain: vec![
                "ollama/llama3".into(),
                "deepseek-chat".into(),
                "claude-sonnet-4-20250514".into(),
            ],
            enable_replanning: true,
            max_rollbacks_per_task: 5,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CheckpointError {
    #[error("checkpoint not found: {0}")]
    NotFound(Uuid),
    #[error("max rollbacks exceeded for task {task_id}: {count}/{max}")]
    MaxRollbacksExceeded {
        task_id: String,
        count: u32,
        max: u32,
    },
    #[error("compensation failed for effect {effect_id}: {reason}")]
    CompensationFailed { effect_id: Uuid, reason: String },
    #[error("recovery exhausted: {0}")]
    RecoveryExhausted(String),
    #[error("no checkpoints for task {0}")]
    NoCheckpoints(String),
}

// ═══════════════════════════════════════════════════════════════════════════
// Side Effect Tracker
// ═══════════════════════════════════════════════════════════════════════════

/// Tracks side effects during agent execution for potential rollback.
#[derive(Debug, Default)]
pub struct SideEffectTracker {
    effects: Vec<SideEffect>,
}

impl SideEffectTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a file write. If previous content is provided, marks as reversible.
    pub fn record_file_write(&mut self, path: &str, previous_content: Option<&str>) -> Uuid {
        let id = Uuid::new_v4();
        let reversible = previous_content.is_some();
        let compensation = previous_content.map(|c| CompensationAction::RestoreFile {
            path: path.to_string(),
            content: c.to_string(),
        });
        self.effects.push(SideEffect {
            id,
            effect_type: SideEffectType::FileWrite {
                path: path.to_string(),
            },
            description: format!("File written: {path}"),
            reversible,
            compensation,
            executed_at: unix_now(),
            reversed: false,
        });
        id
    }

    /// Record a file creation (new file). Reversible via deletion.
    pub fn record_file_create(&mut self, path: &str) -> Uuid {
        let id = Uuid::new_v4();
        self.effects.push(SideEffect {
            id,
            effect_type: SideEffectType::FileCreate {
                path: path.to_string(),
            },
            description: format!("File created: {path}"),
            reversible: true,
            compensation: Some(CompensationAction::DeleteFile {
                path: path.to_string(),
            }),
            executed_at: unix_now(),
            reversed: false,
        });
        id
    }

    /// Record a file deletion. Reversible if previous content provided.
    pub fn record_file_delete(&mut self, path: &str, previous_content: &str) -> Uuid {
        let id = Uuid::new_v4();
        self.effects.push(SideEffect {
            id,
            effect_type: SideEffectType::FileDelete {
                path: path.to_string(),
            },
            description: format!("File deleted: {path}"),
            reversible: true,
            compensation: Some(CompensationAction::RestoreFile {
                path: path.to_string(),
                content: previous_content.to_string(),
            }),
            executed_at: unix_now(),
            reversed: false,
        });
        id
    }

    /// Record a message sent. Best-effort reversible via correction.
    pub fn record_message_sent(&mut self, platform: &str, channel: &str, message_id: &str) -> Uuid {
        let id = Uuid::new_v4();
        self.effects.push(SideEffect {
            id,
            effect_type: SideEffectType::MessageSent {
                platform: platform.to_string(),
                channel: channel.to_string(),
                message_id: message_id.to_string(),
            },
            description: format!("Message sent to {platform}/{channel}"),
            reversible: true,
            compensation: Some(CompensationAction::SendCorrection {
                platform: platform.to_string(),
                channel: channel.to_string(),
                correction: format!(
                    "[CORRECTION] Previous message {message_id} was sent in error and has been rolled back."
                ),
            }),
            executed_at: unix_now(),
            reversed: false,
        });
        id
    }

    /// Record an API call. Generally irreversible.
    pub fn record_api_call(&mut self, url: &str, method: &str, status: u16) -> Uuid {
        let id = Uuid::new_v4();
        self.effects.push(SideEffect {
            id,
            effect_type: SideEffectType::ApiCall {
                url: url.to_string(),
                method: method.to_string(),
                response_status: status,
            },
            description: format!("{method} {url} -> {status}"),
            reversible: false,
            compensation: Some(CompensationAction::ManualReview {
                reason: format!("API call {method} {url} cannot be automatically reversed"),
            }),
            executed_at: unix_now(),
            reversed: false,
        });
        id
    }

    /// Record a git commit. Reversible via git revert.
    pub fn record_git_commit(&mut self, repo: &str, commit_hash: &str) -> Uuid {
        let id = Uuid::new_v4();
        self.effects.push(SideEffect {
            id,
            effect_type: SideEffectType::GitCommit {
                repo: repo.to_string(),
                commit_hash: commit_hash.to_string(),
            },
            description: format!("Git commit {commit_hash} in {repo}"),
            reversible: true,
            compensation: Some(CompensationAction::GitRevert {
                repo: repo.to_string(),
                commit_hash: commit_hash.to_string(),
            }),
            executed_at: unix_now(),
            reversed: false,
        });
        id
    }

    /// Record an agent delegation. Reversible via cancellation.
    pub fn record_delegation(&mut self, delegate_agent_id: &str, delegation_id: &str) -> Uuid {
        let id = Uuid::new_v4();
        self.effects.push(SideEffect {
            id,
            effect_type: SideEffectType::Delegation {
                delegate_agent_id: delegate_agent_id.to_string(),
                delegation_id: delegation_id.to_string(),
            },
            description: format!("Delegated to agent {delegate_agent_id}"),
            reversible: true,
            compensation: Some(CompensationAction::CancelDelegation {
                delegation_id: delegation_id.to_string(),
            }),
            executed_at: unix_now(),
            reversed: false,
        });
        id
    }

    /// Record a custom side effect.
    pub fn record_custom(
        &mut self,
        category: &str,
        description: &str,
        reversible: bool,
        compensation: Option<CompensationAction>,
    ) -> Uuid {
        let id = Uuid::new_v4();
        self.effects.push(SideEffect {
            id,
            effect_type: SideEffectType::Custom {
                category: category.to_string(),
            },
            description: description.to_string(),
            reversible,
            compensation,
            executed_at: unix_now(),
            reversed: false,
        });
        id
    }

    /// Take all recorded effects, emptying the tracker.
    pub fn drain(&mut self) -> Vec<SideEffect> {
        std::mem::take(&mut self.effects)
    }

    pub fn all(&self) -> &[SideEffect] {
        &self.effects
    }

    pub fn reversible_count(&self) -> usize {
        self.effects.iter().filter(|e| e.reversible).count()
    }

    pub fn irreversible_count(&self) -> usize {
        self.effects.iter().filter(|e| !e.reversible).count()
    }

    pub fn is_empty(&self) -> bool {
        self.effects.is_empty()
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Checkpoint Manager
// ═══════════════════════════════════════════════════════════════════════════

/// Manages execution checkpoints and rollback for agent tasks.
#[derive(Debug)]
pub struct CheckpointManager {
    /// task_id → list of checkpoints (ordered by creation time).
    checkpoints: HashMap<String, Vec<ExecutionCheckpoint>>,
    /// Rollback history.
    rollback_history: Vec<RollbackResult>,
    /// Rollback count per task.
    rollback_counts: HashMap<String, u32>,
    /// Policy.
    policy: CheckpointPolicy,
    /// Maximum checkpoints per task before pruning oldest.
    max_checkpoints_per_task: usize,
}

impl CheckpointManager {
    pub fn new(policy: CheckpointPolicy) -> Self {
        Self {
            checkpoints: HashMap::new(),
            rollback_history: Vec::new(),
            rollback_counts: HashMap::new(),
            policy,
            max_checkpoints_per_task: 20,
        }
    }

    pub fn with_max_checkpoints(mut self, max: usize) -> Self {
        self.max_checkpoints_per_task = max;
        self
    }

    /// Check if a checkpoint should be created before this step.
    pub fn should_checkpoint(&self, step: &StepInfo) -> bool {
        if self.policy.checkpoint_every_step {
            return true;
        }

        if step.risk_level >= self.policy.checkpoint_above_risk_level {
            return true;
        }

        for cap in &step.capabilities_used {
            if self.policy.checkpoint_before_capabilities.contains(cap) {
                return true;
            }
        }

        false
    }

    /// Create and store a checkpoint.
    #[allow(clippy::too_many_arguments)]
    pub fn create_checkpoint(
        &mut self,
        agent_id: &str,
        task_id: &str,
        label: &str,
        step_index: usize,
        plan_state: serde_json::Value,
        step_outputs: Vec<StepOutput>,
        side_effects: Vec<SideEffect>,
        memory_checkpoint_id: Option<Uuid>,
        fuel_consumed: f64,
    ) -> ExecutionCheckpoint {
        let cp = ExecutionCheckpoint {
            id: Uuid::new_v4(),
            agent_id: agent_id.to_string(),
            task_id: task_id.to_string(),
            label: label.to_string(),
            step_index,
            plan_state,
            step_outputs,
            memory_checkpoint_id,
            side_effects,
            fuel_consumed,
            created_at: unix_now(),
        };

        let list = self.checkpoints.entry(task_id.to_string()).or_default();

        // Prune oldest if at capacity
        while list.len() >= self.max_checkpoints_per_task {
            list.remove(0);
        }

        list.push(cp.clone());
        cp
    }

    /// Rollback to a specific checkpoint.
    pub fn rollback(
        &mut self,
        task_id: &str,
        checkpoint_id: Uuid,
        reason: &str,
    ) -> Result<RollbackResult, CheckpointError> {
        // Check rollback limit
        let count = self.rollback_counts.get(task_id).copied().unwrap_or(0);
        if count >= self.policy.max_rollbacks_per_task {
            return Err(CheckpointError::MaxRollbacksExceeded {
                task_id: task_id.to_string(),
                count,
                max: self.policy.max_rollbacks_per_task,
            });
        }

        let list = self
            .checkpoints
            .get(task_id)
            .ok_or_else(|| CheckpointError::NoCheckpoints(task_id.to_string()))?;

        let cp_idx = list
            .iter()
            .position(|cp| cp.id == checkpoint_id)
            .ok_or(CheckpointError::NotFound(checkpoint_id))?;

        let cp = &list[cp_idx];

        // Collect all side effects from checkpoints AFTER the target checkpoint.
        let mut effects_to_compensate = Vec::new();
        for later_cp in list.iter().skip(cp_idx + 1) {
            effects_to_compensate.extend(later_cp.side_effects.iter().cloned());
        }

        // Attempt compensation
        let mut compensated = Vec::new();
        let mut irreversible = Vec::new();
        for effect in &effects_to_compensate {
            if effect.reversible && effect.compensation.is_some() {
                compensated.push(effect.id);
            } else {
                irreversible.push(effect.id);
            }
        }

        // Calculate steps rolled back
        let current_max_step = list.last().map(|c| c.step_index).unwrap_or(cp.step_index);
        let steps_rolled_back = current_max_step.saturating_sub(cp.step_index);

        let result = RollbackResult {
            checkpoint_id,
            agent_id: cp.agent_id.clone(),
            task_id: task_id.to_string(),
            reason: reason.to_string(),
            steps_rolled_back,
            memory_rolled_back: cp.memory_checkpoint_id.is_some(),
            effects_compensated: compensated,
            effects_irreversible: irreversible,
            recovery: RecoveryStrategy::RetryStep {
                step_index: cp.step_index,
                attempt: count + 1,
                max_attempts: self.policy.max_retries_per_step,
            },
            performed_at: unix_now(),
        };

        // Prune checkpoints after the rollback target
        if let Some(list) = self.checkpoints.get_mut(task_id) {
            list.truncate(cp_idx + 1);
        }

        // Increment rollback count
        *self.rollback_counts.entry(task_id.to_string()).or_insert(0) += 1;

        self.rollback_history.push(result.clone());
        Ok(result)
    }

    /// Choose a recovery strategy based on failure count and policy.
    pub fn choose_recovery(
        &self,
        task_id: &str,
        step_index: usize,
        failure_count: u32,
        current_model: &str,
    ) -> RecoveryStrategy {
        // 1. Retry if under limit
        if failure_count < self.policy.max_retries_per_step {
            return RecoveryStrategy::RetryStep {
                step_index,
                attempt: failure_count + 1,
                max_attempts: self.policy.max_retries_per_step,
            };
        }

        // 2. Model escalation
        if self.policy.enable_model_escalation {
            let chain = &self.policy.model_escalation_chain;
            let current_pos = chain.iter().position(|m| m == current_model);
            let next_pos = current_pos.map(|p| p + 1).unwrap_or(0);
            if next_pos < chain.len() {
                return RecoveryStrategy::EscalateModel {
                    step_index,
                    from_model: current_model.to_string(),
                    to_model: chain[next_pos].clone(),
                };
            }
        }

        // 3. Replanning
        if self.policy.enable_replanning {
            return RecoveryStrategy::Replan {
                from_step: step_index,
            };
        }

        // 4. Escalate to human
        let rollback_count = self.rollback_counts.get(task_id).copied().unwrap_or(0);
        if rollback_count < self.policy.max_rollbacks_per_task {
            return RecoveryStrategy::EscalateToHuman {
                reason: format!(
                    "Step {step_index} failed after {failure_count} retries with all models"
                ),
            };
        }

        // 5. Abort
        RecoveryStrategy::Abort {
            reason: format!(
                "Task {task_id}: all recovery strategies exhausted at step {step_index}"
            ),
        }
    }

    /// Get checkpoints for a task.
    pub fn get_checkpoints(&self, task_id: &str) -> &[ExecutionCheckpoint] {
        self.checkpoints
            .get(task_id)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Get the most recent checkpoint for a task.
    pub fn latest_checkpoint(&self, task_id: &str) -> Option<&ExecutionCheckpoint> {
        self.checkpoints.get(task_id).and_then(|v| v.last())
    }

    /// Get rollback count for a task.
    pub fn rollback_count(&self, task_id: &str) -> u32 {
        self.rollback_counts.get(task_id).copied().unwrap_or(0)
    }

    /// Get rollback history.
    pub fn rollback_history(&self) -> &[RollbackResult] {
        &self.rollback_history
    }

    /// Clean up after task completion.
    pub fn clear_task(&mut self, task_id: &str) {
        self.checkpoints.remove(task_id);
        self.rollback_counts.remove(task_id);
    }

    /// Get the policy.
    pub fn policy(&self) -> &CheckpointPolicy {
        &self.policy
    }

    /// Update the policy.
    pub fn set_policy(&mut self, policy: CheckpointPolicy) {
        self.policy = policy;
    }

    /// Total checkpoint count across all tasks.
    pub fn total_checkpoints(&self) -> usize {
        self.checkpoints.values().map(|v| v.len()).sum()
    }

    /// Summary stats for monitoring.
    pub fn stats(&self) -> CheckpointStats {
        CheckpointStats {
            active_tasks: self.checkpoints.len(),
            total_checkpoints: self.total_checkpoints(),
            total_rollbacks: self.rollback_history.len(),
        }
    }
}

/// Summary statistics for the checkpoint system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointStats {
    pub active_tasks: usize,
    pub total_checkpoints: usize,
    pub total_rollbacks: usize,
}

// ═══════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn default_mgr() -> CheckpointManager {
        CheckpointManager::new(CheckpointPolicy::default())
    }

    fn step_info(name: &str, caps: &[&str], risk: u8) -> StepInfo {
        StepInfo {
            step_index: 0,
            step_name: name.to_string(),
            capabilities_used: caps.iter().map(|s| s.to_string()).collect(),
            risk_level: risk,
        }
    }

    // ── should_checkpoint tests ──────────────────────────────────────

    #[test]
    fn checkpoint_every_step_always_true() {
        let mut mgr = default_mgr();
        mgr.policy.checkpoint_every_step = true;
        assert!(mgr.should_checkpoint(&step_info("read", &["fs.read"], 1)));
    }

    #[test]
    fn checkpoint_matches_capability() {
        let mgr = default_mgr();
        assert!(mgr.should_checkpoint(&step_info("write", &["fs.write"], 1)));
        assert!(mgr.should_checkpoint(&step_info("exec", &["process.exec"], 1)));
    }

    #[test]
    fn checkpoint_risk_level_threshold() {
        let mgr = default_mgr();
        assert!(mgr.should_checkpoint(&step_info("risky", &["custom.x"], 5)));
        assert!(!mgr.should_checkpoint(&step_info("safe", &["custom.x"], 1)));
    }

    #[test]
    fn checkpoint_no_match_returns_false() {
        let mgr = default_mgr();
        assert!(!mgr.should_checkpoint(&step_info("read", &["fs.read"], 1)));
    }

    // ── create_checkpoint tests ──────────────────────────────────────

    #[test]
    fn create_checkpoint_stores_correctly() {
        let mut mgr = default_mgr();
        let cp = mgr.create_checkpoint(
            "agent-1",
            "task-1",
            "before write",
            2,
            serde_json::json!({"step": 2}),
            vec![],
            vec![],
            None,
            100.0,
        );
        assert_eq!(cp.step_index, 2);
        assert_eq!(mgr.get_checkpoints("task-1").len(), 1);
    }

    #[test]
    fn create_checkpoint_prunes_oldest() {
        let mut mgr = CheckpointManager::new(CheckpointPolicy::default()).with_max_checkpoints(3);

        for i in 0..5 {
            mgr.create_checkpoint(
                "a",
                "t",
                &format!("cp-{i}"),
                i,
                serde_json::json!({}),
                vec![],
                vec![],
                None,
                0.0,
            );
        }
        assert_eq!(mgr.get_checkpoints("t").len(), 3);
        // Oldest pruned: remaining should be steps 2, 3, 4
        assert_eq!(mgr.get_checkpoints("t")[0].step_index, 2);
    }

    // ── rollback tests ───────────────────────────────────────────────

    #[test]
    fn rollback_returns_correct_result() {
        let mut mgr = default_mgr();
        let cp = mgr.create_checkpoint(
            "a",
            "t",
            "cp1",
            0,
            serde_json::json!({}),
            vec![],
            vec![],
            None,
            0.0,
        );
        mgr.create_checkpoint(
            "a",
            "t",
            "cp2",
            1,
            serde_json::json!({}),
            vec![],
            vec![],
            None,
            50.0,
        );

        let result = mgr.rollback("t", cp.id, "step 1 failed").unwrap();
        assert_eq!(result.steps_rolled_back, 1);
        assert_eq!(result.agent_id, "a");
        // Checkpoint list should be truncated to just cp1
        assert_eq!(mgr.get_checkpoints("t").len(), 1);
    }

    #[test]
    fn rollback_increments_count() {
        let mut mgr = default_mgr();
        let cp = mgr.create_checkpoint(
            "a",
            "t",
            "cp",
            0,
            serde_json::json!({}),
            vec![],
            vec![],
            None,
            0.0,
        );
        assert_eq!(mgr.rollback_count("t"), 0);
        mgr.rollback("t", cp.id, "fail").unwrap();
        assert_eq!(mgr.rollback_count("t"), 1);
    }

    #[test]
    fn rollback_exceeding_max_returns_error() {
        let mut mgr = CheckpointManager::new(CheckpointPolicy {
            max_rollbacks_per_task: 1,
            ..Default::default()
        });

        let cp1 = mgr.create_checkpoint(
            "a",
            "t",
            "cp1",
            0,
            serde_json::json!({}),
            vec![],
            vec![],
            None,
            0.0,
        );
        mgr.rollback("t", cp1.id, "fail 1").unwrap();

        let cp2 = mgr.create_checkpoint(
            "a",
            "t",
            "cp2",
            0,
            serde_json::json!({}),
            vec![],
            vec![],
            None,
            0.0,
        );
        let result = mgr.rollback("t", cp2.id, "fail 2");
        assert!(matches!(
            result,
            Err(CheckpointError::MaxRollbacksExceeded { .. })
        ));
    }

    #[test]
    fn rollback_not_found() {
        let mut mgr = default_mgr();
        mgr.create_checkpoint(
            "a",
            "t",
            "cp",
            0,
            serde_json::json!({}),
            vec![],
            vec![],
            None,
            0.0,
        );
        let result = mgr.rollback("t", Uuid::new_v4(), "fail");
        assert!(matches!(result, Err(CheckpointError::NotFound(_))));
    }

    #[test]
    fn rollback_no_checkpoints() {
        let mut mgr = default_mgr();
        let result = mgr.rollback("nonexistent", Uuid::new_v4(), "fail");
        assert!(matches!(result, Err(CheckpointError::NoCheckpoints(_))));
    }

    #[test]
    fn rollback_compensates_side_effects() {
        let mut mgr = default_mgr();
        let cp1 = mgr.create_checkpoint(
            "a",
            "t",
            "cp1",
            0,
            serde_json::json!({}),
            vec![],
            vec![],
            None,
            0.0,
        );

        // Second checkpoint has side effects
        let effects = vec![
            SideEffect {
                id: Uuid::new_v4(),
                effect_type: SideEffectType::FileCreate {
                    path: "/tmp/x".into(),
                },
                description: "created file".into(),
                reversible: true,
                compensation: Some(CompensationAction::DeleteFile {
                    path: "/tmp/x".into(),
                }),
                executed_at: unix_now(),
                reversed: false,
            },
            SideEffect {
                id: Uuid::new_v4(),
                effect_type: SideEffectType::ApiCall {
                    url: "https://api.example.com".into(),
                    method: "POST".into(),
                    response_status: 200,
                },
                description: "API call".into(),
                reversible: false,
                compensation: None,
                executed_at: unix_now(),
                reversed: false,
            },
        ];
        mgr.create_checkpoint(
            "a",
            "t",
            "cp2",
            1,
            serde_json::json!({}),
            vec![],
            effects,
            None,
            50.0,
        );

        let result = mgr.rollback("t", cp1.id, "step 1 failed").unwrap();
        assert_eq!(result.effects_compensated.len(), 1);
        assert_eq!(result.effects_irreversible.len(), 1);
    }

    // ── choose_recovery tests ────────────────────────────────────────

    #[test]
    fn recovery_first_failure_retries() {
        let mgr = default_mgr();
        let r = mgr.choose_recovery("t", 2, 0, "ollama/llama3");
        assert!(matches!(r, RecoveryStrategy::RetryStep { attempt: 1, .. }));
    }

    #[test]
    fn recovery_max_retries_escalates_model() {
        let mgr = default_mgr();
        let r = mgr.choose_recovery("t", 2, 3, "ollama/llama3");
        assert!(matches!(
            r,
            RecoveryStrategy::EscalateModel { to_model, .. } if to_model == "deepseek-chat"
        ));
    }

    #[test]
    fn recovery_model_chain_exhausted_replans() {
        let mgr = default_mgr();
        let r = mgr.choose_recovery("t", 2, 3, "claude-sonnet-4-20250514");
        assert!(matches!(r, RecoveryStrategy::Replan { .. }));
    }

    #[test]
    fn recovery_all_exhausted_escalates_human() {
        let mgr = CheckpointManager::new(CheckpointPolicy {
            enable_replanning: false,
            ..Default::default()
        });
        let r = mgr.choose_recovery("t", 2, 3, "claude-sonnet-4-20250514");
        assert!(matches!(r, RecoveryStrategy::EscalateToHuman { .. }));
    }

    #[test]
    fn recovery_unknown_model_starts_chain() {
        let mgr = default_mgr();
        let r = mgr.choose_recovery("t", 0, 3, "some-unknown-model");
        assert!(matches!(
            r,
            RecoveryStrategy::EscalateModel { to_model, .. } if to_model == "ollama/llama3"
        ));
    }

    // ── Side effect tracker tests ────────────────────────────────────

    #[test]
    fn tracker_file_write_with_previous_is_reversible() {
        let mut t = SideEffectTracker::new();
        t.record_file_write("/tmp/test", Some("old content"));
        assert_eq!(t.reversible_count(), 1);
        assert_eq!(t.irreversible_count(), 0);
    }

    #[test]
    fn tracker_file_write_without_previous_is_irreversible() {
        let mut t = SideEffectTracker::new();
        t.record_file_write("/tmp/test", None);
        assert_eq!(t.reversible_count(), 0);
        assert_eq!(t.irreversible_count(), 1);
    }

    #[test]
    fn tracker_file_create_is_reversible() {
        let mut t = SideEffectTracker::new();
        t.record_file_create("/tmp/new");
        assert_eq!(t.reversible_count(), 1);
        let e = &t.all()[0];
        assert!(matches!(
            e.compensation,
            Some(CompensationAction::DeleteFile { .. })
        ));
    }

    #[test]
    fn tracker_message_sent_is_reversible() {
        let mut t = SideEffectTracker::new();
        t.record_message_sent("slack", "#general", "msg-123");
        assert_eq!(t.reversible_count(), 1);
    }

    #[test]
    fn tracker_api_call_is_irreversible() {
        let mut t = SideEffectTracker::new();
        t.record_api_call("https://api.example.com", "POST", 200);
        assert_eq!(t.irreversible_count(), 1);
        assert!(matches!(
            t.all()[0].compensation,
            Some(CompensationAction::ManualReview { .. })
        ));
    }

    #[test]
    fn tracker_git_commit_is_reversible() {
        let mut t = SideEffectTracker::new();
        t.record_git_commit("/repo", "abc123");
        assert_eq!(t.reversible_count(), 1);
        assert!(matches!(
            t.all()[0].compensation,
            Some(CompensationAction::GitRevert { .. })
        ));
    }

    #[test]
    fn tracker_delegation_is_reversible() {
        let mut t = SideEffectTracker::new();
        t.record_delegation("agent-2", "del-1");
        assert_eq!(t.reversible_count(), 1);
    }

    #[test]
    fn tracker_drain_empties() {
        let mut t = SideEffectTracker::new();
        t.record_file_create("/a");
        t.record_api_call("http://x", "GET", 200);
        let drained = t.drain();
        assert_eq!(drained.len(), 2);
        assert!(t.is_empty());
    }

    #[test]
    fn tracker_custom_reversible() {
        let mut t = SideEffectTracker::new();
        t.record_custom(
            "db",
            "wrote row",
            true,
            Some(CompensationAction::Custom {
                description: "delete row".into(),
            }),
        );
        assert_eq!(t.reversible_count(), 1);
    }

    #[test]
    fn tracker_custom_irreversible() {
        let mut t = SideEffectTracker::new();
        t.record_custom("email", "sent email", false, None);
        assert_eq!(t.irreversible_count(), 1);
    }

    // ── Integration tests ────────────────────────────────────────────

    #[test]
    fn full_lifecycle_checkpoint_rollback_retry() {
        let mut mgr = default_mgr();
        let mut tracker = SideEffectTracker::new();

        // Step 0 completes successfully
        let outputs = vec![StepOutput {
            step_index: 0,
            step_name: "analyze".into(),
            output: serde_json::json!("analysis complete"),
            tool_used: None,
            completed_at: unix_now(),
            success: true,
        }];

        // Checkpoint before risky step 1
        let cp = mgr.create_checkpoint(
            "agent-1",
            "task-1",
            "before_write",
            1,
            serde_json::json!({"plan": "write files"}),
            outputs,
            tracker.drain(),
            None,
            100.0,
        );

        // Step 1 produces side effects then fails
        tracker.record_file_write("/tmp/output.txt", Some("old content"));
        tracker.record_file_create("/tmp/new.txt");

        // Failure → rollback
        let result = mgr.rollback("task-1", cp.id, "write step failed").unwrap();
        assert_eq!(result.steps_rolled_back, 0); // rolled back to step 1
        assert_eq!(result.checkpoint_id, cp.id);

        // Recovery: retry
        let recovery = mgr.choose_recovery("task-1", 1, 0, "ollama/llama3");
        assert!(matches!(recovery, RecoveryStrategy::RetryStep { .. }));
    }

    #[test]
    fn model_escalation_lifecycle() {
        let mgr = default_mgr();

        // Fail 3 times with local model
        let r = mgr.choose_recovery("t", 0, 3, "ollama/llama3");
        assert!(matches!(
            r,
            RecoveryStrategy::EscalateModel { ref to_model, .. } if to_model == "deepseek-chat"
        ));

        // Fail 3 more times with deepseek
        let r = mgr.choose_recovery("t", 0, 3, "deepseek-chat");
        assert!(matches!(
            r,
            RecoveryStrategy::EscalateModel { ref to_model, .. } if to_model == "claude-sonnet-4-20250514"
        ));

        // Fail 3 more times with claude → replan
        let r = mgr.choose_recovery("t", 0, 3, "claude-sonnet-4-20250514");
        assert!(matches!(r, RecoveryStrategy::Replan { .. }));
    }

    #[test]
    fn max_rollbacks_then_abort() {
        let mut mgr = CheckpointManager::new(CheckpointPolicy {
            max_rollbacks_per_task: 2,
            enable_replanning: false,
            enable_model_escalation: false,
            ..Default::default()
        });

        for _ in 0..2 {
            let cp = mgr.create_checkpoint(
                "a",
                "t",
                "cp",
                0,
                serde_json::json!({}),
                vec![],
                vec![],
                None,
                0.0,
            );
            mgr.rollback("t", cp.id, "fail").unwrap();
        }

        // Third rollback blocked
        let cp = mgr.create_checkpoint(
            "a",
            "t",
            "cp",
            0,
            serde_json::json!({}),
            vec![],
            vec![],
            None,
            0.0,
        );
        let result = mgr.rollback("t", cp.id, "fail 3");
        assert!(matches!(
            result,
            Err(CheckpointError::MaxRollbacksExceeded {
                count: 2,
                max: 2,
                ..
            })
        ));

        // Recovery should suggest abort
        let r = mgr.choose_recovery("t", 0, 3, "x");
        assert!(matches!(r, RecoveryStrategy::Abort { .. }));
    }

    #[test]
    fn clear_task_removes_state() {
        let mut mgr = default_mgr();
        mgr.create_checkpoint(
            "a",
            "t",
            "cp",
            0,
            serde_json::json!({}),
            vec![],
            vec![],
            None,
            0.0,
        );
        assert_eq!(mgr.get_checkpoints("t").len(), 1);
        mgr.clear_task("t");
        assert_eq!(mgr.get_checkpoints("t").len(), 0);
        assert_eq!(mgr.rollback_count("t"), 0);
    }

    #[test]
    fn stats_work() {
        let mut mgr = default_mgr();
        mgr.create_checkpoint(
            "a",
            "t1",
            "cp",
            0,
            serde_json::json!({}),
            vec![],
            vec![],
            None,
            0.0,
        );
        mgr.create_checkpoint(
            "a",
            "t2",
            "cp",
            0,
            serde_json::json!({}),
            vec![],
            vec![],
            None,
            0.0,
        );
        let stats = mgr.stats();
        assert_eq!(stats.active_tasks, 2);
        assert_eq!(stats.total_checkpoints, 2);
    }
}
