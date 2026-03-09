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
    if state.speculative_policy.is_none() {
        return SpeculativeDecision::Commit;
    }

    let (capabilities, fuel_budget) = match state.agent_context.as_ref() {
        Some(ctx) => {
            let ctx = ctx.borrow();
            (ctx.capabilities().to_vec(), ctx.fuel_budget())
        }
        None => (vec![], 0),
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
pub fn link_host_functions(linker: &mut Linker<WasmAgentState>) -> Result<(), SandboxError> {
    link_nexus_log(linker)?;
    link_nexus_emit_audit(linker)?;
    link_nexus_llm_query(linker)?;
    link_nexus_fs_read(linker)?;
    link_nexus_fs_write(linker)?;
    link_nexus_request_approval(linker)?;
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
                    _ => return,
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

                // Speculative policy gate — before real execution
                let mut decision = check_speculation(state, "llm_query");

                // Threat detection: even if policy says Commit, scan the
                // side-effect and escalate if threats are found.
                if state.speculative_policy.is_some() && decision == SpeculativeDecision::Commit {
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

                // Speculative policy gate
                let mut decision = check_speculation(state, "fs_read");

                // Threat detection on file read path
                if state.speculative_policy.is_some() && decision == SpeculativeDecision::Commit {
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

                // Speculative policy gate
                let mut decision = check_speculation(state, "fs_write");

                // Threat detection on file write path
                if state.speculative_policy.is_some() && decision == SpeculativeDecision::Commit {
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
                let desc = read_wasm_str(&caller, &memory, desc_ptr, desc_len);
                let state = caller.data_mut();

                if !state
                    .allowed_host_functions
                    .contains(&"request_approval".to_string())
                {
                    state.host_calls_made += 1;
                    return -1;
                }

                // Speculative policy gate
                let mut decision = check_speculation(state, "request_approval");

                // Threat detection on approval request
                if state.speculative_policy.is_some() && decision == SpeculativeDecision::Commit {
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
                let record = ctx.borrow_mut().request_approval(&desc);
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
