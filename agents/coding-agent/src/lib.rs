//! Governed coding agent runtime for repository-aware test/fix iterations.

use nexus_kernel::audit::{AuditEvent, AuditTrail, EventType};
use nexus_kernel::autonomy::{AutonomyGuard, AutonomyLevel};
use nexus_kernel::consent::{ApprovalRequest, ConsentRuntime, GovernedOperation};
use nexus_kernel::errors::AgentError;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use uuid::Uuid;

const FUEL_COST_PLAN: u64 = 5;
const FUEL_COST_READ: u64 = 2;
const FUEL_COST_WRITE: u64 = 8;
const FUEL_COST_TEST_RUN: u64 = 15;

const CAP_FS_READ: &str = "fs.read";
const CAP_FS_WRITE: &str = "fs.write";
const CAP_PROCESS_EXEC: &str = "process.exec";

#[derive(Debug, Clone, Deserialize)]
pub struct CodingAgentConfig {
    pub repo_path: String,
    pub objective: String,
    pub test_command: String,
    #[serde(default = "default_max_iterations")]
    pub max_iterations: u32,
    #[serde(default = "default_target_files")]
    pub target_files: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CodingAgentManifest {
    pub name: String,
    pub version: String,
    pub capabilities: Vec<String>,
    pub fuel_budget: u64,
    #[serde(default)]
    pub autonomy_level: Option<u8>,
    #[serde(default)]
    pub consent_policy_path: Option<String>,
    #[serde(default)]
    pub requester_id: Option<String>,
    pub schedule: Option<String>,
    pub llm_model: Option<String>,
    pub config: CodingAgentConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProposedWrite {
    pub path: String,
    pub content: String,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IterationPlan {
    pub summary: String,
    pub read_paths: Vec<String>,
    pub writes: Vec<ProposedWrite>,
    pub run_tests: bool,
}

#[derive(Debug, Clone)]
pub struct PlanningContext {
    pub iteration: u32,
    pub objective: String,
    pub last_test_failure: Option<String>,
    pub target_files: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestExecution {
    pub success: bool,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone)]
pub struct CodingAgentRunReport {
    pub success: bool,
    pub iterations: u32,
    pub fuel_consumed: u64,
    pub fuel_remaining: u64,
    pub modified_files: Vec<String>,
    pub last_test_output: Option<String>,
    pub status: String,
    pub dry_run: bool,
    pub audit_events: Vec<AuditEvent>,
}

pub trait CodingPlanner {
    fn plan(&mut self, context: PlanningContext) -> Result<IterationPlan, AgentError>;
}

pub trait CodingIoProxy {
    fn read_file(&mut self, relative_path: &str) -> Result<String, AgentError>;
    fn write_file(&mut self, relative_path: &str, content: &str) -> Result<(), AgentError>;
    fn run_tests(&mut self, command: &str) -> Result<TestExecution, AgentError>;
}

pub trait ApprovalGate {
    fn approve_write(&mut self, write: &ProposedWrite, iteration: u32) -> bool;
    fn approve_test_run(&mut self, command: &str, iteration: u32) -> bool;
}

pub struct CodingDependencies {
    pub planner: Box<dyn CodingPlanner>,
    pub io: Box<dyn CodingIoProxy>,
    pub approval: Box<dyn ApprovalGate>,
}

impl CodingDependencies {
    pub fn dry_run_defaults(config: &CodingAgentConfig) -> Self {
        Self {
            planner: Box::new(DryRunPlanner::new()),
            io: Box::new(DryRunIo::with_seed_files(config.target_files.as_slice())),
            approval: Box::new(AutoApprovalGate),
        }
    }

    pub fn live_defaults(config: &CodingAgentConfig) -> Result<Self, AgentError> {
        Ok(Self {
            planner: Box::new(BaselinePlanner::new(config.target_files.clone())),
            io: Box::new(LocalCodingIo::new(PathBuf::from(
                config.repo_path.as_str(),
            ))?),
            approval: Box::new(AutoApprovalGate),
        })
    }
}

pub struct CodingAgent {
    manifest: CodingAgentManifest,
    dependencies: CodingDependencies,
    dry_run: bool,
    agent_id: Uuid,
    audit_trail: AuditTrail,
    autonomy_guard: AutonomyGuard,
    consent_runtime: Option<ConsentRuntime>,
    fuel_consumed: u64,
    modified_files: BTreeSet<String>,
}

impl CodingAgent {
    pub fn new(manifest: CodingAgentManifest, dry_run: bool) -> Result<Self, AgentError> {
        let dependencies = if dry_run {
            CodingDependencies::dry_run_defaults(&manifest.config)
        } else {
            CodingDependencies::live_defaults(&manifest.config)?
        };
        Ok(Self::with_dependencies(manifest, dry_run, dependencies))
    }

    pub fn with_dependencies(
        manifest: CodingAgentManifest,
        dry_run: bool,
        dependencies: CodingDependencies,
    ) -> Self {
        Self {
            autonomy_guard: AutonomyGuard::new(AutonomyLevel::from_manifest(
                manifest.autonomy_level,
            )),
            manifest,
            dependencies,
            dry_run,
            agent_id: Uuid::new_v4(),
            audit_trail: AuditTrail::new(),
            consent_runtime: None,
            fuel_consumed: 0,
            modified_files: BTreeSet::new(),
        }
    }

    pub fn run(&mut self) -> Result<CodingAgentRunReport, AgentError> {
        self.audit_trail.append_event(
            self.agent_id,
            EventType::StateChange,
            json!({
                "step": "start",
                "agent": self.manifest.name,
                "version": self.manifest.version,
                "dry_run": self.dry_run,
                "objective": self.manifest.config.objective,
                "max_iterations": self.manifest.config.max_iterations,
                "schedule": self.manifest.schedule,
                "llm_model": self.manifest.llm_model,
                "autonomy_level": self.autonomy_guard.level().as_str(),
            }),
        );

        let mut last_test_output: Option<String> = None;
        let max_iterations = self.manifest.config.max_iterations.max(1);

        for iteration in 1..=max_iterations {
            self.require_operation(
                GovernedOperation::ToolCall,
                format!("plan:{iteration}").as_bytes(),
            )?;
            self.charge_fuel(FUEL_COST_PLAN)?;
            let plan = self.dependencies.planner.plan(PlanningContext {
                iteration,
                objective: self.manifest.config.objective.clone(),
                last_test_failure: last_test_output.clone(),
                target_files: self.manifest.config.target_files.clone(),
            })?;

            self.audit_trail.append_event(
                self.agent_id,
                EventType::LlmCall,
                json!({
                    "step": "plan",
                    "iteration": iteration,
                    "summary": plan.summary,
                    "reads": plan.read_paths.len(),
                    "writes": plan.writes.len(),
                    "run_tests": plan.run_tests,
                }),
            );

            for path in &plan.read_paths {
                self.require_operation(GovernedOperation::ToolCall, path.as_bytes())?;
                self.ensure_capability(CAP_FS_READ)?;
                self.charge_fuel(FUEL_COST_READ)?;
                let content = self.dependencies.io.read_file(path.as_str())?;
                self.audit_trail.append_event(
                    self.agent_id,
                    EventType::ToolCall,
                    json!({
                        "step": "read",
                        "iteration": iteration,
                        "path": path,
                        "bytes": content.len(),
                    }),
                );
            }

            for write in &plan.writes {
                self.require_operation(GovernedOperation::ToolCall, write.path.as_bytes())?;
                self.ensure_capability(CAP_FS_WRITE)?;
                if !self.dependencies.approval.approve_write(write, iteration) {
                    self.audit_trail.append_event(
                        self.agent_id,
                        EventType::UserAction,
                        json!({
                            "step": "approval_denied",
                            "iteration": iteration,
                            "type": "write",
                            "path": write.path,
                        }),
                    );
                    return Err(AgentError::SupervisorError(
                        "write action denied by approval gate".to_string(),
                    ));
                }

                self.charge_fuel(FUEL_COST_WRITE)?;
                self.dependencies
                    .io
                    .write_file(write.path.as_str(), write.content.as_str())?;
                self.modified_files.insert(write.path.clone());

                self.audit_trail.append_event(
                    self.agent_id,
                    EventType::ToolCall,
                    json!({
                        "step": "write",
                        "iteration": iteration,
                        "path": write.path,
                        "summary": write.summary,
                        "bytes": write.content.len(),
                    }),
                );
            }

            if !plan.run_tests {
                continue;
            }

            self.ensure_capability(CAP_PROCESS_EXEC)?;
            let test_command = self.manifest.config.test_command.clone();
            self.require_operation(GovernedOperation::ToolCall, test_command.as_bytes())?;
            if !self
                .dependencies
                .approval
                .approve_test_run(test_command.as_str(), iteration)
            {
                self.audit_trail.append_event(
                    self.agent_id,
                    EventType::UserAction,
                    json!({
                        "step": "approval_denied",
                        "iteration": iteration,
                        "type": "test_run",
                        "command": test_command,
                    }),
                );
                return Err(AgentError::SupervisorError(
                    "test run denied by approval gate".to_string(),
                ));
            }

            self.charge_fuel(FUEL_COST_TEST_RUN)?;
            let test_result = self.dependencies.io.run_tests(test_command.as_str())?;
            self.audit_trail.append_event(
                self.agent_id,
                EventType::ToolCall,
                json!({
                    "step": "test",
                    "iteration": iteration,
                    "command": test_command,
                    "success": test_result.success,
                    "exit_code": test_result.exit_code,
                    "stdout_bytes": test_result.stdout.len(),
                    "stderr_bytes": test_result.stderr.len(),
                }),
            );

            if test_result.success {
                self.audit_trail.append_event(
                    self.agent_id,
                    EventType::StateChange,
                    json!({
                        "step": "complete",
                        "success": true,
                        "iteration": iteration,
                    }),
                );
                return Ok(self.build_report(
                    true,
                    iteration,
                    Some(combine_test_output(&test_result)),
                    "All tests passed".to_string(),
                ));
            }

            last_test_output = Some(combine_test_output(&test_result));
        }

        self.audit_trail.append_event(
            self.agent_id,
            EventType::StateChange,
            json!({
                "step": "complete",
                "success": false,
                "iterations": max_iterations,
            }),
        );

        Ok(self.build_report(
            false,
            max_iterations,
            last_test_output,
            format!("Max iterations reached ({max_iterations}) without a passing test run"),
        ))
    }

    fn build_report(
        &self,
        success: bool,
        iterations: u32,
        last_test_output: Option<String>,
        status: String,
    ) -> CodingAgentRunReport {
        CodingAgentRunReport {
            success,
            iterations,
            fuel_consumed: self.fuel_consumed,
            fuel_remaining: self.manifest.fuel_budget.saturating_sub(self.fuel_consumed),
            modified_files: self.modified_files.iter().cloned().collect(),
            last_test_output,
            status,
            dry_run: self.dry_run,
            audit_events: self.audit_trail.events().to_vec(),
        }
    }

    fn ensure_capability(&self, capability: &str) -> Result<(), AgentError> {
        if self
            .manifest
            .capabilities
            .iter()
            .any(|allowed| allowed == capability)
        {
            return Ok(());
        }

        Err(AgentError::CapabilityDenied(capability.to_string()))
    }

    fn charge_fuel(&mut self, amount: u64) -> Result<(), AgentError> {
        if self.fuel_consumed.saturating_add(amount) > self.manifest.fuel_budget {
            return Err(AgentError::FuelExhausted);
        }
        self.fuel_consumed += amount;
        Ok(())
    }

    fn require_operation(
        &mut self,
        operation: GovernedOperation,
        payload: &[u8],
    ) -> Result<(), AgentError> {
        let agent_id = self.agent_id;
        self.autonomy_guard
            .require_tool_call(self.agent_id, &mut self.audit_trail)
            .map_err(AgentError::from)?;
        self.with_consent_runtime(|runtime, audit_trail| {
            runtime
                .enforce_operation(operation, agent_id, payload, audit_trail)
                .map_err(AgentError::from)
        })
    }

    fn ensure_consent_runtime(&mut self) -> Result<(), AgentError> {
        if self.consent_runtime.is_none() {
            self.consent_runtime = Some(ConsentRuntime::from_manifest(
                self.manifest.consent_policy_path.as_deref(),
                self.manifest.requester_id.as_deref(),
                self.manifest.name.as_str(),
            )?);
        }
        Ok(())
    }

    fn with_consent_runtime<T>(
        &mut self,
        f: impl FnOnce(&mut ConsentRuntime, &mut AuditTrail) -> Result<T, AgentError>,
    ) -> Result<T, AgentError> {
        self.ensure_consent_runtime()?;
        let mut runtime = self.consent_runtime.take().ok_or_else(|| {
            AgentError::SupervisorError("consent runtime was not initialized".to_string())
        })?;
        let result = f(&mut runtime, &mut self.audit_trail);
        self.consent_runtime = Some(runtime);
        result
    }

    pub fn pending_approvals(&self) -> Vec<ApprovalRequest> {
        match &self.consent_runtime {
            Some(runtime) => runtime.pending_requests(),
            None => Vec::new(),
        }
    }

    pub fn approve_request(
        &mut self,
        request_id: &str,
        approver_id: &str,
    ) -> Result<(), AgentError> {
        self.with_consent_runtime(|runtime, audit_trail| {
            runtime
                .approve(request_id, approver_id, audit_trail)
                .map_err(AgentError::from)
        })
    }

    pub fn deny_request(&mut self, request_id: &str, approver_id: &str) -> Result<(), AgentError> {
        self.with_consent_runtime(|runtime, audit_trail| {
            runtime
                .deny(request_id, approver_id, audit_trail)
                .map_err(AgentError::from)
        })
    }
}

pub fn load_manifest(path: &Path) -> Result<CodingAgentManifest, AgentError> {
    let manifest_str = fs::read_to_string(path).map_err(|error| {
        AgentError::ManifestError(format!(
            "unable to read manifest '{}': {error}",
            path.display()
        ))
    })?;
    toml::from_str::<CodingAgentManifest>(manifest_str.as_str())
        .map_err(|error| AgentError::ManifestError(format!("invalid manifest format: {error}")))
}

pub fn run_coding_agent_from_manifest(
    manifest_path: &Path,
    dry_run: bool,
) -> Result<CodingAgentRunReport, AgentError> {
    let manifest = load_manifest(manifest_path)?;
    let mut agent = CodingAgent::new(manifest, dry_run)?;
    agent.run()
}

#[derive(Default)]
pub struct AutoApprovalGate;

impl ApprovalGate for AutoApprovalGate {
    fn approve_write(&mut self, _write: &ProposedWrite, _iteration: u32) -> bool {
        true
    }

    fn approve_test_run(&mut self, _command: &str, _iteration: u32) -> bool {
        true
    }
}

#[derive(Default)]
pub struct DenyAllApprovalGate;

impl ApprovalGate for DenyAllApprovalGate {
    fn approve_write(&mut self, _write: &ProposedWrite, _iteration: u32) -> bool {
        false
    }

    fn approve_test_run(&mut self, _command: &str, _iteration: u32) -> bool {
        false
    }
}

#[derive(Default)]
pub struct DryRunPlanner;

impl DryRunPlanner {
    pub fn new() -> Self {
        Self
    }
}

impl CodingPlanner for DryRunPlanner {
    fn plan(&mut self, context: PlanningContext) -> Result<IterationPlan, AgentError> {
        let patch_path = context
            .target_files
            .first()
            .cloned()
            .unwrap_or_else(|| "src/lib.rs".to_string());

        let mut writes = Vec::new();
        if context.iteration == 1 {
            writes.push(ProposedWrite {
                path: patch_path.clone(),
                content: "pub fn nexus_healthcheck() -> bool {\n    true\n}\n".to_string(),
                summary: "Apply first fix candidate".to_string(),
            });
        } else if context.last_test_failure.is_some() {
            writes.push(ProposedWrite {
                path: patch_path.clone(),
                content: "pub fn nexus_healthcheck() -> bool {\n    true\n}\n\npub fn nexus_status() -> &'static str {\n    \"ok\"\n}\n".to_string(),
                summary: "Refine fix after test feedback".to_string(),
            });
        }

        Ok(IterationPlan {
            summary: format!("dry-run planning iteration {}", context.iteration),
            read_paths: context.target_files,
            writes,
            run_tests: true,
        })
    }
}

pub struct BaselinePlanner {
    target_files: Vec<String>,
}

impl BaselinePlanner {
    pub fn new(target_files: Vec<String>) -> Self {
        Self { target_files }
    }
}

impl CodingPlanner for BaselinePlanner {
    fn plan(&mut self, context: PlanningContext) -> Result<IterationPlan, AgentError> {
        let read_paths = if context.target_files.is_empty() {
            self.target_files.clone()
        } else {
            context.target_files
        };

        Ok(IterationPlan {
            summary: "baseline planner: read code and run test command".to_string(),
            read_paths,
            writes: Vec::new(),
            run_tests: true,
        })
    }
}

pub struct DryRunIo {
    files: BTreeMap<String, String>,
    outcomes: VecDeque<TestExecution>,
}

impl DryRunIo {
    pub fn with_seed_files(seed_files: &[String]) -> Self {
        let mut files = BTreeMap::new();
        for path in seed_files {
            files.insert(
                path.clone(),
                format!("// dry-run seed for {path}\npub fn placeholder() {{}}\n"),
            );
        }
        if files.is_empty() {
            files.insert(
                "src/lib.rs".to_string(),
                "// dry-run default\npub fn placeholder() {}\n".to_string(),
            );
        }

        let outcomes = VecDeque::from(vec![
            TestExecution {
                success: false,
                exit_code: Some(101),
                stdout: "running 12 tests".to_string(),
                stderr: "1 test failed: placeholder behavior".to_string(),
            },
            TestExecution {
                success: true,
                exit_code: Some(0),
                stdout: "running 12 tests\nall tests passed".to_string(),
                stderr: String::new(),
            },
        ]);

        Self { files, outcomes }
    }
}

impl CodingIoProxy for DryRunIo {
    fn read_file(&mut self, relative_path: &str) -> Result<String, AgentError> {
        Ok(self
            .files
            .get(relative_path)
            .cloned()
            .unwrap_or_else(|| format!("// synthetic file for {relative_path}")))
    }

    fn write_file(&mut self, relative_path: &str, content: &str) -> Result<(), AgentError> {
        self.files
            .insert(relative_path.to_string(), content.to_string());
        Ok(())
    }

    fn run_tests(&mut self, _command: &str) -> Result<TestExecution, AgentError> {
        if let Some(next) = self.outcomes.pop_front() {
            return Ok(next);
        }

        Ok(TestExecution {
            success: true,
            exit_code: Some(0),
            stdout: "all tests passed".to_string(),
            stderr: String::new(),
        })
    }
}

pub struct LocalCodingIo {
    repo_root: PathBuf,
}

impl LocalCodingIo {
    pub fn new(repo_root: PathBuf) -> Result<Self, AgentError> {
        if !repo_root.exists() {
            return Err(AgentError::SupervisorError(format!(
                "repo path '{}' does not exist",
                repo_root.display()
            )));
        }

        Ok(Self { repo_root })
    }

    fn resolve_relative_path(&self, relative_path: &str) -> Result<PathBuf, AgentError> {
        let relative = sanitize_relative_path(relative_path)?;
        Ok(self.repo_root.join(relative))
    }
}

impl CodingIoProxy for LocalCodingIo {
    fn read_file(&mut self, relative_path: &str) -> Result<String, AgentError> {
        let path = self.resolve_relative_path(relative_path)?;
        fs::read_to_string(path).map_err(|error| {
            AgentError::SupervisorError(format!("failed reading '{relative_path}': {error}"))
        })
    }

    fn write_file(&mut self, relative_path: &str, content: &str) -> Result<(), AgentError> {
        let path = self.resolve_relative_path(relative_path)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                AgentError::SupervisorError(format!(
                    "failed creating parent for '{relative_path}': {error}"
                ))
            })?;
        }

        fs::write(path, content).map_err(|error| {
            AgentError::SupervisorError(format!("failed writing '{relative_path}': {error}"))
        })
    }

