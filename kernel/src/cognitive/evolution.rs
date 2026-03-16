//! Self-Evolution Engine — strategy scoring, selection, prompt optimization,
//! and cross-agent learning.

use super::memory_manager::AgentMemoryManager;
use crate::errors::AgentError;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

// ── Traits ──────────────────────────────────────────────────────────────────

/// Persistence backend for strategy scores.
pub trait StrategyStore: Send + Sync {
    fn upsert_strategy_score(
        &self,
        agent_id: &str,
        strategy_hash: &str,
        goal_type: &str,
        success: bool,
        fuel: f64,
        duration: f64,
    ) -> Result<(), String>;

    fn load_top_strategies(
        &self,
        agent_id: &str,
        goal_type: &str,
        limit: usize,
    ) -> Result<Vec<StrategyScore>, String>;

    fn load_strategy_history(
        &self,
        agent_id: &str,
        limit: usize,
    ) -> Result<Vec<StrategyScore>, String>;
}

/// LLM backend for prompt optimization.
pub trait EvolutionLlm: Send + Sync {
    fn optimize_prompt(&self, prompt: &str) -> Result<String, String>;
}

// ── Data Types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyScore {
    pub agent_id: String,
    pub strategy_hash: String,
    pub goal_type: String,
    pub uses: i64,
    pub successes: i64,
    pub total_fuel: f64,
    pub total_duration_secs: f64,
    pub composite_score: f64,
}

impl StrategyScore {
    /// Compute composite score from components.
    /// composite = 0.5 * success_rate + 0.3 * fuel_efficiency + 0.2 * speed_score
    pub fn compute_composite(success_rate: f64, fuel_efficiency: f64, speed_score: f64) -> f64 {
        0.5 * success_rate.clamp(0.0, 1.0)
            + 0.3 * fuel_efficiency.clamp(0.0, 2.0)
            + 0.2 * speed_score.clamp(0.0, 2.0)
    }

