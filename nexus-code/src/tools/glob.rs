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

        let base_dir =
            match ctx.resolve_path(input.get("path").and_then(|v| v.as_str()).unwrap_or(".")) {
                Ok(path) => path,
                Err(e) => return ToolResult::from_path_error(e),
            };

        let max_results = input
            .get("max_results")
            .and_then(|v| v.as_u64())
            .unwrap_or(100) as usize;

        let root = match ctx.workspace_root() {
            Ok(root) => root,
            Err(e) => return ToolResult::from_path_error(e),
        };
        // Patterns only match an already-contained file list; they never drive
        // filesystem traversal. Preserve absolute-inside patterns without
        // allowing a pattern to select a different authority root.
        let pattern_path = std::path::Path::new(pattern);
        let absolute_pattern = pattern_path.is_absolute();
        if pattern_path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return ToolResult::from_path_error(ToolContext::workspace_denied(
                "glob traversal is not allowed",
            ));
        }
        let (match_base, pattern_path) = if absolute_pattern {
            // Both spellings identify the same root validated by workspace_root
            // (e.g. macOS /var aliases or Windows native vs verbatim paths).
            match pattern_path
                .strip_prefix(&root)
                .or_else(|_| pattern_path.strip_prefix(&ctx.working_dir))
            {
                Ok(relative) => (root.as_path(), relative),
                Err(_) => {
                    return ToolResult::from_path_error(ToolContext::workspace_denied(
                        "glob is outside the workspace",
                    ))
                }
            }
        } else {
            if pattern_path.components().any(|c| {
                matches!(
                    c,
                    std::path::Component::Prefix(_) | std::path::Component::RootDir
                )
            }) {
                return ToolResult::from_path_error(ToolContext::workspace_denied(
                    "unsafe glob prefix",
                ));
            }
            (base_dir.as_path(), pattern_path)
        };
        let normalized: std::path::PathBuf = pattern_path
            .components()
            .filter(|c| !matches!(c, std::path::Component::CurDir))
            .collect();
        let pattern_text = if normalized.as_os_str().is_empty() {
            ".".into()
        } else {
            normalized.to_string_lossy()
        };
        match glob::Pattern::new(&pattern_text) {
            Ok(pattern) => {
                // An explicitly named symlink prefix is a requested path, not
                // an incidental directory entry to skip during enumeration.
                let mut literal_prefix = match_base.to_path_buf();
                for component in pattern_path.components() {
                    if component
                        .as_os_str()
                        .to_string_lossy()
                        .contains(['*', '?', '[', ']'])
                    {
                        break;
                    }
                    literal_prefix.push(component.as_os_str());
                }
                if let Err(e) = ctx.resolve_workspace_path(&literal_prefix) {
                    return ToolResult::from_path_error(e);
                }
                let scan_base = if absolute_pattern { &root } else { &base_dir };
                let paths = match super::traversal::paths(ctx, scan_base, false, true) {
                    Ok(paths) => paths,
                    Err(e) => return ToolResult::from_path_error(e),
                };
                let options = glob::MatchOptions {
                    require_literal_separator: true,
                    ..Default::default()
                };
                let results: Vec<String> = paths
                    .iter()
                    .filter(|path| {
                        path.strip_prefix(match_base).is_ok_and(|relative| {
                            let relative = if relative.as_os_str().is_empty() {
                                std::path::Path::new(".")
                            } else {
                                relative
                            };
                            pattern.matches_path_with(relative, options)
                        })
                    })
                    .take(max_results)
                    .map(|path| {
                        let relative = path.strip_prefix(&root).expect("contained path");
                        if relative.as_os_str().is_empty() {
                            ".".into()
                        } else {
                            relative.to_string_lossy().to_string()
                        }
                    })
                    .collect();

                if results.is_empty() {
                    ToolResult::success(format!("No files match '{}'", pattern.as_str()))
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
