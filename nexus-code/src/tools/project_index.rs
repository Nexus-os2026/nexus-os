//! ProjectIndexTool — scan project structure, file types, and definitions.

use super::{NxTool, ToolContext, ToolResult};
use async_trait::async_trait;
use serde_json::json;

/// Scan and report project structure.
pub struct ProjectIndexTool;

#[async_trait]
impl NxTool for ProjectIndexTool {
    fn name(&self) -> &str {
        "project_index"
    }

    fn description(&self) -> &str {
        "Scan the project and report its structure: directory tree, file type counts, \
         detected language/framework, and optionally function/struct/class definitions. \
         Use this to understand a project before making changes."
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "depth": {
                    "type": "integer",
                    "description": "Directory tree depth (default: 3, max: 5)"
                },
                "include_definitions": {
                    "type": "boolean",
                    "description": "Include function/struct/class definitions (default: false)"
                },
                "path": {
                    "type": "string",
                    "description": "Subdirectory to index (default: project root)"
                }
            }
        })
    }

    fn estimated_fuel(&self, _input: &serde_json::Value) -> u64 {
        15
    }

    async fn execute(&self, input: serde_json::Value, ctx: &ToolContext) -> ToolResult {
        let depth = input
            .get("depth")
            .and_then(|v| v.as_u64())
            .unwrap_or(3)
            .min(5) as usize;
        let include_defs = input
            .get("include_definitions")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let base_path = input
            .get("path")
            .and_then(|v| v.as_str())
            .map(|p| ctx.resolve_path(p))
            .unwrap_or_else(|| ctx.working_dir.clone());

        let mut output = String::new();

        output.push_str("## Project Structure\n\n");
        output.push_str(&build_tree(&base_path, depth, 0));
        output.push('\n');

        output.push_str("## File Types\n\n");
        let stats = count_file_types(&base_path);
        for (ext, count) in &stats {
            output.push_str(&format!("  .{}: {} files\n", ext, count));
        }
        output.push('\n');

        output.push_str("## Detected Stack\n\n");
        if base_path.join("Cargo.toml").exists() {
            output.push_str("  Language: Rust\n");
        }
        if base_path.join("package.json").exists() {
            output.push_str("  Language: JavaScript/TypeScript\n");
        }
        if base_path.join("pyproject.toml").exists() || base_path.join("setup.py").exists() {
            output.push_str("  Language: Python\n");
        }
        if base_path.join("go.mod").exists() {
            output.push_str("  Language: Go\n");
        }
        output.push('\n');

        if include_defs {
            output.push_str("## Key Definitions\n\n");
            let defs = extract_definitions(&base_path);
            for def in defs.iter().take(100) {
                output.push_str(&format!("  {}\n", def));
            }
            if defs.len() > 100 {
                output.push_str(&format!("  ... and {} more\n", defs.len() - 100));
            }
        }

        ToolResult::success(output)
    }
}

/// Build a directory tree string.
pub fn build_tree(path: &std::path::Path, max_depth: usize, current_depth: usize) -> String {
    if current_depth >= max_depth {
        return String::new();
    }

    let mut entries: Vec<_> = std::fs::read_dir(path)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            !name.starts_with('.')
                && name != "node_modules"
                && name != "target"
                && name != "__pycache__"
        })
        .collect();
    entries.sort_by_key(|e| e.file_name());

    let mut result = String::new();
    let indent = "  ".repeat(current_depth);

    for entry in &entries {
        let name = entry.file_name().to_string_lossy().to_string();
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);

        if is_dir {
            result.push_str(&format!("{}\u{1f4c1} {}/\n", indent, name));
            result.push_str(&build_tree(&entry.path(), max_depth, current_depth + 1));
        } else {
            result.push_str(&format!("{}  {}\n", indent, name));
        }
    }

    result
}

/// Count files by extension.
pub fn count_file_types(path: &std::path::Path) -> Vec<(String, usize)> {
    let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    walk_for_types(path, &mut counts);
    let mut sorted: Vec<_> = counts.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));
    sorted.into_iter().take(15).collect()
}

fn walk_for_types(path: &std::path::Path, counts: &mut std::collections::HashMap<String, usize>) {
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') || name == "target" || name == "node_modules" {
                continue;
            }
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                walk_for_types(&entry.path(), counts);
            } else if let Some(ext) = entry.path().extension() {
                *counts.entry(ext.to_string_lossy().to_string()).or_insert(0) += 1;
            }
        }
    }
}

/// Extract function/struct/class definitions.
pub fn extract_definitions(path: &std::path::Path) -> Vec<String> {
    let mut defs = Vec::new();
    walk_for_defs(path, &mut defs);
    defs
}

fn walk_for_defs(path: &std::path::Path, defs: &mut Vec<String>) {
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') || name == "target" || name == "node_modules" {
                continue;
            }
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                walk_for_defs(&entry.path(), defs);
            } else {
                let ext = entry
                    .path()
                    .extension()
                    .map(|e| e.to_string_lossy().to_string())
                    .unwrap_or_default();
                if matches!(ext.as_str(), "rs" | "ts" | "js" | "py" | "go") {
                    if let Ok(content) = std::fs::read_to_string(entry.path()) {
                        let file_name = entry.path().to_string_lossy().to_string();
                        for (i, line) in content.lines().enumerate() {
                            let trimmed = line.trim();
                            let is_def = match ext.as_str() {
                                "rs" => {
                                    trimmed.starts_with("pub fn ")
                                        || trimmed.starts_with("pub struct ")
                                        || trimmed.starts_with("pub enum ")
                                        || trimmed.starts_with("pub trait ")
                                }
                                "py" => {
                                    trimmed.starts_with("def ") || trimmed.starts_with("class ")
                                }
                                "go" => {
                                    trimmed.starts_with("func ") || trimmed.starts_with("type ")
                                }
                                _ => false,
                            };
                            if is_def {
                                let display = if trimmed.len() > 80 {
                                    &trimmed[..80]
                                } else {
                                    trimmed
                                };
                                defs.push(format!("{}:{}: {}", file_name, i + 1, display));
                            }
                        }
                    }
                }
            }
        }
    }
}
