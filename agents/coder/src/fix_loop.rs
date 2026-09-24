use crate::test_runner::{run_tests, TestError, TestResult};
use crate::writer::FileChange;
use nexus_connectors_llm::gateway::{
    select_provider, AgentRuntimeContext, GovernedLlmGateway, ProviderSelectionConfig,
};
use nexus_connectors_llm::providers::LlmProvider;
use nexus_sdk::audit::{AuditEvent, AuditTrail, EventType};
use nexus_sdk::errors::AgentError;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashSet;
use std::fs;
use std::path::{Component, Path, PathBuf};
use uuid::Uuid;

const DEFAULT_MAX_ITERATIONS: u32 = 5;
const DEFAULT_FUEL_BUDGET: u64 = 5_000;
const FUEL_COST_TEST_RUN: u64 = 40;
const FUEL_COST_FIX_GENERATION: u64 = 20;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FixResult {
    Success {
        iterations: u32,
        applied_changes: usize,
        last_result: TestResult,
        audit_events: Vec<AuditEvent>,
    },
    MaxIterationsReached {
        iterations: u32,
        remaining_errors: Vec<TestError>,
        last_result: TestResult,
        audit_events: Vec<AuditEvent>,
    },
}

pub trait TestExecutor {
    fn run_tests(&mut self, project_path: &Path) -> Result<TestResult, AgentError>;
}

pub trait ErrorFixer {
    fn propose_fixes(
        &mut self,
        project_path: &Path,
        errors: &[TestError],
        iteration: u32,
    ) -> Result<Vec<FileChange>, AgentError>;
}

pub struct FrameworkTestExecutor;

impl TestExecutor for FrameworkTestExecutor {
    fn run_tests(&mut self, project_path: &Path) -> Result<TestResult, AgentError> {
        run_tests(project_path)
    }
}

pub struct LlmErrorFixer {
    gateway: GovernedLlmGateway<Box<dyn LlmProvider>>,
    runtime: AgentRuntimeContext,
}

impl Default for LlmErrorFixer {
    fn default() -> Self {
        Self::new()
    }
}

impl LlmErrorFixer {
    pub fn new() -> Self {
        let config = ProviderSelectionConfig::from_env();
        let provider: Box<dyn LlmProvider> = select_provider(&config).unwrap_or_else(|_| {
            Box::new(nexus_connectors_llm::providers::OllamaProvider::from_env())
        });
        let gateway = GovernedLlmGateway::new(provider);
        let capabilities = ["llm.query".to_string()]
            .into_iter()
            .collect::<HashSet<_>>();
        let runtime = AgentRuntimeContext {
            agent_id: Uuid::new_v4(),
            capabilities,
            fuel_remaining: 2_000,
        };
        Self { gateway, runtime }
    }
}

impl ErrorFixer for LlmErrorFixer {
    fn propose_fixes(
        &mut self,
        _project_path: &Path,
        errors: &[TestError],
        iteration: u32,
    ) -> Result<Vec<FileChange>, AgentError> {
        let prompt = format!(
            "Iteration {iteration}. Analyze {} errors and propose minimal patch strategy.",
            errors.len()
        );
        // Best-effort: query LLM for fix strategy analysis (result not used directly)
        let _ = self
            .gateway
            .query(&mut self.runtime, prompt.as_str(), 96, "mock-1")?;

        // Safe default: no automatic mutation when strategy is uncertain.
        Ok(Vec::new())
    }
}

pub fn fix_until_pass(
    project_path: impl AsRef<Path>,
    changes: Vec<FileChange>,
    max_iterations: u32,
) -> Result<FixResult, AgentError> {
    let mut executor = FrameworkTestExecutor;
    let mut fixer = LlmErrorFixer::new();
    fix_until_pass_with(
        project_path,
        changes,
        max_iterations,
        &mut executor,
        &mut fixer,
    )
}