    fn run_tests(&mut self, command: &str) -> Result<TestExecution, AgentError> {
        let output = spawn_shell_command(command, self.repo_root.as_path())?;
        Ok(TestExecution {
            success: output.status.success(),
            exit_code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }
}

fn spawn_shell_command(command: &str, workdir: &Path) -> Result<std::process::Output, AgentError> {
    let mut process = if cfg!(target_os = "windows") {
        let mut cmd = Command::new("cmd");
        cmd.arg("/C").arg(command);
        cmd
    } else {
        let mut cmd = Command::new("sh");
        cmd.arg("-lc").arg(command);
        cmd
    };

    process.current_dir(workdir);
    process.output().map_err(|error| {
        AgentError::SupervisorError(format!(
            "failed executing test command '{command}': {error}"
        ))
    })
}

fn sanitize_relative_path(relative_path: &str) -> Result<PathBuf, AgentError> {
    let path = Path::new(relative_path);
    if path.as_os_str().is_empty() || path.is_absolute() {
        return Err(AgentError::SupervisorError(format!(
            "path '{relative_path}' must be relative"
        )));
    }

    for component in path.components() {
        match component {
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(AgentError::SupervisorError(format!(
                    "path '{relative_path}' escapes repo root"
                )));
            }
            Component::Normal(_) | Component::CurDir => {}
        }
    }

    Ok(path.to_path_buf())
}

fn combine_test_output(test: &TestExecution) -> String {
    let mut parts = Vec::new();
    if !test.stdout.trim().is_empty() {
        parts.push(test.stdout.trim().to_string());
    }
    if !test.stderr.trim().is_empty() {
        parts.push(test.stderr.trim().to_string());
    }
    if parts.is_empty() {
        return "test command produced no output".to_string();
    }
    parts.join("\n")
}

fn default_max_iterations() -> u32 {
    3
}

fn default_target_files() -> Vec<String> {
    vec!["src/lib.rs".to_string()]
}

#[cfg(test)]
mod tests {
    use super::{
        ApprovalGate, AutoApprovalGate, CodingAgent, CodingAgentConfig, CodingAgentManifest,
        CodingDependencies, CodingIoProxy, CodingPlanner, DenyAllApprovalGate, IterationPlan,
        PlanningContext, ProposedWrite, TestExecution,
    };
    use nexus_kernel::errors::AgentError;
    use std::collections::VecDeque;

