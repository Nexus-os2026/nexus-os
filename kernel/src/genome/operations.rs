//! Genetic operations: mutation, crossover (breeding), and selection.
//!
//! All numeric genes are clamped to \[0.0, 1.0\] after mutation.
//! System prompt mutation is left to the caller (requires LLM access).

use super::dna::*;
use rand::Rng;
use std::collections::{HashMap, HashSet};

// ─── Mutation ────────────────────────────────────────────────────────────────

/// Mutate a genome's numeric genes. Each gene is perturbed with probability
/// equal to `genome.genes.evolution.mutation_rate`.
///
/// System prompt is NOT mutated here — call [`mutate_with_prompt`] if you
/// have an LLM-generated prompt variant.
pub fn mutate(genome: &AgentGenome) -> AgentGenome {
    let mut child = genome.clone();
    let mut rng = rand::thread_rng();
    let rate = genome.genes.evolution.mutation_rate;

    // Personality (numeric traits)
    if rng.gen::<f64>() < rate {
        child.genes.personality.verbosity =
            clamp01(perturb(rng.gen(), genome.genes.personality.verbosity));
    }
    if rng.gen::<f64>() < rate {
        child.genes.personality.creativity =
            clamp01(perturb(rng.gen(), genome.genes.personality.creativity));
    }
    if rng.gen::<f64>() < rate {
        child.genes.personality.assertiveness =
            clamp01(perturb(rng.gen(), genome.genes.personality.assertiveness));
    }

    // Reasoning
    if rng.gen::<f64>() < rate {
        child.genes.reasoning.temperature =
            clamp01(perturb(rng.gen(), genome.genes.reasoning.temperature));
    }
    if rng.gen::<f64>() < rate {
        let d = genome.genes.reasoning.depth as f64;
        child.genes.reasoning.depth = (d + rng.gen_range(-1.0..=1.0)).round().max(1.0) as u32;
    }
    if rng.gen::<f64>() < rate {
        let h = genome.genes.reasoning.planning_horizon as f64;
        child.genes.reasoning.planning_horizon =
            (h + rng.gen_range(-1.0..=1.0)).round().max(1.0) as u32;
    }

    // Autonomy (numeric traits — level is NOT mutated for safety)
    if rng.gen::<f64>() < rate {
        child.genes.autonomy.risk_tolerance =
            clamp01(perturb(rng.gen(), genome.genes.autonomy.risk_tolerance));
    }
    if rng.gen::<f64>() < rate {
        child.genes.autonomy.escalation_threshold = clamp01(perturb(
            rng.gen(),
            genome.genes.autonomy.escalation_threshold,
        ));
    }

    // Domain weights
    for weight in child.genes.capabilities.domain_weights.values_mut() {
        if rng.gen::<f64>() < rate {
            *weight = clamp01(perturb(rng.gen(), *weight));
        }
    }

    // Evolution metadata
    child.genes.evolution.generation = genome.genes.evolution.generation + 1;
    child.generation = genome.generation + 1;
    child.genes.evolution.lineage.push(genome.agent_id.clone());
    child.parents = vec![genome.agent_id.clone()];
    child.agent_id = format!("{}-mut{}", genome.agent_id, child.generation);
    child.phenotype = Phenotype::default();

    child
}

/// Apply mutation AND replace the system prompt with an externally-generated
/// variant (e.g. from an LLM-based prompt breeder).
pub fn mutate_with_prompt(genome: &AgentGenome, new_prompt: String) -> AgentGenome {
    let mut child = mutate(genome);
    child.genes.personality.system_prompt = new_prompt;
    child
}

// ─── Crossover (breeding) ────────────────────────────────────────────────────

