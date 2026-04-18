//! Broker adapter.
//!
//! - Wraps `nexus-collaboration` (`agents/collaboration`).
//! - Entry point (Phase 1): adapter's own `run()` — routes a coordination
//!   prompt through the resolved provider. Phase 2 will connect to the
//!   collaboration blackboard/channel primitives.
//! - Default `TaskProfile`: Medium reasoning, Basic tool-use, Batch latency,
//!   Medium context, Public privacy, Low cost. Coordination chatter is
//!   structured and compact; doesn't need the headroom Artisan does.

use crate::adapters::invoke_resolved_provider;
use crate::capability::{AgentCapabilityDescriptor, CapabilityInvocation, SwarmCapability};
use crate::error::SwarmError;
use crate::profile::{
    ContextSize, CostClass, LatencyClass, PrivacyClass, ReasoningTier, TaskProfile, ToolUseLevel,
};
use crate::provider::Provider;
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

pub struct BrokerAdapter {
    providers: Arc<HashMap<String, Arc<dyn Provider>>>,
}

impl BrokerAdapter {
    pub fn new(providers: Arc<HashMap<String, Arc<dyn Provider>>>) -> Self {
        Self { providers }
    }
}

#[async_trait]
impl SwarmCapability for BrokerAdapter {
    fn descriptor(&self) -> AgentCapabilityDescriptor {
        AgentCapabilityDescriptor {
            id: "broker".into(),
            name: "Broker".into(),
            role: "Cross-agent coordination (wraps nexus-collaboration)".into(),
            task_profile_default: TaskProfile {
                reasoning: ReasoningTier::Medium,
                tool_use: ToolUseLevel::Basic,
                latency: LatencyClass::Batch,
                context: ContextSize::Medium,
                privacy: PrivacyClass::Public,
                cost: CostClass::Low,
            },
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["directive"],
                "properties": {
                    "directive": {"type": "string"}
                }
            }),
            output_schema: serde_json::json!({
                "type": "object",
                "properties": {"text": {"type": "string"}}
            }),
            max_parallel: 2,
            cost_class: CostClass::Low,
            todo_reason: None,
        }
    }

    async fn run(&self, invocation: CapabilityInvocation) -> Result<Value, SwarmError> {
        let directive = invocation
            .inputs
            .get("node_inputs")
            .and_then(|n| n.get("directive"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let parents = serde_json::to_string_pretty(&invocation.parent_outputs)
            .unwrap_or_else(|_| "{}".into());
        let prompt = format!(
            "You are Broker, the coordination voice of the swarm.\n\
             Upstream outputs:\n{parents}\n\n\
             Directive: {directive}\n\n\
             Produce a structured coordination note — what each downstream \n\
             capability should do next, in 1-3 short bullets. No prose."
        );
        invoke_resolved_provider(&self.providers, &invocation, prompt, 1024).await
    }
}
