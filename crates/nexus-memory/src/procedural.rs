//! Procedural memory — learned behaviors that alter how agents act.
//!
//! This is the most dangerous memory type.  Every promoted procedure becomes an
//! automatic behavior the agent can invoke.  Strict promotion gates, regression
//! detection, and demotion paths ensure that only well-evidenced patterns
//! survive.
//!
//! ## Lifecycle
//!
//! ```text
//! Candidate → (promotion gate) → Promoted/Active → (regression?) → Flagged → Demoted
//! ```
//!
//! ## Invariants
//!
//! - No procedure is promoted without meeting the autonomy-level threshold.
//! - Regression checks run after every recorded execution.
//! - Demotion creates an episodic entry (audit trail).

use std::collections::HashMap;

use chrono::Utc;
use uuid::Uuid;

use crate::types::*;

/// Procedural memory store for a single agent.
pub struct ProceduralMemory {
    /// Active promoted procedures.
    procedures: Vec<MemoryEntry>,
    /// Candidates being tracked (not yet promoted).
    candidates: Vec<ProcedureCandidate>,
    /// Execution history for regression tracking (procedure_id → recent executions).
    execution_history: HashMap<MemoryId, Vec<ProcedureExecution>>,
    /// Maximum promoted procedures.
    max_procedures: usize,
    /// Maximum candidates being tracked.
    max_candidates: usize,
    /// How many recent executions to keep per procedure for regression tracking.
    regression_window: usize,
}

impl ProceduralMemory {
    /// Creates a new procedural memory store.
    pub fn new(max_procedures: usize) -> Self {
        Self {
            procedures: Vec::new(),
            candidates: Vec::new(),
            execution_history: HashMap::new(),
            max_procedures,
            max_candidates: 100,
            regression_window: 20,
        }
    }

    /// Adds a promoted procedure.  Validates content is a `Procedure` variant.
    pub fn add_procedure(&mut self, entry: MemoryEntry) -> Result<MemoryId, MemoryError> {
        let expected = entry.content.expected_memory_type();
        if expected != MemoryType::Procedural {
            return Err(MemoryError::TypeMismatch {
                content_type: expected,
                declared_type: MemoryType::Procedural,
            });
        }
        if entry.memory_type != MemoryType::Procedural {
            return Err(MemoryError::TypeMismatch {
                content_type: expected,
                declared_type: entry.memory_type,
            });
        }

        let active_count = self
            .procedures
            .iter()
            .filter(|e| e.validation_state != ValidationState::Revoked)
            .count();
        if active_count >= self.max_procedures {
            return Err(MemoryError::QuotaExceeded {
                agent_id: entry.agent_id.clone(),
                memory_type: MemoryType::Procedural,
                current: active_count,
                max: self.max_procedures,
            });
        }

        let id = entry.id;
        self.procedures.push(entry);
        Ok(id)
    }

    /// Registers a new candidate pattern being tracked.
    pub fn add_candidate(&mut self, candidate: ProcedureCandidate) -> Result<Uuid, MemoryError> {
        if self.candidates.len() >= self.max_candidates {
            return Err(MemoryError::QuotaExceeded {
                agent_id: candidate.agent_id.clone(),
                memory_type: MemoryType::Procedural,
                current: self.candidates.len(),
                max: self.max_candidates,
            });
        }
        let id = candidate.id;
        self.candidates.push(candidate);
        Ok(id)
    }

    /// Records an execution outcome for a procedure.
    pub fn record_execution(&mut self, procedure_id: MemoryId, execution: ProcedureExecution) {
        let history = self.execution_history.entry(procedure_id).or_default();
        history.push(execution);
        // Keep only the most recent `regression_window` executions
        if history.len() > self.regression_window {
            let excess = history.len() - self.regression_window;
            history.drain(..excess);
        }
    }

    /// Returns a reference to a procedure by ID.
    pub fn get_procedure(&self, id: MemoryId) -> Option<&MemoryEntry> {
        self.procedures.iter().find(|e| e.id == id)
    }

    /// Returns a mutable reference to a procedure by ID.
    pub fn get_procedure_mut(&mut self, id: MemoryId) -> Option<&mut MemoryEntry> {
        self.procedures.iter_mut().find(|e| e.id == id)
    }

