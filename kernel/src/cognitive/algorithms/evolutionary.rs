//! Evolution Engine — plan optimization through mutation and selection.
//!
//! Applies evolutionary pressure to agent plans: generate mutations of step
//! sequences, score them with a fitness function, and select the best candidate.
//! Tracks stagnation to know when to stop evolving.

use crate::cognitive::types::AgentStep;
use std::cmp::Ordering;

/// A plan scored by the evolution engine.
#[derive(Debug, Clone)]
pub struct ScoredPlan {
    pub steps: Vec<AgentStep>,
    pub score: f64,
    pub generation: usize,
    pub mutations_applied: Vec<String>,
}

/// Evolves agent plans through mutation, scoring, and selection.
#[derive(Debug, Clone)]
pub struct EvolutionEngine {
    mutation_rate: f64,
    population: Vec<ScoredPlan>,
    generation: usize,
    stagnation_counter: usize,
    max_stagnation: usize,
}

impl Default for EvolutionEngine {
    fn default() -> Self {
        Self::new(0.3)
    }
}

impl EvolutionEngine {
    pub fn new(mutation_rate: f64) -> Self {
        Self {
            mutation_rate: mutation_rate.clamp(0.0, 1.0),
            population: Vec::new(),
            generation: 0,
            stagnation_counter: 0,
            max_stagnation: 5,
        }
    }

    /// Optimize a plan using a fitness function.
    /// Returns the best plan found (original or mutated).
    pub fn optimize_plan(
        &mut self,
        steps: Vec<AgentStep>,
        fitness_fn: impl Fn(&[AgentStep]) -> f64,
    ) -> Vec<AgentStep> {
        let base_score = fitness_fn(&steps);

        // Generate mutations
        let mutations = self.generate_mutations(&steps);

        // Score each mutation
        let mut candidates: Vec<ScoredPlan> = mutations
            .into_iter()
            .map(|(mutated_steps, mutation_desc)| {
                let score = fitness_fn(&mutated_steps);
                ScoredPlan {
                    steps: mutated_steps,
                    score,
                    generation: self.generation,
                    mutations_applied: vec![mutation_desc],
                }
            })
            .collect();

        // Add original plan as a candidate
        candidates.push(ScoredPlan {
            steps: steps.clone(),
            score: base_score,
            generation: self.generation,
            mutations_applied: vec![],
        });

        // Select best
        candidates.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(Ordering::Equal));
        let best = candidates.remove(0);

        // Track stagnation
        self.generation += 1;
        if let Some(prev_best) = self.population.last() {
            if best.score <= prev_best.score {
                self.stagnation_counter += 1;
            } else {
                self.stagnation_counter = 0;
            }
        }
        self.population.push(best.clone());

