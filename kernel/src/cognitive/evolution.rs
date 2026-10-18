//! Self-Evolution Engine — strategy scoring, selection, prompt optimization,
//! and cross-agent learning.

use super::memory_manager::AgentMemoryManager;
use crate::errors::AgentError;
use crate::hm::{
    ControlRecord, DerivedItem, DispatchContext, DispatchIssuer, NewMemory, Parent, Principal,
    PromptBuilder, PromptSource, RecallSpec, RecordTable, SourceKind, StrategyWrite,
};
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

/// LLM backend for prompt optimization (P3-HM-CONTRACT-02 §11.4): the
/// call runs under the goal's post-goal context; the reply is derived.
pub trait EvolutionLlm: Send + Sync {
    fn optimize_prompt(
        &self,
        ctx: &mut DispatchContext,
        prompt: &mut PromptSource<'_>,
    ) -> Result<DerivedItem, String>;
}

/// The agent of an agent context.
fn agent_of(ctx: &DispatchContext) -> Result<String, AgentError> {
    match ctx.principal() {
        Principal::Agent(id) => Ok(id.to_string()),
        _ => Err(AgentError::SupervisorError(
            "evolution: refused: not an agent's unit".to_string(),
        )),
    }
}

fn hm_error(e: impl std::fmt::Display) -> AgentError {
    AgentError::SupervisorError(format!("historical memory: {e}"))
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

    /// Record a task outcome and update the strategy score (§6.5
    /// `strategy_score`, `evolution_procedural`): one labelled contribution
    /// to the aggregate, then a labelled procedural row derived from it,
    /// both in the unit `ctx` (the goal's post-goal context).
    #[allow(clippy::too_many_arguments)]
    pub fn record_outcome(
        &self,
        issuer: &DispatchIssuer,
        ctx: &DispatchContext,
        _task_id: &str,
        strategy_hash: &str,
        goal_type: &str,
        success: bool,
        fuel_consumed: f64,
        duration_secs: f64,
        goal_fuel_budget: f64,
        estimated_duration: f64,
    ) -> Result<(), AgentError> {
        let agent_id = agent_of(ctx)?;
        // Persist raw outcome
        let receipt = issuer
            .upsert_strategy(
                ctx,
                &StrategyWrite {
                    strategy_hash,
                    goal_type,
                    success,
                    fuel: fuel_consumed,
                    duration_secs,
                },
                &[],
            )
            .map_err(|e| AgentError::SupervisorError(format!("strategy store error: {e}")))?;

        // Load updated score to compute composite
        let strategies = self
            .store
            .load_top_strategies(&agent_id, goal_type, 100)
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

            // Store as procedural memory with relevance = composite; it is
            // derived from the aggregate.
            let strategy_desc =
                format!("strategy:{strategy_hash} goal_type:{goal_type} composite:{composite:.3}");
            AgentMemoryManager::record_procedural(
                issuer,
                ctx,
                SourceKind::EvolutionProcedural,
                &strategy_desc,
                composite,
                &[Parent::Record(receipt.record_ref())],
            )?;
        }

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record_task_result(
        &self,
        issuer: &DispatchIssuer,
        ctx: &DispatchContext,
        task_id: &str,
        strategy_hash: &str,
        goal_type: &str,
        success: bool,
        fuel_consumed: f64,
        duration_secs: f64,
        goal_fuel_budget: f64,
        estimated_duration: f64,
    ) -> Result<(), AgentError> {
        self.record_outcome(
            issuer,
            ctx,
            task_id,
            strategy_hash,
            goal_type,
            success,
            fuel_consumed,
            duration_secs,
            goal_fuel_budget,
            estimated_duration,
        )
    }

    /// Select the best strategy for a goal type: the top eligible aggregate
    /// of the unit's agent, read route-first (§12.3). Its hash is a control
    /// value; a write built from it names the record as its parent.
    pub fn select_best_strategy(
        &self,
        issuer: &DispatchIssuer,
        ctx: &DispatchContext,
        goal_type: &str,
    ) -> Result<Option<ControlRecord>, AgentError> {
        let spec = RecallSpec {
            column_eq: Some(("goal_type", goal_type)),
            ..RecallSpec::table(RecordTable::StrategyScores, 1)
        };
        Ok(issuer
            .control_records(ctx, &spec)
            .map_err(|e| AgentError::SupervisorError(format!("strategy load error: {e}")))?
            .into_iter()
            .next())
    }

    /// Optimize the planning prompt using LLM analysis of recent outcomes,
    /// under the goal's context `ctx` (§11.3: never a freshly resolved
    /// policy). The optimized prompt is stored as an `optimized_prompt`
    /// semantic row derived from the reply.
    pub fn optimize_planning_prompt(
        &self,
        issuer: &DispatchIssuer,
        ctx: &mut DispatchContext,
        current_prompt: &str,
        recent_outcomes: &[(bool, f64, f64)], // (success, fuel, duration)
        llm: &dyn EvolutionLlm,
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
            .optimize_prompt(ctx, &mut |c: &mut DispatchContext| {
                let mut b = PromptBuilder::new(c);
                b.text(&optimization_prompt);
                Some(b.build())
            })
            .map_err(|e| AgentError::SupervisorError(format!("llm optimize error: {e}")))?;

        // Store optimized prompt as semantic memory
        AgentMemoryManager::record_semantic(
            issuer,
            ctx,
            SourceKind::OptimizedPrompt,
            &format!("optimized_planning_prompt: {}", optimized.text()),
            &[Parent::Derived(&optimized)],
        )?;

        Ok(optimized.text().to_string())
    }

    /// Share a high-performing strategy from one agent to another (§14
    /// rule 3): a new procedural row of the target, in the target's unit,
    /// labelled with the source aggregate's label ⊔ the target unit's, with
    /// a derivation edge to the source. Refused when that label is not
    /// `UNRESTRICTED_ORIGIN` and the target agent is not local-only.
    pub fn share_learning(
        &self,
        issuer: &DispatchIssuer,
        target: &DispatchContext,
        from_agent: &str,
        strategy: &str,
        score: f64,
    ) -> Result<(), AgentError> {
        let source_agent = uuid::Uuid::parse_str(from_agent).map_err(hm_error)?;
        // Discount because it's learned, not experienced firsthand
        let discounted_score = score * 0.7;
        let strategy_desc = format!(
            "cross_agent_learning from:{from_agent} strategy:{strategy} original_score:{score:.3}"
        );
        let value = serde_json::json!({
            "strategy": strategy_desc,
            "success_rate": discounted_score,
        });
        let key = format!("strategy:{}", hash_strategy(&strategy_desc));
        issuer
            .share_strategy_to_agent(
                target,
                source_agent,
                strategy,
                &NewMemory {
                    source_kind: SourceKind::CrossAgentLearning,
                    memory_type: "procedural",
                    key: &key,
                    value_json: &value.to_string(),
                    relevance_score: 1.0,
                },
            )
            .map(|_| ())
            .map_err(hm_error)
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
        records: &nexus_persistence::NexusDatabase,
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

        // Count cross-agent learnings, by their role (§6.6: `source_kind`,
        // never a substring of the content).
        let cross_agent_learnings_received = crate::hm::count_records_of_kind(
            records,
            RecordTable::AgentMemory,
            agent_id,
            SourceKind::CrossAgentLearning,
        )
        .unwrap_or(0);

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
    use crate::hm::testing::{label_source, rows, DbMemoryStore, ScriptedLlm, TestHm};
    use crate::hm::{DispatchContext, RouteClass};
    use nexus_persistence::{NexusDatabase, StateStore};
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    // ── In-memory strategy store (the store's own unit tests) ──

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

    /// The composite of an aggregate's counters, as a live scorer would give
    /// it (production stores 0.0: P3-HM-CONTRACT-02 §14 rule 4).
    fn composite_of(s: &StrategyScore) -> f64 {
        let sr = s.success_rate();
        let fe = if s.total_fuel > 0.0 {
            s.uses as f64 / s.total_fuel
        } else {
            1.0
        };
        let ss = if s.total_duration_secs > 0.0 {
            s.uses as f64 / s.total_duration_secs
        } else {
            1.0
        };
        StrategyScore::compute_composite(sr, fe, ss)
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
                existing.composite_score = composite_of(existing);
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
                s.composite_score = composite_of(&s);
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

    // ── The strategy store over the synthetic database the issuer writes ──

    /// Reads the aggregates the labelled writes made; composites are
    /// computed from their counters ([`composite_of`]).
    struct DbStrategyStore {
        db: Arc<NexusDatabase>,
    }

    fn scored(rows: Vec<nexus_persistence::StrategyScoreRow>, limit: usize) -> Vec<StrategyScore> {
        let mut out: Vec<StrategyScore> = rows
            .into_iter()
            .map(|r| {
                let mut s = StrategyScore {
                    agent_id: r.agent_id,
                    strategy_hash: r.strategy_hash,
                    goal_type: r.goal_type,
                    uses: r.uses,
                    successes: r.successes,
                    total_fuel: r.total_fuel,
                    total_duration_secs: r.total_duration_secs,
                    composite_score: 0.0,
                };
                s.composite_score = composite_of(&s);
                s
            })
            .collect();
        out.sort_by(|a, b| {
            b.composite_score
                .partial_cmp(&a.composite_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        out.truncate(limit);
        out
    }

    impl StrategyStore for DbStrategyStore {
        fn upsert_strategy_score(
            &self,
            _agent_id: &str,
            _strategy_hash: &str,
            _goal_type: &str,
            _success: bool,
            _fuel: f64,
            _duration: f64,
        ) -> Result<(), String> {
            Err("contributions are labelled writes of the issuer".to_string())
        }

        fn load_top_strategies(
            &self,
            agent_id: &str,
            goal_type: &str,
            limit: usize,
        ) -> Result<Vec<StrategyScore>, String> {
            let rows = self
                .db
                .load_top_strategies(agent_id, goal_type, 10_000)
                .map_err(|e| e.to_string())?;
            Ok(scored(rows, limit))
        }

        fn load_strategy_history(
            &self,
            agent_id: &str,
            limit: usize,
        ) -> Result<Vec<StrategyScore>, String> {
            let rows = self
                .db
                .load_strategy_history(agent_id, 10_000)
                .map_err(|e| e.to_string())?;
            Ok(scored(rows, limit))
        }
    }

    // ── Fixture: named agents (registered UUIDs, local-only by default) ──

    struct Fixture {
        hm: TestHm,
        tracker: EvolutionTracker,
        memory: AgentMemoryManager,
        agents: Mutex<HashMap<String, uuid::Uuid>>,
    }

    impl Fixture {
        fn agent(&self, name: &str) -> String {
            self.agents
                .lock()
                .unwrap()
                .entry(name.to_string())
                .or_insert_with(|| self.hm.agent(name))
                .to_string()
        }

        fn ctx(&self, name: &str) -> DispatchContext {
            let id = uuid::Uuid::parse_str(&self.agent(name)).unwrap();
            self.hm.goal(id)
        }

        #[allow(clippy::too_many_arguments)]
        fn record(
            &self,
            name: &str,
            task: &str,
            strategy: &str,
            goal_type: &str,
            success: bool,
            fuel: f64,
            duration: f64,
            budget: f64,
            estimated: f64,
        ) -> Result<(), AgentError> {
            self.tracker.record_outcome(
                &self.hm.issuer,
                &self.ctx(name),
                task,
                strategy,
                goal_type,
                success,
                fuel,
                duration,
                budget,
                estimated,
            )
        }

        /// TEST FIXTURE: writes each aggregate's composite (not a §5.2
        /// digest column, so no label changes) as a live scorer would.
        fn sync_composites(&self) {
            for agent in self.agents.lock().unwrap().values() {
                for s in self
                    .tracker
                    .store
                    .load_strategy_history(&agent.to_string(), 10_000)
                    .unwrap()
                {
                    crate::hm::testing::exec(
                        &self.hm.db,
                        &format!(
                            "UPDATE strategy_scores SET composite_score = {} \
                             WHERE agent_id = '{}' AND strategy_hash = '{}'",
                            s.composite_score, s.agent_id, s.strategy_hash
                        ),
                    );
                }
            }
        }

        /// `(key, value_json, source_kind)` of every memory of `name`.
        fn memories(&self, name: &str) -> Vec<(String, String, String)> {
            rows(
                &self.hm.db,
                &format!(
                    "SELECT CAST(m.id AS TEXT), m.key, m.value_json FROM agent_memory m \
                     WHERE m.agent_id = '{}' ORDER BY m.id",
                    self.agent(name)
                ),
            )
            .into_iter()
            .map(|r| {
                let kind = label_source(&self.hm.db, "agent_memory", &r[0]);
                (r[1].clone(), r[2].clone(), kind)
            })
            .collect()
        }
    }

    fn make_tracker_and_memory() -> Fixture {
        let hm = TestHm::new();
        let tracker = EvolutionTracker::new(Box::new(DbStrategyStore { db: hm.db.clone() }));
        let memory = AgentMemoryManager::new(Box::new(DbMemoryStore { db: hm.db.clone() }));
        Fixture {
            hm,
            tracker,
            memory,
            agents: Mutex::new(HashMap::new()),
        }
    }

    // ── Tests ──

    #[test]
    fn test_record_outcome_all_success() {
        let f = make_tracker_and_memory();
        for _ in 0..10 {
            f.record(
                "agent1", "task1", "strat_a", "coding", true, 10.0, 5.0, 100.0, 50.0,
            )
            .unwrap();
        }
        let strategies = f
            .tracker
            .store
            .load_top_strategies(&f.agent("agent1"), "coding", 10)
            .unwrap();
        assert_eq!(strategies.len(), 1);
        assert!((strategies[0].success_rate() - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_record_outcome_mixed() {
        let f = make_tracker_and_memory();
        for _ in 0..5 {
            f.record(
                "agent1", "task1", "strat_b", "coding", true, 10.0, 5.0, 100.0, 50.0,
            )
            .unwrap();
        }
        for _ in 0..5 {
            f.record(
                "agent1", "task2", "strat_b", "coding", false, 10.0, 5.0, 100.0, 50.0,
            )
            .unwrap();
        }
        let strategies = f
            .tracker
            .store
            .load_top_strategies(&f.agent("agent1"), "coding", 10)
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
        let f = make_tracker_and_memory();
        // Strategy A: 100% success
        for _ in 0..5 {
            f.record(
                "agent1",
                "t",
                "strat_best",
                "coding",
                true,
                10.0,
                5.0,
                100.0,
                50.0,
            )
            .unwrap();
        }
        // Strategy B: 0% success
        for _ in 0..5 {
            f.record(
                "agent1",
                "t",
                "strat_worst",
                "coding",
                false,
                10.0,
                5.0,
                100.0,
                50.0,
            )
            .unwrap();
        }
        // Strategy C: 50% success
        for i in 0..4 {
            f.record(
                "agent1",
                "t",
                "strat_mid",
                "coding",
                i % 2 == 0,
                10.0,
                5.0,
                100.0,
                50.0,
            )
            .unwrap();
        }
        f.sync_composites();

        let best = f
            .tracker
            .select_best_strategy(&f.hm.issuer, &f.ctx("agent1"), "coding")
            .unwrap();
        assert_eq!(
            best.as_ref().and_then(|b| b.field("strategy_hash")),
            Some("strat_best")
        );
    }

    #[test]
    fn test_select_best_strategy_empty_returns_none() {
        let f = make_tracker_and_memory();
        let best = f
            .tracker
            .select_best_strategy(&f.hm.issuer, &f.ctx("agent1"), "coding")
            .unwrap();
        assert!(best.is_none());
    }

    #[test]
    fn test_optimize_planning_prompt_calls_llm() {
        let f = make_tracker_and_memory();
        let llm = ScriptedLlm::replying(
            &f.hm.issuer,
            RouteClass::Local,
            "improved prompt: be more specific about goals",
        );
        let outcomes = vec![(true, 10.0, 5.0), (false, 20.0, 15.0), (true, 8.0, 3.0)];

        let result = f
            .tracker
            .optimize_planning_prompt(
                &f.hm.issuer,
                &mut f.ctx("agent1"),
                "original prompt",
                &outcomes,
                &llm,
            )
            .unwrap();

        assert_eq!(result, "improved prompt: be more specific about goals");
        // The model saw the prompt and the outcomes.
        let sent = llm.sent();
        assert_eq!(sent.len(), 1);
        assert!(sent[0].contains("original prompt") && sent[0].contains("Task 2: FAILURE"));
    }

    #[test]
    fn test_optimize_planning_prompt_stores_semantic_memory() {
        let f = make_tracker_and_memory();
        let llm = ScriptedLlm::replying(&f.hm.issuer, RouteClass::Local, "optimized prompt v2");
        let outcomes = vec![(true, 10.0, 5.0)];

        f.tracker
            .optimize_planning_prompt(
                &f.hm.issuer,
                &mut f.ctx("agent1"),
                "old prompt",
                &outcomes,
                &llm,
            )
            .unwrap();

        let memories = f.memories("agent1");
        assert!(!memories.is_empty());
        let (_, value, kind) = memories
            .iter()
            .find(|(_, v, _)| v.contains("optimized_planning_prompt"))
            .unwrap();
        assert!(value.contains("optimized prompt v2"));
        // Its role is its label's source kind (§6.6).
        assert_eq!(kind, "optimized_prompt");
    }

    #[test]
    fn test_share_learning_creates_memory_for_target() {
        let f = make_tracker_and_memory();
        // The source's strategy exists as an aggregate (§14 rule 3 copies
        // from a record).
        f.record(
            "agent_a",
            "t",
            "fast_search_strategy",
            "coding",
            true,
            1.0,
            1.0,
            100.0,
            50.0,
        )
        .unwrap();
        f.tracker
            .share_learning(
                &f.hm.issuer,
                &f.ctx("agent_b"),
                &f.agent("agent_a"),
                "fast_search_strategy",
                0.9,
            )
            .unwrap();

        let memories: Vec<_> = f
            .memories("agent_b")
            .into_iter()
            .filter(|(_, v, _)| v.contains("cross_agent_learning"))
            .collect();
        assert_eq!(memories.len(), 1);
        assert_eq!(memories[0].2, "cross_agent_learning");
    }

    #[test]
    fn test_share_learning_discount_factor() {
        let f = make_tracker_and_memory();
        let score = 0.9;
        f.record(
            "agent_a", "t", "strat_x", "coding", true, 1.0, 1.0, 100.0, 50.0,
        )
        .unwrap();
        f.tracker
            .share_learning(
                &f.hm.issuer,
                &f.ctx("agent_b"),
                &f.agent("agent_a"),
                "strat_x",
                score,
            )
            .unwrap();

        // The procedural memory stores success_rate as the discounted score
        let memories: Vec<_> = f
            .memories("agent_b")
            .into_iter()
            .filter(|(_, v, _)| v.contains("strat_x"))
            .collect();
        assert!(!memories.is_empty());
        // Value json should contain the original score info
        assert!(memories[0].1.contains("0.9"));
    }

    #[test]
    fn test_share_learning_only_high_scores() {
        let f = make_tracker_and_memory();
        // Record a low-score strategy
        for _ in 0..5 {
            f.record(
                "agent_a",
                "t",
                "low_strat",
                "coding",
                false,
                10.0,
                5.0,
                100.0,
                50.0,
            )
            .unwrap();
        }

        let shareable = f
            .tracker
            .discover_shareable_strategies(&[f.agent("agent_a").as_str()], 0.8)
            .unwrap();
        assert!(shareable.is_empty());
    }

    #[test]
    fn test_discover_shareable_strategies_min_score_filter() {
        let f = make_tracker_and_memory();
        // Record a high-score strategy
        for _ in 0..10 {
            f.record(
                "agent_a",
                "t",
                "great_strat",
                "coding",
                true,
                1.0,
                1.0,
                100.0,
                50.0,
            )
            .unwrap();
        }

        let shareable = f
            .tracker
            .discover_shareable_strategies(&[f.agent("agent_a").as_str()], 0.4)
            .unwrap();
        assert!(!shareable.is_empty());
        assert_eq!(shareable[0].0, f.agent("agent_a"));
        assert_eq!(shareable[0].1, "great_strat");
    }

    #[test]
    fn test_get_evolution_metrics_all_fields() {
        let f = make_tracker_and_memory();
        for _ in 0..5 {
            f.record(
                "agent1", "t", "strat1", "coding", true, 10.0, 5.0, 100.0, 50.0,
            )
            .unwrap();
        }
        for _ in 0..3 {
            f.record(
                "agent1", "t", "strat2", "coding", false, 20.0, 10.0, 100.0, 50.0,
            )
            .unwrap();
        }

        let metrics = f
            .tracker
            .get_evolution_metrics(&f.agent("agent1"), &f.memory, &f.hm.db)
            .unwrap();
        assert_eq!(metrics.total_tasks_completed, 8);
        assert!((metrics.overall_success_rate - 5.0 / 8.0).abs() < 0.01);
        assert!(!metrics.top_strategies.is_empty());
        assert!(metrics.memories_count > 0);
    }

    #[test]
    fn test_get_evolution_metrics_trend_data() {
        let f = make_tracker_and_memory();
        for i in 0..5 {
            f.record(
                "agent1",
                "t",
                &format!("strat_{i}"),
                "coding",
                true,
                10.0,
                5.0,
                100.0,
                50.0,
            )
            .unwrap();
        }

        let metrics = f
            .tracker
            .get_evolution_metrics(&f.agent("agent1"), &f.memory, &f.hm.db)
            .unwrap();
        assert_eq!(metrics.success_rate_trend.len(), 5);
        assert_eq!(metrics.fuel_efficiency_trend.len(), 5);
    }

    #[test]
    fn test_get_evolution_metrics_improvement_percentage() {
        let f = make_tracker_and_memory();
        // One bad strategy
        for _ in 0..5 {
            f.record(
                "agent1", "t", "bad", "coding", false, 100.0, 50.0, 100.0, 50.0,
            )
            .unwrap();
        }
        // One good strategy
        for _ in 0..5 {
            f.record("agent1", "t", "good", "coding", true, 1.0, 1.0, 100.0, 50.0)
                .unwrap();
        }

        let metrics = f
            .tracker
            .get_evolution_metrics(&f.agent("agent1"), &f.memory, &f.hm.db)
            .unwrap();
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
        let f = make_tracker_and_memory();

        // Agent runs through 3 goals with same strategy, success accumulates
        for i in 0..3 {
            let success = i > 0; // first fails, rest succeed
            f.record(
                "agent1",
                &format!("goal_{i}"),
                "iterative_approach",
                "coding",
                success,
                10.0,
                5.0,
                100.0,
                50.0,
            )
            .unwrap();
        }
        f.sync_composites();

        // Best strategy should be selected
        let best = f
            .tracker
            .select_best_strategy(&f.hm.issuer, &f.ctx("agent1"), "coding")
            .unwrap();
        assert_eq!(
            best.as_ref().and_then(|b| b.field("strategy_hash")),
            Some("iterative_approach")
        );

        // Metrics should show accumulated data
        let metrics = f
            .tracker
            .get_evolution_metrics(&f.agent("agent1"), &f.memory, &f.hm.db)
            .unwrap();
        assert_eq!(metrics.total_tasks_completed, 3);
    }

    #[test]
    fn test_cross_agent_learning_flow() {
        let f = make_tracker_and_memory();

        // Agent A develops a great strategy
        for _ in 0..10 {
            f.record(
                "agent_a",
                "t",
                "great_strat",
                "coding",
                true,
                1.0,
                1.0,
                100.0,
                50.0,
            )
            .unwrap();
        }

        // Discover and share
        let shareable = f
            .tracker
            .discover_shareable_strategies(&[f.agent("agent_a").as_str()], 0.4)
            .unwrap();
        assert!(!shareable.is_empty());

        for (from_agent, strat, score) in &shareable {
            f.tracker
                .share_learning(&f.hm.issuer, &f.ctx("agent_b"), from_agent, strat, *score)
                .unwrap();
        }

        // Agent B should now have the strategy as a memory
        let b_memories: Vec<_> = f
            .memories("agent_b")
            .into_iter()
            .filter(|(_, v, _)| v.contains("great_strat"))
            .collect();
        assert!(!b_memories.is_empty());
        assert!(b_memories[0].1.contains("cross_agent_learning"));
    }

    /// §6.6: the evolution metrics count cross-agent learnings by their
    /// role (`source_kind`), never by a substring of their content.
    #[test]
    fn hm_m3_n5_cross_agent_learnings_are_counted_by_source_kind() {
        let f = make_tracker_and_memory();
        for _ in 0..10 {
            f.record(
                "agent_a",
                "t",
                "great_strat",
                "coding",
                true,
                1.0,
                1.0,
                100.0,
                50.0,
            )
            .unwrap();
        }
        // A memory of agent B whose content names the role, written as an
        // ordinary loop memory: not a learning.
        AgentMemoryManager::record_semantic(
            &f.hm.issuer,
            &f.ctx("agent_b"),
            SourceKind::LoopEpisodic,
            "cross_agent_learning lookalike",
            &[],
        )
        .unwrap();
        let received = || {
            f.tracker
                .get_evolution_metrics(&f.agent("agent_b"), &f.memory, &f.hm.db)
                .unwrap()
                .cross_agent_learnings_received
        };
        assert_eq!(received(), 0);
        let shareable = f
            .tracker
            .discover_shareable_strategies(&[f.agent("agent_a").as_str()], 0.4)
            .unwrap();
        assert!(!shareable.is_empty());
        for (from_agent, strat, score) in &shareable {
            f.tracker
                .share_learning(&f.hm.issuer, &f.ctx("agent_b"), from_agent, strat, *score)
                .unwrap();
        }
        assert_eq!(received(), shareable.len() as u64);
    }

    #[test]
    fn test_get_agent_strategies() {
        let f = make_tracker_and_memory();
        for _ in 0..3 {
            f.record("agent1", "t", "s1", "coding", true, 10.0, 5.0, 100.0, 50.0)
                .unwrap();
        }
        let strategies = f.tracker.get_agent_strategies(&f.agent("agent1")).unwrap();
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
