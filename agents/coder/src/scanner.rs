use nexus_sdk::errors::AgentError;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Component, Path};
use std::process::Command;
use std::time::UNIX_EPOCH;

const DEFAULT_DEPTH_LIMIT: usize = 10;
const DEFAULT_MAX_FILE_SIZE_BYTES: u64 = 1_048_576;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Language {
    Rust,
    TypeScript,
    JavaScript,
    Python,
    Go,
    Toml,
    Json,
    Yaml,
    Markdown,
    Shell,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileEntry {
    pub path: String,
    pub size: u64,
    pub language: Language,
    pub last_modified: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitCommit {
    pub hash: String,
    pub author: String,
    pub subject: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitInfo {
    pub branch: String,
    pub recent_commits: Vec<GitCommit>,
    pub contributors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectMap {
    pub root_path: String,
    pub file_tree: Vec<FileEntry>,
    pub languages: HashMap<Language, usize>,
    pub entry_points: Vec<String>,
    pub config_files: Vec<String>,
    pub test_files: Vec<String>,
    pub total_lines: usize,
    pub git_info: Option<GitInfo>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScannerConfig {
    pub depth_limit: usize,
    pub max_file_size_bytes: u64,
}

impl Default for ScannerConfig {
    fn default() -> Self {
        Self {
            depth_limit: DEFAULT_DEPTH_LIMIT,
            max_file_size_bytes: DEFAULT_MAX_FILE_SIZE_BYTES,
        }
    }
}

#[derive(Debug, Default)]
struct ScanAccumulator {
    file_tree: Vec<FileEntry>,
    languages: HashMap<Language, usize>,
    entry_points: Vec<String>,
    config_files: Vec<String>,
    test_files: Vec<String>,
    total_lines: usize,
}

pub fn scan_project(path: impl AsRef<Path>) -> Result<ProjectMap, AgentError> {
    scan_project_with_config(path, ScannerConfig::default())
}

pub fn scan_project_with_config(
    path: impl AsRef<Path>,
    config: ScannerConfig,
) -> Result<ProjectMap, AgentError> {
    let root = path.as_ref();
    if !root.exists() {
        return Err(AgentError::SupervisorError(format!(
            "project path '{}' does not exist",
            root.display()
        )));
    }

    let mut state = ScanAccumulator::default();
    scan_dir(root, root, 0, config, &mut state)?;
    state
        .file_tree
        .sort_by(|left, right| left.path.cmp(&right.path));
    state.entry_points.sort();
    state.entry_points.dedup();
    state.config_files.sort();
    state.config_files.dedup();
    state.test_files.sort();
    state.test_files.dedup();

    Ok(ProjectMap {
        root_path: root.to_string_lossy().to_string(),
        file_tree: state.file_tree,
        languages: state.languages,
        entry_points: state.entry_points,
        config_files: state.config_files,
        test_files: state.test_files,
        total_lines: state.total_lines,
        git_info: collect_git_info(root),
    })
}

pub fn detect_language(path: &Path, shebang_line: Option<&str>) -> Language {
    let from_ext = path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase());

    if let Some(ext) = from_ext.as_deref() {
        match ext {
            "rs" => return Language::Rust,
            "ts" | "tsx" => return Language::TypeScript,
            "js" | "jsx" | "mjs" | "cjs" => return Language::JavaScript,
            "py" => return Language::Python,
            "go" => return Language::Go,
            "toml" => return Language::Toml,
            "json" => return Language::Json,
            "yaml" | "yml" => return Language::Yaml,
            "md" | "markdown" => return Language::Markdown,
            "sh" | "bash" | "zsh" => return Language::Shell,
            _ => {}
        }
    }

    if let Some(line) = shebang_line {
        let lower = line.to_ascii_lowercase();
        if lower.contains("python") {
            return Language::Python;
        }
        if lower.contains("node") || lower.contains("deno") {
            return Language::JavaScript;
        }
        if lower.contains("bash") || lower.contains("/sh") {
            return Language::Shell;
        }
    }

    Language::Unknown
}

fn scan_dir(
    root: &Path,
    dir: &Path,
    depth: usize,
    config: ScannerConfig,
    state: &mut ScanAccumulator,
) -> Result<(), AgentError> {
    if depth > config.depth_limit {
        return Ok(());
    }

    let entries = fs::read_dir(dir).map_err(|error| {
        AgentError::SupervisorError(format!("failed to read '{}': {error}", dir.display()))
    })?;

    for entry in entries {
        let entry = entry.map_err(|error| {
            AgentError::SupervisorError(format!(
                "failed reading directory entry in '{}': {error}",
                dir.display()
            ))
        })?;
        let path = entry.path();
        let file_name = entry.file_name();
        let name = file_name.to_string_lossy();

        let file_type = entry.file_type().map_err(|error| {
            AgentError::SupervisorError(format!(
                "failed resolving file type for '{}': {error}",
                path.display()
            ))
        })?;

        if file_type.is_dir() {
            if is_ignored_dir(name.as_ref()) {
                continue;
            }
            scan_dir(root, path.as_path(), depth + 1, config, state)?;
            continue;
        }

        if !file_type.is_file() {
            continue;
        }

        let metadata = entry.metadata().map_err(|error| {
            AgentError::SupervisorError(format!(
                "failed reading metadata for '{}': {error}",
                path.display()
            ))
        })?;
        if metadata.len() > config.max_file_size_bytes {
            continue;
        }

        let rel_path = make_relative(root, path.as_path())?;
        let shebang = read_shebang(path.as_path());
        let language = detect_language(path.as_path(), shebang.as_deref());
        let modified = metadata
            .modified()
            .ok()
            .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
            .map(|value| value.as_secs())
            .unwrap_or(0);

        state.file_tree.push(FileEntry {
            path: rel_path.clone(),
            size: metadata.len(),
            language,
            last_modified: modified,
        });
        *state.languages.entry(language).or_insert(0) += 1;

        if is_entry_point(path.as_path()) {
            state.entry_points.push(rel_path.clone());
        }
        if is_config_file(path.as_path()) {
            state.config_files.push(rel_path.clone());
        }
        if is_test_file(path.as_path()) {
            state.test_files.push(rel_path.clone());
        }

        if let Ok(content) = fs::read_to_string(path.as_path()) {
            state.total_lines += content.lines().count();
        }
    }

    Ok(())
}

fn make_relative(root: &Path, path: &Path) -> Result<String, AgentError> {
    let rel = path.strip_prefix(root).map_err(|error| {
        AgentError::SupervisorError(format!(
            "failed to strip root prefix '{}' from '{}': {error}",
            root.display(),
            path.display()
        ))
    })?;
    Ok(rel.to_string_lossy().to_string())
}

fn read_shebang(path: &Path) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;
    let first = content.lines().next()?;
    if first.starts_with("#!") {
        return Some(first.to_string());
    }
    None
}

fn is_ignored_dir(name: &str) -> bool {
    matches!(
        name,
        ".git" | "node_modules" | "target" | "__pycache__" | ".venv"
    )
}

fn is_entry_point(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some("main.rs")
            | Some("index.ts")
            | Some("index.js")
            | Some("main.py")
            | Some("app.py")
            | Some("main.go")
            | Some("server.ts")
            | Some("server.js")
    )
}

fn is_config_file(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some("Cargo.toml")
            | Some("Cargo.lock")
            | Some("package.json")
            | Some("pyproject.toml")
            | Some("go.mod")
            | Some("pytest.ini")
            | Some("jest.config.js")
            | Some("jest.config.ts")
            | Some("tsconfig.json")
            | Some("pnpm-workspace.yaml")
    )
}

fn is_test_file(path: &Path) -> bool {
    if path.components().any(|component| {
        matches!(component, Component::Normal(name) if name.to_string_lossy().eq_ignore_ascii_case("tests"))
    }) {
        return true;
    }

    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    let lower_file = file_name.to_ascii_lowercase();
    lower_file.starts_with("test_")
        || lower_file.ends_with("_test.rs")
        || lower_file.ends_with("_test.go")
        || lower_file.ends_with(".test.ts")
        || lower_file.ends_with(".test.js")
        || lower_file.ends_with(".spec.ts")
        || lower_file.ends_with(".spec.js")
}

fn collect_git_info(root: &Path) -> Option<GitInfo> {
    if !is_git_repository(root) {
        return None;
    }

    let branch = run_command(root, "git", ["rev-parse", "--abbrev-ref", "HEAD"])?;
    let commits_raw = run_command(
        root,
        "git",
        ["log", "--pretty=format:%H%x1f%an%x1f%s", "-n", "8"],
    )?;
    let contributors_raw = run_command(root, "git", ["shortlog", "-sn", "-n", "HEAD"])?;

    let recent_commits = commits_raw
        .lines()
        .filter_map(|line| {
            let mut parts = line.split('\u{1f}');
            let hash = parts.next()?.trim().to_string();
            let author = parts.next()?.trim().to_string();
            let subject = parts.next()?.trim().to_string();
            if hash.is_empty() || subject.is_empty() {
                return None;
            }
            Some(GitCommit {
                hash,
                author,
                subject,
            })
        })
        .collect::<Vec<_>>();

    let contributors = contributors_raw
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                return None;
            }
            let mut parts = trimmed.split_whitespace();
            let _count = parts.next();
            let name = parts.collect::<Vec<_>>().join(" ");
            if name.is_empty() {
                return None;
            }
            Some(name)
        })
        .collect::<Vec<_>>();

    Some(GitInfo {
        branch: branch.trim().to_string(),
        recent_commits,
        contributors,
    })
}

fn is_git_repository(root: &Path) -> bool {
    run_command(root, "git", ["rev-parse", "--is-inside-work-tree"])
        .map(|value| value.trim() == "true")
        .unwrap_or(false)
}

fn run_command<const N: usize>(cwd: &Path, program: &str, args: [&str; N]) -> Option<String> {
    let output = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}

pub fn cargo_manifests(project_map: &ProjectMap) -> HashSet<String> {
    project_map
        .config_files
        .iter()
        .filter(|path| {
            Path::new(path.as_str())
                .file_name()
                .and_then(|name| name.to_str())
                == Some("Cargo.toml")
        })
        .cloned()
        .collect::<HashSet<_>>()
}
