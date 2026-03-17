//! Agent manifest generation — create complete agent manifests from specs.

use serde::{Deserialize, Serialize};

use crate::genome::{genome_from_manifest, JsonAgentManifest};

/// Specification for a new agent to be created by the Genesis engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSpec {
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub system_prompt: String,
    pub autonomy_level: u32,
    pub capabilities: Vec<String>,
    pub tools: Vec<String>,
    pub category: String,
    pub reasoning_strategy: String,
    pub temperature: f64,
    /// Agent IDs that inspired this agent's creation.
    pub parent_agents: Vec<String>,
}

/// Build the LLM prompt to generate a system prompt for the new agent.
pub fn build_system_prompt_generation_prompt(spec: &AgentSpec) -> String {
    format!(
        r#"You are an expert AI agent designer for Nexus OS, a governed AI operating system.

Create a detailed system prompt for a new AI agent with these specifications:

Name: {name}
Display Name: {display_name}
Purpose: {description}
Capabilities: {capabilities}
Autonomy Level: L{level} ({level_desc})
Category: {category}
Reasoning Strategy: {strategy}

The system prompt should:
- Start with "You are {display_name}, ..." establishing the agent's identity
- Define the agent's expertise domains and specialist knowledge
- Specify how it approaches tasks in its domain step by step
- Include safety guidelines appropriate for L{level} autonomy
- Be specific enough to outperform a generic LLM on this domain
- Be 200-400 words
- Use numbered guidelines (like other Nexus agents)

Return ONLY the system prompt text, no JSON wrapping, no code blocks."#,
        name = spec.name,
        display_name = spec.display_name,
        description = spec.description,
        capabilities = spec.capabilities.join(", "),
        level = spec.autonomy_level,
        level_desc = autonomy_level_description(spec.autonomy_level),
        category = spec.category,
        strategy = spec.reasoning_strategy,
    )
}

/// Generate a `JsonAgentManifest` from a spec with the generated system prompt.
pub fn generate_manifest(spec: &AgentSpec) -> JsonAgentManifest {
    let fuel_budget = match spec.autonomy_level {
        0..=1 => 5_000,
        2 => 10_000,
        3 => 15_000,
        4 => 25_000,
        _ => 50_000,
    };

    JsonAgentManifest {
        name: spec.name.clone(),
        version: "1.0.0".to_string(),
        description: spec.system_prompt.clone(),
        capabilities: spec.capabilities.clone(),
        autonomy_level: spec.autonomy_level,
        fuel_budget,
        llm_model: None,
        schedule: None,
        default_goal: None,
    }
}

/// Generate the genome JSON for the new agent (for storage alongside the manifest).
pub fn generate_genome_json(spec: &AgentSpec) -> Result<String, String> {
    let manifest = generate_manifest(spec);
    let genome = genome_from_manifest(&manifest);
    serde_json::to_string_pretty(&genome).map_err(|e| format!("Genome serialization error: {e}"))
}

fn autonomy_level_description(level: u32) -> &'static str {
    match level {
        0 => "Inert",
        1 => "Suggest — human decides",
        2 => "Act with approval — human approves",
        3 => "Act then report — post-action review",
        4 => "Autonomous bounded — anomaly-triggered",
        5 => "Full autonomy — kernel override only",
        _ => "Transcendent",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_spec() -> AgentSpec {
        AgentSpec {
            name: "nexus-testgen".to_string(),
            display_name: "TestGen".to_string(),
            description: "A test agent for unit testing".to_string(),
            system_prompt: "You are TestGen, a testing specialist.".to_string(),
            autonomy_level: 3,
            capabilities: vec!["fs.read".to_string(), "fs.write".to_string()],
            tools: vec!["fs.read".to_string(), "fs.write".to_string()],
            category: "coding".to_string(),
            reasoning_strategy: "chain_of_thought".to_string(),
            temperature: 0.7,
            parent_agents: Vec::new(),
        }
    }

    #[test]
    fn generate_manifest_from_spec() {
        let spec = sample_spec();
        let manifest = generate_manifest(&spec);
        assert_eq!(manifest.name, "nexus-testgen");
        assert_eq!(manifest.autonomy_level, 3);
        assert_eq!(manifest.fuel_budget, 15_000);
        assert_eq!(manifest.capabilities, spec.capabilities);
    }

    #[test]
    fn generate_genome_from_spec() {
        let spec = sample_spec();
        let json = generate_genome_json(&spec).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["agent_id"], "nexus-testgen");
        assert_eq!(parsed["generation"], 0);
    }

    #[test]
    fn system_prompt_generation_prompt_includes_spec() {
        let spec = sample_spec();
        let prompt = build_system_prompt_generation_prompt(&spec);
        assert!(prompt.contains("nexus-testgen"));
        assert!(prompt.contains("TestGen"));
        assert!(prompt.contains("L3"));
    }

    #[test]
    fn fuel_budget_scales_with_autonomy() {
        let mut spec = sample_spec();
        spec.autonomy_level = 1;
        assert_eq!(generate_manifest(&spec).fuel_budget, 5_000);
        spec.autonomy_level = 4;
        assert_eq!(generate_manifest(&spec).fuel_budget, 25_000);
    }
}
