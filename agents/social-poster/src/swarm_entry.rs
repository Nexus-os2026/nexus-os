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
//!   "publish_status":
//!       "skipped_dry_run" | "blocked_by_compliance" | "deferred"
//!     | "credentials_missing" | "auth_failure"
//!     | { "published": { "post_id": "...", "url": "..." } }
//!     | { "rate_limited": { "retry_after_secs": 60 } }
//!     | { "failed": { "reason": "..." } },
//!   "tokens_used": 152,
//!   "dry_run": true
//! }
//! ```
//!
//! # Phase events
//!
//! Per-call emission count varies by branch:
//!
//! - dry_run / blocked_by_compliance: 6 phases
//! - credentials_missing: 7 phases (adds `checking_credentials`)
//! - published / publish failures: 9 phases
//!   (adds `checking_credentials`, `publishing`, `publish_complete`)
//!
//! Sequence (subset on non-publish branches):
//!
//! 1. `parsing_input`         — `{ channel, audience_chars, has_research }`
//! 2. `drafting`              — `{ model_id, max_tokens }`
//! 3. `parsing_response`      — `{ response_chars }`
//! 4. `counting_recent_posts` — `{ channel, account_id, window_secs, recent_posts }` (Bug W)
//! 5. `reviewing`             — `{ allowed, decision, recent_posts }`
//! 6. `checking_credentials`  — `{ channel, credentials_present }` (Bug V, real-publish path)
//! 7. `publishing`            — `{ channel, account_id, draft_chars }` (Bug V, before connector)
//! 8. `publish_complete`      — `{ channel, publish_status }` (Bug V, after connector)
//! 9. `complete`              — `{ tokens_used, draft_chars, publish_status }`
//!
//! # Publish step (Bug V: real Twitter wiring)
//!
//! When `dry_run: true`, no real publishing happens — the post is drafted,
//! compliance-checked, and returned with `skipped_dry_run`. When
//! `dry_run: false`:
//!
//! - The pre-flight `checking_credentials` event reports whether OAuth1
//!   keys are configured. If missing, V short-circuits at
//!   `credentials_missing` without invoking the connector (no silent
//!   fall-through to mock mode).
//! - Otherwise, the `PublishExecutor` trait runs the connector inside
//!   `tokio::task::spawn_blocking` (the connector is sync via
//!   `reqwest::blocking`). Result is mapped to `published`,
//!   `rate_limited`, `auth_failure`, or `failed`.
//! - `record_publish` fires only on `published` (Bug AH tracks the
//!   reserve→confirm pattern that would close the partial-failure hole).
//!
//! `Deferred` is retained one revision for backwards compatibility with
//! external consumers; Bug AI tracks deletion.
//!
//! # Privacy class semantics
//!
//! Herald's TaskProfile is `Sensitive` per `routing_defaults.rs`, meaning
//! the **LLM call** routes only to local-capable providers (the Router
//! enforces this). The destination (public Twitter) is unrelated —
//! privacy class governs which provider can run the prompt, not what
//! the prompt produces or where the result goes.

use crate::channel::ChannelKey;
use crate::publish_state::{PublishStateError, PublishStateHandle};
use crate::retry::{RetryConfig, RetryingPublishExecutor};
use async_trait::async_trait;
use nexus_connectors_web::twitter::{TweetResult, TwitterConnector};
use nexus_connectors_web::WebAgentContext;
use nexus_content::compliance::{check_compliance, ComplianceDecision};
use nexus_content::generator::SocialPlatform;
pub(crate) use nexus_kernel::errors::AgentError as KernelAgentError;
use nexus_kernel::secrets::SecretsFacade;
use nexus_swarm_core::{AgentError, AgentExecutionContext, InvokeRequest, SwarmAgentEntry};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

/// Bug W: trailing window for the per-channel post-count read. Hard-
/// coded for now; Bug AC tracks making it configurable from
/// `agents/social-poster/manifest.toml`'s `posts_per_day`.
const COMPLIANCE_WINDOW: Duration = Duration::from_secs(24 * 60 * 60);

/// Bug V: stable input format for `content_hash`. Doc-only — the actual
/// `format!` call uses these three components. Documented to keep
/// future readers from drifting it (changing the format silently
/// invalidates dedupe across deploys).
#[allow(dead_code)]
const CONTENT_HASH_INPUT_FORMAT: &str = "{platform}|{account_id}|{text}";

/// Bug V: Twitter publish budget for the per-call `WebAgentContext`.
/// Mirrors the value `RealPublishStep` uses in the legacy
/// `agents/social-poster/src/lib.rs:498` path. The connector charges
/// 10 fuel for `post_status_update_idempotent`; we set headroom for one call.
const PUBLISH_FUEL_BUDGET: u64 = 50;

/// Bug V: indirection trait for the real Twitter publish call. Keeps
/// `SocialPosterEntry::execute` testable without making real HTTP
/// calls. Production wiring uses `RealPublishExecutor` (constructed
/// inside `SocialPosterEntry::new`); tests inject a stub via
/// `SocialPosterEntry::with_publish_executor`.
///
/// Two methods:
/// - `credentials_present` — fast sync check used to gate the publish
///   path and surface `CredentialsMissing` without invoking the
///   connector. The production impl reads the same kernel config
///   that the connector's loader reads (see `x_credentials_present`).
/// - `publish` — the actual call. `text` is moved because the
///   production impl spawns a blocking task and needs to own it.
#[async_trait]
pub trait PublishExecutor: Send + Sync {
    fn credentials_present(&self) -> bool;
    /// Bug BK: publish with a caller-supplied idempotency key.
    /// `RetryingPublishExecutor` captures one UUID per logical
    /// publish and passes it on every retry attempt so the
    /// connector's idempotency cache short-circuits on a
    /// successful retry replay.
    async fn publish_with_request_id(
        &self,
        text: String,
        request_id: uuid::Uuid,
    ) -> Result<TweetResult, KernelAgentError>;
    /// Bug V: convenience wrapper. Generates a fresh
    /// `request_id` per call. Default impl satisfies all
    /// existing `.publish(text)` callers without trait churn.
    async fn publish(&self, text: String) -> Result<TweetResult, KernelAgentError> {
        self.publish_with_request_id(text, uuid::Uuid::new_v4())
            .await
    }
}

/// Bug V: production `PublishExecutor`. Wraps `TwitterConnector`'s
/// sync (`reqwest::blocking`) idempotent publish call in
/// `tokio::task::spawn_blocking`. Builds a fresh `WebAgentContext`
/// per call (see locked decision #2) — capabilities and fuel are
/// adapter-local concerns, not entry-level state.
pub struct RealPublishExecutor {
    /// Bug AK Commit 2: facade is the canonical credential source
    /// after the SocialConfig migration. `credentials_present`
    /// reads four `social.*` keys; missing keys return false.
    facade: Arc<SecretsFacade>,
    /// Bug BG: persistent idempotency backing for
    /// `TwitterConnector::with_db`. The `publish` body uses this to
    /// build a connector whose `IdempotencyManager` survives process
    /// restart. Closes Bug AF's deferred swarm-path threading.
    db: Arc<nexus_persistence::NexusDatabase>,
}

impl RealPublishExecutor {
    pub fn new(facade: Arc<SecretsFacade>, db: Arc<nexus_persistence::NexusDatabase>) -> Self {
        Self { facade, db }
    }
}

#[async_trait]
impl PublishExecutor for RealPublishExecutor {
    fn credentials_present(&self) -> bool {
        // Bug AK Commit 2: replaces the old free fn
        // `x_credentials_present` that read NexusConfig.social.x_*.
        // Vault names use the post-migration renames (see
        // `kernel::secrets::migrate` module header). All four
        // OAuth1 fields must be present and non-empty for a real
        // publish; bearer-token mode is not currently consumed by
        // the connector's idempotent publish call so we don't gate
        // on it.
        let names = [
            "x_consumer_key",
            "x_consumer_secret",
            "x_access_token",
            "x_access_token_secret",
        ];
        // AK-2: `credentials_present` is a trait method without
        // AgentExecutionContext in scope; it's a pre-flight
        // presence check (not a per-agent-invocation read). Use
        // the SYSTEM-class `startup` sentinel — threading ctx
        // through the PublishExecutor trait is out of AK-2 scope.
        let audit_ctx = nexus_kernel::secrets::SecretAuditCtx::startup();
        names.iter().all(|name| {
            self.facade
                .get_secret(&audit_ctx, "social", name)
                .map(|s| !s.value.is_empty())
                .unwrap_or(false)
        })
    }

