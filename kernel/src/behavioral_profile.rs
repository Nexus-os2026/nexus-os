//! Behavioral profiling: per-agent baselines and semantic drift detection.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const MIN_SAMPLES_FOR_BASELINE: u64 = 50;

// ── Data types ──

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResourceProfile {
    pub avg_memory_bytes: u64,
    pub avg_cpu_seconds: f64,
    pub avg_fuel_per_action: f64,
    pub max_fuel_single_action: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehavioralBaseline {
    pub agent_id: String,
    pub action_histogram: HashMap<String, u64>,
    pub action_frequency: f64,
    pub peak_frequency: f64,
    pub typical_resource_usage: ResourceProfile,
    pub observation_window_minutes: u64,
    pub samples_collected: u64,
    pub established: bool,
}

impl BehavioralBaseline {
    fn new(agent_id: &str) -> Self {
        Self {
            agent_id: agent_id.to_string(),
            action_histogram: HashMap::new(),
            action_frequency: 0.0,
            peak_frequency: 0.0,
            typical_resource_usage: ResourceProfile::default(),
            observation_window_minutes: 0,
            samples_collected: 0,
            established: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionRecord {
    pub action_type: String,
    pub timestamp: u64,
    pub fuel_cost: u64,
    pub resource_usage: Option<ResourceProfile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionWindow {
    pub actions: Vec<ActionRecord>,
    pub window_start: u64,
}

impl ActionWindow {
    fn new(start: u64) -> Self {
        Self {
            actions: Vec::new(),
            window_start: start,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DriftType {
    FrequencySpike,
    UnusualActionType,
    ResourceAnomaly,
    PatternShift,
    RoleDeviation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum DriftSeverity {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftAlert {
    pub agent_id: String,
    pub drift_type: DriftType,
    pub severity: DriftSeverity,
    pub details: String,
    pub current_value: f64,
    pub baseline_value: f64,
    pub deviation_factor: f64,
    pub timestamp: u64,
}

// ── Profiler ──

pub struct BehavioralProfiler {
    baselines: HashMap<String, BehavioralBaseline>,
    active_windows: HashMap<String, ActionWindow>,
    drift_threshold: f64,
    alert_callback: Option<Box<dyn Fn(DriftAlert) + Send + Sync>>,
}

impl std::fmt::Debug for BehavioralProfiler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BehavioralProfiler")
            .field("baselines", &self.baselines)
            .field("active_windows", &self.active_windows)
            .field("drift_threshold", &self.drift_threshold)
            .field(
                "alert_callback",
                &self.alert_callback.as_ref().map(|_| "..."),
            )
            .finish()
    }
}

impl BehavioralProfiler {
    pub fn new(drift_threshold: f64) -> Self {
        Self {
            baselines: HashMap::new(),
            active_windows: HashMap::new(),
            drift_threshold: drift_threshold.max(1.0),
            alert_callback: None,
        }
    }

    pub fn set_alert_callback(&mut self, callback: impl Fn(DriftAlert) + Send + Sync + 'static) {
        self.alert_callback = Some(Box::new(callback));
    }

    pub fn record_action(&mut self, agent_id: &str, action: ActionRecord) {
        let timestamp = action.timestamp;

        // Ensure baseline entry exists.
        let baseline = self
            .baselines
            .entry(agent_id.to_string())
            .or_insert_with(|| BehavioralBaseline::new(agent_id));

        // Only accumulate into baseline while it is still being built.
        if !baseline.established {
            *baseline
                .action_histogram
                .entry(action.action_type.clone())
                .or_insert(0) += 1;
            baseline.samples_collected += 1;

            let n = baseline.samples_collected as f64;
            let prev_avg_fuel = baseline.typical_resource_usage.avg_fuel_per_action;
            baseline.typical_resource_usage.avg_fuel_per_action =
                prev_avg_fuel + (action.fuel_cost as f64 - prev_avg_fuel) / n;
            if action.fuel_cost > baseline.typical_resource_usage.max_fuel_single_action {
                baseline.typical_resource_usage.max_fuel_single_action = action.fuel_cost;
            }

            if let Some(ref res) = action.resource_usage {
                let prev_mem = baseline.typical_resource_usage.avg_memory_bytes as f64;
                baseline.typical_resource_usage.avg_memory_bytes =
                    (prev_mem + (res.avg_memory_bytes as f64 - prev_mem) / n) as u64;

                let prev_cpu = baseline.typical_resource_usage.avg_cpu_seconds;
                baseline.typical_resource_usage.avg_cpu_seconds =
                    prev_cpu + (res.avg_cpu_seconds - prev_cpu) / n;
            }
        }

        // Add to active window.
        let window = self
            .active_windows
            .entry(agent_id.to_string())
            .or_insert_with(|| ActionWindow::new(timestamp));
        window.actions.push(action);

        // Recompute frequency from window (only stored in baseline during build phase).
        if let Some(first) = window.actions.first() {
            let elapsed_secs = timestamp.saturating_sub(first.timestamp);
            if elapsed_secs > 0 {
                let actions_per_min = (window.actions.len() as f64 / elapsed_secs as f64) * 60.0;
                if let Some(baseline) = self.baselines.get_mut(agent_id) {
                    if !baseline.established {
                        baseline.action_frequency = actions_per_min;
                        if actions_per_min > baseline.peak_frequency {
                            baseline.peak_frequency = actions_per_min;
                        }
                    }
                }
            }
        }

        // Update observation window minutes (only during build phase).
        {
            let is_building = self
                .baselines
                .get(agent_id)
                .map(|b| !b.established)
                .unwrap_or(false);
            if is_building {
                if let Some(first) = self
                    .active_windows
                    .get(agent_id)
                    .and_then(|w| w.actions.first())
                {
                    let minutes = timestamp.saturating_sub(first.timestamp).saturating_div(60);
                    if let Some(baseline) = self.baselines.get_mut(agent_id) {
                        baseline.observation_window_minutes = minutes;
                    }
                }
            }
        }

        // Prune window to last 10 minutes of data.
        let Some(window) = self.active_windows.get_mut(agent_id) else {
            return;
        };
        let cutoff = timestamp.saturating_sub(600);
        window.actions.retain(|a| a.timestamp >= cutoff);
        if let Some(first) = window.actions.first() {
            window.window_start = first.timestamp;
        }
    }

    pub fn check_drift(&self, agent_id: &str) -> Vec<DriftAlert> {
        let mut alerts = Vec::new();

        let Some(baseline) = self.baselines.get(agent_id) else {
            return alerts;
        };

        if !baseline.established {
            return alerts;
        }

        let Some(window) = self.active_windows.get(agent_id) else {
            return alerts;
        };

        let now = window.actions.last().map(|a| a.timestamp).unwrap_or(0);

        // 1. Frequency spike check.
        if baseline.action_frequency > 0.0 {
            let current_freq = self.current_frequency(window);
            if current_freq > 0.0 && baseline.action_frequency > 0.0 {
                let factor = current_freq / baseline.action_frequency;
                if factor >= self.drift_threshold {
                    let severity = severity_from_factor(factor, self.drift_threshold);
                    alerts.push(DriftAlert {
                        agent_id: agent_id.to_string(),
                        drift_type: DriftType::FrequencySpike,
                        severity,
                        details: format!(
                            "action frequency {:.1}/min vs baseline {:.1}/min",
                            current_freq, baseline.action_frequency
                        ),
                        current_value: current_freq,
                        baseline_value: baseline.action_frequency,
                        deviation_factor: factor,
                        timestamp: now,
                    });
                }
            }
        }

        // 2. Unusual action type check.
        let total_baseline: u64 = baseline.action_histogram.values().sum();
        for action in &window.actions {
            let action_type = &action.action_type;
            match baseline.action_histogram.get(action_type) {
                None => {
                    // Action type never seen in baseline.
                    alerts.push(DriftAlert {
                        agent_id: agent_id.to_string(),
                        drift_type: DriftType::UnusualActionType,
                        severity: DriftSeverity::High,
                        details: format!(
                            "action type '{}' never observed in baseline",
                            action_type
                        ),
                        current_value: 1.0,
                        baseline_value: 0.0,
                        deviation_factor: f64::INFINITY,
                        timestamp: action.timestamp,
                    });
                }
                Some(&count) if total_baseline > 0 => {
                    let pct = (count as f64 / total_baseline as f64) * 100.0;
                    if pct < 1.0 {
                        alerts.push(DriftAlert {
                            agent_id: agent_id.to_string(),
                            drift_type: DriftType::UnusualActionType,
                            severity: DriftSeverity::Medium,
                            details: format!(
                                "action type '{}' is rare ({:.2}% of baseline)",
                                action_type, pct
                            ),
                            current_value: pct,
                            baseline_value: 1.0,
                            deviation_factor: 1.0 / pct.max(0.01),
                            timestamp: action.timestamp,
                        });
                    }
                }
                _ => {}
            }
        }

        // Deduplicate unusual-action alerts by action_type (keep first).
        let mut seen_action_types = std::collections::HashSet::new();
        alerts.retain(|alert| {
            if alert.drift_type == DriftType::UnusualActionType {
                seen_action_types.insert(alert.details.clone())
            } else {
                true
            }
        });

        // 3. Resource anomaly check (fuel per action).
        let baseline_fuel = baseline.typical_resource_usage.avg_fuel_per_action;
        if baseline_fuel > 0.0 {
            let window_fuel_total: u64 = window.actions.iter().map(|a| a.fuel_cost).sum();
            let window_count = window.actions.len();
            if window_count > 0 {
                let window_avg = window_fuel_total as f64 / window_count as f64;
                let factor = window_avg / baseline_fuel;
                if factor >= self.drift_threshold {
                    let severity = severity_from_factor(factor, self.drift_threshold);
                    alerts.push(DriftAlert {
                        agent_id: agent_id.to_string(),
                        drift_type: DriftType::ResourceAnomaly,
                        severity,
                        details: format!(
                            "avg fuel/action {:.1} vs baseline {:.1}",
                            window_avg, baseline_fuel
                        ),
                        current_value: window_avg,
                        baseline_value: baseline_fuel,
                        deviation_factor: factor,
                        timestamp: now,
                    });
                }
            }
        }

        alerts
    }

    pub fn establish_baseline(&mut self, agent_id: &str) -> bool {
        let Some(baseline) = self.baselines.get_mut(agent_id) else {
            return false;
        };

        if baseline.samples_collected < MIN_SAMPLES_FOR_BASELINE {
            return false;
        }

        baseline.established = true;
        true
    }

    pub fn get_baseline(&self, agent_id: &str) -> Option<&BehavioralBaseline> {
        self.baselines.get(agent_id)
    }

    pub fn reset_baseline(&mut self, agent_id: &str) {
        self.baselines.remove(agent_id);
        self.active_windows.remove(agent_id);
    }

    fn current_frequency(&self, window: &ActionWindow) -> f64 {
        if window.actions.len() < 2 {
            return 0.0;
        }
        let first = window.actions.first().map(|a| a.timestamp).unwrap_or(0);
        let last = window.actions.last().map(|a| a.timestamp).unwrap_or(0);
        let elapsed = last.saturating_sub(first);
        if elapsed == 0 {
            return 0.0;
        }
        (window.actions.len() as f64 / elapsed as f64) * 60.0
    }
}

fn severity_from_factor(factor: f64, threshold: f64) -> DriftSeverity {
    if factor >= threshold * 4.0 {
        DriftSeverity::Critical
    } else if factor >= threshold * 3.0 {
        DriftSeverity::High
    } else if factor >= threshold * 2.0 {
        DriftSeverity::Medium
    } else {
        DriftSeverity::Low
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_action(action_type: &str, timestamp: u64, fuel_cost: u64) -> ActionRecord {
        ActionRecord {
            action_type: action_type.to_string(),
            timestamp,
            fuel_cost,
            resource_usage: None,
        }
    }

    #[test]
    fn record_action_accumulates() {
        let mut profiler = BehavioralProfiler::new(2.0);

        for i in 0..10 {
            profiler.record_action("agent-1", make_action("tool_call", i, 10));
        }

        let baseline = profiler.get_baseline("agent-1").unwrap();
        assert_eq!(baseline.samples_collected, 10);
        assert_eq!(baseline.action_histogram.get("tool_call"), Some(&10));
    }

    #[test]
    fn baseline_not_established_under_minimum() {
        let mut profiler = BehavioralProfiler::new(2.0);

        for i in 0..30 {
            profiler.record_action("agent-1", make_action("tool_call", i, 10));
        }

        assert!(!profiler.establish_baseline("agent-1"));
        let baseline = profiler.get_baseline("agent-1").unwrap();
        assert!(!baseline.established);
    }

    #[test]
    fn baseline_established_at_threshold() {
        let mut profiler = BehavioralProfiler::new(2.0);

        for i in 0..50 {
            profiler.record_action("agent-1", make_action("tool_call", i, 10));
        }

        assert!(profiler.establish_baseline("agent-1"));
        let baseline = profiler.get_baseline("agent-1").unwrap();
        assert!(baseline.established);
        assert_eq!(baseline.samples_collected, 50);
    }

    #[test]
    fn no_drift_within_baseline() {
        let mut profiler = BehavioralProfiler::new(2.0);

        // Build baseline with "tool_call" actions at 1/sec, fuel=10.
        for i in 0..60 {
            profiler.record_action("agent-1", make_action("tool_call", i * 6, 10));
        }
        profiler.establish_baseline("agent-1");

        // Record actions matching the baseline pattern (same frequency, same fuel).
        let base_time = 1000;
        for i in 0..10 {
            profiler.record_action("agent-1", make_action("tool_call", base_time + i * 6, 10));
        }

        let alerts = profiler.check_drift("agent-1");
        assert!(
            alerts.is_empty(),
            "should have no drift alerts for baseline-matching behavior, got: {:?}",
            alerts.iter().map(|a| &a.details).collect::<Vec<_>>()
        );
    }

    #[test]
    fn baseline_requires_minimum_samples() {
        let mut profiler = BehavioralProfiler::new(2.0);

        for i in 0..49 {
            profiler.record_action("agent-1", make_action("tool_call", i, 10));
        }
        assert!(!profiler.establish_baseline("agent-1"));

        profiler.record_action("agent-1", make_action("tool_call", 49, 10));
        assert!(profiler.establish_baseline("agent-1"));

        let baseline = profiler.get_baseline("agent-1").unwrap();
        assert!(baseline.established);
        assert_eq!(baseline.samples_collected, 50);
    }

    #[test]
    fn no_drift_before_baseline_established() {
        let mut profiler = BehavioralProfiler::new(2.0);

        for i in 0..10 {
            profiler.record_action("agent-1", make_action("tool_call", i, 10));
        }

        let alerts = profiler.check_drift("agent-1");
        assert!(alerts.is_empty());
    }

    #[test]
    fn frequency_spike_detected() {
        let mut profiler = BehavioralProfiler::new(2.0);

        // Build baseline: 1 action every 6 seconds over 5 minutes = 10 actions/min.
        for i in 0..60 {
            profiler.record_action("agent-1", make_action("tool_call", i * 6, 10));
        }
        profiler.establish_baseline("agent-1");
        let baseline_freq = profiler.get_baseline("agent-1").unwrap().action_frequency;
        assert!(baseline_freq > 0.0, "baseline frequency should be set");

        // Spike: 1 action per second for 30 seconds (= 60/min), well past the
        // 10-minute window cutoff so old baseline actions are pruned.
        let base_time = 1000;
        for i in 0..30 {
            profiler.record_action("agent-1", make_action("tool_call", base_time + i, 10));
        }

        let alerts = profiler.check_drift("agent-1");
        let freq_alerts: Vec<_> = alerts
            .iter()
            .filter(|a| a.drift_type == DriftType::FrequencySpike)
            .collect();
        assert!(
            !freq_alerts.is_empty(),
            "should detect frequency spike, got alerts: {:?}",
            alerts
        );
        assert!(freq_alerts[0].deviation_factor >= 2.0);
    }

    #[test]
    fn unusual_action_type_detected() {
        let mut profiler = BehavioralProfiler::new(2.0);

        // Build baseline with only tool_call actions.
        for i in 0..60 {
            profiler.record_action("agent-1", make_action("tool_call", i, 10));
        }
        profiler.establish_baseline("agent-1");

        // Now do something never seen.
        profiler.record_action("agent-1", make_action("terminal_command", 100, 10));

        let alerts = profiler.check_drift("agent-1");
        let unusual: Vec<_> = alerts
            .iter()
            .filter(|a| a.drift_type == DriftType::UnusualActionType)
            .collect();
        assert!(!unusual.is_empty());
        assert_eq!(unusual[0].severity, DriftSeverity::High);
    }

    #[test]
    fn resource_anomaly_detected() {
        let mut profiler = BehavioralProfiler::new(2.0);

        // Build baseline with fuel_cost=10 per action.
        for i in 0..60 {
            profiler.record_action("agent-1", make_action("tool_call", i, 10));
        }
        profiler.establish_baseline("agent-1");

        // Spike fuel cost to 50 (5x baseline). Use timestamps far enough away
        // that baseline-era actions are pruned from the 10-minute window.
        let base_time = 1000;
        for i in 0..10 {
            profiler.record_action("agent-1", make_action("tool_call", base_time + i, 50));
        }

        let alerts = profiler.check_drift("agent-1");
        let resource_alerts: Vec<_> = alerts
            .iter()
            .filter(|a| a.drift_type == DriftType::ResourceAnomaly)
            .collect();
        assert!(!resource_alerts.is_empty());
        assert!(resource_alerts[0].deviation_factor >= 2.0);
    }

    #[test]
    fn reset_clears_everything() {
        let mut profiler = BehavioralProfiler::new(2.0);

        for i in 0..60 {
            profiler.record_action("agent-1", make_action("tool_call", i, 10));
        }
        profiler.establish_baseline("agent-1");
        assert!(profiler.get_baseline("agent-1").is_some());

        profiler.reset_baseline("agent-1");
        assert!(profiler.get_baseline("agent-1").is_none());
    }

    #[test]
    fn histogram_tracks_multiple_action_types() {
        let mut profiler = BehavioralProfiler::new(2.0);

        for i in 0..30 {
            profiler.record_action("agent-1", make_action("tool_call", i, 10));
        }
        for i in 30..60 {
            profiler.record_action("agent-1", make_action("llm_call", i, 20));
        }

        let baseline = profiler.get_baseline("agent-1").unwrap();
        assert_eq!(baseline.action_histogram.get("tool_call"), Some(&30));
        assert_eq!(baseline.action_histogram.get("llm_call"), Some(&30));
        assert_eq!(baseline.samples_collected, 60);
    }

    #[test]
    fn resource_running_average_converges() {
        let mut profiler = BehavioralProfiler::new(2.0);

        // 50 actions with fuel_cost=10, then 50 with fuel_cost=20.
        for i in 0..50 {
            profiler.record_action("agent-1", make_action("tool_call", i, 10));
        }
        for i in 50..100 {
            profiler.record_action("agent-1", make_action("tool_call", i, 20));
        }

        let baseline = profiler.get_baseline("agent-1").unwrap();
        // Running average of 50x10 + 50x20 over 100 samples = 15.0.
        let avg = baseline.typical_resource_usage.avg_fuel_per_action;
        assert!((avg - 15.0).abs() < 0.1, "expected ~15.0, got {}", avg);
        assert_eq!(baseline.typical_resource_usage.max_fuel_single_action, 20);
    }

    #[test]
    fn unknown_agent_returns_no_alerts() {
        let profiler = BehavioralProfiler::new(2.0);
        assert!(profiler.check_drift("nonexistent").is_empty());
    }

    #[test]
    fn severity_escalates_with_deviation() {
        assert_eq!(severity_from_factor(2.0, 2.0), DriftSeverity::Low);
        assert_eq!(severity_from_factor(4.0, 2.0), DriftSeverity::Medium);
        assert_eq!(severity_from_factor(6.0, 2.0), DriftSeverity::High);
        assert_eq!(severity_from_factor(8.0, 2.0), DriftSeverity::Critical);
    }
}
