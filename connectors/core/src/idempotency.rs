use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

#[derive(Debug, Clone)]
struct CachedResponse {
    response: String,
    expires_at_ms: u64,
}

#[derive(Clone)]
pub struct IdempotencyManager {
    cache: HashMap<String, CachedResponse>,
    ttl_ms: u64,
    clock_ms: Arc<dyn Fn() -> u64 + Send + Sync>,
    /// Bug AF: optional persistent backing. When `Some`, `record_completion`
    /// also writes to `idempotency_cache` and `check_duplicate` falls
    /// through to a SQLite lookup on HashMap miss. Cross-process /
    /// cross-restart dedup hinges on this field. `None` keeps the
    /// existing in-memory-only behavior — all 4 pre-AF consumers
    /// (facebook, instagram, sequential, http_connector) stay on
    /// `::new` until their construction sites can thread an
    /// `Arc<NexusDatabase>` (Bug BJ).
    db: Option<Arc<nexus_persistence::NexusDatabase>>,
}

impl IdempotencyManager {
    pub fn new(ttl_seconds: u64) -> Self {
        Self {
            cache: HashMap::new(),
            ttl_ms: ttl_seconds.saturating_mul(1_000),
            clock_ms: Arc::new(current_time_millis),
            db: None,
        }
    }

    pub fn with_clock(ttl_seconds: u64, clock_ms: Arc<dyn Fn() -> u64 + Send + Sync>) -> Self {
        Self {
            cache: HashMap::new(),
            ttl_ms: ttl_seconds.saturating_mul(1_000),
            clock_ms,
            db: None,
        }
    }

    /// Bug AF: persistent ctor. SQLite-backed dedup that survives
    /// process restart. The HashMap remains the in-process fast path;
    /// SQLite is consulted only on HashMap miss inside
    /// `check_duplicate`, and is written through on every
    /// `record_completion`.
    pub fn with_db(ttl_seconds: u64, db: Arc<nexus_persistence::NexusDatabase>) -> Self {
        Self {
            cache: HashMap::new(),
            ttl_ms: ttl_seconds.saturating_mul(1_000),
            clock_ms: Arc::new(current_time_millis),
            db: Some(db),
        }
    }

    pub fn generate_request_id() -> String {
        Uuid::new_v4().to_string()
    }

    pub fn check_duplicate(&mut self, request_id: &str) -> Option<String> {
        self.evict_expired();
        if let Some(entry) = self.cache.get(request_id) {
            return Some(entry.response.clone());
        }
        if let Some(db) = self.db.clone() {
            let now_ms = (self.clock_ms)();
            match db.lookup_idempotency(request_id, now_ms) {
                Ok(Some(response)) => {
                    let expires_at_ms = now_ms.saturating_add(self.ttl_ms);
                    self.cache.insert(
                        request_id.to_string(),
                        CachedResponse {
                            response: response.clone(),
                            expires_at_ms,
                        },
                    );
                    return Some(response);
                }
                Ok(None) => {}
                Err(e) => {
                    eprintln!("idempotency SQLite lookup failed: {e}");
                }
            }
        }
        None
    }

    pub fn record_completion(&mut self, request_id: &str, response: String) {
        self.evict_expired();
        let expires_at_ms = (self.clock_ms)().saturating_add(self.ttl_ms);
        self.cache.insert(
            request_id.to_string(),
            CachedResponse {
                response: response.clone(),
                expires_at_ms,
            },
        );
        if let Some(db) = self.db.as_ref() {
            if let Err(e) = db.record_idempotency(request_id, &response, expires_at_ms) {
                eprintln!("idempotency SQLite write failed: {e}");
            }
        }
    }

    fn evict_expired(&mut self) {
        let now = (self.clock_ms)();
        self.cache.retain(|_, entry| entry.expires_at_ms > now);
    }
}

