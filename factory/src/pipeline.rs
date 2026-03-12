//! Autonomous Software Factory — build, test, deploy, and monitor pipelines.
//!
//! Agents autonomously manage the full software lifecycle: create projects,
//! run builds, execute tests, deploy artifacts, and auto-fix failures.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

// ── Types ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProjectStatus {
    Created,
    Building,
    Testing,
    Deploying,
    Running,
    Failed(String),
    Stopped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoftwareProject {
    pub id: String,
    pub name: String,
    pub language: String,
    pub source_dir: String,
    pub build_command: String,
    pub test_command: String,
    pub deploy_command: Option<String>,
    pub status: ProjectStatus,
    pub created_at: u64,
    pub last_build_at: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildResult {
    pub project_id: String,
    pub success: bool,
    pub output: String,
    pub errors: Vec<String>,
    pub duration_ms: u64,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    pub project_id: String,
    pub success: bool,
    pub passed: u32,
    pub failed: u32,
    pub skipped: u32,
    pub output: String,
    pub duration_ms: u64,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployResult {
    pub project_id: String,
    pub success: bool,
    pub environment: String,
    pub output: String,
    pub url: Option<String>,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineResult {
    pub project_id: String,
    pub build: BuildResult,
    pub test: Option<TestResult>,
    pub deploy: Option<DeployResult>,
    pub overall_success: bool,
    pub total_duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixAttempt {
    pub error: String,
    pub suggestion: String,
    pub file: Option<String>,
    pub applied: bool,
}

// ── Helpers ─────────────────────────────────────────────────────────────

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Run a shell command in a given directory. Returns (success, stdout+stderr).
///
/// **DEPRECATED**: Prefer `run_typed_tool()` which uses `TypedTool` for
/// injection-safe execution. This function is kept for backward compatibility
/// with user-defined build/test/deploy commands that don't map to a TypedTool.
fn run_command(command: &str, cwd: &str) -> (bool, String, u64) {
    let start = std::time::Instant::now();

    let result = std::process::Command::new("sh")
        .arg("-c")
        .arg(command)
        .current_dir(cwd)
        .output();

    let duration = start.elapsed().as_millis() as u64;

    match result {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let combined = if stderr.is_empty() {
                stdout
            } else {
                format!("{stdout}\n{stderr}")
            };
            (output.status.success(), combined, duration)
        }
        Err(e) => (false, format!("Failed to execute command: {e}"), duration),
    }
}

/// Execute a `TypedTool` in the given working directory.
/// Returns (success, output, duration_ms) — same shape as `run_command`
/// but without shell injection risk.
fn run_typed_tool(tool: &nexus_kernel::typed_tools::TypedTool, cwd: &str) -> (bool, String, u64) {
    let path = std::path::Path::new(cwd);
    match nexus_kernel::typed_tools::execute_typed_tool(tool, path) {
        Ok(output) => {
            let combined = if output.stderr.is_empty() {
                output.stdout
            } else {
                format!("{}\n{}", output.stdout, output.stderr)
            };
            (output.exit_code == 0, combined, output.duration_ms)
        }
        Err(e) => (false, format!("TypedTool execution failed: {e}"), 0),
    }
}

/// Try to convert a language's default build command to a TypedTool.
/// Returns None if no mapping exists (falls back to shell).
fn build_tool_for_language(language: &str) -> Option<nexus_kernel::typed_tools::TypedTool> {
    use nexus_kernel::typed_tools::TypedTool;
    match language {
        "rust" => Some(TypedTool::CargoBuild {
            package: None,
            release: false,
        }),
        "javascript" | "typescript" => Some(TypedTool::NpmBuild),
        _ => None,
    }
}

/// Try to convert a language's default test command to a TypedTool.
fn test_tool_for_language(language: &str) -> Option<nexus_kernel::typed_tools::TypedTool> {
    use nexus_kernel::typed_tools::TypedTool;
    match language {
        "rust" => Some(TypedTool::CargoTest {
            package: None,
            test_name: None,
        }),
        "javascript" | "typescript" => Some(TypedTool::NpmTest),
        _ => None,
    }
}

fn default_build_command(language: &str) -> String {
    match language {
        "rust" => "cargo build".to_string(),
        "python" => "python -m py_compile *.py".to_string(),
        "javascript" | "typescript" => "npm run build".to_string(),
        "go" => "go build ./...".to_string(),
        _ => "echo 'no build step'".to_string(),
    }
}

fn default_test_command(language: &str) -> String {
    match language {
        "rust" => "cargo test".to_string(),
        "python" => "python -m pytest -v".to_string(),
        "javascript" | "typescript" => "npm test".to_string(),
        "go" => "go test ./...".to_string(),
        _ => "echo 'no test step'".to_string(),
    }
}

// ── FactoryPipeline ─────────────────────────────────────────────────────

pub struct FactoryPipeline {
    projects: HashMap<String, SoftwareProject>,
    build_history: Vec<BuildResult>,
    auto_fix_enabled: bool,
}

impl Default for FactoryPipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl FactoryPipeline {
    pub fn new() -> Self {
        Self {
            projects: HashMap::new(),
            build_history: Vec::new(),
            auto_fix_enabled: true,
        }
    }

    /// Create a new project tracked by the factory.
    pub fn create_project(
        &mut self,
        name: &str,
        language: &str,
        source_dir: &str,
    ) -> SoftwareProject {
        let id = Uuid::new_v4().to_string();
        let project = SoftwareProject {
            id: id.clone(),
            name: name.to_string(),
            language: language.to_string(),
            source_dir: source_dir.to_string(),
            build_command: default_build_command(language),
            test_command: default_test_command(language),
            deploy_command: None,
            status: ProjectStatus::Created,
            created_at: now_secs(),
            last_build_at: None,
        };
        self.projects.insert(id, project.clone());
        project
    }

    /// Run the build step for a project.
    ///
    /// Uses `TypedTool` for known languages (Rust, JS/TS) to avoid shell
    /// injection. Falls back to `run_command` for custom build commands.
    pub fn build_project(&mut self, project_id: &str) -> Result<BuildResult, String> {
        let project = self
            .projects
            .get_mut(project_id)
            .ok_or_else(|| format!("Project not found: {project_id}"))?;

        project.status = ProjectStatus::Building;

        // Prefer TypedTool when the build command matches the default for the language
        let (success, output, duration) =
            if project.build_command == default_build_command(&project.language) {
                if let Some(tool) = build_tool_for_language(&project.language) {
                    run_typed_tool(&tool, &project.source_dir)
                } else {
                    run_command(&project.build_command, &project.source_dir)
                }
            } else {
                run_command(&project.build_command, &project.source_dir)
            };

        let errors: Vec<String> = if success {
            Vec::new()
        } else {
            output
                .lines()
                .filter(|l| {
                    let lower = l.to_lowercase();
                    lower.contains("error") || lower.contains("failed")
                })
                .map(|s| s.to_string())
                .collect()
        };

        let result = BuildResult {
            project_id: project_id.to_string(),
            success,
            output: output.clone(),
            errors: errors.clone(),
            duration_ms: duration,
            timestamp: now_secs(),
        };

        project.last_build_at = Some(result.timestamp);
        if success {
            project.status = ProjectStatus::Created; // ready for next step
        } else {
            project.status =
                ProjectStatus::Failed(errors.first().cloned().unwrap_or_else(|| output.clone()));
        }

        self.build_history.push(result.clone());
        Ok(result)
    }

    /// Run the test step for a project.
    ///
    /// Uses `TypedTool` for known languages (Rust, JS/TS) to avoid shell
    /// injection. Falls back to `run_command` for custom test commands.
    pub fn test_project(&mut self, project_id: &str) -> Result<TestResult, String> {
        let project = self
            .projects
            .get_mut(project_id)
            .ok_or_else(|| format!("Project not found: {project_id}"))?;

        project.status = ProjectStatus::Testing;

        let (success, output, duration) =
            if project.test_command == default_test_command(&project.language) {
                if let Some(tool) = test_tool_for_language(&project.language) {
                    run_typed_tool(&tool, &project.source_dir)
                } else {
                    run_command(&project.test_command, &project.source_dir)
                }
            } else {
                run_command(&project.test_command, &project.source_dir)
            };

        // Parse test counts from output (best-effort).
        let (passed, failed, skipped) = parse_test_counts(&output);

        let result = TestResult {
            project_id: project_id.to_string(),
            success,
            passed,
            failed,
            skipped,
            output,
            duration_ms: duration,
            timestamp: now_secs(),
        };

        if !success {
            project.status = ProjectStatus::Failed("Tests failed".to_string());
        } else {
            project.status = ProjectStatus::Created;
        }

        Ok(result)
    }

    /// Deploy a project.
    pub fn deploy_project(&mut self, project_id: &str) -> Result<DeployResult, String> {
        let project = self
            .projects
            .get_mut(project_id)
            .ok_or_else(|| format!("Project not found: {project_id}"))?;

        let deploy_cmd = project
            .deploy_command
            .clone()
            .unwrap_or_else(|| "echo 'deployed (stub)'".to_string());

        project.status = ProjectStatus::Deploying;
        let (success, output, _duration) = run_command(&deploy_cmd, &project.source_dir);

        let result = DeployResult {
            project_id: project_id.to_string(),
            success,
            environment: "production".to_string(),
            output,
            url: if success {
                Some(format!("https://{}.nexus.local", project.name))
            } else {
                None
            },
            timestamp: now_secs(),
        };

        if success {
            project.status = ProjectStatus::Running;
        } else {
            project.status = ProjectStatus::Failed("Deploy failed".to_string());
        }

        Ok(result)
    }

    /// Run the full pipeline: build → test → deploy.
    pub fn run_full_pipeline(&mut self, project_id: &str) -> Result<PipelineResult, String> {
        let start = std::time::Instant::now();

        let build = self.build_project(project_id)?;
        if !build.success {
            return Ok(PipelineResult {
                project_id: project_id.to_string(),
                build,
                test: None,
                deploy: None,
                overall_success: false,
                total_duration_ms: start.elapsed().as_millis() as u64,
            });
        }

        let test = self.test_project(project_id)?;
        if !test.success {
            return Ok(PipelineResult {
                project_id: project_id.to_string(),
                build,
                test: Some(test),
                deploy: None,
                overall_success: false,
                total_duration_ms: start.elapsed().as_millis() as u64,
            });
        }

        let deploy_result = if self
            .projects
            .get(project_id)
            .and_then(|p| p.deploy_command.as_ref())
            .is_some()
        {
            Some(self.deploy_project(project_id)?)
        } else {
            None
        };

        let overall = deploy_result.as_ref().is_none_or(|d| d.success);

        Ok(PipelineResult {
            project_id: project_id.to_string(),
            build,
            test: Some(test),
            deploy: deploy_result,
            overall_success: overall,
            total_duration_ms: start.elapsed().as_millis() as u64,
        })
    }

    /// Suggest fixes for build errors. Stub — returns placeholder suggestions.
    pub fn auto_fix_build_errors(&self, _project_id: &str, errors: &[String]) -> Vec<FixAttempt> {
        if !self.auto_fix_enabled {
            return Vec::new();
        }

        errors
            .iter()
            .map(|err| {
                let (suggestion, file) = suggest_fix(err);
                FixAttempt {
                    error: err.clone(),
                    suggestion,
                    file,
                    applied: false,
                }
            })
            .collect()
    }

    pub fn get_project(&self, id: &str) -> Option<&SoftwareProject> {
        self.projects.get(id)
    }

    pub fn list_projects(&self) -> Vec<&SoftwareProject> {
        self.projects.values().collect()
    }

    pub fn get_build_history(&self, project_id: &str) -> Vec<&BuildResult> {
        self.build_history
            .iter()
            .filter(|b| b.project_id == project_id)
            .collect()
    }

    pub fn auto_fix_enabled(&self) -> bool {
        self.auto_fix_enabled
    }

    pub fn set_auto_fix_enabled(&mut self, enabled: bool) {
        self.auto_fix_enabled = enabled;
    }

    pub fn project_count(&self) -> usize {
        self.projects.len()
    }
}

// ── Parsing helpers ─────────────────────────────────────────────────────

fn parse_test_counts(output: &str) -> (u32, u32, u32) {
    // Best-effort: look for "N passed", "N failed", "N skipped" patterns.
    let mut passed = 0u32;
    let mut failed = 0u32;
    let mut skipped = 0u32;

    for line in output.lines() {
        let lower = line.to_lowercase();
        if let Some(n) = extract_count_before(&lower, "passed") {
            passed = n;
        }
        if let Some(n) = extract_count_before(&lower, "failed") {
            failed = n;
        }
        if let Some(n) = extract_count_before(&lower, "skipped")
            .or_else(|| extract_count_before(&lower, "ignored"))
        {
            skipped = n;
        }
    }
    (passed, failed, skipped)
}

fn extract_count_before(line: &str, keyword: &str) -> Option<u32> {
    let idx = line.find(keyword)?;
    let before = line[..idx].trim();
    let last_token = before
        .rsplit_once(char::is_whitespace)
        .map_or(before, |t| t.1);
    // Strip semicolons, commas, etc.
    let cleaned: String = last_token.chars().filter(|c| c.is_ascii_digit()).collect();
    cleaned.parse().ok()
}

fn suggest_fix(error: &str) -> (String, Option<String>) {
    let lower = error.to_lowercase();

    if lower.contains("not found") || lower.contains("cannot find") {
        (
            "Check import paths and ensure the referenced module/type exists.".to_string(),
            None,
        )
    } else if lower.contains("type mismatch") || lower.contains("expected") {
        (
            "Verify function signatures match their call sites.".to_string(),
            None,
        )
    } else if lower.contains("unused") {
        (
            "Remove or prefix with underscore to suppress unused warnings.".to_string(),
            None,
        )
    } else if lower.contains("missing") {
        (
            "Add the missing field, import, or dependency.".to_string(),
            None,
        )
    } else {
        (
            "Review the error context and consult documentation.".to_string(),
            None,
        )
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_dir() -> String {
        std::env::temp_dir().display().to_string()
    }

    #[test]
    fn test_create_project() {
        let mut factory = FactoryPipeline::new();
        let project = factory.create_project("my-app", "rust", &tmp_dir());

        assert_eq!(project.name, "my-app");
        assert_eq!(project.language, "rust");
        assert_eq!(project.build_command, "cargo build");
        assert_eq!(project.test_command, "cargo test");
        assert!(matches!(project.status, ProjectStatus::Created));
        assert!(factory.get_project(&project.id).is_some());
    }

    #[test]
    fn test_list_projects() {
        let mut factory = FactoryPipeline::new();
        factory.create_project("app-a", "rust", &tmp_dir());
        factory.create_project("app-b", "python", &tmp_dir());
        factory.create_project("app-c", "go", &tmp_dir());

        assert_eq!(factory.list_projects().len(), 3);
        assert_eq!(factory.project_count(), 3);
    }

    #[test]
    fn test_default_build_commands() {
        assert_eq!(default_build_command("rust"), "cargo build");
        assert_eq!(default_build_command("python"), "python -m py_compile *.py");
        assert_eq!(default_build_command("javascript"), "npm run build");
        assert_eq!(default_build_command("go"), "go build ./...");
        assert_eq!(default_build_command("unknown"), "echo 'no build step'");
    }

    #[test]
    fn test_default_test_commands() {
        assert_eq!(default_test_command("rust"), "cargo test");
        assert_eq!(default_test_command("python"), "python -m pytest -v");
        assert_eq!(default_test_command("typescript"), "npm test");
        assert_eq!(default_test_command("go"), "go test ./...");
    }

    #[test]
    fn test_build_project_success() {
        let mut factory = FactoryPipeline::new();
        let mut project = factory.create_project("echo-app", "rust", &tmp_dir());
        // Override build command to something that always succeeds.
        project.build_command = "echo 'build ok'".to_string();
        factory.projects.insert(project.id.clone(), project.clone());

        let result = factory.build_project(&project.id).unwrap();
        assert!(result.success);
        assert!(result.output.contains("build ok"));
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_build_project_failure() {
        let mut factory = FactoryPipeline::new();
        let mut project = factory.create_project("fail-app", "rust", &tmp_dir());
        project.build_command = "sh -c 'echo error: compilation failed && exit 1'".to_string();
        factory.projects.insert(project.id.clone(), project.clone());

        let result = factory.build_project(&project.id).unwrap();
        assert!(!result.success);
        assert!(!result.errors.is_empty());

        let p = factory.get_project(&project.id).unwrap();
        assert!(matches!(p.status, ProjectStatus::Failed(_)));
    }

    #[test]
    fn test_build_project_not_found() {
        let mut factory = FactoryPipeline::new();
        let err = factory.build_project("nonexistent").unwrap_err();
        assert!(err.contains("not found"));
    }

    #[test]
    fn test_test_project() {
        let mut factory = FactoryPipeline::new();
        let mut project = factory.create_project("test-app", "rust", &tmp_dir());
        project.test_command = "echo '5 passed; 0 failed; 1 ignored'".to_string();
        factory.projects.insert(project.id.clone(), project.clone());

        let result = factory.test_project(&project.id).unwrap();
        assert!(result.success);
        assert_eq!(result.passed, 5);
        assert_eq!(result.failed, 0);
        assert_eq!(result.skipped, 1);
    }

    #[test]
    fn test_deploy_project() {
        let mut factory = FactoryPipeline::new();
        let mut project = factory.create_project("deploy-app", "rust", &tmp_dir());
        project.deploy_command = Some("echo 'deployed successfully'".to_string());
        factory.projects.insert(project.id.clone(), project.clone());

        let result = factory.deploy_project(&project.id).unwrap();
        assert!(result.success);
        assert_eq!(result.environment, "production");
        assert!(result.url.is_some());

        let p = factory.get_project(&project.id).unwrap();
        assert!(matches!(p.status, ProjectStatus::Running));
    }

    #[test]
    fn test_full_pipeline_success() {
        let mut factory = FactoryPipeline::new();
        let mut project = factory.create_project("pipeline-app", "rust", &tmp_dir());
        project.build_command = "echo 'build ok'".to_string();
        project.test_command = "echo '3 passed; 0 failed'".to_string();
        // No deploy command — pipeline should still succeed.
        factory.projects.insert(project.id.clone(), project.clone());

        let result = factory.run_full_pipeline(&project.id).unwrap();
        assert!(result.overall_success);
        assert!(result.build.success);
        assert!(result.test.is_some());
        assert!(result.test.as_ref().unwrap().success);
        assert!(result.deploy.is_none()); // no deploy command set
    }

    #[test]
    fn test_full_pipeline_build_failure_stops() {
        let mut factory = FactoryPipeline::new();
        let mut project = factory.create_project("fail-pipe", "rust", &tmp_dir());
        project.build_command = "sh -c 'echo error && exit 1'".to_string();
        factory.projects.insert(project.id.clone(), project.clone());

        let result = factory.run_full_pipeline(&project.id).unwrap();
        assert!(!result.overall_success);
        assert!(!result.build.success);
        assert!(result.test.is_none()); // never ran
        assert!(result.deploy.is_none());
    }

    #[test]
    fn test_auto_fix_suggestions() {
        let factory = FactoryPipeline::new();
        let errors = vec![
            "error: module not found".to_string(),
            "error: type mismatch".to_string(),
            "warning: unused variable".to_string(),
        ];

        let fixes = factory.auto_fix_build_errors("proj-1", &errors);
        assert_eq!(fixes.len(), 3);
        assert!(fixes[0].suggestion.contains("import paths"));
        assert!(fixes[1].suggestion.contains("function signatures"));
        assert!(fixes[2].suggestion.contains("underscore"));
        assert!(fixes.iter().all(|f| !f.applied));
    }

    #[test]
    fn test_auto_fix_disabled() {
        let mut factory = FactoryPipeline::new();
        factory.set_auto_fix_enabled(false);
        assert!(!factory.auto_fix_enabled());

        let fixes = factory.auto_fix_build_errors("proj-1", &["error".to_string()]);
        assert!(fixes.is_empty());
    }

    #[test]
    fn test_build_history() {
        let mut factory = FactoryPipeline::new();
        let mut project = factory.create_project("hist-app", "rust", &tmp_dir());
        project.build_command = "echo 'ok'".to_string();
        factory.projects.insert(project.id.clone(), project.clone());

        factory.build_project(&project.id).unwrap();
        factory.build_project(&project.id).unwrap();

        let history = factory.get_build_history(&project.id);
        assert_eq!(history.len(), 2);
    }

    #[test]
    fn test_parse_test_counts() {
        let output = "test result: ok. 10 passed; 2 failed; 1 ignored;";
        let (p, f, s) = parse_test_counts(output);
        assert_eq!(p, 10);
        assert_eq!(f, 2);
        assert_eq!(s, 1);
    }

    #[test]
    fn test_parse_test_counts_empty() {
        let (p, f, s) = parse_test_counts("no test output here");
        assert_eq!(p, 0);
        assert_eq!(f, 0);
        assert_eq!(s, 0);
    }
}
