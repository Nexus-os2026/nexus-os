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
        let program = if which_exists("rg") { "rg" } else { "grep" };
        execute_search(input, ctx, program).await
    }
}

// Shared by production backend selection and regressions forcing either real
// backend. Candidate authorization/privacy logic is never substituted in tests.
async fn execute_search(input: serde_json::Value, ctx: &ToolContext, program: &str) -> ToolResult {
    let pattern = match input.get("pattern").and_then(|v| v.as_str()) {
        Some(p) => p,
        None => return ToolResult::error("Missing required parameter: pattern"),
    };

    let search_path =
        match ctx.resolve_path(input.get("path").and_then(|v| v.as_str()).unwrap_or(".")) {
            Ok(path) => path,
            Err(e) => return ToolResult::from_path_error(e),
        };

    let max_results = input
        .get("max_results")
        .and_then(|v| v.as_u64())
        .unwrap_or(50);

    let files = match super::search_policy::files(ctx, &search_path) {
        Ok(files) => files,
        Err(e) => return ToolResult::from_path_error(e),
    };
    let root = match ctx.workspace_root() {
        Ok(root) => root,
        Err(e) => return ToolResult::from_path_error(e),
    };
    // Use ripgrep-style override grammar only as a final reducing filter. The
    // matcher cannot restore candidates removed by privacy or access policy.
    let mut include = ignore::overrides::OverrideBuilder::new(&root);
    if let Some(pattern) = input.get("include").and_then(|v| v.as_str()) {
        if let Err(e) = include.add(pattern) {
            return ToolResult::error(format!("Invalid include glob: {e}"));
        }
    }
    let include = match include.build() {
        Ok(include) => include,
        Err(e) => return ToolResult::error(format!("Invalid include glob: {e}")),
    };
    let files: Vec<_> = files
        .iter()
        .filter(|path| {
            !include.matched(path, false).is_ignore()
                // Preserve directory exclusions (e.g. !generated/) without
                // asking a backend to traverse or discover files.
                && path.ancestors().skip(1).take_while(|p| *p != root && p.starts_with(&root))
                    .all(|parent| !include.matched(parent, true).is_ignore())
        })
        .map(|path| {
            path.strip_prefix(&root)
                .expect("contained path")
                .to_path_buf()
        })
        .collect();
    search_files(program, &root, &files, pattern, max_results).await
}

// Neither backend is allowed to discover paths itself. Only validated regular
// files are passed, and user patterns are data rather than command-line options.
async fn search_files(
    program: &str,
    root: &std::path::Path,
    files: &[std::path::PathBuf],
    pattern: &str,
    max_results: u64,
) -> ToolResult {
    let mut lines = Vec::new();
    for batch in files.chunks(128) {
        if lines.len() as u64 >= max_results {
            break;
        }
        let mut cmd = tokio::process::Command::new(program);
        cmd.current_dir(root)
            .env_remove("RIPGREP_CONFIG_PATH")
            .env_remove("GREP_OPTIONS");
        if program == "rg" {
            // Nexus has already applied project-local ignore and binary policy.
            // Do not rediscover configuration or enable recursive traversal.
            cmd.args([
                "--no-config",
                "--no-ignore",
                "--no-follow",
                "--color=never",
                "--line-number",
                "--with-filename",
                "--no-heading",
            ])
            .arg(format!("--max-count={max_results}"));
        } else {
            cmd.args(["-nH", "--color=never"])
                .arg(format!("-m{max_results}"));
        }
        cmd.arg("-e").arg(pattern).arg("--").args(batch);
        match cmd.output().await {
            Ok(output) if output.status.success() || output.status.code() == Some(1) => {
                lines.extend(
                    String::from_utf8_lossy(&output.stdout)
                        .lines()
                        .map(str::to_owned),
                );
            }
            Ok(_) => return ToolResult::error("Search failed (invalid pattern or file I/O error)"),
            Err(e) => return ToolResult::error(format!("Search failed: {e}")),
        }
    }
    lines.truncate(usize::try_from(max_results).unwrap_or(usize::MAX));
    if lines.is_empty() {
        ToolResult::success(format!("No matches found for '{pattern}'"))
    } else {
        ToolResult::success(format!(
            "{} match{} found:\n{}",
            lines.len(),
            if lines.len() == 1 { "" } else { "es" },
            lines.join("\n")
        ))
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[tokio::test]
    async fn p0_002b_grep_backend_receives_only_contained_files_and_literal_patterns() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("work");
        let outside = temp.path().join("outside");
        std::fs::create_dir(&root).unwrap();
        std::fs::create_dir(&outside).unwrap();
        let root = root.canonicalize().unwrap();
        std::fs::write(root.join("inside.txt"), "inside marker\n-f sentinel\n").unwrap();
        std::fs::write(outside.join("secret.txt"), "outside marker\n").unwrap();
        std::os::unix::fs::symlink(&outside, root.join("escape")).unwrap();
        let ctx = ToolContext {
            working_dir: root.clone(),
            blocked_paths: vec![],
            max_file_scope: None,
            non_interactive: true,
        };
        let files = super::super::search_policy::files(&ctx, &root).unwrap();
        let files: Vec<_> = files
            .iter()
            .map(|p| p.strip_prefix(&root).unwrap().to_path_buf())
            .collect();
        assert_eq!(files, vec![std::path::PathBuf::from("inside.txt")]);
        let result = search_files("grep", &root, &files, "marker", 50).await;
        assert!(result.success && result.output.contains("inside marker"));
        assert!(!result.output.contains("outside marker"));
        let result = search_files("grep", &root, &files, "-f sentinel", 50).await;
        assert!(result.success && result.output.contains("inside.txt:2:-f sentinel"));
        assert_eq!(
            std::fs::read(outside.join("secret.txt")).unwrap(),
            b"outside marker\n"
        );
        assert_eq!(std::fs::read_dir(&outside).unwrap().count(), 1);
    }
}

#[cfg(test)]
#[path = "search_tests.rs"]
mod privacy_tests;
