//! Glob tool — find files matching a glob pattern.

use super::{NxTool, ToolContext, ToolResult};
use async_trait::async_trait;
use serde_json::json;

/// Find files matching a glob pattern.
pub struct GlobTool;

#[async_trait]
impl NxTool for GlobTool {
    fn name(&self) -> &str {
        "glob"
    }

    fn description(&self) -> &str {
        "Find files matching a glob pattern (e.g., '**/*.rs', 'src/**/*.ts'). \
         Returns matching file paths relative to the working directory."
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Glob pattern to match files (e.g., '**/*.rs')"
                },
                "path": {
                    "type": "string",
                    "description": "Base directory for the glob (default: working directory)"
                },
                "max_results": {
                    "type": "integer",
                    "description": "Maximum number of results (default: 100)"
                }
            },
            "required": ["pattern"]
        })
    }

    fn estimated_fuel(&self, _input: &serde_json::Value) -> u64 {
        5
    }

    async fn execute(&self, input: serde_json::Value, ctx: &ToolContext) -> ToolResult {
        let pattern = match input.get("pattern").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return ToolResult::error("Missing required parameter: pattern"),
        };

        let base_dir = input
            .get("path")
            .and_then(|v| v.as_str())
            .map(|p| ctx.resolve_path(p))
            .unwrap_or_else(|| ctx.working_dir.clone());

        let max_results = input
            .get("max_results")
            .and_then(|v| v.as_u64())
            .unwrap_or(100) as usize;

        let full_pattern = base_dir.join(pattern);
        let pattern_str = full_pattern.to_string_lossy().to_string();

        match glob::glob(&pattern_str) {
            Ok(paths) => {
                let mut results: Vec<String> = Vec::new();
                for entry in paths {
                    if results.len() >= max_results {
                        break;
                    }
                    match entry {
                        Ok(path) => {
                            let display = path
                                .strip_prefix(&ctx.working_dir)
                                .unwrap_or(&path)
                                .to_string_lossy()
                                .to_string();
                            results.push(display);
                        }
                        Err(e) => {
                            tracing::debug!("Glob entry error: {}", e);
                        }
                    }
                }

                if results.is_empty() {
                    ToolResult::success(format!("No files match '{}'", pattern))
                } else {
                    ToolResult::success(format!(
                        "{} file{} found:\n{}",
                        results.len(),
                        if results.len() == 1 { "" } else { "s" },
                        results.join("\n")
                    ))
                }
            }
            Err(e) => ToolResult::error(format!("Invalid glob pattern '{}': {}", pattern, e)),
        }
    }
}
