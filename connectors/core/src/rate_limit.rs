use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RateLimitDecision {
    Allowed,
    RateLimited { retry_after_ms: u64 },
}

#[derive(Debug, Clone)]
struct RateLimitConfig {
    max_requests: usize,
    window_seconds: u64,
}

#[derive(Debug, Clone)]
struct ConnectorWindow {
    config: RateLimitConfig,
    request_times_ms: VecDeque<u64>,
}

#[derive(Clone)]
pub struct RateLimiter {
    state: Arc<Mutex<HashMap<String, ConnectorWindow>>>,
    clock_ms: Arc<dyn Fn() -> u64 + Send + Sync>,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(HashMap::new())),
            clock_ms: Arc::new(current_time_millis),
        }
    }

    pub fn with_clock(clock_ms: Arc<dyn Fn() -> u64 + Send + Sync>) -> Self {
        Self {
            state: Arc::new(Mutex::new(HashMap::new())),
            clock_ms,
        }
    }

    pub fn configure(&self, connector_id: &str, max_requests: usize, window_seconds: u64) {
        let mut guard = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };

        guard.insert(
            connector_id.to_string(),
            ConnectorWindow {
                config: RateLimitConfig {
                    max_requests,
                    window_seconds,
                },
                request_times_ms: VecDeque::new(),
            },
        );
    }

    pub fn check(&self, connector_id: &str) -> RateLimitDecision {
        let now_ms = (self.clock_ms)();
        let mut guard = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };

        let Some(window) = guard.get_mut(connector_id) else {
            return RateLimitDecision::Allowed;
        };

        let window_ms = window.config.window_seconds.saturating_mul(1_000);
        while let Some(oldest) = window.request_times_ms.front().copied() {
            if now_ms.saturating_sub(oldest) >= window_ms {
                let _ = window.request_times_ms.pop_front();
            } else {
                break;
            }
        }

        if window.request_times_ms.len() < window.config.max_requests {
            window.request_times_ms.push_back(now_ms);
            return RateLimitDecision::Allowed;
        }

        let retry_after_ms = match window.request_times_ms.front().copied() {
            Some(oldest) => {
                let elapsed = now_ms.saturating_sub(oldest);
                window_ms.saturating_sub(elapsed)
            }
            None => 0,
        };

        RateLimitDecision::RateLimited { retry_after_ms }
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
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
    use super::{RateLimitDecision, RateLimiter};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;

    #[test]
    fn test_rate_limit_enforcement() {
        let now = Arc::new(AtomicU64::new(0));
        let clock_now = Arc::clone(&now);
        let limiter = RateLimiter::with_clock(Arc::new(move || clock_now.load(Ordering::SeqCst)));
        limiter.configure("http", 5, 60);

        for _ in 0..5 {
            let decision = limiter.check("http");
            assert_eq!(decision, RateLimitDecision::Allowed);
        }

        let sixth = limiter.check("http");
        match sixth {
            RateLimitDecision::Allowed => panic!("expected sixth request to be rate limited"),
            RateLimitDecision::RateLimited { retry_after_ms } => {
                assert!(retry_after_ms <= 60_000);
            }
        }

        now.store(60_001, Ordering::SeqCst);
        let after_window = limiter.check("http");
        assert_eq!(after_window, RateLimitDecision::Allowed);
    }
}