    /// Finds procedures whose trigger condition matches the given trigger
    /// (case-insensitive substring match).  Sorted by trust score descending.
    pub fn find_matching_procedures(&self, trigger: &str) -> Vec<&MemoryEntry> {
        let trigger_lower = trigger.to_lowercase();
        let mut matched: Vec<&MemoryEntry> = self
            .procedures
            .iter()
            .filter(|e| e.validation_state != ValidationState::Revoked)
            .filter(|e| {
                if let MemoryContent::Procedure {
                    trigger_condition, ..
                } = &e.content
                {
                    trigger_condition.to_lowercase().contains(&trigger_lower)
                } else {
                    false
                }
            })
            .collect();
        matched.sort_by(|a, b| {
            b.trust_score
                .partial_cmp(&a.trust_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        matched
    }

    /// Checks if a candidate meets the promotion threshold for the given
    /// autonomy level.  Returns `None` if not yet eligible.
    pub fn check_promotion(
        &self,
        candidate_id: Uuid,
        autonomy_level: u8,
    ) -> Result<Option<PromotionEvidence>, MemoryError> {
        let candidate = self
            .candidates
            .iter()
            .find(|c| c.id == candidate_id)
            .ok_or(MemoryError::CandidateNotFound(candidate_id))?;

        let thresholds = match PromotionThresholds::for_autonomy_level(autonomy_level) {
            Some(t) => t,
            None => return Ok(None), // cannot self-promote at this level
        };

        let successes = candidate
            .executions
            .iter()
            .filter(|e| matches!(e.outcome, Outcome::Success { .. }))
            .count() as u32;
        let total = candidate.executions.len() as u32;

        if total == 0 {
            return Ok(None);
        }

        let success_rate = successes as f32 / total as f32;

        if successes < thresholds.min_successes {
            return Ok(None);
        }
        if success_rate < thresholds.min_success_rate {
            return Ok(None);
        }

        // Check evidence window
        let window_hours = if let (Some(first), Some(last)) =
            (candidate.executions.first(), candidate.executions.last())
        {
            (last.executed_at - first.executed_at).num_hours() as u64
        } else {
            0
        };

        if window_hours < thresholds.min_evidence_window_hours {
            return Ok(None);
        }

        let evidence_task_ids: Vec<String> = candidate
            .executions
            .iter()
            .filter(|e| matches!(e.outcome, Outcome::Success { .. }))
            .map(|e| e.task_id.clone())
            .collect();

        let window_start = candidate
            .executions
            .first()
            .map(|e| e.executed_at)
            .unwrap_or_else(Utc::now);
        let window_end = candidate
            .executions
            .last()
            .map(|e| e.executed_at)
            .unwrap_or_else(Utc::now);

        Ok(Some(PromotionEvidence {
            evidence_task_ids,
            success_count: successes,
            total_count: total,
            success_rate,
            evidence_window_start: window_start,
            evidence_window_end: window_end,
            approved_by: PromotionApprover::Automatic { autonomy_level },
            promoted_at: Utc::now(),
        }))
    }

    /// Promotes a candidate to a full procedure entry.
    ///
    /// Removes from candidates, creates a `MemoryEntry` with
    /// `epistemic_class = LearnedBehavior`.
    pub fn promote(
        &mut self,
        candidate_id: Uuid,
        evidence: PromotionEvidence,
    ) -> Result<MemoryEntry, MemoryError> {
        let idx = self
            .candidates
            .iter()
            .position(|c| c.id == candidate_id)
            .ok_or(MemoryError::CandidateNotFound(candidate_id))?;

        let candidate = self.candidates.remove(idx);
        let now = Utc::now();

        let entry = MemoryEntry {
            id: Uuid::new_v4(),
            schema_version: 1,
            agent_id: candidate.agent_id,
            memory_type: MemoryType::Procedural,
            epistemic_class: EpistemicClass::LearnedBehavior {
                evidence_task_ids: evidence.evidence_task_ids,
                success_rate: evidence.success_rate,
            },
            validation_state: ValidationState::Corroborated,
            content: MemoryContent::Procedure {
                name: candidate.name,
                description: candidate.description,
                trigger_condition: candidate.trigger_condition,
                steps: candidate.steps,
            },
            embedding: None,
            created_at: now,
            updated_at: now,
            valid_from: now,
            valid_to: None,
            trust_score: evidence.success_rate,
            importance: 0.8,
            confidence: evidence.success_rate,
            supersedes: None,
            derived_from: vec![],
            source_task_id: None,
            source_conversation_id: None,
            scope: MemoryScope::Agent,
            sensitivity: SensitivityClass::Internal,
            access_count: 0,
            last_accessed: now,
            version: 1,
            ttl: None,
            tags: vec!["promoted".into()],
        };

        self.procedures.push(entry.clone());
        Ok(entry)
    }

    /// Checks if a procedure's recent performance warrants flagging or demotion.
    pub fn check_regression(&self, procedure_id: MemoryId) -> Option<RegressionEvent> {
        let entry = self.procedures.iter().find(|e| e.id == procedure_id)?;
        let history = self.execution_history.get(&procedure_id);

        // No execution history at all or empty history — check for staleness only
        let history = match history {
            Some(h) if !h.is_empty() => h,
            _ => {
                let days_since = (Utc::now() - entry.last_accessed).num_days();
                if days_since > 30 {
                    return Some(RegressionEvent {
                        procedure_id,
                        agent_id: entry.agent_id.clone(),
                        trigger: RegressionTrigger::StaleUnused {
                            days_since_last_use: days_since as u64,
                        },
                        success_rate_at_trigger: 0.0,
                        recent_executions: vec![],
                        action_taken: RegressionAction::Flagged,
                        timestamp: Utc::now(),
                    });
                }
                return None;
            }
        };

        // Use up to the last 10 executions for regression check
        let recent: Vec<&ProcedureExecution> = history.iter().rev().take(10).collect();
        let successes = recent
            .iter()
            .filter(|e| matches!(e.outcome, Outcome::Success { .. }))
            .count();
        let total = recent.len();
        let success_rate = successes as f32 / total as f32;

        if success_rate < 0.4 {
            return Some(RegressionEvent {
                procedure_id,
                agent_id: entry.agent_id.clone(),
                trigger: RegressionTrigger::SuccessRateDroppedBelowDemote,
                success_rate_at_trigger: success_rate,
                recent_executions: recent.into_iter().cloned().collect(),
                action_taken: RegressionAction::Demoted {
                    reason: format!(
                        "Success rate {:.0}% below demote threshold (40%)",
                        success_rate * 100.0
                    ),
                },
                timestamp: Utc::now(),
            });
        }

        if success_rate < 0.6 {
            return Some(RegressionEvent {
                procedure_id,
                agent_id: entry.agent_id.clone(),
                trigger: RegressionTrigger::SuccessRateDroppedBelowFlag,
                success_rate_at_trigger: success_rate,
                recent_executions: recent.into_iter().cloned().collect(),
                action_taken: RegressionAction::Flagged,
                timestamp: Utc::now(),
            });
        }

        // Check staleness even when we have history
        if let Some(last) = history.last() {
            let days_since = (Utc::now() - last.executed_at).num_days();
            if days_since > 30 {
                return Some(RegressionEvent {
                    procedure_id,
                    agent_id: entry.agent_id.clone(),
                    trigger: RegressionTrigger::StaleUnused {
                        days_since_last_use: days_since as u64,
                    },
                    success_rate_at_trigger: success_rate,
                    recent_executions: recent.into_iter().cloned().collect(),
                    action_taken: RegressionAction::Flagged,
                    timestamp: Utc::now(),
                });
            }
        }

        None
    }

    /// Demotes a procedure — removes from the active list and returns the entry.
    pub fn demote(
        &mut self,
        procedure_id: MemoryId,
        _reason: &str,
    ) -> Result<MemoryEntry, MemoryError> {
        let idx = self
            .procedures
            .iter()
            .position(|e| e.id == procedure_id)
            .ok_or(MemoryError::EntryNotFound(procedure_id))?;

        let mut entry = self.procedures.remove(idx);
        entry.validation_state = ValidationState::Deprecated;
        entry.updated_at = Utc::now();
        self.execution_history.remove(&procedure_id);
        Ok(entry)
    }

    /// Flags a procedure for review.
    pub fn flag(&mut self, procedure_id: MemoryId) -> Result<(), MemoryError> {
        let entry = self
            .procedures
            .iter_mut()
            .find(|e| e.id == procedure_id)
            .ok_or(MemoryError::EntryNotFound(procedure_id))?;
        entry.validation_state = ValidationState::Contested;
        entry.updated_at = Utc::now();
        Ok(())
    }

    /// Returns all promoted procedures.
    pub fn all_procedures(&self) -> &[MemoryEntry] {
        &self.procedures
    }

    /// Returns all candidates.
    pub fn all_candidates(&self) -> &[ProcedureCandidate] {
        &self.candidates
    }

    /// Returns the number of active procedures.
    pub fn procedure_count(&self) -> usize {
        self.procedures.len()
    }

    /// Returns the number of candidates.
    pub fn candidate_count(&self) -> usize {
        self.candidates.len()
    }

    /// Inserts a pre-existing entry directly (used by restore from persistence).
    pub fn insert_entry(&mut self, entry: MemoryEntry) {
        self.procedures.push(entry);
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_procedure_entry(agent_id: &str, name: &str, trigger: &str) -> MemoryEntry {
        let now = Utc::now();
        MemoryEntry {
            id: Uuid::new_v4(),
            schema_version: 1,
            agent_id: agent_id.into(),
            memory_type: MemoryType::Procedural,
            epistemic_class: EpistemicClass::LearnedBehavior {
                evidence_task_ids: vec!["t1".into()],
                success_rate: 0.9,
            },
            validation_state: ValidationState::Corroborated,
            content: MemoryContent::Procedure {
                name: name.into(),
                description: format!("procedure: {name}"),
                trigger_condition: trigger.into(),
                steps: vec![ProcedureStep {
                    order: 1,
                    description: "do the thing".into(),
                    tool: None,
                    expected_outcome: None,
                }],
            },
            embedding: None,
            created_at: now,
            updated_at: now,
            valid_from: now,
            valid_to: None,
            trust_score: 0.9,
            importance: 0.8,
            confidence: 0.9,
            supersedes: None,
            derived_from: vec![],
            source_task_id: None,
            source_conversation_id: None,
            scope: MemoryScope::Agent,
            sensitivity: SensitivityClass::Internal,
            access_count: 0,
            last_accessed: now,
            version: 1,
            ttl: None,
            tags: vec!["promoted".into()],
        }
    }

    fn make_candidate(agent_id: &str, name: &str) -> ProcedureCandidate {
        ProcedureCandidate {
            id: Uuid::new_v4(),
            agent_id: agent_id.into(),
            name: name.into(),
            description: format!("candidate: {name}"),
            trigger_condition: "when needed".into(),
            steps: vec![ProcedureStep {
                order: 1,
                description: "step 1".into(),
                tool: None,
                expected_outcome: None,
            }],
            executions: vec![],
            created_at: Utc::now(),
            state: ProcedureState::Candidate,
        }
    }

    fn make_execution(
        procedure_id: MemoryId,
        task: &str,
        success: bool,
        at: chrono::DateTime<Utc>,
    ) -> ProcedureExecution {
        ProcedureExecution {
            procedure_id,
            task_id: task.into(),
            executed_at: at,
            outcome: if success {
                Outcome::Success {
                    details: "ok".into(),
                }
            } else {
                Outcome::Failure {
                    reason: "failed".into(),
                }
            },
            duration_ms: 100,
        }
    }

    #[test]
    fn add_procedure_and_retrieve() {
        let mut mem = ProceduralMemory::new(10);
        let entry = make_procedure_entry("a", "deploy", "when deploy requested");
        let id = mem.add_procedure(entry).unwrap();
        assert!(mem.get_procedure(id).is_some());
        assert_eq!(mem.procedure_count(), 1);
    }

    #[test]
    fn find_matching_procedures() {
        let mut mem = ProceduralMemory::new(10);
        mem.add_procedure(make_procedure_entry("a", "deploy", "when deploy requested"))
            .unwrap();
        mem.add_procedure(make_procedure_entry("a", "test", "when tests needed"))
            .unwrap();

        let matches = mem.find_matching_procedures("deploy");
        assert_eq!(matches.len(), 1);
    }

    #[test]
    fn quota_enforcement() {
        let mut mem = ProceduralMemory::new(1);
        mem.add_procedure(make_procedure_entry("a", "p1", "t1"))
            .unwrap();
        let result = mem.add_procedure(make_procedure_entry("a", "p2", "t2"));
        assert!(matches!(result, Err(MemoryError::QuotaExceeded { .. })));
    }

    #[test]
    fn add_candidate() {
        let mut mem = ProceduralMemory::new(10);
        let c = make_candidate("a", "candidate-1");
        let id = mem.add_candidate(c).unwrap();
        assert_eq!(mem.candidate_count(), 1);
        assert!(mem.all_candidates().iter().any(|c| c.id == id));
    }

    #[test]
    fn record_executions() {
        let mut mem = ProceduralMemory::new(10);
        let entry = make_procedure_entry("a", "p", "t");
        let id = mem.add_procedure(entry).unwrap();

        for i in 0..5 {
            mem.record_execution(
                id,
                make_execution(id, &format!("task-{i}"), true, Utc::now()),
            );
        }

        assert_eq!(mem.execution_history.get(&id).unwrap().len(), 5);
    }

    #[test]
    fn regression_window_trimming() {
        let mut mem = ProceduralMemory::new(10);
        mem.regression_window = 5;
        let entry = make_procedure_entry("a", "p", "t");
        let id = mem.add_procedure(entry).unwrap();

        for i in 0..10 {
            mem.record_execution(
                id,
                make_execution(id, &format!("task-{i}"), true, Utc::now()),
            );
        }

        assert_eq!(mem.execution_history.get(&id).unwrap().len(), 5);
    }

    #[test]
    fn check_promotion_below_threshold() {
        let mut mem = ProceduralMemory::new(10);
        let mut c = make_candidate("a", "c1");
        // Only 2 successes — not enough for any level
        let now = Utc::now();
        c.executions
            .push(make_execution(Uuid::nil(), "t1", true, now));
        c.executions
            .push(make_execution(Uuid::nil(), "t2", true, now));
        let id = mem.add_candidate(c).unwrap();

        let result = mem.check_promotion(id, 3).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn check_promotion_meets_threshold() {
        let mut mem = ProceduralMemory::new(10);
        let mut c = make_candidate("a", "c1");
        let base = Utc::now() - chrono::Duration::hours(72);
        // 10 successes over 72 hours — meets L3 threshold
        for i in 0..10 {
            let at = base + chrono::Duration::hours(i * 7);
            c.executions
                .push(make_execution(Uuid::nil(), &format!("t{i}"), true, at));
        }
        let id = mem.add_candidate(c).unwrap();

        let result = mem.check_promotion(id, 3).unwrap();
        assert!(result.is_some());
        let evidence = result.unwrap();
        assert_eq!(evidence.success_count, 10);
        assert!((evidence.success_rate - 1.0).abs() < 1e-5);
    }

    #[test]
    fn promote_creates_entry() {
        let mut mem = ProceduralMemory::new(10);
        let c = make_candidate("a", "deploy-flow");
        let id = mem.add_candidate(c).unwrap();

        let evidence = PromotionEvidence {
            evidence_task_ids: vec!["t1".into()],
            success_count: 10,
            total_count: 10,
            success_rate: 1.0,
            evidence_window_start: Utc::now(),
            evidence_window_end: Utc::now(),
            approved_by: PromotionApprover::Automatic { autonomy_level: 3 },
            promoted_at: Utc::now(),
        };

        let entry = mem.promote(id, evidence).unwrap();
        assert_eq!(entry.memory_type, MemoryType::Procedural);
        assert!(matches!(
            entry.epistemic_class,
            EpistemicClass::LearnedBehavior { .. }
        ));
        assert_eq!(mem.candidate_count(), 0);
        assert_eq!(mem.procedure_count(), 1);
    }

    #[test]
    fn regression_high_success_returns_none() {
        let mut mem = ProceduralMemory::new(10);
        let entry = make_procedure_entry("a", "p", "t");
        let id = mem.add_procedure(entry).unwrap();

        for i in 0..10 {
            mem.record_execution(id, make_execution(id, &format!("t{i}"), true, Utc::now()));
        }

        assert!(mem.check_regression(id).is_none());
    }

    #[test]
    fn regression_below_flag_threshold() {
        let mut mem = ProceduralMemory::new(10);
        let entry = make_procedure_entry("a", "p", "t");
        let id = mem.add_procedure(entry).unwrap();

        // 5 success + 5 failure = 50% < 60% flag threshold
        for i in 0..10 {
            mem.record_execution(id, make_execution(id, &format!("t{i}"), i < 5, Utc::now()));
        }

        let event = mem.check_regression(id).unwrap();
        assert!(matches!(
            event.trigger,
            RegressionTrigger::SuccessRateDroppedBelowFlag
        ));
        assert!(matches!(event.action_taken, RegressionAction::Flagged));
    }

    #[test]
    fn regression_below_demote_threshold() {
        let mut mem = ProceduralMemory::new(10);
        let entry = make_procedure_entry("a", "p", "t");
        let id = mem.add_procedure(entry).unwrap();

        // 3 success + 7 failure = 30% < 40% demote threshold
        for i in 0..10 {
            mem.record_execution(id, make_execution(id, &format!("t{i}"), i < 3, Utc::now()));
        }

        let event = mem.check_regression(id).unwrap();
        assert!(matches!(
            event.trigger,
            RegressionTrigger::SuccessRateDroppedBelowDemote
        ));
        assert!(matches!(
            event.action_taken,
            RegressionAction::Demoted { .. }
        ));
    }

    #[test]
    fn regression_stale_unused() {
        let mut mem = ProceduralMemory::new(10);
        let mut entry = make_procedure_entry("a", "p", "t");
        entry.last_accessed = Utc::now() - chrono::Duration::days(45);
        let id = mem.add_procedure(entry).unwrap();

        // No executions + last_accessed > 30 days
        let event = mem.check_regression(id).unwrap();
        assert!(matches!(
            event.trigger,
            RegressionTrigger::StaleUnused { .. }
        ));
    }

    #[test]
    fn demote_removes_procedure() {
        let mut mem = ProceduralMemory::new(10);
        let entry = make_procedure_entry("a", "p", "t");
        let id = mem.add_procedure(entry).unwrap();
        assert_eq!(mem.procedure_count(), 1);

        let demoted = mem.demote(id, "regression").unwrap();
        assert_eq!(demoted.validation_state, ValidationState::Deprecated);
        assert_eq!(mem.procedure_count(), 0);
    }

    #[test]
    fn flag_sets_contested() {
        let mut mem = ProceduralMemory::new(10);
        let entry = make_procedure_entry("a", "p", "t");
        let id = mem.add_procedure(entry).unwrap();

        mem.flag(id).unwrap();
        let flagged = mem.get_procedure(id).unwrap();
        assert_eq!(flagged.validation_state, ValidationState::Contested);
    }

    #[test]
    fn promotion_thresholds_l0_l2_return_none() {
        assert!(PromotionThresholds::for_autonomy_level(0).is_none());
        assert!(PromotionThresholds::for_autonomy_level(1).is_none());
        assert!(PromotionThresholds::for_autonomy_level(2).is_none());
    }

    #[test]
    fn promotion_thresholds_l3() {
        let t = PromotionThresholds::for_autonomy_level(3).unwrap();
        assert_eq!(t.min_successes, 10);
        assert!((t.min_success_rate - 0.9).abs() < 1e-5);
    }

    #[test]
    fn promotion_thresholds_l4_l5() {
        let t = PromotionThresholds::for_autonomy_level(4).unwrap();
        assert_eq!(t.min_successes, 5);
        assert!((t.min_success_rate - 0.8).abs() < 1e-5);
    }

    #[test]
    fn promotion_thresholds_l6() {
        let t = PromotionThresholds::for_autonomy_level(6).unwrap();
        assert_eq!(t.min_successes, 3);
        assert!((t.min_success_rate - 0.7).abs() < 1e-5);
    }

    #[test]
    fn rejects_non_procedural_content() {
        let mut mem = ProceduralMemory::new(10);
        let now = Utc::now();
        let entry = MemoryEntry {
            id: Uuid::new_v4(),
            schema_version: 1,
            agent_id: "a".into(),
            memory_type: MemoryType::Procedural,
            epistemic_class: EpistemicClass::Observation,
            validation_state: ValidationState::Unverified,
            content: MemoryContent::Context {
                key: "k".into(),
                value: serde_json::json!(1),
            },
            embedding: None,
            created_at: now,
            updated_at: now,
            valid_from: now,
            valid_to: None,
            trust_score: 0.5,
            importance: 0.5,
            confidence: 0.5,
            supersedes: None,
            derived_from: vec![],
            source_task_id: None,
            source_conversation_id: None,
            scope: MemoryScope::Agent,
            sensitivity: SensitivityClass::Internal,
            access_count: 0,
            last_accessed: now,
            version: 1,
            ttl: None,
            tags: vec![],
        };

        assert!(matches!(
            mem.add_procedure(entry),
            Err(MemoryError::TypeMismatch { .. })
        ));
    }
}
