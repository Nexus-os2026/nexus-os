use crate::scanner::ProjectMap;
use nexus_kernel::errors::AgentError;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_FILES_IN_CONTEXT: usize = 32;
const MAX_FILE_CHARS: usize = 10_000;
const RECENT_WINDOW_SECS: u64 = 60 * 60 * 24 * 30;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextFile {
    pub path: String,
    pub relevance_score: f64,
    pub reason: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CodeContext {
    pub task_description: String,
    pub files: Vec<ContextFile>,
    pub truncated_files: usize,
    pub total_chars: usize,
}

pub fn build_context(
    project_map: &ProjectMap,
    task_description: &str,
) -> Result<CodeContext, AgentError> {
    let root = PathBuf::from(project_map.root_path.as_str());
    let keywords = task_keywords(task_description);
    let task_mentions_connector = keywords.iter().any(|keyword| keyword == "connector");
    let recent_files = recent_changed_files(root.as_path());

    let mut scored = Vec::new();
    let mut base_relevant_modules = HashSet::new();
    for entry in &project_map.file_tree {
        let mut score = 0.0_f64;
        let mut reasons = Vec::new();
        let normalized_path = normalize_rel_path(entry.path.as_str());
        let lower_path = normalized_path.to_ascii_lowercase();

        for keyword in &keywords {
            if lower_path.contains(keyword) {
                score += 6.0;
                reasons.push(format!("path matches keyword '{keyword}'"));
            }
        }

        if project_map
            .config_files
            .iter()
            .any(|path| path == &entry.path)
        {
            score += 1.5;
            reasons.push("config file".to_string());
        }
        if project_map
            .test_files
            .iter()
            .any(|path| path == &entry.path)
        {
            score += 1.2;
            reasons.push("test example".to_string());
        }
        if recent_files.contains(normalized_path.as_str()) {
            score += 2.5;
            reasons.push("recent git change".to_string());
        }

        if path_ends_with_components(
            entry.path.as_str(),
            &["connectors", "core", "src", "connector.rs"],
        ) && task_mentions_connector
        {
            score += 12.0;
            reasons.push("core connector trait".to_string());
        }
        if lower_path.contains("connector") && task_mentions_connector {
            score += 4.0;
            reasons.push("connector-related file".to_string());
        }

        let age_secs = current_unix_timestamp().saturating_sub(entry.last_modified);
        if age_secs <= RECENT_WINDOW_SECS {
            score += 0.8;
            reasons.push("recently modified".to_string());
        }

        if score > 5.0 {
            if let Some(module_name) = module_name(entry.path.as_str()) {
                base_relevant_modules.insert(module_name);
            }
        }
        if score > 0.0 {
            scored.push((entry.path.clone(), score, reasons.join(", ")));
        }
    }

    let import_bonus = compute_import_bonus(project_map, base_relevant_modules);
    for (path, bonus) in import_bonus {
        if let Some((_, score, reasons)) = scored
            .iter_mut()
            .find(|(candidate, _, _)| candidate == &path)
        {
            *score += bonus;
            if bonus > 0.0 {
                reasons.push_str(", import-graph proximity");
            }
        } else if bonus > 0.0 {
            scored.push((path, bonus, "import-graph proximity".to_string()));
        }
    }

    // Always include foundational files, even if scores are low.
    for path in always_include(project_map)? {
        if scored.iter().all(|(candidate, _, _)| candidate != &path) {
            scored.push((path, 1.0, "always-include baseline".to_string()));
        }
    }

    scored.sort_by(|left, right| {
        right
            .1
            .partial_cmp(&left.1)
            .unwrap_or(Ordering::Equal)
            .then(left.0.cmp(&right.0))
    });

    let mut files = Vec::new();
    let mut truncated_files = 0_usize;
    let mut total_chars = 0_usize;
    for (path, score, reason) in scored.into_iter().take(MAX_FILES_IN_CONTEXT) {
        let full_path = root.join(path.as_str());
        let Ok(content) = fs::read_to_string(full_path) else {
            continue;
        };
        let (selected, truncated) = select_relevant_sections(content.as_str(), keywords.as_slice());
        if truncated {
            truncated_files += 1;
        }
        total_chars += selected.chars().count();
        files.push(ContextFile {
            path,
            relevance_score: score,
            reason,
            content: selected,
        });
    }

    Ok(CodeContext {
        task_description: task_description.to_string(),
        files,
        truncated_files,
        total_chars,
    })
}

fn task_keywords(task: &str) -> Vec<String> {
    let mut keywords = task
        .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
        .map(|part| part.trim().to_ascii_lowercase())
        .filter(|part| part.len() >= 3)
        .collect::<Vec<_>>();
    keywords.sort();
    keywords.dedup();
    keywords
}

fn module_name(path: &str) -> Option<String> {
    let file_name = Path::new(path).file_stem()?.to_string_lossy().to_string();
    if file_name.is_empty() {
        return None;
    }
    Some(file_name)
}

fn compute_import_bonus(
    project_map: &ProjectMap,
    modules: HashSet<String>,
) -> HashMap<String, f64> {
    let mut bonuses = HashMap::new();
    if modules.is_empty() {
        return bonuses;
    }

    let root = PathBuf::from(project_map.root_path.as_str());
    for entry in &project_map.file_tree {
        let full_path = root.join(entry.path.as_str());
        let Ok(content) = fs::read_to_string(full_path) else {
            continue;
        };
        let lower = content.to_ascii_lowercase();
        let mut hit_count = 0_u32;
        for module in &modules {
            if lower.contains(module.as_str()) {
                hit_count += 1;
            }
        }
        if hit_count > 0 {
            bonuses.insert(entry.path.clone(), f64::from(hit_count) * 0.9);
        }
    }
    bonuses
}

fn always_include(project_map: &ProjectMap) -> Result<Vec<String>, AgentError> {
    let root = PathBuf::from(project_map.root_path.as_str());
    let mut required = HashSet::new();

    for config in &project_map.config_files {
        required.insert(config.clone());
    }
    for test in &project_map.test_files {
        if required.len() >= MAX_FILES_IN_CONTEXT {
            break;
        }
        required.insert(test.clone());
    }

    for entry in &project_map.file_tree {
        if required.len() >= MAX_FILES_IN_CONTEXT {
            break;
        }
        let full_path = root.join(entry.path.as_str());
        let Ok(content) = fs::read_to_string(full_path) else {
            continue;
        };
        if is_type_definition_file(entry.path.as_str(), content.as_str()) {
            required.insert(entry.path.clone());
        }
    }

    let mut ordered = required.into_iter().collect::<Vec<_>>();
    ordered.sort();
    Ok(ordered)
}

fn is_type_definition_file(path: &str, content: &str) -> bool {
    let lower_path = path.to_ascii_lowercase();
    if lower_path.contains("types")
        || lower_path.ends_with("lib.rs")
        || lower_path.ends_with("mod.rs")
        || lower_path.ends_with(".d.ts")
        || lower_path.contains("schema")
    {
        return true;
    }

    let trimmed = content.to_ascii_lowercase();
    trimmed.contains("trait ") || trimmed.contains("interface ") || trimmed.contains("type ")
}

fn select_relevant_sections(content: &str, keywords: &[String]) -> (String, bool) {
    if content.chars().count() <= MAX_FILE_CHARS {
        return (content.to_string(), false);
    }

    let lines = content.lines().collect::<Vec<_>>();
    if keywords.is_empty() {
        return (truncate_chars(content, MAX_FILE_CHARS), true);
    }

    let mut selected_ranges = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let lower = line.to_ascii_lowercase();
        if keywords.iter().any(|keyword| lower.contains(keyword)) {
            let start = index.saturating_sub(6);
            let end = (index + 6).min(lines.len().saturating_sub(1));
            selected_ranges.push((start, end));
        }
    }

    if selected_ranges.is_empty() {
        return (truncate_chars(content, MAX_FILE_CHARS), true);
    }

    selected_ranges.sort_by(|left, right| left.0.cmp(&right.0));
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for (start, end) in selected_ranges {
        if let Some(last) = merged.last_mut() {
            if start <= last.1 + 1 {
                last.1 = last.1.max(end);
            } else {
                merged.push((start, end));
            }
        } else {
            merged.push((start, end));
        }
    }

    let mut chunks = Vec::new();
    for (start, end) in merged {
        chunks.push(lines[start..=end].join("\n"));
    }
    let selected = chunks.join("\n...\n");
    if selected.chars().count() <= MAX_FILE_CHARS {
        return (selected, true);
    }
    (truncate_chars(selected.as_str(), MAX_FILE_CHARS), true)
}

