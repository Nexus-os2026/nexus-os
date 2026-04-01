//! Planner agent — read-only investigation and structured plan generation.
//!
//! The Planner operates in read-only mode: its tool registry only contains
//! file_read, search, and glob. It physically cannot modify the codebase.

use crate::tools::ToolRegistry;

/// A plan step produced by the Planner.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PlanStep {
    /// Step number (1-indexed).
    pub step: u32,
    /// What to do (human-readable).
    pub description: String,
    /// Tool to use (e.g., "file_edit", "bash").
    pub tool: String,
    /// Tool input as JSON.
    pub input: serde_json::Value,
}

/// A complete plan produced by the Planner.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Plan {
    /// Summary of what the plan accomplishes.
    pub summary: String,
    /// Ordered steps.
    pub steps: Vec<PlanStep>,
}

/// Create a read-only tool registry for the Planner.
/// The Planner can ONLY use: file_read, search, glob.
/// It CANNOT use: file_write, file_edit, bash.
pub fn planner_tool_registry() -> ToolRegistry {
    ToolRegistry::with_tools(vec![
        Box::new(crate::tools::file_read::FileReadTool),
        Box::new(crate::tools::search::SearchTool),
        Box::new(crate::tools::glob::GlobTool),
    ])
}

/// Build the Planner's system prompt.
/// Instructs the LLM to investigate the codebase and produce a structured plan
/// WITHOUT making any changes.
pub fn planner_system_prompt(base: &str, task: &str) -> String {
    format!(
        "{}\n\n## Your Role: PLANNER (Read-Only)\n\n\
         You are the PLANNER. Your job is to investigate the codebase and produce a structured plan.\n\n\
         CRITICAL RULES:\n\
         - You can ONLY read files, search, and glob. You CANNOT write, edit, or execute commands.\n\
         - Investigate thoroughly before planning. Read relevant files. Search for related code.\n\
         - Your final response MUST be a JSON plan in this exact format:\n\n\
         ```json\n\
         {{\n\
           \"summary\": \"Brief description of what this plan accomplishes\",\n\
           \"steps\": [\n\
             {{\"step\": 1, \"description\": \"What to do\", \"tool\": \"file_edit\", \"input\": {{...}}}},\n\
             {{\"step\": 2, \"description\": \"What to do\", \"tool\": \"bash\", \"input\": {{...}}}}\n\
           ]\n\
         }}\n\
         ```\n\n\
         TASK: {}\n",
        base, task
    )
}

/// Parse a Plan from the Planner's response text.
/// Extracts JSON from markdown code blocks if present.
pub fn parse_plan(response: &str) -> Result<Plan, crate::error::NxError> {
    // Try to extract JSON from ```json ... ``` block
    let json_str = if let Some(start) = response.find("```json") {
        let after_marker = &response[start + 7..];
        if let Some(end) = after_marker.find("```") {
            after_marker[..end].trim()
        } else {
            response.trim()
        }
    } else if let Some(start) = response.find('{') {
        // Try to find raw JSON
        if let Some(end) = response.rfind('}') {
            &response[start..=end]
        } else {
            response.trim()
        }
    } else {
        response.trim()
    };

    serde_json::from_str(json_str).map_err(|e| {
        let preview = if response.len() > 200 {
            &response[..200]
        } else {
            response
        };
        crate::error::NxError::ConfigError(format!(
            "Failed to parse plan: {}. Response preview: {}",
            e, preview
        ))
    })
}
