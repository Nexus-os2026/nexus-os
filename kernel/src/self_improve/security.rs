//! Layer 4: Security Evolution — evolves threat detection rules by learning
//! from false positives and false negatives.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

// ── Types ───────────────────────────────────────────────────────────────────

/// A security event that was either correctly or incorrectly classified.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityEvent {
    pub event_id: String,
    pub rule_id: String,
    pub description: String,
    pub input_sample: String,
    pub was_blocked: bool,
    pub timestamp: u64,
}

/// Accuracy tracking for a single detection rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RulePerformance {
    pub rule_id: String,
    pub true_positives: u64,
    pub false_positives: u64,
    pub true_negatives: u64,
    pub false_negatives: u64,
    pub accuracy: f64,
    pub last_updated: u64,
}

impl RulePerformance {
    pub fn new(rule_id: impl Into<String>) -> Self {
        Self {
            rule_id: rule_id.into(),
            true_positives: 0,
            false_positives: 0,
            true_negatives: 0,
            false_negatives: 0,
            accuracy: 1.0,
            last_updated: epoch_secs(),
        }
    }

    pub fn recalculate_accuracy(&mut self) {
        let total =
            self.true_positives + self.false_positives + self.true_negatives + self.false_negatives;
        if total == 0 {
            self.accuracy = 1.0;
        } else {
            self.accuracy = (self.true_positives + self.true_negatives) as f64 / total as f64;
        }
        self.last_updated = epoch_secs();
    }
}

/// Summary of security evolution state for the UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityEvolutionReport {
    pub total_false_positives: u64,
    pub total_false_negatives: u64,
    pub rules_tracked: usize,
    pub weak_rules: Vec<RulePerformance>,
    pub overall_accuracy: f64,
}

// ── SecurityEvolver ─────────────────────────────────────────────────────────

/// Evolves detection rules based on observed accuracy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityEvolver {
    false_positives: Vec<SecurityEvent>,
    false_negatives: Vec<SecurityEvent>,
    rule_accuracy: HashMap<String, RulePerformance>,
    accuracy_threshold: f64,
    max_events: usize,
}

impl SecurityEvolver {
    pub fn new() -> Self {
        Self {
            false_positives: Vec::new(),
            false_negatives: Vec::new(),
            rule_accuracy: HashMap::new(),
            accuracy_threshold: 0.80,
            max_events: 500,
        }
    }

    /// User overrides a block → the rule was too aggressive (false positive).
    pub fn record_false_positive(&mut self, rule_id: &str, event: SecurityEvent) {
        let perf = self
            .rule_accuracy
            .entry(rule_id.to_string())
            .or_insert_with(|| RulePerformance::new(rule_id));
        perf.false_positives += 1;
        perf.recalculate_accuracy();

        self.false_positives.push(event);
        if self.false_positives.len() > self.max_events {
            self.false_positives.remove(0);
        }
    }

    /// A threat was detected after the fact → the rules missed it (false negative).
    pub fn record_false_negative(&mut self, rule_id: &str, event: SecurityEvent) {
        let perf = self
            .rule_accuracy
            .entry(rule_id.to_string())
            .or_insert_with(|| RulePerformance::new(rule_id));
        perf.false_negatives += 1;
        perf.recalculate_accuracy();

        self.false_negatives.push(event);
        if self.false_negatives.len() > self.max_events {
            self.false_negatives.remove(0);
        }
    }

    /// Record a correct detection (true positive).
    pub fn record_true_positive(&mut self, rule_id: &str) {
        let perf = self
            .rule_accuracy
            .entry(rule_id.to_string())
            .or_insert_with(|| RulePerformance::new(rule_id));
        perf.true_positives += 1;
        perf.recalculate_accuracy();
    }

    /// Record a correct pass-through (true negative).
    pub fn record_true_negative(&mut self, rule_id: &str) {
        let perf = self
            .rule_accuracy
            .entry(rule_id.to_string())
            .or_insert_with(|| RulePerformance::new(rule_id));
        perf.true_negatives += 1;
        perf.recalculate_accuracy();
    }

    /// Get rules with accuracy below threshold — candidates for evolution.
    pub fn weak_rules(&self) -> Vec<&RulePerformance> {
        self.rule_accuracy
            .values()
            .filter(|r| r.accuracy < self.accuracy_threshold)
            .collect()
    }

    /// Get accuracy for a specific rule.
    pub fn rule_accuracy(&self, rule_id: &str) -> Option<&RulePerformance> {
        self.rule_accuracy.get(rule_id)
    }

