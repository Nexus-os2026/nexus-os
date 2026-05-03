//! Synthetic SwarmCapability used to satisfy the Director's registry
//! lookup. Every constructed capability is non-stub, free, and accepts
//! any `CapabilityInvocation` shape.

use async_trait::async_trait;
use nexus_swarm::capability::{AgentCapabilityDescriptor, CapabilityInvocation, SwarmCapability};
use nexus_swarm::error::SwarmError;
use nexus_swarm::profile::{CostClass, TaskProfile};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

/// Synthetic capability with a configurable name. Records every call
/// onto a shared `Arc<Mutex<Vec<String>>>` so that B.2 scenarios can
/// assert which capabilities were exercised. Returns a deterministic
/// `{"synthetic": <name>}` JSON envelope.
pub struct SyntheticCapability {
    name: String,
    call_log: Arc<Mutex<Vec<String>>>,
}

impl SyntheticCapability {
    /// Build a capability registered under `name`. The name is also the
    /// `capability_id` the Director's canned plan must reference.
    pub fn named(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            call_log: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Shared handle to the call log. Cloning the `Arc` lets a scenario
    /// inspect the log without unwrapping the registered capability.
    pub fn call_log(&self) -> Arc<Mutex<Vec<String>>> {
        Arc::clone(&self.call_log)
    }
}

#[async_trait]
impl SwarmCapability for SyntheticCapability {
    fn descriptor(&self) -> AgentCapabilityDescriptor {
        AgentCapabilityDescriptor {
            id: self.name.clone(),
            name: self.name.clone(),
            role: "synthetic test capability".to_string(),
            task_profile_default: TaskProfile::local_light(),
            input_schema: json!({}),
            output_schema: json!({}),
            max_parallel: 1,
            cost_class: CostClass::Free,
            todo_reason: None,
        }
    }

    async fn run(&self, _invocation: CapabilityInvocation) -> Result<Value, SwarmError> {
        self.call_log.lock().unwrap().push(self.name.clone());
        Ok(json!({ "synthetic": self.name }))
    }
}
