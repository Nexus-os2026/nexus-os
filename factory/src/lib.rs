//! Agent Factory: natural language intent to governed agent scaffolding.

pub mod approval;
pub mod capabilities;
pub mod code_gen;
pub mod intent;
pub mod manifest_gen;
pub mod notifications;
pub mod pipeline;
pub mod remote;

use crate::intent::TaskType;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuiltInAgentTemplate {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub task_type: TaskType,
}

pub fn built_in_agent_templates() -> Vec<BuiltInAgentTemplate> {
    vec![
        BuiltInAgentTemplate {
            id: "social-poster",
            name: "Social Poster Agent",
            description: "Draft, approve, and publish social content with governance controls.",
            task_type: TaskType::ContentPosting,
        },
        BuiltInAgentTemplate {
            id: "coding-agent",
            name: "Coding Agent",
            description: "Read repositories, generate changes, and validate with tests.",
            task_type: TaskType::Research,
        },
        BuiltInAgentTemplate {
            id: "self-improve-agent",
            name: "Self-Improve Agent",
            description:
                "Learns from outcomes, updates prompts/strategies, and records audited versions.",
            task_type: TaskType::SelfImprove,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::built_in_agent_templates;

    #[test]
    fn test_self_improve_template_registered() {
        let templates = built_in_agent_templates();
        assert!(templates
            .iter()
            .any(|template| template.id == "self-improve-agent"));
    }
}