    fn sample_manifest(fuel_budget: u64, capabilities: &[&str]) -> CodingAgentManifest {
        CodingAgentManifest {
            name: "coding-agent".to_string(),
            version: "2.0.0".to_string(),
            capabilities: capabilities
                .iter()
                .map(|item| (*item).to_string())
                .collect(),
            fuel_budget,
            autonomy_level: Some(1),
            consent_policy_path: None,
            requester_id: None,
            schedule: None,
            llm_model: Some("claude-sonnet-4-5".to_string()),
            config: CodingAgentConfig {
                repo_path: ".".to_string(),
                objective: "Fix tests".to_string(),
                test_command: "cargo test -p sample".to_string(),
                max_iterations: 3,
                target_files: vec!["src/lib.rs".to_string()],
            },
        }
    }

    struct StaticPlanner {
        plans: VecDeque<IterationPlan>,
    }

    impl StaticPlanner {
        fn new(plans: Vec<IterationPlan>) -> Self {
            Self {
                plans: VecDeque::from(plans),
            }
        }
    }

    impl CodingPlanner for StaticPlanner {
        fn plan(&mut self, _context: PlanningContext) -> Result<IterationPlan, AgentError> {
            self.plans
                .pop_front()
                .ok_or_else(|| AgentError::SupervisorError("missing planned iteration".to_string()))
        }
    }

