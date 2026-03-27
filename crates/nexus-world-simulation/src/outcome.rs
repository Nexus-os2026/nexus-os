use serde::{Deserialize, Serialize};

/// The complete result of a simulation run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationResult {
    pub scenario_id: String,
    pub success: bool,
    pub step_results: Vec<StepResult>,
    pub risk_assessment: RiskAssessment,
    pub duration_ms: u64,
    pub recommendation: Recommendation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepResult {
    pub step: u32,
    pub success: bool,
    pub output: String,
    pub side_effects: Vec<SideEffect>,
    pub risk: StepRisk,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SideEffect {
    FileCreated {
        path: String,
    },
    FileModified {
        path: String,
    },
    FileDeleted {
        path: String,
    },
    DataLoss {
        description: String,
    },
    ServiceDisruption {
        service: String,
        duration_estimate: String,
    },
    NetworkCall {
        url: String,
    },
    AgentSpawned {
        agent_id: String,
    },
    StateChange {
        component: String,
        from: String,
        to: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepRisk {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskAssessment {
    pub level: RiskLevel,
    pub score: f64,
    pub factors: Vec<String>,
    pub mitigations: Vec<String>,
}

impl RiskAssessment {
    pub fn low(note: &str) -> Self {
        Self {
            level: RiskLevel::Low,
            score: 0.1,
            factors: vec![note.into()],
            mitigations: Vec::new(),
        }
    }
    pub fn medium(note: &str) -> Self {
        Self {
            level: RiskLevel::Medium,
            score: 0.5,
            factors: vec![note.into()],
            mitigations: Vec::new(),
        }
    }
    pub fn high(note: &str) -> Self {
        Self {
            level: RiskLevel::High,
            score: 0.8,
            factors: vec![note.into()],
            mitigations: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Recommendation {
    Proceed {
        confidence: f64,
        note: String,
    },
    ProceedWithCaution {
        confidence: f64,
        precautions: Vec<String>,
    },
    NeedsReview {
        reason: String,
    },
    DoNotProceed {
        reason: String,
    },
}