fn truncate_chars(input: &str, max_chars: usize) -> String {
    let mut output = String::with_capacity(max_chars.min(input.len()));
    for (index, ch) in input.chars().enumerate() {
        if index >= max_chars {
            break;
        }
        output.push(ch);
    }
    output
}

fn recent_changed_files(root: &Path) -> HashSet<String> {
    let output = Command::new("git")
        .args(["log", "--name-only", "--pretty=format:", "-n", "30"])
        .current_dir(root)
        .output();
    let Ok(output) = output else {
        return HashSet::new();
    };
    if !output.status.success() {
        return HashSet::new();
    }

    let text = String::from_utf8_lossy(&output.stdout);
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(normalize_rel_path)
        .collect::<HashSet<_>>()
}

fn current_unix_timestamp() -> u64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_secs(),
        Err(_) => 0,
    }
}

fn normalize_rel_path(path: &str) -> String {
    Path::new(path)
        .components()
        .filter_map(|component| match component {
            Component::Normal(name) => Some(name.to_string_lossy().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn path_ends_with_components(path: &str, suffix: &[&str]) -> bool {
    let components = Path::new(path)
        .components()
        .filter_map(|component| match component {
            Component::Normal(name) => Some(name.to_string_lossy().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>();

    if components.len() < suffix.len() {
        return false;
    }

    components[components.len() - suffix.len()..]
        .iter()
        .map(String::as_str)
        .eq(suffix.iter().copied())
}
