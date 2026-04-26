//! Bug W: SqlitePublishState integration tests.
//!
//! Cover the production-path persistence layer end-to-end: round-trip
//! `record_publish` → `recent_post_count`, window-boundary semantics,
//! per-channel isolation, and durability across reopen of the same
//! SQLite file (proves restart-safety for the compliance counter).

use nexus_content::generator::SocialPlatform;
use nexus_persistence::NexusDatabase;
use social_poster_agent::channel::ChannelKey;
use social_poster_agent::publish_state::{PublishStateHandle, SqlitePublishState};
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;

fn fresh_state() -> (SqlitePublishState, TempDir) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("nexus.db");
    let db = NexusDatabase::open(&path).expect("open");
    (SqlitePublishState::new(Arc::new(db)), dir)
}

#[tokio::test]
async fn round_trip_record_then_count() {
    let (state, _dir) = fresh_state();
    let key = ChannelKey::new(SocialPlatform::X, "default");
    for _ in 0..4 {
        state.record_publish(&key, None).await.expect("record");
    }
    let n = state
        .recent_post_count(&key, Duration::from_secs(86_400))
        .await
        .expect("count");
    assert_eq!(n, 4);
}

#[tokio::test]
async fn record_publish_persists_optional_content_hash() {
    let (state, _dir) = fresh_state();
    let key = ChannelKey::new(SocialPlatform::Facebook, "marketing");
    state
        .record_publish(&key, Some("sha256:deadbeef".into()))
        .await
        .expect("record");
    state.record_publish(&key, None).await.expect("record");
    let n = state
        .recent_post_count(&key, Duration::from_secs(86_400))
        .await
        .expect("count");
    assert_eq!(n, 2, "both rows count regardless of content_hash presence");
}

#[tokio::test]
async fn channel_keys_are_isolated_across_platform_and_account() {
    let (state, _dir) = fresh_state();
    let x_a = ChannelKey::new(SocialPlatform::X, "acct-a");
    let x_b = ChannelKey::new(SocialPlatform::X, "acct-b");
    let ig_a = ChannelKey::new(SocialPlatform::Instagram, "acct-a");

    for _ in 0..3 {
        state.record_publish(&x_a, None).await.expect("record");
    }
    state.record_publish(&x_b, None).await.expect("record");

    assert_eq!(
        state
            .recent_post_count(&x_a, Duration::from_secs(86_400))
            .await
            .expect("count"),
        3
    );
    assert_eq!(
        state
            .recent_post_count(&x_b, Duration::from_secs(86_400))
            .await
            .expect("count"),
        1
    );
    assert_eq!(
        state
            .recent_post_count(&ig_a, Duration::from_secs(86_400))
            .await
            .expect("count"),
        0,
        "(Instagram, acct-a) is its own composite key"
    );
}

#[tokio::test]
async fn window_excludes_records_older_than_cutoff() {
    // Insert via the typed helper at known epoch seconds, then read
    // through the trait surface with a tight window. Confirms the
    // SQL `published_at >= cutoff` boundary is correct.
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("nexus.db");
    let db = Arc::new(NexusDatabase::open(&path).expect("open"));

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    // 3 inside a 1-hour window, 2 outside.
    db.record_social_publish("X", "default", now - 60, None)
        .expect("seed");
    db.record_social_publish("X", "default", now - 600, None)
        .expect("seed");
    db.record_social_publish("X", "default", now - 3000, None)
        .expect("seed");
    db.record_social_publish("X", "default", now - 7200, None)
        .expect("seed");
    db.record_social_publish("X", "default", now - 86_400, None)
        .expect("seed");

    let state = SqlitePublishState::new(db);
    let key = ChannelKey::new(SocialPlatform::X, "default");
    let n = state
        .recent_post_count(&key, Duration::from_secs(3600))
        .await
        .expect("count");
    assert_eq!(n, 3);
}

#[tokio::test]
async fn count_survives_reopen_of_same_db_file() {
    // Bug W ships persistence — restart durability is the contract.
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("nexus.db");
    let key = ChannelKey::new(SocialPlatform::Instagram, "default");

    {
        let db = Arc::new(NexusDatabase::open(&path).expect("open"));
        let state = SqlitePublishState::new(db);
        for _ in 0..7 {
            state.record_publish(&key, None).await.expect("record");
        }
    } // drops db handle — WAL flush

    let db_again = Arc::new(NexusDatabase::open(&path).expect("reopen"));
    let state_again = SqlitePublishState::new(db_again);
    let n = state_again
        .recent_post_count(&key, Duration::from_secs(86_400))
        .await
        .expect("count");
    assert_eq!(n, 7, "rows must survive a process-style restart");
}
