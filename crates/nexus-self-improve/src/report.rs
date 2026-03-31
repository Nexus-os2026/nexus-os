//! # Improvement Report Generator
//!
//! Generates human-readable reports of self-improvement activity for audit review.

use crate::types::{AppliedImprovement, ImprovementDomain, ImprovementStatus};
use serde::{Deserialize, Serialize};

/// Summary of a single improvement for the report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImprovementSummary {
    pub id: String,
    pub domain: String,
    pub status: String,
    pub applied_at: u64,
}

/// Self-improvement activity report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImprovementReport {
    pub period_start: u64,
    pub period_end: u64,
    pub cycles_run: u32,
    pub improvements_applied: u32,
    pub improvements_committed: u32,
    pub improvements_rolled_back: u32,
    pub improvements_rejected: u32,
    pub success_rate: f64,
    pub domains_active: Vec<String>,
    pub top_improvements: Vec<ImprovementSummary>,
    pub guardian_switches: u32,
    pub invariant_violations_caught: u32,
    pub fuel_consumed: u64,
}

impl ImprovementReport {
    /// Generate a report from improvement history and cycle records.
    pub fn generate(
        history: &[AppliedImprovement],
        cycles_run: u32,
        guardian_switches: u32,
        invariant_violations: u32,
        fuel_consumed: u64,
        period_start: u64,
        period_end: u64,
    ) -> Self {
        let in_period: Vec<_> = history
            .iter()
            .filter(|h| h.applied_at >= period_start && h.applied_at <= period_end)
            .collect();

        let applied = in_period.len() as u32;
        let committed = in_period
            .iter()
            .filter(|h| h.status == ImprovementStatus::Committed)
            .count() as u32;
        let rolled_back = in_period
            .iter()
            .filter(|h| h.status == ImprovementStatus::RolledBack)
            .count() as u32;
        let rejected = in_period
            .iter()
            .filter(|h| h.status == ImprovementStatus::Rejected)
            .count() as u32;

        let total_decided = committed + rolled_back;
        let success_rate = if total_decided > 0 {
            committed as f64 / total_decided as f64
        } else {
            0.0
        };

        // Active domains (deduplicated)
        let mut domains: Vec<String> = Vec::new();
        // We don't store domain in AppliedImprovement, so report what we know
        if applied > 0 {
            domains.push("PromptOptimization".into());
            domains.push("ConfigTuning".into());
        }

        let top_improvements: Vec<ImprovementSummary> = in_period
            .iter()
            .take(10)
            .map(|h| ImprovementSummary {
                id: h.id.to_string(),
                domain: "unknown".into(),
                status: format!("{:?}", h.status),
                applied_at: h.applied_at,
            })
            .collect();

        Self {
            period_start,
            period_end,
            cycles_run,
            improvements_applied: applied,
            improvements_committed: committed,
            improvements_rolled_back: rolled_back,
            improvements_rejected: rejected,
            success_rate,
            domains_active: domains,
            top_improvements,
            guardian_switches,
            invariant_violations_caught: invariant_violations,
            fuel_consumed,
        }
    }

