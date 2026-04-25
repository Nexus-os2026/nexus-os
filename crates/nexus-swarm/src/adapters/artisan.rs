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
use crate::context::AgentExecutionContext;
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
        let prompt = build_prompt(&invocation);
        invoke_resolved_provider(&self.providers, &invocation, prompt, 4096).await
    }

    async fn run_with_context(
        &self,
        invocation: CapabilityInvocation,
        ctx: &AgentExecutionContext,
    ) -> Result<Value, SwarmError> {
        // Phase 4b: delegate to coder-agent's SwarmAgentEntry impl. The
        // entry handles its own phase emission, prompt building, provider
        // dispatch, and per-node budget recording. The Phase 4a
        // `invoke_with_context` prompt-shell stays available via `run`
        // (legacy path used when the resolved provider isn't registered
        // in the coordinator's map — see coordinator.rs spawn closure).
        use coder_agent::swarm_entry::CoderEntry;
        use nexus_swarm_core::SwarmAgentEntry;
        CoderEntry::new()
            .execute(invocation.inputs, ctx)
            .await
            .map_err(map_agent_error)
    }
}

fn map_agent_error(err: nexus_swarm_core::AgentError) -> SwarmError {
    match err {
        nexus_swarm_core::AgentError::Cancelled => SwarmError::Cancelled,
        nexus_swarm_core::AgentError::Provider(p) => SwarmError::from(p),
        nexus_swarm_core::AgentError::InvalidInput(msg) => SwarmError::AgentInvalidInput {
            agent: "artisan".into(),
            detail: msg,
        },
        nexus_swarm_core::AgentError::Internal(msg) => SwarmError::AgentInternal {
            agent: "artisan".into(),
            detail: msg,
        },
    }
}

fn build_prompt(invocation: &CapabilityInvocation) -> String {
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
    let parents =
        serde_json::to_string_pretty(&invocation.parent_outputs).unwrap_or_else(|_| "{}".into());
    format!(
        "You are Artisan, a focused code-writer.\n\
         Language: {language}\n\
         Parent outputs:\n{parents}\n\n\
         Task: {instruction}\n\n\
         Return ONLY the code. No markdown fences, no prose."
    )
}
