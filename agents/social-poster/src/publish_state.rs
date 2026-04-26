//! Per-channel post-count source (Bug W).
//!
//! `PublishStateHandle` is the surface Herald's swarm path uses to drive
//! the compliance gate's `recent_posts` argument. Two impls ship here:
//!
//! - `InMemoryPublishState` — `Mutex<HashMap<ChannelKey, Vec<i64>>>` of
//!   post epoch-seconds. Used by tests and any caller that wants
//!   process-local counters with no durability.
//! - `SqlitePublishState` — backed by `nexus-persistence`'s
//!   `social_publish_log` table. Production. Survives restart.
//!
//! `recent_post_count(channel, window)` returns the number of posts on
//! that channel within the trailing `window`. `record_publish` appends a
//! new row stamped with the current epoch second. Bug W reads;
//! Bug V (Phase 5) is the publish-trigger that calls `record_publish`
//! after a successful post.
//!
//! Window is passed as a `Duration` per call rather than baked into the
//! impl so the trait stays neutral to platform-specific limits. Bug AC
//! tracks making the value caller-configurable from manifest.toml.

use crate::channel::ChannelKey;
use async_trait::async_trait;
use nexus_content::generator::SocialPlatform;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug)]
pub enum PublishStateError {
    Storage(String),
    Internal(String),
}

impl std::fmt::Display for PublishStateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Storage(d) => write!(f, "publish state storage error: {d}"),
            Self::Internal(d) => write!(f, "publish state internal error: {d}"),
        }
    }
}

impl std::error::Error for PublishStateError {}

#[async_trait]
pub trait PublishStateHandle: Send + Sync {
    /// Number of posts on `channel` whose `published_at` is within
    /// `window` of now (epoch seconds, inclusive lower bound).
    async fn recent_post_count(
        &self,
        channel: &ChannelKey,
        window: Duration,
    ) -> Result<usize, PublishStateError>;

    /// Append a publish record. Stamps `published_at = now()`.
    /// `content_hash` is optional; V will populate it with a digest of
    /// the published payload for idempotency / dedupe purposes.
    async fn record_publish(
        &self,
        channel: &ChannelKey,
        content_hash: Option<String>,
    ) -> Result<(), PublishStateError>;
}

/// Now in epoch seconds. Helper so tests can shim the clock if/when we
/// need to (currently they don't).
fn now_epoch_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Map `SocialPlatform` enum to a stable string for storage. Variants
/// are uppercase to mirror the platform's wire identity ("X" not "x").
pub(crate) fn platform_label(p: SocialPlatform) -> &'static str {
    match p {
        SocialPlatform::X => "X",
        SocialPlatform::Instagram => "Instagram",
        SocialPlatform::Facebook => "Facebook",
    }
}

// ── In-memory impl ──────────────────────────────────────────────────────────

#[derive(Default)]
pub struct InMemoryPublishState {
    posts: Mutex<HashMap<ChannelKey, Vec<i64>>>,
}

impl InMemoryPublishState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Test-only: insert a post at an arbitrary epoch second. Production
    /// callers go through `record_publish`.
    pub fn insert_at(&self, channel: ChannelKey, published_at_secs: i64) {
        let mut guard = self.posts.lock().unwrap_or_else(|p| p.into_inner());
        guard.entry(channel).or_default().push(published_at_secs);
    }
}

#[async_trait]
impl PublishStateHandle for InMemoryPublishState {
    async fn recent_post_count(
        &self,
        channel: &ChannelKey,
        window: Duration,
    ) -> Result<usize, PublishStateError> {
        let now = now_epoch_secs();
        let cutoff = now.saturating_sub(window.as_secs() as i64);
        let guard = self.posts.lock().unwrap_or_else(|p| p.into_inner());
        let count = guard
            .get(channel)
            .map(|stamps| stamps.iter().filter(|&&t| t >= cutoff).count())
            .unwrap_or(0);
        Ok(count)
    }

    async fn record_publish(
        &self,
        channel: &ChannelKey,
        _content_hash: Option<String>,
    ) -> Result<(), PublishStateError> {
        // content_hash isn't materialized by the in-memory impl — the
        // SQLite impl persists it for V's idempotency path. The trait
        // contract is that it's accepted, not necessarily retained at
        // every layer.
        let mut guard = self.posts.lock().unwrap_or_else(|p| p.into_inner());
        guard
            .entry(channel.clone())
            .or_default()
            .push(now_epoch_secs());
        Ok(())
    }
}

