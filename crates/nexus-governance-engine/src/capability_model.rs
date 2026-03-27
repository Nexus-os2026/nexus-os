//! Capability model with hierarchical permissions.

use serde::{Deserialize, Serialize};

/// A capability in the hierarchical model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capability {
    pub name: String,
    pub description: String,
    pub parent: Option<String>,
    pub risk_level: RiskLevel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

/// Default capability registry.
pub fn default_capabilities() -> Vec<Capability> {
    vec![
        Capability {
            name: "llm.query".into(),
            description: "Query LLM models".into(),
            parent: None,
            risk_level: RiskLevel::Low,
        },
        Capability {
            name: "fs.read".into(),
            description: "Read files".into(),
            parent: None,
            risk_level: RiskLevel::Low,
        },
        Capability {
            name: "fs.write".into(),
            description: "Write files".into(),
            parent: None,
            risk_level: RiskLevel::Medium,
        },
        Capability {
            name: "web.search".into(),
            description: "Search the web".into(),
            parent: None,
            risk_level: RiskLevel::Low,
        },
        Capability {
            name: "web.read".into(),
            description: "Read web pages".into(),
            parent: None,
            risk_level: RiskLevel::Low,
        },
        Capability {
            name: "process.exec".into(),
            description: "Execute system processes".into(),
            parent: None,
            risk_level: RiskLevel::High,
        },
        Capability {
            name: "social.post".into(),
            description: "Post to social media".into(),
            parent: None,
            risk_level: RiskLevel::High,
        },
        Capability {
            name: "messaging.send".into(),
            description: "Send messages".into(),
            parent: None,
            risk_level: RiskLevel::Medium,
        },
        Capability {
            name: "self.modify".into(),
            description: "Self-modification".into(),
            parent: None,
            risk_level: RiskLevel::Critical,
        },
        Capability {
            name: "agent.create".into(),
            description: "Create new agents".into(),
            parent: None,
            risk_level: RiskLevel::Critical,
        },
    ]
}
