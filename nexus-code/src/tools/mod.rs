//! Tool system — governed execution of file, shell, and search operations.
//!
//! Every tool invocation flows through the governance pipeline:
//! capability check -> fuel reservation -> consent classification -> execute -> audit -> fuel consumption.

pub mod bash;
pub mod file_edit;
pub mod file_read;
pub mod file_write;
pub mod git;
pub mod glob;
mod path_policy;
pub mod project_index;
pub mod screen_analyze;
pub mod screen_capture;
pub mod screen_interact;
pub mod search;
mod search_policy;
pub mod sub_agent_tool;
pub mod test_runner;
mod traversal;
pub mod web_fetch;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Instant;

// ─── Tool Result ───

/// The outcome of a tool execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub success: bool,
    pub output: String,
    /// Wall-clock execution duration in milliseconds.
    #[serde(default)]
    pub duration_ms: u64,
    // Preserve the security category in-process without changing the wire result.
    #[serde(skip)]
    filesystem_denial: Option<(String, String)>,
}

impl ToolResult {
    pub fn success(output: impl Into<String>) -> Self {
        Self {
            success: true,
            output: output.into(),
            duration_ms: 0,
            filesystem_denial: None,
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            success: false,
            output: message.into(),
            duration_ms: 0,
            filesystem_denial: None,
        }
    }

    /// Set the execution duration.
    pub fn with_duration(mut self, duration_ms: u64) -> Self {
        self.duration_ms = duration_ms;
        self
    }

    pub fn is_success(&self) -> bool {
        self.success
    }

    pub fn from_path_error(error: crate::error::NxError) -> Self {
        let mut result = Self::error(error.to_string());
        if let crate::error::NxError::CapabilityDenied { capability, reason } = error {
            result.filesystem_denial = Some((capability, reason));
        }
        result
    }

    pub fn filesystem_error(&self) -> Option<crate::error::NxError> {
        self.filesystem_denial.as_ref().map(|(capability, reason)| {
            crate::error::NxError::CapabilityDenied {
                capability: capability.clone(),
                reason: reason.clone(),
            }
        })
    }

    /// Short summary for audit log (truncated to 200 chars).
    pub fn summary(&self) -> String {
        let prefix = if self.success { "OK" } else { "ERR" };
        let truncated = if self.output.len() > 200 {
            format!("{}...", &self.output[..200])
        } else {
            self.output.clone()
        };
        format!("{} ({}ms): {}", prefix, self.duration_ms, truncated)
    }
}

// ─── Tool Context ───

/// Execution context passed to every tool. Immutable during execution.
#[derive(Debug, Clone)]
pub struct ToolContext {
    /// Mandatory absolute project root supplied by the session's application state.
    /// An empty/relative/invalid root grants no filesystem authority. Desktop/CLI
    /// currently supply process cwd as a compatibility root, not per-agent authority.
    pub working_dir: std::path::PathBuf,
    /// Paths the agent cannot touch (from NEXUSCODE.md blocked_paths).
    pub blocked_paths: Vec<String>,
    /// Max file scope glob pattern (from NEXUSCODE.md max_file_scope).
    pub max_file_scope: Option<String>,
    /// Whether we're in non-interactive mode (headless/print).
    pub non_interactive: bool,
}

impl ToolContext {
    pub fn workspace_root(&self) -> Result<std::path::PathBuf, crate::error::NxError> {
        if !self.working_dir.is_absolute() {
            return Err(Self::workspace_denied(
                "an absolute workspace root is required",
            ));
        }
        let root = self
            .working_dir
            .canonicalize()
            .map_err(|_| Self::workspace_denied("workspace root is unavailable"))?;
        if !root.is_dir() {
            return Err(Self::workspace_denied("workspace root must be a directory"));
        }
        Ok(root)
    }

    pub(crate) fn workspace_denied(reason: &str) -> crate::error::NxError {
        crate::error::NxError::CapabilityDenied {
            capability: "path.workspace".into(),
            reason: reason.into(),
        }
    }

    /// Resolve and validate before use. Optional globs may only narrow this root.
    /// Reuses P0-002A; concurrent namespace replacement/hard links remain outside
    /// this path-validation boundary.
    pub fn resolve_workspace_path(
        &self,
        path: &std::path::Path,
    ) -> Result<std::path::PathBuf, crate::error::NxError> {
        let resolved = self.resolve_contained_path(path)?;
        let root = self.workspace_root()?;

        // Check blocked paths
        for blocked in &self.blocked_paths {
            if path_policy::matches(blocked, &resolved, &self.working_dir, &root) {
                return Err(crate::error::NxError::CapabilityDenied {
                    capability: "path.access".to_string(),
                    reason: "Path is in blocked_paths".into(),
                });
            }
        }

        // Check max_file_scope if set
        if let Some(ref scope) = self.max_file_scope {
            if !path_policy::matches(scope, &resolved, &self.working_dir, &root) {
                return Err(crate::error::NxError::CapabilityDenied {
                    capability: "path.scope".to_string(),
                    reason: "Path is outside max_file_scope".into(),
                });
            }
        }

        Ok(resolved)
    }

