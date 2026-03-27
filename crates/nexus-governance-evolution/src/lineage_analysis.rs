//! Cross-generation attack pattern detection.

use serde::{Deserialize, Serialize};

/// Suspicious lineage pattern.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineageAlert {
    pub agent_id: String,
    pub pattern: SuspicionPattern,
    pub evidence: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SuspicionPattern {
    /// Parent denied for cap X, child later approved for same X.
    ConstraintProbingViaDelegation,
    /// Lineage depth exceeds threshold.
    DeepGenerationSpawning,
}

/// Analyze lineage for suspicious patterns.
pub struct LineageAnalyzer {
    /// Maximum allowed lineage depth before flagging.
    max_depth: u32,
    alerts: Vec<LineageAlert>,
}

impl Default for LineageAnalyzer {
    fn default() -> Self {
        Self::new(5)
    }
}

impl LineageAnalyzer {
    pub fn new(max_depth: u32) -> Self {
        Self {
            max_depth,
            alerts: Vec::new(),
        }
    }

    /// Check for constraint probing via delegation.
    pub fn check_delegation_probing(
        &mut self,
        parent_id: &str,
        child_id: &str,
        capability: &str,
        parent_was_denied: bool,
        child_was_approved: bool,
    ) {
        if parent_was_denied && child_was_approved {
            self.alerts.push(LineageAlert {
                agent_id: child_id.to_string(),
                pattern: SuspicionPattern::ConstraintProbingViaDelegation,
                evidence: format!(
                    "Parent {parent_id} denied for '{capability}', child {child_id} approved for same"
                ),
            });
        }
    }

    /// Check for deep generation spawning.
    pub fn check_lineage_depth(&mut self, agent_id: &str, depth: u32) {
        if depth > self.max_depth {
            self.alerts.push(LineageAlert {
                agent_id: agent_id.to_string(),
                pattern: SuspicionPattern::DeepGenerationSpawning,
                evidence: format!("Lineage depth {depth} exceeds threshold {}", self.max_depth),
            });
        }
    }

    pub fn alerts(&self) -> &[LineageAlert] {
        &self.alerts
    }

    pub fn alert_count(&self) -> usize {
        self.alerts.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lineage_suspicious_delegation() {
        let mut analyzer = LineageAnalyzer::new(5);
        analyzer.check_delegation_probing("parent-1", "child-1", "process.exec", true, true);

        assert_eq!(analyzer.alert_count(), 1);
        assert!(matches!(
            analyzer.alerts()[0].pattern,
            SuspicionPattern::ConstraintProbingViaDelegation
        ));
    }

    #[test]
    fn test_lineage_deep_generation_flagged() {
        let mut analyzer = LineageAnalyzer::new(5);
        analyzer.check_lineage_depth("deep-agent", 8);

        assert_eq!(analyzer.alert_count(), 1);
        assert!(matches!(
            analyzer.alerts()[0].pattern,
            SuspicionPattern::DeepGenerationSpawning
        ));
    }

    #[test]
    fn test_legitimate_delegation_not_flagged() {
        let mut analyzer = LineageAnalyzer::new(5);
        // Parent denied, child also denied — not suspicious
        analyzer.check_delegation_probing("parent-1", "child-1", "process.exec", true, false);
        assert_eq!(analyzer.alert_count(), 0);

        // Normal depth
        analyzer.check_lineage_depth("normal-agent", 3);
        assert_eq!(analyzer.alert_count(), 0);
    }
}