    pub fn success_rate(&self) -> f64 {
        if self.uses == 0 {
            0.0
        } else {
            self.successes as f64 / self.uses as f64
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvolutionMetrics {
    pub total_tasks_completed: u64,
    pub overall_success_rate: f64,
    pub success_rate_trend: Vec<(String, f64)>,
    pub fuel_efficiency_trend: Vec<(String, f64)>,
    pub top_strategies: Vec<(String, f64)>,
    pub improvement_percentage: f64,
    pub memories_count: u64,
    pub cross_agent_learnings_received: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyInfo {
    pub strategy_hash: String,
    pub goal_type: String,
    pub uses: i64,
    pub successes: i64,
    pub composite_score: f64,
    pub success_rate: f64,
}

// ── EvolutionTracker ────────────────────────────────────────────────────────

pub struct EvolutionTracker {
    store: Box<dyn StrategyStore>,
}

impl EvolutionTracker {
    pub fn new(store: Box<dyn StrategyStore>) -> Self {
        Self { store }
    }

    /// Record a task outcome and update the strategy score.
    #[allow(clippy::too_many_arguments)]
    pub fn record_outcome(
        &self,
        agent_id: &str,
        _task_id: &str,
        strategy_hash: &str,
        goal_type: &str,
        success: bool,
        fuel_consumed: f64,
        duration_secs: f64,
        goal_fuel_budget: f64,
        estimated_duration: f64,
        memory_mgr: &AgentMemoryManager,
    ) -> Result<(), AgentError> {
        // Persist raw outcome
        self.store
            .upsert_strategy_score(
                agent_id,
                strategy_hash,
                goal_type,
                success,
                fuel_consumed,
                duration_secs,
            )
            .map_err(|e| AgentError::SupervisorError(format!("strategy store error: {e}")))?;

        // Load updated score to compute composite
        let strategies = self
            .store
            .load_top_strategies(agent_id, goal_type, 100)
            .map_err(|e| AgentError::SupervisorError(format!("strategy load error: {e}")))?;

        if let Some(s) = strategies.iter().find(|s| s.strategy_hash == strategy_hash) {
            let success_rate = s.success_rate();
            let fuel_efficiency = if s.total_fuel > 0.0 {
                (goal_fuel_budget * s.uses as f64) / s.total_fuel
            } else {
                1.0
            };
            let speed_score = if s.total_duration_secs > 0.0 {
                (estimated_duration * s.uses as f64) / s.total_duration_secs
            } else {
                1.0
            };
            let composite =
                StrategyScore::compute_composite(success_rate, fuel_efficiency, speed_score);

            // Store as procedural memory with relevance = composite
            let strategy_desc =
                format!("strategy:{strategy_hash} goal_type:{goal_type} composite:{composite:.3}");
            memory_mgr.store_procedural(agent_id, &strategy_desc, composite)?;
        }

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record_task_result(
        &self,
        agent_id: &str,
        task_id: &str,
        strategy_hash: &str,
        goal_type: &str,
        success: bool,
        fuel_consumed: f64,
        duration_secs: f64,
        goal_fuel_budget: f64,
        estimated_duration: f64,
        memory_mgr: &AgentMemoryManager,
    ) -> Result<(), AgentError> {
        self.record_outcome(
            agent_id,
            task_id,
            strategy_hash,
            goal_type,
            success,
            fuel_consumed,
            duration_secs,
            goal_fuel_budget,
            estimated_duration,
            memory_mgr,
        )
    }

    /// Select the best strategy for a goal type.
    pub fn select_best_strategy(
        &self,
        agent_id: &str,
        goal_type: &str,
    ) -> Result<Option<String>, AgentError> {
        let strategies = self
            .store
            .load_top_strategies(agent_id, goal_type, 1)
            .map_err(|e| AgentError::SupervisorError(format!("strategy load error: {e}")))?;

        Ok(strategies.into_iter().next().map(|s| s.strategy_hash))
    }

    /// Optimize the planning prompt using LLM analysis of recent outcomes.
    pub fn optimize_planning_prompt(
        &self,
        agent_id: &str,
        current_prompt: &str,
        recent_outcomes: &[(bool, f64, f64)], // (success, fuel, duration)
        llm: &dyn EvolutionLlm,
        memory_mgr: &AgentMemoryManager,
    ) -> Result<String, AgentError> {
        let outcomes_desc: Vec<String> = recent_outcomes
            .iter()
            .enumerate()
            .map(|(i, (success, fuel, dur))| {
                format!(
                    "Task {}: {} (fuel: {:.1}, duration: {:.1}s)",
                    i + 1,
                    if *success { "SUCCESS" } else { "FAILURE" },
                    fuel,
                    dur
                )
            })
            .collect();

        let optimization_prompt = format!(
            "Analyze these task outcomes. The current planning prompt is: {current_prompt}\n\
             Recent results:\n{}\n\
             Suggest specific improvements to the planning prompt to increase success rate. \
             Return ONLY the improved prompt text, no explanations.",
            outcomes_desc.join("\n")
        );

        let optimized = llm
            .optimize_prompt(&optimization_prompt)
            .map_err(|e| AgentError::SupervisorError(format!("llm optimize error: {e}")))?;

        // Store optimized prompt as semantic memory
        memory_mgr.store_semantic(agent_id, &format!("optimized_planning_prompt: {optimized}"))?;

        Ok(optimized)
    }

    /// Share a high-performing strategy from one agent to another.
    pub fn share_learning(
        &self,
        from_agent: &str,
        to_agent: &str,
        strategy: &str,
        score: f64,
        memory_mgr: &AgentMemoryManager,
    ) -> Result<(), AgentError> {
        // Discount because it's learned, not experienced firsthand
        let discounted_score = score * 0.7;
        let strategy_desc = format!(
            "cross_agent_learning from:{from_agent} strategy:{strategy} original_score:{score:.3}"
        );
        memory_mgr.store_procedural(to_agent, &strategy_desc, discounted_score)?;
        Ok(())
    }

    /// Discover strategies above a minimum composite score.
    pub fn discover_shareable_strategies(
        &self,
        agent_ids: &[&str],
        min_score: f64,
    ) -> Result<Vec<(String, String, f64)>, AgentError> {
        let mut shareable = Vec::new();
        for agent_id in agent_ids {
            let strategies = self
                .store
                .load_strategy_history(agent_id, 50)
                .map_err(|e| AgentError::SupervisorError(format!("strategy load error: {e}")))?;

            for s in strategies {
                if s.composite_score >= min_score {
                    shareable.push((
                        agent_id.to_string(),
                        s.strategy_hash.clone(),
                        s.composite_score,
                    ));
                }
            }
        }
        Ok(shareable)
    }

    /// Get evolution metrics for an agent.
    pub fn get_evolution_metrics(
        &self,
        agent_id: &str,
        memory_mgr: &AgentMemoryManager,
    ) -> Result<EvolutionMetrics, AgentError> {
        let all_strategies = self
            .store
            .load_strategy_history(agent_id, 100)
            .map_err(|e| AgentError::SupervisorError(format!("strategy load error: {e}")))?;

        let total_tasks: u64 = all_strategies.iter().map(|s| s.uses as u64).sum();
        let total_successes: u64 = all_strategies.iter().map(|s| s.successes as u64).sum();
        let overall_success_rate = if total_tasks > 0 {
            total_successes as f64 / total_tasks as f64
        } else {
            0.0
        };

        // Top 5 strategies by composite score
        let mut sorted = all_strategies.clone();
        sorted.sort_by(|a, b| {
            b.composite_score
                .partial_cmp(&a.composite_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let top_strategies: Vec<(String, f64)> = sorted
            .iter()
            .take(5)
            .map(|s| (s.strategy_hash.clone(), s.composite_score))
            .collect();

        // Improvement: compare first vs last strategies (by usage order)
        let improvement_percentage = if sorted.len() >= 2 {
            let first_score = sorted.last().map(|s| s.composite_score).unwrap_or(0.0);
            let last_score = sorted.first().map(|s| s.composite_score).unwrap_or(0.0);
            if first_score > 0.0 {
                ((last_score - first_score) / first_score) * 100.0
            } else if last_score > 0.0 {
                100.0
            } else {
                0.0
            }
        } else {
            0.0
        };

        // Count memories
        let memories = memory_mgr
            .recall_relevant(agent_id, "", 100)
            .unwrap_or_default();
        let memories_count = memories.len() as u64;

        // Count cross-agent learnings
        let cross_agent_learnings_received = memories
            .iter()
            .filter(|m| m.value_json.contains("cross_agent_learning"))
            .count() as u64;

        // Trend data: use strategy scores as data points
        let success_rate_trend: Vec<(String, f64)> = sorted
            .iter()
            .take(30)
            .map(|s| (s.strategy_hash.clone(), s.success_rate()))
            .collect();

        let fuel_efficiency_trend: Vec<(String, f64)> = sorted
            .iter()
            .take(30)
            .map(|s| {
                let eff = if s.total_fuel > 0.0 && s.uses > 0 {
                    s.uses as f64 / s.total_fuel
                } else {
                    0.0
                };
                (s.strategy_hash.clone(), eff)
            })
            .collect();

        Ok(EvolutionMetrics {
            total_tasks_completed: total_tasks,
            overall_success_rate,
            success_rate_trend,
            fuel_efficiency_trend,
            top_strategies,
            improvement_percentage,
            memories_count,
            cross_agent_learnings_received,
        })
    }

    /// Get strategy info for an agent.
    pub fn get_agent_strategies(&self, agent_id: &str) -> Result<Vec<StrategyInfo>, AgentError> {
        let strategies = self
            .store
            .load_strategy_history(agent_id, 50)
            .map_err(|e| AgentError::SupervisorError(format!("strategy load error: {e}")))?;

        Ok(strategies
            .into_iter()
            .map(|s| StrategyInfo {
                success_rate: s.success_rate(),
                strategy_hash: s.strategy_hash,
                goal_type: s.goal_type,
                uses: s.uses,
                successes: s.successes,
                composite_score: s.composite_score,
            })
            .collect())
    }
}

/// Hash a strategy string for consistent identification.
pub fn hash_strategy(s: &str) -> String {
    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cognitive::memory_manager::{MemoryEntry, MemoryStore};
    use std::sync::Mutex;

    // ── In-memory strategy store ──

    struct InMemoryStrategyStore {
        scores: Mutex<Vec<StrategyScore>>,
    }

    impl InMemoryStrategyStore {
        fn new() -> Self {
            Self {
                scores: Mutex::new(Vec::new()),
            }
        }
    }

    impl StrategyStore for InMemoryStrategyStore {
        fn upsert_strategy_score(
            &self,
            agent_id: &str,
            strategy_hash: &str,
            goal_type: &str,
            success: bool,
            fuel: f64,
            duration: f64,
        ) -> Result<(), String> {
            let mut scores = self.scores.lock().unwrap();
            if let Some(existing) = scores
                .iter_mut()
                .find(|s| s.agent_id == agent_id && s.strategy_hash == strategy_hash)
            {
                existing.uses += 1;
                if success {
                    existing.successes += 1;
                }
                existing.total_fuel += fuel;
                existing.total_duration_secs += duration;
                existing.goal_type = goal_type.to_string();
                // Recompute composite
                let sr = existing.success_rate();
                let fe = if existing.total_fuel > 0.0 {
                    existing.uses as f64 / existing.total_fuel
                } else {
                    1.0
                };
                let ss = if existing.total_duration_secs > 0.0 {
                    existing.uses as f64 / existing.total_duration_secs
                } else {
                    1.0
                };
                existing.composite_score = StrategyScore::compute_composite(sr, fe, ss);
            } else {
                let mut s = StrategyScore {
                    agent_id: agent_id.to_string(),
                    strategy_hash: strategy_hash.to_string(),
                    goal_type: goal_type.to_string(),
                    uses: 1,
                    successes: if success { 1 } else { 0 },
                    total_fuel: fuel,
                    total_duration_secs: duration,
                    composite_score: 0.0,
                };
                let sr = s.success_rate();
                let fe = if s.total_fuel > 0.0 {
                    1.0 / s.total_fuel
                } else {
                    1.0
                };
                let ss = if s.total_duration_secs > 0.0 {
                    1.0 / s.total_duration_secs
                } else {
                    1.0
                };
                s.composite_score = StrategyScore::compute_composite(sr, fe, ss);
                scores.push(s);
            }
            Ok(())
        }

        fn load_top_strategies(
            &self,
            agent_id: &str,
            goal_type: &str,
            limit: usize,
        ) -> Result<Vec<StrategyScore>, String> {
            let scores = self.scores.lock().unwrap();
            let mut filtered: Vec<StrategyScore> = scores
                .iter()
                .filter(|s| s.agent_id == agent_id && s.goal_type == goal_type)
                .cloned()
                .collect();
            filtered.sort_by(|a, b| {
                b.composite_score
                    .partial_cmp(&a.composite_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            filtered.truncate(limit);
            Ok(filtered)
        }

        fn load_strategy_history(
            &self,
            agent_id: &str,
            limit: usize,
        ) -> Result<Vec<StrategyScore>, String> {
            let scores = self.scores.lock().unwrap();
            let mut filtered: Vec<StrategyScore> = scores
                .iter()
                .filter(|s| s.agent_id == agent_id)
                .cloned()
                .collect();
            filtered.sort_by(|a, b| {
                b.composite_score
                    .partial_cmp(&a.composite_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            filtered.truncate(limit);
            Ok(filtered)
        }
    }

    // ── In-memory memory store (for AgentMemoryManager) ──

    struct InMemoryMemoryStore {
        memories: Mutex<Vec<MemoryEntry>>,
        next_id: Mutex<i64>,
    }

    impl InMemoryMemoryStore {
        fn new() -> Self {
            Self {
                memories: Mutex::new(Vec::new()),
                next_id: Mutex::new(1),
            }
        }
    }

    impl MemoryStore for InMemoryMemoryStore {
        fn save_memory(
            &self,
            agent_id: &str,
            memory_type: &str,
            key: &str,
            value_json: &str,
        ) -> Result<(), String> {
            let mut id = self.next_id.lock().unwrap();
            let entry = MemoryEntry {
                id: *id,
                agent_id: agent_id.to_string(),
                memory_type: memory_type.to_string(),
                key: key.to_string(),
                value_json: value_json.to_string(),
                relevance_score: 1.0,
                access_count: 0,
                created_at: "now".to_string(),
                last_accessed: "now".to_string(),
            };
            *id += 1;
            self.memories.lock().unwrap().push(entry);
            Ok(())
        }

        fn load_memories(
            &self,
            agent_id: &str,
            memory_type: Option<&str>,
            limit: usize,
        ) -> Result<Vec<MemoryEntry>, String> {
            let mems = self.memories.lock().unwrap();
            let filtered: Vec<MemoryEntry> = mems
                .iter()
                .filter(|m| m.agent_id == agent_id)
                .filter(|m| memory_type.is_none() || Some(m.memory_type.as_str()) == memory_type)
                .take(limit)
                .cloned()
                .collect();
            Ok(filtered)
        }

        fn touch_memory(&self, id: i64) -> Result<(), String> {
            let mut mems = self.memories.lock().unwrap();
            if let Some(m) = mems.iter_mut().find(|m| m.id == id) {
                m.access_count += 1;
            }
            Ok(())
        }

        fn decay_memories(&self, agent_id: &str, decay_factor: f64) -> Result<(), String> {
            let mut mems = self.memories.lock().unwrap();
            for m in mems.iter_mut().filter(|m| m.agent_id == agent_id) {
                m.relevance_score *= decay_factor;
            }
            Ok(())
        }
    }

    // ── Mock LLM ──

    struct MockLlm {
        response: String,
    }

    impl MockLlm {
        fn new(response: &str) -> Self {
            Self {
                response: response.to_string(),
            }
        }
    }

    impl EvolutionLlm for MockLlm {
        fn optimize_prompt(&self, _prompt: &str) -> Result<String, String> {
            Ok(self.response.clone())
        }
    }

    // ── Helper ──

    fn make_tracker_and_memory() -> (EvolutionTracker, AgentMemoryManager) {
        let store = InMemoryStrategyStore::new();
        let mem_store = InMemoryMemoryStore::new();
        (
            EvolutionTracker::new(Box::new(store)),
            AgentMemoryManager::new(Box::new(mem_store)),
        )
    }

    // ── Tests ──

    #[test]
    fn test_record_outcome_all_success() {
        let (tracker, memory) = make_tracker_and_memory();
        for _ in 0..10 {
            tracker
                .record_outcome(
                    "agent1", "task1", "strat_a", "coding", true, 10.0, 5.0, 100.0, 50.0, &memory,
                )
                .unwrap();
        }
        let strategies = tracker
            .store
            .load_top_strategies("agent1", "coding", 10)
            .unwrap();
        assert_eq!(strategies.len(), 1);
        assert!((strategies[0].success_rate() - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_record_outcome_mixed() {
        let (tracker, memory) = make_tracker_and_memory();
        for _ in 0..5 {
            tracker
                .record_outcome(
                    "agent1", "task1", "strat_b", "coding", true, 10.0, 5.0, 100.0, 50.0, &memory,
                )
                .unwrap();
        }
        for _ in 0..5 {
            tracker
                .record_outcome(
                    "agent1", "task2", "strat_b", "coding", false, 10.0, 5.0, 100.0, 50.0, &memory,
                )
                .unwrap();
        }
        let strategies = tracker
            .store
            .load_top_strategies("agent1", "coding", 10)
            .unwrap();
        assert_eq!(strategies.len(), 1);
        assert!((strategies[0].success_rate() - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_composite_score_formula() {
        // composite = 0.5 * success_rate + 0.3 * fuel_eff + 0.2 * speed
        let composite = StrategyScore::compute_composite(1.0, 1.0, 1.0);
        assert!((composite - 1.0).abs() < 0.001);

        let composite2 = StrategyScore::compute_composite(0.0, 0.0, 0.0);
        assert!((composite2).abs() < 0.001);

        let composite3 = StrategyScore::compute_composite(0.5, 0.5, 0.5);
        assert!((composite3 - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_select_best_strategy_returns_highest() {
        let (tracker, memory) = make_tracker_and_memory();
        // Strategy A: 100% success
        for _ in 0..5 {
            tracker
                .record_outcome(
                    "agent1",
                    "t",
                    "strat_best",
                    "coding",
                    true,
                    10.0,
                    5.0,
                    100.0,
                    50.0,
                    &memory,
                )
                .unwrap();
        }
        // Strategy B: 0% success
        for _ in 0..5 {
            tracker
                .record_outcome(
                    "agent1",
                    "t",
                    "strat_worst",
                    "coding",
                    false,
                    10.0,
                    5.0,
                    100.0,
                    50.0,
                    &memory,
                )
                .unwrap();
        }
        // Strategy C: 50% success
        for i in 0..4 {
            tracker
                .record_outcome(
                    "agent1",
                    "t",
                    "strat_mid",
                    "coding",
                    i % 2 == 0,
                    10.0,
                    5.0,
                    100.0,
                    50.0,
                    &memory,
                )
                .unwrap();
        }

        let best = tracker.select_best_strategy("agent1", "coding").unwrap();
        assert_eq!(best, Some("strat_best".to_string()));
    }

    #[test]
    fn test_select_best_strategy_empty_returns_none() {
        let (tracker, _memory) = make_tracker_and_memory();
        let best = tracker.select_best_strategy("agent1", "coding").unwrap();
        assert!(best.is_none());
    }

    #[test]
    fn test_optimize_planning_prompt_calls_llm() {
        let (tracker, memory) = make_tracker_and_memory();
        let llm = MockLlm::new("improved prompt: be more specific about goals");
        let outcomes = vec![(true, 10.0, 5.0), (false, 20.0, 15.0), (true, 8.0, 3.0)];

        let result = tracker
            .optimize_planning_prompt("agent1", "original prompt", &outcomes, &llm, &memory)
            .unwrap();

        assert_eq!(result, "improved prompt: be more specific about goals");
    }

    #[test]
    fn test_optimize_planning_prompt_stores_semantic_memory() {
        let (tracker, memory) = make_tracker_and_memory();
        let llm = MockLlm::new("optimized prompt v2");
        let outcomes = vec![(true, 10.0, 5.0)];

        tracker
            .optimize_planning_prompt("agent1", "old prompt", &outcomes, &llm, &memory)
            .unwrap();

        let memories = memory
            .recall_relevant("agent1", "optimized_planning_prompt", 10)
            .unwrap();
        assert!(!memories.is_empty());
        assert!(memories[0].value_json.contains("optimized prompt v2"));
    }

    #[test]
    fn test_share_learning_creates_memory_for_target() {
        let (tracker, memory) = make_tracker_and_memory();
        tracker
            .share_learning("agent_a", "agent_b", "fast_search_strategy", 0.9, &memory)
            .unwrap();

        let memories = memory
            .recall_relevant("agent_b", "cross_agent_learning", 10)
            .unwrap();
        assert_eq!(memories.len(), 1);
        assert!(memories[0].value_json.contains("cross_agent_learning"));
    }

    #[test]
    fn test_share_learning_discount_factor() {
        let (tracker, memory) = make_tracker_and_memory();
        let score = 0.9;
        tracker
            .share_learning("agent_a", "agent_b", "strat_x", score, &memory)
            .unwrap();

        // The procedural memory stores success_rate as the discounted score
        let memories = memory.recall_relevant("agent_b", "strat_x", 10).unwrap();
        assert!(!memories.is_empty());
        // Value json should contain the original score info
        assert!(memories[0].value_json.contains("0.9"));
    }

    #[test]
    fn test_share_learning_only_high_scores() {
        let (tracker, memory) = make_tracker_and_memory();
        // Record a low-score strategy
        for _ in 0..5 {
            tracker
                .record_outcome(
                    "agent_a",
                    "t",
                    "low_strat",
                    "coding",
                    false,
                    10.0,
                    5.0,
                    100.0,
                    50.0,
                    &memory,
                )
                .unwrap();
        }

        let shareable = tracker
            .discover_shareable_strategies(&["agent_a"], 0.8)
            .unwrap();
        assert!(shareable.is_empty());
    }

    #[test]
    fn test_discover_shareable_strategies_min_score_filter() {
        let (tracker, memory) = make_tracker_and_memory();
        // Record a high-score strategy
        for _ in 0..10 {
            tracker
                .record_outcome(
                    "agent_a",
                    "t",
                    "great_strat",
                    "coding",
                    true,
                    1.0,
                    1.0,
                    100.0,
                    50.0,
                    &memory,
                )
                .unwrap();
        }

        let shareable = tracker
            .discover_shareable_strategies(&["agent_a"], 0.4)
            .unwrap();
        assert!(!shareable.is_empty());
        assert_eq!(shareable[0].0, "agent_a");
        assert_eq!(shareable[0].1, "great_strat");
    }

    #[test]
    fn test_get_evolution_metrics_all_fields() {
        let (tracker, memory) = make_tracker_and_memory();
        for _ in 0..5 {
            tracker
                .record_outcome(
                    "agent1", "t", "strat1", "coding", true, 10.0, 5.0, 100.0, 50.0, &memory,
                )
                .unwrap();
        }
        for _ in 0..3 {
            tracker
                .record_outcome(
                    "agent1", "t", "strat2", "coding", false, 20.0, 10.0, 100.0, 50.0, &memory,
                )
                .unwrap();
        }

        let metrics = tracker.get_evolution_metrics("agent1", &memory).unwrap();
        assert_eq!(metrics.total_tasks_completed, 8);
        assert!((metrics.overall_success_rate - 5.0 / 8.0).abs() < 0.01);
        assert!(!metrics.top_strategies.is_empty());
        assert!(metrics.memories_count > 0);
    }

    #[test]
    fn test_get_evolution_metrics_trend_data() {
        let (tracker, memory) = make_tracker_and_memory();
        for i in 0..5 {
            tracker
                .record_outcome(
                    "agent1",
                    "t",
                    &format!("strat_{i}"),
                    "coding",
                    true,
                    10.0,
                    5.0,
                    100.0,
                    50.0,
                    &memory,
                )
                .unwrap();
        }

        let metrics = tracker.get_evolution_metrics("agent1", &memory).unwrap();
        assert_eq!(metrics.success_rate_trend.len(), 5);
        assert_eq!(metrics.fuel_efficiency_trend.len(), 5);
    }

    #[test]
    fn test_get_evolution_metrics_improvement_percentage() {
        let (tracker, memory) = make_tracker_and_memory();
        // One bad strategy
        for _ in 0..5 {
            tracker
                .record_outcome(
                    "agent1", "t", "bad", "coding", false, 100.0, 50.0, 100.0, 50.0, &memory,
                )
                .unwrap();
        }
        // One good strategy
        for _ in 0..5 {
            tracker
                .record_outcome(
                    "agent1", "t", "good", "coding", true, 1.0, 1.0, 100.0, 50.0, &memory,
                )
                .unwrap();
        }

        let metrics = tracker.get_evolution_metrics("agent1", &memory).unwrap();
        // Best score > worst score so improvement should be positive
        assert!(metrics.improvement_percentage > 0.0);
    }

    #[test]
    fn test_upsert_strategy_score_insert_new() {
        let store = InMemoryStrategyStore::new();
        store
            .upsert_strategy_score("a1", "s1", "coding", true, 10.0, 5.0)
            .unwrap();
        let strategies = store.load_top_strategies("a1", "coding", 10).unwrap();
        assert_eq!(strategies.len(), 1);
        assert_eq!(strategies[0].uses, 1);
        assert_eq!(strategies[0].successes, 1);
    }

    #[test]
    fn test_upsert_strategy_score_update_existing() {
        let store = InMemoryStrategyStore::new();
        store
            .upsert_strategy_score("a1", "s1", "coding", true, 10.0, 5.0)
            .unwrap();
        store
            .upsert_strategy_score("a1", "s1", "coding", false, 20.0, 10.0)
            .unwrap();
        let strategies = store.load_top_strategies("a1", "coding", 10).unwrap();
        assert_eq!(strategies.len(), 1);
        assert_eq!(strategies[0].uses, 2);
        assert_eq!(strategies[0].successes, 1);
        assert!((strategies[0].success_rate() - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_integration_full_loop() {
        let (tracker, memory) = make_tracker_and_memory();

        // Agent runs through 3 goals with same strategy, success accumulates
        for i in 0..3 {
            let success = i > 0; // first fails, rest succeed
            tracker
                .record_outcome(
                    "agent1",
                    &format!("goal_{i}"),
                    "iterative_approach",
                    "coding",
                    success,
                    10.0,
                    5.0,
                    100.0,
                    50.0,
                    &memory,
                )
                .unwrap();
        }

        // Best strategy should be selected
        let best = tracker.select_best_strategy("agent1", "coding").unwrap();
        assert_eq!(best, Some("iterative_approach".to_string()));

        // Metrics should show accumulated data
        let metrics = tracker.get_evolution_metrics("agent1", &memory).unwrap();
        assert_eq!(metrics.total_tasks_completed, 3);
    }

    #[test]
    fn test_cross_agent_learning_flow() {
        let (tracker, memory) = make_tracker_and_memory();

        // Agent A develops a great strategy
        for _ in 0..10 {
            tracker
                .record_outcome(
                    "agent_a",
                    "t",
                    "great_strat",
                    "coding",
                    true,
                    1.0,
                    1.0,
                    100.0,
                    50.0,
                    &memory,
                )
                .unwrap();
        }

        // Discover and share
        let shareable = tracker
            .discover_shareable_strategies(&["agent_a"], 0.4)
            .unwrap();
        assert!(!shareable.is_empty());

        for (from_agent, strat, score) in &shareable {
            tracker
                .share_learning(from_agent, "agent_b", strat, *score, &memory)
                .unwrap();
        }

        // Agent B should now have the strategy as a memory
        let b_memories = memory
            .recall_relevant("agent_b", "great_strat", 10)
            .unwrap();
        assert!(!b_memories.is_empty());
        assert!(b_memories[0].value_json.contains("cross_agent_learning"));
    }

    #[test]
    fn test_get_agent_strategies() {
        let (tracker, memory) = make_tracker_and_memory();
        for _ in 0..3 {
            tracker
                .record_outcome(
                    "agent1", "t", "s1", "coding", true, 10.0, 5.0, 100.0, 50.0, &memory,
                )
                .unwrap();
        }
        let strategies = tracker.get_agent_strategies("agent1").unwrap();
        assert_eq!(strategies.len(), 1);
        assert_eq!(strategies[0].strategy_hash, "s1");
        assert_eq!(strategies[0].uses, 3);
        assert_eq!(strategies[0].successes, 3);
        assert!((strategies[0].success_rate - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_hash_strategy_deterministic() {
        let h1 = hash_strategy("my_strategy");
        let h2 = hash_strategy("my_strategy");
        assert_eq!(h1, h2);
        let h3 = hash_strategy("different_strategy");
        assert_ne!(h1, h3);
    }
}
