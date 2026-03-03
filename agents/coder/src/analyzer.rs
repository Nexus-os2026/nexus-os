use crate::scanner::{cargo_manifests, Language, ProjectMap};
use nexus_connectors_llm::gateway::{AgentRuntimeContext, GovernedLlmGateway};
use nexus_connectors_llm::providers::{LlmProvider, MockProvider};
use nexus_kernel::errors::AgentError;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectType {
    RustWorkspace,
    RustCrate,
    NodeMonorepo,
    NodeProject,
    PythonPackage,
    GoModule,
    Polyglot,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiSurfaceEntry {
    pub file: String,
    pub symbol: String,
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchitectureReport {
    pub project_type: ProjectType,
    pub module_dependencies: HashMap<String, Vec<String>>,
    pub design_patterns: Vec<String>,
    pub test_frameworks: Vec<String>,
    pub api_surface: Vec<ApiSurfaceEntry>,
    pub dependency_graph: HashMap<String, Vec<String>>,
    pub llm_summary: Option<String>,
}

pub fn analyze(project_map: &ProjectMap) -> Result<ArchitectureReport, AgentError> {
    let project_type = detect_project_type(project_map);
    let module_dependencies = extract_module_dependencies(project_map)?;
    let design_patterns = detect_design_patterns(project_map);
    let test_frameworks = detect_test_frameworks(project_map);
    let api_surface = extract_api_surface(project_map)?;
    let dependency_graph = module_dependencies.clone();
    let llm_summary = query_llm_summary(project_map, &project_type, test_frameworks.as_slice());

    Ok(ArchitectureReport {
        project_type,
        module_dependencies,
        design_patterns,
        test_frameworks,
        api_surface,
        dependency_graph,
        llm_summary,
    })
}

pub fn detect_project_type(project_map: &ProjectMap) -> ProjectType {
    let cargo_count = cargo_manifests(project_map).len();
    let has_workspace = project_map
        .config_files
        .iter()
        .filter(|path| path.ends_with("Cargo.toml"))
        .any(|manifest| {
            read_file(project_map, manifest)
                .map(|content| content.contains("[workspace]"))
                .unwrap_or(false)
        });
    if has_workspace {
        return ProjectType::RustWorkspace;
    }
    if cargo_count > 0 {
        return ProjectType::RustCrate;
    }

    let has_package_json = project_map
        .config_files
        .iter()
        .any(|path| path.ends_with("package.json"));
    let has_pnpm_workspace = project_map
        .config_files
        .iter()
        .any(|path| path.ends_with("pnpm-workspace.yaml"));
    if has_package_json && has_pnpm_workspace {
        return ProjectType::NodeMonorepo;
    }
    if has_package_json {
        return ProjectType::NodeProject;
    }

    if project_map
        .config_files
        .iter()
        .any(|path| path.ends_with("pyproject.toml") || path.ends_with("setup.py"))
    {
        return ProjectType::PythonPackage;
    }

    if project_map
        .config_files
        .iter()
        .any(|path| path.ends_with("go.mod"))
    {
        return ProjectType::GoModule;
    }

    if project_map.languages.len() >= 3 {
        return ProjectType::Polyglot;
    }
    ProjectType::Unknown
}

fn extract_module_dependencies(
    project_map: &ProjectMap,
) -> Result<HashMap<String, Vec<String>>, AgentError> {
    let mut graph: HashMap<String, Vec<String>> = HashMap::new();

    for entry in &project_map.file_tree {
        let Some(content) = read_file(project_map, entry.path.as_str()) else {
            continue;
        };
        let deps = match entry.language {
            Language::Rust => parse_rust_dependencies(content.as_str()),
            Language::TypeScript | Language::JavaScript => parse_js_dependencies(content.as_str()),
            Language::Python => parse_python_dependencies(content.as_str()),
            Language::Go => parse_go_dependencies(content.as_str()),
            _ => Vec::new(),
        };
        if deps.is_empty() {
            continue;
        }
        graph.insert(entry.path.clone(), dedupe_sorted(deps));
    }

    Ok(graph)
}

fn detect_design_patterns(project_map: &ProjectMap) -> Vec<String> {
    let lower_paths = project_map
        .file_tree
        .iter()
        .map(|entry| entry.path.to_ascii_lowercase())
        .collect::<Vec<_>>();

    let has_controller = lower_paths.iter().any(|path| path.contains("controller"));
    let has_model = lower_paths.iter().any(|path| path.contains("model"));
    let has_view = lower_paths.iter().any(|path| path.contains("view"));
    let has_service = lower_paths.iter().any(|path| path.contains("service"));
    let has_repo = lower_paths.iter().any(|path| path.contains("repository"));
    let has_kernel = lower_paths.iter().any(|path| path.starts_with("kernel/"));
    let has_connector = lower_paths
        .iter()
        .any(|path| path.starts_with("connectors/"));
    let has_app = lower_paths.iter().any(|path| path.starts_with("app/"));

    let mut patterns = Vec::new();
    if has_controller && has_model && has_view {
        patterns.push("MVC".to_string());
    }
    if has_service && has_repo {
        patterns.push("Layered".to_string());
    }
    if has_kernel && has_connector && has_app {
        patterns.push("Layered".to_string());
    }
    if patterns.is_empty() {
        patterns.push("Monolith".to_string());
    }
    patterns.sort();
    patterns.dedup();
    patterns
}

fn detect_test_frameworks(project_map: &ProjectMap) -> Vec<String> {
    let mut frameworks = BTreeSet::new();

    if project_map
        .config_files
        .iter()
        .any(|path| path.ends_with("Cargo.toml"))
    {
        frameworks.insert("cargo test".to_string());
    }
    if project_map
        .config_files
        .iter()
        .any(|path| path.ends_with("package.json"))
    {
        frameworks.insert("npm test".to_string());
    }
    if project_map
        .config_files
        .iter()
        .any(|path| path.ends_with("jest.config.js") || path.ends_with("jest.config.ts"))
    {
        frameworks.insert("jest".to_string());
    }
    if project_map
        .config_files
        .iter()
        .any(|path| path.ends_with("pyproject.toml") || path.ends_with("pytest.ini"))
    {
        frameworks.insert("pytest".to_string());
    }
    if project_map
        .config_files
        .iter()
        .any(|path| path.ends_with("go.mod"))
    {
        frameworks.insert("go test".to_string());
    }

    frameworks.into_iter().collect::<Vec<_>>()
}

fn extract_api_surface(project_map: &ProjectMap) -> Result<Vec<ApiSurfaceEntry>, AgentError> {
    let mut surface = Vec::new();

    for entry in &project_map.file_tree {
        let Some(content) = read_file(project_map, entry.path.as_str()) else {
            continue;
        };

        for line in content.lines() {
            let trimmed = line.trim();
            match entry.language {
                Language::Rust => {
                    if let Some(symbol) = parse_decl(trimmed, "pub fn ") {
                        surface.push(ApiSurfaceEntry {
                            file: entry.path.clone(),
                            symbol,
                            kind: "public_function".to_string(),
                        });
                    } else if let Some(symbol) = parse_decl(trimmed, "pub struct ") {
                        surface.push(ApiSurfaceEntry {
                            file: entry.path.clone(),
                            symbol,
                            kind: "public_struct".to_string(),
                        });
                    } else if let Some(symbol) = parse_decl(trimmed, "pub trait ") {
                        surface.push(ApiSurfaceEntry {
                            file: entry.path.clone(),
                            symbol,
                            kind: "public_trait".to_string(),
                        });
                    } else if let Some(symbol) = parse_decl(trimmed, "pub enum ") {
                        surface.push(ApiSurfaceEntry {
                            file: entry.path.clone(),
                            symbol,
                            kind: "public_enum".to_string(),
                        });
                    } else if let Some(endpoint) = parse_endpoint_attribute(trimmed) {
                        surface.push(ApiSurfaceEntry {
                            file: entry.path.clone(),
                            symbol: endpoint,
                            kind: "rest_endpoint".to_string(),
                        });
                    }
                }
                Language::TypeScript | Language::JavaScript => {
                    if let Some(symbol) = parse_decl(trimmed, "export function ") {
                        surface.push(ApiSurfaceEntry {
                            file: entry.path.clone(),
                            symbol,
                            kind: "exported_function".to_string(),
                        });
                    } else if let Some(symbol) = parse_decl(trimmed, "export class ") {
                        surface.push(ApiSurfaceEntry {
                            file: entry.path.clone(),
                            symbol,
                            kind: "exported_class".to_string(),
                        });
                    } else if let Some(symbol) = parse_decl(trimmed, "export interface ") {
                        surface.push(ApiSurfaceEntry {
                            file: entry.path.clone(),
                            symbol,
                            kind: "exported_interface".to_string(),
                        });
                    } else if let Some(endpoint) = parse_js_endpoint(trimmed) {
                        surface.push(ApiSurfaceEntry {
                            file: entry.path.clone(),
                            symbol: endpoint,
                            kind: "rest_endpoint".to_string(),
                        });
                    }
                }
                Language::Python => {
                    if let Some(symbol) = parse_decl(trimmed, "def ") {
                        surface.push(ApiSurfaceEntry {
                            file: entry.path.clone(),
                            symbol,
                            kind: "function".to_string(),
                        });
                    } else if let Some(endpoint) = parse_python_endpoint(trimmed) {
                        surface.push(ApiSurfaceEntry {
                            file: entry.path.clone(),
                            symbol: endpoint,
                            kind: "rest_endpoint".to_string(),
                        });
                    }
                }
                _ => {}
            }
        }
    }

    surface.sort_by(|left, right| {
        left.file
            .cmp(&right.file)
            .then(left.symbol.cmp(&right.symbol))
    });
    surface.dedup_by(|left, right| {
        left.file == right.file && left.symbol == right.symbol && left.kind == right.kind
    });
    Ok(surface)
}

fn query_llm_summary(
    project_map: &ProjectMap,
    project_type: &ProjectType,
    test_frameworks: &[String],
) -> Option<String> {
    let provider: Box<dyn LlmProvider> = Box::new(MockProvider::new());
    let mut gateway = GovernedLlmGateway::new(provider);
    let capabilities = ["llm.query".to_string()]
        .into_iter()
        .collect::<HashSet<_>>();
    let mut runtime = AgentRuntimeContext {
        agent_id: Uuid::new_v4(),
        capabilities,
        fuel_remaining: 1_000,
    };
    let prompt = format!(
        "Project type: {:?}. Languages: {:?}. Tests: {:?}. Summarize architecture patterns in one sentence.",
        project_type, project_map.languages, test_frameworks
    );

    gateway
        .query(&mut runtime, prompt.as_str(), 48, "mock-1")
        .ok()
        .map(|response| response.output_text)
}

fn read_file(project_map: &ProjectMap, rel_path: &str) -> Option<String> {
    let root = PathBuf::from(project_map.root_path.as_str());
    let full = root.join(rel_path);
    fs::read_to_string(full).ok()
}

fn parse_rust_dependencies(content: &str) -> Vec<String> {
    let mut deps = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(value) = parse_decl(trimmed, "use ") {
            let root = value
                .split("::")
                .next()
                .unwrap_or_default()
                .trim()
                .trim_end_matches(';')
                .to_string();
            if !root.is_empty() {
                deps.push(root);
            }
        } else if let Some(value) = parse_decl(trimmed, "mod ") {
            let module = value.trim_end_matches(';').to_string();
            if !module.is_empty() {
                deps.push(module);
            }
        }
    }
    deps
}

fn parse_js_dependencies(content: &str) -> Vec<String> {
    let mut deps = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("import ") {
            if let Some(from_idx) = trimmed.find(" from ") {
                let source = trimmed[from_idx + 6..].trim();
                if let Some(module) = quoted_value(source) {
                    deps.push(module);
                }
            }
        } else if let Some(start) = trimmed.find("require(") {
            let rest = &trimmed[start + "require(".len()..];
            if let Some(module) = quoted_value(rest) {
                deps.push(module);
            }
        }
    }
    deps
}

