//! # Config Optimizer
//!
//! Runtime configuration optimization engine. Analyzes system metrics against
//! tunable parameters and proposes bounded adjustments.

use crate::types::{ProposedChange, SystemMetrics};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Category of tunable parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ParameterCategory {
    Scheduling,
    Caching,
    RateLimiting,
    Memory,
    Inference,
}

/// A single tunable parameter with bounds.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunableParameter {
    pub key: String,
    pub description: String,
    pub current_value: f64,
    pub min_value: f64,
    pub max_value: f64,
    pub step_size: f64,
    pub impact_metric: String,
    pub category: ParameterCategory,
}

/// A suggestion to change a config parameter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigSuggestion {
    pub parameter_key: String,
    pub current_value: f64,
    pub suggested_value: f64,
    pub reasoning: String,
    pub expected_impact: f64,
}

/// Configuration for the optimizer.
#[derive(Debug, Clone)]
pub struct ConfigOptimizerConfig {
    /// Minimum metric deviation to trigger a suggestion (0.0–1.0).
    pub trigger_threshold: f64,
}

impl Default for ConfigOptimizerConfig {
    fn default() -> Self {
        Self {
            trigger_threshold: 0.1,
        }
    }
}

/// The config optimizer engine.
pub struct ConfigOptimizer {
    config: ConfigOptimizerConfig,
    pub parameters: HashMap<String, TunableParameter>,
}

impl ConfigOptimizer {
    pub fn new(config: ConfigOptimizerConfig) -> Self {
        let mut optimizer = Self {
            config,
            parameters: HashMap::new(),
        };
        optimizer.register_defaults();
        optimizer
    }

    /// Register the default tunable parameters for Nexus OS.
    pub fn register_defaults(&mut self) {
        let defaults = vec![
            TunableParameter {
                key: "agent_max_concurrent_tasks".into(),
                description: "Maximum concurrent tasks per agent".into(),
                current_value: 3.0,
                min_value: 1.0,
                max_value: 10.0,
                step_size: 1.0,
                impact_metric: "scheduling_wait_time".into(),
                category: ParameterCategory::Scheduling,
            },
            TunableParameter {
                key: "memory_gc_interval_seconds".into(),
                description: "Interval between memory garbage collection cycles".into(),
                current_value: 300.0,
                min_value: 30.0,
                max_value: 3600.0,
                step_size: 30.0,
                impact_metric: "memory_usage_bytes".into(),
                category: ParameterCategory::Memory,
            },
            TunableParameter {
                key: "cache_max_entries".into(),
                description: "Maximum entries in the LRU cache".into(),
                current_value: 1000.0,
                min_value: 100.0,
                max_value: 10000.0,
                step_size: 100.0,
                impact_metric: "cache_hit_rate".into(),
                category: ParameterCategory::Caching,
            },
            TunableParameter {
                key: "llm_request_timeout_seconds".into(),
                description: "Timeout for LLM API requests".into(),
                current_value: 60.0,
                min_value: 10.0,
                max_value: 300.0,
                step_size: 5.0,
                impact_metric: "llm_timeout_rate".into(),
                category: ParameterCategory::Inference,
            },
            TunableParameter {
                key: "inference_batch_size".into(),
                description: "Batch size for inference requests".into(),
                current_value: 4.0,
                min_value: 1.0,
                max_value: 32.0,
                step_size: 1.0,
                impact_metric: "inference_throughput".into(),
                category: ParameterCategory::Inference,
            },
            TunableParameter {
                key: "audit_flush_interval_ms".into(),
                description: "Interval for flushing audit events to storage".into(),
                current_value: 1000.0,
                min_value: 100.0,
                max_value: 5000.0,
                step_size: 100.0,
                impact_metric: "audit_write_latency".into(),
                category: ParameterCategory::Memory,
            },
            TunableParameter {
                key: "fuel_warning_threshold_percent".into(),
                description: "Fuel level at which to warn agents".into(),
                current_value: 80.0,
                min_value: 50.0,
                max_value: 95.0,
                step_size: 5.0,
                impact_metric: "fuel_exhaustion_rate".into(),
                category: ParameterCategory::RateLimiting,
            },
            TunableParameter {
                key: "canary_duration_minutes".into(),
                description: "Duration of canary monitoring after improvement".into(),
                current_value: 30.0,
                min_value: 5.0,
                max_value: 120.0,
                step_size: 5.0,
                impact_metric: "rollback_rate".into(),
                category: ParameterCategory::Scheduling,
            },
        ];

        for p in defaults {
            self.parameters.insert(p.key.clone(), p);
        }
    }