    struct StubIo {
        tests: VecDeque<TestExecution>,
    }

    impl StubIo {
        fn from_tests(tests: Vec<TestExecution>) -> Self {
            Self {
                tests: VecDeque::from(tests),
            }
        }
    }

    impl CodingIoProxy for StubIo {
        fn read_file(&mut self, _relative_path: &str) -> Result<String, AgentError> {
            Ok("fn placeholder() {}".to_string())
        }

        fn write_file(&mut self, _relative_path: &str, _content: &str) -> Result<(), AgentError> {
            Ok(())
        }

        fn run_tests(&mut self, _command: &str) -> Result<TestExecution, AgentError> {
            self.tests.pop_front().ok_or_else(|| {
                AgentError::SupervisorError("missing test execution result".to_string())
            })
        }
    }

    #[test]
    fn test_dry_run_converges_and_emits_audit() {
        let manifest = sample_manifest(
            200,
            &[
                super::CAP_FS_READ,
                super::CAP_FS_WRITE,
                super::CAP_PROCESS_EXEC,
            ],
        );
        let plans = vec![
            IterationPlan {
                summary: "first pass".to_string(),
                read_paths: vec!["src/lib.rs".to_string()],
                writes: vec![ProposedWrite {
                    path: "src/lib.rs".to_string(),
                    content: "pub fn value() -> u32 { 1 }".to_string(),
                    summary: "initial patch".to_string(),
                }],
                run_tests: true,
            },
            IterationPlan {
                summary: "second pass".to_string(),
                read_paths: vec!["src/lib.rs".to_string()],
                writes: vec![ProposedWrite {
                    path: "src/lib.rs".to_string(),
                    content: "pub fn value() -> u32 { 2 }".to_string(),
                    summary: "fix assertion".to_string(),
                }],
                run_tests: true,
            },
        ];
        let io = StubIo::from_tests(vec![
            TestExecution {
                success: false,
                exit_code: Some(101),
                stdout: "running 5 tests".to_string(),
                stderr: "1 failed".to_string(),
            },
            TestExecution {
                success: true,
                exit_code: Some(0),
                stdout: "all passed".to_string(),
                stderr: String::new(),
            },
        ]);
        let deps = CodingDependencies {
            planner: Box::new(StaticPlanner::new(plans)),
            io: Box::new(io),
            approval: Box::new(AutoApprovalGate),
        };

        let mut agent = CodingAgent::with_dependencies(manifest, true, deps);
        let report = agent.run().expect("dry-run should complete");

        assert!(report.success);
        assert_eq!(report.iterations, 2);
        assert_eq!(report.modified_files, vec!["src/lib.rs".to_string()]);
        assert!(
            report.audit_events.len() >= 6,
            "expected audit events for start/plan/read/write/test/complete"
        );
    }

