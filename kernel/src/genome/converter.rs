//! Convert agent manifests to genomes and back.
//!
//! Reads `agents/prebuilt/*.json` and generates generation-0 genomes.

use super::dna::*;
use std::collections::HashMap;

/// A simplified agent manifest as stored in `agents/prebuilt/*.json`.
///
/// This is deliberately separate from `kernel::manifest::AgentManifest` which
/// is TOML-oriented. The prebuilt JSON files include a `description` field
/// that serves as the system prompt.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct JsonAgentManifest {
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub autonomy_level: u32,
    #[serde(default)]
    pub fuel_budget: u64,
    #[serde(default)]
    pub llm_model: Option<String>,
    #[serde(default)]
    pub schedule: Option<String>,
    #[serde(default)]
    pub default_goal: Option<String>,
}

/// Generate a generation-0 genome from a prebuilt agent manifest.
pub fn genome_from_manifest(manifest: &JsonAgentManifest) -> AgentGenome {
    let description = &manifest.description;

    // Infer personality traits from description text
    let tone = infer_tone(description);
    let verbosity = infer_verbosity(description);
    let creativity = infer_creativity(description);
    let assertiveness = infer_assertiveness(description, manifest.autonomy_level);

    // Map capabilities to domain categories
    let (domains, domain_weights) = infer_domains(&manifest.capabilities, description);

    // Infer reasoning from autonomy level
    let (strategy, depth, self_reflection, planning_horizon) =
        infer_reasoning(manifest.autonomy_level);

    let genes = GeneSet {
        personality: PersonalityGenes {
            system_prompt: description.clone(),
            tone,
            verbosity,
            creativity,
            assertiveness,
        },
        capabilities: CapabilityGenes {
            domains,
            domain_weights,
            tools: manifest.capabilities.clone(),
            max_context_tokens: 128_000,
        },
        reasoning: ReasoningGenes {
            strategy,
            depth,
            temperature: infer_temperature(manifest.autonomy_level),
            self_reflection,
            planning_horizon,
        },
        autonomy: AutonomyGenes {
            level: manifest.autonomy_level,
            risk_tolerance: infer_risk_tolerance(manifest.autonomy_level),
            escalation_threshold: infer_escalation_threshold(manifest.autonomy_level),
            requires_approval: infer_approval_requirements(&manifest.capabilities),
        },
        evolution: EvolutionGenes {
            mutation_rate: 0.1,
            fitness_history: Vec::new(),
            generation: 0,
            lineage: Vec::new(),
        },
    };

    AgentGenome::new(&manifest.name, genes)
}

/// Convert a genome back to a JSON manifest (for agent registration).
pub fn manifest_from_genome(genome: &AgentGenome) -> JsonAgentManifest {
    JsonAgentManifest {
        name: genome.agent_id.clone(),
        version: genome.genome_version.clone(),
        description: genome.genes.personality.system_prompt.clone(),
        capabilities: genome.genes.capabilities.tools.clone(),
        autonomy_level: genome.genes.autonomy.level,
        fuel_budget: 10_000,
        llm_model: None,
        schedule: None,
        default_goal: None,
    }
}

// ─── Inference helpers ───────────────────────────────────────────────────────

fn infer_tone(description: &str) -> String {
    let lower = description.to_lowercase();
    if lower.contains("academic") || lower.contains("research") || lower.contains("scholarly") {
        "academic".to_string()
    } else if lower.contains("creative") || lower.contains("artistic") || lower.contains("design") {
        "creative".to_string()
    } else if lower.contains("casual") || lower.contains("friendly") || lower.contains("chat") {
        "casual".to_string()
    } else if lower.contains("code") || lower.contains("debug") || lower.contains("engineer") {
        "technical".to_string()
    } else {
        "professional".to_string()
    }
}

fn infer_verbosity(description: &str) -> f64 {
    let word_count = description.split_whitespace().count();
    // Longer descriptions tend to indicate more verbose agents
    match word_count {
        0..=50 => 0.3,
        51..=150 => 0.5,
        151..=300 => 0.7,
        _ => 0.9,
    }
}

