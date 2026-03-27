use serde::{Deserialize, Serialize};

use crate::pipeline::PipelineStage;

/// An artifact produced by a pipeline stage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectArtifact {
    pub id: String,
    pub project_id: String,
    pub artifact_type: String,
    pub stage: PipelineStage,
    pub produced_by: String,
    pub content: ArtifactContent,
    pub quality_score: Option<f64>,
    pub created_at: u64,
    pub version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ArtifactContent {
    Requirements {
        user_stories: Vec<String>,
        acceptance_criteria: Vec<String>,
        constraints: Vec<String>,
        priorities: Vec<(String, Priority)>,
    },
    Architecture {
        components: Vec<Component>,
        interactions: Vec<String>,
        technology_choices: Vec<(String, String)>,
        risks: Vec<String>,
    },
    SourceCode {
        files: Vec<CodeFile>,
        language: String,
        framework: Option<String>,
        entry_point: Option<String>,
    },
    TestResults {
        total: u32,
        passed: u32,
        failed: u32,
        skipped: u32,
        coverage_percent: Option<f64>,
        failures: Vec<TestFailure>,
    },
    ReviewOutcome {
        approved: bool,
        comments: Vec<ReviewComment>,
        requested_changes: Vec<String>,
    },
    DeploymentResult {
        success: bool,
        environment: String,
        url: Option<String>,
        logs: String,
    },
    VerificationReport {
        all_checks_passed: bool,
        checks: Vec<VerificationCheck>,
    },
    Text(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Priority {
    Critical,
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Component {
    pub name: String,
    pub description: String,
    pub responsibility: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeFile {
    pub path: String,
    pub content: String,
    pub language: String,
    pub lines: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestFailure {
    pub test_name: String,
    pub expected: String,
    pub actual: String,
    pub error_message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewComment {
    pub reviewer: String,
    pub file: Option<String>,
    pub line: Option<u32>,
    pub comment: String,
    pub severity: ReviewSeverity,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReviewSeverity {
    Blocker,
    Major,
    Minor,
    Suggestion,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationCheck {
    pub name: String,
    pub passed: bool,
    pub details: String,
}
