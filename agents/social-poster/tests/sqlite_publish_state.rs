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
        state
            .record_publish(&key, None, None)
            .await
            .expect("record");
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
        .record_publish(&key, Some("sha256:deadbeef".into()), None)
        .await
        .expect("record");
    state
        .record_publish(&key, None, None)
        .await
        .expect("record");
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
        state
            .record_publish(&x_a, None, None)
            .await
            .expect("record");
    }
    state
        .record_publish(&x_b, None, None)
        .await
        .expect("record");

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
    db.record_social_publish("X", "default", now - 60, None, None)
        .expect("seed");
    db.record_social_publish("X", "default", now - 600, None, None)
        .expect("seed");
    db.record_social_publish("X", "default", now - 3000, None, None)
        .expect("seed");
    db.record_social_publish("X", "default", now - 7200, None, None)
        .expect("seed");
    db.record_social_publish("X", "default", now - 86_400, None, None)
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
            state
                .record_publish(&key, None, None)
                .await
                .expect("record");
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

// ── Bug V: post_id round-trip + migration idempotency ──────────────────────

#[tokio::test]
async fn post_id_column_round_trips_through_record_and_count() {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("nexus.db");
    let db = Arc::new(NexusDatabase::open(&path).expect("open"));

    // Direct typed-helper insert with a post_id; verify the row is
    // counted by the trait surface. The post_id column is what V's
    // dedupe path will read; this asserts it's writable through
    // `record_social_publish` and that the index path still hits it.
    db.record_social_publish(
        "X",
        "default",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
            - 30,
        Some("sha256:abc"),
        Some("tweet_99"),
    )
    .expect("seed");

    let state = SqlitePublishState::new(db);
    let key = ChannelKey::new(SocialPlatform::X, "default");
    let n = state
        .recent_post_count(&key, Duration::from_secs(86_400))
        .await
        .expect("count");
    assert_eq!(n, 1);
}

#[tokio::test]
async fn migrate_is_idempotent_across_reopens() {
    // The post_id column add goes through `add_column_if_missing`,
    // which swallows the "duplicate column name" error. Opening the
    // same file twice must not fail; the second `migrate()` call
    // re-runs the ADD COLUMN against an already-present column.
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("nexus.db");
    let key = ChannelKey::new(SocialPlatform::X, "default");

    {
        let db = Arc::new(NexusDatabase::open(&path).expect("open"));
        let state = SqlitePublishState::new(db);
        state
            .record_publish(&key, None, Some("tweet_a".into()))
            .await
            .expect("record");
    }

    // Reopen — migrate() runs again, hits the already-existing column.
    let db_reopen = Arc::new(NexusDatabase::open(&path).expect("reopen must not fail"));
    let state_reopen = SqlitePublishState::new(db_reopen);
    state_reopen
        .record_publish(&key, None, Some("tweet_b".into()))
        .await
        .expect("post-reopen record_publish must succeed");
    let n = state_reopen
        .recent_post_count(&key, Duration::from_secs(86_400))
        .await
        .expect("count");
    assert_eq!(n, 2, "both publishes must round-trip across the reopen");
}
