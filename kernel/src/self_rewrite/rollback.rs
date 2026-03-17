//! Automatic rollback engine — monitors system health after patch application
//! and automatically reverts patches that cause regressions.
//!
//! Health is monitored for 5 minutes (configurable) after each patch.
//! Any regression triggers immediate rollback.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use super::SelfRewriteError;

/// A record of a rollback event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackEvent {
    pub patch_id: Uuid,
    pub reason: String,
    pub reverted_at: u64,
    pub health_metrics_before: Value,
    pub health_metrics_after: Value,
}

/// Health snapshot used for comparison.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthSnapshot {
    pub avg_latency_ms: f64,
    pub error_rate: f64,
    pub memory_usage_bytes: u64,
    pub cpu_usage_pct: f64,
    pub timestamp: u64,
}

impl HealthSnapshot {
    /// Convert to a serde_json::Value for storage.
    pub fn to_value(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }
}

/// Automatic rollback engine.
#[derive(Debug, Clone)]
pub struct RollbackEngine {
    /// Monitoring window in seconds (default 300 = 5 minutes).
    monitoring_window_secs: u64,
    /// Latency regression threshold (percentage increase that triggers rollback).
    latency_regression_threshold_pct: f64,
    /// Error rate increase threshold that triggers rollback.
    error_rate_threshold: f64,
    /// Memory increase threshold (percentage) that triggers rollback.
    memory_regression_threshold_pct: f64,
    /// History of rollback events.
    rollback_history: Vec<RollbackEvent>,
    /// Pre-patch health snapshots keyed by patch ID.
    baselines: std::collections::HashMap<Uuid, HealthSnapshot>,
}

impl RollbackEngine {
    pub fn new() -> Self {
        Self {
            monitoring_window_secs: 300, // 5 minutes
            latency_regression_threshold_pct: 10.0,
            error_rate_threshold: 0.01,
            memory_regression_threshold_pct: 20.0,
            rollback_history: Vec::new(),
            baselines: std::collections::HashMap::new(),
        }
    }

    pub fn with_thresholds(
        monitoring_window_secs: u64,
        latency_regression_pct: f64,
        error_rate_threshold: f64,
        memory_regression_pct: f64,
    ) -> Self {
        Self {
            monitoring_window_secs,
            latency_regression_threshold_pct: latency_regression_pct,
            error_rate_threshold,
            memory_regression_threshold_pct: memory_regression_pct,
            rollback_history: Vec::new(),
            baselines: std::collections::HashMap::new(),
        }
    }

    /// Record a pre-patch health baseline for monitoring.
    pub fn record_baseline(&mut self, patch_id: Uuid, snapshot: HealthSnapshot) {
        self.baselines.insert(patch_id, snapshot);
    }

    /// Get the monitoring window in seconds.
    pub fn monitoring_window(&self) -> u64 {
        self.monitoring_window_secs
    }

    /// Monitor health by comparing the current snapshot against the baseline.
    /// Returns `Ok(true)` if healthy, `Ok(false)` if regression detected,
    /// or `Err` if no baseline exists.
    pub fn monitor_health(
        &self,
        patch_id: Uuid,
        current: &HealthSnapshot,
    ) -> Result<bool, SelfRewriteError> {
        let baseline = self
            .baselines
            .get(&patch_id)
            .ok_or_else(|| SelfRewriteError::PatchNotFound(patch_id.to_string()))?;

        // Check latency regression
        if baseline.avg_latency_ms > 0.0 {
            let latency_increase_pct = ((current.avg_latency_ms - baseline.avg_latency_ms)
                / baseline.avg_latency_ms)
                * 100.0;
            if latency_increase_pct > self.latency_regression_threshold_pct {
                return Ok(false);
            }
        }

        // Check error rate increase
        let error_rate_delta = current.error_rate - baseline.error_rate;
        if error_rate_delta > self.error_rate_threshold {
            return Ok(false);
        }

        // Check memory regression
        if baseline.memory_usage_bytes > 0 {
            let memory_increase_pct = ((current.memory_usage_bytes as f64
                - baseline.memory_usage_bytes as f64)
                / baseline.memory_usage_bytes as f64)
                * 100.0;
            if memory_increase_pct > self.memory_regression_threshold_pct {
                return Ok(false);
            }
        }

        Ok(true)
    }

