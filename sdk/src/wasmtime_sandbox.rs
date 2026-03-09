//! Real wasmtime-backed sandbox for executing .wasm agents with true memory isolation,
//! fuel metering, and governance-gated host functions.
//!
//! One `Engine` is shared across all agents (thread-safe compilation cache).
//! Each agent gets its own `Store<WasmAgentState>` — the isolation boundary.

use crate::context::AgentContext;
use crate::sandbox::{SandboxConfig, SandboxError, SandboxResult, SandboxRuntime};
use crate::wasm_signature::{self, SignaturePolicy, SignatureVerification};
use crate::wasmtime_host_functions::{self, SpeculativePolicy};
use ed25519_dalek::VerifyingKey;
use nexus_kernel::audit::EventType;
use serde_json::json;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;
use wasmtime::{Engine, Linker, Module, Store, StoreLimits, StoreLimitsBuilder};

/// Per-agent state held inside the wasmtime Store.
/// Carries governance context so host functions can check permissions.
///
/// Host functions access the real `AgentContext` through `agent_context`,
/// which delegates all governance (capability checks, fuel, audit) to the
/// existing `AgentContext` methods. No governance logic is reimplemented here.
pub struct WasmAgentState {
    pub agent_id: String,
    pub capabilities: Vec<String>,
    pub allowed_host_functions: Vec<String>,
    pub outputs: Vec<String>,
    pub host_calls_made: u64,
    pub killed: bool,
    pub kill_reason: Option<String>,
    pub limiter: StoreLimits,
    /// Shared reference to the AgentContext for governance delegation.
    /// Set before execution, cleared after. Host functions borrow this to call
    /// ctx.llm_query(), ctx.read_file(), etc.
    pub agent_context: Option<Rc<RefCell<AgentContext>>>,
    /// Optional speculative policy. When `Some`, host functions check the policy
    /// before executing and may block or require human review.
    /// When `None`, all calls proceed normally (exact 6.1 behavior).
    pub speculative_policy: Option<SpeculativePolicy>,
}

impl std::fmt::Debug for WasmAgentState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WasmAgentState")
            .field("agent_id", &self.agent_id)
            .field("allowed_host_functions", &self.allowed_host_functions)
            .field("host_calls_made", &self.host_calls_made)
            .field("killed", &self.killed)
            .finish()
    }
}

/// Wasmtime-backed sandbox implementing `SandboxRuntime`.
///
/// Compiles and executes real `.wasm` bytecode with:
/// - Memory isolation via separate `Store` per agent
/// - Fuel metering mapped to the Nexus OS fuel system
/// - Configurable memory ceilings via `StoreLimits`
/// - Governance-gated host functions
pub struct WasmtimeSandbox {
    engine: Arc<Engine>,
    config: SandboxConfig,
    started_at: Option<Instant>,
    killed: bool,
    kill_reason: Option<String>,
    fuel_consumed: u64,
    host_calls_made: u64,
    outputs: Vec<String>,
    /// Peak memory observed from the wasm instance (bytes).
    memory_used: usize,
    /// Ed25519 signature policy for wasm modules.
    signature_policy: SignaturePolicy,
    /// Trusted Ed25519 public keys for module verification.
    trusted_keys: Vec<VerifyingKey>,
    /// Optional speculative policy propagated to WasmAgentState on each execution.
    speculative_policy: Option<SpeculativePolicy>,
}

impl std::fmt::Debug for WasmtimeSandbox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WasmtimeSandbox")
            .field("config", &self.config)
            .field("killed", &self.killed)
            .field("fuel_consumed", &self.fuel_consumed)
            .field("host_calls_made", &self.host_calls_made)
            .field("memory_used", &self.memory_used)
            .finish()
    }
}

impl WasmtimeSandbox {
    /// Create a new wasmtime sandbox with the given config.
    /// The `Engine` is shared (cheap to clone via `Arc`) so callers can reuse it.
    /// Defaults to `AllowUnsigned` signature policy (no keys configured).
    pub fn new(config: SandboxConfig, engine: Arc<Engine>) -> Self {
        Self {
            engine,
            config,
            started_at: None,
            killed: false,
            kill_reason: None,
            fuel_consumed: 0,
            host_calls_made: 0,
            outputs: Vec::new(),
            memory_used: 0,
            signature_policy: SignaturePolicy::AllowUnsigned,
            trusted_keys: Vec::new(),
            speculative_policy: None,
        }
    }

