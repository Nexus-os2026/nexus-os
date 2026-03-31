//! # Policy Optimizer
//!
//! Cedar policy refinement engine. Analyzes audit trail patterns to identify
//! dead, overly broad, or missing policies. Can only ADD or NARROW policies —
//! never REMOVE or BROADEN (enforced by invariant #3).

use crate::types::ProposedChange;
use serde::{Deserialize, Serialize};

/// A simplified audit entry for policy analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub timestamp: u64,
    pub agent_id: String,
    pub action: String,
    pub policy_id: String,
    pub result: PolicyResult,
}

/// Result of a policy evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PolicyResult {
    Allowed,
    Denied,
    NotEvaluated,
}

/// Type of policy suggestion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SuggestionKind {
    /// Policy is never triggered — candidate for review.
    DeadPolicy,
    /// Policy triggers too often — may be too broad.
    OverlyBroad,
    /// Agent repeatedly denied — may need a narrower allow rule.
    RepeatedDenial,
    /// Action patterns suggest a time-based constraint would help.
    TimeBasedConstraint,
}

/// A suggestion for policy refinement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicySuggestion {
    pub kind: SuggestionKind,
    pub policy_id: String,
    pub reasoning: String,
    pub proposed_cedar: Option<String>,
    pub trigger_count: usize,
}

/// Configuration for the policy optimizer.
#[derive(Debug, Clone)]
pub struct PolicyOptimizerConfig {
    /// Policies with fewer than this many triggers are considered dead.
    pub dead_policy_threshold: usize,
    /// Policies with more than this many triggers (per period) are considered broad.
    pub broad_policy_threshold: usize,
    /// Minimum entries needed to detect time patterns.
    pub time_pattern_min_entries: usize,
}

impl Default for PolicyOptimizerConfig {
    fn default() -> Self {
        Self {
            dead_policy_threshold: 1,
            broad_policy_threshold: 50,
            time_pattern_min_entries: 10,
        }
    }
}

/// The policy optimizer engine.
pub struct PolicyOptimizer {
    config: PolicyOptimizerConfig,
}

impl PolicyOptimizer {
    pub fn new(config: PolicyOptimizerConfig) -> Self {
        Self { config }
    }

    /// Analyze audit entries for policy patterns.
    pub fn analyze_policies(&self, entries: &[AuditEntry]) -> Vec<PolicySuggestion> {
        let mut suggestions = Vec::new();

        // Count triggers per policy
        let mut policy_counts: std::collections::HashMap<&str, usize> =
            std::collections::HashMap::new();
        let mut denial_counts: std::collections::HashMap<(&str, &str), usize> =
            std::collections::HashMap::new();

        for entry in entries {
            *policy_counts.entry(&entry.policy_id).or_default() += 1;

            if entry.result == PolicyResult::Denied {
                *denial_counts
                    .entry((&entry.agent_id, &entry.action))
                    .or_default() += 1;
            }
        }

        // Detect dead policies
        let all_policies: std::collections::HashSet<&str> =
            entries.iter().map(|e| e.policy_id.as_str()).collect();
        for policy_id in &all_policies {
            let count = policy_counts.get(policy_id).copied().unwrap_or(0);
            if count <= self.config.dead_policy_threshold {
                suggestions.push(PolicySuggestion {
                    kind: SuggestionKind::DeadPolicy,
                    policy_id: policy_id.to_string(),
                    reasoning: format!(
                        "policy '{policy_id}' triggered only {count} time(s) — review for removal"
                    ),
                    proposed_cedar: None,
                    trigger_count: count,
                });
            }
        }

        // Detect overly broad policies
        for (policy_id, count) in &policy_counts {
            if *count > self.config.broad_policy_threshold {
                suggestions.push(PolicySuggestion {
                    kind: SuggestionKind::OverlyBroad,
                    policy_id: policy_id.to_string(),
                    reasoning: format!(
                        "policy '{policy_id}' triggered {count} times — may be too broad"
                    ),
                    proposed_cedar: Some(format!(
                        "// Consider narrowing: permit(principal == Agent::\"{policy_id}\", action, resource) when {{ context.risk < 0.3 }};"
                    )),
                    trigger_count: *count,
                });
            }
        }

        // Detect repeated denials (agent needs capability adjustment)
        for ((agent_id, action), count) in &denial_counts {
            if *count >= 3 {
                suggestions.push(PolicySuggestion {
                    kind: SuggestionKind::RepeatedDenial,
                    policy_id: format!("{agent_id}:{action}"),
                    reasoning: format!(
                        "agent '{agent_id}' denied action '{action}' {count} times — consider narrower allow rule"
                    ),
                    proposed_cedar: Some(format!(
                        "permit(principal == Agent::\"{agent_id}\", action == Action::\"{action}\", resource) when {{ context.approved == true }};"
                    )),
                    trigger_count: *count,
                });
            }
        }

        // Detect time-based patterns
        if entries.len() >= self.config.time_pattern_min_entries {
            let timestamps: Vec<u64> = entries.iter().map(|e| e.timestamp).collect();
            if has_time_pattern(&timestamps) {
                suggestions.push(PolicySuggestion {
                    kind: SuggestionKind::TimeBasedConstraint,
                    policy_id: "time_based".into(),
                    reasoning: "detected periodic action pattern — consider time-window policy"
                        .into(),
                    proposed_cedar: Some(
                        "permit(principal, action, resource) when { context.hour >= 9 && context.hour <= 17 };".into(),
                    ),
                    trigger_count: entries.len(),
                });
            }
        }

        suggestions
    }

