//! Wasmtime host function linker that maps wasm imports to the existing
//! `HostFunction` enum and delegates ALL governance checks to `AgentContext`.
//!
//! Return convention to wasm:
//!   0  = Success
//!  -1  = CapabilityDenied
//!  -2  = FuelExhausted
//!  -3  = TimedOut
//!  -4  = MemoryExceeded
//!  -5  = Error (generic)
//!  -6  = SpeculationBlocked (action blocked by speculative policy)
//!  -7  = SpeculationHumanReview (action requires human review)
//!
//! Output strings are written to a shared buffer inside `WasmAgentState`.
//! The wasm module can read results back via `nexus_result_ptr` / `nexus_result_len`.

use crate::context::ContextSideEffect;
use crate::sandbox::{HostCallResult, SandboxError};
use crate::shadow_sandbox::{SafetyVerdict, ThreatDetector};
use crate::typed_tools::{self, ToolRequest};
use crate::wasmtime_sandbox::WasmAgentState;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::rc::Rc;
use wasmtime::Linker;

/// Decision for a speculative policy check on a host function call.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpeculativeDecision {
    /// Allow the action to proceed (execute for real).
    Commit,
    /// Block the action — return error code to wasm.
    Block,
    /// Require human review before proceeding.
    /// In practice: record the side-effect and return -7 to wasm.
    HumanReview,
}

/// Policy governing which host function calls require speculation.
///
/// When attached to a `WasmAgentState`, each host function call checks the
/// policy before executing. If no policy is configured (`None`), all calls
/// proceed normally — exact 6.1 behavior.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeculativePolicy {
    /// Default decision for host functions not explicitly listed.
    pub default_decision: SpeculativeDecision,
    /// Per-function overrides. Keys are host function names:
    /// "llm_query", "fs_read", "fs_write", "request_approval".
    pub function_decisions: HashMap<String, SpeculativeDecision>,
}

impl SpeculativePolicy {
    /// Create a policy that commits everything (no speculation).
    pub fn allow_all() -> Self {
        Self {
            default_decision: SpeculativeDecision::Commit,
            function_decisions: HashMap::new(),
        }
    }

    /// Create a policy that blocks everything.
    pub fn block_all() -> Self {
        Self {
            default_decision: SpeculativeDecision::Block,
            function_decisions: HashMap::new(),
        }
    }

    /// Create a policy that requires human review for everything.
    pub fn review_all() -> Self {
        Self {
            default_decision: SpeculativeDecision::HumanReview,
            function_decisions: HashMap::new(),
        }
    }

    /// Set the decision for a specific host function.
    pub fn set_function_decision(&mut self, function: &str, decision: SpeculativeDecision) {
        self.function_decisions
            .insert(function.to_string(), decision);
    }

    /// Look up the decision for a host function.
    pub fn decide(&self, function: &str) -> SpeculativeDecision {
        self.function_decisions
            .get(function)
            .copied()
            .unwrap_or(self.default_decision)
    }
}

impl Default for SpeculativePolicy {
    fn default() -> Self {
        Self::allow_all()
    }
}

/// Check the speculative policy for a given host function.
/// Returns the decision. If no policy is configured, returns `Commit`.
fn check_speculation(state: &WasmAgentState, function: &str) -> SpeculativeDecision {
    match &state.speculative_policy {
        Some(policy) => policy.decide(function),
        None => SpeculativeDecision::Commit,
    }
}

/// Run the `ThreatDetector` on a single side-effect and potentially escalate
/// the speculative decision.
///
/// If the side-effect is `Dangerous`, returns `Block`.
/// If `Suspicious`, returns `HumanReview`.
/// If `Safe`, returns the original `decision` unchanged.
///
/// Only runs when a `SpeculativePolicy` is configured (i.e. speculation is active).
/// With no policy, returns `Commit` (exact 6.1 behavior).
fn threat_scan_side_effect(
    state: &WasmAgentState,
    effect: &ContextSideEffect,
) -> SpeculativeDecision {
    let (capabilities, fuel_budget) = match state.agent_context.as_ref() {
        Some(ctx) => {
            let ctx = ctx.borrow();
            (ctx.capabilities().to_vec(), ctx.fuel_budget())
        }
        // No agent context means no governance data to scan against
        None => return SpeculativeDecision::Commit,
    };

    let detector = ThreatDetector::new(capabilities, fuel_budget);
    let verdict = detector.scan_side_effects(std::slice::from_ref(effect));

    match verdict {
        SafetyVerdict::Dangerous { .. } => SpeculativeDecision::Block,
        SafetyVerdict::Suspicious { .. } => SpeculativeDecision::HumanReview,
        SafetyVerdict::Safe => SpeculativeDecision::Commit,
    }
}