    /// Create a new wasmtime sandbox with a default engine and `AllowUnsigned` policy.
    pub fn with_defaults(config: SandboxConfig) -> Result<Self, SandboxError> {
        let mut wasm_config = wasmtime::Config::new();
        wasm_config.consume_fuel(true);
        wasm_config.max_wasm_stack(512 * 1024); // 512 KB stack per agent

        let engine = Engine::new(&wasm_config)
            .map_err(|e| SandboxError::ConfigError(format!("wasmtime engine init: {e}")))?;

        Ok(Self::new(config, Arc::new(engine)))
    }

    /// Build a `Store<WasmAgentState>` with fuel and memory limits from config + context.
    fn build_store(
        &self,
        ctx_ref: &Rc<RefCell<AgentContext>>,
    ) -> Result<Store<WasmAgentState>, SandboxError> {
        let limiter = StoreLimitsBuilder::new()
            .memory_size(self.config.memory_limit_bytes)
            .build();

        let fuel_remaining = ctx_ref.borrow().fuel_remaining();
        let agent_id = ctx_ref.borrow().agent_id().to_string();

        let state = WasmAgentState {
            agent_id,
            capabilities: Vec::new(), // capabilities checked via AgentContext, not here
            allowed_host_functions: self.config.allowed_host_functions.clone(),
            outputs: Vec::new(),
            host_calls_made: 0,
            killed: false,
            kill_reason: None,
            limiter,
            agent_context: Some(Rc::clone(ctx_ref)),
            speculative_policy: self.speculative_policy.clone(),
        };

        let mut store = Store::new(&self.engine, state);
        store.limiter(|s| &mut s.limiter);

        // Map the agent's remaining fuel to wasmtime fuel.
        // Each Nexus fuel unit = 10_000 wasmtime fuel instructions.
        let wasm_fuel = fuel_remaining.saturating_mul(10_000);
        store
            .set_fuel(wasm_fuel)
            .map_err(|e| SandboxError::ConfigError(format!("set fuel: {e}")))?;

        Ok(store)
    }

    /// Kill with a specific reason (e.g. from SafetySupervisor halt).
    /// Unlike `SandboxRuntime::kill()`, this preserves the caller's reason string
    /// so audit logs reflect why the agent was halted.
    pub fn kill_with_reason(&mut self, reason: &str) -> Result<(), SandboxError> {
        if self.killed {
            return Err(SandboxError::AlreadyKilled);
        }
        self.killed = true;
        self.kill_reason = Some(reason.to_string());
        Ok(())
    }

    /// Total Nexus fuel consumed across all executions of this sandbox.
    pub fn fuel_consumed(&self) -> u64 {
        self.fuel_consumed
    }

    /// Whether this sandbox has been killed (by fuel exhaustion, manual kill, or safety halt).
    pub fn is_killed(&self) -> bool {
        self.killed
    }

    /// The reason this sandbox was killed, if any.
    pub fn kill_reason(&self) -> Option<&str> {
        self.kill_reason.as_deref()
    }

    /// Set the signature verification policy.
    pub fn set_signature_policy(&mut self, policy: SignaturePolicy) {
        self.signature_policy = policy;
    }

    /// Add a trusted Ed25519 public key for module verification.
    pub fn add_trusted_key(&mut self, key: VerifyingKey) {
        self.trusted_keys.push(key);
    }

    /// Current signature policy.
    pub fn signature_policy(&self) -> &SignaturePolicy {
        &self.signature_policy
    }

    /// Set the speculative policy for host function interception.
    /// When `Some`, host functions check the policy before executing.
    /// When `None`, all calls proceed normally (exact 6.1 behavior).
    pub fn set_speculative_policy(&mut self, policy: Option<SpeculativePolicy>) {
        self.speculative_policy = policy;
    }

    /// Current speculative policy, if any.
    pub fn speculative_policy(&self) -> Option<&SpeculativePolicy> {
        self.speculative_policy.as_ref()
    }

    /// Calculate how much Nexus fuel was consumed based on wasmtime fuel delta.
    fn nexus_fuel_from_wasm(wasm_fuel_consumed: u64) -> u64 {
        // Inverse of the 10_000 multiplier, rounded up so at least 1 unit consumed
        // if any wasm instructions ran.
        if wasm_fuel_consumed == 0 {
            0
        } else {
            wasm_fuel_consumed
                .saturating_add(9_999)
                .saturating_div(10_000)
        }
    }
}

