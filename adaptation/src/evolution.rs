//! Self-improving agent evolution engine.
//!
//! Agents autonomously improve their strategies through mutation, evaluation,
//! and selection — with governance approval gates before deployment.

use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use uuid::Uuid;

// ── Configuration ───────────────────────────────────────────────────────────

/// Controls the evolution engine's behaviour.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvolutionConfig {
    pub enabled: bool,
    pub max_generations: usize,
    pub improvement_threshold: f64,
    pub rollback_on_regression: bool,
    pub require_approval: bool,
    pub evaluation_samples: usize,
}

impl Default for EvolutionConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            max_generations: 100,
            improvement_threshold: 0.05,
            rollback_on_regression: true,
            require_approval: true,
            evaluation_samples: 10,
        }
    }
}

// ── Core types ──────────────────────────────────────────────────────────────

/// A versioned strategy attached to an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Strategy {
    pub id: String,
    pub version: u32,
    pub agent_id: String,
    pub name: String,
    pub parameters: serde_json::Value,
    pub score: f64,
    pub created_at: u64,
    pub parent_id: Option<String>,
}

/// Outcome of a single evolution step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvolutionResult {
    pub generation: u32,
    pub parent_strategy: String,
    pub child_strategy: String,
    pub parent_score: f64,
    pub child_score: f64,
    pub improvement: f64,
    pub accepted: bool,
    pub reason: String,
}

/// Accumulated history of evolution for one agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvolutionHistory {
    pub agent_id: String,
    pub total_generations: u32,
    pub total_improvements: u32,
    pub total_regressions: u32,
    pub current_best_score: f64,
    pub results: Vec<EvolutionResult>,
}

/// The kind of mutation applied to produce a candidate strategy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MutationType {
    ParameterTweak,
    StrategySwap,
    PromptRefinement,
    ThresholdAdjustment,
    Custom(String),
}

// ── Evolution engine ────────────────────────────────────────────────────────

/// Drives iterative strategy improvement for governed agents.
pub struct EvolutionEngine {
    config: EvolutionConfig,
    strategies: HashMap<String, Strategy>,
    active_strategies: HashMap<String, String>,
    history: HashMap<String, EvolutionHistory>,
}

impl EvolutionEngine {
    pub fn new(config: EvolutionConfig) -> Self {
        Self {
            config,
            strategies: HashMap::new(),
            active_strategies: HashMap::new(),
            history: HashMap::new(),
        }
    }

    /// Returns the engine configuration.
    pub fn config(&self) -> &EvolutionConfig {
        &self.config
    }

    /// Total number of registered strategies across all agents.
    pub fn total_strategies(&self) -> usize {
        self.strategies.len()
    }

    /// Number of agents with an active strategy.
    pub fn active_agent_count(&self) -> usize {
        self.active_strategies.len()
    }

    /// Register a strategy. The first strategy for an agent becomes active.
    pub fn register_strategy(&mut self, strategy: Strategy) -> Result<(), String> {
        let agent_id = strategy.agent_id.clone();
        let strategy_id = strategy.id.clone();

        self.strategies.insert(strategy_id.clone(), strategy);

        // Set as active if agent has no active strategy yet
        self.active_strategies
            .entry(agent_id)
            .or_insert(strategy_id);

        Ok(())
    }

    /// Get the currently active strategy for an agent.
    pub fn get_active_strategy(&self, agent_id: &str) -> Option<&Strategy> {
        self.active_strategies
            .get(agent_id)
            .and_then(|id| self.strategies.get(id))
    }

    /// Create a mutated copy of a strategy.
    pub fn mutate_strategy(&self, strategy: &Strategy, mutation: MutationType) -> Strategy {
        let new_id = Uuid::new_v4().to_string();
        let seed = deterministic_seed(&strategy.id, strategy.version);
        let new_params = apply_mutation(&strategy.parameters, &mutation, seed);

        Strategy {
            id: new_id,
            version: strategy.version + 1,
            agent_id: strategy.agent_id.clone(),
            name: strategy.name.clone(),
            parameters: new_params,
            score: 0.0,
            created_at: now_secs(),
            parent_id: Some(strategy.id.clone()),
        }
    }

    /// Evaluate a strategy by running `test_fn` multiple times and averaging.
    pub fn evaluate_strategy(
        &self,
        strategy: &Strategy,
        test_fn: impl Fn(&Strategy) -> f64,
    ) -> f64 {
        let samples = self.config.evaluation_samples.max(1);
        let total: f64 = (0..samples).map(|_| test_fn(strategy)).sum();
        total / samples as f64
    }