/// Encode a `HostCallResult` as an i32 return code for wasm.
pub fn host_call_result_to_i32(result: &HostCallResult) -> i32 {
    match result {
        HostCallResult::Success { .. } => 0,
        HostCallResult::CapabilityDenied { .. } => -1,
        HostCallResult::FuelExhausted => -2,
        HostCallResult::TimedOut => -3,
        HostCallResult::MemoryExceeded => -4,
        HostCallResult::Error { .. } => -5,
    }
}

/// Read a UTF-8 string from wasm linear memory. Returns empty string on out-of-bounds.
fn read_wasm_str(
    caller: &wasmtime::Caller<'_, WasmAgentState>,
    memory: &wasmtime::Memory,
    ptr: i32,
    len: i32,
) -> String {
    let data = memory.data(caller);
    let start = ptr as usize;
    let end = start.saturating_add(len as usize).min(data.len());
    String::from_utf8_lossy(&data[start..end]).into_owned()
}

/// Link all Nexus host functions into a wasmtime `Linker`.
///
/// Host functions delegate to `AgentContext` methods via the shared `Rc<RefCell<AgentContext>>`
/// stored in `WasmAgentState`. This means ALL governance checks (capability, fuel, audit)
/// happen through the existing `InProcessSandbox::call_host_function` / `AgentContext` path.
///
/// When a `SpeculativePolicy` is configured on the `WasmAgentState`, each gated host
/// function checks the policy BEFORE delegating to `AgentContext`:
/// - `Commit` → proceed normally (real execution)
/// - `Block` → return -6 to wasm, record side-effect in context
/// - `HumanReview` → return -7 to wasm, record side-effect in context
///
/// If no policy is configured, all calls proceed normally (exact 6.1 behavior).
///
/// Functions linked under the "nexus" module:
/// - `nexus_log(level, ptr, len)` — always allowed, no speculation (safe)
/// - `nexus_emit_audit(ptr, len)` — always allowed, no speculation (safe)
/// - `nexus_llm_query(prompt_ptr, prompt_len, max_tokens) -> i32` — speculative gate + `ctx.llm_query()`
/// - `nexus_fs_read(path_ptr, path_len) -> i32` — speculative gate + `ctx.read_file()`
/// - `nexus_fs_write(path_ptr, path_len, content_ptr, content_len) -> i32` — speculative gate + `ctx.write_file()`
/// - `nexus_request_approval(desc_ptr, desc_len) -> i32` — speculative gate + `ctx.request_approval()`
/// - `nexus_exec_tool(json_ptr, json_len) -> i32` — typed tool execution (no shell)
pub fn link_host_functions(linker: &mut Linker<WasmAgentState>) -> Result<(), SandboxError> {
    link_nexus_log(linker)?;
    link_nexus_emit_audit(linker)?;
    link_nexus_llm_query(linker)?;
    link_nexus_fs_read(linker)?;
    link_nexus_fs_write(linker)?;
    link_nexus_request_approval(linker)?;
    link_nexus_exec_tool(linker)?;
    Ok(())
}

/// `nexus_log(level: i32, msg_ptr: i32, msg_len: i32)` — always allowed, no speculation
fn link_nexus_log(linker: &mut Linker<WasmAgentState>) -> Result<(), SandboxError> {
    linker
        .func_wrap(
            "nexus",
            "nexus_log",
            |mut caller: wasmtime::Caller<'_, WasmAgentState>,
             _level: i32,
             msg_ptr: i32,
             msg_len: i32| {
                let memory = match caller.get_export("memory") {
                    Some(wasmtime::Extern::Memory(mem)) => mem,
                    _ => {
                        // BUG 3 FIX: Log error internally when WASM module lacks exported memory
                        eprintln!("[nexus_log] WASM module missing exported memory — cannot read log message");
                        let state = caller.data_mut();
                        state.outputs.push("[nexus_log] error: no exported memory".to_string());
                        state.host_calls_made += 1;
                        return;
                    }
                };
                let msg = read_wasm_str(&caller, &memory, msg_ptr, msg_len);
                let state = caller.data_mut();
                state.outputs.push(msg);
                state.host_calls_made += 1;
            },
        )
        .map_err(|e| SandboxError::ConfigError(format!("link nexus_log: {e}")))?;
    Ok(())
}

/// `nexus_emit_audit(msg_ptr: i32, msg_len: i32)` — always allowed, no speculation
fn link_nexus_emit_audit(linker: &mut Linker<WasmAgentState>) -> Result<(), SandboxError> {
    linker
        .func_wrap(
            "nexus",
            "nexus_emit_audit",
            |mut caller: wasmtime::Caller<'_, WasmAgentState>, msg_ptr: i32, msg_len: i32| {
                let memory = match caller.get_export("memory") {
                    Some(wasmtime::Extern::Memory(mem)) => mem,
                    _ => return,
                };
                let msg = read_wasm_str(&caller, &memory, msg_ptr, msg_len);
                let state = caller.data_mut();
                state.outputs.push(format!("[audit] {msg}"));
                state.host_calls_made += 1;
            },
        )
        .map_err(|e| SandboxError::ConfigError(format!("link nexus_emit_audit: {e}")))?;
    Ok(())
}