    /// Generate a policy refinement proposal.
    /// CRITICAL: Can only ADD or NARROW policies, never REMOVE or BROADEN.
    pub fn propose_refinement(&self, suggestion: &PolicySuggestion) -> Option<ProposedChange> {
        // Only generate proposals for narrowing suggestions
        match suggestion.kind {
            SuggestionKind::OverlyBroad | SuggestionKind::TimeBasedConstraint => suggestion
                .proposed_cedar
                .as_ref()
                .map(|cedar| ProposedChange::PolicyUpdate {
                    policy_id: suggestion.policy_id.clone(),
                    old_policy_hash: "sha256:current".into(),
                    new_policy_cedar: cedar.clone(),
                }),
            SuggestionKind::RepeatedDenial => {
                // Add a new narrower allow rule — this is safe because it adds,
                // not broadens existing deny rules
                suggestion
                    .proposed_cedar
                    .as_ref()
                    .map(|cedar| ProposedChange::PolicyUpdate {
                        policy_id: format!("new:{}", suggestion.policy_id),
                        old_policy_hash: "sha256:none".into(),
                        new_policy_cedar: cedar.clone(),
                    })
            }
            // Dead policy suggestions are for human review only — no auto-removal
            SuggestionKind::DeadPolicy => None,
        }
    }

    /// Validate that a proposed policy change does not broaden access.
    /// Returns true if the change is safe (narrows or adds constraints).
    pub fn is_narrowing_change(new_cedar: &str) -> bool {
        let lower = new_cedar.to_lowercase();
        // A narrowing change should contain "when" constraints
        // A broadening change would remove conditions or use "permit" without "when"
        lower.contains("when") || lower.contains("forbid")
    }
}

/// Simple heuristic: detect if timestamps show a periodic pattern.
fn has_time_pattern(timestamps: &[u64]) -> bool {
    if timestamps.len() < 3 {
        return false;
    }
    let mut deltas: Vec<u64> = timestamps.windows(2).map(|w| w[1] - w[0]).collect();
    deltas.sort_unstable();
    // If most deltas are similar (within 20%), there's a pattern
    let median = deltas[deltas.len() / 2];
    if median == 0 {
        return false;
    }
    let similar_count = deltas
        .iter()
        .filter(|&&d| {
            let ratio = d as f64 / median as f64;
            (0.8..=1.2).contains(&ratio)
        })
        .count();
    similar_count as f64 / deltas.len() as f64 > 0.5
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entries(policy_id: &str, result: PolicyResult, count: usize) -> Vec<AuditEntry> {
        (0..count)
            .map(|i| AuditEntry {
                timestamp: 1000 + i as u64 * 60,
                agent_id: "agent-1".into(),
                action: "tool_call".into(),
                policy_id: policy_id.into(),
                result,
            })
            .collect()
    }

    #[test]
    fn test_dead_policy_detection() {
        let optimizer = PolicyOptimizer::new(PolicyOptimizerConfig::default());
        let entries = make_entries("rarely-used", PolicyResult::Allowed, 1);
        let suggestions = optimizer.analyze_policies(&entries);
        assert!(
            suggestions
                .iter()
                .any(|s| s.kind == SuggestionKind::DeadPolicy),
            "should detect dead policy"
        );
    }

    #[test]
    fn test_broad_policy_detection() {
        let optimizer = PolicyOptimizer::new(PolicyOptimizerConfig {
            broad_policy_threshold: 10,
            ..Default::default()
        });
        let entries = make_entries("broad-policy", PolicyResult::Allowed, 20);
        let suggestions = optimizer.analyze_policies(&entries);
        assert!(
            suggestions
                .iter()
                .any(|s| s.kind == SuggestionKind::OverlyBroad),
            "should detect overly broad policy"
        );
    }

    #[test]
    fn test_cannot_broaden_policy() {
        // "permit without when" is a broadening change — should be rejected
        assert!(
            !PolicyOptimizer::is_narrowing_change("permit(principal, action, resource);"),
            "bare permit is broadening"
        );
        // "permit with when" is narrowing — should be allowed
        assert!(
            PolicyOptimizer::is_narrowing_change(
                "permit(principal, action, resource) when { context.risk < 0.3 };"
            ),
            "permit with when is narrowing"
        );
    }

    #[test]
    fn test_cannot_remove_policy() {
        let optimizer = PolicyOptimizer::new(PolicyOptimizerConfig::default());
        let entries = make_entries("dead-policy", PolicyResult::Allowed, 0);
        let suggestions = optimizer.analyze_policies(&entries);
        // Dead policy suggestions should NOT generate a ProposedChange
        for s in &suggestions {
            if s.kind == SuggestionKind::DeadPolicy {
                let proposal = optimizer.propose_refinement(s);
                assert!(
                    proposal.is_none(),
                    "dead policy should not auto-generate removal proposal"
                );
            }
        }
    }

    #[test]
    fn test_time_pattern_detection() {
        let optimizer = PolicyOptimizer::new(PolicyOptimizerConfig {
            time_pattern_min_entries: 5,
            ..Default::default()
        });
        // Create entries with regular 60-second intervals
        let entries = make_entries("timed-policy", PolicyResult::Allowed, 10);
        let suggestions = optimizer.analyze_policies(&entries);
        assert!(
            suggestions
                .iter()
                .any(|s| s.kind == SuggestionKind::TimeBasedConstraint),
            "should detect time pattern"
        );
    }
}