fn infer_creativity(description: &str) -> f64 {
    let lower = description.to_lowercase();
    let creative_terms = [
        "creative",
        "artistic",
        "design",
        "brainstorm",
        "innovative",
        "imagine",
        "novel",
        "original",
        "invent",
    ];
    let count = creative_terms.iter().filter(|t| lower.contains(*t)).count();
    (0.4 + count as f64 * 0.1).min(1.0)
}

fn infer_assertiveness(description: &str, autonomy_level: u32) -> f64 {
    let base: f64 = match autonomy_level {
        0..=1 => 0.2,
        2 => 0.4,
        3 => 0.5,
        4 => 0.6,
        5 => 0.8,
        _ => 0.9,
    };
    let lower = description.to_lowercase();
    if lower.contains("autonomous") || lower.contains("decisive") || lower.contains("command") {
        (base + 0.1).min(1.0)
    } else {
        base
    }
}

fn infer_domains(
    capabilities: &[String],
    description: &str,
) -> (Vec<String>, HashMap<String, f64>) {
    let lower = description.to_lowercase();
    let mut domains = Vec::new();
    let mut weights = HashMap::new();

    let domain_map: Vec<(&str, &[&str])> = vec![
        (
            "code_generation",
            &["code", "program", "implement", "write code", "build"],
        ),
        (
            "code_review",
            &["review", "audit", "scan", "quality", "lint"],
        ),
        (
            "debugging",
            &["debug", "fix", "bug", "error", "troubleshoot"],
        ),
        (
            "architecture",
            &["architect", "design", "system", "infrastructure"],
        ),
        ("research", &["research", "investigate", "analyze", "study"]),
        (
            "writing",
            &["write", "content", "blog", "article", "documentation"],
        ),
        (
            "security",
            &["security", "vulnerability", "threat", "audit", "defense"],
        ),
        (
            "data_analysis",
            &["data", "analytics", "metrics", "statistics"],
        ),
        (
            "teaching",
            &["teach", "explain", "learn", "tutor", "mentor"],
        ),
        ("planning", &["plan", "strategy", "roadmap", "coordinate"]),
        ("social_media", &["social", "post", "tweet", "community"]),
        ("web_automation", &["web", "browse", "scrape", "navigate"]),
        ("devops", &["deploy", "ci/cd", "pipeline", "infrastructure"]),
    ];

    for (domain, keywords) in &domain_map {
        let match_count = keywords.iter().filter(|kw| lower.contains(*kw)).count();
        if match_count > 0 {
            let weight = (0.5 + match_count as f64 * 0.15).min(1.0);
            domains.push(domain.to_string());
            weights.insert(domain.to_string(), weight);
        }
    }

    // Add capability-derived domains
    for cap in capabilities {
        match cap.as_str() {
            "web.search" | "web.read" if !domains.contains(&"web_automation".to_string()) => {
                domains.push("web_automation".to_string());
                weights.insert("web_automation".to_string(), 0.6);
            }
            "fs.read" | "fs.write" if !domains.contains(&"code_generation".to_string()) => {
                domains.push("code_generation".to_string());
                weights.insert("code_generation".to_string(), 0.5);
            }
            _ => {}
        }
    }

    if domains.is_empty() {
        domains.push("general".to_string());
        weights.insert("general".to_string(), 0.5);
    }

    (domains, weights)
}

fn infer_reasoning(autonomy_level: u32) -> (String, u32, bool, u32) {
    match autonomy_level {
        0..=1 => ("direct".to_string(), 1, false, 1),
        2 => ("chain_of_thought".to_string(), 2, false, 3),
        3 => ("chain_of_thought".to_string(), 3, true, 5),
        4 => ("tree_of_thought".to_string(), 4, true, 7),
        5 => ("tree_of_thought".to_string(), 5, true, 10),
        _ => ("tree_of_thought".to_string(), 5, true, 15),
    }
}

fn infer_temperature(autonomy_level: u32) -> f64 {
    match autonomy_level {
        0..=1 => 0.3,
        2 => 0.5,
        3 => 0.7,
        4..=5 => 0.8,
        _ => 0.9,
    }
}

