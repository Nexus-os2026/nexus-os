//! Agent DNA — structured, evolvable representation of an agent's identity.
//!
//! Every agent gets a genome with gene categories: personality, capabilities,
//! reasoning, autonomy, and evolution metadata. Two agents can breed to
//! create a hybrid offspring.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Complete genome for a Nexus OS agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentGenome {
    pub genome_version: String,
    pub agent_id: String,
    pub generation: u32,
    /// Parent agent IDs (empty for generation-0 agents).
    pub parents: Vec<String>,
    pub genes: GeneSet,
    pub phenotype: Phenotype,
}

/// All gene categories for an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneSet {
    pub personality: PersonalityGenes,
    pub capabilities: CapabilityGenes,
    pub reasoning: ReasoningGenes,
    pub autonomy: AutonomyGenes,
    pub evolution: EvolutionGenes,
}

/// Personality traits — how the agent communicates and behaves.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalityGenes {
    pub system_prompt: String,
    /// Tone: professional, casual, academic, creative, technical.
    pub tone: String,
    /// 0.0 = terse, 1.0 = verbose.
    pub verbosity: f64,
    /// 0.0 = conservative, 1.0 = wild.
    pub creativity: f64,
    /// 0.0 = passive/advisory, 1.0 = commanding/decisive.
    pub assertiveness: f64,
}

/// What the agent can do — domains and tools.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityGenes {
    pub domains: Vec<String>,
    pub domain_weights: HashMap<String, f64>,
    pub tools: Vec<String>,
    pub max_context_tokens: u64,
}

/// How the agent thinks — strategy and parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningGenes {
    /// Strategy: chain_of_thought, tree_of_thought, react, direct.
    pub strategy: String,
    /// Reasoning depth (number of internal steps).
    pub depth: u32,
    /// LLM temperature.
    pub temperature: f64,
    /// Whether the agent reviews its own output before submitting.
    pub self_reflection: bool,
    /// How many steps ahead the agent plans.
    pub planning_horizon: u32,
}

/// Governance traits — how autonomous the agent is.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutonomyGenes {
    /// Autonomy level (0–6).
    pub level: u32,
    /// 0.0 = risk-averse, 1.0 = risk-seeking.
    pub risk_tolerance: f64,
    /// Confidence threshold below which the agent escalates.
    pub escalation_threshold: f64,
    /// Actions that always require human approval.
    pub requires_approval: Vec<String>,
}

/// Evolution metadata — lineage and mutation parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvolutionGenes {
    /// Probability of mutating each numeric gene per generation.
    pub mutation_rate: f64,
    /// Fitness scores from past evaluations.
    pub fitness_history: Vec<f64>,
    /// Current generation number.
    pub generation: u32,
    /// Ancestor agent IDs (oldest first).
    pub lineage: Vec<String>,
}

/// Observable traits derived from task performance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Phenotype {
    pub avg_task_score: f64,
    pub tasks_completed: u64,
    /// How specialized vs generalist (0.0 = generalist, 1.0 = specialist).
    pub specialization_index: f64,
    pub user_satisfaction: f64,
}

impl AgentGenome {
    /// Create a generation-0 genome with default phenotype.
    pub fn new(agent_id: impl Into<String>, genes: GeneSet) -> Self {
        Self {
            genome_version: "1.0".to_string(),
            agent_id: agent_id.into(),
            generation: 0,
            parents: Vec::new(),
            genes,
            phenotype: Phenotype::default(),
        }
    }

    /// Record a fitness score in the evolution history.
    pub fn record_fitness(&mut self, score: f64) {
        self.genes.evolution.fitness_history.push(score);
        // Update phenotype average
        let history = &self.genes.evolution.fitness_history;
        if !history.is_empty() {
            self.phenotype.avg_task_score = history.iter().sum::<f64>() / history.len() as f64;
        }
        self.phenotype.tasks_completed += 1;
    }

    /// Average fitness across all recorded evaluations.
    pub fn average_fitness(&self) -> f64 {
        let h = &self.genes.evolution.fitness_history;
        if h.is_empty() {
            return 0.0;
        }
        h.iter().sum::<f64>() / h.len() as f64
    }
}

impl Default for Phenotype {
    fn default() -> Self {
        Self {
            avg_task_score: 0.0,
            tasks_completed: 0,
            specialization_index: 0.5,
            user_satisfaction: 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_genes() -> GeneSet {
        GeneSet {
            personality: PersonalityGenes {
                system_prompt: "You are a test agent.".to_string(),
                tone: "professional".to_string(),
                verbosity: 0.5,
                creativity: 0.5,
                assertiveness: 0.5,
            },
            capabilities: CapabilityGenes {
                domains: vec!["testing".to_string()],
                domain_weights: HashMap::from([("testing".to_string(), 1.0)]),
                tools: vec!["fs.read".to_string()],
                max_context_tokens: 128_000,
            },
            reasoning: ReasoningGenes {
                strategy: "chain_of_thought".to_string(),
                depth: 3,
                temperature: 0.7,
                self_reflection: true,
                planning_horizon: 5,
            },
            autonomy: AutonomyGenes {
                level: 3,
                risk_tolerance: 0.4,
                escalation_threshold: 0.7,
                requires_approval: vec!["file_delete".to_string()],
            },
            evolution: EvolutionGenes {
                mutation_rate: 0.1,
                fitness_history: Vec::new(),
                generation: 0,
                lineage: Vec::new(),
            },
        }
    }

    #[test]
    fn genome_serialization_roundtrip() {
        let genome = AgentGenome::new("test-agent", sample_genes());
        let json = serde_json::to_string_pretty(&genome).unwrap();
        let decoded: AgentGenome = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.agent_id, "test-agent");
        assert_eq!(decoded.generation, 0);
        assert!(decoded.parents.is_empty());
    }

    #[test]
    fn record_fitness_updates_phenotype() {
        let mut genome = AgentGenome::new("test-agent", sample_genes());
        genome.record_fitness(0.8);
        genome.record_fitness(0.6);
        assert_eq!(genome.phenotype.tasks_completed, 2);
        assert!((genome.average_fitness() - 0.7).abs() < 1e-9);
    }

    #[test]
    fn default_phenotype() {
        let p = Phenotype::default();
        assert_eq!(p.avg_task_score, 0.0);
        assert_eq!(p.tasks_completed, 0);
    }
}