impl SandboxRuntime for WasmtimeSandbox {
    fn execute(&mut self, agent_code: &[u8], ctx: &mut AgentContext) -> SandboxResult {
        self.started_at = Some(Instant::now());

        if self.killed {
            return SandboxResult {
                completed: false,
                outputs: self.outputs.clone(),
                fuel_used: self.fuel_consumed,
                host_calls_made: self.host_calls_made,
                killed: true,
                kill_reason: self.kill_reason.clone(),
            };
        }

        // Verify Ed25519 signature before compilation
        let (sig_result, wasm_bytes) = wasm_signature::verify_wasm_signature(
            agent_code,
            &self.trusted_keys,
            &self.signature_policy,
        );

        // Log verification result to audit trail
        let agent_id = ctx.agent_id();
        ctx.audit_trail_mut()
            .append_event(
                agent_id,
                EventType::ToolCall,
                json!({
                    "action": "wasm_signature_check",
                    "result": format!("{:?}", sig_result),
                    "accepted": sig_result.is_accepted(),
                }),
            )
            .expect("audit: fail-closed");

        if !sig_result.is_accepted() {
            let reason = match &sig_result {
                SignatureVerification::UnsignedRejected => {
                    "unsigned wasm module rejected by signature policy".to_string()
                }
                SignatureVerification::Invalid { reason } => {
                    format!("wasm signature invalid: {reason}")
                }
                _ => "signature verification failed".to_string(),
            };
            return SandboxResult {
                completed: false,
                outputs: vec![reason],
                fuel_used: 0,
                host_calls_made: 0,
                killed: false,
                kill_reason: None,
            };
        }

        // Compile the wasm module (using only the wasm portion, sans appended signature)
        let module = match Module::new(&self.engine, wasm_bytes) {
            Ok(m) => m,
            Err(e) => {
                return SandboxResult {
                    completed: false,
                    outputs: vec![format!("wasm compile error: {e}")],
                    fuel_used: 0,
                    host_calls_made: 0,
                    killed: false,
                    kill_reason: None,
                };
            }
        };

        // Wrap AgentContext in Rc<RefCell> so host functions can borrow it.
        // SAFETY: We take ctx by &mut, wrap it temporarily, and unwrap after execution.
        // The Rc<RefCell<>> never escapes this function — it's dropped with the Store.
        //
        // We need to move the AgentContext into the Rc temporarily. We swap it out,
        // run the wasm, then swap it back.
        let placeholder = AgentContext::new(ctx.agent_id(), Vec::new(), 0);
        let real_ctx = std::mem::replace(ctx, placeholder);
        let ctx_ref = Rc::new(RefCell::new(real_ctx));

        // Build store with fuel + memory limits
        let mut store = match self.build_store(&ctx_ref) {
            Ok(s) => s,
            Err(e) => {
                // Restore ctx before returning
                let _ = std::mem::replace(ctx, Rc::try_unwrap(ctx_ref).ok().unwrap().into_inner());
                return SandboxResult {
                    completed: false,
                    outputs: vec![format!("store init error: {e}")],
                    fuel_used: 0,
                    host_calls_made: 0,
                    killed: false,
                    kill_reason: None,
                };
            }
        };

        let fuel_before = store.get_fuel().unwrap_or(0);

        // Link host functions — delegates to wasmtime_host_functions module
        let mut linker = Linker::new(&self.engine);
        if let Err(e) = wasmtime_host_functions::link_host_functions(&mut linker) {
            // Drop store first to release Rc reference, then restore ctx
            store.data_mut().agent_context = None;
            drop(store);
            let _ = std::mem::replace(ctx, Rc::try_unwrap(ctx_ref).ok().unwrap().into_inner());
            return SandboxResult {
                completed: false,
                outputs: vec![format!("linker error: {e}")],
                fuel_used: 0,
                host_calls_made: 0,
                killed: false,
                kill_reason: None,
            };
        }

        // Instantiate
        let instance = match linker.instantiate(&mut store, &module) {
            Ok(inst) => inst,
            Err(e) => {
                store.data_mut().agent_context = None;
                drop(store);
                let _ = std::mem::replace(ctx, Rc::try_unwrap(ctx_ref).ok().unwrap().into_inner());
                return SandboxResult {
                    completed: false,
                    outputs: vec![format!("wasm instantiate error: {e}")],
                    fuel_used: 0,
                    host_calls_made: 0,
                    killed: false,
                    kill_reason: None,
                };
            }
        };

        // Look for a default export: _start (WASI) or nexus_main
        let func = instance
            .get_typed_func::<(), ()>(&mut store, "_start")
            .or_else(|_| instance.get_typed_func::<(), ()>(&mut store, "nexus_main"));

        let completed = match func {
            Ok(entry) => match entry.call(&mut store, ()) {
                Ok(()) => true,
                Err(trap) => {
                    let fuel_after = store.get_fuel().unwrap_or(0);
                    if fuel_after == 0 {
                        self.killed = true;
                        self.kill_reason = Some("fuel_exhausted".to_string());
                    } else {
                        let state = store.data();
                        if !state.killed {
                            self.outputs.push(format!("wasm trap: {trap}"));
                        }
                    }
                    false
                }
            },
            Err(_) => {
                // No entry point found — module loaded but nothing to run.
                // Still counts as successful load (useful for testing module compilation).
                true
            }
        };

        // Collect results from the agent state
        let fuel_after = store.get_fuel().unwrap_or(0);
        let wasm_fuel_consumed = fuel_before.saturating_sub(fuel_after);
        let nexus_fuel = Self::nexus_fuel_from_wasm(wasm_fuel_consumed);

        let agent_state = store.data();
        self.outputs.extend(agent_state.outputs.clone());
        self.host_calls_made += agent_state.host_calls_made;
        self.fuel_consumed += nexus_fuel;

        // Track memory: check if instance exported memory
        if let Some(wasmtime::Extern::Memory(mem)) = instance.get_export(&mut store, "memory") {
            self.memory_used = mem.data_size(&store);
        }

        // Release the Rc inside the Store, then restore AgentContext
        store.data_mut().agent_context = None;
        drop(store);
        let _ = std::mem::replace(ctx, Rc::try_unwrap(ctx_ref).ok().unwrap().into_inner());

        // Report wasm-level fuel consumption back to AgentContext so the
        // kernel's AgentFuelLedger stays in sync. Host function costs
        // (llm_query, read_file, etc.) were already deducted by AgentContext
        // methods during execution — this only covers instruction-level fuel.
        ctx.deduct_wasm_fuel(nexus_fuel);

        SandboxResult {
            completed,
            outputs: self.outputs.clone(),
            fuel_used: self.fuel_consumed,
            host_calls_made: self.host_calls_made,
            killed: self.killed,
            kill_reason: self.kill_reason.clone(),
        }
    }