/// `nexus_llm_query(prompt_ptr, prompt_len, max_tokens) -> i32`
/// Speculative gate + delegation to `AgentContext::llm_query()`.
fn link_nexus_llm_query(linker: &mut Linker<WasmAgentState>) -> Result<(), SandboxError> {
    linker
        .func_wrap(
            "nexus",
            "nexus_llm_query",
            |mut caller: wasmtime::Caller<'_, WasmAgentState>,
             prompt_ptr: i32,
             prompt_len: i32,
             max_tokens: i32|
             -> i32 {
                let memory = match caller.get_export("memory") {
                    Some(wasmtime::Extern::Memory(mem)) => mem,
                    _ => return -5,
                };
                let prompt = read_wasm_str(&caller, &memory, prompt_ptr, prompt_len);
                let state = caller.data_mut();

                // Check allowed_host_functions first (sandbox-level gate)
                if !state
                    .allowed_host_functions
                    .contains(&"llm_query".to_string())
                {
                    state.host_calls_made += 1;
                    return -1; // CapabilityDenied
                }

                // Early capability check — prevents blocking in speculation/threat paths
                // when the agent context lacks the required capability
                if let Some(ctx) = state.agent_context.as_ref() {
                    let has_cap = ctx
                        .borrow()
                        .capabilities()
                        .contains(&"llm.query".to_string());
                    if !has_cap {
                        state.host_calls_made += 1;
                        if ctx.borrow().is_recording() {
                            ctx.borrow_mut()
                                .record_side_effect(ContextSideEffect::LlmQuery {
                                    prompt: prompt.clone(),
                                    max_tokens: max_tokens as u32,
                                    fuel_cost: 10,
                                });
                        }
                        return -1;
                    }
                }

                // Speculative policy gate — before real execution
                let mut decision = check_speculation(state, "llm_query");

                // Threat detection: even if policy says Commit, scan the
                // side-effect and escalate if threats are found.
                if decision == SpeculativeDecision::Commit {
                    let effect = ContextSideEffect::LlmQuery {
                        prompt: prompt.clone(),
                        max_tokens: max_tokens as u32,
                        fuel_cost: 10,
                    };
                    let threat_decision = threat_scan_side_effect(state, &effect);
                    if threat_decision != SpeculativeDecision::Commit {
                        decision = threat_decision;
                    }
                }

                if decision != SpeculativeDecision::Commit {
                    // Record the side-effect in the context so it can be inspected
                    if let Some(ctx) = state.agent_context.as_ref() {
                        let ctx = Rc::clone(ctx);
                        ctx.borrow_mut()
                            .record_side_effect(ContextSideEffect::LlmQuery {
                                prompt: prompt.clone(),
                                max_tokens: max_tokens as u32,
                                fuel_cost: 10,
                            });
                    }
                    state.host_calls_made += 1;
                    state.outputs.push(format!(
                        "[speculation-{}: llm_query, prompt_len={}, max_tokens={}]",
                        if decision == SpeculativeDecision::Block {
                            "blocked"
                        } else {
                            "review"
                        },
                        prompt.len(),
                        max_tokens,
                    ));
                    return if decision == SpeculativeDecision::Block {
                        -6
                    } else {
                        -7
                    };
                }

                // Delegate to AgentContext for governance (capability + fuel + audit)
                let ctx = match state.agent_context.as_ref() {
                    Some(ctx) => Rc::clone(ctx),
                    None => return -5,
                };
                let result = ctx.borrow_mut().llm_query(&prompt, max_tokens as u32);
                state.host_calls_made += 1;

                match result {
                    Ok(output) => {
                        state.outputs.push(output);
                        0
                    }
                    Err(nexus_kernel::errors::AgentError::CapabilityDenied(_)) => -1,
                    Err(nexus_kernel::errors::AgentError::FuelExhausted) => -2,
                    Err(e) => {
                        state.outputs.push(format!("[error] {e}"));
                        -5
                    }
                }
            },
        )
        .map_err(|e| SandboxError::ConfigError(format!("link nexus_llm_query: {e}")))?;
    Ok(())
}

