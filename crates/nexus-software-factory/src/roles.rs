use serde::{Deserialize, Serialize};

/// Roles in the autonomous software factory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FactoryRole {
    ProductManager,
    Architect,
    Developer,
    QualityAssurance,
    DevOps,
}

impl FactoryRole {
    pub fn output_artifact(&self) -> &str {
        match self {
            Self::ProductManager => "requirements_spec",
            Self::Architect => "architecture_doc",
            Self::Developer => "source_code",
            Self::QualityAssurance => "test_results",
            Self::DevOps => "deployment_manifest",
        }
    }

    pub fn suggested_capabilities(&self) -> Vec<&str> {
        match self {
            Self::ProductManager => vec!["reasoning", "communication", "requirements"],
            Self::Architect => vec!["architecture", "design", "planning"],
            Self::Developer => vec!["coding", "implementation", "debugging"],
            Self::QualityAssurance => vec!["testing", "quality", "verification"],
            Self::DevOps => vec!["deployment", "infrastructure", "monitoring"],
        }
    }

    pub fn min_autonomy(&self) -> u8 {
        match self {
            Self::ProductManager => 3,
            Self::Architect => 4,
            Self::Developer => 3,
            Self::QualityAssurance => 3,
            Self::DevOps => 5,
        }
    }

    pub fn display_name(&self) -> &str {
        match self {
            Self::ProductManager => "Product Manager",
            Self::Architect => "Architect",
            Self::Developer => "Developer",
            Self::QualityAssurance => "QA Engineer",
            Self::DevOps => "DevOps Engineer",
        }
    }
}

/// A team member assigned to a factory role.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamMember {
    pub agent_id: String,
    pub agent_name: String,
    pub role: FactoryRole,
    pub autonomy_level: u8,
    pub capability_score: Option<f64>,
    pub assigned_at: u64,
}
