//! FileEdit tool — search-and-replace with forensic content hashing.

use super::{NxTool, ToolContext, ToolResult};
use async_trait::async_trait;
use serde_json::json;
use sha2::{Digest, Sha256};

/// Edit a file by replacing a specific text occurrence.
/// The old_text must appear exactly once in the file.
/// Records the pre-edit content hash for forensic verification.
pub struct FileEditTool;

#[async_trait]
impl NxTool for FileEditTool {
    fn name(&self) -> &str {
        "file_edit"
    }

    fn description(&self) -> &str {
        "Edit a file by replacing a specific text occurrence. The old_text must \
         appear exactly once in the file (to prevent ambiguous edits). Reports a \
         pre-edit content hash for audit verification."
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "File path"
                },
                "old_text": {
                    "type": "string",
                    "description": "The exact text to find (must appear exactly once)"
                },
                "new_text": {
                    "type": "string",
                    "description": "The replacement text"
                }
            },
            "required": ["path", "old_text", "new_text"]
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
        let old_text = match input.get("old_text").and_then(|v| v.as_str()) {
            Some(t) => t,
            None => return ToolResult::error("Missing required parameter: old_text"),
        };
        let new_text = match input.get("new_text").and_then(|v| v.as_str()) {
            Some(t) => t,
            None => return ToolResult::error("Missing required parameter: new_text"),
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

        // Compute pre-edit hash for forensic verification
        let pre_hash = hex::encode(Sha256::digest(content.as_bytes()));

        // Count occurrences
        let count = content.matches(old_text).count();
        if count == 0 {
            return ToolResult::error(format!(
                "old_text not found in '{}'. The text to replace does not exist in the file.",
                path_str
            ));
        }
        if count > 1 {
            return ToolResult::error(format!(
                "old_text found {} times in '{}'. It must appear exactly once for an unambiguous edit.",
                count, path_str
            ));
        }

        // Replace
        let new_content = content.replacen(old_text, new_text, 1);

        // Compute post-edit hash
        let post_hash = hex::encode(Sha256::digest(new_content.as_bytes()));

        // Write back
        match tokio::fs::write(&path, &new_content).await {
            Ok(()) => ToolResult::success(format!(
                "Edited '{}': replaced {} chars with {} chars (pre-hash: {}..., post-hash: {}...)",
                path_str,
                old_text.len(),
                new_text.len(),
                &pre_hash[..12],
                &post_hash[..12]
            )),
            Err(e) => ToolResult::error(format!("Failed to write '{}': {}", path_str, e)),
        }
    }
}