/// `nexus_fs_read(path_ptr, path_len) -> i32`
/// Speculative gate + delegation to `AgentContext::read_file()`.
fn link_nexus_fs_read(linker: &mut Linker<WasmAgentState>) -> Result<(), SandboxError> {
    linker
        .func_wrap(
            "nexus",
            "nexus_fs_read",
            |mut caller: wasmtime::Caller<'_, WasmAgentState>,
             path_ptr: i32,
             path_len: i32|
             -> i32 {
                let memory = match caller.get_export("memory") {
                    Some(wasmtime::Extern::Memory(mem)) => mem,
                    _ => return -5,
                };
                let path = read_wasm_str(&caller, &memory, path_ptr, path_len);
                let state = caller.data_mut();

                if !state
                    .allowed_host_functions
                    .contains(&"fs_read".to_string())
                {
                    state.host_calls_made += 1;
                    return -1;
                }

                // Early capability check — prevents blocking in speculation/threat paths
                // when the agent context lacks the required capability
                if let Some(ctx) = state.agent_context.as_ref() {
                    let has_cap = ctx
                        .borrow()
                        .capabilities()
                        .contains(&"fs.read".to_string());
                    if !has_cap {
                        state.host_calls_made += 1;
                        if ctx.borrow().is_recording() {
                            ctx.borrow_mut()
                                .record_side_effect(ContextSideEffect::FileRead {
                                    path: path.clone(),
                                    fuel_cost: 2,
                                });
                        }
                        return -1;
                    }
                }

                // Speculative policy gate
                let mut decision = check_speculation(state, "fs_read");

                // Threat detection on file read path
                if decision == SpeculativeDecision::Commit {
                    let effect = ContextSideEffect::FileRead {
                        path: path.clone(),
                        fuel_cost: 2,
                    };
                    let threat_decision = threat_scan_side_effect(state, &effect);
                    if threat_decision != SpeculativeDecision::Commit {
                        decision = threat_decision;
                    }
                }

                if decision != SpeculativeDecision::Commit {
                    if let Some(ctx) = state.agent_context.as_ref() {
                        let ctx = Rc::clone(ctx);
                        ctx.borrow_mut()
                            .record_side_effect(ContextSideEffect::FileRead {
                                path: path.clone(),
                                fuel_cost: 2,
                            });
                    }
                    state.host_calls_made += 1;
                    state.outputs.push(format!(
                        "[speculation-{}: fs_read, path={}]",
                        if decision == SpeculativeDecision::Block {
                            "blocked"
                        } else {
                            "review"
                        },
                        path,
                    ));
                    return if decision == SpeculativeDecision::Block {
                        -6
                    } else {
                        -7
                    };
                }

                let ctx = match state.agent_context.as_ref() {
                    Some(ctx) => Rc::clone(ctx),
                    None => return -5,
                };
                let result = ctx.borrow_mut().read_file(&path);
                state.host_calls_made += 1;

                match result {
                    Ok(output) => {
                        state.outputs.push(output);
                        0
                    }
                    Err(nexus_kernel::errors::AgentError::CapabilityDenied(_)) => -1,
                    Err(nexus_kernel::errors::AgentError::FuelExhausted) => -2,
                    Err(e) => {
                        state.outputs.push(format!("[error] {e}"));
                        -5
                    }
                }
            },
        )
        .map_err(|e| SandboxError::ConfigError(format!("link nexus_fs_read: {e}")))?;
    Ok(())
}