pub fn fix_until_pass_with(
    project_path: impl AsRef<Path>,
    changes: Vec<FileChange>,
    max_iterations: u32,
    executor: &mut dyn TestExecutor,
    fixer: &mut dyn ErrorFixer,
) -> Result<FixResult, AgentError> {
    let mut audit = AuditTrail::new();
    let mut fuel_remaining = DEFAULT_FUEL_BUDGET;
    let mut pending_changes = changes;
    let mut applied_changes = 0_usize;
    let limit = if max_iterations == 0 {
        DEFAULT_MAX_ITERATIONS
    } else {
        max_iterations
    };
    let project_path = project_path.as_ref();

    let mut last_result = TestResult {
        framework: crate::test_runner::TestFramework::Unknown,
        passed: 0,
        failed: 0,
        errors: Vec::new(),
        stdout: String::new(),
        stderr: String::new(),
    };

    for iteration in 1..=limit {
        if !pending_changes.is_empty() {
            applied_changes += apply_changes(project_path, pending_changes.as_slice())?;
            if let Err(e) = audit.append_event(
                Uuid::nil(),
                EventType::ToolCall,
                json!({
                    "step": "apply_changes",
                    "iteration": iteration,
                    "changes": pending_changes.len(),
                }),
            ) {
                tracing::error!("Audit append failed: {e}");
            }
        }

        charge_fuel(&mut fuel_remaining, FUEL_COST_TEST_RUN)?;
        let test_result = executor.run_tests(project_path)?;
        if let Err(e) = audit.append_event(
            Uuid::nil(),
            EventType::ToolCall,
            json!({
                "step": "run_tests",
                "iteration": iteration,
                "framework": format!("{:?}", test_result.framework),
                "passed": test_result.passed,
                "failed": test_result.failed,
            }),
        ) {
            tracing::error!("Audit append failed: {e}");
        }

        if test_result.failed == 0 && test_result.errors.is_empty() {
            return Ok(FixResult::Success {
                iterations: iteration,
                applied_changes,
                last_result: test_result,
                audit_events: audit.events().to_vec(),
            });
        }

        last_result = test_result;
        if iteration == limit {
            return Ok(FixResult::MaxIterationsReached {
                iterations: limit,
                remaining_errors: last_result.errors.clone(),
                last_result,
                audit_events: audit.events().to_vec(),
            });
        }

        charge_fuel(&mut fuel_remaining, FUEL_COST_FIX_GENERATION)?;
        pending_changes =
            fixer.propose_fixes(project_path, last_result.errors.as_slice(), iteration)?;
        if let Err(e) = audit.append_event(
            Uuid::nil(),
            EventType::LlmCall,
            json!({
                "step": "generate_fix",
                "iteration": iteration,
                "proposed_changes": pending_changes.len(),
                "remaining_fuel": fuel_remaining,
            }),
        ) {
            tracing::error!("Audit append failed: {e}");
        }
    }

    Ok(FixResult::MaxIterationsReached {
        iterations: limit,
        remaining_errors: last_result.errors.clone(),
        last_result,
        audit_events: audit.events().to_vec(),
    })
}

fn charge_fuel(fuel_remaining: &mut u64, cost: u64) -> Result<(), AgentError> {
    if *fuel_remaining < cost {
        return Err(AgentError::FuelExhausted);
    }
    *fuel_remaining -= cost;
    Ok(())
}

fn apply_changes(project_path: &Path, changes: &[FileChange]) -> Result<usize, AgentError> {
    let mut applied = 0_usize;
    for change in changes {
        match change {
            FileChange::Create(path, content) => {
                let full = resolve(project_path, path.as_str())?;
                if let Some(parent) = full.parent() {
                    fs::create_dir_all(parent).map_err(|error| {
                        AgentError::SupervisorError(format!(
                            "failed creating parent '{}': {error}",
                            parent.display()
                        ))
                    })?;
                }
                fs::write(full, content).map_err(|error| {
                    AgentError::SupervisorError(format!("failed creating '{path}': {error}"))
                })?;
                applied += 1;
            }
            FileChange::Modify(path, _old, new) => {
                let full = resolve(project_path, path.as_str())?;
                if let Some(parent) = full.parent() {
                    fs::create_dir_all(parent).map_err(|error| {
                        AgentError::SupervisorError(format!(
                            "failed creating parent '{}': {error}",
                            parent.display()
                        ))
                    })?;
                }
                fs::write(full, new).map_err(|error| {
                    AgentError::SupervisorError(format!("failed modifying '{path}': {error}"))
                })?;
                applied += 1;
            }
            FileChange::Delete(path) => {
                let full =
                    nexus_sdk::workspace::resolve_entry_for_unlink(project_path, Path::new(path))?;
                // Retain the fix loop's relative-only input policy, without
                // following the final symlink as read/write resolution does.
                validate_relative_path(path)?;
                if fs::symlink_metadata(&full).is_ok() {
                    fs::remove_file(full).map_err(|error| {
                        AgentError::SupervisorError(format!("failed deleting '{path}': {error}"))
                    })?;
                    applied += 1;
                }
            }
        }
    }

    Ok(applied)
}

fn resolve(project_path: &Path, relative_path: &str) -> Result<PathBuf, AgentError> {
    let relative = validate_relative_path(relative_path)?;
    nexus_sdk::workspace::resolve_path(project_path, relative)
}

fn validate_relative_path(relative_path: &str) -> Result<&Path, AgentError> {
    let relative = Path::new(relative_path);
    if relative.is_absolute() {
        return Err(AgentError::SupervisorError(format!(
            "path '{relative_path}' must be relative"
        )));
    }
    for component in relative.components() {
        match component {
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(AgentError::SupervisorError(format!(
                    "path '{relative_path}' escapes project root"
                )));
            }
            Component::Normal(_) | Component::CurDir => {}
        }
    }
    Ok(relative)
}

#[cfg(all(test, unix))]
mod containment_tests {
    use super::*;