    #[test]
    fn test_capability_denied_for_write() {
        let manifest = sample_manifest(200, &[super::CAP_FS_READ, super::CAP_PROCESS_EXEC]);
        let plan = IterationPlan {
            summary: "attempt write".to_string(),
            read_paths: Vec::new(),
            writes: vec![ProposedWrite {
                path: "src/lib.rs".to_string(),
                content: "pub fn x() {}".to_string(),
                summary: "patch".to_string(),
            }],
            run_tests: false,
        };
        let deps = CodingDependencies {
            planner: Box::new(StaticPlanner::new(vec![plan])),
            io: Box::new(StubIo::from_tests(Vec::new())),
            approval: Box::new(AutoApprovalGate),
        };

        let mut agent = CodingAgent::with_dependencies(manifest, true, deps);
        let result = agent.run();
        assert!(matches!(
            result,
            Err(AgentError::CapabilityDenied(cap)) if cap == super::CAP_FS_WRITE
        ));
    }

    #[test]
    fn test_fuel_exhaustion_is_enforced() {
        let manifest = sample_manifest(
            10,
            &[
                super::CAP_FS_READ,
                super::CAP_FS_WRITE,
                super::CAP_PROCESS_EXEC,
            ],
        );
        let plan = IterationPlan {
            summary: "read and test".to_string(),
            read_paths: vec!["src/lib.rs".to_string()],
            writes: Vec::new(),
            run_tests: true,
        };
        let deps = CodingDependencies {
            planner: Box::new(StaticPlanner::new(vec![plan])),
            io: Box::new(StubIo::from_tests(vec![TestExecution {
                success: true,
                exit_code: Some(0),
                stdout: "ok".to_string(),
                stderr: String::new(),
            }])),
            approval: Box::new(AutoApprovalGate),
        };

        let mut agent = CodingAgent::with_dependencies(manifest, true, deps);
        let result = agent.run();
        assert!(matches!(result, Err(AgentError::FuelExhausted)));
    }