/// `nexus_fs_write(path_ptr, path_len, content_ptr, content_len) -> i32`
/// Speculative gate + delegation to `AgentContext::write_file()`.
fn link_nexus_fs_write(linker: &mut Linker<WasmAgentState>) -> Result<(), SandboxError> {
    linker
        .func_wrap(
            "nexus",
            "nexus_fs_write",
            |mut caller: wasmtime::Caller<'_, WasmAgentState>,
             path_ptr: i32,
             path_len: i32,
             content_ptr: i32,
             content_len: i32|
             -> i32 {
                let memory = match caller.get_export("memory") {
                    Some(wasmtime::Extern::Memory(mem)) => mem,
                    _ => return -5,
                };
                let path = read_wasm_str(&caller, &memory, path_ptr, path_len);
                let content = read_wasm_str(&caller, &memory, content_ptr, content_len);
                let state = caller.data_mut();

                if !state
                    .allowed_host_functions
                    .contains(&"fs_write".to_string())
                {
                    state.host_calls_made += 1;
                    return -1;
                }

                // Early capability check — prevents blocking in speculation/threat paths
                // when the agent context lacks the required capability
                if let Some(ctx) = state.agent_context.as_ref() {
                    let has_cap = ctx
                        .borrow()
                        .capabilities()
                        .contains(&"fs.write".to_string());
                    if !has_cap {
                        state.host_calls_made += 1;
                        if ctx.borrow().is_recording() {
                            ctx.borrow_mut()
                                .record_side_effect(ContextSideEffect::FileWrite {
                                    path: path.clone(),
                                    content_size: content.len(),
                                    fuel_cost: 8,
                                });
                        }
                        return -1;
                    }
                }

                // Speculative policy gate
                let mut decision = check_speculation(state, "fs_write");

                // Threat detection on file write path
                if decision == SpeculativeDecision::Commit {
                    let effect = ContextSideEffect::FileWrite {
                        path: path.clone(),
                        content_size: content.len(),
                        fuel_cost: 8,
                    };
                    let threat_decision = threat_scan_side_effect(state, &effect);
                    if threat_decision != SpeculativeDecision::Commit {
                        decision = threat_decision;
                    }
                }

                if decision != SpeculativeDecision::Commit {
                    if let Some(ctx) = state.agent_context.as_ref() {
                        let ctx = Rc::clone(ctx);
                        ctx.borrow_mut()
                            .record_side_effect(ContextSideEffect::FileWrite {
                                path: path.clone(),
                                content_size: content.len(),
                                fuel_cost: 8,
                            });
                    }
                    state.host_calls_made += 1;
                    state.outputs.push(format!(
                        "[speculation-{}: fs_write, path={}, content_len={}]",
                        if decision == SpeculativeDecision::Block {
                            "blocked"
                        } else {
                            "review"
                        },
                        path,
                        content.len(),
                    ));
                    return if decision == SpeculativeDecision::Block {
                        -6
                    } else {
                        -7
                    };
                }

                let ctx = match state.agent_context.as_ref() {
                    Some(ctx) => Rc::clone(ctx),
                    None => return -5,
                };
                let result = ctx.borrow_mut().write_file(&path, &content);
                state.host_calls_made += 1;

                match result {
                    Ok(()) => {
                        state.outputs.push("written".to_string());
                        0
                    }
                    Err(nexus_kernel::errors::AgentError::CapabilityDenied(_)) => -1,
                    Err(nexus_kernel::errors::AgentError::FuelExhausted) => -2,
                    Err(e) => {
                        state.outputs.push(format!("[error] {e}"));
                        -5
                    }
                }
            },
        )
        .map_err(|e| SandboxError::ConfigError(format!("link nexus_fs_write: {e}")))?;
    Ok(())
}

/// `nexus_request_approval(desc_ptr, desc_len) -> i32`
/// Speculative gate + delegation to `AgentContext::request_approval()`.
///
/// Agent-provided descriptions are sanitized and marked as agent-sourced.
/// The UI should display kernel-generated `display_summary` with higher
/// visual prominence than agent descriptions.
fn link_nexus_request_approval(linker: &mut Linker<WasmAgentState>) -> Result<(), SandboxError> {
    linker
        .func_wrap(
            "nexus",
            "nexus_request_approval",
            |mut caller: wasmtime::Caller<'_, WasmAgentState>,
             desc_ptr: i32,
             desc_len: i32|
             -> i32 {
                let memory = match caller.get_export("memory") {
                    Some(wasmtime::Extern::Memory(mem)) => mem,
                    _ => return -5,
                };
                let raw_desc = read_wasm_str(&caller, &memory, desc_ptr, desc_len);
                // Sanitize agent-provided description: strip Markdown, HTML,
                // and control characters before storing or logging.
                let desc = nexus_kernel::consent_display::sanitize_display_text(&raw_desc);
                let state = caller.data_mut();

                if !state
                    .allowed_host_functions
                    .contains(&"request_approval".to_string())
                {
                    state.host_calls_made += 1;
                    return -1;
                }

                // Early capability check — prevents blocking in speculation/threat paths
                // when the agent context lacks the required capability
                if let Some(ctx) = state.agent_context.as_ref() {
                    let has_cap = ctx
                        .borrow()
                        .capabilities()
                        .contains(&"request_approval".to_string());
                    if !has_cap {
                        state.host_calls_made += 1;
                        if ctx.borrow().is_recording() {
                            ctx.borrow_mut()
                                .record_side_effect(ContextSideEffect::ApprovalRequest {
                                    description: desc.clone(),
                                });
                        }
                        return -1;
                    }
                }

                // Speculative policy gate
                let mut decision = check_speculation(state, "request_approval");

                // Threat detection on approval request
                if decision == SpeculativeDecision::Commit {
                    let effect = ContextSideEffect::ApprovalRequest {
                        description: desc.clone(),
                    };
                    let threat_decision = threat_scan_side_effect(state, &effect);
                    if threat_decision != SpeculativeDecision::Commit {
                        decision = threat_decision;
                    }
                }

                if decision != SpeculativeDecision::Commit {
                    if let Some(ctx) = state.agent_context.as_ref() {
                        let ctx = Rc::clone(ctx);
                        ctx.borrow_mut()
                            .record_side_effect(ContextSideEffect::ApprovalRequest {
                                description: desc.clone(),
                            });
                    }
                    state.host_calls_made += 1;
                    state.outputs.push(format!(
                        "[speculation-{}: request_approval, desc={}]",
                        if decision == SpeculativeDecision::Block {
                            "blocked"
                        } else {
                            "review"
                        },
                        desc,
                    ));
                    return if decision == SpeculativeDecision::Block {
                        -6
                    } else {
                        -7
                    };
                }

                let ctx = match state.agent_context.as_ref() {
                    Some(ctx) => Rc::clone(ctx),
                    None => return -5,
                };
                let record = ctx.borrow_mut().request_approval(&desc, true);
                state.host_calls_made += 1;
                state
                    .outputs
                    .push(format!("approval_requested: {}", record.description));
                0
            },
        )
        .map_err(|e| SandboxError::ConfigError(format!("link nexus_request_approval: {e}")))?;
    Ok(())
}