    // For bounded enumeration and project-policy discovery only. Ordinary
    // operations must still pass resolve_workspace_path after privacy filtering.
    pub(super) fn resolve_contained_path(
        &self,
        path: &std::path::Path,
    ) -> Result<std::path::PathBuf, crate::error::NxError> {
        let root = self.workspace_root()?;
        nexus_kernel::workspace::resolve_path(&root, path).map_err(|e| match e {
            nexus_kernel::errors::AgentError::CapabilityDenied(_) => Self::workspace_denied(
                "filesystem path is outside the workspace or unsafe to resolve",
            ),
            _ => crate::error::NxError::Io(std::io::Error::other(e.to_string())),
        })
    }

    pub fn check_path_allowed(&self, path: &std::path::Path) -> Result<(), crate::error::NxError> {
        self.resolve_workspace_path(path).map(|_| ())
    }

    pub fn resolve_path(&self, path: &str) -> Result<std::path::PathBuf, crate::error::NxError> {
        self.resolve_workspace_path(std::path::Path::new(path))
    }
}

// ─── Tool Trait ───

/// Every tool in Nexus Code implements this trait.
/// Tools are stateless — all context comes from ToolContext.
/// Tools are NEVER called directly — they are always invoked through
/// `execute_governed()` which enforces the full governance pipeline.
#[async_trait]
pub trait NxTool: Send + Sync {
    /// Unique tool name (e.g., "file_read", "bash").
    /// Must match the name used in Capability::for_tool().
    fn name(&self) -> &str;

    /// Human-readable description for LLM system prompt.
    fn description(&self) -> &str;

    /// JSON Schema describing the tool's input parameters.
    /// This schema is included in the LLM system prompt so the model
    /// knows how to call this tool.
    fn input_schema(&self) -> serde_json::Value;

    /// Execute the tool with the given input.
    /// Called ONLY after the governance pipeline has approved the invocation.
    /// Input is a JSON object matching the input_schema.
    async fn execute(&self, input: serde_json::Value, ctx: &ToolContext) -> ToolResult;

    /// Estimated fuel cost for this invocation.
    /// Used for pre-execution fuel reservation.
    /// Default: 10 units. Override for expensive operations.
    fn estimated_fuel(&self, _input: &serde_json::Value) -> u64 {
        10
    }

    /// Override the capability for this specific invocation.
    /// Default: None (uses Capability::for_tool(self.name())).
    /// Override for tools where capability depends on input (e.g., GitTool).
    fn required_capability(
        &self,
        _input: &serde_json::Value,
    ) -> Option<crate::governance::Capability> {
        None
    }
}

// ─── Tool Registry ───

/// Registry of all available tools.
pub struct ToolRegistry {
    tools: Vec<Box<dyn NxTool>>,
}

impl ToolRegistry {
    /// Create a registry with a specific set of tools.
    pub fn with_tools(tools: Vec<Box<dyn NxTool>>) -> Self {
        Self { tools }
    }

    /// Create a registry with all built-in tools.
    pub fn with_defaults() -> Self {
        let tools: Vec<Box<dyn NxTool>> = vec![
            Box::new(file_read::FileReadTool),
            Box::new(file_write::FileWriteTool),
            Box::new(file_edit::FileEditTool),
            Box::new(bash::BashTool),
            Box::new(search::SearchTool),
            Box::new(glob::GlobTool),
            Box::new(git::GitTool),
            Box::new(test_runner::TestRunnerTool),
            Box::new(sub_agent_tool::SubAgentTool),
            Box::new(project_index::ProjectIndexTool),
            Box::new(web_fetch::WebFetchTool),
        ];
        Self { tools }
    }

    /// Get a tool by name.
    pub fn get(&self, name: &str) -> Option<&dyn NxTool> {
        self.tools
            .iter()
            .find(|t| t.name() == name)
            .map(|t| t.as_ref())
    }

    /// List all tool names.
    pub fn list(&self) -> Vec<&str> {
        self.tools.iter().map(|t| t.name()).collect()
    }