    fn kill(&mut self) -> Result<(), SandboxError> {
        if self.killed {
            return Err(SandboxError::AlreadyKilled);
        }
        self.killed = true;
        self.kill_reason = Some("manually_killed".to_string());
        Ok(())
    }

    fn memory_usage(&self) -> usize {
        self.memory_used
    }

    fn elapsed_secs(&self) -> u64 {
        self.started_at.map(|s| s.elapsed().as_secs()).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn make_ctx(capabilities: Vec<&str>, fuel: u64) -> AgentContext {
        AgentContext::new(
            Uuid::new_v4(),
            capabilities.into_iter().map(|s| s.to_string()).collect(),
            fuel,
        )
    }

    fn make_sandbox() -> WasmtimeSandbox {
        WasmtimeSandbox::with_defaults(SandboxConfig::default()).unwrap()
    }

    #[test]
    fn invalid_wasm_returns_compile_error() {
        let mut sandbox = make_sandbox();
        let mut ctx = make_ctx(vec!["llm.query"], 1000);

        let result = sandbox.execute(b"not valid wasm", &mut ctx);
        assert!(!result.completed);
        assert!(result.outputs[0].contains("wasm compile error"));
    }

    #[test]
    fn valid_minimal_wasm_loads() {
        // Minimal valid wasm module: (module)
        let wasm = wat::parse_str("(module)").unwrap();
        let mut sandbox = make_sandbox();
        let mut ctx = make_ctx(vec![], 1000);

        let result = sandbox.execute(&wasm, &mut ctx);
        assert!(result.completed);
        assert!(!result.killed);
    }

    #[test]
    fn fuel_exhaustion_traps_cleanly() {
        // Module with an infinite loop — should exhaust fuel
        let wasm = wat::parse_str(
            r#"(module
                (func (export "_start")
                    (loop $inf
                        (br $inf)
                    )
                )
            )"#,
        )
        .unwrap();

        let mut sandbox = WasmtimeSandbox::with_defaults(SandboxConfig {
            memory_limit_bytes: 1024 * 1024,
            execution_timeout_secs: 300,
            allowed_host_functions: vec![],
        })
        .unwrap();
        // Give very little fuel so it exhausts quickly
        let mut ctx = make_ctx(vec![], 1);

        let result = sandbox.execute(&wasm, &mut ctx);
        assert!(!result.completed);
        assert!(sandbox.killed);
        assert_eq!(sandbox.kill_reason.as_deref(), Some("fuel_exhausted"));
    }

