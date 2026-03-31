//! # Behavioral Envelope
//!
//! Mathematically bounds agent behavioral drift using the Drift Bounds Theorem:
//! **D\* = α/γ** where α is the drift rate and γ is the recovery rate.
//!
//! If an agent's behavior drifts beyond D\*, the Simplex Guardian switches to
//! the verified baseline configuration.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};

/// Maximum observations retained per metric.
const MAX_HISTORY: usize = 200;

/// A bounded metric with baseline, bounds, and rolling history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricBound {
    pub name: String,
    pub baseline: f64,
    pub lower_bound: f64,
    pub upper_bound: f64,
    pub current: f64,
    pub history: VecDeque<(u64, f64)>,
}

impl MetricBound {
    pub fn new(name: impl Into<String>, baseline: f64, tolerance: f64) -> Self {
        Self {
            name: name.into(),
            baseline,
            lower_bound: baseline - tolerance,
            upper_bound: baseline + tolerance,
            current: baseline,
            history: VecDeque::new(),
        }
    }

    /// Whether the current value is within bounds.
    pub fn is_within(&self) -> bool {
        self.current >= self.lower_bound && self.current <= self.upper_bound
    }

    /// Normalized deviation from baseline (0.0 = at baseline, 1.0 = at bound edge).
    pub fn normalized_deviation(&self) -> f64 {
        let half_range = (self.upper_bound - self.lower_bound) / 2.0;
        if half_range == 0.0 {
            return 0.0;
        }
        ((self.current - self.baseline).abs() / half_range).min(2.0)
    }
}

/// Agent behavioral contract with mathematical drift bounds.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehavioralEnvelope {
    pub agent_id: String,
    pub metrics: HashMap<String, MetricBound>,
    /// α: observed rate of behavioral change per observation.
    pub drift_rate: f64,
    /// γ: rate of correction back to baseline.
    pub recovery_rate: f64,
    /// EMA alpha for drift rate estimation.
    ema_alpha: f64,
}

impl BehavioralEnvelope {
    pub fn new(agent_id: impl Into<String>) -> Self {
        Self {
            agent_id: agent_id.into(),
            metrics: HashMap::new(),
            drift_rate: 0.0,
            recovery_rate: 0.1,
            ema_alpha: 0.1,
        }
    }

    /// Add a metric to track.
    pub fn add_metric(&mut self, name: impl Into<String>, baseline: f64, tolerance: f64) {
        let name = name.into();
        self.metrics
            .insert(name.clone(), MetricBound::new(name, baseline, tolerance));
    }

    /// Check if all metrics are within bounds.
    pub fn is_within_bounds(&self) -> bool {
        self.metrics.values().all(MetricBound::is_within)
    }

    /// Calculate current RMS drift from baseline across all metrics.
    pub fn current_drift(&self) -> f64 {
        if self.metrics.is_empty() {
            return 0.0;
        }
        let sum_sq: f64 = self
            .metrics
            .values()
            .map(|m| {
                let dev = m.normalized_deviation();
                dev * dev
            })
            .sum();
        (sum_sq / self.metrics.len() as f64).sqrt()
    }

    /// Drift Bounds Theorem guarantee: D* = α/γ.
    pub fn drift_bound_guarantee(&self) -> f64 {
        if self.recovery_rate > 0.0 {
            self.drift_rate / self.recovery_rate
        } else {
            f64::INFINITY
        }
    }

    /// Predict whether proposed metric changes would violate bounds.
    pub fn would_violate(&self, predicted_metrics: &HashMap<String, f64>) -> bool {
        for (name, &value) in predicted_metrics {
            if let Some(bound) = self.metrics.get(name) {
                if value < bound.lower_bound || value > bound.upper_bound {
                    return true;
                }
            }
        }
        false
    }