        best.steps
    }

    /// Backward-compatible: optimize without a fitness function (uses step-count heuristic).
    pub fn optimize_plan_simple(&mut self, steps: Vec<AgentStep>) -> Vec<AgentStep> {
        self.optimize_plan(steps, |s| {
            // Default fitness: prefer fewer steps with lower fuel cost
            let step_penalty = s.len() as f64 * 0.1;
            let fuel_penalty: f64 = s.iter().map(|st| st.fuel_cost).sum::<f64>() * 0.01;
            1.0 - step_penalty - fuel_penalty
        })
    }

    fn generate_mutations(&self, steps: &[AgentStep]) -> Vec<(Vec<AgentStep>, String)> {
        let mut mutations = Vec::new();

        // Mutation 1: Reorder — swap two adjacent steps
        if steps.len() > 2 {
            let mut reordered = steps.to_vec();
            // Swap the first two non-first steps (index 1 and 2)
            reordered.swap(1, 2);
            mutations.push((reordered, "step_reorder_1_2".to_string()));
        }

        // Mutation 2: Drop the highest-fuel-cost step (if >2 steps)
        if steps.len() > 2 {
            let mut trimmed = steps.to_vec();
            if let Some(max_idx) = trimmed
                .iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| {
                    a.fuel_cost
                        .partial_cmp(&b.fuel_cost)
                        .unwrap_or(Ordering::Equal)
                })
                .map(|(i, _)| i)
            {
                let desc = format!("drop_expensive_step_{max_idx}");
                trimmed.remove(max_idx);
                mutations.push((trimmed, desc));
            }
        }

        // Mutation 3: Reduce max_retries on all steps (less fuel)
        {
            let mut reduced = steps.to_vec();
            for step in &mut reduced {
                step.max_retries = step.max_retries.saturating_sub(1).max(1);
            }
            mutations.push((reduced, "reduce_retries".to_string()));
        }

        // Mutation 4: Boost max_retries on all steps (more resilience)
        {
            let mut boosted = steps.to_vec();
            for step in &mut boosted {
                step.max_retries = (step.max_retries + 1).min(5);
            }
            mutations.push((boosted, "boost_retries".to_string()));
        }

        // Mutation 5: Duplicate first step (redundancy)
        if !steps.is_empty() && self.mutation_rate > 0.2 {
            let mut duplicated = steps.to_vec();
            let copy = duplicated[0].clone();
            duplicated.insert(0, copy);
            duplicated[0].id = format!("{}_dup", steps[0].id);
            mutations.push((duplicated, "duplicate_first_step".to_string()));
        }

        mutations
    }

    /// Whether evolution has stagnated (no improvement for max_stagnation generations).
    pub fn is_stagnated(&self) -> bool {
        self.stagnation_counter >= self.max_stagnation
    }

    /// Score history across generations.
    pub fn improvement_history(&self) -> Vec<f64> {
        self.population.iter().map(|p| p.score).collect()
    }

    /// The best plan ever found.
    pub fn best_plan(&self) -> Option<&ScoredPlan> {
        self.population
            .iter()
            .max_by(|a, b| a.score.partial_cmp(&b.score).unwrap_or(Ordering::Equal))
    }

    /// Current generation number.
    pub fn generation(&self) -> usize {
        self.generation
    }

    /// Number of plans in history.
    pub fn population_size(&self) -> usize {
        self.population.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cognitive::types::PlannedAction;

    fn make_step(goal_id: &str, fuel: f64) -> AgentStep {
        let mut step = AgentStep::new(
            goal_id.to_string(),
            PlannedAction::LlmQuery {
                prompt: "test".to_string(),
                context: vec![],
            },
        );
        step.fuel_cost = fuel;
        step
    }

    #[test]
    fn optimization_improves_score() {
        let mut engine = EvolutionEngine::new(0.3);
        let steps = vec![
            make_step("g1", 10.0),
            make_step("g1", 20.0),
            make_step("g1", 5.0),
        ];

        // Fitness function: penalize total fuel cost
        let optimized = engine.optimize_plan(steps.clone(), |s| {
            let fuel: f64 = s.iter().map(|st| st.fuel_cost).sum();
            100.0 - fuel
        });

        let original_fuel: f64 = steps.iter().map(|s| s.fuel_cost).sum();
        let optimized_fuel: f64 = optimized.iter().map(|s| s.fuel_cost).sum();
        // Either fewer steps or same/better fuel
        assert!(optimized_fuel <= original_fuel || optimized.len() < steps.len());
    }

    #[test]
    fn mutation_generation() {
        let engine = EvolutionEngine::new(0.3);
        let steps = vec![
            make_step("g1", 1.0),
            make_step("g1", 2.0),
            make_step("g1", 3.0),
        ];
        let mutations = engine.generate_mutations(&steps);
        assert!(mutations.len() >= 4, "should generate at least 4 mutations");
        // Each mutation should have a description
        for (_, desc) in &mutations {
            assert!(!desc.is_empty());
        }
    }

    #[test]
    fn stagnation_detection() {
        let mut engine = EvolutionEngine::new(0.3);
        engine.max_stagnation = 3;

        let steps = vec![make_step("g1", 1.0)];
        // Run optimization with a constant fitness (always stagnates)
        for _ in 0..5 {
            engine.optimize_plan(steps.clone(), |_| 0.5);
        }
        assert!(engine.is_stagnated(), "should detect stagnation");
    }

    #[test]
    fn history_tracking() {
        let mut engine = EvolutionEngine::new(0.3);
        assert_eq!(engine.population_size(), 0);

        let steps = vec![make_step("g1", 1.0)];
        engine.optimize_plan(steps.clone(), |_| 0.5);
        assert_eq!(engine.population_size(), 1);
        assert_eq!(engine.generation(), 1);

        engine.optimize_plan(steps, |_| 0.7);
        assert_eq!(engine.population_size(), 2);
        assert_eq!(engine.improvement_history().len(), 2);
    }

    #[test]
    fn best_plan_selection() {
        let mut engine = EvolutionEngine::new(0.3);
        let steps = vec![make_step("g1", 1.0)];

        engine.optimize_plan(steps.clone(), |_| 0.3);
        engine.optimize_plan(steps.clone(), |_| 0.9);
        engine.optimize_plan(steps, |_| 0.6);

        let best = engine.best_plan().unwrap();
        assert!(best.score >= 0.9);
    }

    #[test]
    fn simple_optimize_backward_compat() {
        let mut engine = EvolutionEngine::new(0.3);
        let steps = vec![make_step("g1", 1.0), make_step("g1", 2.0)];
        let result = engine.optimize_plan_simple(steps);
        assert!(!result.is_empty());
    }

    #[test]
    fn empty_plan_handled() {
        let mut engine = EvolutionEngine::new(0.3);
        let result = engine.optimize_plan(vec![], |_| 1.0);
        assert!(result.is_empty());
    }
}