    #[test]
    fn kill_prevents_execution() {
        let wasm = wat::parse_str("(module)").unwrap();
        let mut sandbox = make_sandbox();
        let mut ctx = make_ctx(vec![], 1000);

        sandbox.kill().unwrap();
        let result = sandbox.execute(&wasm, &mut ctx);
        assert!(!result.completed);
        assert!(result.killed);

        // Double kill errors
        assert_eq!(sandbox.kill(), Err(SandboxError::AlreadyKilled));
    }

    #[test]
    fn host_function_log_captures_output() {
        // Module that calls nexus_log
        let wasm = wat::parse_str(
            r#"(module
                (import "nexus" "nexus_log" (func $log (param i32 i32 i32)))
                (memory (export "memory") 1)
                (data (i32.const 0) "hello from wasm")
                (func (export "_start")
                    (call $log (i32.const 0) (i32.const 0) (i32.const 15))
                )
            )"#,
        )
        .unwrap();

        let mut sandbox = make_sandbox();
        let mut ctx = make_ctx(vec![], 1000);

        let result = sandbox.execute(&wasm, &mut ctx);
        assert!(result.completed);
        assert!(result.outputs.contains(&"hello from wasm".to_string()));
        assert_eq!(result.host_calls_made, 1);
    }

    #[test]
    fn memory_usage_tracked() {
        // Module with 1 page of memory (64KB)
        let wasm = wat::parse_str(
            r#"(module
                (memory (export "memory") 1)
            )"#,
        )
        .unwrap();

        let mut sandbox = make_sandbox();
        let mut ctx = make_ctx(vec![], 1000);

        sandbox.execute(&wasm, &mut ctx);
        assert!(sandbox.memory_usage() >= 65536); // at least 1 wasm page
    }

    #[test]
    fn kill_with_reason_preserves_reason() {
        let mut sandbox = make_sandbox();

        sandbox
            .kill_with_reason("safety supervisor halted: three-strike rule")
            .unwrap();

        assert!(sandbox.is_killed());
        assert_eq!(
            sandbox.kill_reason(),
            Some("safety supervisor halted: three-strike rule")
        );

        // Double kill returns error
        assert_eq!(
            sandbox.kill_with_reason("again"),
            Err(SandboxError::AlreadyKilled)
        );
    }

    #[test]
    fn fuel_reported_back_to_context() {
        // Module that does some work (loop with bounded iterations)
        let wasm = wat::parse_str(
            r#"(module
                (func (export "_start")
                    (local $i i32)
                    (block $done
                        (loop $loop
                            (local.get $i)
                            (i32.const 100)
                            (i32.ge_u)
                            (br_if $done)
                            (local.get $i)
                            (i32.const 1)
                            (i32.add)
                            (local.set $i)
                            (br $loop)
                        )
                    )
                )
            )"#,
        )
        .unwrap();

        let mut sandbox = make_sandbox();
        let mut ctx = make_ctx(vec![], 1000);
        let fuel_before = ctx.fuel_remaining();

        let result = sandbox.execute(&wasm, &mut ctx);
        assert!(result.completed);

        // Fuel should have been deducted from context
        assert!(ctx.fuel_remaining() < fuel_before);
        // sandbox.fuel_consumed() should match the delta
        assert!(sandbox.fuel_consumed() > 0);

        // Audit trail should have a wasm_fuel_consumed event
        let has_wasm_fuel_event = ctx.audit_trail().events().iter().any(|e| {
            e.payload.get("action").and_then(|v| v.as_str()) == Some("wasm_fuel_consumed")
        });
        assert!(has_wasm_fuel_event);
    }

    #[test]
    fn fuel_exhaustion_reports_back_to_context() {
        let wasm = wat::parse_str(
            r#"(module
                (func (export "_start")
                    (loop $inf (br $inf))
                )
            )"#,
        )
        .unwrap();

        let mut sandbox = WasmtimeSandbox::with_defaults(SandboxConfig {
            memory_limit_bytes: 1024 * 1024,
            execution_timeout_secs: 300,
            allowed_host_functions: vec![],
        })
        .unwrap();
        let mut ctx = make_ctx(vec![], 5);

        sandbox.execute(&wasm, &mut ctx);

        // After fuel exhaustion, context fuel should be reduced
        assert!(ctx.fuel_remaining() < 5);
        assert!(sandbox.is_killed());
    }
}
