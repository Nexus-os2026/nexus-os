//! Benchmark data pipeline — generates paper-ready tables from results.

use super::{BenchmarkReport, GovernanceMetrics};
use serde::Serialize;

/// Complete paper data package.
#[derive(Debug, Clone, Serialize)]
pub struct PaperDataPackage {
    pub pass_rate_table: Vec<PassRateRow>,
    pub overhead_table: crate::governance_metrics::AggregateGovernanceMetrics,
    pub provider_comparison: Vec<ProviderComparisonRow>,
    pub tool_usage: Vec<ToolUsageRow>,
    pub fuel_distribution: Vec<u64>,
    pub turns_distribution: Vec<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PassRateRow {
    pub provider: String,
    pub model: String,
    pub tasks: usize,
    pub passed: usize,
    pub pass_rate: f64,
    pub avg_fuel: f64,
    pub avg_turns: f64,
    pub avg_time_secs: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderComparisonRow {
    pub provider: String,
    pub model: String,
    pub pass_rate: f64,
    pub avg_fuel: f64,
    pub avg_audit_entries: f64,
    pub avg_tools_per_task: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolUsageRow {
    pub tool_name: String,
    pub usage_count: usize,
    pub usage_percentage: f64,
}

impl PaperDataPackage {
    /// Generate from benchmark reports.
    pub fn from_reports(reports: &[BenchmarkReport]) -> Self {
        let pass_rate_table: Vec<PassRateRow> = reports
            .iter()
            .map(|r| PassRateRow {
                provider: r.provider.clone(),
                model: r.model.clone(),
                tasks: r.total_tasks,
                passed: r.passed,
                pass_rate: r.pass_rate,
                avg_fuel: r.avg_fuel,
                avg_turns: r.avg_turns,
                avg_time_secs: r.avg_time_secs,
            })
            .collect();

        let provider_comparison: Vec<ProviderComparisonRow> = reports
            .iter()
            .map(|r| {
                let metrics = GovernanceMetrics::from_results(&r.results);
                ProviderComparisonRow {
                    provider: r.provider.clone(),
                    model: r.model.clone(),
                    pass_rate: r.pass_rate,
                    avg_fuel: metrics.avg_fuel_per_task,
                    avg_audit_entries: metrics.avg_audit_entries_per_task,
                    avg_tools_per_task: metrics.avg_tools_per_task,
                }
            })
            .collect();

        // Aggregate tool usage
        let mut tool_counts: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        let mut total_tools = 0usize;
        for report in reports {
            let metrics = GovernanceMetrics::from_results(&report.results);
            for (tool, count) in &metrics.tool_usage_distribution {
                *tool_counts.entry(tool.clone()).or_insert(0) += count;
                total_tools += count;
            }
        }
        let mut tool_usage: Vec<ToolUsageRow> = tool_counts
            .into_iter()
            .map(|(name, count)| ToolUsageRow {
                tool_name: name,
                usage_count: count,
                usage_percentage: if total_tools > 0 {
                    count as f64 / total_tools as f64 * 100.0
                } else {
                    0.0
                },
            })
            .collect();
        tool_usage.sort_by(|a, b| b.usage_count.cmp(&a.usage_count));

        let fuel_distribution: Vec<u64> = reports
            .iter()
            .flat_map(|r| r.results.iter().map(|t| t.fuel_consumed))
            .collect();
        let turns_distribution: Vec<u32> = reports
            .iter()
            .flat_map(|r| r.results.iter().map(|t| t.turns))
            .collect();

        Self {
            pass_rate_table,
            overhead_table: crate::governance_metrics::AggregateGovernanceMetrics::default(),
            provider_comparison,
            tool_usage,
            fuel_distribution,
            turns_distribution,
        }
    }

    /// Generate LaTeX tables for the paper.
    pub fn to_latex(&self) -> String {
        let mut out = String::new();

        // Table 1: Pass rates
        out.push_str("% Table 1: SWE-bench Pass Rates\n");
        out.push_str("\\begin{table}[h]\n\\centering\n");
        out.push_str("\\caption{SWE-bench Verified Results by Provider}\n");
        out.push_str("\\begin{tabular}{lrrrr}\n\\toprule\n");
        out.push_str(
            "Provider/Model & Tasks & Pass\\% & Avg Fuel & Avg Time (s) \\\\\n\\midrule\n",
        );
        for row in &self.pass_rate_table {
            out.push_str(&format!(
                "{}/{} & {} & {:.1}\\% & {:.0} & {:.1} \\\\\n",
                row.provider,
                row.model,
                row.tasks,
                row.pass_rate * 100.0,
                row.avg_fuel,
                row.avg_time_secs
            ));
        }
        out.push_str("\\bottomrule\n\\end{tabular}\n\\end{table}\n\n");

        // Table 2: Governance overhead
        out.push_str("% Table 2: Governance Overhead\n");
        out.push_str("\\begin{table}[h]\n\\centering\n");
        out.push_str("\\caption{Governance Pipeline Overhead per Tool Invocation}\n");
        out.push_str("\\begin{tabular}{lr}\n\\toprule\n");
        out.push_str("Gate & Avg Time ($\\mu$s) \\\\\n\\midrule\n");
        out.push_str(&format!(
            "Capability ACL Check & {:.0} \\\\\n",
            self.overhead_table.avg_capability_check_us
        ));
        out.push_str(&format!(
            "Fuel Reservation & {:.0} \\\\\n",
            self.overhead_table.avg_fuel_reservation_us
        ));
        out.push_str(&format!(
            "Consent Classification & {:.0} \\\\\n",
            self.overhead_table.avg_consent_classification_us
        ));
        out.push_str(&format!(
            "Audit Recording & {:.0} \\\\\n",
            self.overhead_table.avg_audit_recording_us
        ));
        out.push_str("\\midrule\n");
        out.push_str(&format!(
            "Total Overhead & {:.0} \\\\\n",
            self.overhead_table.avg_governance_overhead_us
        ));
        out.push_str(&format!(
            "Overhead \\% & {:.2}\\% \\\\\n",
            self.overhead_table.avg_overhead_percentage
        ));
        out.push_str("\\bottomrule\n\\end{tabular}\n\\end{table}\n\n");

        // Table 3: Tool usage
        out.push_str("% Table 3: Tool Usage\n");
        out.push_str("\\begin{table}[h]\n\\centering\n");
        out.push_str("\\caption{Tool Usage Distribution}\n");
        out.push_str("\\begin{tabular}{lrr}\n\\toprule\n");
        out.push_str("Tool & Count & \\% \\\\\n\\midrule\n");
        for row in self.tool_usage.iter().take(10) {
            out.push_str(&format!(
                "{} & {} & {:.1}\\% \\\\\n",
                row.tool_name.replace('_', "\\_"),
                row.usage_count,
                row.usage_percentage
            ));
        }
        out.push_str("\\bottomrule\n\\end{tabular}\n\\end{table}\n");

        out
    }

    /// Save as JSON.
    pub fn save(&self, path: &std::path::Path) -> Result<(), crate::error::NxError> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| crate::error::NxError::ConfigError(format!("{}", e)))?;
        std::fs::write(path, json)?;
        Ok(())
    }
}
