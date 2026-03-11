//! WASM-ready agent sandbox with memory limits, time limits, and capability-gated host functions.
//!
//! Uses a trait-based abstraction (`SandboxRuntime`) so the concrete implementation
//! can be swapped from `InProcessSandbox` to a wasmtime-backed sandbox later.

use crate::context::AgentContext;
use nexus_kernel::errors::AgentError;
use serde::{Deserialize, Serialize};
use std::time::Instant;

const DEFAULT_MEMORY_LIMIT: usize = 256 * 1024 * 1024; // 256 MB
const DEFAULT_TIMEOUT_SECS: u64 = 300;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxConfig {
    pub memory_limit_bytes: usize,
    pub execution_timeout_secs: u64,
    pub allowed_host_functions: Vec<String>,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            memory_limit_bytes: DEFAULT_MEMORY_LIMIT,
            execution_timeout_secs: DEFAULT_TIMEOUT_SECS,
            allowed_host_functions: vec![
                "llm_query".to_string(),
                "fs_read".to_string(),
                "fs_write".to_string(),
                "request_approval".to_string(),
            ],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HostFunction {
    LlmQuery { prompt: String, max_tokens: u32 },
    FsRead { path: String },
    FsWrite { path: String, content: String },
    RequestApproval { description: String },
}

impl HostFunction {
    pub fn name(&self) -> &str {
        match self {
            Self::LlmQuery { .. } => "llm_query",
            Self::FsRead { .. } => "fs_read",
            Self::FsWrite { .. } => "fs_write",
            Self::RequestApproval { .. } => "request_approval",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HostCallResult {
    Success { output: String },
    CapabilityDenied { function: String },
    FuelExhausted,
    TimedOut,
    MemoryExceeded,
    Error { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxResult {
    pub completed: bool,
    pub outputs: Vec<String>,
    pub fuel_used: u64,
    pub host_calls_made: u64,
    pub killed: bool,
    pub kill_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SandboxError {
    AlreadyKilled,
    ConfigError(String),
}

impl std::fmt::Display for SandboxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AlreadyKilled => write!(f, "sandbox already killed"),
            Self::ConfigError(msg) => write!(f, "sandbox config error: {msg}"),
        }
    }
}

/// Trait for sandbox runtimes. Can be backed by in-process execution or WASM.
pub trait SandboxRuntime {
    fn execute(&mut self, agent_code: &[u8], ctx: &mut AgentContext) -> SandboxResult;
    fn kill(&mut self) -> Result<(), SandboxError>;
    fn memory_usage(&self) -> usize;
    fn elapsed_secs(&self) -> u64;
}

#[derive(Debug)]
pub struct InProcessSandbox {
    config: SandboxConfig,
    memory_used: usize,
    started_at: Option<Instant>,
    killed: bool,
    kill_reason: Option<String>,
    host_calls_made: u64,
    fuel_consumed: u64,
    outputs: Vec<String>,
}

impl InProcessSandbox {
    pub fn new(config: SandboxConfig) -> Self {
        Self {
            config,
            memory_used: 0,
            started_at: None,
            killed: false,
            kill_reason: None,
            host_calls_made: 0,
            fuel_consumed: 0,
            outputs: Vec::new(),
        }
    }

    /// Check memory and time limits. Returns an error result if violated.
    pub fn check_limits(&mut self) -> Result<(), HostCallResult> {
        if self.memory_used > self.config.memory_limit_bytes {
            self.killed = true;
            self.kill_reason = Some("memory_exceeded".to_string());
            return Err(HostCallResult::MemoryExceeded);
        }
        if let Some(started) = self.started_at {
            if started.elapsed().as_secs() >= self.config.execution_timeout_secs {
                self.killed = true;
                self.kill_reason = Some("timed_out".to_string());
                return Err(HostCallResult::TimedOut);
            }
        }
        Ok(())
    }

    /// Call a host function through the sandbox, enforcing all governance checks.
    pub fn call_host_function(
        &mut self,
        func: HostFunction,
        ctx: &mut AgentContext,
    ) -> HostCallResult {
        // Check if sandbox has been killed
        if self.killed {
            return HostCallResult::Error {
                reason: self
                    .kill_reason
                    .clone()
                    .unwrap_or_else(|| "manually_killed".to_string()),
            };
        }

        // Check if function is in the allowed list
        if !self
            .config
            .allowed_host_functions
            .contains(&func.name().to_string())
        {
            return HostCallResult::CapabilityDenied {
                function: func.name().to_string(),
            };
        }

        // Check limits before executing
        if let Err(result) = self.check_limits() {
            return result;
        }

        self.host_calls_made += 1;
        let fuel_before = ctx.fuel_remaining();

        // Delegate to AgentContext methods
        let result = match func {
            HostFunction::LlmQuery { prompt, max_tokens } => {
                match ctx.llm_query(&prompt, max_tokens) {
                    Ok(output) => {
                        self.outputs.push(output.clone());
                        HostCallResult::Success { output }
                    }
                    Err(AgentError::CapabilityDenied(cap)) => HostCallResult::CapabilityDenied {
                        function: format!("llm_query (requires {})", cap),
                    },
                    Err(AgentError::FuelExhausted) => HostCallResult::FuelExhausted,
                    Err(e) => HostCallResult::Error {
                        reason: e.to_string(),
                    },
                }
            }
            HostFunction::FsRead { path } => match ctx.read_file(&path) {
                Ok(output) => {
                    self.outputs.push(output.clone());
                    HostCallResult::Success { output }
                }
                Err(AgentError::CapabilityDenied(cap)) => HostCallResult::CapabilityDenied {
                    function: format!("fs_read (requires {})", cap),
                },
                Err(AgentError::FuelExhausted) => HostCallResult::FuelExhausted,
                Err(e) => HostCallResult::Error {
                    reason: e.to_string(),
                },
            },
            HostFunction::FsWrite { path, content } => match ctx.write_file(&path, &content) {
                Ok(()) => HostCallResult::Success {
                    output: "written".to_string(),
                },
                Err(AgentError::CapabilityDenied(cap)) => HostCallResult::CapabilityDenied {
                    function: format!("fs_write (requires {})", cap),
                },
                Err(AgentError::FuelExhausted) => HostCallResult::FuelExhausted,
                Err(e) => HostCallResult::Error {
                    reason: e.to_string(),
                },
            },
            HostFunction::RequestApproval { description } => {
                let record = ctx.request_approval(&description, true);
                HostCallResult::Success {
                    output: format!("approval_requested: {}", record.description),
                }
            }
        };

        self.fuel_consumed += fuel_before - ctx.fuel_remaining();
        result
    }

    /// Simulate memory usage for testing purposes.
    pub fn simulate_memory_usage(&mut self, bytes: usize) {
        self.memory_used += bytes;
    }

    pub fn is_killed(&self) -> bool {
        self.killed
    }

    pub fn kill_reason(&self) -> Option<&str> {
        self.kill_reason.as_deref()
    }
}

impl SandboxRuntime for InProcessSandbox {
    fn execute(&mut self, _agent_code: &[u8], _ctx: &mut AgentContext) -> SandboxResult {
        self.started_at = Some(Instant::now());

        // In a real WASM runtime, agent_code would be compiled and executed here.
        // For the in-process sandbox, this is a no-op — callers use call_host_function directly.

        SandboxResult {
            completed: !self.killed,
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

    fn make_sandbox(allowed: Vec<&str>) -> InProcessSandbox {
        InProcessSandbox::new(SandboxConfig {
            memory_limit_bytes: 1024 * 1024, // 1 MB for tests
            execution_timeout_secs: 300,
            allowed_host_functions: allowed.into_iter().map(|s| s.to_string()).collect(),
        })
    }

    #[test]
    fn executes_host_function_with_valid_capability() {
        let mut ctx = make_ctx(vec!["llm.query"], 100);
        let mut sandbox = make_sandbox(vec!["llm_query"]);

        let result = sandbox.call_host_function(
            HostFunction::LlmQuery {
                prompt: "hello".to_string(),
                max_tokens: 50,
            },
            &mut ctx,
        );

        assert!(matches!(result, HostCallResult::Success { .. }));
        assert_eq!(sandbox.host_calls_made, 1);
        assert_eq!(ctx.fuel_remaining(), 90); // 100 - 10 (LLM cost)
    }

    #[test]
    fn blocks_host_function_not_in_allowed_list() {
        let mut ctx = make_ctx(vec!["llm.query", "fs.read"], 100);
        // Only allow llm_query, not fs_read
        let mut sandbox = make_sandbox(vec!["llm_query"]);

        let result = sandbox.call_host_function(
            HostFunction::FsRead {
                path: "/etc/passwd".to_string(),
            },
            &mut ctx,
        );

        assert_eq!(
            result,
            HostCallResult::CapabilityDenied {
                function: "fs_read".to_string()
            }
        );
        // No fuel should have been consumed
        assert_eq!(ctx.fuel_remaining(), 100);
    }

    #[test]
    fn memory_limit_exceeded_kills_sandbox() {
        let mut ctx = make_ctx(vec!["llm.query"], 100);
        let mut sandbox = make_sandbox(vec!["llm_query"]);

        // Simulate exceeding memory
        sandbox.simulate_memory_usage(2 * 1024 * 1024); // 2 MB > 1 MB limit

        let result = sandbox.call_host_function(
            HostFunction::LlmQuery {
                prompt: "test".to_string(),
                max_tokens: 10,
            },
            &mut ctx,
        );

        assert_eq!(result, HostCallResult::MemoryExceeded);
        assert!(sandbox.is_killed());
        assert_eq!(sandbox.kill_reason(), Some("memory_exceeded"));
    }

    #[test]
    fn time_limit_exceeded_kills_sandbox() {
        let mut ctx = make_ctx(vec!["llm.query"], 100);
        let mut sandbox = InProcessSandbox::new(SandboxConfig {
            memory_limit_bytes: 1024 * 1024,
            execution_timeout_secs: 0, // immediate timeout
            allowed_host_functions: vec!["llm_query".to_string()],
        });

        // Start the clock
        sandbox.started_at = Some(Instant::now() - std::time::Duration::from_secs(1));

        let result = sandbox.call_host_function(
            HostFunction::LlmQuery {
                prompt: "test".to_string(),
                max_tokens: 10,
            },
            &mut ctx,
        );

        assert_eq!(result, HostCallResult::TimedOut);
        assert!(sandbox.is_killed());
        assert_eq!(sandbox.kill_reason(), Some("timed_out"));
    }

    #[test]
    fn manual_kill_stops_subsequent_calls() {
        let mut ctx = make_ctx(vec!["llm.query"], 100);
        let mut sandbox = make_sandbox(vec!["llm_query"]);

        // First call succeeds
        let result = sandbox.call_host_function(
            HostFunction::LlmQuery {
                prompt: "first".to_string(),
                max_tokens: 10,
            },
            &mut ctx,
        );
        assert!(matches!(result, HostCallResult::Success { .. }));

        // Kill the sandbox
        assert!(sandbox.kill().is_ok());
        assert!(sandbox.is_killed());

        // Subsequent call fails
        let result = sandbox.call_host_function(
            HostFunction::LlmQuery {
                prompt: "second".to_string(),
                max_tokens: 10,
            },
            &mut ctx,
        );
        assert_eq!(
            result,
            HostCallResult::Error {
                reason: "manually_killed".to_string()
            }
        );

        // Double kill returns error
        assert_eq!(sandbox.kill(), Err(SandboxError::AlreadyKilled));
    }

    #[test]
    fn fuel_exhaustion_propagated_from_context() {
        let mut ctx = make_ctx(vec!["llm.query"], 15); // Only 15 fuel
        let mut sandbox = make_sandbox(vec!["llm_query"]);

        // First call costs 10 fuel — succeeds
        let result = sandbox.call_host_function(
            HostFunction::LlmQuery {
                prompt: "first".to_string(),
                max_tokens: 10,
            },
            &mut ctx,
        );
        assert!(matches!(result, HostCallResult::Success { .. }));
        assert_eq!(ctx.fuel_remaining(), 5);

        // Second call needs 10 but only 5 left — fuel exhausted
        let result = sandbox.call_host_function(
            HostFunction::LlmQuery {
                prompt: "second".to_string(),
                max_tokens: 10,
            },
            &mut ctx,
        );
        assert_eq!(result, HostCallResult::FuelExhausted);
    }

    #[test]
    fn every_host_call_checks_capability() {
        let mut ctx = make_ctx(vec!["fs.read"], 100); // Only fs.read, not llm.query
        let mut sandbox = make_sandbox(vec!["llm_query", "fs_read", "fs_write"]);

        // fs_read allowed (agent has fs.read capability)
        let result = sandbox.call_host_function(
            HostFunction::FsRead {
                path: "/tmp/ok".to_string(),
            },
            &mut ctx,
        );
        assert!(matches!(result, HostCallResult::Success { .. }));

        // llm_query denied (agent lacks llm.query capability)
        let result = sandbox.call_host_function(
            HostFunction::LlmQuery {
                prompt: "test".to_string(),
                max_tokens: 10,
            },
            &mut ctx,
        );
        assert!(matches!(result, HostCallResult::CapabilityDenied { .. }));

        // fs_write denied (agent lacks fs.write capability)
        let result = sandbox.call_host_function(
            HostFunction::FsWrite {
                path: "/tmp/bad".to_string(),
                content: "data".to_string(),
            },
            &mut ctx,
        );
        assert!(matches!(result, HostCallResult::CapabilityDenied { .. }));
    }

    #[test]
    fn execute_returns_sandbox_result() {
        let mut ctx = make_ctx(vec!["llm.query"], 100);
        let mut sandbox = make_sandbox(vec!["llm_query"]);

        // Make some host calls first
        sandbox.call_host_function(
            HostFunction::LlmQuery {
                prompt: "hello".to_string(),
                max_tokens: 10,
            },
            &mut ctx,
        );

        let result = sandbox.execute(b"agent_bytecode", &mut ctx);
        assert!(result.completed);
        assert!(!result.killed);
        assert_eq!(result.host_calls_made, 1);
        assert_eq!(result.fuel_used, 10);
        assert!(result.kill_reason.is_none());
        assert_eq!(result.outputs.len(), 1);
    }

    #[test]
    fn request_approval_through_sandbox() {
        let mut ctx = make_ctx(vec![], 100); // No special capabilities needed
        let mut sandbox = make_sandbox(vec!["request_approval"]);

        let result = sandbox.call_host_function(
            HostFunction::RequestApproval {
                description: "deploy to prod".to_string(),
            },
            &mut ctx,
        );

        match result {
            HostCallResult::Success { output } => {
                assert!(output.contains("deploy to prod"));
            }
            other => panic!("expected Success, got {:?}", other),
        }
        assert_eq!(ctx.approval_records().len(), 1);
    }

    #[test]
    fn fs_write_through_sandbox() {
        let mut ctx = make_ctx(vec!["fs.write"], 100);
        let mut sandbox = make_sandbox(vec!["fs_write"]);

        let result = sandbox.call_host_function(
            HostFunction::FsWrite {
                path: "/tmp/out.txt".to_string(),
                content: "hello world".to_string(),
            },
            &mut ctx,
        );

        assert_eq!(
            result,
            HostCallResult::Success {
                output: "written".to_string()
            }
        );
        assert_eq!(ctx.fuel_remaining(), 92); // 100 - 8 (write cost)
    }

    #[test]
    fn default_config_allows_all_host_functions() {
        let config = SandboxConfig::default();
        assert_eq!(config.memory_limit_bytes, 256 * 1024 * 1024);
        assert_eq!(config.execution_timeout_secs, 300);
        assert_eq!(config.allowed_host_functions.len(), 4);
        assert!(config
            .allowed_host_functions
            .contains(&"llm_query".to_string()));
        assert!(config
            .allowed_host_functions
            .contains(&"fs_read".to_string()));
        assert!(config
            .allowed_host_functions
            .contains(&"fs_write".to_string()));
        assert!(config
            .allowed_host_functions
            .contains(&"request_approval".to_string()));
    }
}
