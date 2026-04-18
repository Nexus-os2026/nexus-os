//! Swarm capability trait + capability descriptors.
//!
//! A `SwarmCapability` is a thin adapter that exposes a real agent crate (or a
//! `NotYetImplemented` stub) to the swarm. The adapter returns an
//! [`AgentCapabilityDescriptor`] that feeds the `CapabilityRegistry` and the
//! Director's planning prompt.

use crate::profile::{CostClass, TaskProfile};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

/// Static metadata describing a capability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCapabilityDescriptor {
    pub id: String,
    pub name: String,
    pub role: String,
    pub task_profile_default: TaskProfile,
    pub input_schema: Value,
    pub output_schema: Value,
    pub max_parallel: u32,
    pub cost_class: CostClass,
    /// Populated only for descriptor stubs (Scout/Watchdog/Prospector).
    /// `CapabilityRegistry::select_for_task` skips any descriptor with a
    /// non-empty `todo_reason`. The string is included in `cargo doc` and
    /// [`Self::is_stub`] for discovery.
    #[serde(default)]
    pub todo_reason: Option<&'static str>,
}

impl AgentCapabilityDescriptor {
    pub fn is_stub(&self) -> bool {
        self.todo_reason.is_some()
    }
}

/// Input/output JSON carried by the coordinator between DAG nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityInvocation {
    pub inputs: Value,
    /// Outputs of parent nodes keyed by parent node id. Empty for roots.
    pub parent_outputs: std::collections::BTreeMap<String, Value>,
}

/// Live behavior of a capability.
#[async_trait]
pub trait SwarmCapability: Send + Sync {
    fn descriptor(&self) -> AgentCapabilityDescriptor;

    /// Execute the capability. Adapters typically dispatch to a real agent
    /// crate here. Stubs must return `Err` — the registry prevents them from
    /// being selected, but defense in depth.
    async fn run(
        &self,
        invocation: CapabilityInvocation,
    ) -> Result<Value, crate::error::SwarmError>;
}

/// Type-erased handle stored in the registry and passed to tasks.
pub type ArcCapability = Arc<dyn SwarmCapability>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::{CostClass, TaskProfile};
    use serde_json::json;

    struct FakeCap;
    #[async_trait]
    impl SwarmCapability for FakeCap {
        fn descriptor(&self) -> AgentCapabilityDescriptor {
            AgentCapabilityDescriptor {
                id: "fake".into(),
                name: "Fake".into(),
                role: "test".into(),
                task_profile_default: TaskProfile::local_light(),
                input_schema: json!({}),
                output_schema: json!({}),
                max_parallel: 1,
                cost_class: CostClass::Free,
                todo_reason: None,
            }
        }
        async fn run(
            &self,
            _invocation: CapabilityInvocation,
        ) -> Result<Value, crate::error::SwarmError> {
            Ok(json!({"ok": true}))
        }
    }

    #[test]
    fn descriptor_reports_non_stub() {
        assert!(!FakeCap.descriptor().is_stub());
    }

    #[test]
    fn descriptor_reports_stub_when_todo_reason_set() {
        let d = AgentCapabilityDescriptor {
            id: "x".into(),
            name: "x".into(),
            role: "x".into(),
            task_profile_default: TaskProfile::local_light(),
            input_schema: serde_json::json!({}),
            output_schema: serde_json::json!({}),
            max_parallel: 0,
            cost_class: CostClass::Free,
            todo_reason: Some("Awaiting scout crate."),
        };
        assert!(d.is_stub());
    }

    #[tokio::test]
    async fn fake_capability_runs() {
        let cap = FakeCap;
        let out = cap
            .run(CapabilityInvocation {
                inputs: serde_json::json!({}),
                parent_outputs: Default::default(),
            })
            .await
            .unwrap();
        assert_eq!(out, serde_json::json!({"ok": true}));
    }
}
