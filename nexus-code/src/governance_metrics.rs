//! Governance overhead instrumentation — measures wall-clock cost of each gate.
//! Lives OUTSIDE src/governance/ (which is frozen).

use serde::{Deserialize, Serialize};

/// Timing measurements for a single governed tool execution.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GovernanceTiming {
    pub capability_check_us: u64,
    pub fuel_reservation_us: u64,
    pub consent_classification_us: u64,
    pub tool_execution_us: u64,
    pub audit_recording_us: u64,
    pub fuel_consumption_us: u64,
    pub total_governance_overhead_us: u64,
    pub total_us: u64,
}

impl GovernanceTiming {
    /// Governance overhead as percentage of total time.
    pub fn overhead_percentage(&self) -> f64 {
        if self.total_us == 0 {
            return 0.0;
        }
        (self.total_governance_overhead_us as f64 / self.total_us as f64) * 100.0
    }
}

/// Aggregated governance metrics across multiple tool executions.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AggregateGovernanceMetrics {
    pub sample_count: usize,
    pub avg_capability_check_us: f64,
    pub avg_fuel_reservation_us: f64,
    pub avg_consent_classification_us: f64,
    pub avg_tool_execution_us: f64,
    pub avg_audit_recording_us: f64,
    pub avg_fuel_consumption_us: f64,
    pub avg_governance_overhead_us: f64,
    pub avg_total_us: f64,
    pub avg_overhead_percentage: f64,
    pub p50_overhead_us: u64,
    pub p95_overhead_us: u64,
    pub p99_overhead_us: u64,
    pub max_overhead_us: u64,
}

impl AggregateGovernanceMetrics {
    /// Compute aggregate metrics from individual timings.
    pub fn from_timings(timings: &[GovernanceTiming]) -> Self {
        if timings.is_empty() {
            return Self::default();
        }

        let n = timings.len() as f64;

        let avg_cap = timings
            .iter()
            .map(|t| t.capability_check_us as f64)
            .sum::<f64>()
            / n;
        let avg_fuel_res = timings
            .iter()
            .map(|t| t.fuel_reservation_us as f64)
            .sum::<f64>()
            / n;
        let avg_consent = timings
            .iter()
            .map(|t| t.consent_classification_us as f64)
            .sum::<f64>()
            / n;
        let avg_exec = timings
            .iter()
            .map(|t| t.tool_execution_us as f64)
            .sum::<f64>()
            / n;
        let avg_audit = timings
            .iter()
            .map(|t| t.audit_recording_us as f64)
            .sum::<f64>()
            / n;
        let avg_fuel_con = timings
            .iter()
            .map(|t| t.fuel_consumption_us as f64)
            .sum::<f64>()
            / n;
        let avg_overhead = timings
            .iter()
            .map(|t| t.total_governance_overhead_us as f64)
            .sum::<f64>()
            / n;
        let avg_total = timings.iter().map(|t| t.total_us as f64).sum::<f64>() / n;
        let avg_pct = timings.iter().map(|t| t.overhead_percentage()).sum::<f64>() / n;

        let mut overheads: Vec<u64> = timings
            .iter()
            .map(|t| t.total_governance_overhead_us)
            .collect();
        overheads.sort();

        let percentile = |p: f64| -> u64 {
            let idx = ((p / 100.0) * (overheads.len() as f64 - 1.0)).round() as usize;
            overheads[idx.min(overheads.len() - 1)]
        };

        Self {
            sample_count: timings.len(),
            avg_capability_check_us: avg_cap,
            avg_fuel_reservation_us: avg_fuel_res,
            avg_consent_classification_us: avg_consent,
            avg_tool_execution_us: avg_exec,
            avg_audit_recording_us: avg_audit,
            avg_fuel_consumption_us: avg_fuel_con,
            avg_governance_overhead_us: avg_overhead,
            avg_total_us: avg_total,
            avg_overhead_percentage: avg_pct,
            p50_overhead_us: percentile(50.0),
            p95_overhead_us: percentile(95.0),
            p99_overhead_us: percentile(99.0),
            max_overhead_us: *overheads.last().unwrap_or(&0),
        }
    }

    /// Format as a LaTeX table row.
    pub fn to_latex_row(&self, label: &str) -> String {
        format!(
            "{} & {:.0} & {:.0} & {:.0} & {:.0} & {:.1}\\% & {} & {} \\\\",
            label,
            self.avg_capability_check_us,
            self.avg_fuel_reservation_us,
            self.avg_consent_classification_us,
            self.avg_audit_recording_us,
            self.avg_overhead_percentage,
            self.p50_overhead_us,
            self.p95_overhead_us,
        )
    }
}

/// Collector that accumulates governance timings during a benchmark run.
pub struct GovernanceTimingCollector {
    timings: Vec<GovernanceTiming>,
}

impl GovernanceTimingCollector {
    pub fn new() -> Self {
        Self {
            timings: Vec::new(),
        }
    }

    pub fn record(&mut self, timing: GovernanceTiming) {
        self.timings.push(timing);
    }

    pub fn timings(&self) -> &[GovernanceTiming] {
        &self.timings
    }

    pub fn aggregate(&self) -> AggregateGovernanceMetrics {
        AggregateGovernanceMetrics::from_timings(&self.timings)
    }
}

impl Default for GovernanceTimingCollector {
    fn default() -> Self {
        Self::new()
    }
}