    /// Run one evolution step for an agent.
    pub fn evolve_once(
        &mut self,
        agent_id: &str,
        mutation: MutationType,
        test_fn: impl Fn(&Strategy) -> f64,
    ) -> Result<EvolutionResult, String> {
        let active_id = self
            .active_strategies
            .get(agent_id)
            .ok_or_else(|| format!("No active strategy for agent {agent_id}"))?
            .clone();

        let current = self
            .strategies
            .get(&active_id)
            .ok_or("Active strategy not found in registry")?
            .clone();

        let candidate = self.mutate_strategy(&current, mutation);
        let parent_score = self.evaluate_strategy(&current, &test_fn);
        let child_score = self.evaluate_strategy(&candidate, &test_fn);
        let improvement = child_score - parent_score;

        let (accepted, reason) = if improvement >= self.config.improvement_threshold {
            (
                true,
                format!("Improvement of {improvement:.4} exceeds threshold"),
            )
        } else if improvement < 0.0 && self.config.rollback_on_regression {
            (
                false,
                format!("Regression of {improvement:.4} — rollback policy active"),
            )
        } else {
            (
                false,
                format!(
                    "Improvement {improvement:.4} below threshold {}",
                    self.config.improvement_threshold
                ),
            )
        };

        let child_id = candidate.id.clone();

        // Store candidate strategy
        let mut stored_candidate = candidate;
        stored_candidate.score = child_score;
        self.strategies
            .insert(stored_candidate.id.clone(), stored_candidate);

        // Update parent score
        if let Some(parent) = self.strategies.get_mut(&active_id) {
            parent.score = parent_score;
        }

        // Switch active if accepted
        if accepted {
            self.active_strategies
                .insert(agent_id.to_string(), child_id.clone());
        }

        let history =
            self.history
                .entry(agent_id.to_string())
                .or_insert_with(|| EvolutionHistory {
                    agent_id: agent_id.to_string(),
                    total_generations: 0,
                    total_improvements: 0,
                    total_regressions: 0,
                    current_best_score: 0.0,
                    results: Vec::new(),
                });

        history.total_generations += 1;
        if accepted {
            history.total_improvements += 1;
            history.current_best_score = child_score;
        } else if improvement < 0.0 {
            history.total_regressions += 1;
        }
        // Keep current_best_score up to date even without improvement
        if parent_score > history.current_best_score {
            history.current_best_score = parent_score;
        }

        let result = EvolutionResult {
            generation: history.total_generations,
            parent_strategy: active_id,
            child_strategy: child_id,
            parent_score,
            child_score,
            improvement,
            accepted,
            reason,
        };

        history.results.push(result.clone());

        Ok(result)
    }

    /// Run multiple evolution steps, stopping early if stuck.
    pub fn evolve_loop(
        &mut self,
        agent_id: &str,
        max_generations: Option<usize>,
        test_fn: impl Fn(&Strategy) -> f64,
    ) -> Result<EvolutionHistory, String> {
        let limit = max_generations.unwrap_or(self.config.max_generations);
        let mut no_improvement_streak = 0u32;
        let mutation_types = [
            MutationType::ParameterTweak,
            MutationType::ThresholdAdjustment,
            MutationType::PromptRefinement,
            MutationType::StrategySwap,
        ];

        for i in 0..limit {
            let mutation = mutation_types[i % mutation_types.len()].clone();
            let result = self.evolve_once(agent_id, mutation, &test_fn)?;

            if result.accepted {
                no_improvement_streak = 0;
            } else {
                no_improvement_streak += 1;
            }

            if no_improvement_streak >= 10 {
                break;
            }
        }

        self.history
            .get(agent_id)
            .cloned()
            .ok_or_else(|| format!("No history found for agent {agent_id}"))
    }

    /// Revert to the parent of the current active strategy.
    pub fn rollback(&mut self, agent_id: &str) -> Result<Strategy, String> {
        let active_id = self
            .active_strategies
            .get(agent_id)
            .ok_or_else(|| format!("No active strategy for agent {agent_id}"))?
            .clone();

        let current = self
            .strategies
            .get(&active_id)
            .ok_or("Active strategy not found")?;

        let parent_id = current
            .parent_id
            .clone()
            .ok_or_else(|| "Cannot rollback: strategy has no parent".to_string())?;

        // Verify parent exists
        if !self.strategies.contains_key(&parent_id) {
            return Err("Parent strategy not found in registry".to_string());
        }

        self.active_strategies
            .insert(agent_id.to_string(), parent_id.clone());

        self.strategies
            .get(&parent_id)
            .cloned()
            .ok_or_else(|| "Parent strategy not found after insertion".to_string())
    }

