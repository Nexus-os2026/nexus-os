//! Prospector — NYI descriptor stub.
//!
//! TODO (Phase 2+): wire to an agent responsible for financial research,
//! arbitrage scanning, and money-making opportunity evaluation.

use crate::adapters::stub_descriptor;
use crate::capability::{AgentCapabilityDescriptor, CapabilityInvocation, SwarmCapability};
use crate::error::SwarmError;
use async_trait::async_trait;
use serde_json::Value;

pub struct ProspectorStub;

#[async_trait]
impl SwarmCapability for ProspectorStub {
    fn descriptor(&self) -> AgentCapabilityDescriptor {
        stub_descriptor(
            "prospector",
            "Prospector",
            "Financial opportunity evaluation (NYI)",
            "No prospector crate yet. See docs/roadmap for planned implementation.",
        )
    }

    async fn run(&self, _invocation: CapabilityInvocation) -> Result<Value, SwarmError> {
        Err(SwarmError::RegistryMiss(
            "prospector capability is a stub".into(),
        ))
    }
}
