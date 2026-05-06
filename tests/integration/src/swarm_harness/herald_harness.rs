//! Bug BL.1: harness-side capability that mirrors
//! HeraldAdapter's external contract (id="herald") but
//! injects a scripted PublishExecutor INSTEAD of the
//! production RealPublishExecutor.
//!
//! HeraldAdapter (`crates/nexus-swarm/src/adapters/
//! herald.rs:55-145`) has no test seam — its
//! `run_with_context` always calls
//! `SocialPosterEntry::new(...)`, which builds a
//! RealPublishExecutor (real Twitter HTTP). To exercise
//! the BK retry decorator end-to-end through the swarm
//! coordinator without touching the network, the harness
//! provides this capability.
//!
//! Wrap-then-inject pattern:
//!
//! Per BK.3 Amendment 4 to ADR 0005,
//! `SocialPosterEntry::with_publish_executor` sets
//! `retry_config: None` to skip the per-run wrap for
//! test-injected executors. To still exercise the BK
//! retry decorator from the harness, this capability
//! constructs a `RetryingPublishExecutor` ITSELF inside
//! `run_with_context`, capturing `ctx.emit`, and injects
//! the WRAPPED executor into
//! `SocialPosterEntry::with_publish_executor`. The
//! decorator is built once per coordinator run (lazy wrap
//! at the capability layer instead of the entry layer).
//!
//! End-to-end behavior identical to the production path:
//! retry attempts emit `phase="retry_attempt"` NodeEvents
//! via `ctx.emit`; the swarm event broadcast carries them
//! to scenario assertions.

use std::sync::Arc;

use async_trait::async_trait;
use nexus_swarm::capability::{AgentCapabilityDescriptor, CapabilityInvocation, SwarmCapability};
use nexus_swarm::context::AgentExecutionContext;
use nexus_swarm::error::SwarmError;
use nexus_swarm::profile::{
    ContextSize, CostClass, LatencyClass, PrivacyClass, ReasoningTier, TaskProfile, ToolUseLevel,
};
use nexus_swarm::SwarmAgentEntry;
use serde_json::Value;
use social_poster_agent::publish_state::{InMemoryPublishState, PublishStateHandle};
use social_poster_agent::retry::{RetryConfig, RetryingPublishExecutor};
use social_poster_agent::swarm_entry::{PublishExecutor, SocialPosterEntry};

/// Test-side replacement for `HeraldAdapter`. Same id
/// (`"herald"`); replaces production publish path with a
/// scripted executor wrapped in the BK retry decorator.
pub struct HeraldHarnessCapability {
    publish_state: Arc<dyn PublishStateHandle>,
    inner_executor: Arc<dyn PublishExecutor>,
    retry_config: RetryConfig,
}

impl HeraldHarnessCapability {
    /// Build with a default in-memory publish state and
    /// the supplied scripted executor.
    pub fn new(inner_executor: Arc<dyn PublishExecutor>) -> Self {
        Self {
            publish_state: Arc::new(InMemoryPublishState::new()),
            inner_executor,
            retry_config: RetryConfig::default(),
        }
    }

    /// Override the retry config. BL.3 scenarios use
    /// fast configs (1ms backoffs) to keep tests fast.
    pub fn with_retry_config(mut self, config: RetryConfig) -> Self {
        self.retry_config = config;
        self
    }
}

#[async_trait]
impl SwarmCapability for HeraldHarnessCapability {
    fn descriptor(&self) -> AgentCapabilityDescriptor {
        AgentCapabilityDescriptor {
            id: "herald".into(),
            name: "Herald (harness)".into(),
            role: "Harness capability for publish-path scenarios".into(),
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
                    "platform": {"type": "string"},
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

    async fn run(&self, _invocation: CapabilityInvocation) -> Result<Value, SwarmError> {
        // BL.1: scenarios always exercise the
        // ctx-bearing path (retry decorator emits via
        // ctx.emit). The bare run() path is
        // unsupported and returns an error.
        // Mirrors HeraldAdapter's variant choice
        // (herald.rs:166-169 uses AgentInternal).
        Err(SwarmError::AgentInternal {
            agent: "herald".into(),
            detail: "HeraldHarnessCapability requires run_with_context".into(),
        })
    }

    async fn run_with_context(
        &self,
        invocation: CapabilityInvocation,
        ctx: &AgentExecutionContext,
    ) -> Result<Value, SwarmError> {
        // Wrap the scripted inner with the retry
        // decorator, capturing ctx.emit. The wrap is
        // local to this run; SocialPosterEntry sees the
        // already-wrapped executor and runs it once.
        let wrapped: Arc<dyn PublishExecutor> = Arc::new(RetryingPublishExecutor::with_emitter(
            Arc::clone(&self.inner_executor),
            self.retry_config,
            Arc::clone(&ctx.emit),
        ));

        let entry =
            SocialPosterEntry::with_publish_executor(Arc::clone(&self.publish_state), wrapped);

        entry
            .execute(invocation.inputs, ctx)
            .await
            .map_err(|e| SwarmError::AgentInternal {
                agent: "herald".into(),
                detail: format!("herald-harness: {e}"),
            })
    }
}