    /// Get evolution history for an agent.
    pub fn get_history(&self, agent_id: &str) -> Option<&EvolutionHistory> {
        self.history.get(agent_id)
    }

    /// Return the highest-scoring strategy for an agent.
    pub fn best_strategy(&self, agent_id: &str) -> Option<&Strategy> {
        self.strategies
            .values()
            .filter(|s| s.agent_id == agent_id)
            .max_by(|a, b| {
                a.score
                    .partial_cmp(&b.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }

    /// Export all strategies (for cross-agent learning).
    pub fn export_strategies(&self) -> Vec<&Strategy> {
        self.strategies.values().collect()
    }
}

// ── Deterministic mutation helpers ──────────────────────────────────────────

/// Produce a deterministic seed from a strategy ID and version.
fn deterministic_seed(id: &str, version: u32) -> u64 {
    let mut hasher = DefaultHasher::new();
    id.hash(&mut hasher);
    version.hash(&mut hasher);
    hasher.finish()
}

/// Apply a mutation to JSON parameters using a deterministic seed.
fn apply_mutation(
    params: &serde_json::Value,
    mutation: &MutationType,
    seed: u64,
) -> serde_json::Value {
    match mutation {
        MutationType::ParameterTweak => tweak_numeric_values(params, seed, 0.10),
        MutationType::ThresholdAdjustment => tweak_numeric_values(params, seed, 0.05),
        MutationType::PromptRefinement => refine_prompt(params, seed),
        MutationType::StrategySwap => swap_strategy(params, seed),
        MutationType::Custom(_) => params.clone(),
    }
}

/// Perturb every numeric value in a JSON tree by ±`magnitude` (deterministic).
fn tweak_numeric_values(value: &serde_json::Value, seed: u64, magnitude: f64) -> serde_json::Value {
    match value {
        serde_json::Value::Number(n) => {
            if let Some(f) = n.as_f64() {
                // Deterministic perturbation in [-magnitude, +magnitude]
                let factor = deterministic_factor(seed, magnitude);
                let tweaked = f * (1.0 + factor);
                serde_json::Value::from(tweaked)
            } else {
                value.clone()
            }
        }
        serde_json::Value::Object(map) => {
            let mut new_map = serde_json::Map::new();
            for (i, (k, v)) in map.iter().enumerate() {
                let child_seed = seed
                    .wrapping_add(i as u64)
                    .wrapping_mul(6364136223846793005);
                new_map.insert(k.clone(), tweak_numeric_values(v, child_seed, magnitude));
            }
            serde_json::Value::Object(new_map)
        }
        serde_json::Value::Array(arr) => {
            let new_arr: Vec<serde_json::Value> = arr
                .iter()
                .enumerate()
                .map(|(i, v)| {
                    let child_seed = seed
                        .wrapping_add(i as u64)
                        .wrapping_mul(1442695040888963407);
                    tweak_numeric_values(v, child_seed, magnitude)
                })
                .collect();
            serde_json::Value::Array(new_arr)
        }
        _ => value.clone(),
    }
}

/// Deterministic factor in [-magnitude, +magnitude] from a seed.
fn deterministic_factor(seed: u64, magnitude: f64) -> f64 {
    // Map seed bits to [-1.0, 1.0]
    let normalized = ((seed % 10000) as f64 / 5000.0) - 1.0;
    normalized * magnitude
}

/// If parameters contain a `system_prompt` field, append optimization hints.
fn refine_prompt(params: &serde_json::Value, _seed: u64) -> serde_json::Value {
    let mut cloned = params.clone();
    if let Some(obj) = cloned.as_object_mut() {
        if let Some(serde_json::Value::String(prompt)) = obj.get("system_prompt") {
            let refined = format!(
                "{} Be concise and precise. Prioritize accuracy over verbosity.",
                prompt
            );
            obj.insert(
                "system_prompt".to_string(),
                serde_json::Value::String(refined),
            );
        }
    }
    cloned
}

/// Swap strategy: rearrange parameter keys or introduce alternative structure.
fn swap_strategy(params: &serde_json::Value, seed: u64) -> serde_json::Value {
    let mut cloned = params.clone();
    if let Some(obj) = cloned.as_object_mut() {
        // Add a marker indicating the swap generation
        obj.insert(
            "_swap_generation".to_string(),
            serde_json::Value::from(seed % 1000),
        );
    }
    cloned
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_strategy(agent_id: &str, name: &str, params: serde_json::Value) -> Strategy {
        Strategy {
            id: Uuid::new_v4().to_string(),
            version: 1,
            agent_id: agent_id.to_string(),
            name: name.to_string(),
            parameters: params,
            score: 0.0,
            created_at: now_secs(),
            parent_id: None,
        }
    }

    #[test]
    fn test_config_defaults() {
        let config = EvolutionConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.max_generations, 100);
        assert!((config.improvement_threshold - 0.05).abs() < f64::EPSILON);
        assert!(config.rollback_on_regression);
        assert!(config.require_approval);
        assert_eq!(config.evaluation_samples, 10);
    }

    #[test]
    fn test_register_strategy() {
        let mut engine = EvolutionEngine::new(EvolutionConfig::default());
        let strategy = make_strategy("agent-1", "baseline", json!({"lr": 0.01}));
        let id = strategy.id.clone();
        engine.register_strategy(strategy).unwrap();
        assert!(engine.strategies.contains_key(&id));
    }

    #[test]
    fn test_register_sets_active() {
        let mut engine = EvolutionEngine::new(EvolutionConfig::default());
        let strategy = make_strategy("agent-1", "baseline", json!({"lr": 0.01}));
        let id = strategy.id.clone();
        engine.register_strategy(strategy).unwrap();
        assert_eq!(engine.active_strategies.get("agent-1"), Some(&id));
    }

    #[test]
    fn test_mutate_parameter_tweak() {
        let engine = EvolutionEngine::new(EvolutionConfig::default());
        let strategy = make_strategy("agent-1", "baseline", json!({"lr": 0.01, "epochs": 10}));
        let mutated = engine.mutate_strategy(&strategy, MutationType::ParameterTweak);

        assert_ne!(mutated.id, strategy.id);
        assert_eq!(mutated.agent_id, strategy.agent_id);
        assert_eq!(mutated.version, strategy.version + 1);
        assert_eq!(mutated.parent_id, Some(strategy.id.clone()));
        // Parameters should differ (numeric values tweaked)
        assert_ne!(mutated.parameters, strategy.parameters);
    }

    #[test]
    fn test_mutate_preserves_structure() {
        let engine = EvolutionEngine::new(EvolutionConfig::default());
        let params = json!({
            "lr": 0.01,
            "epochs": 10,
            "label": "test",
            "nested": {"alpha": 0.5}
        });
        let strategy = make_strategy("agent-1", "baseline", params);
        let mutated = engine.mutate_strategy(&strategy, MutationType::ParameterTweak);

        let orig_obj = strategy.parameters.as_object().unwrap();
        let mutated_obj = mutated.parameters.as_object().unwrap();

        // Same keys preserved
        assert_eq!(orig_obj.len(), mutated_obj.len());
        for key in orig_obj.keys() {
            assert!(mutated_obj.contains_key(key), "Missing key: {key}");
        }
        // String values unchanged
        assert_eq!(mutated_obj.get("label"), orig_obj.get("label"));
        // Nested structure preserved
        assert!(mutated_obj.get("nested").unwrap().is_object());
    }

    #[test]
    fn test_evaluate_strategy() {
        let config = EvolutionConfig {
            evaluation_samples: 5,
            ..EvolutionConfig::default()
        };
        let engine = EvolutionEngine::new(config);
        let strategy = make_strategy("agent-1", "baseline", json!({"lr": 0.01}));

        let score = engine.evaluate_strategy(&strategy, |_| 0.8);
        assert!((score - 0.8).abs() < f64::EPSILON);
    }

    #[test]
    fn test_evolve_once_improvement() {
        let config = EvolutionConfig {
            improvement_threshold: 0.01,
            evaluation_samples: 1,
            ..EvolutionConfig::default()
        };
        let mut engine = EvolutionEngine::new(config);
        let strategy = make_strategy("agent-1", "baseline", json!({"lr": 0.01}));
        engine.register_strategy(strategy).unwrap();

        // test_fn that scores newer versions higher
        let result = engine
            .evolve_once("agent-1", MutationType::ParameterTweak, |s| {
                s.version as f64 * 0.5
            })
            .unwrap();

        assert!(result.accepted);
        assert!(result.improvement > 0.0);
        assert!(result.child_score > result.parent_score);
    }

    #[test]
    fn test_evolve_once_regression() {
        let config = EvolutionConfig {
            improvement_threshold: 0.01,
            rollback_on_regression: true,
            evaluation_samples: 1,
            ..EvolutionConfig::default()
        };
        let mut engine = EvolutionEngine::new(config);
        let strategy = make_strategy("agent-1", "baseline", json!({"lr": 0.01}));
        let original_id = strategy.id.clone();
        engine.register_strategy(strategy).unwrap();

        // test_fn that scores newer versions lower
        let result = engine
            .evolve_once("agent-1", MutationType::ParameterTweak, |s| {
                1.0 / (s.version as f64 + 1.0)
            })
            .unwrap();

        assert!(!result.accepted);
        assert!(result.improvement < 0.0);
        // Active strategy unchanged
        assert_eq!(engine.active_strategies.get("agent-1"), Some(&original_id));
    }

    #[test]
    fn test_evolve_once_records_history() {
        let config = EvolutionConfig {
            evaluation_samples: 1,
            ..EvolutionConfig::default()
        };
        let mut engine = EvolutionEngine::new(config);
        let strategy = make_strategy("agent-1", "baseline", json!({"x": 1}));
        engine.register_strategy(strategy).unwrap();

        engine
            .evolve_once("agent-1", MutationType::ParameterTweak, |_| 0.5)
            .unwrap();

        let history = engine.get_history("agent-1").unwrap();
        assert_eq!(history.total_generations, 1);
        assert_eq!(history.results.len(), 1);
    }

    #[test]
    fn test_rollback() {
        let config = EvolutionConfig {
            improvement_threshold: 0.0,
            evaluation_samples: 1,
            ..EvolutionConfig::default()
        };
        let mut engine = EvolutionEngine::new(config);
        let strategy = make_strategy("agent-1", "baseline", json!({"x": 1}));
        let original_id = strategy.id.clone();
        engine.register_strategy(strategy).unwrap();

        // Evolve once (will be accepted since threshold is 0 and version 2 scores higher)
        engine
            .evolve_once("agent-1", MutationType::ParameterTweak, |s| {
                s.version as f64
            })
            .unwrap();

        // Active should now be the child
        assert_ne!(engine.active_strategies.get("agent-1"), Some(&original_id));

        // Rollback
        let rolled_back = engine.rollback("agent-1").unwrap();
        assert_eq!(rolled_back.id, original_id);
        assert_eq!(engine.active_strategies.get("agent-1"), Some(&original_id));
    }

    #[test]
    fn test_rollback_no_parent() {
        let mut engine = EvolutionEngine::new(EvolutionConfig::default());
        let strategy = make_strategy("agent-1", "baseline", json!({"x": 1}));
        engine.register_strategy(strategy).unwrap();

        let result = engine.rollback("agent-1");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("no parent"));
    }

    #[test]
    fn test_evolve_loop() {
        let config = EvolutionConfig {
            improvement_threshold: 0.0,
            evaluation_samples: 1,
            max_generations: 100,
            ..EvolutionConfig::default()
        };
        let mut engine = EvolutionEngine::new(config);
        let strategy = make_strategy("agent-1", "baseline", json!({"x": 1}));
        engine.register_strategy(strategy).unwrap();

        let history = engine
            .evolve_loop("agent-1", Some(5), |s| s.version as f64 * 0.1)
            .unwrap();

        assert_eq!(history.total_generations, 5);
        assert_eq!(history.results.len(), 5);
    }

    #[test]
    fn test_best_strategy() {
        let config = EvolutionConfig {
            improvement_threshold: 0.0,
            evaluation_samples: 1,
            ..EvolutionConfig::default()
        };
        let mut engine = EvolutionEngine::new(config);
        let strategy = make_strategy("agent-1", "baseline", json!({"x": 1}));
        engine.register_strategy(strategy).unwrap();

        // Evolve several times — each new version scores higher
        for _ in 0..3 {
            engine
                .evolve_once("agent-1", MutationType::ParameterTweak, |s| {
                    s.version as f64 * 0.3
                })
                .unwrap();
        }

        let best = engine.best_strategy("agent-1").unwrap();
        // Best should have the highest score among all strategies for agent-1
        let all_scores: Vec<f64> = engine
            .strategies
            .values()
            .filter(|s| s.agent_id == "agent-1")
            .map(|s| s.score)
            .collect();
        assert!(
            (best.score - all_scores.iter().cloned().fold(f64::NEG_INFINITY, f64::max)).abs()
                < f64::EPSILON
        );
    }

    #[test]
    fn test_export_strategies() {
        let mut engine = EvolutionEngine::new(EvolutionConfig::default());
        engine
            .register_strategy(make_strategy("a1", "s1", json!({})))
            .unwrap();
        engine
            .register_strategy(make_strategy("a2", "s2", json!({})))
            .unwrap();
        engine
            .register_strategy(make_strategy("a3", "s3", json!({})))
            .unwrap();

        let exported = engine.export_strategies();
        assert_eq!(exported.len(), 3);
    }
}