    async fn publish_with_request_id(
        &self,
        text: String,
        request_id: Uuid,
    ) -> Result<TweetResult, KernelAgentError> {
        // Bug BK.2: request_id is now caller-supplied. The retry
        // decorator passes the same id across attempts so the
        // connector's idempotency cache replays a successful retry.
        // Bug BG: persistent idempotency via TwitterConnector::with_db
        // is unchanged.
        let request_id_string = request_id.to_string();
        let db = Arc::clone(&self.db);
        let join = tokio::task::spawn_blocking(move || {
            let mut connector = TwitterConnector::with_db(db);
            let mut agent_ctx = WebAgentContext::new(
                Uuid::new_v4(),
                ["social.x.post".to_string(), "social.x.read".to_string()]
                    .into_iter()
                    .collect::<HashSet<_>>(),
                PUBLISH_FUEL_BUDGET,
            );
            connector.post_status_update_idempotent(&mut agent_ctx, &text, &request_id_string)
        })
        .await;
        match join {
            Ok(inner) => inner,
            Err(join_err) => Err(KernelAgentError::SupervisorError(format!(
                "publish task join error: {join_err}"
            ))),
        }
    }
}

/// Bug V: deterministic dedupe digest. Format is locked by
/// `CONTENT_HASH_INPUT_FORMAT`. Returned as a hex sha256.
fn content_hash_for(platform: SocialPlatform, account_id: &str, text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(
        format!(
            "{platform}|{account_id}|{text}",
            platform = platform_label(platform),
            account_id = account_id,
            text = text,
        )
        .as_bytes(),
    );
    format!("{:x}", hasher.finalize())
}

/// Bug V: parse "retry after {n} ms" from the connector's
/// `AgentError::SupervisorError` text. The connector formats the
/// rate-limit message at `connectors/web/src/twitter.rs:354`.
/// Returns whole seconds (rounding up partials) so callers get a
/// retry-friendly integer.
fn parse_retry_after_ms_to_secs(msg: &str) -> Option<u64> {
    // Pattern fragment: "retry after {ms} ms"
    let needle = "retry after ";
    let start = msg.find(needle)? + needle.len();
    let rest = &msg[start..];
    let end = rest.find(' ')?;
    let ms: u64 = rest[..end].parse().ok()?;
    Some(ms.div_ceil(1_000))
}

/// Bug V: classify a connector error message into the shape V's
/// `PublishStatus` exposes. Connector's error stringification is the
/// only signal — it flattens everything into
/// `AgentError::SupervisorError(String)` (see preflight Q1).
pub(crate) fn classify_publish_error(msg: &str) -> PublishStatus {
    let lower = msg.to_ascii_lowercase();
    if lower.contains("rate limited") || lower.contains("rate_limited") {
        PublishStatus::RateLimited {
            retry_after_secs: parse_retry_after_ms_to_secs(msg),
        }
    } else if lower.contains(" 401")
        || lower.contains("unauthorized")
        || lower.contains("auth")
        || lower.contains("credentials are not configured")
    {
        // The "credentials are not configured" branch fires when the
        // connector itself rejects a request with no creds. Our
        // pre-flight `x_credentials_present` check catches this earlier
        // for the swarm path, but the connector may still raise it on
        // a race between config save and call.
        PublishStatus::AuthFailure
    } else {
        PublishStatus::Failed {
            reason: msg.to_string(),
        }
    }
}

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
    /// Bug W: account identity within the platform. Single-tenant
    /// callers omit and the `default_account_id` shim returns
    /// `"default"` so the (platform, account_id) composite key always
    /// has a value. Multi-account support flips this to required.
    #[serde(default = "default_account_id")]
    pub account_id: String,
}

fn default_dry_run() -> bool {
    true
}

