//! Layer 3: Kernel Performance Self-Optimization — tracks operation latencies,
//! detects regressions, and queues bottlenecks for automated repair.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

// ── Types ───────────────────────────────────────────────────────────────────

/// Direction of a metric over time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Trend {
    Improving,
    Stable,
    Degrading,
}

/// A detected performance bottleneck.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bottleneck {
    pub operation: String,
    pub avg_latency_ms: f64,
    pub p99_latency_ms: f64,
    pub sample_count: u32,
    pub trend: Trend,
    pub detected_at: u64,
    pub suggested_fix: Option<String>,
}

/// Point-in-time memory snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySnapshot {
    pub timestamp: u64,
    pub heap_mb: f64,
    pub active_agents: u32,
}

/// Full performance report for the UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceReport {
    pub operations: Vec<OperationStats>,
    pub bottlenecks: Vec<Bottleneck>,
    pub memory_trend: Trend,
    pub latest_memory_mb: f64,
}

/// Per-operation statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationStats {
    pub operation: String,
    pub avg_ms: f64,
    pub p99_ms: f64,
    pub sample_count: u32,
    pub trend: Trend,
}

// ── PerformanceEvolver ──────────────────────────────────────────────────────

/// Tracks response times, detects regressions, and queues bottlenecks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceEvolver {
    metrics: HashMap<String, Vec<f64>>,
    memory_snapshots: Vec<MemorySnapshot>,
    bottleneck_queue: Vec<Bottleneck>,
    auto_optimize: bool,
    min_samples: u32,
    regression_threshold: f64,
    max_samples_per_op: usize,
    max_memory_snapshots: usize,
}

impl PerformanceEvolver {
    pub fn new() -> Self {
        Self {
            metrics: HashMap::new(),
            memory_snapshots: Vec::new(),
            bottleneck_queue: Vec::new(),
            auto_optimize: true,
            min_samples: 100,
            regression_threshold: 0.20, // 20% slower = regression
            max_samples_per_op: 1000,
            max_memory_snapshots: 500,
        }
    }

    /// Record timing for an operation.
    pub fn record_timing(&mut self, operation: &str, latency_ms: f64) {
        let timings = self.metrics.entry(operation.to_string()).or_default();
        timings.push(latency_ms);

        // Rolling window
        if timings.len() > self.max_samples_per_op {
            timings.remove(0);
        }

        // Check for regression once we have enough samples
        if timings.len() >= self.min_samples as usize {
            let window = 50.min(timings.len() / 2);
            let recent_avg: f64 =
                timings[timings.len() - window..].iter().sum::<f64>() / window as f64;
            let baseline_avg: f64 = timings[..window].iter().sum::<f64>() / window as f64;

            if baseline_avg > 0.0 && recent_avg > baseline_avg * (1.0 + self.regression_threshold) {
                // Check if we already have this bottleneck queued
                if !self
                    .bottleneck_queue
                    .iter()
                    .any(|b| b.operation == operation)
                {
                    self.bottleneck_queue.push(Bottleneck {
                        operation: operation.to_string(),
                        avg_latency_ms: recent_avg,
                        p99_latency_ms: percentile(timings, 0.99),
                        sample_count: timings.len() as u32,
                        trend: Trend::Degrading,
                        detected_at: epoch_secs(),
                        suggested_fix: None,
                    });
                }
            }
        }
    }

    /// Record a memory snapshot.
    pub fn record_memory(&mut self, heap_mb: f64, active_agents: u32) {
        self.memory_snapshots.push(MemorySnapshot {
            timestamp: epoch_secs(),
            heap_mb,
            active_agents,
        });
        if self.memory_snapshots.len() > self.max_memory_snapshots {
            self.memory_snapshots.remove(0);
        }
    }

    /// Get all detected bottlenecks.
    pub fn bottlenecks(&self) -> &[Bottleneck] {
        &self.bottleneck_queue
    }

    /// Drain bottlenecks (after optimization attempt).
    pub fn drain_bottlenecks(&mut self) -> Vec<Bottleneck> {
        std::mem::take(&mut self.bottleneck_queue)
    }

    /// Compute trend for a specific operation.
    pub fn operation_trend(&self, operation: &str) -> Trend {
        let timings = match self.metrics.get(operation) {
            Some(t) if t.len() >= 20 => t,
            _ => return Trend::Stable,
        };
        let window = 10.min(timings.len() / 2);
        let recent: f64 = timings[timings.len() - window..].iter().sum::<f64>() / window as f64;
        let baseline: f64 = timings[..window].iter().sum::<f64>() / window as f64;

        if baseline == 0.0 {
            return Trend::Stable;
        }
        let ratio = recent / baseline;
        if ratio < 0.9 {
            Trend::Improving
        } else if ratio > 1.2 {
            Trend::Degrading
        } else {
            Trend::Stable
        }
    }

