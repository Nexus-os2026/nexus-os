//! Watchdog — NYI descriptor stub.
//!
//! TODO (Phase 2+): wire to an agent responsible for anomaly detection,
//! adversarial stress, and runtime-invariant monitoring.

use crate::adapters::stub_descriptor;
use crate::capability::{AgentCapabilityDescriptor, CapabilityInvocation, SwarmCapability};
use crate::error::SwarmError;
use async_trait::async_trait;
use serde_json::Value;

pub struct WatchdogStub;

#[async_trait]
impl SwarmCapability for WatchdogStub {
    fn descriptor(&self) -> AgentCapabilityDescriptor {
        stub_descriptor(
            "watchdog",
            "Watchdog",
            "Runtime invariant monitoring (NYI)",
            "No watchdog crate yet. See docs/roadmap for planned implementation.",
        )
    }

    async fn run(&self, _invocation: CapabilityInvocation) -> Result<Value, SwarmError> {
        Err(SwarmError::RegistryMiss(
            "watchdog capability is a stub".into(),
        ))
    }
}
