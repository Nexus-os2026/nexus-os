//! Nexus OS Plugin SDK for building governed agents.
//!
//! Provides the `NexusAgent` trait, `AgentContext` for capability-gated operations,
//! `ManifestBuilder` for fluent manifest construction, and `TestHarness` for testing.

pub mod agent_trait;
pub mod context;
pub mod manifest;
pub mod sandbox;
pub mod testing;

pub use agent_trait::{AgentOutput, NexusAgent};
pub use context::AgentContext;
pub use manifest::ManifestBuilder;
pub use sandbox::{
    HostCallResult, HostFunction, InProcessSandbox, SandboxConfig, SandboxError, SandboxResult,
    SandboxRuntime,
};
pub use testing::TestHarness;