    /// Register an additional tool into the registry.
    pub fn register(&mut self, tool: Box<dyn NxTool>) {
        self.tools.push(tool);
    }

    /// Register the 3 computer use tools (screen_capture, screen_interact, screen_analyze).
    /// Called when --computer-use flag is active or /computer-use on command is issued.
    pub fn register_computer_use_tools(&mut self) {
        self.tools.push(Box::new(screen_capture::ScreenCaptureTool));
        self.tools
            .push(Box::new(screen_interact::ScreenInteractTool));
        self.tools.push(Box::new(screen_analyze::ScreenAnalyzeTool));
    }

    /// Unregister the 3 computer use tools.
    /// Called when /computer-use off command is issued.
    pub fn unregister_computer_use_tools(&mut self) {
        self.tools.retain(|t| {
            !matches!(
                t.name(),
                "screen_capture" | "screen_interact" | "screen_analyze"
            )
        });
    }

    /// Get all tools (for building LLM system prompt).
    pub fn all(&self) -> &[Box<dyn NxTool>] {
        &self.tools
    }

    /// Build the tool descriptions section for the LLM system prompt.
    /// Returns a formatted string describing all available tools and their schemas.
    pub fn build_tool_prompt(&self) -> String {
        let mut sections = Vec::new();
        for tool in &self.tools {
            sections.push(format!(
                "### {}\n{}\n\nParameters:\n```json\n{}\n```",
                tool.name(),
                tool.description(),
                serde_json::to_string_pretty(&tool.input_schema()).unwrap_or_default()
            ));
        }
        sections.join("\n\n")
    }
}

// ─── Governed Execution Pipeline ───

/// Extract a context string from tool input for capability checking.
/// For file tools, this is the file path. For bash, the command.
/// Public alias for use by the REPL.
pub fn extract_tool_context(input: &serde_json::Value, tool_name: &str) -> String {
    extract_context(input, tool_name)
}

/// Summarize tool input for audit logging (truncated to 300 chars).
/// Public alias for use by the REPL.
pub fn summarize_tool_input(input: &serde_json::Value) -> String {
    summarize_input(input)
}

/// Create a tool instance by name. Returns None if unknown.
/// This avoids borrow conflicts when the tool registry and governance
/// are in the same struct — we construct a fresh (stateless) tool instead
/// of borrowing from the registry.
pub fn create_tool(name: &str) -> Option<Box<dyn NxTool>> {
    match name {
        "file_read" => Some(Box::new(file_read::FileReadTool)),
        "file_write" => Some(Box::new(file_write::FileWriteTool)),
        "file_edit" => Some(Box::new(file_edit::FileEditTool)),
        "bash" => Some(Box::new(bash::BashTool)),
        "search" => Some(Box::new(search::SearchTool)),
        "glob" => Some(Box::new(glob::GlobTool)),
        "git" => Some(Box::new(git::GitTool)),
        "test_runner" => Some(Box::new(test_runner::TestRunnerTool)),
        "sub_agent" => Some(Box::new(sub_agent_tool::SubAgentTool)),
        "project_index" => Some(Box::new(project_index::ProjectIndexTool)),
        "web_fetch" => Some(Box::new(web_fetch::WebFetchTool)),
        "screen_capture" => Some(Box::new(screen_capture::ScreenCaptureTool)),
        "screen_interact" => Some(Box::new(screen_interact::ScreenInteractTool)),
        "screen_analyze" => Some(Box::new(screen_analyze::ScreenAnalyzeTool)),
        _ => None,
    }
}

fn extract_context(input: &serde_json::Value, tool_name: &str) -> String {
    match tool_name {
        "file_read" | "file_write" | "file_edit" => input
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or("<unknown path>")
            .to_string(),
        "bash" => input
            .get("command")
            .and_then(|v| v.as_str())
            .unwrap_or("<unknown command>")
            .to_string(),
        "search" | "glob" => input
            .get("pattern")
            .and_then(|v| v.as_str())
            .unwrap_or("<unknown pattern>")
            .to_string(),
        "git" => {
            let subcmd = input
                .get("subcommand")
                .and_then(|v| v.as_str())
                .unwrap_or("status");
            format!("git {}", subcmd)
        }
        "test_runner" => input
            .get("command")
            .and_then(|v| v.as_str())
            .unwrap_or("test")
            .to_string(),
        "sub_agent" => input
            .get("task")
            .and_then(|v| v.as_str())
            .unwrap_or("<sub-agent>")
            .to_string(),
        "project_index" => "project scan".to_string(),
        "screen_capture" => {
            let window = input
                .get("window")
                .and_then(|v| v.as_str())
                .unwrap_or("full screen");
            format!("screenshot: {}", window)
        }
        "screen_interact" => {
            let action = input
                .get("action")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            format!("screen: {}", action)
        }
        "screen_analyze" => {
            let question = input
                .get("question")
                .and_then(|v| v.as_str())
                .unwrap_or("<vision query>");
            format!("vision: {}", question)
        }
        _ => serde_json::to_string(input).unwrap_or_else(|_| "<unknown>".to_string()),
    }
}