fn default_account_id() -> String {
    "default".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublishStatus {
    /// `dry_run` was true; nothing was sent.
    SkippedDryRun,
    /// Compliance check rejected the draft; no publish attempted.
    BlockedByCompliance,
    /// `dry_run` was false but real publish wiring is Phase 5.
    /// Bug V keeps this variant alive one revision so the swarm tests
    /// in `crates/nexus-swarm/tests/` (which run with no creds) can
    /// distinguish "no creds → deferred" from "creds present + posted".
    /// Bug AI tracks deletion after one rev of dogfooding.
    Deferred,
    /// Bug V: real publish succeeded.
    Published {
        post_id: String,
        url: Option<String>,
    },
    /// Bug V: Twitter returned a 429 (or the connector translated one).
    /// `retry_after_secs` is whatever the connector parsed out of the
    /// rate-limit message; `None` if the message lacked a number.
    RateLimited { retry_after_secs: Option<u64> },
    /// Bug V: 401 / "auth" / "unauthorized" from the connector.
    /// Distinct so the swarm log can surface "rotate creds" without
    /// parsing strings.
    AuthFailure,
    /// Bug V: `load_twitter_credentials()` returned `None` — config
    /// has at least one empty OAuth1 field. The connector would
    /// silently fall through to mock mode; V detects and returns this
    /// instead so callers don't think they shipped.
    CredentialsMissing,
    /// Bug V: catch-all for transport, 5xx, content-rejected, and
    /// any other connector error not matched above.
    Failed { reason: String },
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

/// Bug W/V: stateful entry. Holds an `Arc<dyn PublishStateHandle>`
/// (Bug W; compliance gate reads from it) and an
/// `Arc<dyn PublishExecutor>` (Bug V; performs the actual Twitter
/// call). Both are kept off `AgentExecutionContext` — publish-specific
/// concerns stay out of `nexus-swarm-core` (per the locked design).
#[derive(Clone)]
pub struct SocialPosterEntry {
    publish_state: Arc<dyn PublishStateHandle>,
    publish_executor: Arc<dyn PublishExecutor>,
    /// Bug BK.3: when Some, `execute()` wraps
    /// `publish_executor` in a per-run
    /// `RetryingPublishExecutor` capturing
    /// `Arc::clone(&ctx.emit)` so retry attempts
    /// surface as `NodeEvent` retry_attempt
    /// emissions. When None (set by the test
    /// seam `with_publish_executor`), the
    /// injected executor is used as-is.
    retry_config: Option<RetryConfig>,
}

impl SocialPosterEntry {
    /// Production constructor. Uses the real Twitter publish
    /// executor; for tests, prefer
    /// [`SocialPosterEntry::with_publish_executor`].
    ///
    /// Bug AK Commit 2: `facade` is threaded into
    /// `RealPublishExecutor` so `credentials_present` resolves
    /// from the kernel `SecretsFacade` rather than reading
    /// `NexusConfig.social.x_*` directly.
    ///
    /// Bug BG: `db` is threaded into `RealPublishExecutor` so the
    /// production publish path uses `TwitterConnector::with_db` +
    /// `post_status_update_idempotent` — closes Bug AF's deferred
    /// swarm-path threading for the persistent idempotency cache.
    pub fn new(
        publish_state: Arc<dyn PublishStateHandle>,
        facade: Arc<SecretsFacade>,
        db: Arc<nexus_persistence::NexusDatabase>,
    ) -> Self {
        // Bug BK.3: store the un-wrapped
        // RealPublishExecutor; execute() builds the
        // per-run retry decorator with a captured
        // emitter. retry_config = Some(default)
        // signals "wrap when executing."
        Self {
            publish_state,
            publish_executor: Arc::new(RealPublishExecutor::new(facade, db)),
            retry_config: Some(RetryConfig::default()),
        }
    }

    /// Test seam: inject a stub `PublishExecutor` so behavior tests
    /// can exercise the V branches without touching the network.
    pub fn with_publish_executor(
        publish_state: Arc<dyn PublishStateHandle>,
        publish_executor: Arc<dyn PublishExecutor>,
    ) -> Self {
        // Bug BK.3: test seam — retry_config=None
        // skips the per-run wrap so injected stubs
        // exercise their own publish behavior
        // exactly once per execute() call.
        Self {
            publish_state,
            publish_executor,
            retry_config: None,
        }
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

        // Bug W: real per-channel post-count drives the compliance gate.
        // Read happens unconditionally — dry_run still benefits from the
        // accurate "would I be rate-limited?" preview signal. Increment
        // does NOT happen here; V wires `record_publish` on real publish.
        let channel_key = ChannelKey::new(parsed.channel, parsed.account_id.clone());
        let recent_posts = self
            .publish_state
            .recent_post_count(&channel_key, COMPLIANCE_WINDOW)
            .await
            .map_err(|e: PublishStateError| AgentError::Internal(format!("{e}")))?;
        ctx.emit
            .emit_phase(
                "counting_recent_posts",
                json!({
                    "channel": platform_label(parsed.channel),
                    "account_id": parsed.account_id,
                    "window_secs": COMPLIANCE_WINDOW.as_secs(),
                    "recent_posts": recent_posts,
                }),
            )
            .await;

        let decision = check_compliance(parsed.channel, recent_posts);
        let allowed = matches!(decision, ComplianceDecision::Allowed);
        let decision_label = match &decision {
            ComplianceDecision::Allowed => "allowed".to_string(),
            ComplianceDecision::Blocked(reason) => format!("blocked: {reason}"),
        };
        ctx.emit
            .emit_phase(
                "reviewing",
                json!({
                    "allowed": allowed,
                    "decision": decision_label,
                    "recent_posts": recent_posts,
                }),
            )
            .await;

        // Publishing — gated on compliance, dry_run, then credentials.
        // Bug V: the real-publish branch lives below the compliance
        // and dry_run gates; only the (allowed && !dry_run) intersection
        // calls the connector.
        let publish_status: PublishStatus = if !allowed {
            PublishStatus::BlockedByCompliance
        } else if parsed.dry_run {
            // W: dry_run reads count, does not record. V keeps this
            // path unchanged — dry_run never reaches the connector.
            PublishStatus::SkippedDryRun
        } else {
            // V: real publish path. Pre-flight cred check first so
            // we fail fast and don't silently fall through to the
            // connector's mock mode.
            let creds_ok = self.publish_executor.credentials_present();
            ctx.emit
                .emit_phase(
                    "checking_credentials",
                    json!({
                        "channel": platform_label(parsed.channel),
                        "credentials_present": creds_ok,
                    }),
                )
                .await;
            if !creds_ok {
                PublishStatus::CredentialsMissing
            } else {
                if ctx.cancelled() {
                    return Err(AgentError::Cancelled);
                }
                ctx.emit
                    .emit_phase(
                        "publishing",
                        json!({
                            "channel": platform_label(parsed.channel),
                            "account_id": parsed.account_id,
                            "draft_chars": draft.chars().count(),
                        }),
                    )
                    .await;
                // Bug BK.3: build the per-run retry decorator with a
                // captured emitter so each retry attempt emits a
                // NodeEvent retry_attempt. Tests using
                // with_publish_executor have retry_config=None and
                // skip the wrap.
                let publish_result = match &self.retry_config {
                    Some(cfg) => {
                        let decorator = RetryingPublishExecutor::with_emitter(
                            Arc::clone(&self.publish_executor),
                            *cfg,
                            Arc::clone(&ctx.emit),
                        );
                        decorator.publish(draft.clone()).await
                    }
                    None => self.publish_executor.publish(draft.clone()).await,
                };
                let status = match publish_result {
                    Ok(TweetResult { tweet_id, .. }) => {
                        // Bug V: record_publish only on confirmed success.
                        // Bug AH tracks the reserve→confirm pattern that
                        // would close the under-counting hole on partial
                        // failures.
                        let hash = content_hash_for(parsed.channel, &parsed.account_id, &draft);
                        if let Err(e) = self
                            .publish_state
                            .record_publish(&channel_key, Some(hash), Some(tweet_id.clone()))
                            .await
                        {
                            // The post landed; the audit row is the only
                            // thing missing. Log and continue — Bug AH
                            // catalogues the consistency hole.
                            tracing::error!(
                                "publish recorded on Twitter but record_publish failed: {e}"
                            );
                        }
                        PublishStatus::Published {
                            url: Some(format!("https://twitter.com/i/web/status/{tweet_id}")),
                            post_id: tweet_id,
                        }
                    }
                    Err(KernelAgentError::FuelExhausted) => {
                        // Budget exhaustion is a swarm-level concern,
                        // not a publish status. Bubble through.
                        return Err(AgentError::Internal("fuel_exhausted".into()));
                    }
                    Err(KernelAgentError::CapabilityDenied(cap)) => {
                        // Capabilities are constructed locally; this
                        // should be unreachable in production. Surface
                        // as Failed so the audit trail captures it.
                        PublishStatus::Failed {
                            reason: format!("capability_denied:{cap}"),
                        }
                    }
                    Err(KernelAgentError::SupervisorError(msg)) => classify_publish_error(&msg),
                    Err(other) => {
                        // Other AgentError variants (FuelViolation,
                        // ApprovalRequired, etc.) — coordinator-level
                        // signals, not publish outcomes.
                        return Err(AgentError::Internal(format!("publish: {other}")));
                    }
                };
                ctx.emit
                    .emit_phase(
                        "publish_complete",
                        json!({
                            "channel": platform_label(parsed.channel),
                            "publish_status": publish_status_label(&status),
                        }),
                    )
                    .await;
                // Bug AE: re-route the failure-shaped statuses to typed
                // Err so the swarm coordinator (and Bug BG's V2 retry
                // loop) sees the structured retry hint without parsing
                // strings. Audit emission (`publish_complete` event)
                // already fired above, so the per-status row is
                // preserved across both Ok and Err paths.
                match status {
                    PublishStatus::RateLimited { retry_after_secs } => {
                        return Err(AgentError::PublishFailed {
                            reason: "rate limited".into(),
                            retryable: true,
                            retry_after_secs,
                        });
                    }
                    PublishStatus::Failed { reason } => {
                        return Err(AgentError::PublishFailed {
                            reason,
                            retryable: false,
                            retry_after_secs: None,
                        });
                    }
                    PublishStatus::AuthFailure => {
                        return Err(AgentError::PublishFailed {
                            reason: "auth failure".into(),
                            retryable: false,
                            retry_after_secs: None,
                        });
                    }
                    other => other,
                }
            }
        };

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
        PublishStatus::Published { .. } => "published",
        PublishStatus::RateLimited { .. } => "rate_limited",
        PublishStatus::AuthFailure => "auth_failure",
        PublishStatus::CredentialsMissing => "credentials_missing",
        PublishStatus::Failed { .. } => "failed",
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
    use nexus_kernel::config::CredentialFacadeConfig;
    use nexus_kernel::secrets::backend_env::EnvBackend;
    use nexus_kernel::secrets::backend_keyring::KeyringBackendAdapter;
    use nexus_kernel::secrets::backend_memory::MemoryBackend;
    use nexus_kernel::secrets::Zeroizing;
    use nexus_swarm_core::emitter::recording::{Recorded, RecordingEmitter};
    use nexus_swarm_core::{
        CancelToken, CostClass, ModelDescriptor, NodeBudget, PrivacyClass, Provider,
        ProviderCapabilities, ProviderError, ProviderHealth, ProviderHealthStatus, ReasoningTier,
    };
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use uuid::Uuid;

    /// Bug BG: in-memory NexusDatabase for unit tests. Mirrors the
    /// pattern in `kernel/src/secrets/tests.rs:34`. Returns a fresh
    /// db per call so tests do not share idempotency cache state.
    fn test_db() -> Arc<nexus_persistence::NexusDatabase> {
        Arc::new(nexus_persistence::NexusDatabase::in_memory().expect("in-memory db"))
    }

    /// Bug AK Commit 2: build a Memory-only facade for unit tests.
    /// Construction returns a fresh facade per call; pass
    /// pre-populated `(scope, name, value)` tuples to seed.
    fn test_facade(seed: &[(&str, &str, &str)]) -> Arc<SecretsFacade> {
        let env = Arc::new(EnvBackend::new());
        let kr = Arc::new(KeyringBackendAdapter::os_keyring());
        let mem = Arc::new(MemoryBackend::new());
        // Seed memory backend before wrapping it.
        for (scope, name, value) in seed {
            use nexus_kernel::secrets::SecretBackend;
            mem.set(scope, name, Zeroizing::new(value.to_string()))
                .expect("seed memory backend");
        }
        let audit = Arc::new(std::sync::Mutex::new(nexus_kernel::audit::AuditTrail::new()));
        Arc::new(SecretsFacade::new(
            env,
            kr,
            None,
            mem,
            &CredentialFacadeConfig::default(),
            audit,
        ))
    }

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
        let out = SocialPosterEntry::new(
            std::sync::Arc::new(crate::publish_state::InMemoryPublishState::new()),
            test_facade(&[]),
            test_db(),
        )
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
    async fn explicit_dry_run_false_with_no_creds_returns_credentials_missing() {
        // V supersedes the W-era `Deferred` path. With dry_run=false
        // and no Twitter creds in the test environment,
        // `RealPublishExecutor::credentials_present` returns false and
        // the entry short-circuits at `CredentialsMissing` without
        // touching the connector.
        let rec = Arc::new(RecordingEmitter::new());
        let ctx = mk_ctx(rec, CancelToken::new(), "post text");
        let input = json!({
            "channel": "X",
            "audience": "audience",
            "message": "msg",
            "dry_run": false,
        });
        let out = SocialPosterEntry::new(
            std::sync::Arc::new(crate::publish_state::InMemoryPublishState::new()),
            test_facade(&[]),
            test_db(),
        )
        .execute(input, &ctx)
        .await
        .expect("ok");
        let parsed: SocialPosterOutput = serde_json::from_value(out).unwrap();
        assert!(!parsed.dry_run);
        assert!(
            matches!(parsed.publish_status, PublishStatus::CredentialsMissing),
            "expected CredentialsMissing under no-creds test env, got {:?}",
            parsed.publish_status
        );
    }

    #[tokio::test]
    async fn invalid_input_returns_invalid_input_error() {
        let rec = Arc::new(RecordingEmitter::new());
        let ctx = mk_ctx(rec, CancelToken::new(), "anything");
        let input = json!({ "channel": "X", "audience": "devs" });
        let err = SocialPosterEntry::new(
            std::sync::Arc::new(crate::publish_state::InMemoryPublishState::new()),
            test_facade(&[]),
            test_db(),
        )
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
        let err = SocialPosterEntry::new(
            std::sync::Arc::new(crate::publish_state::InMemoryPublishState::new()),
            test_facade(&[]),
            test_db(),
        )
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
        let err = SocialPosterEntry::new(
            std::sync::Arc::new(crate::publish_state::InMemoryPublishState::new()),
            test_facade(&[]),
            test_db(),
        )
        .execute(input, &ctx)
        .await
        .expect_err("expected Cancelled");
        assert!(matches!(err, AgentError::Cancelled));
    }

    #[tokio::test]
    async fn dry_run_emits_six_phases_in_order() {
        // V renamed/reshaped from the W-era `emits_seven_phases_in_order`.
        // dry_run never reaches the connector, so V's
        // checking_credentials / publishing / publish_complete events
        // do not fire. The W-era terminal "publishing" emission was
        // dropped in V (its "publishing" now means "about to invoke
        // the connector" — a different semantic). Net effect: the
        // dry_run path emits 6 phases, not 7.
        let rec = Arc::new(RecordingEmitter::new());
        let ctx = mk_ctx(rec.clone(), CancelToken::new(), "post text");
        let input = json!({
            "channel": "X",
            "audience": "devs",
            "message": "ship it",
        });
        SocialPosterEntry::new(
            std::sync::Arc::new(crate::publish_state::InMemoryPublishState::new()),
            test_facade(&[]),
            test_db(),
        )
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
                "counting_recent_posts",
                "reviewing",
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
        let out = SocialPosterEntry::new(
            std::sync::Arc::new(crate::publish_state::InMemoryPublishState::new()),
            test_facade(&[]),
            test_db(),
        )
        .execute(input, &ctx)
        .await
        .expect("ok");
        let parsed: SocialPosterOutput = serde_json::from_value(out).unwrap();
        assert!(parsed.draft.chars().count() <= 280);
        assert!(parsed.draft.ends_with('…'));
    }

    // ── Bug W: per-channel post-count drives the compliance gate ────────

    fn now_secs() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0)
    }

    async fn count_recorded(
        state: &crate::publish_state::InMemoryPublishState,
        key: &ChannelKey,
    ) -> usize {
        // Use the trait surface to assert "no record_publish call" by
        // measuring the post count before/after across an arbitrarily
        // wide window. record_publish stamps now(); seeded posts via
        // insert_at use a known epoch so reads are deterministic.
        state
            .recent_post_count(key, std::time::Duration::from_secs(86_400))
            .await
            .expect("ok")
    }

    #[tokio::test]
    async fn w_dry_run_with_seeded_count_under_limit_skips_publish_and_does_not_increment() {
        let rec = Arc::new(RecordingEmitter::new());
        let ctx = mk_ctx(rec.clone(), CancelToken::new(), "Body of the post.");
        let state = std::sync::Arc::new(crate::publish_state::InMemoryPublishState::new());
        let key = ChannelKey::new(SocialPlatform::X, "default");
        // Seed 7 recent posts (well under X's 300 limit).
        let now = now_secs();
        for i in 0..7 {
            state.insert_at(key.clone(), now - (i * 60));
        }
        let before = count_recorded(&state, &key).await;
        assert_eq!(before, 7);

        let input = json!({
            "channel": "X",
            "audience": "Rust devs",
            "message": "drafty",
            "dry_run": true,
        });
        let out = SocialPosterEntry::new(state.clone(), test_facade(&[]), test_db())
            .execute(input, &ctx)
            .await
            .expect("ok");
        let parsed: SocialPosterOutput = serde_json::from_value(out).unwrap();
        assert!(matches!(
            parsed.publish_status,
            PublishStatus::SkippedDryRun
        ));
        assert!(parsed.dry_run);

        // The counting_recent_posts emit must reflect the seeded count.
        let snap = rec.snapshot().await;
        let recent_posts_event = snap
            .iter()
            .find_map(|r| match r {
                Recorded::Phase { phase, payload, .. } if phase == "counting_recent_posts" => {
                    Some(payload.clone())
                }
                _ => None,
            })
            .expect("counting_recent_posts phase emitted");
        assert_eq!(
            recent_posts_event
                .get("recent_posts")
                .and_then(|v| v.as_u64()),
            Some(7),
            "compliance gate must read seeded count, not 0"
        );

        // No record_publish was called: count is unchanged.
        let after = count_recorded(&state, &key).await;
        assert_eq!(after, before, "dry_run path must not record");
    }

    #[tokio::test]
    async fn w_count_at_or_above_limit_blocks_by_compliance_even_when_dry_run_true() {
        let rec = Arc::new(RecordingEmitter::new());
        let ctx = mk_ctx(rec, CancelToken::new(), "ignored draft");
        let state = std::sync::Arc::new(crate::publish_state::InMemoryPublishState::new());
        // Instagram limit is 25 — seed exactly 25 to trigger Blocked.
        let key = ChannelKey::new(SocialPlatform::Instagram, "default");
        let now = now_secs();
        for i in 0..25 {
            state.insert_at(key.clone(), now - (i * 30));
        }

        let input = json!({
            "channel": "Instagram",
            "audience": "creators",
            "message": "draft",
            "dry_run": true,
        });
        let out = SocialPosterEntry::new(state.clone(), test_facade(&[]), test_db())
            .execute(input, &ctx)
            .await
            .expect("ok");
        let parsed: SocialPosterOutput = serde_json::from_value(out).unwrap();
        assert!(
            matches!(parsed.publish_status, PublishStatus::BlockedByCompliance),
            "expected BlockedByCompliance, got {:?}",
            parsed.publish_status
        );
        // Compliance block wins over dry_run path — count unchanged.
        let after = count_recorded(&state, &key).await;
        assert_eq!(after, 25);
    }

    #[tokio::test]
    async fn w_dry_run_false_no_creds_does_not_record() {
        // V supersedes Deferred: with dry_run=false and no creds, the
        // V flow returns CredentialsMissing without invoking the
        // connector. Either way (W's Deferred or V's CredentialsMissing)
        // the invariant is the same — record_publish must NOT fire on
        // the no-creds path.
        let rec = Arc::new(RecordingEmitter::new());
        let ctx = mk_ctx(rec, CancelToken::new(), "real-run draft");
        let state = std::sync::Arc::new(crate::publish_state::InMemoryPublishState::new());
        let key = ChannelKey::new(SocialPlatform::X, "default");
        let before = count_recorded(&state, &key).await;

        let input = json!({
            "channel": "X",
            "audience": "devs",
            "message": "ship it",
            "dry_run": false,
        });
        let out = SocialPosterEntry::new(state.clone(), test_facade(&[]), test_db())
            .execute(input, &ctx)
            .await
            .expect("ok");
        let parsed: SocialPosterOutput = serde_json::from_value(out).unwrap();
        assert!(matches!(
            parsed.publish_status,
            PublishStatus::CredentialsMissing
        ));
        let after = count_recorded(&state, &key).await;
        assert_eq!(after, before, "no-creds path must not record");
    }

    #[tokio::test]
    async fn w_account_id_isolates_compliance_decision_per_channel_key() {
        // Same platform, two account_ids: one over the limit, one empty.
        // The over-limit account must be blocked; the empty one must
        // pass. Confirms the gate keys on (platform, account_id), not
        // platform alone.
        let rec_a = Arc::new(RecordingEmitter::new());
        let rec_b = Arc::new(RecordingEmitter::new());
        let state = std::sync::Arc::new(crate::publish_state::InMemoryPublishState::new());
        let blocked_key = ChannelKey::new(SocialPlatform::Instagram, "acct-blocked");
        let now = now_secs();
        for i in 0..25 {
            state.insert_at(blocked_key.clone(), now - (i * 30));
        }

        let ctx_a = mk_ctx(rec_a, CancelToken::new(), "draft");
        let blocked_out = SocialPosterEntry::new(state.clone(), test_facade(&[]), test_db())
            .execute(
                json!({
                    "channel": "Instagram",
                    "audience": "x",
                    "message": "y",
                    "account_id": "acct-blocked",
                }),
                &ctx_a,
            )
            .await
            .expect("ok");
        let blocked_parsed: SocialPosterOutput = serde_json::from_value(blocked_out).unwrap();
        assert!(matches!(
            blocked_parsed.publish_status,
            PublishStatus::BlockedByCompliance
        ));

        let ctx_b = mk_ctx(rec_b, CancelToken::new(), "draft");
        let allowed_out = SocialPosterEntry::new(state.clone(), test_facade(&[]), test_db())
            .execute(
                json!({
                    "channel": "Instagram",
                    "audience": "x",
                    "message": "y",
                    "account_id": "acct-fresh",
                }),
                &ctx_b,
            )
            .await
            .expect("ok");
        let allowed_parsed: SocialPosterOutput = serde_json::from_value(allowed_out).unwrap();
        assert!(matches!(
            allowed_parsed.publish_status,
            PublishStatus::SkippedDryRun
        ));
    }

    // ── Bug V: real publish wiring (stub executor) ─────────────────────

    /// Test PublishExecutor stub. Configurable creds-present flag and
    /// scripted publish outcomes. Records the publish-call payload so
    /// tests can assert what the entry hands to the executor.
    struct StubExecutor {
        creds_present: bool,
        outcome: tokio::sync::Mutex<StubOutcome>,
        last_text: tokio::sync::Mutex<Option<String>>,
    }

    enum StubOutcome {
        Ok(TweetResult),
        SupervisorErr(String),
        Capability(String),
        Fuel,
    }

    impl StubExecutor {
        fn ok(creds: bool, tweet_id: &str) -> Arc<Self> {
            Arc::new(Self {
                creds_present: creds,
                outcome: tokio::sync::Mutex::new(StubOutcome::Ok(TweetResult {
                    tweet_id: tweet_id.into(),
                    posted_at: 0,
                })),
                last_text: tokio::sync::Mutex::new(None),
            })
        }
        fn supervisor_err(creds: bool, msg: &str) -> Arc<Self> {
            Arc::new(Self {
                creds_present: creds,
                outcome: tokio::sync::Mutex::new(StubOutcome::SupervisorErr(msg.into())),
                last_text: tokio::sync::Mutex::new(None),
            })
        }
        fn no_creds() -> Arc<Self> {
            Arc::new(Self {
                creds_present: false,
                outcome: tokio::sync::Mutex::new(StubOutcome::SupervisorErr(
                    "should-not-be-called".into(),
                )),
                last_text: tokio::sync::Mutex::new(None),
            })
        }
        fn capability_denied(creds: bool, cap: &str) -> Arc<Self> {
            Arc::new(Self {
                creds_present: creds,
                outcome: tokio::sync::Mutex::new(StubOutcome::Capability(cap.into())),
                last_text: tokio::sync::Mutex::new(None),
            })
        }
        fn fuel_exhausted(creds: bool) -> Arc<Self> {
            Arc::new(Self {
                creds_present: creds,
                outcome: tokio::sync::Mutex::new(StubOutcome::Fuel),
                last_text: tokio::sync::Mutex::new(None),
            })
        }
    }

    #[async_trait]
    impl PublishExecutor for StubExecutor {
        fn credentials_present(&self) -> bool {
            self.creds_present
        }
        async fn publish_with_request_id(
            &self,
            text: String,
            _request_id: Uuid,
        ) -> Result<TweetResult, KernelAgentError> {
            *self.last_text.lock().await = Some(text);
            let outcome = std::mem::replace(
                &mut *self.outcome.lock().await,
                StubOutcome::SupervisorErr("consumed".into()),
            );
            match outcome {
                StubOutcome::Ok(r) => Ok(r),
                StubOutcome::SupervisorErr(msg) => Err(KernelAgentError::SupervisorError(msg)),
                StubOutcome::Capability(c) => Err(KernelAgentError::CapabilityDenied(c)),
                StubOutcome::Fuel => Err(KernelAgentError::FuelExhausted),
            }
        }
    }

    fn v_input(account_id: &str, dry_run: bool) -> Value {
        json!({
            "channel": "X",
            "audience": "devs",
            "message": "ship it",
            "account_id": account_id,
            "dry_run": dry_run,
        })
    }

    #[tokio::test]
    async fn v_dry_run_with_creds_still_skipped_no_publish_no_record() {
        let rec = Arc::new(RecordingEmitter::new());
        let ctx = mk_ctx(rec.clone(), CancelToken::new(), "draft body");
        let state = std::sync::Arc::new(crate::publish_state::InMemoryPublishState::new());
        let executor = StubExecutor::ok(true, "tweet_should_not_be_used");
        let entry = SocialPosterEntry::with_publish_executor(state.clone(), executor.clone());
        let out = entry
            .execute(v_input("default", true), &ctx)
            .await
            .expect("ok");
        let parsed: SocialPosterOutput = serde_json::from_value(out).unwrap();
        assert!(matches!(
            parsed.publish_status,
            PublishStatus::SkippedDryRun
        ));
        assert!(
            executor.last_text.lock().await.is_none(),
            "publish must not be called"
        );
        // No record_publish either.
        let key = ChannelKey::new(SocialPlatform::X, "default");
        assert_eq!(count_recorded(&state, &key).await, 0);
    }

    #[tokio::test]
    async fn v_creds_missing_returns_credentials_missing_no_publish_no_record() {
        let rec = Arc::new(RecordingEmitter::new());
        let ctx = mk_ctx(rec.clone(), CancelToken::new(), "draft body");
        let state = std::sync::Arc::new(crate::publish_state::InMemoryPublishState::new());
        let executor = StubExecutor::no_creds();
        let entry = SocialPosterEntry::with_publish_executor(state.clone(), executor.clone());
        let out = entry
            .execute(v_input("default", false), &ctx)
            .await
            .expect("ok");
        let parsed: SocialPosterOutput = serde_json::from_value(out).unwrap();
        assert!(matches!(
            parsed.publish_status,
            PublishStatus::CredentialsMissing
        ));
        assert!(executor.last_text.lock().await.is_none());
        let key = ChannelKey::new(SocialPlatform::X, "default");
        assert_eq!(count_recorded(&state, &key).await, 0);
        // checking_credentials event must have fired with present=false.
        let snap = rec.snapshot().await;
        let cc = snap
            .iter()
            .find_map(|r| match r {
                Recorded::Phase { phase, payload, .. } if phase == "checking_credentials" => {
                    Some(payload.clone())
                }
                _ => None,
            })
            .expect("checking_credentials emitted");
        assert_eq!(
            cc.get("credentials_present").and_then(|v| v.as_bool()),
            Some(false)
        );
    }

    #[tokio::test]
    async fn v_publish_success_returns_published_and_records_post_id() {
        let rec = Arc::new(RecordingEmitter::new());
        let ctx = mk_ctx(rec.clone(), CancelToken::new(), "draft body");
        let state = std::sync::Arc::new(crate::publish_state::InMemoryPublishState::new());
        let executor = StubExecutor::ok(true, "fake_123");
        let entry = SocialPosterEntry::with_publish_executor(state.clone(), executor.clone());
        let out = entry
            .execute(v_input("default", false), &ctx)
            .await
            .expect("ok");
        let parsed: SocialPosterOutput = serde_json::from_value(out).unwrap();
        match parsed.publish_status {
            PublishStatus::Published { post_id, url } => {
                assert_eq!(post_id, "fake_123");
                assert_eq!(
                    url.as_deref(),
                    Some("https://twitter.com/i/web/status/fake_123")
                );
            }
            other => panic!("expected Published, got {other:?}"),
        }
        // record_publish fired exactly once with the post_id.
        let key = ChannelKey::new(SocialPlatform::X, "default");
        let entries = state.entries_for(&key);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].post_id.as_deref(), Some("fake_123"));
        // The publish call received the trimmed/draft text.
        let last = executor.last_text.lock().await.clone();
        assert!(last.is_some(), "publish must have been called");
    }

    #[tokio::test]
    async fn v_published_path_emits_nine_phases_in_order() {
        // V replaces W's emits_seven_phases_in_order with a 9-phase
        // happy-path assertion (W's terminal "publishing" emission was
        // dropped; V emits "checking_credentials", "publishing",
        // "publish_complete" only on the real-publish path).
        let rec = Arc::new(RecordingEmitter::new());
        let ctx = mk_ctx(rec.clone(), CancelToken::new(), "draft body");
        let state = std::sync::Arc::new(crate::publish_state::InMemoryPublishState::new());
        let executor = StubExecutor::ok(true, "tweet_xyz");
        let entry = SocialPosterEntry::with_publish_executor(state, executor);
        entry
            .execute(v_input("default", false), &ctx)
            .await
            .expect("ok");
        let phases: Vec<&str> = rec
            .snapshot()
            .await
            .iter()
            .filter_map(|r| match r {
                Recorded::Phase { phase, .. } => Some(phase.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .into_iter()
            .map(|s| Box::leak(s.to_string().into_boxed_str()) as &str)
            .collect();
        assert_eq!(
            phases,
            vec![
                "parsing_input",
                "drafting",
                "parsing_response",
                "counting_recent_posts",
                "reviewing",
                "checking_credentials",
                "publishing",
                "publish_complete",
                "complete",
            ]
        );
    }

    #[tokio::test]
    async fn v_rate_limit_message_parses_to_seconds() {
        // Bug AE: rate-limited publishes now surface as
        // Err(AgentError::PublishFailed { retryable: true, .. }) so the
        // coordinator (and Bug BG's V2 retry loop) can act on the
        // typed retry hint without parsing strings. The
        // `publish_complete` phase event STILL fires before the Err
        // return, preserving the audit row across the new wire shape.
        let rec = Arc::new(RecordingEmitter::new());
        let ctx = mk_ctx(rec.clone(), CancelToken::new(), "draft body");
        let state = std::sync::Arc::new(crate::publish_state::InMemoryPublishState::new());
        // Connector formats: "social.x rate limited, retry after 60000 ms"
        let executor =
            StubExecutor::supervisor_err(true, "social.x rate limited, retry after 60000 ms");
        let entry = SocialPosterEntry::with_publish_executor(state.clone(), executor);
        let err = entry
            .execute(v_input("default", false), &ctx)
            .await
            .expect_err("rate-limited publish must surface as AgentError::PublishFailed");
        match err {
            AgentError::PublishFailed {
                reason,
                retryable,
                retry_after_secs,
            } => {
                assert_eq!(reason, "rate limited");
                assert!(retryable);
                assert_eq!(retry_after_secs, Some(60));
            }
            other => panic!("expected PublishFailed, got {other:?}"),
        }
        // No record_publish on rate-limit.
        let key = ChannelKey::new(SocialPlatform::X, "default");
        assert_eq!(count_recorded(&state, &key).await, 0);
        // Bug AE PHASE G4: audit row preserved — `publish_complete`
        // phase event must still fire on the new Err path.
        let phases: Vec<String> = rec
            .snapshot()
            .await
            .iter()
            .filter_map(|r| match r {
                Recorded::Phase { phase, .. } => Some(phase.clone()),
                _ => None,
            })
            .collect();
        assert!(
            phases.iter().any(|p| p == "publish_complete"),
            "publish_complete phase missing from emissions: {phases:?}"
        );
    }

    #[tokio::test]
    async fn v_rate_limit_message_without_number_yields_none() {
        // The connector path with no parseable retry_after — fall back
        // to None rather than a fabricated number. Bug AE: surfaces as
        // Err with retryable=true, retry_after_secs=None.
        let state = std::sync::Arc::new(crate::publish_state::InMemoryPublishState::new());
        let executor = StubExecutor::supervisor_err(true, "rate limited but no number");
        let rec = Arc::new(RecordingEmitter::new());
        let ctx = mk_ctx(rec, CancelToken::new(), "d");
        let err = SocialPosterEntry::with_publish_executor(state, executor)
            .execute(v_input("default", false), &ctx)
            .await
            .expect_err("rate-limited publish must surface as AgentError::PublishFailed");
        assert!(matches!(
            err,
            AgentError::PublishFailed {
                retryable: true,
                retry_after_secs: None,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn v_auth_error_returns_auth_failure() {
        // Bug AE: auth failures surface as Err(AgentError::PublishFailed
        // { retryable: false, reason: "auth failure" }). User action
        // required to rotate creds — no auto-retry.
        let state = std::sync::Arc::new(crate::publish_state::InMemoryPublishState::new());
        let executor =
            StubExecutor::supervisor_err(true, "x request failed with status 401 Unauthorized");
        let rec = Arc::new(RecordingEmitter::new());
        let ctx = mk_ctx(rec, CancelToken::new(), "d");
        let err = SocialPosterEntry::with_publish_executor(state.clone(), executor)
            .execute(v_input("default", false), &ctx)
            .await
            .expect_err("auth failure must surface as AgentError::PublishFailed");
        match err {
            AgentError::PublishFailed {
                reason,
                retryable,
                retry_after_secs,
            } => {
                assert_eq!(reason, "auth failure");
                assert!(!retryable);
                assert_eq!(retry_after_secs, None);
            }
            other => panic!("expected PublishFailed, got {other:?}"),
        }
        let key = ChannelKey::new(SocialPlatform::X, "default");
        assert_eq!(count_recorded(&state, &key).await, 0);
    }

    #[tokio::test]
    async fn v_generic_5xx_returns_failed_with_reason() {
        // Bug AE: generic 5xx publishes surface as
        // Err(AgentError::PublishFailed { retryable: false, reason }).
        // Coordinator-level retry policy decides whether to attempt
        // again; this commit only exposes the typed shape.
        let state = std::sync::Arc::new(crate::publish_state::InMemoryPublishState::new());
        let executor = StubExecutor::supervisor_err(
            true,
            "x request failed with status 503 Service Unavailable",
        );
        let rec = Arc::new(RecordingEmitter::new());
        let ctx = mk_ctx(rec, CancelToken::new(), "d");
        let err = SocialPosterEntry::with_publish_executor(state.clone(), executor)
            .execute(v_input("default", false), &ctx)
            .await
            .expect_err("generic publish failure must surface as AgentError::PublishFailed");
        match err {
            AgentError::PublishFailed {
                reason,
                retryable,
                retry_after_secs,
            } => {
                assert!(reason.contains("503"), "expected 503 in reason: {reason}");
                assert!(!retryable);
                assert_eq!(retry_after_secs, None);
            }
            other => panic!("expected PublishFailed, got {other:?}"),
        }
        let key = ChannelKey::new(SocialPlatform::X, "default");
        assert_eq!(count_recorded(&state, &key).await, 0);
    }

    #[tokio::test]
    async fn v_capability_denied_returns_failed() {
        // Bug AE: capability denied surfaces through PublishStatus::Failed
        // (synthesized inside the publish dispatch) and then re-routes
        // to Err(AgentError::PublishFailed { retryable: false }).
        let state = std::sync::Arc::new(crate::publish_state::InMemoryPublishState::new());
        let executor = StubExecutor::capability_denied(true, "social.x.post");
        let rec = Arc::new(RecordingEmitter::new());
        let ctx = mk_ctx(rec, CancelToken::new(), "d");
        let err = SocialPosterEntry::with_publish_executor(state, executor)
            .execute(v_input("default", false), &ctx)
            .await
            .expect_err("capability_denied must surface as AgentError::PublishFailed");
        match err {
            AgentError::PublishFailed {
                reason, retryable, ..
            } => {
                assert!(reason.contains("capability_denied"));
                assert!(!retryable);
            }
            other => panic!("expected PublishFailed, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn v_fuel_exhausted_bubbles_to_internal() {
        let state = std::sync::Arc::new(crate::publish_state::InMemoryPublishState::new());
        let executor = StubExecutor::fuel_exhausted(true);
        let rec = Arc::new(RecordingEmitter::new());
        let ctx = mk_ctx(rec, CancelToken::new(), "d");
        let err = SocialPosterEntry::with_publish_executor(state, executor)
            .execute(v_input("default", false), &ctx)
            .await
            .expect_err("fuel exhaustion must surface as AgentError, not PublishStatus");
        match err {
            AgentError::Internal(msg) => assert!(msg.contains("fuel_exhausted")),
            other => panic!("expected Internal, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn v_compliance_block_wins_over_real_publish_path() {
        let rec = Arc::new(RecordingEmitter::new());
        let ctx = mk_ctx(rec, CancelToken::new(), "d");
        let state = std::sync::Arc::new(crate::publish_state::InMemoryPublishState::new());
        // Seed Instagram (limit 25) past the gate.
        let key = ChannelKey::new(SocialPlatform::Instagram, "default");
        let now = now_secs();
        for i in 0..25 {
            state.insert_at(key.clone(), now - (i * 30));
        }
        let executor = StubExecutor::ok(true, "should_not_be_called");
        let entry = SocialPosterEntry::with_publish_executor(state.clone(), executor.clone());
        let out = entry
            .execute(
                json!({
                    "channel": "Instagram",
                    "audience": "x",
                    "message": "y",
                    "dry_run": false,
                }),
                &ctx,
            )
            .await
            .expect("ok");
        let parsed: SocialPosterOutput = serde_json::from_value(out).unwrap();
        assert!(matches!(
            parsed.publish_status,
            PublishStatus::BlockedByCompliance
        ));
        assert!(
            executor.last_text.lock().await.is_none(),
            "compliance block must short-circuit before publish"
        );
    }

    #[tokio::test]
    async fn v_content_hash_is_stable_for_same_inputs() {
        let h1 = content_hash_for(SocialPlatform::X, "default", "hello world");
        let h2 = content_hash_for(SocialPlatform::X, "default", "hello world");
        let h3 = content_hash_for(SocialPlatform::X, "other", "hello world");
        let h4 = content_hash_for(SocialPlatform::Instagram, "default", "hello world");
        assert_eq!(h1, h2, "same inputs must hash identically");
        assert_ne!(h1, h3, "account_id must affect the digest");
        assert_ne!(h1, h4, "platform must affect the digest");
        assert_eq!(h1.len(), 64, "sha256 hex must be 64 chars");
    }

    // ── Bug AK Commit 2: facade-threaded credentials_present ────────

    #[tokio::test]
    async fn ak2_credentials_present_false_on_empty_facade() {
        let facade = test_facade(&[]);
        let exec = RealPublishExecutor::new(facade, test_db());
        assert!(!exec.credentials_present());
    }

    #[tokio::test]
    async fn ak2_credentials_present_true_when_all_four_keys_in_facade() {
        let facade = test_facade(&[
            ("social", "x_consumer_key", "ck"),
            ("social", "x_consumer_secret", "cs"),
            ("social", "x_access_token", "at"),
            ("social", "x_access_token_secret", "ats"),
        ]);
        let exec = RealPublishExecutor::new(facade, test_db());
        assert!(exec.credentials_present());
    }

    #[tokio::test]
    async fn ak2_credentials_present_false_when_only_three_keys_present() {
        let facade = test_facade(&[
            ("social", "x_consumer_key", "ck"),
            ("social", "x_consumer_secret", "cs"),
            ("social", "x_access_token", "at"),
            // x_access_token_secret missing
        ]);
        let exec = RealPublishExecutor::new(facade, test_db());
        assert!(!exec.credentials_present());
    }

    #[tokio::test]
    async fn ak2_social_poster_entry_constructs_with_facade() {
        // Smoke test for the new SocialPosterEntry::new signature.
        // No execute call — just verify construction succeeds and
        // credentials_present reflects the seeded facade.
        let state = std::sync::Arc::new(crate::publish_state::InMemoryPublishState::new());
        let facade = test_facade(&[
            ("social", "x_consumer_key", "ck"),
            ("social", "x_consumer_secret", "cs"),
            ("social", "x_access_token", "at"),
            ("social", "x_access_token_secret", "ats"),
        ]);
        let _entry = SocialPosterEntry::new(state, Arc::clone(&facade), test_db());
        // Indirect assertion: build a separate executor with the
        // same facade and check it sees the seeded credentials.
        let exec = RealPublishExecutor::new(facade, test_db());
        assert!(exec.credentials_present());
    }

    // ===== Bug BK.2: retry decorator tests =====

    use crate::retry::{RetryConfig, RetryingPublishExecutor};

    /// Multi-shot scripted executor for retry-decorator tests.
    /// Each `publish_with_request_id` call pops the next outcome
    /// from the queue and records the request_id seen.
    struct ScriptableExecutor {
        creds_present: bool,
        outcomes: tokio::sync::Mutex<std::collections::VecDeque<StubOutcome>>,
        request_ids_seen: tokio::sync::Mutex<Vec<Uuid>>,
    }

    impl ScriptableExecutor {
        fn new(creds: bool, outcomes: Vec<StubOutcome>) -> Arc<Self> {
            Arc::new(Self {
                creds_present: creds,
                outcomes: tokio::sync::Mutex::new(outcomes.into()),
                request_ids_seen: tokio::sync::Mutex::new(Vec::new()),
            })
        }
    }

    #[async_trait]
    impl PublishExecutor for ScriptableExecutor {
        fn credentials_present(&self) -> bool {
            self.creds_present
        }

        async fn publish_with_request_id(
            &self,
            _text: String,
            request_id: Uuid,
        ) -> Result<TweetResult, KernelAgentError> {
            self.request_ids_seen.lock().await.push(request_id);
            let outcome =
                self.outcomes
                    .lock()
                    .await
                    .pop_front()
                    .unwrap_or(StubOutcome::SupervisorErr(
                        "scripted outcome exhausted".into(),
                    ));
            match outcome {
                StubOutcome::Ok(r) => Ok(r),
                StubOutcome::SupervisorErr(msg) => Err(KernelAgentError::SupervisorError(msg)),
                StubOutcome::Capability(c) => Err(KernelAgentError::CapabilityDenied(c)),
                StubOutcome::Fuel => Err(KernelAgentError::FuelExhausted),
            }
        }
    }

    /// Mirrors the literal TweetResult construction in
    /// StubExecutor::ok above. TweetResult does not derive Default.
    fn mk_tweet_result(tweet_id: &str) -> TweetResult {
        TweetResult {
            tweet_id: tweet_id.to_string(),
            posted_at: 0,
        }
    }

    fn fast_test_config(jitter: f64) -> RetryConfig {
        RetryConfig {
            max_attempts: 3,
            initial_backoff_ms: 1,
            backoff_multiplier: 2.0,
            max_backoff_secs: 1,
            retry_after_cap_secs: 1,
            jitter_pct: jitter,
        }
    }

    #[tokio::test]
    async fn bk2_retry_then_success_returns_ok() {
        let inner = ScriptableExecutor::new(
            true,
            vec![
                StubOutcome::SupervisorErr("social.x rate limited, retry after 1 ms".into()),
                StubOutcome::Ok(mk_tweet_result("tw-success")),
            ],
        );
        let decorator = RetryingPublishExecutor::new(inner.clone(), fast_test_config(0.0));
        let result = decorator.publish("hello".into()).await;
        match result {
            Ok(t) => assert_eq!(t.tweet_id, "tw-success"),
            Err(e) => panic!("expected Ok, got {e:?}"),
        }
        let ids = inner.request_ids_seen.lock().await;
        assert_eq!(ids.len(), 2, "two attempts expected (one retry)");
        assert_eq!(ids[0], ids[1], "request_id must be reused across retries");
    }

    #[tokio::test]
    async fn bk2_non_retryable_error_is_not_retried() {
        let inner = ScriptableExecutor::new(
            true,
            vec![
                StubOutcome::Capability("social.x.post".into()),
                StubOutcome::Ok(mk_tweet_result("must-not-reach")),
            ],
        );
        let decorator = RetryingPublishExecutor::new(inner.clone(), fast_test_config(0.0));
        let result = decorator.publish("hello".into()).await;
        assert!(matches!(result, Err(KernelAgentError::CapabilityDenied(_))));
        let ids = inner.request_ids_seen.lock().await;
        assert_eq!(ids.len(), 1, "non-retryable must not retry");
    }

    #[tokio::test]
    async fn bk2_retry_exhaustion_returns_last_error() {
        let inner = ScriptableExecutor::new(
            true,
            vec![
                StubOutcome::SupervisorErr("social.x rate limited, retry after 1 ms".into()),
                StubOutcome::SupervisorErr("social.x rate limited, retry after 1 ms".into()),
                StubOutcome::SupervisorErr("social.x rate limited, retry after 1 ms".into()),
            ],
        );
        let decorator = RetryingPublishExecutor::new(inner.clone(), fast_test_config(0.0));
        let result = decorator.publish("hello".into()).await;
        assert!(matches!(result, Err(KernelAgentError::SupervisorError(_))));
        let ids = inner.request_ids_seen.lock().await;
        assert_eq!(ids.len(), 3, "exactly max_attempts=3 calls");
        // request_id reused across all attempts
        let first = ids[0];
        for (i, id) in ids.iter().enumerate() {
            assert_eq!(*id, first, "id at attempt {} differs from first", i + 1);
        }
    }

    #[tokio::test]
    async fn bk2_server_hint_is_capped() {
        // Server says "retry after 600000 ms" (600s). Decorator
        // must cap at retry_after_cap_secs=1. Test budget: ~1.5s.
        let inner = ScriptableExecutor::new(
            true,
            vec![
                StubOutcome::SupervisorErr("social.x rate limited, retry after 600000 ms".into()),
                StubOutcome::Ok(mk_tweet_result("tw-after-cap")),
            ],
        );
        let decorator = RetryingPublishExecutor::new(inner, fast_test_config(0.0));
        let start = std::time::Instant::now();
        let result = decorator.publish("hello".into()).await;
        let elapsed = start.elapsed();
        assert!(result.is_ok(), "should succeed on retry");
        assert!(
            elapsed < std::time::Duration::from_millis(2000),
            "cap must bound server hint sleep; elapsed={:?}",
            elapsed
        );
    }

    #[tokio::test]
    async fn bk2_default_config_matches_adr_0005() {
        let c = RetryConfig::default();
        assert_eq!(c.max_attempts, 3);
        assert_eq!(c.initial_backoff_ms, 200);
        assert_eq!(c.backoff_multiplier, 2.0);
        assert_eq!(c.max_backoff_secs, 60);
        assert_eq!(c.retry_after_cap_secs, 300);
        assert_eq!(c.jitter_pct, 0.20);
    }

    // ===== Bug BK.3: NodeEvent emission tests =====
    // (Recorded + RecordingEmitter already imported at the top
    //  of this `mod tests` block.)

    /// Helper: extract retry_attempt phase events from a
    /// RecordingEmitter snapshot.
    async fn retry_attempt_events(rec: &RecordingEmitter) -> Vec<serde_json::Value> {
        rec.snapshot()
            .await
            .into_iter()
            .filter_map(|r| match r {
                Recorded::Phase { phase, payload } if phase == "retry_attempt" => Some(payload),
                _ => None,
            })
            .collect()
    }

    #[tokio::test]
    async fn bk3_emits_retry_attempt_on_retry() {
        let inner = ScriptableExecutor::new(
            true,
            vec![
                StubOutcome::SupervisorErr("social.x rate limited, retry after 1 ms".into()),
                StubOutcome::Ok(mk_tweet_result("tw-after-retry")),
            ],
        );
        let rec = Arc::new(RecordingEmitter::new());
        let emitter: Arc<dyn nexus_swarm_core::emitter::EventEmitter> = rec.clone();
        let decorator =
            RetryingPublishExecutor::with_emitter(inner, fast_test_config(0.0), emitter);
        let result = decorator.publish("hello".into()).await;
        assert!(result.is_ok(), "expected success after retry");
        let events = retry_attempt_events(&rec).await;
        assert_eq!(events.len(), 1, "exactly one retry_attempt event");
        let payload = &events[0];
        assert_eq!(
            payload["attempt_num"].as_u64(),
            Some(2),
            "first retry has attempt_num=2"
        );
        assert!(
            payload["wait_secs"].as_f64().is_some(),
            "wait_secs must be a number"
        );
        let summary = payload["last_error_summary"]
            .as_str()
            .expect("last_error_summary must be a string");
        assert!(
            summary.contains("rate limited"),
            "summary should reflect the underlying error: got {summary:?}"
        );
    }

    #[tokio::test]
    async fn bk3_no_retry_attempt_event_on_first_success() {
        let inner =
            ScriptableExecutor::new(true, vec![StubOutcome::Ok(mk_tweet_result("tw-immediate"))]);
        let rec = Arc::new(RecordingEmitter::new());
        let emitter: Arc<dyn nexus_swarm_core::emitter::EventEmitter> = rec.clone();
        let decorator =
            RetryingPublishExecutor::with_emitter(inner, fast_test_config(0.0), emitter);
        let result = decorator.publish("hello".into()).await;
        assert!(result.is_ok());
        let events = retry_attempt_events(&rec).await;
        assert!(
            events.is_empty(),
            "first-attempt success must not emit retry_attempt"
        );
    }

    #[tokio::test]
    async fn bk3_attempt_num_increments_across_retries() {
        let inner = ScriptableExecutor::new(
            true,
            vec![
                StubOutcome::SupervisorErr("social.x rate limited, retry after 1 ms".into()),
                StubOutcome::SupervisorErr("social.x rate limited, retry after 1 ms".into()),
                StubOutcome::Ok(mk_tweet_result("tw-third")),
            ],
        );
        let rec = Arc::new(RecordingEmitter::new());
        let emitter: Arc<dyn nexus_swarm_core::emitter::EventEmitter> = rec.clone();
        let decorator =
            RetryingPublishExecutor::with_emitter(inner, fast_test_config(0.0), emitter);
        let result = decorator.publish("hello".into()).await;
        assert!(result.is_ok(), "expected success on third attempt");
        let events = retry_attempt_events(&rec).await;
        assert_eq!(events.len(), 2, "two retry_attempt events for two retries");
        assert_eq!(events[0]["attempt_num"].as_u64(), Some(2));
        assert_eq!(events[1]["attempt_num"].as_u64(), Some(3));
    }
}
