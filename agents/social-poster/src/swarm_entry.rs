//! Swarm entry point for `social-poster-agent`.
//!
//! Implements [`SwarmAgentEntry`] so the swarm coordinator's Herald
//! adapter can dispatch into this crate via a single
//! `SocialPosterEntry::new().execute(input, ctx)` call. All LLM traffic
//! flows through `ctx.provider.invoke(...)`; tokens are recorded into
//! `ctx.budget`; cancellation is honoured between phases.
//!
//! # Input schema
//!
//! ```json
//! {
//!   "channel": "X",
//!   "audience": "Rust developers interested in async runtimes",
//!   "message": "Tokio 1.50 just shipped a new spawn_blocking optimization",
//!   "tone": "concise",
//!   "research_summary": "Optional pre-fetched context the LLM should weave in.",
//!   "max_tokens": 512,
//!   "dry_run": true
//! }
//! ```
//!
//! `channel`, `audience`, and `message` are required. `dry_run` defaults
//! to `true` — tests get isolation for free; production opts in with
//! `dry_run: false`.
//!
//! # Output schema
//!
//! ```json
//! {
//!   "draft": "Full text of the generated post",
//!   "channel": "X",
//!   "compliance": "Allowed",
//!   "publish_status": "skipped_dry_run" | "deferred" | "blocked_by_compliance",
//!   "tokens_used": 152,
//!   "dry_run": true
//! }
//! ```
//!
//! # Phase events
//!
//! Six emissions per execution. Names are semantic, not generic:
//!
//! 1. `parsing_input`     — `{ channel, audience_chars, has_research }`
//! 2. `drafting`          — `{ model_id, max_tokens }` (just before the provider call)
//! 3. `parsing_response`  — `{ response_chars }` (after the provider call)
//! 4. `reviewing`         — `{ allowed, decision }` (compliance check)
//! 5. `publishing`        — `{ dry_run, channel, publish_status }`
//! 6. `complete`          — `{ tokens_used, draft_chars, publish_status }`
//!
//! # Publish step (Phase 5 deferral)
//!
//! When `dry_run: true`, no real publishing happens — the post is drafted,
//! compliance-checked, and returned. When `dry_run: false`, the publish
//! step is **structurally present but not wired**: `publish_status:
//! "deferred"` is returned without invoking `TwitterConnector`. Real
//! publish requires plumbing `WebAgentContext` (governance/fuel) and API
//! credentials through to swarm context — Phase 5 work tracked as
//! Bug V in the backlog.
//!
//! # Privacy class semantics
//!
//! Herald's TaskProfile is `Sensitive` per `routing_defaults.rs`, meaning
//! the **LLM call** routes only to local-capable providers (the Router
//! enforces this). The destination (public Twitter) is unrelated —
//! privacy class governs which provider can run the prompt, not what
//! the prompt produces or where the result goes.

use async_trait::async_trait;
use nexus_content::compliance::{check_compliance, ComplianceDecision};
use nexus_content::generator::SocialPlatform;
use nexus_swarm_core::{AgentError, AgentExecutionContext, InvokeRequest, SwarmAgentEntry};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Deserialize)]
pub struct SocialPosterInput {
    pub channel: SocialPlatform,
    pub audience: String,
    pub message: String,
    #[serde(default)]
    pub tone: Option<String>,
    /// Optional pre-fetched research summary the LLM should incorporate.
    /// 4b-herald deliberately does NOT do live web research — that's
    /// Phase 5 once swarm context has a place for `WebAgentContext`.
    #[serde(default)]
    pub research_summary: Option<String>,
    #[serde(default)]
    pub max_tokens: Option<u32>,
    /// Defaults to `true` so tests opting out of fields still get
    /// isolated. Production callers must set `dry_run: false` explicitly.
    #[serde(default = "default_dry_run")]
    pub dry_run: bool,
}

