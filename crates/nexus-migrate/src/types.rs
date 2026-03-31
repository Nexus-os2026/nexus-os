use serde::{Deserialize, Serialize};
use std::fmt;

/// Source framework being migrated from.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SourceFramework {
    CrewAI,
    LangGraph,
}

impl fmt::Display for SourceFramework {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CrewAI => write!(f, "CrewAI"),
            Self::LangGraph => write!(f, "LangGraph"),
        }
    }
}

/// Result of a migration operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationResult {
    pub source_framework: SourceFramework,
    pub agents_converted: Vec<ConvertedAgent>,
    pub tasks_converted: Vec<ConvertedTask>,
    pub workflows_converted: Vec<ConvertedWorkflow>,
    pub warnings: Vec<MigrationWarning>,
    pub errors: Vec<MigrationError>,
    pub summary: MigrationSummary,
}

/// A converted agent ready for Nexus OS.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvertedAgent {
    pub original_name: String,
    pub nexus_agent_id: String,
    pub role: String,
    pub goal: String,
    pub backstory: String,
    pub autonomy_level: u8,
    pub capabilities: Vec<String>,
    pub llm_provider: Option<String>,
    pub llm_model: Option<String>,
    pub tools: Vec<ConvertedTool>,
    /// Full Nexus OS agent genome as JSON.
    pub genome: serde_json::Value,
}

/// A converted task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvertedTask {
    pub original_name: String,
    pub nexus_task_id: String,
    pub description: String,
    pub expected_output: Option<String>,
    pub assigned_agent: Option<String>,
    /// Other task IDs this depends on.
    pub dependencies: Vec<String>,
}

/// A converted workflow (from LangGraph graphs).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvertedWorkflow {
    pub original_name: String,
    pub nexus_workflow_id: String,
    pub nodes: Vec<WorkflowNode>,
    pub edges: Vec<WorkflowEdge>,
    pub entry_point: String,
    pub state_schema: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowNode {
    pub id: String,
    pub node_type: WorkflowNodeType,
    pub agent_id: Option<String>,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum WorkflowNodeType {
    AgentExecution,
    ToolCall,
    ConditionalBranch,
    HumanReview,
    Start,
    End,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowEdge {
    pub from_node: String,
    pub to_node: String,
    pub condition: Option<String>,
}

/// Tool mapping from source framework to Nexus OS capability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvertedTool {
    pub original_name: String,
    pub nexus_capability: String,
    /// `true` if we found a Nexus OS equivalent, `false` if unmapped.
    pub mapped: bool,
    pub notes: Option<String>,
}

/// Warning about something that was converted but might need attention.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationWarning {
    pub item: String,
    pub message: String,
    pub suggestion: String,
}

/// Error about something that could not be converted.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationError {
    pub item: String,
    pub message: String,
    pub original_content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationSummary {
    pub source_framework: SourceFramework,
    pub total_agents_found: usize,
    pub agents_converted: usize,
    pub total_tasks_found: usize,
    pub tasks_converted: usize,
    pub total_workflows_found: usize,
    pub workflows_converted: usize,
    pub warnings_count: usize,
    pub errors_count: usize,
}

/// Complete output from a migration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationOutput {
    pub agents: Vec<serde_json::Value>,
    pub workflows: Vec<serde_json::Value>,
    pub report: String,
}

#[derive(Debug, thiserror::Error)]
pub enum MigrateError {
    #[error("Failed to parse YAML: {0}")]
    YamlParse(String),
    #[error("Failed to parse Python source: {0}")]
    PythonParse(String),
    #[error("Invalid source format: {0}")]
    InvalidFormat(String),
    #[error("IO error: {0}")]
    Io(String),
}

impl Serialize for MigrateError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}
