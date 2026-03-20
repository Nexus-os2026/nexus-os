//! Swarm Coordinator — parallel variant evaluation for agent steps.
//!
//! Creates multiple parameter variants of an agent step, evaluates them,
//! and selects the best-performing variant using swarm intelligence.

use crate::cognitive::types::AgentStep;
use std::cmp::Ordering;

/// A solution scored by the swarm evaluator.
#[derive(Debug, Clone)]
pub struct RankedSolution {
    pub steps: Vec<AgentStep>,
    pub fitness: f64,
    pub generation: usize,
    pub agent_id: String,
}

/// Manages parallel agent step evaluation with swarm intelligence.
#[derive(Debug, Clone)]
pub struct SwarmCoordinator {
    population_size: usize,
    best_solutions: Vec<RankedSolution>,
    convergence_threshold: f64,
    max_generations: usize,
    generation: usize,
}

impl Default for SwarmCoordinator {
    fn default() -> Self {
        Self::new(4)
    }
}

impl SwarmCoordinator {
    pub fn new(population_size: usize) -> Self {
        Self {
            population_size: population_size.max(2),
            best_solutions: Vec::new(),
            convergence_threshold: 0.01,
            max_generations: 20,
            generation: 0,
        }
    }

    /// Backward-compatible: bump max_retries for swarm resilience.
    pub fn prepare_parallel_step(&self, step: &mut AgentStep) {
        if step.max_retries < 3 {
            step.max_retries = 3;
        }
    }

    /// Create N variants of a step with diverse parameters for parallel evaluation.
    /// If `count` is 0, uses the configured population size.
    pub fn prepare_parallel_variants(&self, step: &AgentStep, count: usize) -> Vec<AgentStep> {
        let count = if count == 0 {
            self.population_size
        } else {
            count
        }
        .max(1);
        let mut variants = Vec::with_capacity(count);

        for i in 0..count {
            let mut variant = step.clone();
            // Vary max_retries to explore different resilience levels
            variant.max_retries = match i % 4 {
                0 => 1,
                1 => 2,
                2 => 3,
                _ => 4,
            };
            // Tag variant ID for tracking
            variant.id = format!("{}_swarm_v{}", step.id, i);
            variants.push(variant);
        }

        variants
    }

    /// Evaluate results from parallel execution and select the best.
    /// Takes (step, result_text, score) tuples.
    pub fn evaluate_swarm_results(
        &mut self,
        results: Vec<(AgentStep, String, f64)>,
    ) -> Option<AgentStep> {
        if results.is_empty() {
            return None;
        }

        let mut scored: Vec<RankedSolution> = results
            .into_iter()
            .map(|(step, _result, fitness)| RankedSolution {
                steps: vec![step],
                fitness,
                generation: self.generation,
                agent_id: String::new(),
            })
            .collect();

        scored.sort_by(|a, b| b.fitness.partial_cmp(&a.fitness).unwrap_or(Ordering::Equal));

        let best = scored.remove(0);
        self.best_solutions.push(best.clone());
        self.generation += 1;

        best.steps.into_iter().next()
    }

    /// Share top-performing parameters across steps by applying the best
    /// max_retries configuration from historical winners.
    pub fn propagate_best_params(&self, steps: &mut [AgentStep]) {
        if let Some(best) = self.best_solution() {
            if let Some(best_step) = best.steps.first() {
                for step in steps.iter_mut() {
                    step.max_retries = step.max_retries.max(best_step.max_retries);
                }
            }
        }
    }

    /// Check if the swarm has converged (recent solutions have similar fitness).
    pub fn has_converged(&self) -> bool {
        if self.best_solutions.len() < 3 {
            return false;
        }
        let recent: Vec<f64> = self
            .best_solutions
            .iter()
            .rev()
            .take(3)
            .map(|s| s.fitness)
            .collect();
        let variance = statistical_variance(&recent);
        variance < self.convergence_threshold
    }

    /// Get the best solution found so far.
    pub fn best_solution(&self) -> Option<&RankedSolution> {
        self.best_solutions
            .iter()
            .max_by(|a, b| a.fitness.partial_cmp(&b.fitness).unwrap_or(Ordering::Equal))
    }

    /// Current generation count.
    pub fn generation(&self) -> usize {
        self.generation
    }

    /// Number of solutions evaluated.
    pub fn solutions_evaluated(&self) -> usize {
        self.best_solutions.len()
    }

