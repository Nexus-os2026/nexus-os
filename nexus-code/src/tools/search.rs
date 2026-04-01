//! Search tool — code search using ripgrep (falls back to grep).

use super::{NxTool, ToolContext, ToolResult};
use async_trait::async_trait;
use serde_json::json;

/// Search for a pattern in files using ripgrep (rg) or grep.
pub struct SearchTool;

/// Check if a command exists on PATH.
fn which_exists(cmd: &str) -> bool {
    std::process::Command::new("which")
        .arg(cmd)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[async_trait]
impl NxTool for SearchTool {
    fn name(&self) -> &str {
        "search"
    }

    fn description(&self) -> &str {
        "Search for a text pattern or regex in files. Uses ripgrep (rg) if \
         available, falls back to grep. Supports file type filtering via \
         'include' glob."
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "The search pattern (regex supported)"
                },
                "path": {
                    "type": "string",
                    "description": "Directory or file to search in (default: working directory)"
                },
                "include": {
                    "type": "string",
                    "description": "File glob pattern to include (e.g., '*.rs', '*.py')"
                },
                "max_results": {
                    "type": "integer",
                    "description": "Maximum number of matching lines (default: 50)"
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

        let search_path = input
            .get("path")
            .and_then(|v| v.as_str())
            .map(|p| ctx.resolve_path(p))
            .unwrap_or_else(|| ctx.working_dir.clone());

        let max_results = input
            .get("max_results")
            .and_then(|v| v.as_u64())
            .unwrap_or(50);

        let include_glob = input.get("include").and_then(|v| v.as_str());

        // Build command: try rg first, fall back to grep
        let mut cmd = if which_exists("rg") {
            let mut c = tokio::process::Command::new("rg");
            c.arg("--line-number")
                .arg("--no-heading")
                .arg(format!("--max-count={}", max_results));
            if let Some(glob) = include_glob {
                c.arg(format!("--glob={}", glob));
            }
            c.arg(pattern).arg(search_path.to_string_lossy().as_ref());
            c
        } else {
            let mut c = tokio::process::Command::new("grep");
            c.arg("-rn").arg(format!("-m{}", max_results));
            if let Some(glob) = include_glob {
                c.arg(format!("--include={}", glob));
            }
            c.arg(pattern).arg(search_path.to_string_lossy().as_ref());
            c
        };

        cmd.current_dir(&ctx.working_dir);

        match cmd.output().await {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if stdout.is_empty() {
                    ToolResult::success(format!("No matches found for '{}'", pattern))
                } else {
                    let line_count = stdout.lines().count();
                    ToolResult::success(format!(
                        "{} match{} found:\n{}",
                        line_count,
                        if line_count == 1 { "" } else { "es" },
                        stdout.trim_end()
                    ))
                }
            }
            Err(e) => ToolResult::error(format!("Search failed: {}", e)),
        }
    }
}
