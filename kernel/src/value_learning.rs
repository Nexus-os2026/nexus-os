//! HITL value learning — learn from human approve/deny decisions to surface
//! auto-approval suggestions. Suggestions are NEVER auto-applied; they are
//! always returned for human review.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// ── Types ───────────────────────────────────────────────────────────────

/// The human decision on a HITL prompt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DecisionVerdict {
    Approved,
    Denied,
}

/// A recorded human decision with context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HitlDecision {
    pub decision_id: String,
    pub agent_id: String,
    pub action: String,
    pub context: String,
    pub verdict: DecisionVerdict,
    pub reason: Option<String>,
    pub timestamp: u64,
}

/// A detected decision pattern from historical data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionPattern {
    pub pattern_id: String,
    pub action: String,
    pub context: String,
    pub total_decisions: usize,
    pub approval_count: usize,
    pub denial_count: usize,
    pub approval_rate: f64,
}

/// A suggestion for auto-approval based on observed patterns.
/// These are NEVER auto-applied — always returned for human review.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoApprovalSuggestion {
    pub suggestion_id: String,
    pub action: String,
    pub context: String,
    pub confidence: f64,
    pub based_on_decisions: usize,
    pub rationale: String,
    pub suggested_at: u64,
}

/// Summary statistics for the value learner.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValueLearningSummary {
    pub total_decisions: usize,
    pub total_approvals: usize,
    pub total_denials: usize,
    pub overall_approval_rate: f64,
    pub unique_actions: usize,
    pub patterns_found: usize,
}

// ── Engine ──────────────────────────────────────────────────────────────

/// Value learner that analyzes human decisions and surfaces patterns.
pub struct ValueLearner {
    decisions: Vec<HitlDecision>,
    /// Minimum number of decisions required to form a pattern.
    min_sample_size: usize,
}

impl Default for ValueLearner {
    fn default() -> Self {
        Self::new(5)
    }
}

impl ValueLearner {
    /// Create a new learner with the given minimum sample size for patterns.
    pub fn new(min_sample_size: usize) -> Self {
        Self {
            decisions: Vec::new(),
            min_sample_size: min_sample_size.max(1),
        }
    }

    /// Record a human decision.
    pub fn record_decision(&mut self, decision: HitlDecision) {
        self.decisions.push(decision);
    }

    /// Record a decision with individual fields (convenience method).
    pub fn record(
        &mut self,
        agent_id: &str,
        action: &str,
        context: &str,
        verdict: DecisionVerdict,
        reason: Option<String>,
    ) {
        let decision = HitlDecision {
            decision_id: Uuid::new_v4().to_string(),
            agent_id: agent_id.to_string(),
            action: action.to_string(),
            context: context.to_string(),
            verdict,
            reason,
            timestamp: now_secs(),
        };
        self.record_decision(decision);
    }

    /// Analyze recorded decisions and find patterns.
    ///
    /// A pattern is an (action, context) pair with at least `min_sample_size`
    /// decisions. Patterns with >80% approval rate are considered high-confidence.
    pub fn analyze_patterns(&self) -> Vec<DecisionPattern> {
        // Group by (action, context).
        let mut groups: HashMap<(String, String), (usize, usize)> = HashMap::new();

        for d in &self.decisions {
            let key = (d.action.clone(), d.context.clone());
            let entry = groups.entry(key).or_insert((0, 0));
            match d.verdict {
                DecisionVerdict::Approved => entry.0 += 1,
                DecisionVerdict::Denied => entry.1 += 1,
            }
        }

        groups
            .into_iter()
            .filter(|(_, (approvals, denials))| approvals + denials >= self.min_sample_size)
            .map(|((action, context), (approvals, denials))| {
                let total = approvals + denials;
                DecisionPattern {
                    pattern_id: Uuid::new_v4().to_string(),
                    action,
                    context,
                    total_decisions: total,
                    approval_count: approvals,
                    denial_count: denials,
                    approval_rate: approvals as f64 / total as f64,
                }
            })
            .collect()
    }

