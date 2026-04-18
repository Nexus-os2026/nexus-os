//! Herald adapter.
//!
//! - Wraps `social-poster-agent` (`agents/social-poster`).
//! - Entry point (Phase 1): adapter's own `run()` — composes a social-media
//!   flavoured prompt and calls the resolved provider. Phase 2 will call the
//!   `social_poster_agent` pipeline directly.
//! - Default `TaskProfile`: Light reasoning, Basic tool-use, Interactive
//!   latency, Medium context, Public privacy, Low cost. Short posts, cheap
//!   to generate, quick turnaround.

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

pub struct HeraldAdapter {
    providers: Arc<HashMap<String, Arc<dyn Provider>>>,
}

impl HeraldAdapter {
    pub fn new(providers: Arc<HashMap<String, Arc<dyn Provider>>>) -> Self {
        Self { providers }
    }
}

#[async_trait]
impl SwarmCapability for HeraldAdapter {
    fn descriptor(&self) -> AgentCapabilityDescriptor {
        AgentCapabilityDescriptor {
            id: "herald".into(),
            name: "Herald".into(),
            role: "Social content generation (wraps social-poster-agent)".into(),
            task_profile_default: TaskProfile {
                reasoning: ReasoningTier::Light,
                tool_use: ToolUseLevel::Basic,
                latency: LatencyClass::Interactive,
                context: ContextSize::Medium,
                privacy: PrivacyClass::Public,
                cost: CostClass::Low,
            },
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["topic", "platform"],
                "properties": {
                    "topic": {"type": "string"},
                    "platform": {"type": "string", "enum": ["twitter", "linkedin", "reddit"]},
                    "style": {"type": "string"}
                }
            }),
            output_schema: serde_json::json!({
                "type": "object",
                "properties": {"text": {"type": "string"}}
            }),
            max_parallel: 4,
            cost_class: CostClass::Low,
            todo_reason: None,
        }
    }

    async fn run(&self, invocation: CapabilityInvocation) -> Result<Value, SwarmError> {
        let topic = invocation
            .inputs
            .get("node_inputs")
            .and_then(|n| n.get("topic"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let platform = invocation
            .inputs
            .get("node_inputs")
            .and_then(|n| n.get("platform"))
            .and_then(|v| v.as_str())
            .unwrap_or("twitter");
        let style = invocation
            .inputs
            .get("node_inputs")
            .and_then(|n| n.get("style"))
            .and_then(|v| v.as_str())
            .unwrap_or("neutral");
        let prompt = format!(
            "You are Herald, a social content writer.\n\
             Platform: {platform}. Style: {style}.\n\
             Topic: {topic}\n\n\
             Write ONE post. Obey platform length limits. No hashtags unless style requires it."
        );
        invoke_resolved_provider(&self.providers, &invocation, prompt, 512).await
    }
}
