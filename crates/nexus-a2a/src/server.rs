//! A2A Server — makes Nexus OS agents discoverable via the A2A protocol.
//!
//! Builds an `AgentCard` for the Nexus OS instance that aggregates skills from
//! all registered agents.  The card is served at `GET /.well-known/agent.json`
//! and individual agent cards at `GET /a2a/agent-card?agent=<name>`.

use crate::types::{
    AgentCapabilities, AgentCard, AgentSkill, AuthScheme, SkillSummary, A2A_PROTOCOL_VERSION,
};
use nexus_kernel::manifest::AgentManifest;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Registry of agent skills for the A2A server.
///
/// Collects skills from all registered Nexus agents and builds a composite
/// AgentCard representing the entire Nexus OS instance.
#[derive(Debug, Clone)]
pub struct SkillRegistry {
    /// Per-agent skills, keyed by agent name.
    agent_skills: HashMap<String, Vec<AgentSkill>>,
    /// Base URL for the A2A server.
    base_url: String,
    /// Instance name.
    instance_name: String,
}

/// Summary of a registered agent in the skill registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisteredAgent {
    pub name: String,
    pub skill_count: usize,
    pub capabilities: Vec<String>,
}

impl SkillRegistry {
    /// Create a new skill registry.
    pub fn new(instance_name: impl Into<String>, base_url: impl Into<String>) -> Self {
        Self {
            agent_skills: HashMap::new(),
            base_url: base_url.into(),
            instance_name: instance_name.into(),
        }
    }

    /// Register an agent's manifest, extracting its skills.
    pub fn register_manifest(&mut self, manifest: &AgentManifest) {
        let card = AgentCard::from_manifest(manifest, &self.base_url);
        self.agent_skills.insert(manifest.name.clone(), card.skills);
    }

    /// Register skills directly for an agent.
    pub fn register_skills(&mut self, agent_name: impl Into<String>, skills: Vec<AgentSkill>) {
        self.agent_skills.insert(agent_name.into(), skills);
    }

    /// Remove an agent from the registry.
    pub fn unregister(&mut self, agent_name: &str) -> bool {
        self.agent_skills.remove(agent_name).is_some()
    }

    /// Get all skills across all agents, flattened.
    pub fn all_skills(&self) -> Vec<&AgentSkill> {
        self.agent_skills.values().flat_map(|v| v.iter()).collect()
    }

    /// Get skill summaries for all agents.
    pub fn all_skill_summaries(&self) -> Vec<SkillSummary> {
        self.all_skills()
            .iter()
            .map(|s| SkillSummary::from(*s))
            .collect()
    }

    /// Get skills for a specific agent.
    pub fn agent_skills(&self, agent_name: &str) -> Option<&[AgentSkill]> {
        self.agent_skills.get(agent_name).map(|v| v.as_slice())
    }

    /// List all registered agents.
    pub fn registered_agents(&self) -> Vec<RegisteredAgent> {
        self.agent_skills
            .iter()
            .map(|(name, skills)| RegisteredAgent {
                name: name.clone(),
                skill_count: skills.len(),
                capabilities: skills.iter().map(|s| s.id.clone()).collect(),
            })
            .collect()
    }

    /// Total number of registered agents.
    pub fn agent_count(&self) -> usize {
        self.agent_skills.len()
    }

    /// Total number of skills across all agents.
    pub fn total_skills(&self) -> usize {
        self.agent_skills.values().map(|v| v.len()).sum()
    }

    /// Build the composite AgentCard for the entire Nexus OS instance.
    ///
    /// This card aggregates all skills from all registered agents and is
    /// served at `GET /.well-known/agent.json`.
    pub fn build_instance_card(&self) -> AgentCard {
        let all_skills: Vec<AgentSkill> = self
            .agent_skills
            .values()
            .flat_map(|v| v.iter().cloned())
            .collect();

        AgentCard {
            name: self.instance_name.clone(),
            description: Some(format!(
                "Nexus OS governed agent instance with {} agents and {} skills",
                self.agent_skills.len(),
                all_skills.len()
            )),
            url: format!("{}/a2a", self.base_url.trim_end_matches('/')),
            version: A2A_PROTOCOL_VERSION.to_string(),
            capabilities: AgentCapabilities {
                streaming: false,
                push_notifications: false,
                state_transition_history: true,
            },
            skills: all_skills,
            authentication: vec![AuthScheme {
                scheme_type: "bearer".to_string(),
                description: Some("JWT bearer token for governed access".to_string()),
            }],
            default_input_modes: vec!["text/plain".to_string(), "application/json".to_string()],
            default_output_modes: vec!["application/json".to_string(), "text/plain".to_string()],
            rate_limit_rpm: Some(60),
        }
    }

    /// Build an AgentCard for a specific registered agent.
    pub fn build_agent_card(&self, agent_name: &str) -> Option<AgentCard> {
        let skills = self.agent_skills.get(agent_name)?;
        Some(AgentCard {
            name: agent_name.to_string(),
            description: Some(format!(
                "Nexus governed agent '{}' with {} skills",
                agent_name,
                skills.len()
            )),
            url: format!("{}/a2a/{}", self.base_url.trim_end_matches('/'), agent_name),
            version: A2A_PROTOCOL_VERSION.to_string(),
            capabilities: AgentCapabilities {
                streaming: false,
                push_notifications: false,
                state_transition_history: false,
            },
            skills: skills.clone(),
            authentication: vec![AuthScheme {
                scheme_type: "bearer".to_string(),
                description: Some("JWT bearer token".to_string()),
            }],
            default_input_modes: vec!["text/plain".to_string()],
            default_output_modes: vec!["application/json".to_string()],
            rate_limit_rpm: Some(30),
        })
    }

