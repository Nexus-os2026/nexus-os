//! Artisan adapter.
//!
//! - Wraps `coder-agent` (`agents/coder`).
//!   Chosen over `coding-agent` because `coder-agent` exposes a finer-grained
//!   module graph (`context`, `llm_codegen`, `writer`, `fix_loop`,
//!   `test_runner`) that a future Phase 2 adapter can drive with explicit
//!   provider handles; `coding-agent` is a full binary with CLI-style entry
//!   points that would require subprocess wiring.
//! - Entry point (Phase 1): the adapter's own `run()` method — it renders a
//!   coder-flavoured prompt from the invocation inputs and parent outputs and
//!   calls the resolved provider. Phase 2 will replace this with calls into
//!   `coder_agent::llm_codegen::generate_code_with_llm`.
//! - Default `TaskProfile`: Medium reasoning, Advanced tool-use, Batch
//!   latency, Large context, Public privacy, Standard cost. Code work needs
//!   headroom on context and advanced tool-use.

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

pub struct ArtisanAdapter {
    providers: Arc<HashMap<String, Arc<dyn Provider>>>,
}

impl ArtisanAdapter {
    pub fn new(providers: Arc<HashMap<String, Arc<dyn Provider>>>) -> Self {
        Self { providers }
    }
}

#[async_trait]
impl SwarmCapability for ArtisanAdapter {
    fn descriptor(&self) -> AgentCapabilityDescriptor {
        AgentCapabilityDescriptor {
            id: "artisan".into(),
            name: "Artisan".into(),
            role: "Code generation and repair (wraps coder-agent)".into(),
            task_profile_default: TaskProfile {
                reasoning: ReasoningTier::Medium,
                tool_use: ToolUseLevel::Advanced,
                latency: LatencyClass::Batch,
                context: ContextSize::Large,
                privacy: PrivacyClass::Public,
                cost: CostClass::Standard,
            },
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["instruction"],
                "properties": {
                    "instruction": {"type": "string"},
                    "language": {"type": "string"}
                }
            }),
            output_schema: serde_json::json!({
                "type": "object",
                "properties": {"text": {"type": "string"}}
            }),
            max_parallel: 1,
            cost_class: CostClass::Standard,
            todo_reason: None,
        }
    }

    async fn run(&self, invocation: CapabilityInvocation) -> Result<Value, SwarmError> {
        let instruction = invocation
            .inputs
            .get("node_inputs")
            .and_then(|n| n.get("instruction"))
            .and_then(|v| v.as_str())
            .unwrap_or("Refine the code per the parent outputs.");
        let language = invocation
            .inputs
            .get("node_inputs")
            .and_then(|n| n.get("language"))
            .and_then(|v| v.as_str())
            .unwrap_or("rust");
        let parents = serde_json::to_string_pretty(&invocation.parent_outputs)
            .unwrap_or_else(|_| "{}".into());
        let prompt = format!(
            "You are Artisan, a focused code-writer.\n\
             Language: {language}\n\
             Parent outputs:\n{parents}\n\n\
             Task: {instruction}\n\n\
             Return ONLY the code. No markdown fences, no prose."
        );
        invoke_resolved_provider(&self.providers, &invocation, prompt, 4096).await
    }
}