    #[test]
    fn p0_002a_fix_loop_denies_outside_entries_resolving_back_inside() {
        for absolute in [false, true] {
            let temp = tempfile::TempDir::new().unwrap();
            let workspace = temp.path().join("workspace");
            let outside = temp.path().join("outside");
            fs::create_dir(&workspace).unwrap();
            fs::create_dir(&outside).unwrap();
            let target = workspace.join("target.txt");
            let back = outside.join("back");
            fs::write(&target, b"inside evidence").unwrap();
            fs::write(outside.join("evidence.txt"), b"outside evidence").unwrap();
            std::os::unix::fs::symlink(&outside, workspace.join("escape")).unwrap();
            std::os::unix::fs::symlink(&target, &back).unwrap();
            let entries = || {
                let mut names: Vec<_> = fs::read_dir(&outside)
                    .unwrap()
                    .map(|entry| entry.unwrap().file_name())
                    .collect();
                names.sort();
                names
            };
            let before = entries();
            let path = if absolute {
                back.to_str().unwrap().to_string()
            } else {
                "escape/back".into()
            };
            let result = apply_changes(&workspace, &[FileChange::Delete(path)]);
            assert_eq!(entries(), before);
            assert_eq!(fs::read_link(&back).unwrap(), target);
            assert_eq!(fs::read(&target).unwrap(), b"inside evidence");
            assert_eq!(
                fs::read(outside.join("evidence.txt")).unwrap(),
                b"outside evidence"
            );
            assert!(
                matches!(result, Err(AgentError::CapabilityDenied(_))),
                "{result:?}"
            );
        }
    }

    #[test]
    fn p0_002a_fix_loop_unlinks_inside_leaf_without_following_it() {
        let temp = tempfile::TempDir::new().unwrap();
        let workspace = temp.path().join("workspace");
        let outside = temp.path().join("outside");
        fs::create_dir(&workspace).unwrap();
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("target.txt"), b"outside evidence").unwrap();
        for target in ["target.txt", "missing.txt"] {
            let link = workspace.join("link");
            std::os::unix::fs::symlink(outside.join(target), &link).unwrap();
            assert_eq!(
                apply_changes(&workspace, &[FileChange::Delete("link".into())]).unwrap(),
                1
            );
            assert!(fs::symlink_metadata(link).is_err());
            assert_eq!(
                fs::read(outside.join("target.txt")).unwrap(),
                b"outside evidence"
            );
            assert_eq!(fs::read_dir(&outside).unwrap().count(), 1);
        }
    }

    #[test]
    fn p0_002a_fix_loop_deletes_normal_inside_file() {
        let temp = tempfile::TempDir::new().unwrap();
        let target = temp.path().join("file.txt");
        fs::write(&target, b"inside evidence").unwrap();
        assert_eq!(
            apply_changes(temp.path(), &[FileChange::Delete("file.txt".into())]).unwrap(),
            1
        );
        assert!(!target.exists());
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 0);
    }

    #[test]
    fn p0_002_fix_loop_denies_symlink_mutations() {
        for operation in ["create", "modify", "delete"] {
            let temp = tempfile::TempDir::new().unwrap();
            let workspace = temp.path().join("workspace");
            let outside = temp.path().join("outside");
            fs::create_dir_all(&workspace).unwrap();
            fs::create_dir_all(&outside).unwrap();
            fs::write(outside.join("secret.txt"), "outside evidence").unwrap();
            std::os::unix::fs::symlink(&outside, workspace.join("escape")).unwrap();
            let change = match operation {
                "create" => FileChange::Create("escape/new/deep/file.txt".into(), "bad".into()),
                "modify" => {
                    FileChange::Modify("escape/secret.txt".into(), String::new(), "bad".into())
                }
                _ => FileChange::Delete("escape/secret.txt".into()),
            };
            let result = apply_changes(&workspace, &[change]);
            assert_eq!(
                fs::read_to_string(outside.join("secret.txt")).unwrap(),
                "outside evidence"
            );
            assert_eq!(fs::read_dir(&outside).unwrap().count(), 1);
            assert!(
                matches!(result, Err(AgentError::CapabilityDenied(_))),
                "{result:?}"
            );
        }
    }

    #[test]
    fn p0_002_fix_loop_deletes_inside_symlink_without_deleting_target() {
        let temp = tempfile::TempDir::new().unwrap();
        let target = temp.path().join("target.txt");
        fs::write(&target, "inside evidence").unwrap();
        std::os::unix::fs::symlink("target.txt", temp.path().join("alias.txt")).unwrap();
        assert_eq!(
            apply_changes(temp.path(), &[FileChange::Delete("alias.txt".into())]).unwrap(),
            1
        );
        assert!(fs::symlink_metadata(temp.path().join("alias.txt")).is_err());
        assert_eq!(fs::read_to_string(target).unwrap(), "inside evidence");
    }
}