fn default_dry_run() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublishStatus {
    /// `dry_run` was true; nothing was sent.
    SkippedDryRun,
    /// Compliance check rejected the draft; no publish attempted.
    BlockedByCompliance,
    /// `dry_run` was false but real publish wiring is Phase 5.
    Deferred,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocialPosterOutput {
    pub draft: String,
    pub channel: SocialPlatform,
    pub compliance: String,
    pub publish_status: PublishStatus,
    pub tokens_used: u64,
    pub dry_run: bool,
}

const DEFAULT_MAX_TOKENS: u32 = 512;

#[derive(Debug, Default, Clone)]
pub struct SocialPosterEntry;

impl SocialPosterEntry {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl SwarmAgentEntry for SocialPosterEntry {
    async fn execute(
        &self,
        input: Value,
        ctx: &AgentExecutionContext,
    ) -> Result<Value, AgentError> {
        let parsed: SocialPosterInput = serde_json::from_value(extract_node_inputs(input))
            .map_err(|e| AgentError::InvalidInput(format!("SocialPosterInput parse: {e}")))?;
        if parsed.message.trim().is_empty() {
            return Err(AgentError::InvalidInput("message must not be empty".into()));
        }
        if parsed.audience.trim().is_empty() {
            return Err(AgentError::InvalidInput(
                "audience must not be empty".into(),
            ));
        }

        ctx.emit
            .emit_phase(
                "parsing_input",
                json!({
                    "channel": platform_label(parsed.channel),
                    "audience_chars": parsed.audience.chars().count(),
                    "has_research": parsed.research_summary.is_some(),
                }),
            )
            .await;
        if ctx.cancelled() {
            return Err(AgentError::Cancelled);
        }

        let max_tokens = parsed.max_tokens.unwrap_or(DEFAULT_MAX_TOKENS);
        let prompt = build_prompt(&parsed);

        ctx.emit
            .emit_phase(
                "drafting",
                json!({ "model_id": ctx.model_id, "max_tokens": max_tokens }),
            )
            .await;

        let resp = ctx
            .provider
            .invoke(InvokeRequest {
                model_id: ctx.model_id.clone(),
                prompt,
                max_tokens,
                temperature: Some(0.6),
                metadata: Value::Null,
            })
            .await?;

        if ctx.cancelled() {
            return Err(AgentError::Cancelled);
        }

        let tokens_used = u64::from(resp.tokens_in).saturating_add(u64::from(resp.tokens_out));
        let delta = {
            let mut guard = ctx.budget.lock().await;
            guard.record(tokens_used, resp.cost_cents);
            *guard
        };
        ctx.emit.emit_budget_update(delta).await;

        let response_chars = resp.text.chars().count();
        ctx.emit
            .emit_phase(
                "parsing_response",
                json!({ "response_chars": response_chars }),
            )
            .await;

        let draft = trim_to_platform_limit(parsed.channel, resp.text.trim());

        // Reviewing: compliance check. `recent_posts: 0` is conservative —
        // we don't have a per-channel post count surface in 4b-herald, so
        // the check exercises rate-limit logic only when callers thread
        // that count in via input later (Phase 5).
        let decision = check_compliance(parsed.channel, 0);
        let allowed = matches!(decision, ComplianceDecision::Allowed);
        let decision_label = match &decision {
            ComplianceDecision::Allowed => "allowed".to_string(),
            ComplianceDecision::Blocked(reason) => format!("blocked: {reason}"),
        };
        ctx.emit
            .emit_phase(
                "reviewing",
                json!({ "allowed": allowed, "decision": decision_label }),
            )
            .await;

        // Publishing — gated on dry_run AND compliance.
        let publish_status = if !allowed {
            PublishStatus::BlockedByCompliance
        } else if parsed.dry_run {
            PublishStatus::SkippedDryRun
        } else {
            // Real publish requires WebAgentContext + Twitter credentials
            // threaded through to swarm context. Tracked as Bug V — Phase
            // 5 work. The pipeline structurally completes here.
            PublishStatus::Deferred
        };
        ctx.emit
            .emit_phase(
                "publishing",
                json!({
                    "dry_run": parsed.dry_run,
                    "channel": platform_label(parsed.channel),
                    "publish_status": publish_status_label(&publish_status),
                }),
            )
            .await;

        ctx.emit
            .emit_phase(
                "complete",
                json!({
                    "tokens_used": tokens_used,
                    "draft_chars": draft.chars().count(),
                    "publish_status": publish_status_label(&publish_status),
                }),
            )
            .await;

        let output = SocialPosterOutput {
            draft,
            channel: parsed.channel,
            compliance: decision_label,
            publish_status,
            tokens_used,
            dry_run: parsed.dry_run,
        };
        serde_json::to_value(output)
            .map_err(|e| AgentError::Internal(format!("serialize SocialPosterOutput: {e}")))
    }
}

fn extract_node_inputs(input: Value) -> Value {
    if let Value::Object(mut map) = input {
        if let Some(inner) = map.remove("node_inputs") {
            return inner;
        }
        return Value::Object(map);
    }
    input
}

fn platform_label(p: SocialPlatform) -> &'static str {
    match p {
        SocialPlatform::X => "X",
        SocialPlatform::Instagram => "Instagram",
        SocialPlatform::Facebook => "Facebook",
    }
}

fn publish_status_label(s: &PublishStatus) -> &'static str {
    match s {
        PublishStatus::SkippedDryRun => "skipped_dry_run",
        PublishStatus::BlockedByCompliance => "blocked_by_compliance",
        PublishStatus::Deferred => "deferred",
    }
}