    /// Trigger a rollback for a patch due to detected regression.
    pub fn trigger_rollback(
        &mut self,
        patch_id: Uuid,
        reason: &str,
        current_health: &HealthSnapshot,
    ) -> Result<RollbackEvent, SelfRewriteError> {
        let baseline = self
            .baselines
            .get(&patch_id)
            .ok_or_else(|| SelfRewriteError::PatchNotFound(patch_id.to_string()))?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let event = RollbackEvent {
            patch_id,
            reason: reason.to_string(),
            reverted_at: now,
            health_metrics_before: baseline.to_value(),
            health_metrics_after: current_health.to_value(),
        };

        self.rollback_history.push(event.clone());
        self.baselines.remove(&patch_id);

        Ok(event)
    }

    /// Get the full rollback history.
    pub fn get_rollback_history(&self) -> &[RollbackEvent] {
        &self.rollback_history
    }

    /// Get rollback events for a specific patch.
    pub fn get_patch_rollbacks(&self, patch_id: Uuid) -> Vec<&RollbackEvent> {
        self.rollback_history
            .iter()
            .filter(|e| e.patch_id == patch_id)
            .collect()
    }

    /// Check if a patch has been rolled back before.
    pub fn was_rolled_back(&self, patch_id: Uuid) -> bool {
        self.rollback_history.iter().any(|e| e.patch_id == patch_id)
    }
}

