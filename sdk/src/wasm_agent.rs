//! `WasmAgent` — adapter that implements `NexusAgent` for WebAssembly agents.
//!
//! Wraps a `WasmtimeSandbox` and wasm bytecode, mapping the `NexusAgent` lifecycle
//! (init/execute/shutdown/checkpoint/restore) to sandbox operations.

use crate::agent_trait::{AgentOutput, NexusAgent};
use crate::context::AgentContext;
use crate::sandbox::{SandboxConfig, SandboxRuntime};
use crate::wasmtime_sandbox::WasmtimeSandbox;
use nexus_kernel::errors::AgentError;
use serde_json::json;
use std::sync::Arc;
use wasmtime::Engine;

/// A WebAssembly-backed agent that runs inside a `WasmtimeSandbox`.
///
/// The wasm bytecode is compiled and executed on each `execute()` call.
/// The sandbox enforces fuel metering, memory limits, and capability-gated
/// host functions — all delegated to `AgentContext`.
#[derive(Debug)]
pub struct WasmAgent {
    /// Raw wasm bytecode (.wasm binary).
    wasm_bytecode: Vec<u8>,
    /// The underlying sandbox runtime.
    sandbox: WasmtimeSandbox,
    /// Whether `init()` has been called successfully.
    initialized: bool,
}

impl WasmAgent {
    /// Create a new WasmAgent from raw wasm bytecode and a sandbox config.
    /// Uses a default wasmtime engine.
    pub fn new(wasm_bytecode: Vec<u8>, config: SandboxConfig) -> Result<Self, AgentError> {
        let sandbox = WasmtimeSandbox::with_defaults(config)
            .map_err(|e| AgentError::SupervisorError(format!("sandbox init: {e}")))?;
        Ok(Self {
            wasm_bytecode,
            sandbox,
            initialized: false,
        })
    }

    /// Create a new WasmAgent with a shared wasmtime engine (for multi-agent scenarios).
    pub fn with_engine(
        wasm_bytecode: Vec<u8>,
        config: SandboxConfig,
        engine: Arc<Engine>,
    ) -> Self {
        Self {
            wasm_bytecode,
            sandbox: WasmtimeSandbox::new(config, engine),
            initialized: false,
        }
    }

    /// Access the underlying sandbox for inspection (fuel consumed, kill state, etc.).
    pub fn sandbox(&self) -> &WasmtimeSandbox {
        &self.sandbox
    }
}

impl NexusAgent for WasmAgent {
    fn init(&mut self, ctx: &mut AgentContext) -> Result<(), AgentError> {
        if self.initialized {
            return Err(AgentError::SupervisorError(
                "agent already initialized".to_string(),
            ));
        }

        // Validate the wasm bytecode by attempting a dry compile.
        // We use a minimal execution (the module may have no entry point, which is fine).
        // This catches invalid wasm before the first execute().
        if wasmtime::Module::validate(&wasmtime::Engine::default(), &self.wasm_bytecode).is_err() {
            return Err(AgentError::SupervisorError(
                "invalid wasm bytecode".to_string(),
            ));
        }

        // Record init in audit trail
        ctx.require_capability("agent.execute")
            .or_else(|_| ctx.require_capability("llm.query"))
            .ok();

        self.initialized = true;
        Ok(())
    }

    fn execute(&mut self, ctx: &mut AgentContext) -> Result<AgentOutput, AgentError> {
        if !self.initialized {
            return Err(AgentError::SupervisorError("not initialized".to_string()));
        }

        let fuel_before = ctx.fuel_remaining();
        let result = self.sandbox.execute(&self.wasm_bytecode, ctx);

        if result.killed {
            let reason = result
                .kill_reason
                .unwrap_or_else(|| "unknown".to_string());
            return Err(AgentError::SupervisorError(format!(
                "agent killed: {reason}"
            )));
        }

        if !result.completed {
            let error_msg = result
                .outputs
                .last()
                .cloned()
                .unwrap_or_else(|| "execution failed".to_string());
            return Err(AgentError::SupervisorError(error_msg));
        }

        let fuel_used = fuel_before.saturating_sub(ctx.fuel_remaining());
        let outputs = result
            .outputs
            .into_iter()
            .map(|s| json!(s))
            .collect();

        Ok(AgentOutput {
            status: "ok".to_string(),
            outputs,
            fuel_used,
        })
    }

    fn shutdown(&mut self, _ctx: &mut AgentContext) -> Result<(), AgentError> {
        if !self.initialized {
            return Ok(());
        }
        // Kill the sandbox to prevent further execution
        let _ = self.sandbox.kill();
        self.initialized = false;
        Ok(())
    }