/// Breed two parent genomes to produce an offspring. Gene categories are
/// inherited whole from one parent (selected per-category), except:
///
/// - **Capabilities**: union of domains, averaged weights
/// - **Autonomy level**: the LOWER (safer) of the two
/// - **System prompt**: concatenation of both (caller should LLM-merge later)
pub fn crossover(parent_a: &AgentGenome, parent_b: &AgentGenome) -> AgentGenome {
    let mut rng = rand::thread_rng();

    // Determine which parent contributes each category (coin flip)
    let personality_from_a: bool = rng.gen();
    let reasoning_from_a: bool = rng.gen();

    // ── Personality ──────────────────────────────────────────────────────
    let personality = if personality_from_a {
        let mut p = parent_a.genes.personality.clone();
        // Blend numeric traits
        p.verbosity = avg(
            parent_a.genes.personality.verbosity,
            parent_b.genes.personality.verbosity,
        );
        p.creativity = avg(
            parent_a.genes.personality.creativity,
            parent_b.genes.personality.creativity,
        );
        p.assertiveness = avg(
            parent_a.genes.personality.assertiveness,
            parent_b.genes.personality.assertiveness,
        );
        // System prompt: combine both so the caller can LLM-merge later
        p.system_prompt = format!(
            "[PARENT A PROMPT]\n{}\n\n[PARENT B PROMPT]\n{}",
            parent_a.genes.personality.system_prompt, parent_b.genes.personality.system_prompt,
        );
        p
    } else {
        let mut p = parent_b.genes.personality.clone();
        p.verbosity = avg(
            parent_a.genes.personality.verbosity,
            parent_b.genes.personality.verbosity,
        );
        p.creativity = avg(
            parent_a.genes.personality.creativity,
            parent_b.genes.personality.creativity,
        );
        p.assertiveness = avg(
            parent_a.genes.personality.assertiveness,
            parent_b.genes.personality.assertiveness,
        );
        p.system_prompt = format!(
            "[PARENT A PROMPT]\n{}\n\n[PARENT B PROMPT]\n{}",
            parent_a.genes.personality.system_prompt, parent_b.genes.personality.system_prompt,
        );
        p
    };

    // ── Capabilities: union ──────────────────────────────────────────────
    let mut domains: Vec<String> = parent_a.genes.capabilities.domains.clone();
    let existing: HashSet<String> = domains.iter().cloned().collect();
    for d in &parent_b.genes.capabilities.domains {
        if !existing.contains(d) {
            domains.push(d.clone());
        }
    }

    let mut domain_weights: HashMap<String, f64> = HashMap::new();
    let all_keys: HashSet<&String> = parent_a
        .genes
        .capabilities
        .domain_weights
        .keys()
        .chain(parent_b.genes.capabilities.domain_weights.keys())
        .collect();
    for key in all_keys {
        let wa = parent_a
            .genes
            .capabilities
            .domain_weights
            .get(key.as_str())
            .copied()
            .unwrap_or(0.0);
        let wb = parent_b
            .genes
            .capabilities
            .domain_weights
            .get(key.as_str())
            .copied()
            .unwrap_or(0.0);
        domain_weights.insert(key.to_string(), avg(wa, wb));
    }

    let mut tools: Vec<String> = parent_a.genes.capabilities.tools.clone();
    let tool_set: HashSet<String> = tools.iter().cloned().collect();
    for t in &parent_b.genes.capabilities.tools {
        if !tool_set.contains(t) {
            tools.push(t.clone());
        }
    }

    let capabilities = CapabilityGenes {
        domains,
        domain_weights,
        tools,
        max_context_tokens: parent_a
            .genes
            .capabilities
            .max_context_tokens
            .max(parent_b.genes.capabilities.max_context_tokens),
    };

    // ── Reasoning ────────────────────────────────────────────────────────
    let reasoning_source = if reasoning_from_a {
        &parent_a.genes
    } else {
        &parent_b.genes
    };
    let reasoning = ReasoningGenes {
        strategy: reasoning_source.reasoning.strategy.clone(),
        depth: (parent_a.genes.reasoning.depth + parent_b.genes.reasoning.depth) / 2,
        temperature: avg(
            parent_a.genes.reasoning.temperature,
            parent_b.genes.reasoning.temperature,
        ),
        self_reflection: parent_a.genes.reasoning.self_reflection
            || parent_b.genes.reasoning.self_reflection,
        planning_horizon: parent_a
            .genes
            .reasoning
            .planning_horizon
            .max(parent_b.genes.reasoning.planning_horizon),
    };

    // ── Autonomy: take the LOWER (safer) level ──────────────────────────
    let autonomy = AutonomyGenes {
        level: parent_a
            .genes
            .autonomy
            .level
            .min(parent_b.genes.autonomy.level),
        risk_tolerance: avg(
            parent_a.genes.autonomy.risk_tolerance,
            parent_b.genes.autonomy.risk_tolerance,
        )
        .min(
            parent_a
                .genes
                .autonomy
                .risk_tolerance
                .min(parent_b.genes.autonomy.risk_tolerance)
                + 0.1,
        ),
        escalation_threshold: parent_a
            .genes
            .autonomy
            .escalation_threshold
            .max(parent_b.genes.autonomy.escalation_threshold),
        requires_approval: {
            let mut approvals: Vec<String> = parent_a.genes.autonomy.requires_approval.clone();
            let set: HashSet<String> = approvals.iter().cloned().collect();
            for a in &parent_b.genes.autonomy.requires_approval {
                if !set.contains(a) {
                    approvals.push(a.clone());
                }
            }
            approvals
        },
    };

    // ── Evolution metadata ───────────────────────────────────────────────
    let gen = parent_a.generation.max(parent_b.generation) + 1;
    let mut lineage = parent_a.genes.evolution.lineage.clone();
    lineage.push(parent_a.agent_id.clone());
    lineage.push(parent_b.agent_id.clone());
    // Deduplicate lineage
    let mut seen = HashSet::new();
    lineage.retain(|id| seen.insert(id.clone()));

    let evolution = EvolutionGenes {
        mutation_rate: avg(
            parent_a.genes.evolution.mutation_rate,
            parent_b.genes.evolution.mutation_rate,
        ),
        fitness_history: Vec::new(),
        generation: gen,
        lineage,
    };

    // ── Offspring name ───────────────────────────────────────────────────
    let a_short = parent_a.agent_id.trim_start_matches("nexus-");
    let b_short = parent_b.agent_id.trim_start_matches("nexus-");
    let offspring_id = format!("{a_short}-{b_short}-gen{gen}");

    AgentGenome {
        genome_version: "1.0".to_string(),
        agent_id: offspring_id,
        generation: gen,
        parents: vec![parent_a.agent_id.clone(), parent_b.agent_id.clone()],
        genes: GeneSet {
            personality,
            capabilities,
            reasoning,
            autonomy,
            evolution,
        },
        phenotype: Phenotype::default(),
    }
}

