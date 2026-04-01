//! FileWrite tool — create or overwrite a file.

use super::{NxTool, ToolContext, ToolResult};
use async_trait::async_trait;
use serde_json::json;

/// Create or overwrite a file with the given content.
pub struct FileWriteTool;

#[async_trait]
impl NxTool for FileWriteTool {
    fn name(&self) -> &str {
        "file_write"
    }

    fn description(&self) -> &str {
        "Create a new file or overwrite an existing file with the provided content. \
         Parent directories are created automatically."
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "File path (relative to working directory or absolute)"
                },
                "content": {
                    "type": "string",
                    "description": "The content to write to the file"
                }
            },
            "required": ["path", "content"]
        })
    }

    fn estimated_fuel(&self, _input: &serde_json::Value) -> u64 {
        10
    }

    async fn execute(&self, input: serde_json::Value, ctx: &ToolContext) -> ToolResult {
        let path_str = match input.get("path").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return ToolResult::error("Missing required parameter: path"),
        };
        let content = match input.get("content").and_then(|v| v.as_str()) {
            Some(c) => c,
            None => return ToolResult::error("Missing required parameter: content"),
        };

        let path = ctx.resolve_path(path_str);

        if let Err(e) = ctx.check_path_allowed(&path) {
            return ToolResult::error(format!("{}", e));
        }

        // Check if the file already exists (for reporting)
        let existed = path.exists();

        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                if let Err(e) = tokio::fs::create_dir_all(parent).await {
                    return ToolResult::error(format!("Failed to create directories: {}", e));
                }
            }
        }

        // Write file
        match tokio::fs::write(&path, content).await {
            Ok(()) => {
                let line_count = content.lines().count();
                let verb = if existed { "Overwrote" } else { "Created" };
                ToolResult::success(format!(
                    "{} '{}' ({} bytes, {} lines)",
                    verb,
                    path_str,
                    content.len(),
                    line_count
                ))
            }
            Err(e) => ToolResult::error(format!("Failed to write '{}': {}", path_str, e)),
        }
    }
}
