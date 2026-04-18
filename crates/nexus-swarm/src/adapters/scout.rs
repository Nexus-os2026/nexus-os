//! Scout — NYI descriptor stub.
//!
//! Exists so the Director planning prompt and `~/.nexus/swarm_routing.toml`
//! can reference the `scout` id without errors. Runtime selection is
//! prevented by [`CapabilityRegistry::select_for_task`](crate::CapabilityRegistry)
//! skipping any stub descriptor.
//!
//! TODO (Phase 2+): wire to whichever future `nexus-scout` crate implements
//! opportunity-discovery / market-scanning.

use crate::adapters::stub_descriptor;
use crate::capability::{AgentCapabilityDescriptor, CapabilityInvocation, SwarmCapability};
use crate::error::SwarmError;
use async_trait::async_trait;
use serde_json::Value;

pub struct ScoutStub;

#[async_trait]
impl SwarmCapability for ScoutStub {
    fn descriptor(&self) -> AgentCapabilityDescriptor {
        stub_descriptor(
            "scout",
            "Scout",
            "Opportunity discovery (NYI)",
            "No scout crate yet. See docs/roadmap for planned implementation.",
        )
    }

    async fn run(&self, _invocation: CapabilityInvocation) -> Result<Value, SwarmError> {
        Err(SwarmError::RegistryMiss(
            "scout capability is a stub — registry must not select it".into(),
        ))
    }
}