/// After crossover, replace the concatenated prompt placeholder with an
/// LLM-bred system prompt.
pub fn set_offspring_prompt(genome: &mut AgentGenome, bred_prompt: String) {
    genome.genes.personality.system_prompt = bred_prompt;
}

// ─── Selection ───────────────────────────────────────────────────────────────

/// Tournament selection: from `candidates`, return the top half by fitness.
/// Each candidate is paired randomly; the higher-fitness one advances.
pub fn tournament_select(candidates: &[AgentGenome]) -> Vec<AgentGenome> {
    if candidates.len() <= 1 {
        return candidates.to_vec();
    }

    let mut indexed: Vec<(usize, f64)> = candidates
        .iter()
        .enumerate()
        .map(|(i, g)| (i, g.average_fitness()))
        .collect();

    // Sort by fitness descending
    indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let keep = candidates.len().div_ceil(2); // top 50%
    indexed
        .into_iter()
        .take(keep)
        .map(|(i, _)| candidates[i].clone())
        .collect()
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn clamp01(v: f64) -> f64 {
    v.clamp(0.0, 1.0)
}

/// Perturb a value by ±10% using a uniform random seed in \[0, 1\].
fn perturb(seed: f64, value: f64) -> f64 {
    let delta = (seed - 0.5) * 0.2; // ±0.1
    value + delta
}

fn avg(a: f64, b: f64) -> f64 {
    (a + b) / 2.0
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_genome(id: &str, level: u32, domains: Vec<&str>) -> AgentGenome {
        AgentGenome::new(
            id,
            GeneSet {
                personality: PersonalityGenes {
                    system_prompt: format!("You are {id}."),
                    tone: "professional".to_string(),
                    verbosity: 0.5,
                    creativity: 0.5,
                    assertiveness: 0.5,
                },
                capabilities: CapabilityGenes {
                    domains: domains.iter().map(|s| s.to_string()).collect(),
                    domain_weights: domains.iter().map(|s| (s.to_string(), 0.8)).collect(),
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
                    level,
                    risk_tolerance: 0.4,
                    escalation_threshold: 0.7,
                    requires_approval: vec!["file_delete".to_string()],
                },
                evolution: EvolutionGenes {
                    mutation_rate: 0.5, // high rate so mutations are likely in tests
                    fitness_history: Vec::new(),
                    generation: 0,
                    lineage: Vec::new(),
                },
            },
        )
    }

    #[test]
    fn mutation_increments_generation() {
        let parent = make_genome("nexus-forge", 3, vec!["code"]);
        let child = mutate(&parent);
        assert_eq!(child.generation, 1);
        assert_eq!(child.genes.evolution.generation, 1);
        assert_eq!(child.parents, vec!["nexus-forge"]);
        assert!(child
            .genes
            .evolution
            .lineage
            .contains(&"nexus-forge".to_string()));
    }

    #[test]
    fn mutation_preserves_autonomy_level() {
        let parent = make_genome("nexus-forge", 3, vec!["code"]);
        // Run mutation many times — level should never change
        for _ in 0..50 {
            let child = mutate(&parent);
            assert_eq!(
                child.genes.autonomy.level, 3,
                "autonomy level must not mutate"
            );
        }
    }

    #[test]
    fn mutation_clamps_to_unit_interval() {
        let mut parent = make_genome("nexus-forge", 3, vec!["code"]);
        parent.genes.personality.verbosity = 0.99;
        parent.genes.personality.creativity = 0.01;
        for _ in 0..100 {
            let child = mutate(&parent);
            assert!(
                child.genes.personality.verbosity >= 0.0
                    && child.genes.personality.verbosity <= 1.0
            );
            assert!(
                child.genes.personality.creativity >= 0.0
                    && child.genes.personality.creativity <= 1.0
            );
        }
    }

    #[test]
    fn crossover_merges_capabilities() {
        let a = make_genome("nexus-forge", 3, vec!["code", "architecture"]);
        let b = make_genome("nexus-scholar", 3, vec!["research", "writing"]);
        let child = crossover(&a, &b);

        assert!(child
            .genes
            .capabilities
            .domains
            .contains(&"code".to_string()));
        assert!(child
            .genes
            .capabilities
            .domains
            .contains(&"research".to_string()));
        assert!(child
            .genes
            .capabilities
            .domains
            .contains(&"architecture".to_string()));
        assert!(child
            .genes
            .capabilities
            .domains
            .contains(&"writing".to_string()));
    }

    #[test]
    fn crossover_takes_safer_autonomy() {
        let a = make_genome("nexus-forge", 3, vec!["code"]);
        let b = make_genome("nexus-sentinel", 1, vec!["security"]);
        let child = crossover(&a, &b);
        assert_eq!(
            child.genes.autonomy.level, 1,
            "offspring should inherit lower autonomy"
        );
    }

    #[test]
    fn crossover_sets_parents() {
        let a = make_genome("nexus-forge", 3, vec!["code"]);
        let b = make_genome("nexus-scholar", 3, vec!["research"]);
        let child = crossover(&a, &b);
        assert_eq!(child.parents.len(), 2);
        assert!(child.parents.contains(&"nexus-forge".to_string()));
        assert!(child.parents.contains(&"nexus-scholar".to_string()));
        assert_eq!(child.generation, 1);
    }

    #[test]
    fn crossover_unions_tools() {
        let mut a = make_genome("a", 3, vec!["code"]);
        a.genes.capabilities.tools = vec!["fs.read".to_string(), "fs.write".to_string()];
        let mut b = make_genome("b", 3, vec!["research"]);
        b.genes.capabilities.tools = vec!["web.search".to_string(), "fs.read".to_string()];

        let child = crossover(&a, &b);
        assert_eq!(child.genes.capabilities.tools.len(), 3); // fs.read, fs.write, web.search
    }

    #[test]
    fn crossover_averages_domain_weights() {
        let mut a = make_genome("a", 3, vec!["code"]);
        a.genes
            .capabilities
            .domain_weights
            .insert("code".to_string(), 0.9);
        let mut b = make_genome("b", 3, vec!["code"]);
        b.genes
            .capabilities
            .domain_weights
            .insert("code".to_string(), 0.5);

        let child = crossover(&a, &b);
        let w = child
            .genes
            .capabilities
            .domain_weights
            .get("code")
            .copied()
            .unwrap_or(0.0);
        assert!(
            (w - 0.7).abs() < 1e-9,
            "expected averaged weight 0.7, got {w}"
        );
    }

    #[test]
    fn tournament_selection_keeps_top_half() {
        let agents: Vec<AgentGenome> = (0..4)
            .map(|i| {
                let mut g = make_genome(&format!("agent-{i}"), 3, vec!["test"]);
                g.genes.evolution.fitness_history = vec![i as f64 * 0.25];
                g
            })
            .collect();
        // fitness: 0.0, 0.25, 0.50, 0.75

        let survivors = tournament_select(&agents);
        assert_eq!(survivors.len(), 2);
        // Top two should be agent-3 (0.75) and agent-2 (0.50)
        let ids: Vec<&str> = survivors.iter().map(|g| g.agent_id.as_str()).collect();
        assert!(ids.contains(&"agent-3"));
        assert!(ids.contains(&"agent-2"));
    }

    #[test]
    fn mutate_with_prompt_replaces_system_prompt() {
        let parent = make_genome("nexus-forge", 3, vec!["code"]);
        let child = mutate_with_prompt(&parent, "You are improved.".to_string());
        assert_eq!(child.genes.personality.system_prompt, "You are improved.");
    }

    #[test]
    fn set_offspring_prompt_works() {
        let a = make_genome("a", 3, vec!["code"]);
        let b = make_genome("b", 3, vec!["research"]);
        let mut child = crossover(&a, &b);
        assert!(child
            .genes
            .personality
            .system_prompt
            .contains("[PARENT A PROMPT]"));
        set_offspring_prompt(&mut child, "You are a hybrid agent.".to_string());
        assert_eq!(
            child.genes.personality.system_prompt,
            "You are a hybrid agent."
        );
    }
}
