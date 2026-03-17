//! Continuous performance profiling — tracks response times, memory, CPU, and
//! error rates to detect bottlenecks and regressions.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use super::SelfRewriteError;

/// Severity of a detected bottleneck.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BottleneckSeverity {
    Low,
    Medium,
    High,
    Critical,
}

/// A single performance measurement for a function/module.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetric {
    pub function_name: String,
    pub module_path: String,
    pub avg_duration_ms: f64,
    pub p99_duration_ms: f64,
    pub call_count: u64,
    pub memory_bytes: u64,
    pub error_rate: f64,
    pub last_measured: u64,
}

/// A detected bottleneck with suggested remediation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bottleneck {
    pub function_name: String,
    pub module_path: String,
    pub severity: BottleneckSeverity,
    pub reason: String,
    pub suggestion: String,
}

/// Internal sample recording for computing aggregates.
#[derive(Debug, Clone)]
struct MetricSamples {
    durations_ms: Vec<f64>,
    memory_bytes: Vec<u64>,
    error_count: u64,
    call_count: u64,
}

impl MetricSamples {
    fn new() -> Self {
        Self {
            durations_ms: Vec::new(),
            memory_bytes: Vec::new(),
            error_count: 0,
            call_count: 0,
        }
    }
}

/// Continuous performance profiler that tracks per-function metrics.
#[derive(Debug, Clone)]
pub struct PerformanceProfiler {
    /// Raw samples keyed by "module_path::function_name".
    samples: HashMap<String, MetricSamples>,
    /// Thresholds for bottleneck detection.
    p99_threshold_ms: f64,
    error_rate_threshold: f64,
    memory_threshold_bytes: u64,
    /// Historical baselines for regression detection.
    baselines: HashMap<String, PerformanceMetric>,
}

impl PerformanceProfiler {
    /// Create a new profiler with default thresholds.
    pub fn new() -> Self {
        Self {
            samples: HashMap::new(),
            p99_threshold_ms: 100.0,
            error_rate_threshold: 0.05,
            memory_threshold_bytes: 50 * 1024 * 1024, // 50 MB
            baselines: HashMap::new(),
        }
    }

    /// Create a profiler with custom thresholds.
    pub fn with_thresholds(
        p99_threshold_ms: f64,
        error_rate_threshold: f64,
        memory_threshold_bytes: u64,
    ) -> Self {
        Self {
            samples: HashMap::new(),
            p99_threshold_ms,
            error_rate_threshold,
            memory_threshold_bytes,
            baselines: HashMap::new(),
        }
    }

    /// Record a metric sample for a function invocation.
    pub fn record_metric(
        &mut self,
        module_path: &str,
        function_name: &str,
        duration_ms: f64,
        memory_bytes: u64,
        is_error: bool,
    ) {
        let key = format!("{module_path}::{function_name}");
        let entry = self.samples.entry(key).or_insert_with(MetricSamples::new);
        entry.durations_ms.push(duration_ms);
        entry.memory_bytes.push(memory_bytes);
        entry.call_count += 1;
        if is_error {
            entry.error_count += 1;
        }
    }

    /// Compute the aggregated metric for a given key.
    fn compute_metric(&self, key: &str, samples: &MetricSamples) -> PerformanceMetric {
        let parts: Vec<&str> = key.rsplitn(2, "::").collect();
        let (function_name, module_path) = if parts.len() == 2 {
            (parts[0].to_string(), parts[1].to_string())
        } else {
            (key.to_string(), String::new())
        };

        let avg_duration_ms = if samples.durations_ms.is_empty() {
            0.0
        } else {
            let sum: f64 = samples.durations_ms.iter().sum();
            sum / samples.durations_ms.len() as f64
        };

        let p99_duration_ms = percentile(&samples.durations_ms, 99.0);

        let max_memory = samples.memory_bytes.iter().copied().max().unwrap_or(0);

        let error_rate = if samples.call_count == 0 {
            0.0
        } else {
            samples.error_count as f64 / samples.call_count as f64
        };

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        PerformanceMetric {
            function_name,
            module_path,
            avg_duration_ms,
            p99_duration_ms,
            call_count: samples.call_count,
            memory_bytes: max_memory,
            error_rate,
            last_measured: now,
        }
    }