fn parse_python_dependencies(content: &str) -> Vec<String> {
    let mut deps = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(value) = parse_decl(trimmed, "import ") {
            for dep in value.split(',') {
                let module = dep
                    .split_whitespace()
                    .next()
                    .unwrap_or_default()
                    .trim()
                    .to_string();
                if !module.is_empty() {
                    deps.push(module);
                }
            }
        } else if let Some(value) = parse_decl(trimmed, "from ") {
            let module = value
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .trim()
                .to_string();
            if !module.is_empty() {
                deps.push(module);
            }
        }
    }
    deps
}

fn parse_go_dependencies(content: &str) -> Vec<String> {
    let mut deps = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(value) = parse_decl(trimmed, "import ") {
            if let Some(module) = quoted_value(value.as_str()) {
                deps.push(module);
            }
        }
    }
    deps
}

fn parse_decl(line: &str, prefix: &str) -> Option<String> {
    if !line.starts_with(prefix) {
        return None;
    }
    let value = line.strip_prefix(prefix)?;
    let symbol = value
        .split(['(', '{', ';', ' ', '<'])
        .next()
        .unwrap_or_default()
        .trim()
        .to_string();
    if symbol.is_empty() {
        return None;
    }
    Some(symbol)
}

fn parse_endpoint_attribute(line: &str) -> Option<String> {
    let methods = ["#[get(", "#[post(", "#[put(", "#[delete("];
    for method in methods {
        if line.starts_with(method) {
            return quoted_value(line).map(|path| format!("{} {}", method, path));
        }
    }
    None
}