    /// Memory usage trend.
    pub fn memory_trend(&self) -> Trend {
        if self.memory_snapshots.len() < 10 {
            return Trend::Stable;
        }
        let window = 5;
        let recent: f64 = self.memory_snapshots[self.memory_snapshots.len() - window..]
            .iter()
            .map(|s| s.heap_mb)
            .sum::<f64>()
            / window as f64;
        let baseline: f64 = self.memory_snapshots[..window]
            .iter()
            .map(|s| s.heap_mb)
            .sum::<f64>()
            / window as f64;

        if baseline == 0.0 {
            return Trend::Stable;
        }
        let ratio = recent / baseline;
        if ratio < 0.9 {
            Trend::Improving
        } else if ratio > 1.2 {
            Trend::Degrading
        } else {
            Trend::Stable
        }
    }

    /// Generate a full performance report.
    pub fn report(&self) -> PerformanceReport {
        let operations = self
            .metrics
            .iter()
            .map(|(op, timings)| {
                let avg = if timings.is_empty() {
                    0.0
                } else {
                    timings.iter().sum::<f64>() / timings.len() as f64
                };
                OperationStats {
                    operation: op.clone(),
                    avg_ms: avg,
                    p99_ms: percentile(timings, 0.99),
                    sample_count: timings.len() as u32,
                    trend: self.operation_trend(op),
                }
            })
            .collect();

        PerformanceReport {
            operations,
            bottlenecks: self.bottleneck_queue.clone(),
            memory_trend: self.memory_trend(),
            latest_memory_mb: self
                .memory_snapshots
                .last()
                .map(|s| s.heap_mb)
                .unwrap_or(0.0),
        }
    }

    /// Whether auto-optimization is enabled.
    pub fn auto_optimize_enabled(&self) -> bool {
        self.auto_optimize
    }

    /// Toggle auto-optimization.
    pub fn set_auto_optimize(&mut self, enabled: bool) {
        self.auto_optimize = enabled;
    }

    /// Average latency for an operation (or 0 if no data).
    pub fn avg_latency(&self, operation: &str) -> f64 {
        self.metrics
            .get(operation)
            .filter(|t| !t.is_empty())
            .map(|t| t.iter().sum::<f64>() / t.len() as f64)
            .unwrap_or(0.0)
    }
}

impl Default for PerformanceEvolver {
    fn default() -> Self {
        Self::new()
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn percentile(values: &[f64], p: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let idx = ((sorted.len() as f64 * p).ceil() as usize).saturating_sub(1);
    sorted[idx.min(sorted.len() - 1)]
}

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

    #[test]
    fn records_timing_and_computes_average() {
        let mut perf = PerformanceEvolver::new();
        perf.record_timing("list_agents", 100.0);
        perf.record_timing("list_agents", 200.0);
        assert!((perf.avg_latency("list_agents") - 150.0).abs() < 1e-9);
    }

    #[test]
    fn detects_regression() {
        let mut perf = PerformanceEvolver::new();
        perf.min_samples = 20;
        // 50 fast samples
        for _ in 0..50 {
            perf.record_timing("list_agents", 50.0);
        }
        // 50 slow samples (3x baseline)
        for _ in 0..50 {
            perf.record_timing("list_agents", 150.0);
        }
        assert!(!perf.bottlenecks().is_empty());
        assert_eq!(perf.bottlenecks()[0].operation, "list_agents");
        assert_eq!(perf.bottlenecks()[0].trend, Trend::Degrading);
    }

    #[test]
    fn no_bottleneck_when_stable() {
        let mut perf = PerformanceEvolver::new();
        perf.min_samples = 20;
        for _ in 0..100 {
            perf.record_timing("fast_op", 10.0);
        }
        assert!(perf.bottlenecks().is_empty());
    }

    #[test]
    fn trend_detecting() {
        let mut perf = PerformanceEvolver::new();
        // Improving: fast recently
        for _ in 0..10 {
            perf.record_timing("improving", 100.0);
        }
        for _ in 0..10 {
            perf.record_timing("improving", 50.0);
        }
        assert_eq!(perf.operation_trend("improving"), Trend::Improving);
    }

    #[test]
    fn history_bounded() {
        let mut perf = PerformanceEvolver::new();
        perf.max_samples_per_op = 10;
        for i in 0..20 {
            perf.record_timing("op", i as f64);
        }
        assert_eq!(perf.metrics["op"].len(), 10);
    }

    #[test]
    fn memory_trend_stable_with_few_samples() {
        let perf = PerformanceEvolver::new();
        assert_eq!(perf.memory_trend(), Trend::Stable);
    }

    #[test]
    fn report_includes_all_operations() {
        let mut perf = PerformanceEvolver::new();
        perf.record_timing("op1", 10.0);
        perf.record_timing("op2", 20.0);
        let report = perf.report();
        assert_eq!(report.operations.len(), 2);
    }

    #[test]
    fn percentile_calculation() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
        assert!((percentile(&values, 0.99) - 10.0).abs() < 1e-9);
        assert!((percentile(&values, 0.5) - 5.0).abs() < 1e-9);
    }
}