fn platform_limit(p: SocialPlatform) -> usize {
    match p {
        SocialPlatform::X => 280,
        SocialPlatform::Instagram => 2200,
        SocialPlatform::Facebook => 63206,
    }
}

fn trim_to_platform_limit(channel: SocialPlatform, text: &str) -> String {
    let limit = platform_limit(channel);
    if text.chars().count() <= limit {
        return text.to_string();
    }
    let truncated: String = text.chars().take(limit.saturating_sub(1)).collect();
    format!("{truncated}…")
}

fn build_prompt(input: &SocialPosterInput) -> String {
    let platform = platform_label(input.channel);
    let tone = input.tone.as_deref().unwrap_or("concise, direct");
    let research_block = match input.research_summary.as_deref() {
        Some(s) if !s.trim().is_empty() => format!("\nResearch context:\n{s}\n"),
        _ => String::new(),
    };
    format!(
        "You are Herald, a social-media content writer.\n\
         Platform: {platform}. Tone: {tone}.\n\
         Audience: {audience}\n\
         {research_block}\
         Message to communicate: {message}\n\n\
         Write ONE post. Obey the platform's natural length and style. \
         Return ONLY the post text — no markdown, no quoting, no preamble.",
        audience = input.audience,
        message = input.message,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use nexus_swarm_core::emitter::recording::{Recorded, RecordingEmitter};
    use nexus_swarm_core::{
        CancelToken, CostClass, ModelDescriptor, NodeBudget, PrivacyClass, Provider,
        ProviderCapabilities, ProviderError, ProviderHealth, ProviderHealthStatus, ReasoningTier,
    };
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use uuid::Uuid;

    struct FakeProvider {
        text: String,
    }

    #[async_trait]
    impl Provider for FakeProvider {
        fn id(&self) -> &str {
            "fake"
        }
        fn capabilities(&self) -> ProviderCapabilities {
            ProviderCapabilities {
                models: vec![ModelDescriptor {
                    id: "fake-small".into(),
                    param_count_b: Some(1),
                    tier: ReasoningTier::Light,
                    context_window: 4096,
                }],
                supports_tool_use: false,
                supports_streaming: false,
                max_context: 4096,
                cost_class: CostClass::Free,
                privacy_class: PrivacyClass::StrictLocal,
            }
        }
        async fn health_check(&self) -> ProviderHealth {
            ProviderHealth {
                provider_id: "fake".into(),
                status: ProviderHealthStatus::Ok,
                latency_ms: Some(1),
                models: vec!["fake-small".into()],
                notes: String::new(),
                checked_at_secs: 0,
            }
        }
        async fn invoke(
            &self,
            _req: InvokeRequest,
        ) -> Result<nexus_swarm_core::InvokeResponse, ProviderError> {
            Ok(nexus_swarm_core::InvokeResponse {
                text: self.text.clone(),
                tokens_in: 9,
                tokens_out: 6,
                cost_cents: 0,
                latency_ms: 0,
                model_id: "fake-small".into(),
            })
        }
    }

    fn mk_ctx(
        emitter: Arc<RecordingEmitter>,
        cancel: CancelToken,
        canned_text: &str,
    ) -> AgentExecutionContext {
        AgentExecutionContext {
            ticket_nonce: Uuid::new_v4(),
            run_id: Uuid::new_v4(),
            node_id: "n-herald".into(),
            capability_id: "herald".into(),
            provider: Arc::new(FakeProvider {
                text: canned_text.into(),
            }),
            model_id: "fake-small".into(),
            emit: emitter,
            cancel,
            budget: Arc::new(Mutex::new(NodeBudget::new())),
        }
    }

    #[tokio::test]
    async fn happy_path_dry_run_returns_draft_and_skipped_publish() {
        let rec = Arc::new(RecordingEmitter::new());
        let ctx = mk_ctx(
            rec.clone(),
            CancelToken::new(),
            "Tokio 1.50 ships better spawn_blocking.",
        );
        let input = json!({
            "channel": "X",
            "audience": "Rust devs",
            "message": "Tokio 1.50 release",
            "tone": "concise",
        });
        let out = SocialPosterEntry::new()
            .execute(input, &ctx)
            .await
            .expect("ok");
        let parsed: SocialPosterOutput =
            serde_json::from_value(out).expect("deserialize SocialPosterOutput");
        assert!(parsed.dry_run);
        assert!(matches!(
            parsed.publish_status,
            PublishStatus::SkippedDryRun
        ));
        assert!(parsed.draft.contains("spawn_blocking"));
        assert_eq!(parsed.tokens_used, 15);
    }

    #[tokio::test]
    async fn explicit_dry_run_false_marks_publish_as_deferred() {
        let rec = Arc::new(RecordingEmitter::new());
        let ctx = mk_ctx(rec, CancelToken::new(), "post text");
        let input = json!({
            "channel": "X",
            "audience": "audience",
            "message": "msg",
            "dry_run": false,
        });
        let out = SocialPosterEntry::new()
            .execute(input, &ctx)
            .await
            .expect("ok");
        let parsed: SocialPosterOutput = serde_json::from_value(out).unwrap();
        assert!(!parsed.dry_run);
        assert!(
            matches!(parsed.publish_status, PublishStatus::Deferred),
            "expected Deferred, got {:?}",
            parsed.publish_status
        );
    }

    #[tokio::test]
    async fn invalid_input_returns_invalid_input_error() {
        let rec = Arc::new(RecordingEmitter::new());
        let ctx = mk_ctx(rec, CancelToken::new(), "anything");
        let input = json!({ "channel": "X", "audience": "devs" });
        let err = SocialPosterEntry::new()
            .execute(input, &ctx)
            .await
            .expect_err("expected InvalidInput");
        match err {
            AgentError::InvalidInput(msg) => {
                assert!(msg.contains("SocialPosterInput parse") || msg.contains("missing"));
            }
            other => panic!("expected InvalidInput, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn empty_message_returns_invalid_input() {
        let rec = Arc::new(RecordingEmitter::new());
        let ctx = mk_ctx(rec, CancelToken::new(), "anything");
        let input = json!({ "channel": "X", "audience": "devs", "message": "  " });
        let err = SocialPosterEntry::new()
            .execute(input, &ctx)
            .await
            .expect_err("expected InvalidInput");
        assert!(matches!(err, AgentError::InvalidInput(_)));
    }

    #[tokio::test]
    async fn cancellation_before_provider_call_returns_cancelled() {
        let rec = Arc::new(RecordingEmitter::new());
        let cancel = CancelToken::new();
        let ctx = mk_ctx(rec, cancel.clone(), "anything");
        cancel.cancel();
        let input = json!({ "channel": "X", "audience": "devs", "message": "msg" });
        let err = SocialPosterEntry::new()
            .execute(input, &ctx)
            .await
            .expect_err("expected Cancelled");
        assert!(matches!(err, AgentError::Cancelled));
    }

    #[tokio::test]
    async fn emits_six_phases_in_order() {
        let rec = Arc::new(RecordingEmitter::new());
        let ctx = mk_ctx(rec.clone(), CancelToken::new(), "post text");
        let input = json!({
            "channel": "X",
            "audience": "devs",
            "message": "ship it",
        });
        SocialPosterEntry::new()
            .execute(input, &ctx)
            .await
            .expect("ok");
        let log = rec.snapshot().await;
        let phases: Vec<&str> = log
            .iter()
            .filter_map(|r| match r {
                Recorded::Phase { phase, .. } => Some(phase.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(
            phases,
            vec![
                "parsing_input",
                "drafting",
                "parsing_response",
                "reviewing",
                "publishing",
                "complete"
            ],
            "phases out of order: {phases:?}"
        );
        let budgets: Vec<&NodeBudget> = log
            .iter()
            .filter_map(|r| match r {
                Recorded::Budget { delta } => Some(delta),
                _ => None,
            })
            .collect();
        assert_eq!(budgets.len(), 1);
        assert_eq!(budgets[0].tokens_consumed, 15);
    }

    #[tokio::test]
    async fn x_channel_draft_truncated_to_280_chars() {
        let long_text = "x".repeat(500);
        let rec = Arc::new(RecordingEmitter::new());
        let ctx = mk_ctx(rec, CancelToken::new(), &long_text);
        let input = json!({
            "channel": "X",
            "audience": "devs",
            "message": "long",
        });
        let out = SocialPosterEntry::new()
            .execute(input, &ctx)
            .await
            .expect("ok");
        let parsed: SocialPosterOutput = serde_json::from_value(out).unwrap();
        assert!(parsed.draft.chars().count() <= 280);
        assert!(parsed.draft.ends_with('…'));
    }
}