    /// Analyze config against metrics and suggest adjustments.
    pub fn analyze_config(&self, metrics: &SystemMetrics) -> Vec<ConfigSuggestion> {
        let metric_map: HashMap<&str, f64> = metrics.iter().map(|(k, &v)| (k, v)).collect();
        let mut suggestions = Vec::new();

        for param in self.parameters.values() {
            if let Some(&metric_value) = metric_map.get(param.impact_metric.as_str()) {
                if let Some(suggestion) = self.suggest_for_parameter(param, metric_value) {
                    suggestions.push(suggestion);
                }
            }
        }

        // Sort by expected impact (highest first)
        suggestions.sort_by(|a, b| {
            b.expected_impact
                .partial_cmp(&a.expected_impact)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        suggestions
    }

    /// Generate a ProposedChange from a suggestion.
    pub fn propose_change(&self, suggestion: &ConfigSuggestion) -> ProposedChange {
        ProposedChange::ConfigChange {
            key: suggestion.parameter_key.clone(),
            old_value: serde_json::Value::from(suggestion.current_value),
            new_value: serde_json::Value::from(suggestion.suggested_value),
            justification: suggestion.reasoning.clone(),
        }
    }

    fn suggest_for_parameter(
        &self,
        param: &TunableParameter,
        metric_value: f64,
    ) -> Option<ConfigSuggestion> {
        // Heuristic: if metric deviates significantly, adjust the parameter
        let deviation = (metric_value - 1.0).abs(); // Assume 1.0 = ideal
        if deviation < self.config.trigger_threshold {
            return None;
        }

        // Direction: if metric > 1.0 (too high), decrease param; if < 1.0, increase
        let direction = if metric_value > 1.0 { -1.0 } else { 1.0 };
        let adjustment = direction * param.step_size;
        let suggested = clamp(
            param.current_value + adjustment,
            param.min_value,
            param.max_value,
        );

        // Don't suggest if already at the limit
        if (suggested - param.current_value).abs() < 1e-9 {
            return None;
        }

        Some(ConfigSuggestion {
            parameter_key: param.key.clone(),
            current_value: param.current_value,
            suggested_value: suggested,
            reasoning: format!(
                "{} metric at {:.2} (deviation {:.2}); adjusting {} from {:.0} to {:.0}",
                param.impact_metric,
                metric_value,
                deviation,
                param.key,
                param.current_value,
                suggested,
            ),
            expected_impact: deviation * 0.5,
        })
    }
}

fn clamp(value: f64, min: f64, max: f64) -> f64 {
    value.max(min).min(max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parameter_within_bounds() {
        let optimizer = ConfigOptimizer::new(ConfigOptimizerConfig::default());
        let param = &optimizer.parameters["cache_max_entries"];
        assert!(param.current_value >= param.min_value);
        assert!(param.current_value <= param.max_value);
    }

    #[test]
    fn test_parameter_exceeds_max_clamped() {
        assert!((clamp(15000.0, 100.0, 10000.0) - 10000.0).abs() < 1e-9);
        assert!((clamp(50.0, 100.0, 10000.0) - 100.0).abs() < 1e-9);
    }

    #[test]
    fn test_suggestion_direction() {
        let optimizer = ConfigOptimizer::new(ConfigOptimizerConfig {
            trigger_threshold: 0.05,
        });

        // High metric (>1.0) → decrease parameter
        let mut metrics = SystemMetrics::new();
        metrics.insert("cache_hit_rate", 1.5); // too high → decrease cache size
        let suggestions = optimizer.analyze_config(&metrics);
        if let Some(s) = suggestions.first() {
            assert!(
                s.suggested_value < s.current_value,
                "high metric should decrease param: {} -> {}",
                s.current_value,
                s.suggested_value
            );
        }
    }

    #[test]
    fn test_default_parameters_registered() {
        let optimizer = ConfigOptimizer::new(ConfigOptimizerConfig::default());
        assert!(
            optimizer.parameters.len() >= 8,
            "should have at least 8 defaults, got {}",
            optimizer.parameters.len()
        );
        assert!(optimizer.parameters.contains_key("cache_max_entries"));
        assert!(optimizer
            .parameters
            .contains_key("llm_request_timeout_seconds"));
    }

    #[test]
    fn test_step_size_enforced() {
        let optimizer = ConfigOptimizer::new(ConfigOptimizerConfig {
            trigger_threshold: 0.05,
        });

        let mut metrics = SystemMetrics::new();
        metrics.insert("cache_hit_rate", 0.5); // low → increase

        let suggestions = optimizer.analyze_config(&metrics);
        if let Some(s) = suggestions.first() {
            let param = &optimizer.parameters[&s.parameter_key];
            let delta = (s.suggested_value - s.current_value).abs();
            assert!(
                (delta - param.step_size).abs() < 1e-9 || delta < param.step_size + 1e-9,
                "change should be step_size ({}), got {}",
                param.step_size,
                delta
            );
        }
    }
}