    /// Detect bottlenecks based on current metrics and thresholds.
    pub fn detect_bottlenecks(&self) -> Vec<Bottleneck> {
        let mut bottlenecks = Vec::new();

        for (key, samples) in &self.samples {
            let metric = self.compute_metric(key, samples);

            // High p99 latency
            if metric.p99_duration_ms > self.p99_threshold_ms {
                let severity = if metric.p99_duration_ms > self.p99_threshold_ms * 10.0 {
                    BottleneckSeverity::Critical
                } else if metric.p99_duration_ms > self.p99_threshold_ms * 5.0 {
                    BottleneckSeverity::High
                } else if metric.p99_duration_ms > self.p99_threshold_ms * 2.0 {
                    BottleneckSeverity::Medium
                } else {
                    BottleneckSeverity::Low
                };

                bottlenecks.push(Bottleneck {
                    function_name: metric.function_name.clone(),
                    module_path: metric.module_path.clone(),
                    severity,
                    reason: format!(
                        "p99 latency {:.1}ms exceeds threshold {:.1}ms",
                        metric.p99_duration_ms, self.p99_threshold_ms
                    ),
                    suggestion: "Consider caching, reducing allocations, or batching work"
                        .to_string(),
                });
            }

            // High error rate
            if metric.error_rate > self.error_rate_threshold {
                bottlenecks.push(Bottleneck {
                    function_name: metric.function_name.clone(),
                    module_path: metric.module_path.clone(),
                    severity: if metric.error_rate > 0.5 {
                        BottleneckSeverity::Critical
                    } else {
                        BottleneckSeverity::High
                    },
                    reason: format!(
                        "error rate {:.1}% exceeds threshold {:.1}%",
                        metric.error_rate * 100.0,
                        self.error_rate_threshold * 100.0
                    ),
                    suggestion: "Investigate root cause of errors, add retries or fallbacks"
                        .to_string(),
                });
            }

            // High memory usage
            if metric.memory_bytes > self.memory_threshold_bytes {
                bottlenecks.push(Bottleneck {
                    function_name: metric.function_name.clone(),
                    module_path: metric.module_path.clone(),
                    severity: BottleneckSeverity::Medium,
                    reason: format!(
                        "memory usage {}MB exceeds threshold {}MB",
                        metric.memory_bytes / (1024 * 1024),
                        self.memory_threshold_bytes / (1024 * 1024)
                    ),
                    suggestion: "Reduce allocations, use streaming, or shrink buffers".to_string(),
                });
            }
        }

        bottlenecks
    }