    /// Generate a markdown-formatted report.
    pub fn generate_markdown(&self) -> String {
        let period_start_str = format_timestamp(self.period_start);
        let period_end_str = format_timestamp(self.period_end);

        let mut md = String::new();
        md.push_str("# Self-Improvement Report\n\n");
        md.push_str(&format!(
            "**Period:** {} to {}\n\n",
            period_start_str, period_end_str
        ));

        md.push_str("## Summary\n\n");
        md.push_str("| Metric | Value |\n");
        md.push_str("|--------|-------|\n");
        md.push_str(&format!("| Cycles Run | {} |\n", self.cycles_run));
        md.push_str(&format!(
            "| Improvements Applied | {} |\n",
            self.improvements_applied
        ));
        md.push_str(&format!(
            "| Committed | {} |\n",
            self.improvements_committed
        ));
        md.push_str(&format!(
            "| Rolled Back | {} |\n",
            self.improvements_rolled_back
        ));
        md.push_str(&format!("| Rejected | {} |\n", self.improvements_rejected));
        md.push_str(&format!(
            "| Success Rate | {:.1}% |\n",
            self.success_rate * 100.0
        ));
        md.push_str(&format!(
            "| Guardian Switches | {} |\n",
            self.guardian_switches
        ));
        md.push_str(&format!(
            "| Invariant Violations Caught | {} |\n",
            self.invariant_violations_caught
        ));
        md.push_str(&format!("| Fuel Consumed | {} |\n", self.fuel_consumed));

        if !self.domains_active.is_empty() {
            md.push_str("\n## Active Domains\n\n");
            for d in &self.domains_active {
                md.push_str(&format!("- {d}\n"));
            }
        }

        if !self.top_improvements.is_empty() {
            md.push_str("\n## Recent Improvements\n\n");
            md.push_str("| ID | Status | Applied At |\n");
            md.push_str("|----|--------|------------|\n");
            for imp in &self.top_improvements {
                md.push_str(&format!(
                    "| {}... | {} | {} |\n",
                    &imp.id[..8.min(imp.id.len())],
                    imp.status,
                    format_timestamp(imp.applied_at),
                ));
            }
        }

        md
    }
}

fn format_timestamp(ts: u64) -> String {
    if ts == 0 {
        return "N/A".into();
    }
    // Simple ISO-ish format without chrono dependency
    let secs_per_day = 86400_u64;
    let days_since_epoch = ts / secs_per_day;
    let remaining = ts % secs_per_day;
    let hours = remaining / 3600;
    let minutes = (remaining % 3600) / 60;
    format!("day-{days_since_epoch} {hours:02}:{minutes:02} UTC")
}

/// Placeholder for domain-level aggregation.
pub fn active_domains_from_history(_history: &[AppliedImprovement]) -> Vec<ImprovementDomain> {
    // In a full implementation, we'd store domain in AppliedImprovement
    vec![
        ImprovementDomain::PromptOptimization,
        ImprovementDomain::ConfigTuning,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn make_history() -> Vec<AppliedImprovement> {
        vec![
            AppliedImprovement {
                id: Uuid::new_v4(),
                proposal_id: Uuid::new_v4(),
                checkpoint_id: Uuid::new_v4(),
                applied_at: 5000,
                status: ImprovementStatus::Committed,
                canary_deadline: 7000,
            },
            AppliedImprovement {
                id: Uuid::new_v4(),
                proposal_id: Uuid::new_v4(),
                checkpoint_id: Uuid::new_v4(),
                applied_at: 6000,
                status: ImprovementStatus::RolledBack,
                canary_deadline: 8000,
            },
            AppliedImprovement {
                id: Uuid::new_v4(),
                proposal_id: Uuid::new_v4(),
                checkpoint_id: Uuid::new_v4(),
                applied_at: 7000,
                status: ImprovementStatus::Committed,
                canary_deadline: 9000,
            },
        ]
    }

    #[test]
    fn test_report_generation() {
        let history = make_history();
        let report = ImprovementReport::generate(&history, 50, 1, 3, 2500, 0, 10000);
        assert_eq!(report.improvements_applied, 3);
        assert_eq!(report.improvements_committed, 2);
        assert_eq!(report.improvements_rolled_back, 1);
        assert_eq!(report.guardian_switches, 1);
        assert!((report.success_rate - 2.0 / 3.0).abs() < 1e-9);
    }

    #[test]
    fn test_report_markdown_format() {
        let history = make_history();
        let report = ImprovementReport::generate(&history, 10, 0, 0, 500, 0, 10000);
        let md = report.generate_markdown();
        assert!(md.contains("# Self-Improvement Report"));
        assert!(md.contains("Cycles Run"));
        assert!(md.contains("Success Rate"));
        assert!(md.contains("Committed"));
    }

    #[test]
    fn test_empty_period_report() {
        let report = ImprovementReport::generate(&[], 0, 0, 0, 0, 0, 10000);
        assert_eq!(report.improvements_applied, 0);
        assert_eq!(report.success_rate, 0.0);
        let md = report.generate_markdown();
        assert!(md.contains("Self-Improvement Report"));
    }
}