    #[test]
    fn test_approval_gate_blocks_mutating_actions() {
        let manifest = sample_manifest(
            100,
            &[
                super::CAP_FS_READ,
                super::CAP_FS_WRITE,
                super::CAP_PROCESS_EXEC,
            ],
        );
        let plan = IterationPlan {
            summary: "write".to_string(),
            read_paths: Vec::new(),
            writes: vec![ProposedWrite {
                path: "src/lib.rs".to_string(),
                content: "pub fn y() {}".to_string(),
                summary: "patch".to_string(),
            }],
            run_tests: false,
        };
        let deps = CodingDependencies {
            planner: Box::new(StaticPlanner::new(vec![plan])),
            io: Box::new(StubIo::from_tests(Vec::new())),
            approval: Box::new(DenyAllApprovalGate),
        };

        let mut agent = CodingAgent::with_dependencies(manifest, true, deps);
        let result = agent.run();
        assert!(
            matches!(result, Err(AgentError::SupervisorError(message)) if message.contains("denied"))
        );
    }

    struct MixedApprovalGate;

    impl ApprovalGate for MixedApprovalGate {
        fn approve_write(&mut self, _write: &ProposedWrite, _iteration: u32) -> bool {
            true
        }

        fn approve_test_run(&mut self, _command: &str, _iteration: u32) -> bool {
            false
        }
    }

    #[test]
    fn test_approval_gate_blocks_test_run() {
        let manifest = sample_manifest(
            100,
            &[
                super::CAP_FS_READ,
                super::CAP_FS_WRITE,
                super::CAP_PROCESS_EXEC,
            ],
        );
        let plan = IterationPlan {
            summary: "test".to_string(),
            read_paths: Vec::new(),
            writes: Vec::new(),
            run_tests: true,
        };
        let deps = CodingDependencies {
            planner: Box::new(StaticPlanner::new(vec![plan])),
            io: Box::new(StubIo::from_tests(vec![TestExecution {
                success: true,
                exit_code: Some(0),
                stdout: "ok".to_string(),
                stderr: String::new(),
            }])),
            approval: Box::new(MixedApprovalGate),
        };

        let mut agent = CodingAgent::with_dependencies(manifest, true, deps);
        let result = agent.run();
        assert!(
            matches!(result, Err(AgentError::SupervisorError(message)) if message.contains("denied"))
        );
    }
}