    fn checkpoint(&self) -> Result<Vec<u8>, AgentError> {
        // Wasm linear memory is not preserved across executions in the current design
        // (each execute() creates a fresh Store). Checkpoint returns the wasm bytecode
        // so the agent can be restored from it.
        Ok(self.wasm_bytecode.clone())
    }

    fn restore(&mut self, data: &[u8]) -> Result<(), AgentError> {
        // Restore replaces the wasm bytecode (from a previous checkpoint).
        // The sandbox is reset to allow re-execution.
        if wasmtime::Module::validate(&wasmtime::Engine::default(), data).is_err() {
            return Err(AgentError::SupervisorError(
                "invalid wasm bytecode in checkpoint data".to_string(),
            ));
        }
        self.wasm_bytecode = data.to_vec();
        self.initialized = true;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sandbox::SandboxConfig;
    use uuid::Uuid;

    fn make_ctx(capabilities: Vec<&str>, fuel: u64) -> AgentContext {
        AgentContext::new(
            Uuid::new_v4(),
            capabilities.into_iter().map(|s| s.to_string()).collect(),
            fuel,
        )
    }

    fn minimal_wasm() -> Vec<u8> {
        wat::parse_str("(module)").unwrap()
    }

    fn hello_wasm() -> Vec<u8> {
        wat::parse_str(
            r#"(module
                (import "nexus" "nexus_log" (func $log (param i32 i32 i32)))
                (memory (export "memory") 1)
                (data (i32.const 0) "hello from wasm agent")
                (func (export "_start")
                    (call $log (i32.const 0) (i32.const 0) (i32.const 21))
                )
            )"#,
        )
        .unwrap()
    }

    #[test]
    fn lifecycle_init_execute_shutdown() {
        let mut agent = WasmAgent::new(hello_wasm(), SandboxConfig::default()).unwrap();
        let mut ctx = make_ctx(vec!["llm.query"], 1000);

        assert!(agent.init(&mut ctx).is_ok());
        let output = agent.execute(&mut ctx).unwrap();
        assert_eq!(output.status, "ok");
        assert!(!output.outputs.is_empty());
        assert!(agent.shutdown(&mut ctx).is_ok());
    }

    #[test]
    fn execute_before_init_fails() {
        let mut agent = WasmAgent::new(minimal_wasm(), SandboxConfig::default()).unwrap();
        let mut ctx = make_ctx(vec![], 1000);

        let result = agent.execute(&mut ctx);
        assert!(result.is_err());
    }

    #[test]
    fn double_init_fails() {
        let mut agent = WasmAgent::new(minimal_wasm(), SandboxConfig::default()).unwrap();
        let mut ctx = make_ctx(vec![], 1000);

        assert!(agent.init(&mut ctx).is_ok());
        assert!(agent.init(&mut ctx).is_err());
    }

    #[test]
    fn invalid_wasm_fails_init() {
        let mut agent =
            WasmAgent::new(b"not valid wasm".to_vec(), SandboxConfig::default()).unwrap();
        let mut ctx = make_ctx(vec![], 1000);

        let result = agent.init(&mut ctx);
        assert!(result.is_err());
    }

    #[test]
    fn checkpoint_and_restore() {
        let wasm = minimal_wasm();
        let agent = WasmAgent::new(wasm.clone(), SandboxConfig::default()).unwrap();

        let data = agent.checkpoint().unwrap();
        assert_eq!(data, wasm);

        let mut agent2 = WasmAgent::new(vec![], SandboxConfig::default()).unwrap();
        assert!(agent2.restore(&data).is_ok());

        // After restore, agent should be initialized and able to execute
        let mut ctx = make_ctx(vec![], 1000);
        let output = agent2.execute(&mut ctx).unwrap();
        assert_eq!(output.status, "ok");
    }

    #[test]
    fn shutdown_idempotent() {
        let mut agent = WasmAgent::new(minimal_wasm(), SandboxConfig::default()).unwrap();
        let mut ctx = make_ctx(vec![], 1000);

        // Shutdown before init is fine
        assert!(agent.shutdown(&mut ctx).is_ok());
        // Shutdown after init
        assert!(agent.init(&mut ctx).is_ok());
        assert!(agent.shutdown(&mut ctx).is_ok());
    }

    #[test]
    fn fuel_tracking_through_agent() {
        let wasm = wat::parse_str(
            r#"(module
                (func (export "_start")
                    (local $i i32)
                    (block $done
                        (loop $loop
                            (local.get $i)
                            (i32.const 50)
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

        let mut agent = WasmAgent::new(wasm, SandboxConfig::default()).unwrap();
        let mut ctx = make_ctx(vec![], 1000);

        agent.init(&mut ctx).unwrap();
        let output = agent.execute(&mut ctx).unwrap();
        assert!(output.fuel_used > 0);
    }
}
