//! Provider abstraction. The trait + supporting types now live in
//! `nexus-swarm-core` so agent crates (coder, social-poster,
//! collaboration) can implement `SwarmAgentEntry::execute` and call
//! `ctx.provider.invoke(...)` without depending on this crate. Existing
//! call sites continue to use `nexus_swarm::provider::*` paths.

pub use nexus_swarm_core::provider::{
    InvokeRequest, InvokeResponse, ModelDescriptor, Provider, ProviderCapabilities, ProviderError,
};