    /// Update envelope with new observations. Recalculates drift rate via EMA.
    pub fn update(&mut self, observations: &HashMap<String, f64>, timestamp: u64) {
        let mut total_delta = 0.0_f64;
        let mut count = 0u32;

        for (name, &value) in observations {
            if let Some(bound) = self.metrics.get_mut(name) {
                let delta = (value - bound.current).abs();
                total_delta += delta;
                count += 1;

                bound.current = value;
                bound.history.push_back((timestamp, value));
                if bound.history.len() > MAX_HISTORY {
                    bound.history.pop_front();
                }
            }
        }

        if count > 0 {
            let avg_delta = total_delta / count as f64;
            // EMA update of drift rate
            self.drift_rate = self.ema_alpha * avg_delta + (1.0 - self.ema_alpha) * self.drift_rate;
        }
    }

    /// Set recovery rate (how fast corrections bring metrics back to baseline).
    pub fn set_recovery_rate(&mut self, gamma: f64) {
        self.recovery_rate = gamma.max(0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_envelope() -> BehavioralEnvelope {
        let mut env = BehavioralEnvelope::new("test-agent");
        env.add_metric("accuracy", 0.9, 0.1); // bounds: [0.8, 1.0]
        env.add_metric("latency", 100.0, 50.0); // bounds: [50, 150]
        env
    }

    #[test]
    fn test_within_bounds() {
        let env = make_envelope();
        assert!(env.is_within_bounds());
    }

    #[test]
    fn test_outside_bounds() {
        let mut env = make_envelope();
        env.metrics.get_mut("accuracy").unwrap().current = 0.5; // below lower bound
        assert!(!env.is_within_bounds());
    }

    #[test]
    fn test_drift_calculation() {
        let mut env = make_envelope();
        // Both at baseline → drift should be 0
        assert!((env.current_drift()).abs() < 1e-9);

        // Push accuracy to boundary edge
        env.metrics.get_mut("accuracy").unwrap().current = 0.8; // at lower bound
        let drift = env.current_drift();
        assert!(
            drift > 0.0,
            "drift should be positive when away from baseline"
        );
    }

    #[test]
    fn test_drift_bound_theorem() {
        let mut env = make_envelope();
        env.drift_rate = 0.05;
        env.recovery_rate = 0.10;
        // D* = α/γ = 0.05/0.10 = 0.5
        assert!((env.drift_bound_guarantee() - 0.5).abs() < 1e-9);
    }

    #[test]
    fn test_drift_bound_zero_recovery() {
        let mut env = make_envelope();
        env.drift_rate = 0.05;
        env.recovery_rate = 0.0;
        assert!(env.drift_bound_guarantee().is_infinite());
    }

    #[test]
    fn test_would_violate_prediction() {
        let env = make_envelope();

        // Within bounds
        let mut good = HashMap::new();
        good.insert("accuracy".to_string(), 0.85);
        assert!(!env.would_violate(&good));

        // Outside bounds
        let mut bad = HashMap::new();
        bad.insert("accuracy".to_string(), 0.5);
        assert!(env.would_violate(&bad));
    }

    #[test]
    fn test_ema_update() {
        let mut env = make_envelope();
        env.ema_alpha = 0.5;
        env.drift_rate = 0.0;

        let mut obs = HashMap::new();
        obs.insert("accuracy".to_string(), 0.95); // delta = 0.05 from baseline 0.9
        obs.insert("latency".to_string(), 110.0); // delta = 10.0 from baseline 100.0

        env.update(&obs, 1000);

        // drift_rate should have increased from 0.0
        assert!(
            env.drift_rate > 0.0,
            "drift rate should increase after observation"
        );

        // Current values should be updated
        assert!((env.metrics["accuracy"].current - 0.95).abs() < 1e-9);
        assert!((env.metrics["latency"].current - 110.0).abs() < 1e-9);

        // History should have one entry
        assert_eq!(env.metrics["accuracy"].history.len(), 1);
    }
}
