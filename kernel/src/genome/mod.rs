//! Agent DNA — genetic genome system for Nexus OS agents.
//!
//! Every agent gets an [`AgentGenome`] — a structured, evolvable representation
//! of its identity. Two agents can **breed** via [`crossover`] to create a
//! hybrid offspring that inherits traits from both parents.
//!
//! # Gene categories
//!
//! | Category | Controls |
//! |----------|----------|
//! | Personality | System prompt, tone, verbosity, creativity |
//! | Capabilities | Domains, tools, context window size |
//! | Reasoning | Strategy (CoT, ToT), depth, temperature |
//! | Autonomy | Level, risk tolerance, approval requirements |
//! | Evolution | Mutation rate, fitness history, lineage |
//!
//! # Genetic operations
//!
//! - [`mutate`] — perturb numeric genes within bounds
//! - [`crossover`] — breed two parents into an offspring
//! - [`tournament_select`] — fitness-based selection (top 50%)

pub mod converter;
pub mod dna;
pub mod operations;

pub use converter::{genome_from_manifest, manifest_from_genome, JsonAgentManifest};
pub use dna::{
    AgentGenome, AutonomyGenes, CapabilityGenes, EvolutionGenes, GeneSet, PersonalityGenes,
    Phenotype, ReasoningGenes,
};
pub use operations::{
    crossover, mutate, mutate_with_prompt, set_offspring_prompt, tournament_select,
};
