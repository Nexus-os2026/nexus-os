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
}

impl IdempotencyManager {
    pub fn new(ttl_seconds: u64) -> Self {
        Self {
            cache: HashMap::new(),
            ttl_ms: ttl_seconds.saturating_mul(1_000),
            clock_ms: Arc::new(current_time_millis),
        }
    }

    pub fn with_clock(ttl_seconds: u64, clock_ms: Arc<dyn Fn() -> u64 + Send + Sync>) -> Self {
        Self {
            cache: HashMap::new(),
            ttl_ms: ttl_seconds.saturating_mul(1_000),
            clock_ms,
        }
    }

    pub fn generate_request_id() -> String {
        Uuid::new_v4().to_string()
    }

    pub fn check_duplicate(&mut self, request_id: &str) -> Option<String> {
        self.evict_expired();
        self.cache
            .get(request_id)
            .map(|entry| entry.response.clone())
    }

    pub fn record_completion(&mut self, request_id: &str, response: String) {
        self.evict_expired();
        let expires_at_ms = (self.clock_ms)().saturating_add(self.ttl_ms);
        self.cache.insert(
            request_id.to_string(),
            CachedResponse {
                response,
                expires_at_ms,
            },
        );
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
}