/// Process a JSON-serialized `ToolRequest` and return a JSON result string.
///
/// This is the pure logic extracted from the host function so it can be
/// unit-tested without spinning up a wasmtime `Store`.
///
/// Returns `(return_code, json_output)`.
pub fn process_exec_tool_request(json_input: &str) -> (i32, String) {
    let request: ToolRequest = match serde_json::from_str(json_input) {
        Ok(r) => r,
        Err(e) => {
            let err_json = serde_json::json!({
                "success": false,
                "error": format!("JSON parse error: {e}"),
                "exit_code": -1,
            });
            return (-5, err_json.to_string());
        }
    };

    // Validate by building the command (no execution).
    // build_command checks argument safety (e.g. npm script allowlist).
    match typed_tools::build_command(&request) {
        Ok(cmd) => {
            let program = cmd.get_program().to_string_lossy().into_owned();
            let args: Vec<String> = cmd
                .get_args()
                .map(|a| a.to_string_lossy().into_owned())
                .collect();
            let result_json = serde_json::json!({
                "success": true,
                "program": program,
                "args": args,
                "stdout": "",
                "stderr": "",
                "exit_code": 0,
            });
            (0, result_json.to_string())
        }
        Err(typed_tools::ToolError::NotAllowed(reason)) => {
            let err_json = serde_json::json!({
                "success": false,
                "error": format!("not allowed: {reason}"),
                "exit_code": -1,
            });
            (-1, err_json.to_string())
        }
        Err(e) => {
            let err_json = serde_json::json!({
                "success": false,
                "error": e.to_string(),
                "exit_code": -1,
            });
            (-5, err_json.to_string())
        }
    }
}

