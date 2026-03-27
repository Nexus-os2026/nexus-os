use serde::{Deserialize, Serialize};

use crate::roles::FactoryRole;

/// The stages of the software development pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum PipelineStage {
    Requirements,
    Architecture,
    Implementation,
    Testing,
    Review,
    Deployment,
    Verification,
}

impl PipelineStage {
    pub fn responsible_role(&self) -> FactoryRole {
        match self {
            Self::Requirements => FactoryRole::ProductManager,
            Self::Architecture => FactoryRole::Architect,
            Self::Implementation => FactoryRole::Developer,
            Self::Testing => FactoryRole::QualityAssurance,
            Self::Review => FactoryRole::ProductManager,
            Self::Deployment => FactoryRole::DevOps,
            Self::Verification => FactoryRole::QualityAssurance,
        }
    }

    pub fn output_artifact(&self) -> &str {
        match self {
            Self::Requirements => "requirements_spec",
            Self::Architecture => "architecture_doc",
            Self::Implementation => "source_code",
            Self::Testing => "test_results",
            Self::Review => "review_outcome",
            Self::Deployment => "deployment_result",
            Self::Verification => "verification_report",
        }
    }

    pub fn next(&self) -> Option<PipelineStage> {
        match self {
            Self::Requirements => Some(Self::Architecture),
            Self::Architecture => Some(Self::Implementation),
            Self::Implementation => Some(Self::Testing),
            Self::Testing => Some(Self::Review),
            Self::Review => Some(Self::Deployment),
            Self::Deployment => Some(Self::Verification),
            Self::Verification => None,
        }
    }

    pub fn previous(&self) -> Option<PipelineStage> {
        match self {
            Self::Requirements => None,
            Self::Architecture => Some(Self::Requirements),
            Self::Implementation => Some(Self::Architecture),
            Self::Testing => Some(Self::Implementation),
            Self::Review => Some(Self::Testing),
            Self::Deployment => Some(Self::Review),
            Self::Verification => Some(Self::Deployment),
        }
    }

    pub fn base_cost(&self) -> u64 {
        match self {
            Self::Requirements => 5_000_000,
            Self::Architecture => 10_000_000,
            Self::Implementation => 20_000_000,
            Self::Testing => 15_000_000,
            Self::Review => 5_000_000,
            Self::Deployment => 10_000_000,
            Self::Verification => 5_000_000,
        }
    }

    pub fn all() -> Vec<PipelineStage> {
        vec![
            Self::Requirements,
            Self::Architecture,
            Self::Implementation,
            Self::Testing,
            Self::Review,
            Self::Deployment,
            Self::Verification,
        ]
    }

    pub fn display_name(&self) -> &str {
        match self {
            Self::Requirements => "Requirements",
            Self::Architecture => "Architecture",
            Self::Implementation => "Implementation",
            Self::Testing => "Testing",
            Self::Review => "Review",
            Self::Deployment => "Deployment",
            Self::Verification => "Verification",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pipeline_stage_order() {
        assert!(PipelineStage::Requirements < PipelineStage::Architecture);
        assert!(PipelineStage::Architecture < PipelineStage::Implementation);
        assert!(PipelineStage::Deployment < PipelineStage::Verification);
    }

    #[test]
    fn test_pipeline_stage_next() {
        assert_eq!(
            PipelineStage::Requirements.next(),
            Some(PipelineStage::Architecture)
        );
        assert_eq!(
            PipelineStage::Deployment.next(),
            Some(PipelineStage::Verification)
        );
        assert_eq!(PipelineStage::Verification.next(), None);
    }
}