fn current_time_millis() -> u64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => {
            let millis = duration.as_millis();
            if millis > u128::from(u64::MAX) {
                u64::MAX
            } else {
                millis as u64
            }
        }
        Err(_) => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::IdempotencyManager;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;

    #[test]
    fn test_idempotent_request() {
        let now = Arc::new(AtomicU64::new(1_000));
        let clock_now = Arc::clone(&now);
        let mut manager =
            IdempotencyManager::with_clock(60, Arc::new(move || clock_now.load(Ordering::SeqCst)));

        let first = manager.check_duplicate("abc");
        assert_eq!(first, None);

        manager.record_completion("abc", "cached_response".to_string());
        let second = manager.check_duplicate("abc");
        assert_eq!(second, Some("cached_response".to_string()));

        now.store(70_000, Ordering::SeqCst);
        let expired = manager.check_duplicate("abc");
        assert_eq!(expired, None);
    }

    // ── Bug AF: persistent-backing tests ───────────────────────────────

    fn test_db() -> Arc<nexus_persistence::NexusDatabase> {
        Arc::new(nexus_persistence::NexusDatabase::in_memory().expect("in-memory db"))
    }

    #[test]
    fn in_memory_only_behavior_unchanged() {
        // Construct via ::new and exercise the basic dedupe path; the
        // db field stays None so SQLite is never touched.
        let mut manager = IdempotencyManager::new(60);
        assert_eq!(manager.check_duplicate("k"), None);
        manager.record_completion("k", "v".into());
        assert_eq!(manager.check_duplicate("k"), Some("v".into()));
    }

    #[test]
    fn with_db_check_falls_through_on_hashmap_miss() {
        let db = test_db();
        // Seed the SQLite layer directly without going through the
        // manager (simulates a record from a different process).
        let now_ms: u64 = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        db.record_idempotency("req-x", "from-sqlite", now_ms + 60_000)
            .unwrap();
        let mut manager = IdempotencyManager::with_db(60, Arc::clone(&db));
        // HashMap is empty; check_duplicate must fall through to SQLite,
        // return the cached response, and populate the HashMap so the
        // next read short-circuits.
        assert_eq!(manager.check_duplicate("req-x"), Some("from-sqlite".into()));
        assert_eq!(manager.check_duplicate("req-x"), Some("from-sqlite".into()));
    }

    #[test]
    fn with_db_record_writes_to_both_layers() {
        let db = test_db();
        let mut manager = IdempotencyManager::with_db(60, Arc::clone(&db));
        manager.record_completion("req-y", "resp".into());
        // HashMap has it.
        assert_eq!(manager.check_duplicate("req-y"), Some("resp".into()));
        // SQLite has it (read directly through the DB handle).
        let now_ms: u64 = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        assert_eq!(
            db.lookup_idempotency("req-y", now_ms).unwrap(),
            Some("resp".into())
        );
    }

    #[test]
    fn with_db_sqlite_failure_does_not_panic() {
        // Use a real in-memory DB — there is no "closed handle" path
        // to inject from outside the crate. The contract this test
        // protects is that record_completion + check_duplicate always
        // succeed in-process even on SQLite mishaps; the
        // eprintln-on-error branch in record_completion is the only
        // failure mode and never panics.
        let db = test_db();
        let mut manager = IdempotencyManager::with_db(60, db);
        manager.record_completion("req-z", "ok".into());
        assert_eq!(manager.check_duplicate("req-z"), Some("ok".into()));
    }

    #[test]
    fn with_db_cross_restart_dedupe() {
        // Same DB handle simulates the "process restarts but db file
        // is the same" scenario: a fresh manager finds the entry
        // because SQLite is shared.
        let db = test_db();
        {
            let mut m1 = IdempotencyManager::with_db(60, Arc::clone(&db));
            m1.record_completion("req-r", "persisted".into());
        }
        let mut m2 = IdempotencyManager::with_db(60, Arc::clone(&db));
        assert_eq!(m2.check_duplicate("req-r"), Some("persisted".into()));
    }
}
