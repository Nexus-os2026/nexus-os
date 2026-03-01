use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum AgentRole {
    Researcher,
    Writer,
    Reviewer,
    Publisher,
    Analyst,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoleProfile {
    pub role: AgentRole,
    pub default_capabilities: Vec<String>,
    pub default_fuel_allocation: u64,
    pub description: String,
    pub expected_inputs: Vec<String>,
    pub expected_outputs: Vec<String>,
}

impl AgentRole {
    pub fn default_profile(self) -> RoleProfile {
        match self {
            AgentRole::Researcher => RoleProfile {
                role: self,
                default_capabilities: vec!["web.search".to_string(), "web.read".to_string()],
                default_fuel_allocation: 2_000,
                description: "Collects source material and structured research notes.".to_string(),
                expected_inputs: vec!["task_brief".to_string()],
                expected_outputs: vec!["citations".to_string(), "research_summary".to_string()],
            },
            AgentRole::Writer => RoleProfile {
                role: self,
                default_capabilities: vec!["llm.query".to_string()],
                default_fuel_allocation: 2_500,
                description: "Drafts publishable content from approved research context."
                    .to_string(),
                expected_inputs: vec!["research_summary".to_string(), "style_guide".to_string()],
                expected_outputs: vec!["draft_content".to_string()],
            },
            AgentRole::Reviewer => RoleProfile {
                role: self,
                default_capabilities: vec!["llm.query".to_string(), "audit.read".to_string()],
                default_fuel_allocation: 1_500,
                description: "Checks policy, quality, and factual consistency before publishing."
                    .to_string(),
                expected_inputs: vec!["draft_content".to_string(), "policy_rules".to_string()],
                expected_outputs: vec![
                    "review_feedback".to_string(),
                    "approval_decision".to_string(),
                ],
            },
            AgentRole::Publisher => RoleProfile {
                role: self,
                default_capabilities: vec!["social.post".to_string(), "messaging.send".to_string()],
                default_fuel_allocation: 1_200,
                description: "Publishes approved content to configured channels.".to_string(),
                expected_inputs: vec![
                    "approved_content".to_string(),
                    "publish_schedule".to_string(),
                ],
                expected_outputs: vec!["publish_receipt".to_string()],
            },
            AgentRole::Analyst => RoleProfile {
                role: self,
                default_capabilities: vec!["audit.read".to_string(), "llm.query".to_string()],
                default_fuel_allocation: 1_800,
                description: "Evaluates outcome metrics and proposes strategy updates.".to_string(),
                expected_inputs: vec![
                    "engagement_metrics".to_string(),
                    "historical_reports".to_string(),
                ],
                expected_outputs: vec![
                    "performance_report".to_string(),
                    "recommendations".to_string(),
                ],
            },
        }
    }

    pub fn canonical_rank(self) -> u8 {
        match self {
            AgentRole::Researcher => 0,
            AgentRole::Writer => 1,
            AgentRole::Reviewer => 2,
            AgentRole::Publisher => 3,
            AgentRole::Analyst => 4,
        }
    }
}

pub fn canonical_pipeline_order() -> [AgentRole; 5] {
    [
        AgentRole::Researcher,
        AgentRole::Writer,
        AgentRole::Reviewer,
        AgentRole::Publisher,
        AgentRole::Analyst,
    ]
}

#[cfg(test)]
mod tests {
    use super::{canonical_pipeline_order, AgentRole};

    #[test]
    fn test_role_profile_defaults() {
        let profile = AgentRole::Researcher.default_profile();
        assert_eq!(profile.role, AgentRole::Researcher);
        assert!(profile
            .default_capabilities
            .iter()
            .any(|cap| cap == "web.search"));
        assert!(profile.default_fuel_allocation > 0);
    }

    #[test]
    fn test_role_order_is_stable() {
        let order = canonical_pipeline_order();
        assert_eq!(order[0], AgentRole::Researcher);
        assert_eq!(order[4], AgentRole::Analyst);
    }
}
