//! Bug BK.2: V2 retry loop for the publish path.
//!
//! `RetryingPublishExecutor` wraps any `PublishExecutor` with a
//! decorator that classifies `KernelAgentError::SupervisorError`
//! payloads via the existing `classify_publish_error` helper,
//! retries on `PublishStatus::RateLimited`, sleeps with
//! multiplicative-jittered exponential backoff, and honors
//! `retry_after_secs` server hints (capped). All non-retryable
//! variants and non-`SupervisorError` errors propagate immediately
//! (fail-closed).
//!
//! Pinned by ADR 0005 (with BK.2 amendments). Trait return type
//! remains `KernelAgentError`; classification happens at the
//! decorator boundary using the existing helper. Migration to
//! `swarm_core::AgentError`-typed publish results is filed as
//! follow-up `BN-RETRY-CLASSIFY-MIGRATION`.
//!
//! `NodeEvent` emission is BK.3 work; this commit emits via
//! `tracing` only.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use nexus_connectors_web::twitter::TweetResult;
use rand::Rng;
use uuid::Uuid;

use crate::swarm_entry::{
    classify_publish_error, KernelAgentError, PublishExecutor, PublishStatus,
};

/// Retry policy. Defaults match ADR 0005:
/// - 3 attempts total (2 retries)
/// - 200ms initial backoff, 2.0× multiplier, 60s cap
/// - ±20% multiplicative jitter
/// - Server `retry_after_secs` hint capped at 300s
#[derive(Debug, Clone, Copy)]
pub struct RetryConfig {
    pub max_attempts: u8,
    pub initial_backoff_ms: u64,
    pub backoff_multiplier: f64,
    pub max_backoff_secs: u64,
    pub retry_after_cap_secs: u64,
    pub jitter_pct: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_backoff_ms: 200,
            backoff_multiplier: 2.0,
            max_backoff_secs: 60,
            retry_after_cap_secs: 300,
            jitter_pct: 0.20,
        }
    }
}

pub struct RetryingPublishExecutor {
    inner: Arc<dyn PublishExecutor>,
    config: RetryConfig,
}

impl RetryingPublishExecutor {
    pub fn new(inner: Arc<dyn PublishExecutor>, config: RetryConfig) -> Self {
        Self { inner, config }
    }

    /// Compute backoff for a given retry sequence number.
    /// `retry_number = 1` is the sleep BEFORE the second attempt
    /// (i.e., before the first retry). Cap is applied before
    /// jitter so the jittered value can briefly exceed cap by
    /// the jitter percentage; this is intentional and matches
    /// common backoff implementations.
    fn compute_backoff(&self, retry_number: u32) -> Duration {
        let exp = self
            .config
            .backoff_multiplier
            .powi((retry_number.saturating_sub(1)) as i32);
        let base_ms = (self.config.initial_backoff_ms as f64) * exp;
        let cap_ms = (self.config.max_backoff_secs as f64) * 1000.0;
        let bounded = base_ms.min(cap_ms);
        let jitter: f64 = if self.config.jitter_pct == 0.0 {
            0.0
        } else {
            rand::thread_rng().gen_range(-self.config.jitter_pct..=self.config.jitter_pct)
        };
        let jittered = (bounded * (1.0 + jitter)).max(0.0);
        Duration::from_millis(jittered as u64)
    }

    /// Apply the `retry_after_cap_secs` ceiling to a
    /// server-supplied hint.
    fn cap_server_hint(&self, hint_secs: u64) -> Duration {
        Duration::from_secs(hint_secs.min(self.config.retry_after_cap_secs))
    }
}

#[async_trait]
impl PublishExecutor for RetryingPublishExecutor {
    fn credentials_present(&self) -> bool {
        self.inner.credentials_present()
    }

    async fn publish_with_request_id(
        &self,
        text: String,
        request_id: Uuid,
    ) -> Result<TweetResult, KernelAgentError> {
        let max_attempts = self.config.max_attempts.max(1);
        let mut attempt: u8 = 1;
        loop {
            let result = self
                .inner
                .publish_with_request_id(text.clone(), request_id)
                .await;
            match result {
                Ok(r) => return Ok(r),
                Err(e) => {
                    if attempt >= max_attempts {
                        tracing::warn!(
                            request_id = %request_id,
                            attempt,
                            max_attempts,
                            "publish retry exhausted"
                        );
                        return Err(e);
                    }
                    let (retryable, retry_after_secs) = classify_for_retry(&e);
                    if !retryable {
                        return Err(e);
                    }
                    let retry_number = attempt as u32;
                    let wait = match retry_after_secs {
                        Some(hint) => self.cap_server_hint(hint),
                        None => self.compute_backoff(retry_number),
                    };
                    // TODO(PB-2): Budget::try_consume(wall_ms = wait.as_millis())
                    // once coordinator wires Budget mutation.
                    let last_error_summary = truncate_error(&e.to_string(), 200);
                    let next_attempt_num = (attempt as u32) + 1;
                    tracing::info!(
                        request_id = %request_id,
                        attempt_num = next_attempt_num,
                        wait_secs = wait.as_secs_f64(),
                        last_error_summary = %last_error_summary,
                        "publish retry scheduled"
                    );
                    // BK.3 will emit SwarmEvent::NodeEvent here with
                    // phase="retry_attempt" and the same payload
                    // shape as the tracing fields above.
                    tokio::time::sleep(wait).await;
                    attempt += 1;
                }
            }
        }
    }
}

/// Fail-closed classifier. Only `SupervisorError` payloads matching
/// `PublishStatus::RateLimited` are retried. All other variants —
/// including future `KernelAgentError` variants added without ADR
/// amendment — are non-retryable by construction.
fn classify_for_retry(err: &KernelAgentError) -> (bool, Option<u64>) {
    match err {
        KernelAgentError::SupervisorError(msg) => match classify_publish_error(msg) {
            PublishStatus::RateLimited { retry_after_secs } => (true, retry_after_secs),
            _ => (false, None),
        },
        _ => (false, None),
    }
}

/// Truncate `s` at `max_chars` Unicode chars. Appends `…` when
/// truncated. Bounds NodeEvent payload size on hot broadcast
/// channels (BK.3).
fn truncate_error(s: &str, max_chars: usize) -> String {
    let count = s.chars().count();
    if count <= max_chars {
        s.to_string()
    } else {
        let prefix: String = s.chars().take(max_chars).collect();
        format!("{prefix}…")
    }
}