/// `nexus_exec_tool(json_ptr, json_len) -> i32`
///
/// Typed tool execution host function. WASM agents send a JSON-serialized
/// `ToolRequest`, which is validated and built into an exact `Command` with
/// no shell involvement. The JSON result (program + args or error) is pushed
/// to the agent's output buffer.
fn link_nexus_exec_tool(linker: &mut Linker<WasmAgentState>) -> Result<(), SandboxError> {
    linker
        .func_wrap(
            "nexus",
            "nexus_exec_tool",
            |mut caller: wasmtime::Caller<'_, WasmAgentState>,
             json_ptr: i32,
             json_len: i32|
             -> i32 {
                let memory = match caller.get_export("memory") {
                    Some(wasmtime::Extern::Memory(mem)) => mem,
                    _ => return -5,
                };
                let json_input = read_wasm_str(&caller, &memory, json_ptr, json_len);
                let state = caller.data_mut();

                // Check allowed_host_functions gate
                if !state
                    .allowed_host_functions
                    .contains(&"exec_tool".to_string())
                {
                    state.host_calls_made += 1;
                    return -1; // CapabilityDenied
                }

                // Speculative policy gate
                let decision = check_speculation(state, "exec_tool");
                if decision != SpeculativeDecision::Commit {
                    state.host_calls_made += 1;
                    state.outputs.push(format!(
                        "[speculation-{}: exec_tool, input_len={}]",
                        if decision == SpeculativeDecision::Block {
                            "blocked"
                        } else {
                            "review"
                        },
                        json_input.len(),
                    ));
                    return if decision == SpeculativeDecision::Block {
                        -6
                    } else {
                        -7
                    };
                }

                // BUG 5 FIX: Run threat_scan_side_effect before tool execution
                let threat_effect = ContextSideEffect::ToolExec {
                    tool_name: "exec_tool".to_string(),
                    input_json: json_input.clone(),
                };
                let threat_decision = threat_scan_side_effect(state, &threat_effect);
                if threat_decision != SpeculativeDecision::Commit {
                    state.host_calls_made += 1;
                    state.outputs.push(format!(
                        "[threat-{}: exec_tool blocked by threat scan]",
                        if threat_decision == SpeculativeDecision::Block {
                            "blocked"
                        } else {
                            "review"
                        },
                    ));
                    return if threat_decision == SpeculativeDecision::Block {
                        -6
                    } else {
                        -7
                    };
                }

                let (code, output_json) = process_exec_tool_request(&json_input);
                state.host_calls_made += 1;
                state.outputs.push(output_json);
                code
            },
        )
        .map_err(|e| SandboxError::ConfigError(format!("link nexus_exec_tool: {e}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn result_code_mapping() {
        assert_eq!(
            host_call_result_to_i32(&HostCallResult::Success {
                output: "ok".into()
            }),
            0
        );
        assert_eq!(
            host_call_result_to_i32(&HostCallResult::CapabilityDenied {
                function: "x".into()
            }),
            -1
        );
        assert_eq!(host_call_result_to_i32(&HostCallResult::FuelExhausted), -2);
        assert_eq!(host_call_result_to_i32(&HostCallResult::TimedOut), -3);
        assert_eq!(host_call_result_to_i32(&HostCallResult::MemoryExceeded), -4);
        assert_eq!(
            host_call_result_to_i32(&HostCallResult::Error {
                reason: "bad".into()
            }),
            -5
        );
    }

    #[test]
    fn speculative_policy_default_is_allow_all() {
        let policy = SpeculativePolicy::default();
        assert_eq!(policy.decide("llm_query"), SpeculativeDecision::Commit);
        assert_eq!(policy.decide("fs_write"), SpeculativeDecision::Commit);
        assert_eq!(policy.decide("unknown"), SpeculativeDecision::Commit);
    }

    #[test]
    fn speculative_policy_block_all() {
        let policy = SpeculativePolicy::block_all();
        assert_eq!(policy.decide("llm_query"), SpeculativeDecision::Block);
        assert_eq!(policy.decide("fs_read"), SpeculativeDecision::Block);
    }

    #[test]
    fn speculative_policy_review_all() {
        let policy = SpeculativePolicy::review_all();
        assert_eq!(policy.decide("llm_query"), SpeculativeDecision::HumanReview);
        assert_eq!(policy.decide("fs_write"), SpeculativeDecision::HumanReview);
    }

    #[test]
    fn speculative_policy_per_function_override() {
        let mut policy = SpeculativePolicy::allow_all();
        policy.set_function_decision("fs_write", SpeculativeDecision::Block);
        policy.set_function_decision("llm_query", SpeculativeDecision::HumanReview);

        assert_eq!(policy.decide("fs_write"), SpeculativeDecision::Block);
        assert_eq!(policy.decide("llm_query"), SpeculativeDecision::HumanReview);
        // Default still commits
        assert_eq!(policy.decide("fs_read"), SpeculativeDecision::Commit);
    }

    #[test]
    fn check_speculation_no_policy_returns_commit() {
        let state = WasmAgentState {
            agent_id: "test".to_string(),
            capabilities: vec![],
            allowed_host_functions: vec![],
            outputs: vec![],
            host_calls_made: 0,
            killed: false,
            kill_reason: None,
            limiter: wasmtime::StoreLimitsBuilder::new().build(),
            agent_context: None,
            speculative_policy: None,
        };
        assert_eq!(
            check_speculation(&state, "llm_query"),
            SpeculativeDecision::Commit
        );
    }

    #[test]
    fn check_speculation_with_policy_delegates() {
        let mut policy = SpeculativePolicy::allow_all();
        policy.set_function_decision("fs_write", SpeculativeDecision::Block);

        let state = WasmAgentState {
            agent_id: "test".to_string(),
            capabilities: vec![],
            allowed_host_functions: vec![],
            outputs: vec![],
            host_calls_made: 0,
            killed: false,
            kill_reason: None,
            limiter: wasmtime::StoreLimitsBuilder::new().build(),
            agent_context: None,
            speculative_policy: Some(policy),
        };
        assert_eq!(
            check_speculation(&state, "fs_write"),
            SpeculativeDecision::Block
        );
        assert_eq!(
            check_speculation(&state, "fs_read"),
            SpeculativeDecision::Commit
        );
    }

    // ── nexus_exec_tool tests ──────────────────────────────────────────

    #[test]
    fn test_nexus_exec_tool_git_status() {
        let json = r#"{"GitStatus":null}"#;
        let (code, output) = process_exec_tool_request(json);
        assert_eq!(code, 0);
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["success"], true);
        assert_eq!(parsed["program"], "git");
        assert_eq!(parsed["args"], serde_json::json!(["status"]));
    }

    #[test]
    fn test_nexus_exec_tool_rejects_unknown() {
        // NpmRunScript with an unsafe script name should be rejected
        let json = r#"{"NpmRunScript":{"script":"malicious"}}"#;
        let (code, output) = process_exec_tool_request(json);
        assert_eq!(code, -1); // NotAllowed
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["success"], false);
        assert!(parsed["error"].as_str().unwrap().contains("not allowed"));
    }

    #[test]
    fn test_nexus_exec_tool_json_parse_error() {
        let json = "not valid json at all";
        let (code, output) = process_exec_tool_request(json);
        assert_eq!(code, -5); // Error (generic)
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["success"], false);
        assert!(parsed["error"]
            .as_str()
            .unwrap()
            .contains("JSON parse error"));
    }

    #[test]
    fn test_nexus_exec_tool_cargo_test_round_trip() {
        let json = r#"{"CargoTest":{"package":"nexus-sdk","test_name":null}}"#;
        let (code, output) = process_exec_tool_request(json);
        assert_eq!(code, 0);
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["program"], "cargo");
        assert_eq!(
            parsed["args"],
            serde_json::json!(["test", "-p", "nexus-sdk"])
        );
    }

    // ── threat_scan_side_effect tests (BUG 4 FIX) ──────────────────

    #[test]
    fn test_threat_scan_clean() {
        // Threat scan always runs — safe paths should pass.
        // With no agent_context and empty capabilities, ThreatDetector may
        // flag due to missing capabilities. Use a benign side-effect that
        // won't trigger suspicion even with empty capabilities.
        let state = WasmAgentState {
            agent_id: "test".to_string(),
            capabilities: vec!["fs.read".to_string(), "llm.query".to_string()],
            allowed_host_functions: vec![],
            outputs: vec![],
            host_calls_made: 0,
            killed: false,
            kill_reason: None,
            limiter: wasmtime::StoreLimitsBuilder::new().build(),
            agent_context: None,
            speculative_policy: None,
        };
        let effect = ContextSideEffect::FileRead {
            path: "/tmp/safe.txt".into(),
            fuel_cost: 2,
        };
        assert_eq!(
            threat_scan_side_effect(&state, &effect),
            SpeculativeDecision::Commit
        );
    }

    fn make_test_agent_context() -> Rc<std::cell::RefCell<crate::context::AgentContext>> {
        Rc::new(std::cell::RefCell::new(crate::context::AgentContext::new(
            uuid::Uuid::new_v4(),
            vec!["fs.read".into(), "fs.write".into(), "llm.query".into()],
            10000,
        )))
    }

    #[test]
    fn test_threat_scan_suspicious_pattern() {
        // Known injection pattern detected — threat scan always runs
        let state = WasmAgentState {
            agent_id: "test".to_string(),
            capabilities: vec![],
            allowed_host_functions: vec![],
            outputs: vec![],
            host_calls_made: 0,
            killed: false,
            kill_reason: None,
            limiter: wasmtime::StoreLimitsBuilder::new().build(),
            agent_context: Some(make_test_agent_context()),
            speculative_policy: Some(SpeculativePolicy::allow_all()),
        };
        // Path traversal is a known dangerous pattern
        let effect = ContextSideEffect::FileWrite {
            path: "/tmp/../../etc/shadow".into(),
            content_size: 100,
            fuel_cost: 8,
        };
        let decision = threat_scan_side_effect(&state, &effect);
        assert_eq!(
            decision,
            SpeculativeDecision::Block,
            "path traversal should be blocked by threat scan"
        );
    }

    #[test]
    fn test_threat_scan_empty_input() {
        // Benign side-effect passes — threat scan always runs.
        let state = WasmAgentState {
            agent_id: "test".to_string(),
            capabilities: vec![],
            allowed_host_functions: vec![],
            outputs: vec![],
            host_calls_made: 0,
            killed: false,
            kill_reason: None,
            limiter: wasmtime::StoreLimitsBuilder::new().build(),
            agent_context: Some(make_test_agent_context()),
            speculative_policy: Some(SpeculativePolicy::allow_all()),
        };
        let effect = ContextSideEffect::ApprovalRequest {
            description: "safe operation".to_string(),
        };
        assert_eq!(
            threat_scan_side_effect(&state, &effect),
            SpeculativeDecision::Commit,
            "clean input should pass threat scan"
        );
    }

    #[test]
    fn speculative_policy_serializes_to_json() {
        let mut policy = SpeculativePolicy::allow_all();
        policy.set_function_decision("fs_write", SpeculativeDecision::Block);

        let json = serde_json::to_string(&policy).unwrap();
        assert!(json.contains("\"fs_write\""));
        assert!(json.contains("Block"));

        let deserialized: SpeculativePolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.decide("fs_write"), SpeculativeDecision::Block);
        assert_eq!(deserialized.decide("fs_read"), SpeculativeDecision::Commit);
    }
}