    /// Get overall accuracy across all rules.
    pub fn overall_accuracy(&self) -> f64 {
        if self.rule_accuracy.is_empty() {
            return 1.0;
        }
        let total: f64 = self.rule_accuracy.values().map(|r| r.accuracy).sum();
        total / self.rule_accuracy.len() as f64
    }

    /// Generate a full security evolution report.
    pub fn report(&self) -> SecurityEvolutionReport {
        SecurityEvolutionReport {
            total_false_positives: self.false_positives.len() as u64,
            total_false_negatives: self.false_negatives.len() as u64,
            rules_tracked: self.rule_accuracy.len(),
            weak_rules: self
                .rule_accuracy
                .values()
                .filter(|r| r.accuracy < self.accuracy_threshold)
                .cloned()
                .collect(),
            overall_accuracy: self.overall_accuracy(),
        }
    }

    /// Get recent false positives for analysis.
    pub fn recent_false_positives(&self, limit: usize) -> Vec<&SecurityEvent> {
        self.false_positives.iter().rev().take(limit).collect()
    }

    /// Get recent false negatives for analysis.
    pub fn recent_false_negatives(&self, limit: usize) -> Vec<&SecurityEvent> {
        self.false_negatives.iter().rev().take(limit).collect()
    }

    /// Set the accuracy threshold below which rules are considered weak.
    pub fn set_accuracy_threshold(&mut self, threshold: f64) {
        self.accuracy_threshold = threshold.clamp(0.0, 1.0);
    }
}

impl Default for SecurityEvolver {
    fn default() -> Self {
        Self::new()
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_event(rule_id: &str, desc: &str) -> SecurityEvent {
        SecurityEvent {
            event_id: "evt-1".to_string(),
            rule_id: rule_id.to_string(),
            description: desc.to_string(),
            input_sample: "test input".to_string(),
            was_blocked: true,
            timestamp: 0,
        }
    }

    #[test]
    fn false_positive_reduces_accuracy() {
        let mut evolver = SecurityEvolver::new();
        // Record some true positives first
        for _ in 0..8 {
            evolver.record_true_positive("rule-ip");
        }
        assert!(evolver.rule_accuracy("rule-ip").unwrap().accuracy > 0.9);

        // Now 5 false positives
        for _ in 0..5 {
            evolver
                .record_false_positive("rule-ip", make_event("rule-ip", "numbered list blocked"));
        }

        let perf = evolver.rule_accuracy("rule-ip").unwrap();
        assert!(perf.accuracy < 0.8, "accuracy should drop below threshold");
        assert_eq!(perf.false_positives, 5);
    }

    #[test]
    fn weak_rules_identified() {
        let mut evolver = SecurityEvolver::new();
        // Good rule
        for _ in 0..10 {
            evolver.record_true_positive("good-rule");
        }
        // Bad rule — all false positives
        for _ in 0..10 {
            evolver.record_false_positive("bad-rule", make_event("bad-rule", "false block"));
        }

        let weak = evolver.weak_rules();
        assert_eq!(weak.len(), 1);
        assert_eq!(weak[0].rule_id, "bad-rule");
    }

    #[test]
    fn overall_accuracy_average() {
        let mut evolver = SecurityEvolver::new();
        // Rule A: 100% accurate
        for _ in 0..10 {
            evolver.record_true_positive("rule-a");
        }
        // Rule B: 0% accurate (all false positives)
        for _ in 0..10 {
            evolver.record_false_positive("rule-b", make_event("rule-b", "bad"));
        }

        let acc = evolver.overall_accuracy();
        assert!((acc - 0.5).abs() < 0.01);
    }

    #[test]
    fn report_complete() {
        let mut evolver = SecurityEvolver::new();
        evolver.record_false_positive("rule-1", make_event("rule-1", "fp"));
        evolver.record_false_negative("rule-2", make_event("rule-2", "fn"));

        let report = evolver.report();
        assert_eq!(report.total_false_positives, 1);
        assert_eq!(report.total_false_negatives, 1);
        assert_eq!(report.rules_tracked, 2);
    }

    #[test]
    fn events_bounded() {
        let mut evolver = SecurityEvolver::new();
        evolver.max_events = 5;
        for i in 0..10 {
            evolver.record_false_positive("rule-1", make_event("rule-1", &format!("event {i}")));
        }
        assert_eq!(evolver.false_positives.len(), 5);
    }
}