    /// Suggest auto-approvals based on patterns that exceed the minimum confidence.
    ///
    /// Only patterns with an approval rate above `min_confidence` are suggested.
    /// These suggestions are NEVER auto-applied — they are always returned for
    /// human review and explicit opt-in.
    pub fn suggest_auto_approvals(&self, min_confidence: f64) -> Vec<AutoApprovalSuggestion> {
        let patterns = self.analyze_patterns();
        let now = now_secs();

        patterns
            .into_iter()
            .filter(|p| p.approval_rate >= min_confidence)
            .map(|p| AutoApprovalSuggestion {
                suggestion_id: Uuid::new_v4().to_string(),
                action: p.action.clone(),
                context: p.context.clone(),
                confidence: p.approval_rate,
                based_on_decisions: p.total_decisions,
                rationale: format!(
                    "Action '{}' in context '{}' was approved {}/{} times ({:.1}%)",
                    p.action,
                    p.context,
                    p.approval_count,
                    p.total_decisions,
                    p.approval_rate * 100.0,
                ),
                suggested_at: now,
            })
            .collect()
    }

    /// Get full decision history.
    pub fn get_decision_history(&self) -> &[HitlDecision] {
        &self.decisions
    }

    /// Get decisions for a specific agent.
    pub fn get_agent_decisions(&self, agent_id: &str) -> Vec<&HitlDecision> {
        self.decisions
            .iter()
            .filter(|d| d.agent_id == agent_id)
            .collect()
    }

    /// Get summary statistics.
    pub fn summary(&self) -> ValueLearningSummary {
        let total = self.decisions.len();
        let approvals = self
            .decisions
            .iter()
            .filter(|d| d.verdict == DecisionVerdict::Approved)
            .count();
        let denials = total - approvals;

        let unique_actions: std::collections::HashSet<&str> =
            self.decisions.iter().map(|d| d.action.as_str()).collect();

        let patterns = self.analyze_patterns();

        ValueLearningSummary {
            total_decisions: total,
            total_approvals: approvals,
            total_denials: denials,
            overall_approval_rate: if total > 0 {
                approvals as f64 / total as f64
            } else {
                0.0
            },
            unique_actions: unique_actions.len(),
            patterns_found: patterns.len(),
        }
    }

