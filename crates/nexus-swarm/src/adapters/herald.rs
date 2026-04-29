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

pub struct HeraldAdapter {
    providers: Arc<HashMap<String, Arc<dyn Provider>>>,
    /// Bug W: per-channel post-count source. Wired into the cloned
    /// `SocialPosterEntry` constructed per request. Adapter init keeps
    /// the handle so the entry can stay stateless across calls (cheap
    /// `Arc::clone` per request, no shared mutable state on the adapter
    /// beyond what the trait already provides).
    publish_state: Arc<dyn social_poster_agent::publish_state::PublishStateHandle>,
}

impl HeraldAdapter {
    pub fn new(
        providers: Arc<HashMap<String, Arc<dyn Provider>>>,
        publish_state: Arc<dyn social_poster_agent::publish_state::PublishStateHandle>,
    ) -> Self {
        Self {
            providers,
            publish_state,
        }
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
        let prompt = build_prompt(&invocation);
        invoke_resolved_provider(&self.providers, &invocation, prompt, 512).await
    }

    async fn run_with_context(
        &self,
        invocation: CapabilityInvocation,
        ctx: &AgentExecutionContext,
    ) -> Result<Value, SwarmError> {
        // Phase 4b-herald: delegate to social-poster-agent's
        // SocialPosterEntry. Real LLM call flows via ctx.provider; per-
        // node tokens recorded into ctx.budget; emissions reach the
        // coordinator's broadcast channel from inside the agent crate.
        // The Phase 4a invoke_with_context prompt-shell stays available
        // via `run` for the provider-not-in-registry coordinator
        // fallback (test fixtures with empty provider maps).
        use nexus_swarm_core::SwarmAgentEntry;
        use social_poster_agent::swarm_entry::SocialPosterEntry;
        SocialPosterEntry::new(Arc::clone(&self.publish_state))
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
            agent: "herald".into(),
            detail: msg,
        },
        nexus_swarm_core::AgentError::PublishFailed {
            reason,
            retryable,
            retry_after_secs,
        } => SwarmError::PublishFailed {
            agent: "herald".into(),
            reason,
            retryable,
            retry_after_secs,
        },
        nexus_swarm_core::AgentError::Internal(msg) => SwarmError::AgentInternal {
            agent: "herald".into(),
            detail: msg,
        },
    }
}

fn build_prompt(invocation: &CapabilityInvocation) -> String {
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
    format!(
        "You are Herald, a social content writer.\n\
         Platform: {platform}. Style: {style}.\n\
         Topic: {topic}\n\n\
         Write ONE post. Obey platform length limits. No hashtags unless style requires it."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Bug AE: AgentError::PublishFailed must round-trip through the
    /// Herald adapter's mapping and reach SwarmError::PublishFailed
    /// with all four fields preserved (agent attribution, reason,
    /// retryable hint, retry_after_secs). The V2 retry loop (Bug BG)
    /// reads `retryable` and `retry_after_secs` to drive the
    /// connector retry policy, so any field loss here is silent
    /// behavior loss.
    #[test]
    fn publish_failed_round_trips_with_all_fields() {
        let agent_err = nexus_swarm_core::AgentError::PublishFailed {
            reason: "rate limited".into(),
            retryable: true,
            retry_after_secs: Some(60),
        };
        let swarm_err = map_agent_error(agent_err);
        match swarm_err {
            SwarmError::PublishFailed {
                agent,
                reason,
                retryable,
                retry_after_secs,
            } => {
                assert_eq!(agent, "herald");
                assert_eq!(reason, "rate limited");
                assert!(retryable);
                assert_eq!(retry_after_secs, Some(60));
            }
            other => panic!("expected SwarmError::PublishFailed, got {other:?}"),
        }
    }

    #[test]
    fn publish_failed_non_retryable_round_trip() {
        let agent_err = nexus_swarm_core::AgentError::PublishFailed {
            reason: "auth failure".into(),
            retryable: false,
            retry_after_secs: None,
        };
        let swarm_err = map_agent_error(agent_err);
        assert!(matches!(
            swarm_err,
            SwarmError::PublishFailed {
                retryable: false,
                retry_after_secs: None,
                ..
            }
        ));
    }
}