    /// Find agents that have a skill matching the given tag.
    pub fn find_agents_by_tag(&self, tag: &str) -> Vec<String> {
        self.agent_skills
            .iter()
            .filter(|(_, skills)| skills.iter().any(|s| s.tags.iter().any(|t| t == tag)))
            .map(|(name, _)| name.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_skill(id: &str, name: &str, tags: &[&str]) -> AgentSkill {
        AgentSkill {
            id: id.to_string(),
            name: name.to_string(),
            description: Some(format!("{name} skill")),
            tags: tags.iter().map(|t| (*t).to_string()).collect(),
            input_modes: vec!["text/plain".to_string()],
            output_modes: vec!["application/json".to_string()],
        }
    }

    #[test]
    fn registry_new_is_empty() {
        let reg = SkillRegistry::new("nexus", "http://localhost:9090");
        assert_eq!(reg.agent_count(), 0);
        assert_eq!(reg.total_skills(), 0);
    }

    #[test]
    fn register_and_list_skills() {
        let mut reg = SkillRegistry::new("nexus", "http://localhost:9090");
        reg.register_skills(
            "coder",
            vec![
                make_skill("code-gen", "Code Generation", &["code", "generation"]),
                make_skill("code-review", "Code Review", &["code", "review"]),
            ],
        );
        assert_eq!(reg.agent_count(), 1);
        assert_eq!(reg.total_skills(), 2);
        assert_eq!(reg.all_skills().len(), 2);
    }

    #[test]
    fn register_multiple_agents() {
        let mut reg = SkillRegistry::new("nexus", "http://localhost:9090");
        reg.register_skills("coder", vec![make_skill("gen", "Gen", &["code"])]);
        reg.register_skills("researcher", vec![make_skill("search", "Search", &["web"])]);
        assert_eq!(reg.agent_count(), 2);
        assert_eq!(reg.total_skills(), 2);
    }

    #[test]
    fn unregister_agent() {
        let mut reg = SkillRegistry::new("nexus", "http://localhost:9090");
        reg.register_skills("temp", vec![make_skill("s1", "S1", &["test"])]);
        assert!(reg.unregister("temp"));
        assert!(!reg.unregister("temp")); // already gone
        assert_eq!(reg.agent_count(), 0);
    }

    #[test]
    fn build_instance_card_aggregates_all_skills() {
        let mut reg = SkillRegistry::new("nexus-os", "https://nexus.example.com");
        reg.register_skills("agent-a", vec![make_skill("s1", "S1", &["web"])]);
        reg.register_skills("agent-b", vec![make_skill("s2", "S2", &["code"])]);
        reg.register_skills(
            "agent-c",
            vec![
                make_skill("s3", "S3", &["data"]),
                make_skill("s4", "S4", &["ai"]),
            ],
        );

        let card = reg.build_instance_card();
        assert_eq!(card.name, "nexus-os");
        assert_eq!(card.skills.len(), 4);
        assert_eq!(card.version, A2A_PROTOCOL_VERSION);
        assert!(card.url.contains("/a2a"));
    }

    #[test]
    fn build_agent_card_for_specific_agent() {
        let mut reg = SkillRegistry::new("nexus", "https://nexus.example.com");
        reg.register_skills("coder", vec![make_skill("gen", "Gen", &["code"])]);

        let card = reg.build_agent_card("coder").unwrap();
        assert_eq!(card.name, "coder");
        assert_eq!(card.skills.len(), 1);
        assert!(reg.build_agent_card("nonexistent").is_none());
    }

    #[test]
    fn find_agents_by_tag() {
        let mut reg = SkillRegistry::new("nexus", "http://localhost");
        reg.register_skills(
            "web-agent",
            vec![make_skill("search", "Search", &["web", "search"])],
        );
        reg.register_skills(
            "code-agent",
            vec![make_skill("gen", "Gen", &["code", "generation"])],
        );
        reg.register_skills(
            "mixed",
            vec![
                make_skill("s1", "S1", &["web"]),
                make_skill("s2", "S2", &["code"]),
            ],
        );

        let web_agents = reg.find_agents_by_tag("web");
        assert_eq!(web_agents.len(), 2);
        assert!(web_agents.contains(&"web-agent".to_string()));
        assert!(web_agents.contains(&"mixed".to_string()));

        let code_agents = reg.find_agents_by_tag("code");
        assert_eq!(code_agents.len(), 2);
    }

    #[test]
    fn registered_agents_list() {
        let mut reg = SkillRegistry::new("nexus", "http://localhost");
        reg.register_skills("a", vec![make_skill("s1", "S1", &["t1"])]);
        reg.register_skills(
            "b",
            vec![
                make_skill("s2", "S2", &["t2"]),
                make_skill("s3", "S3", &["t3"]),
            ],
        );

        let agents = reg.registered_agents();
        assert_eq!(agents.len(), 2);
        let a = agents.iter().find(|a| a.name == "a").unwrap();
        assert_eq!(a.skill_count, 1);
        let b = agents.iter().find(|a| a.name == "b").unwrap();
        assert_eq!(b.skill_count, 2);
    }

    #[test]
    fn instance_card_json_roundtrip() {
        let mut reg = SkillRegistry::new("nexus", "http://localhost:9090");
        reg.register_skills("agent", vec![make_skill("s1", "S1", &["test"])]);
        let card = reg.build_instance_card();
        let json = serde_json::to_string(&card).unwrap();
        let parsed: AgentCard = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "nexus");
        assert_eq!(parsed.skills.len(), 1);
    }
}