/// Summarize tool input for audit logging (truncated to 300 chars).
fn summarize_input(input: &serde_json::Value) -> String {
    let s = serde_json::to_string(input).unwrap_or_default();
    if s.len() > 300 {
        format!("{}...", &s[..300])
    } else {
        s
    }
}

/// Execute a tool through the full governance pipeline.
///
/// This is the ONLY way tools are invoked in Nexus Code.
/// There is no bypass. There is no --dangerously-skip-permissions.
///
/// Pipeline:
/// 1. Capability ACL check (from Capability::for_tool mapping)
/// 2. Fuel reservation (estimated fuel cost)
/// 3. Consent classification (Tier1 auto-approve, Tier2/3 need user consent)
/// 4. Tool execution (timed, scoped to working directory)
/// 5. Audit trail recording (ToolInvocation + ToolResult with duration)
/// 6. Fuel consumption recording
///
/// For Tier1 tools: runs to completion and returns Ok(ToolResult).
/// For Tier2/3 tools: returns Err(NxError::ConsentRequired { request })
///   -> the REPL must present the consent prompt to the user
///   -> then call execute_after_consent() with the user's decision
pub async fn execute_governed(
    tool: &dyn NxTool,
    input: serde_json::Value,
    ctx: &ToolContext,
    kernel: &mut crate::governance::GovernanceKernel,
) -> Result<ToolResult, crate::error::NxError> {
    let tool_name = tool.name();
    let context = extract_context(&input, tool_name);
    let fuel_estimate = tool.estimated_fuel(&input);

    // ── Gate 0: Dynamic capability check (for tools with input-dependent capabilities) ──
    if let Some(cap) = tool.required_capability(&input) {
        kernel.capabilities.check(cap, &context)?;
        kernel
            .audit
            .record(crate::governance::AuditAction::CapabilityCheck {
                capability: cap.as_str().to_string(),
                granted: true,
            });
    }

    // ── Gate 1-3: Capability + Fuel + Consent ──
    let auth_result = kernel.authorize_tool(tool_name, &context, fuel_estimate)?;

    match auth_result {
        crate::governance::AuthorizationResult::Authorized(_decision) => {
            // Tier1: auto-approved, proceed to execution
        }
        crate::governance::AuthorizationResult::ConsentNeeded(request) => {
            // Tier2/3: return to REPL for user consent
            return Err(crate::error::NxError::ConsentRequired { request });
        }
    }

    // ── Gate 4: Execute (timed) ──
    kernel
        .audit
        .record(crate::governance::AuditAction::ToolInvocation {
            tool: tool_name.to_string(),
            args_summary: summarize_input(&input),
        });

    let start = Instant::now();
    let result = tool.execute(input, ctx).await;
    let duration_ms = start.elapsed().as_millis() as u64;
    let result = result.with_duration(duration_ms);

    // ── Gate 5: Audit result (includes duration) ──
    kernel
        .audit
        .record(crate::governance::AuditAction::ToolResult {
            tool: tool_name.to_string(),
            success: result.is_success(),
            summary: result.summary(),
        });

    // ── Gate 6: Fuel consumption ──
    kernel.fuel.consume(
        "tool",
        crate::governance::FuelCost {
            input_tokens: 0,
            output_tokens: 0,
            fuel_units: fuel_estimate,
            estimated_usd: 0.0,
        },
    );
    kernel.fuel.release_reservation(fuel_estimate);

    if let Some(error) = result.filesystem_error() {
        return Err(error);
    }
    Ok(result)
}

