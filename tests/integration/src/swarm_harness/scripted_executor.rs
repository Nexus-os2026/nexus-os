//! Bug BL.1: harness-side scripted PublishExecutor.
//!
//! Mirrors `agents/social-poster/src/swarm_entry.rs`'s
//! ScriptableExecutor (cfg(test) + private to that mod).
//! That symbol is unreachable from `tests/integration/`,
//! so this file re-creates the same shape for harness
//! scenarios (BL.3).
//!
//! Each `publish_with_request_id` call:
//!   - Records the `request_id` (so scenarios can assert
//!     id reuse across retries).
//!   - Pops the next `ScriptedOutcome` from the queue.
//!   - Returns the corresponding Result.
//!
//! The default `publish` impl from the trait generates a
//! fresh UUID per call when invoked directly. The retry
//! decorator (BK.2/BK.3) calls `publish_with_request_id`
//! with a stable id captured at decorator entry, so the
//! recorded ids are stable across retries when the
//! decorator wraps this executor.

use std::collections::VecDeque;
use std::sync::Arc;

use async_trait::async_trait;
use nexus_connectors_web::twitter::TweetResult;
use nexus_kernel::errors::AgentError as KernelAgentError;
use social_poster_agent::swarm_entry::PublishExecutor;
use uuid::Uuid;

/// Per-attempt scripted outcome. Mirrors the variant set
/// from agents/social-poster/src/swarm_entry.rs's
/// StubOutcome (cfg(test)) so the harness exercises the
/// same error classes the BK.2 unit tests cover.
#[derive(Debug, Clone)]
pub enum ScriptedOutcome {
    /// Successful publish; carries the tweet id.
    Ok(String),
    /// Maps to KernelAgentError::SupervisorError(msg).
    /// The retry decorator's classifier parses these via
    /// `classify_publish_error` (raised to pub(crate) in
    /// BK.2). RateLimited messages of the form
    /// "social.x rate limited, retry after N ms" are
    /// retryable; "Failed", "AuthFailure" are not.
    SupervisorErr(String),
    /// Maps to KernelAgentError::CapabilityDenied(cap).
    /// Non-retryable per the BK fail-closed classifier.
    Capability(String),
    /// Maps to KernelAgentError::FuelExhausted.
    /// Non-retryable.
    Fuel,
}

pub struct ScriptedPublishExecutor {
    creds_present: bool,
    outcomes: tokio::sync::Mutex<VecDeque<ScriptedOutcome>>,
    request_ids_seen: tokio::sync::Mutex<Vec<Uuid>>,
}

impl ScriptedPublishExecutor {
    pub fn new(creds_present: bool, outcomes: Vec<ScriptedOutcome>) -> Arc<Self> {
        Arc::new(Self {
            creds_present,
            outcomes: tokio::sync::Mutex::new(outcomes.into()),
            request_ids_seen: tokio::sync::Mutex::new(Vec::new()),
        })
    }

    /// Snapshot the recorded request_ids in attempt order.
    /// Scenarios use this to assert that the BK retry
    /// decorator reuses the same id across retries.
    pub async fn request_ids_seen(&self) -> Vec<Uuid> {
        self.request_ids_seen.lock().await.clone()
    }

    /// Number of remaining (unconsumed) scripted outcomes.
    pub async fn remaining(&self) -> usize {
        self.outcomes.lock().await.len()
    }
}

#[async_trait]
impl PublishExecutor for ScriptedPublishExecutor {
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
                .unwrap_or(ScriptedOutcome::SupervisorErr(
                    "scripted outcome exhausted".into(),
                ));
        match outcome {
            ScriptedOutcome::Ok(tweet_id) => Ok(TweetResult {
                tweet_id,
                posted_at: 0,
            }),
            ScriptedOutcome::SupervisorErr(msg) => Err(KernelAgentError::SupervisorError(msg)),
            ScriptedOutcome::Capability(cap) => Err(KernelAgentError::CapabilityDenied(cap)),
            ScriptedOutcome::Fuel => Err(KernelAgentError::FuelExhausted),
        }
    }
}
