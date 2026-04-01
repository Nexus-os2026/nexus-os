//! FileRead tool — read file contents with optional line range.

use super::{NxTool, ToolContext, ToolResult};
use async_trait::async_trait;
use serde_json::json;

/// Read the contents of a file.
/// Supports optional line range (start_line, end_line) for reading specific portions.
pub struct FileReadTool;

#[async_trait]
impl NxTool for FileReadTool {
    fn name(&self) -> &str {
        "file_read"
    }

    fn description(&self) -> &str {
        "Read the contents of a file. Optionally specify a line range with \
         start_line and end_line (1-indexed, inclusive)."
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "File path (relative to working directory or absolute)"
                },
                "start_line": {
                    "type": "integer",
                    "description": "Start line (1-indexed, inclusive). Optional."
                },
                "end_line": {
                    "type": "integer",
                    "description": "End line (1-indexed, inclusive). Optional."
                }
            },
            "required": ["path"]
        })
    }

    fn estimated_fuel(&self, _input: &serde_json::Value) -> u64 {
        5
    }

    async fn execute(&self, input: serde_json::Value, ctx: &ToolContext) -> ToolResult {
        let path_str = match input.get("path").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return ToolResult::error("Missing required parameter: path"),
        };

        let path = ctx.resolve_path(path_str);

        if let Err(e) = ctx.check_path_allowed(&path) {
            return ToolResult::error(format!("{}", e));
        }

        // Read file
        let content = match tokio::fs::read_to_string(&path).await {
            Ok(c) => c,
            Err(e) => return ToolResult::error(format!("Failed to read '{}': {}", path_str, e)),
        };

        // Apply line range if specified
        let start = input
            .get("start_line")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize);
        let end = input
            .get("end_line")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize);

        let lines: Vec<&str> = content.lines().collect();
        let total_lines = lines.len();

        let output = match (start, end) {
            (Some(s), Some(e)) => {
                let s = s.saturating_sub(1); // 1-indexed -> 0-indexed
                let e = e.min(total_lines);
                if s >= total_lines {
                    return ToolResult::error(format!(
                        "start_line {} exceeds file length ({} lines)",
                        s + 1,
                        total_lines
                    ));
                }
                format!(
                    "[Lines {}-{} of {}]\n{}",
                    s + 1,
                    e,
                    total_lines,
                    lines[s..e].join("\n")
                )
            }
            (Some(s), None) => {
                let s = s.saturating_sub(1);
                if s >= total_lines {
                    return ToolResult::error(format!(
                        "start_line {} exceeds file length ({} lines)",
                        s + 1,
                        total_lines
                    ));
                }
                format!(
                    "[Lines {}-{} of {}]\n{}",
                    s + 1,
                    total_lines,
                    total_lines,
                    lines[s..].join("\n")
                )
            }
            _ => {
                format!("[{} lines]\n{}", total_lines, content)
            }
        };

        ToolResult::success(output)
    }
}