impl Default for RollbackEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn baseline_snapshot() -> HealthSnapshot {
        HealthSnapshot {
            avg_latency_ms: 10.0,
            error_rate: 0.01,
            memory_usage_bytes: 100 * 1024 * 1024,
            cpu_usage_pct: 30.0,
            timestamp: 1700000000,
        }
    }

    fn healthy_snapshot() -> HealthSnapshot {
        HealthSnapshot {
            avg_latency_ms: 9.5,
            error_rate: 0.01,
            memory_usage_bytes: 100 * 1024 * 1024,
            cpu_usage_pct: 28.0,
            timestamp: 1700000300,
        }
    }

    fn regressed_latency_snapshot() -> HealthSnapshot {
        HealthSnapshot {
            avg_latency_ms: 20.0, // 100% increase
            error_rate: 0.01,
            memory_usage_bytes: 100 * 1024 * 1024,
            cpu_usage_pct: 30.0,
            timestamp: 1700000300,
        }
    }

    fn regressed_error_snapshot() -> HealthSnapshot {
        HealthSnapshot {
            avg_latency_ms: 10.0,
            error_rate: 0.10, // big increase
            memory_usage_bytes: 100 * 1024 * 1024,
            cpu_usage_pct: 30.0,
            timestamp: 1700000300,
        }
    }

    fn regressed_memory_snapshot() -> HealthSnapshot {
        HealthSnapshot {
            avg_latency_ms: 10.0,
            error_rate: 0.01,
            memory_usage_bytes: 200 * 1024 * 1024, // doubled
            cpu_usage_pct: 30.0,
            timestamp: 1700000300,
        }
    }

    #[test]
    fn healthy_system_passes() {
        let mut engine = RollbackEngine::new();
        let patch_id = Uuid::new_v4();
        engine.record_baseline(patch_id, baseline_snapshot());

        let healthy = engine
            .monitor_health(patch_id, &healthy_snapshot())
            .unwrap();
        assert!(healthy);
    }

    #[test]
    fn latency_regression_detected() {
        let mut engine = RollbackEngine::new();
        let patch_id = Uuid::new_v4();
        engine.record_baseline(patch_id, baseline_snapshot());

        let healthy = engine
            .monitor_health(patch_id, &regressed_latency_snapshot())
            .unwrap();
        assert!(!healthy);
    }

    #[test]
    fn error_rate_regression_detected() {
        let mut engine = RollbackEngine::new();
        let patch_id = Uuid::new_v4();
        engine.record_baseline(patch_id, baseline_snapshot());

        let healthy = engine
            .monitor_health(patch_id, &regressed_error_snapshot())
            .unwrap();
        assert!(!healthy);
    }

    #[test]
    fn memory_regression_detected() {
        let mut engine = RollbackEngine::new();
        let patch_id = Uuid::new_v4();
        engine.record_baseline(patch_id, baseline_snapshot());

        let healthy = engine
            .monitor_health(patch_id, &regressed_memory_snapshot())
            .unwrap();
        assert!(!healthy);
    }

    #[test]
    fn trigger_rollback_records_event() {
        let mut engine = RollbackEngine::new();
        let patch_id = Uuid::new_v4();
        engine.record_baseline(patch_id, baseline_snapshot());

        let current = regressed_latency_snapshot();
        let event = engine
            .trigger_rollback(patch_id, "latency regression", &current)
            .unwrap();

        assert_eq!(event.patch_id, patch_id);
        assert!(event.reason.contains("latency"));
        assert_eq!(engine.get_rollback_history().len(), 1);
    }

    #[test]
    fn rollback_removes_baseline() {
        let mut engine = RollbackEngine::new();
        let patch_id = Uuid::new_v4();
        engine.record_baseline(patch_id, baseline_snapshot());

        let current = regressed_latency_snapshot();
        engine
            .trigger_rollback(patch_id, "regression", &current)
            .unwrap();

        // Monitoring should now fail — no baseline
        let err = engine
            .monitor_health(patch_id, &healthy_snapshot())
            .unwrap_err();
        assert!(matches!(err, SelfRewriteError::PatchNotFound(_)));
    }

    #[test]
    fn was_rolled_back_check() {
        let mut engine = RollbackEngine::new();
        let patch_id = Uuid::new_v4();
        engine.record_baseline(patch_id, baseline_snapshot());

        assert!(!engine.was_rolled_back(patch_id));

        engine
            .trigger_rollback(patch_id, "test", &regressed_latency_snapshot())
            .unwrap();
        assert!(engine.was_rolled_back(patch_id));
    }

    #[test]
    fn monitoring_window_default() {
        let engine = RollbackEngine::new();
        assert_eq!(engine.monitoring_window(), 300);
    }

    #[test]
    fn custom_thresholds() {
        let engine = RollbackEngine::with_thresholds(60, 5.0, 0.005, 10.0);
        assert_eq!(engine.monitoring_window(), 60);
    }

    #[test]
    fn rollback_event_serialization() {
        let event = RollbackEvent {
            patch_id: Uuid::new_v4(),
            reason: "latency regression".into(),
            reverted_at: 1700000300,
            health_metrics_before: serde_json::json!({"latency": 10.0}),
            health_metrics_after: serde_json::json!({"latency": 20.0}),
        };
        let json = serde_json::to_string(&event).unwrap();
        let back: RollbackEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(back.patch_id, event.patch_id);
        assert_eq!(back.reason, "latency regression");
    }

    #[test]
    fn health_snapshot_serialization() {
        let snap = baseline_snapshot();
        let val = snap.to_value();
        assert!(val.get("avg_latency_ms").is_some());

        let json = serde_json::to_string(&snap).unwrap();
        let back: HealthSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(back.avg_latency_ms, 10.0);
    }

    #[test]
    fn monitor_missing_baseline_errors() {
        let engine = RollbackEngine::new();
        let err = engine
            .monitor_health(Uuid::new_v4(), &healthy_snapshot())
            .unwrap_err();
        assert!(matches!(err, SelfRewriteError::PatchNotFound(_)));
    }
}