    /// Total number of recorded decisions.
    pub fn decision_count(&self) -> usize {
        self.decisions.len()
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn record_n(
        learner: &mut ValueLearner,
        action: &str,
        context: &str,
        approvals: usize,
        denials: usize,
    ) {
        for _ in 0..approvals {
            learner.record("agent-1", action, context, DecisionVerdict::Approved, None);
        }
        for _ in 0..denials {
            learner.record("agent-1", action, context, DecisionVerdict::Denied, None);
        }
    }

    #[test]
    fn test_record_decision() {
        let mut learner = ValueLearner::new(5);
        learner.record(
            "agent-1",
            "file_read",
            "workspace",
            DecisionVerdict::Approved,
            None,
        );
        assert_eq!(learner.decision_count(), 1);
        assert_eq!(learner.get_decision_history()[0].action, "file_read");
    }

    #[test]
    fn test_record_decision_struct() {
        let mut learner = ValueLearner::new(5);
        let decision = HitlDecision {
            decision_id: Uuid::new_v4().to_string(),
            agent_id: "a1".to_string(),
            action: "deploy".to_string(),
            context: "production".to_string(),
            verdict: DecisionVerdict::Denied,
            reason: Some("Not ready".to_string()),
            timestamp: now_secs(),
        };
        learner.record_decision(decision);
        assert_eq!(learner.decision_count(), 1);
        assert_eq!(
            learner.get_decision_history()[0].verdict,
            DecisionVerdict::Denied
        );
    }

    #[test]
    fn test_analyze_patterns_below_threshold() {
        let mut learner = ValueLearner::new(5);
        // Only 3 decisions — below min_sample_size of 5.
        record_n(&mut learner, "read", "ctx", 3, 0);
        let patterns = learner.analyze_patterns();
        assert!(patterns.is_empty());
    }

    #[test]
    fn test_analyze_patterns_above_threshold() {
        let mut learner = ValueLearner::new(5);
        record_n(&mut learner, "read", "workspace", 8, 2);

        let patterns = learner.analyze_patterns();
        assert_eq!(patterns.len(), 1);
        assert_eq!(patterns[0].action, "read");
        assert_eq!(patterns[0].total_decisions, 10);
        assert_eq!(patterns[0].approval_count, 8);
        assert_eq!(patterns[0].denial_count, 2);
        assert!((patterns[0].approval_rate - 0.8).abs() < f64::EPSILON);
    }

    #[test]
    fn test_analyze_patterns_multiple() {
        let mut learner = ValueLearner::new(5);
        record_n(&mut learner, "read", "ctx_a", 9, 1);
        record_n(&mut learner, "write", "ctx_b", 2, 8);
        record_n(&mut learner, "delete", "ctx_c", 3, 0); // Below threshold.

        let patterns = learner.analyze_patterns();
        assert_eq!(patterns.len(), 2); // read and write, not delete.
    }

    #[test]
    fn test_suggest_auto_approvals_high_confidence() {
        let mut learner = ValueLearner::new(5);
        record_n(&mut learner, "read", "safe_ctx", 9, 1); // 90% approval.
        record_n(&mut learner, "write", "risky_ctx", 3, 7); // 30% approval.

        let suggestions = learner.suggest_auto_approvals(0.8);
        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0].action, "read");
        assert!(suggestions[0].confidence >= 0.8);
        assert!(!suggestions[0].rationale.is_empty());
    }

    #[test]
    fn test_suggest_auto_approvals_none_qualify() {
        let mut learner = ValueLearner::new(5);
        record_n(&mut learner, "deploy", "prod", 4, 6); // 40% — too low.

        let suggestions = learner.suggest_auto_approvals(0.8);
        assert!(suggestions.is_empty());
    }

    #[test]
    fn test_suggest_auto_approvals_perfect() {
        let mut learner = ValueLearner::new(5);
        record_n(&mut learner, "log_read", "audit", 10, 0); // 100%.

        let suggestions = learner.suggest_auto_approvals(0.8);
        assert_eq!(suggestions.len(), 1);
        assert!((suggestions[0].confidence - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_get_agent_decisions() {
        let mut learner = ValueLearner::new(5);
        learner.record("agent-1", "a", "c", DecisionVerdict::Approved, None);
        learner.record("agent-2", "b", "c", DecisionVerdict::Denied, None);
        learner.record("agent-1", "c", "c", DecisionVerdict::Approved, None);

        assert_eq!(learner.get_agent_decisions("agent-1").len(), 2);
        assert_eq!(learner.get_agent_decisions("agent-2").len(), 1);
        assert_eq!(learner.get_agent_decisions("agent-3").len(), 0);
    }

    #[test]
    fn test_summary() {
        let mut learner = ValueLearner::new(5);
        record_n(&mut learner, "read", "ctx_a", 8, 2);
        record_n(&mut learner, "write", "ctx_b", 3, 7);

        let summary = learner.summary();
        assert_eq!(summary.total_decisions, 20);
        assert_eq!(summary.total_approvals, 11);
        assert_eq!(summary.total_denials, 9);
        assert!((summary.overall_approval_rate - 0.55).abs() < f64::EPSILON);
        assert_eq!(summary.unique_actions, 2);
        assert_eq!(summary.patterns_found, 2);
    }

    #[test]
    fn test_summary_empty() {
        let learner = ValueLearner::new(5);
        let summary = learner.summary();
        assert_eq!(summary.total_decisions, 0);
        assert!(summary.overall_approval_rate.abs() < f64::EPSILON);
        assert_eq!(summary.unique_actions, 0);
    }

    #[test]
    fn test_default_learner() {
        let learner = ValueLearner::default();
        assert_eq!(learner.decision_count(), 0);
        assert_eq!(learner.min_sample_size, 5);
    }

    #[test]
    fn test_min_sample_size_floor() {
        let learner = ValueLearner::new(0);
        assert_eq!(learner.min_sample_size, 1);
    }

    #[test]
    fn test_context_matters_for_patterns() {
        let mut learner = ValueLearner::new(5);
        // Same action, different contexts → separate patterns.
        record_n(&mut learner, "deploy", "staging", 9, 1);
        record_n(&mut learner, "deploy", "production", 1, 9);

        let suggestions = learner.suggest_auto_approvals(0.8);
        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0].context, "staging");
    }
}