    /// Maximum generations before stopping.
    pub fn max_generations(&self) -> usize {
        self.max_generations
    }
}

/// Compute variance of a slice of f64 values.
fn statistical_variance(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cognitive::types::PlannedAction;

    fn make_step(goal_id: &str) -> AgentStep {
        AgentStep::new(
            goal_id.to_string(),
            PlannedAction::LlmQuery {
                prompt: "test".to_string(),
                context: vec![],
            },
        )
    }

    #[test]
    fn variant_generation_creates_diverse_steps() {
        let coord = SwarmCoordinator::new(4);
        let step = make_step("goal-1");
        let variants = coord.prepare_parallel_variants(&step, 4);
        assert_eq!(variants.len(), 4);
        // Each variant should have a unique ID
        let ids: Vec<&str> = variants.iter().map(|v| v.id.as_str()).collect();
        let unique: std::collections::HashSet<&str> = ids.iter().copied().collect();
        assert_eq!(unique.len(), 4);
        // Variants should have different max_retries
        let retries: Vec<u32> = variants.iter().map(|v| v.max_retries).collect();
        assert_eq!(retries, vec![1, 2, 3, 4]);
    }

    #[test]
    fn evaluate_results_selects_best() {
        let mut coord = SwarmCoordinator::new(4);
        let s1 = make_step("g1");
        let s2 = make_step("g1");
        let s3 = make_step("g1");

        let results = vec![
            (s1, "low result".to_string(), 0.3),
            (s2.clone(), "high result".to_string(), 0.9),
            (s3, "mid result".to_string(), 0.6),
        ];
        let best = coord.evaluate_swarm_results(results).unwrap();
        // Best solution should be from the highest-scoring result
        assert_eq!(coord.best_solution().unwrap().fitness, 0.9);
        assert_eq!(coord.generation(), 1);
        // The returned step comes from the best solution
        assert!(best.id.len() > 0);
    }

    #[test]
    fn convergence_detection() {
        let mut coord = SwarmCoordinator::new(4);
        // Add 3 solutions with nearly identical fitness
        for _ in 0..3 {
            let step = make_step("g1");
            coord.evaluate_swarm_results(vec![(step, "ok".to_string(), 0.85)]);
        }
        assert!(coord.has_converged(), "identical fitness should converge");
    }

    #[test]
    fn no_convergence_with_diverse_results() {
        let mut coord = SwarmCoordinator::new(4);
        for score in [0.1, 0.5, 0.9] {
            let step = make_step("g1");
            coord.evaluate_swarm_results(vec![(step, "ok".to_string(), score)]);
        }
        assert!(
            !coord.has_converged(),
            "diverse fitness should not converge"
        );
    }

    #[test]
    fn best_solution_tracking() {
        let mut coord = SwarmCoordinator::new(4);
        assert!(coord.best_solution().is_none());

        let s1 = make_step("g1");
        coord.evaluate_swarm_results(vec![(s1, "low".to_string(), 0.3)]);
        assert!((coord.best_solution().unwrap().fitness - 0.3).abs() < f64::EPSILON);

        let s2 = make_step("g1");
        coord.evaluate_swarm_results(vec![(s2, "high".to_string(), 0.95)]);
        assert!((coord.best_solution().unwrap().fitness - 0.95).abs() < f64::EPSILON);
    }

    #[test]
    fn parameter_propagation() {
        let mut coord = SwarmCoordinator::new(4);
        // Create a best solution with max_retries=4
        let mut best_step = make_step("g1");
        best_step.max_retries = 4;
        coord.evaluate_swarm_results(vec![(best_step, "great".to_string(), 0.95)]);

        let mut steps = vec![make_step("g1"), make_step("g1")];
        coord.propagate_best_params(&mut steps);
        for step in &steps {
            assert!(step.max_retries >= 4);
        }
    }

    #[test]
    fn backward_compatible_prepare() {
        let coord = SwarmCoordinator::new(4);
        let mut step = make_step("g1");
        assert_eq!(step.max_retries, 2);
        coord.prepare_parallel_step(&mut step);
        assert_eq!(step.max_retries, 3);
    }

    #[test]
    fn empty_results_returns_none() {
        let mut coord = SwarmCoordinator::new(4);
        assert!(coord.evaluate_swarm_results(vec![]).is_none());
    }
}
