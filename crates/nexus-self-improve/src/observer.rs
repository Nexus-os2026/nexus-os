//! # Observer
//!
//! Stage 1 of the self-improvement pipeline. Monitors system metrics,
//! audit trail, test results, and capability scores. Emits [`ImprovementSignal`]s
//! when metrics deviate from rolling baselines.

use crate::types::{
    EvidenceItem, ImprovementDomain, ImprovementSignal, MetricBaseline, SignalSource, SystemMetrics,
};
use std::collections::HashMap;
use uuid::Uuid;

/// Configuration for the Observer.
#[derive(Debug, Clone)]
pub struct ObserverConfig {
    /// How many standard deviations from baseline triggers a signal.
    pub sigma_threshold: f64,
    /// Exponential moving average alpha for baseline updates (0.0–1.0).
    pub ema_alpha: f64,
    /// Minimum samples before a baseline is considered valid.
    pub min_samples: u64,
}

impl Default for ObserverConfig {
    fn default() -> Self {
        Self {
            sigma_threshold: 2.0,
            ema_alpha: 0.1,
            min_samples: 10,
        }
    }
}

/// The Observer monitors the system and generates improvement signals.
pub struct Observer {
    baselines: HashMap<String, MetricBaseline>,
    config: ObserverConfig,
}

impl Observer {
    pub fn new(config: ObserverConfig) -> Self {
        Self {
            baselines: HashMap::new(),
            config,
        }
    }

    /// Collect metrics and emit signals for any that deviate from baseline.
    pub fn observe(&mut self, metrics: &SystemMetrics) -> Vec<ImprovementSignal> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let mut signals = Vec::new();

        for (name, &value) in metrics.iter() {
            if let Some(baseline) = self.baselines.get(name) {
                // Use a floor of 1% of mean to avoid division by zero on constant series
                let effective_std = baseline.std_dev.max(baseline.mean.abs() * 0.01).max(1e-9);
                if baseline.sample_count >= self.config.min_samples {
                    let deviation = (value - baseline.mean) / effective_std;
                    if deviation.abs() > self.config.sigma_threshold {
                        signals.push(ImprovementSignal {
                            id: Uuid::new_v4(),
                            timestamp: now,
                            domain: classify_domain(name),
                            source: SignalSource::PerformanceProfiler,
                            metric_name: name.to_string(),
                            current_value: value,
                            baseline_value: baseline.mean,
                            deviation_sigma: deviation,
                            evidence: vec![EvidenceItem {
                                timestamp: now,
                                description: format!(
                                    "{name} deviated {deviation:.1}σ from baseline ({} → {})",
                                    baseline.mean, value
                                ),
                                data: serde_json::json!({
                                    "metric": name,
                                    "current": value,
                                    "baseline_mean": baseline.mean,
                                    "baseline_std": baseline.std_dev,
                                }),
                            }],
                        });
                    }
                }
            }

            self.update_baseline(name, value);
        }

        signals
    }

    /// Observe from test failure signals.
    pub fn observe_test_failure(
        &self,
        test_name: &str,
        failure_rate: f64,
    ) -> Option<ImprovementSignal> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        if failure_rate > 0.0 {
            Some(ImprovementSignal {
                id: Uuid::new_v4(),
                timestamp: now,
                domain: ImprovementDomain::ConfigTuning,
                source: SignalSource::TestSuite,
                metric_name: format!("test_failure:{test_name}"),
                current_value: failure_rate,
                baseline_value: 0.0,
                deviation_sigma: f64::INFINITY,
                evidence: vec![EvidenceItem {
                    timestamp: now,
                    description: format!("test {test_name} failing at rate {failure_rate:.1}%"),
                    data: serde_json::json!({ "test": test_name, "rate": failure_rate }),
                }],
            })
        } else {
            None
        }
    }

    /// Observe from audit trail anomalies.
    pub fn observe_audit_anomaly(
        &self,
        agent_id: &str,
        anomaly_description: &str,
    ) -> ImprovementSignal {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        ImprovementSignal {
            id: Uuid::new_v4(),
            timestamp: now,
            domain: ImprovementDomain::GovernancePolicy,
            source: SignalSource::AuditTrail,
            metric_name: format!("audit_anomaly:{agent_id}"),
            current_value: 1.0,
            baseline_value: 0.0,
            deviation_sigma: f64::INFINITY,
            evidence: vec![EvidenceItem {
                timestamp: now,
                description: anomaly_description.to_string(),
                data: serde_json::json!({ "agent_id": agent_id }),
            }],
        }
    }

    /// Observe capability score degradation.
    pub fn observe_capability_degradation(
        &self,
        agent_id: &str,
        old_score: f64,
        new_score: f64,
    ) -> Option<ImprovementSignal> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        if new_score < old_score * 0.9 {
            Some(ImprovementSignal {
                id: Uuid::new_v4(),
                timestamp: now,
                domain: ImprovementDomain::PromptOptimization,
                source: SignalSource::CapabilityMeasurement,
                metric_name: format!("capability_score:{agent_id}"),
                current_value: new_score,
                baseline_value: old_score,
                deviation_sigma: (old_score - new_score) / (old_score * 0.1).max(0.01),
                evidence: vec![EvidenceItem {
                    timestamp: now,
                    description: format!(
                        "agent {agent_id} capability score dropped {old_score:.2} → {new_score:.2}"
                    ),
                    data: serde_json::json!({
                        "agent_id": agent_id,
                        "old_score": old_score,
                        "new_score": new_score,
                    }),
                }],
            })
        } else {
            None
        }
    }

    /// Get current baselines (for inspection/testing).
    pub fn baselines(&self) -> &HashMap<String, MetricBaseline> {
        &self.baselines
    }

    fn update_baseline(&mut self, name: &str, value: f64) {
        let baseline = self
            .baselines
            .entry(name.to_string())
            .or_insert(MetricBaseline {
                mean: value,
                std_dev: 0.0,
                sample_count: 0,
            });

        let alpha = self.config.ema_alpha;
        let old_mean = baseline.mean;

        // EMA update
        baseline.mean = alpha * value + (1.0 - alpha) * baseline.mean;

        // Running variance estimate (Welford-like with EMA)
        let diff = value - old_mean;
        let new_variance =
            (1.0 - alpha) * (baseline.std_dev * baseline.std_dev + alpha * diff * diff);
        baseline.std_dev = new_variance.sqrt();

        baseline.sample_count += 1;
    }
}