/// Execute a tool through the governance pipeline WITH timing instrumentation.
/// Used during benchmarking to measure governance overhead.
pub async fn execute_governed_instrumented(
    tool: &dyn NxTool,
    input: serde_json::Value,
    ctx: &ToolContext,
    kernel: &mut crate::governance::GovernanceKernel,
) -> Result<(ToolResult, crate::governance_metrics::GovernanceTiming), crate::error::NxError> {
    let mut timing = crate::governance_metrics::GovernanceTiming::default();
    let total_start = Instant::now();

    let tool_name = tool.name();
    let context = extract_context(&input, tool_name);
    let fuel_estimate = tool.estimated_fuel(&input);

    // Gate 0: Dynamic capability check (timed)
    let t = Instant::now();
    if let Some(cap) = tool
        .required_capability(&input)
        .or_else(|| crate::governance::Capability::for_tool(tool_name))
    {
        kernel.capabilities.check(cap, &context)?;
    }
    timing.capability_check_us = t.elapsed().as_micros() as u64;

    // Gate 1-3: Authorize (fuel + consent, timed together)
    let t = Instant::now();
    let auth_result = kernel.authorize_tool(tool_name, &context, fuel_estimate)?;
    timing.fuel_reservation_us = t.elapsed().as_micros() as u64;

    let t = Instant::now();
    match auth_result {
        crate::governance::AuthorizationResult::Authorized(_) => {}
        crate::governance::AuthorizationResult::ConsentNeeded(request) => {
            return Err(crate::error::NxError::ConsentRequired { request });
        }
    }
    timing.consent_classification_us = t.elapsed().as_micros() as u64;

    // Gate 4: Audit pre-execution (timed)
    let t = Instant::now();
    kernel
        .audit
        .record(crate::governance::AuditAction::ToolInvocation {
            tool: tool_name.to_string(),
            args_summary: summarize_input(&input),
        });
    let audit_pre_us = t.elapsed().as_micros() as u64;

    // Gate 5: Tool execution (timed)
    let t = Instant::now();
    let result = tool.execute(input, ctx).await;
    timing.tool_execution_us = t.elapsed().as_micros() as u64;
    let result = result.with_duration(timing.tool_execution_us / 1000);

    // Gate 6: Audit post-execution (timed)
    let t = Instant::now();
    kernel
        .audit
        .record(crate::governance::AuditAction::ToolResult {
            tool: tool_name.to_string(),
            success: result.is_success(),
            summary: result.summary(),
        });
    let audit_post_us = t.elapsed().as_micros() as u64;
    timing.audit_recording_us = audit_pre_us + audit_post_us;

    // Gate 7: Fuel consumption (timed)
    let t = Instant::now();
    kernel.fuel.consume(
        "tool",
        crate::governance::FuelCost {
            input_tokens: 0,
            output_tokens: 0,
            fuel_units: fuel_estimate,
            estimated_usd: 0.0,
        },
    );
    kernel.fuel.release_reservation(fuel_estimate);
    timing.fuel_consumption_us = t.elapsed().as_micros() as u64;

    timing.total_us = total_start.elapsed().as_micros() as u64;
    timing.total_governance_overhead_us = timing.total_us.saturating_sub(timing.tool_execution_us);

    if let Some(error) = result.filesystem_error() {
        return Err(error);
    }
    Ok((result, timing))
}

/// Execute a tool AFTER the user has granted consent (Tier2/3).
/// Called by the REPL after presenting a consent prompt and getting approval.
///
/// This function:
/// 1. Finalizes the consent decision (Ed25519-signed)
/// 2. Executes the tool (timed)
/// 3. Records audit + fuel
///
/// If granted=false, finalize_authorization returns Err(ConsentDenied)
/// and the tool is NOT executed.
pub async fn execute_after_consent(
    tool: &dyn NxTool,
    input: serde_json::Value,
    ctx: &ToolContext,
    kernel: &mut crate::governance::GovernanceKernel,
    request: &crate::governance::ConsentRequest,
    granted: bool,
) -> Result<ToolResult, crate::error::NxError> {
    let fuel_estimate = tool.estimated_fuel(&input);

    // Finalize consent (returns Err(ConsentDenied) if denied)
    kernel.finalize_authorization(request, granted, fuel_estimate)?;

    // Execute (timed)
    kernel
        .audit
        .record(crate::governance::AuditAction::ToolInvocation {
            tool: tool.name().to_string(),
            args_summary: summarize_input(&input),
        });

    let start = Instant::now();
    let result = tool.execute(input, ctx).await;
    let duration_ms = start.elapsed().as_millis() as u64;
    let result = result.with_duration(duration_ms);

    // Audit + fuel
    kernel
        .audit
        .record(crate::governance::AuditAction::ToolResult {
            tool: tool.name().to_string(),
            success: result.is_success(),
            summary: result.summary(),
        });

    kernel.fuel.consume(
        "tool",
        crate::governance::FuelCost {
            input_tokens: 0,
            output_tokens: 0,
            fuel_units: fuel_estimate,
            estimated_usd: 0.0,
        },
    );
    kernel.fuel.release_reservation(fuel_estimate);

    if let Some(error) = result.filesystem_error() {
        return Err(error);
    }
    Ok(result)
}
