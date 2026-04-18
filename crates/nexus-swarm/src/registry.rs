//! `CapabilityRegistry` — in-memory, read-mostly index of registered
//! capabilities.
//!
//! Adapters register themselves at boot. Stub descriptors (scout, watchdog,
//! prospector) live in the registry so the Director's planning prompt and
//! `~/.nexus/swarm_routing.toml` can reference their ids without errors, but
//! [`CapabilityRegistry::select_for_task`] skips any capability whose
//! descriptor reports `is_stub() == true`.

use crate::capability::{AgentCapabilityDescriptor, ArcCapability};
use crate::profile::TaskProfile;
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Default, Clone)]
pub struct CapabilityRegistry {
    entries: BTreeMap<String, ArcCapability>,
}

impl CapabilityRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, cap: ArcCapability) {
        let id = cap.descriptor().id;
        self.entries.insert(id, cap);
    }

    pub fn list(&self) -> Vec<AgentCapabilityDescriptor> {
        self.entries.values().map(|c| c.descriptor()).collect()
    }

    pub fn get(&self, id: &str) -> Option<ArcCapability> {
        self.entries.get(id).map(Arc::clone)
    }

    /// Find the best capability whose descriptor satisfies the given task
    /// profile. Stub descriptors are **never** selected. Returns `None` if
    /// nothing qualifies.
    ///
    /// "Best" here means: first entry (alphabetical id) whose declared
    /// default profile is reasoning-compatible (≥ required tier) and
    /// privacy-compatible (the provider's privacy class must satisfy the
    /// profile's, but the check happens in the router — here we just gate
    /// on the capability's declared acceptable privacy).
    pub fn select_for_task(&self, profile: &TaskProfile) -> Option<ArcCapability> {
        self.entries
            .values()
            .find(|cap| {
                let d = cap.descriptor();
                if d.is_stub() {
                    return false;
                }
                d.task_profile_default.reasoning >= profile.reasoning
                    && d.task_profile_default.tool_use >= profile.tool_use
                    && profile.privacy.satisfied_by(d.task_profile_default.privacy)
            })
            .map(Arc::clone)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::{CapabilityInvocation, SwarmCapability};
    use crate::error::SwarmError;
    use crate::profile::{CostClass, PrivacyClass, ReasoningTier, ToolUseLevel};
    use async_trait::async_trait;
    use serde_json::{json, Value};

    struct Cap(AgentCapabilityDescriptor);
    #[async_trait]
    impl SwarmCapability for Cap {
        fn descriptor(&self) -> AgentCapabilityDescriptor {
            self.0.clone()
        }
        async fn run(&self, _i: CapabilityInvocation) -> Result<Value, SwarmError> {
            Ok(json!({}))
        }
    }

    fn desc(
        id: &str,
        tier: ReasoningTier,
        stub: Option<&'static str>,
    ) -> AgentCapabilityDescriptor {
        AgentCapabilityDescriptor {
            id: id.into(),
            name: id.into(),
            role: "test".into(),
            task_profile_default: TaskProfile {
                reasoning: tier,
                tool_use: ToolUseLevel::Basic,
                latency: crate::profile::LatencyClass::Batch,
                context: crate::profile::ContextSize::Medium,
                privacy: PrivacyClass::StrictLocal,
                cost: CostClass::Free,
            },
            input_schema: json!({}),
            output_schema: json!({}),
            max_parallel: 1,
            cost_class: CostClass::Free,
            todo_reason: stub,
        }
    }

    #[test]
    fn register_and_list() {
        let mut r = CapabilityRegistry::new();
        r.register(Arc::new(Cap(desc("a", ReasoningTier::Light, None))));
        r.register(Arc::new(Cap(desc("b", ReasoningTier::Medium, None))));
        assert_eq!(r.list().len(), 2);
    }

    #[test]
    fn select_skips_stub_descriptors() {
        let mut r = CapabilityRegistry::new();
        r.register(Arc::new(Cap(desc(
            "scout",
            ReasoningTier::Expert,
            Some("Scout crate not implemented"),
        ))));
        r.register(Arc::new(Cap(desc("artisan", ReasoningTier::Medium, None))));
        let picked = r
            .select_for_task(&TaskProfile::local_light())
            .expect("some capability");
        assert_eq!(picked.descriptor().id, "artisan");
    }

    #[test]
    fn select_returns_none_when_only_stubs_match() {
        let mut r = CapabilityRegistry::new();
        r.register(Arc::new(Cap(desc(
            "scout",
            ReasoningTier::Expert,
            Some("stub"),
        ))));
        assert!(r.select_for_task(&TaskProfile::local_light()).is_none());
    }
}