// ── SQLite-backed impl ──────────────────────────────────────────────────────

pub struct SqlitePublishState {
    db: Arc<nexus_persistence::NexusDatabase>,
}

impl SqlitePublishState {
    pub fn new(db: Arc<nexus_persistence::NexusDatabase>) -> Self {
        Self { db }
    }
}

#[async_trait]
impl PublishStateHandle for SqlitePublishState {
    async fn recent_post_count(
        &self,
        channel: &ChannelKey,
        window: Duration,
    ) -> Result<usize, PublishStateError> {
        let now = now_epoch_secs();
        let cutoff = now.saturating_sub(window.as_secs() as i64);
        let platform = platform_label(channel.platform);
        self.db
            .count_social_publishes_in_window(platform, &channel.account_id, cutoff)
            .map(|c| c as usize)
            .map_err(|e| PublishStateError::Storage(format!("{e}")))
    }

    async fn record_publish(
        &self,
        channel: &ChannelKey,
        content_hash: Option<String>,
    ) -> Result<(), PublishStateError> {
        let platform = platform_label(channel.platform);
        let now = now_epoch_secs();
        self.db
            .record_social_publish(platform, &channel.account_id, now, content_hash.as_deref())
            .map_err(|e| PublishStateError::Storage(format!("{e}")))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn inmemory_empty_count_is_zero() {
        let state = InMemoryPublishState::new();
        let key = ChannelKey::default_account(SocialPlatform::X);
        let n = state
            .recent_post_count(&key, Duration::from_secs(86_400))
            .await
            .expect("ok");
        assert_eq!(n, 0);
    }

    #[tokio::test]
    async fn inmemory_insert_n_count_returns_n_within_window() {
        let state = InMemoryPublishState::new();
        let key = ChannelKey::default_account(SocialPlatform::X);
        for _ in 0..5 {
            state.record_publish(&key, None).await.expect("ok");
        }
        let n = state
            .recent_post_count(&key, Duration::from_secs(86_400))
            .await
            .expect("ok");
        assert_eq!(n, 5);
    }

    #[tokio::test]
    async fn inmemory_posts_older_than_window_not_counted() {
        let state = InMemoryPublishState::new();
        let key = ChannelKey::default_account(SocialPlatform::X);
        let now = now_epoch_secs();
        // 3 inside the 1-hour window, 2 outside.
        state.insert_at(key.clone(), now - 30); // inside
        state.insert_at(key.clone(), now - 600); // inside
        state.insert_at(key.clone(), now - 3000); // inside (under 3600)
        state.insert_at(key.clone(), now - 7200); // 2h ago — outside
        state.insert_at(key.clone(), now - 86_400); // 24h ago — outside
        let n = state
            .recent_post_count(&key, Duration::from_secs(3600))
            .await
            .expect("ok");
        assert_eq!(n, 3);
    }

    #[tokio::test]
    async fn inmemory_independent_counts_per_channel_key() {
        let state = InMemoryPublishState::new();
        let x_a = ChannelKey::new(SocialPlatform::X, "acct-a");
        let x_b = ChannelKey::new(SocialPlatform::X, "acct-b");
        let ig_a = ChannelKey::new(SocialPlatform::Instagram, "acct-a");
        for _ in 0..3 {
            state.record_publish(&x_a, None).await.expect("ok");
        }
        state.record_publish(&x_b, None).await.expect("ok");
        // ig_a never written.
        assert_eq!(
            state
                .recent_post_count(&x_a, Duration::from_secs(86_400))
                .await
                .expect("ok"),
            3
        );
        assert_eq!(
            state
                .recent_post_count(&x_b, Duration::from_secs(86_400))
                .await
                .expect("ok"),
            1
        );
        assert_eq!(
            state
                .recent_post_count(&ig_a, Duration::from_secs(86_400))
                .await
                .expect("ok"),
            0
        );
    }

    #[tokio::test]
    async fn inmemory_record_publish_accepts_content_hash() {
        let state = InMemoryPublishState::new();
        let key = ChannelKey::default_account(SocialPlatform::Facebook);
        // The in-memory impl doesn't retain the hash, but the call must
        // succeed (the trait contract is accept, not retain).
        state
            .record_publish(&key, Some("sha256:abc".into()))
            .await
            .expect("ok");
        let n = state
            .recent_post_count(&key, Duration::from_secs(86_400))
            .await
            .expect("ok");
        assert_eq!(n, 1);
    }
}