    /// Return the top N functions by average duration (hot paths).
    pub fn get_hot_paths(&self, top_n: usize) -> Vec<PerformanceMetric> {
        let mut metrics: Vec<PerformanceMetric> = self
            .samples
            .iter()
            .map(|(key, samples)| self.compute_metric(key, samples))
            .collect();

        metrics.sort_by(|a, b| {
            b.avg_duration_ms
                .partial_cmp(&a.avg_duration_ms)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        metrics.truncate(top_n);
        metrics
    }

    /// Save current metrics as baselines for future regression detection.
    pub fn save_baselines(&mut self) {
        for (key, samples) in &self.samples {
            let metric = self.compute_metric(key, samples);
            self.baselines.insert(key.clone(), metric);
        }
    }

    /// Compare current metrics against saved baselines, returning regressions.
    pub fn get_regression_report(&self) -> Result<Vec<Bottleneck>, SelfRewriteError> {
        let mut regressions = Vec::new();

        for (key, samples) in &self.samples {
            if let Some(baseline) = self.baselines.get(key) {
                let current = self.compute_metric(key, samples);

                // Latency regression: >20% increase
                if baseline.avg_duration_ms > 0.0 {
                    let pct_change = (current.avg_duration_ms - baseline.avg_duration_ms)
                        / baseline.avg_duration_ms
                        * 100.0;
                    if pct_change > 20.0 {
                        regressions.push(Bottleneck {
                            function_name: current.function_name,
                            module_path: current.module_path,
                            severity: if pct_change > 100.0 {
                                BottleneckSeverity::Critical
                            } else if pct_change > 50.0 {
                                BottleneckSeverity::High
                            } else {
                                BottleneckSeverity::Medium
                            },
                            reason: format!(
                                "latency regression {pct_change:.1}% (baseline {:.1}ms → current {:.1}ms)",
                                baseline.avg_duration_ms, current.avg_duration_ms
                            ),
                            suggestion: "Revert recent changes or optimize the regression"
                                .to_string(),
                        });
                    }
                }
            }
        }

        Ok(regressions)
    }
}

impl Default for PerformanceProfiler {
    fn default() -> Self {
        Self::new()
    }
}

/// Compute the pth percentile of a sorted copy of `values`.
fn percentile(values: &[f64], p: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let idx = ((p / 100.0) * (sorted.len() as f64 - 1.0)).ceil() as usize;
    let idx = idx.min(sorted.len() - 1);
    sorted[idx]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_and_detect_bottleneck() {
        let mut profiler = PerformanceProfiler::with_thresholds(10.0, 0.05, 1024);

        // Record a fast function — no bottleneck
        profiler.record_metric("kernel::audit", "append_event", 2.0, 512, false);
        assert!(profiler.detect_bottlenecks().is_empty());

        // Record a slow function — should detect bottleneck
        for _ in 0..10 {
            profiler.record_metric("kernel::firewall", "check_rule", 50.0, 512, false);
        }
        let bottlenecks = profiler.detect_bottlenecks();
        assert!(!bottlenecks.is_empty());
        assert_eq!(bottlenecks[0].function_name, "check_rule");
    }

    #[test]
    fn high_error_rate_detected() {
        let mut profiler = PerformanceProfiler::with_thresholds(1000.0, 0.05, u64::MAX);

        for i in 0..10 {
            profiler.record_metric("kernel::net", "send", 1.0, 100, i < 5);
        }
        let bottlenecks = profiler.detect_bottlenecks();
        assert!(bottlenecks.iter().any(|b| b.reason.contains("error rate")));
    }

    #[test]
    fn hot_paths_sorted_by_duration() {
        let mut profiler = PerformanceProfiler::new();
        profiler.record_metric("mod_a", "slow_fn", 200.0, 100, false);
        profiler.record_metric("mod_b", "fast_fn", 1.0, 100, false);
        profiler.record_metric("mod_c", "medium_fn", 50.0, 100, false);

        let hot = profiler.get_hot_paths(2);
        assert_eq!(hot.len(), 2);
        assert_eq!(hot[0].function_name, "slow_fn");
        assert_eq!(hot[1].function_name, "medium_fn");
    }

    #[test]
    fn regression_detection() {
        let mut profiler = PerformanceProfiler::new();

        // Baseline
        profiler.record_metric("mod_a", "fn_a", 10.0, 100, false);
        profiler.save_baselines();

        // Clear and record regressed metric
        profiler.samples.clear();
        profiler.record_metric("mod_a", "fn_a", 25.0, 100, false);

        let regressions = profiler.get_regression_report().unwrap();
        assert!(!regressions.is_empty());
        assert!(regressions[0].reason.contains("regression"));
    }

    #[test]
    fn percentile_computation() {
        assert_eq!(percentile(&[], 99.0), 0.0);
        assert_eq!(percentile(&[5.0], 99.0), 5.0);
        let vals = vec![1.0, 2.0, 3.0, 4.0, 100.0];
        assert_eq!(percentile(&vals, 99.0), 100.0);
    }

    #[test]
    fn metric_serialization() {
        let m = PerformanceMetric {
            function_name: "test_fn".into(),
            module_path: "kernel::test".into(),
            avg_duration_ms: 12.5,
            p99_duration_ms: 45.0,
            call_count: 100,
            memory_bytes: 2048,
            error_rate: 0.01,
            last_measured: 1700000000,
        };
        let json = serde_json::to_string(&m).unwrap();
        let back: PerformanceMetric = serde_json::from_str(&json).unwrap();
        assert_eq!(back.function_name, "test_fn");
        assert_eq!(back.call_count, 100);
    }
}