fn infer_risk_tolerance(autonomy_level: u32) -> f64 {
    match autonomy_level {
        0 => 0.0,
        1 => 0.1,
        2 => 0.3,
        3 => 0.4,
        4 => 0.6,
        5 => 0.8,
        _ => 0.9,
    }
}

fn infer_escalation_threshold(autonomy_level: u32) -> f64 {
    // Lower autonomy → escalate more readily (lower threshold)
    match autonomy_level {
        0..=1 => 0.3,
        2 => 0.5,
        3 => 0.7,
        4 => 0.8,
        5 => 0.9,
        _ => 0.95,
    }
}

fn infer_approval_requirements(capabilities: &[String]) -> Vec<String> {
    let mut approvals = Vec::new();
    for cap in capabilities {
        match cap.as_str() {
            "fs.write" => approvals.push("file_delete".to_string()),
            "process.exec" => approvals.push("system_command".to_string()),
            "social.post" | "social.x.post" => approvals.push("social_post".to_string()),
            "messaging.send" => approvals.push("send_message".to_string()),
            _ => {}
        }
    }
    approvals
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_manifest() -> JsonAgentManifest {
        JsonAgentManifest {
            name: "nexus-forge".to_string(),
            version: "1.0.0".to_string(),
            description: "You are Nexus Forge, the content creation powerhouse. You research, write, edit, and refine any type of written content to publication quality.".to_string(),
            capabilities: vec!["web.search".to_string(), "web.read".to_string(), "fs.read".to_string(), "fs.write".to_string()],
            autonomy_level: 3,
            fuel_budget: 12000,
            llm_model: Some("qwen3.5:9b".to_string()),
            schedule: None,
            default_goal: None,
        }
    }

    #[test]
    fn manifest_to_genome_basic() {
        let genome = genome_from_manifest(&sample_manifest());
        assert_eq!(genome.agent_id, "nexus-forge");
        assert_eq!(genome.generation, 0);
        assert!(genome.parents.is_empty());
        assert_eq!(genome.genes.autonomy.level, 3);
    }

    #[test]
    fn manifest_to_genome_infers_domains() {
        let genome = genome_from_manifest(&sample_manifest());
        // Should infer writing domain from "write" and "content" in description
        assert!(
            genome
                .genes
                .capabilities
                .domains
                .contains(&"writing".to_string()),
            "should infer writing domain, got: {:?}",
            genome.genes.capabilities.domains
        );
    }

    #[test]
    fn manifest_to_genome_preserves_tools() {
        let manifest = sample_manifest();
        let genome = genome_from_manifest(&manifest);
        assert_eq!(genome.genes.capabilities.tools, manifest.capabilities);
    }

    #[test]
    fn genome_to_manifest_roundtrip() {
        let manifest = sample_manifest();
        let genome = genome_from_manifest(&manifest);
        let back = manifest_from_genome(&genome);
        assert_eq!(back.name, "nexus-forge");
        assert_eq!(back.autonomy_level, 3);
        assert_eq!(back.capabilities, manifest.capabilities);
    }

    #[test]
    fn infer_tone_technical() {
        assert_eq!(infer_tone("You are a code debugging engineer"), "technical");
    }

    #[test]
    fn infer_tone_academic() {
        assert_eq!(infer_tone("A scholarly research assistant"), "academic");
    }

    #[test]
    fn infer_tone_creative() {
        assert_eq!(infer_tone("An artistic creative design agent"), "creative");
    }

    #[test]
    fn security_agent_has_security_domain() {
        let manifest = JsonAgentManifest {
            name: "nexus-sentinel".to_string(),
            version: "1.0.0".to_string(),
            description: "You scan for security vulnerabilities and audit code for threats."
                .to_string(),
            capabilities: vec!["fs.read".to_string()],
            autonomy_level: 2,
            fuel_budget: 5000,
            llm_model: None,
            schedule: None,
            default_goal: None,
        };
        let genome = genome_from_manifest(&manifest);
        assert!(genome
            .genes
            .capabilities
            .domains
            .contains(&"security".to_string()));
    }
}