fn parse_js_endpoint(line: &str) -> Option<String> {
    let methods = [".get(", ".post(", ".put(", ".delete("];
    for method in methods {
        if let Some(index) = line.find(method) {
            let tail = &line[index + method.len()..];
            if let Some(path) = quoted_value(tail) {
                return Some(format!("{} {}", method.trim_matches('.'), path));
            }
        }
    }
    None
}

fn parse_python_endpoint(line: &str) -> Option<String> {
    let methods = ["@app.get(", "@app.post(", "@router.get(", "@router.post("];
    for method in methods {
        if line.starts_with(method) {
            return quoted_value(line).map(|path| format!("{method}{path}"));
        }
    }
    None
}

fn quoted_value(input: &str) -> Option<String> {
    let single = input.find('\'');
    let double = input.find('"');

    let (start, quote) = match (single, double) {
        (Some(s), Some(d)) if s < d => (s, '\''),
        (Some(s), Some(_)) => (s, '\''),
        (None, Some(d)) => (d, '"'),
        (Some(s), None) => (s, '\''),
        (None, None) => return None,
    };

    let rest = &input[start + 1..];
    let end = rest.find(quote)?;
    let value = rest[..end].trim().to_string();
    if value.is_empty() {
        return None;
    }
    Some(value)
}

fn dedupe_sorted(values: Vec<String>) -> Vec<String> {
    let mut unique = values
        .into_iter()
        .filter(|value| !value.trim().is_empty())
        .collect::<Vec<_>>();
    unique.sort();
    unique.dedup();
    unique
}