/// Classify a metric name into an improvement domain.
fn classify_domain(metric_name: &str) -> ImprovementDomain {
    let lower = metric_name.to_lowercase();
    if lower.contains("latency") || lower.contains("throughput") || lower.contains("cpu") {
        ImprovementDomain::ConfigTuning
    } else if lower.contains("prompt") || lower.contains("completion") {
        ImprovementDomain::PromptOptimization
    } else if lower.contains("policy") || lower.contains("governance") {
        ImprovementDomain::GovernancePolicy
    } else if lower.contains("schedule") || lower.contains("queue") {
        ImprovementDomain::SchedulingPolicy
    } else if lower.contains("route") || lower.contains("model_select") {
        ImprovementDomain::RoutingStrategy
    } else {
        ImprovementDomain::ConfigTuning
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_metrics(values: &[(&str, f64)]) -> SystemMetrics {
        let mut m = SystemMetrics::new();
        for (k, v) in values {
            m.insert(*k, *v);
        }
        m
    }

    #[test]
    fn test_observer_sigma_threshold_detection() {
        let mut observer = Observer::new(ObserverConfig {
            sigma_threshold: 2.0,
            ema_alpha: 0.1,
            min_samples: 5,
            ..Default::default()
        });

        // Feed stable values to build baseline
        for _ in 0..20 {
            let m = make_metrics(&[("latency_p99", 100.0)]);
            let signals = observer.observe(&m);
            assert!(
                signals.is_empty(),
                "should not fire during baseline building"
            );
        }

        // Now inject a spike
        let m = make_metrics(&[("latency_p99", 500.0)]);
        let signals = observer.observe(&m);
        assert!(
            !signals.is_empty(),
            "should detect spike above sigma threshold"
        );
        assert_eq!(signals[0].metric_name, "latency_p99");
        assert!(signals[0].deviation_sigma > 2.0);
    }

    #[test]
    fn test_observer_baseline_update_with_ema() {
        let mut observer = Observer::new(ObserverConfig {
            ema_alpha: 0.5,
            ..Default::default()
        });

        observer.observe(&make_metrics(&[("m1", 100.0)]));
        let bl = &observer.baselines()["m1"];
        assert!((bl.mean - 100.0).abs() < 0.01);
        assert_eq!(bl.sample_count, 1);

        observer.observe(&make_metrics(&[("m1", 200.0)]));
        let bl = &observer.baselines()["m1"];
        // EMA: 0.5 * 200 + 0.5 * 100 = 150
        assert!((bl.mean - 150.0).abs() < 0.01);
        assert_eq!(bl.sample_count, 2);
    }

    #[test]
    fn test_observer_signal_from_test_failure() {
        let observer = Observer::new(ObserverConfig::default());
        let signal = observer.observe_test_failure("test_foo", 0.15);
        assert!(signal.is_some());
        let s = signal.unwrap();
        assert_eq!(s.source, SignalSource::TestSuite);
        assert!(s.metric_name.contains("test_foo"));
    }

    #[test]
    fn test_observer_no_signal_for_zero_failure_rate() {
        let observer = Observer::new(ObserverConfig::default());
        assert!(observer.observe_test_failure("test_bar", 0.0).is_none());
    }

    #[test]
    fn test_observer_capability_degradation_detection() {
        let observer = Observer::new(ObserverConfig::default());
        // 20% drop should trigger
        let signal = observer.observe_capability_degradation("agent-1", 0.9, 0.7);
        assert!(signal.is_some());

        // 5% drop should not trigger (below 10% threshold)
        let signal = observer.observe_capability_degradation("agent-1", 0.9, 0.86);
        assert!(signal.is_none());
    }
}
