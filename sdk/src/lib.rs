//! Nexus OS Plugin SDK for building governed agents.
//!
//! Provides the `NexusAgent` trait, `AgentContext` for capability-gated operations,
//! `ManifestBuilder` for fluent manifest construction, and `TestHarness` for testing.
//!
//! Agent crates should depend on `nexus-sdk` (not `nexus-kernel` directly) and use
//! the prelude for common imports:
//!
//! ```rust,ignore
//! use nexus_sdk::prelude::*;
//! ```

pub mod agent_trait;
pub mod context;
pub mod manifest;
pub mod prelude;
pub mod sandbox;
pub mod shadow_sandbox;
pub mod testing;
pub mod wasm_agent;
pub mod wasm_signature;
pub mod wasmtime_host_functions;
pub mod wasmtime_sandbox;

pub use agent_trait::{AgentOutput, NexusAgent};
pub use context::{AgentContext, ContextSideEffect};
pub use manifest::ManifestBuilder;
pub use sandbox::{
    HostCallResult, HostFunction, InProcessSandbox, SandboxConfig, SandboxError, SandboxResult,
    SandboxRuntime,
};
pub use testing::TestHarness;
pub use shadow_sandbox::{SafetyVerdict, ShadowResult, ShadowSandbox, SideEffect, ThreatDetector};
pub use wasm_agent::WasmAgent;
pub use wasm_signature::{SignaturePolicy, SignatureVerification};
pub use wasmtime_host_functions::{SpeculativeDecision, SpeculativePolicy};
pub use wasmtime_sandbox::{WasmAgentState, WasmtimeSandbox};

// Re-export core kernel modules at SDK top level for convenience.
pub use nexus_kernel::audit;
pub use nexus_kernel::autonomy;
pub use nexus_kernel::config;
pub use nexus_kernel::consent;
pub use nexus_kernel::errors;
pub use nexus_kernel::fuel_hardening;
pub use nexus_kernel::kill_gates;
pub use nexus_kernel::lifecycle;
pub use nexus_kernel::manifest as kernel_manifest;
pub use nexus_kernel::redaction;
pub use nexus_kernel::supervisor;
