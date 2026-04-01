//! SWE-bench evaluation harness and benchmarking infrastructure.

pub mod data_pipeline;
pub mod harness;
pub mod report;
pub mod swe_bench;

use serde::{Deserialize, Serialize};

/// Result of a single benchmark task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    pub task_id: String,
    pub success: bool,
    pub patch: String,
    pub turns: u32,
    pub fuel_consumed: u64,
    pub time_secs: f64,
    pub tools_used: Vec<String>,
    pub audit_entries: usize,
    pub error: Option<String>,
}

/// Aggregate benchmark results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkReport {
    pub total_tasks: usize,
    pub passed: usize,
    pub failed: usize,
    pub errored: usize,
    pub pass_rate: f64,
    pub avg_turns: f64,
    pub avg_fuel: f64,
    pub avg_time_secs: f64,
    pub total_time_secs: f64,
    pub provider: String,
    pub model: String,
    pub results: Vec<TaskResult>,
}

/// Governance metrics aggregated across a benchmark run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceMetrics {
    pub avg_fuel_per_task: f64,
    pub avg_audit_entries_per_task: f64,
    pub avg_tools_per_task: f64,
    pub total_fuel: u64,
    pub total_audit_entries: usize,
    pub tool_usage_distribution: std::collections::HashMap<String, usize>,
}

impl GovernanceMetrics {
    pub fn from_results(results: &[TaskResult]) -> Self {
        let n = results.len().max(1) as f64;
        let total_fuel: u64 = results.iter().map(|r| r.fuel_consumed).sum();
        let total_audit: usize = results.iter().map(|r| r.audit_entries).sum();

        let mut tool_dist: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        for result in results {
            for tool in &result.tools_used {
                *tool_dist.entry(tool.clone()).or_insert(0) += 1;
            }
        }

        let avg_tools: f64 = results
            .iter()
            .map(|r| r.tools_used.len() as f64)
            .sum::<f64>()
            / n;

        Self {
            avg_fuel_per_task: total_fuel as f64 / n,
            avg_audit_entries_per_task: total_audit as f64 / n,
            avg_tools_per_task: avg_tools,
            total_fuel,
            total_audit_entries: total_audit,
            tool_usage_distribution: tool_dist,
        }
    }
}
