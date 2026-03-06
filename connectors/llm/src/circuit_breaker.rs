//! Provider-level circuit breaker for LLM gateway health tracking.
//!
//! Three-state model: Closed (healthy) -> Open (failing) -> HalfOpen (testing recovery).

use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

#[derive(Debug)]
pub struct ProviderCircuitBreaker {
    state: CircuitState,
    failure_count: u32,
    failure_threshold: u32,
    reset_timeout: Duration,
    last_failure_at: Option<Instant>,
    half_open_attempted: bool,
}

impl ProviderCircuitBreaker {
    pub fn new(failure_threshold: u32, reset_timeout: Duration) -> Self {
        Self {
            state: CircuitState::Closed,
            failure_count: 0,
            failure_threshold,
            reset_timeout,
            last_failure_at: None,
            half_open_attempted: false,
        }
    }

    pub fn state(&self) -> CircuitState {
        self.state
    }

    /// Returns true if a request is allowed through the breaker.
    pub fn allow_request(&mut self) -> bool {
        match self.state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                if let Some(last) = self.last_failure_at {
                    if last.elapsed() >= self.reset_timeout {
                        self.state = CircuitState::HalfOpen;
                        self.half_open_attempted = false;
                        return self.allow_request();
                    }
                }
                false
            }
            CircuitState::HalfOpen => {
                if self.half_open_attempted {
                    false
                } else {
                    self.half_open_attempted = true;
                    true
                }
            }
        }
    }

    pub fn record_success(&mut self) {
        match self.state {
            CircuitState::HalfOpen => {
                self.state = CircuitState::Closed;
                self.failure_count = 0;
                self.last_failure_at = None;
                self.half_open_attempted = false;
            }
            CircuitState::Closed => {
                self.failure_count = 0;
            }
            CircuitState::Open => {}
        }
    }

    pub fn record_failure(&mut self) {
        self.failure_count += 1;
        self.last_failure_at = Some(Instant::now());

        match self.state {
            CircuitState::Closed => {
                if self.failure_count >= self.failure_threshold {
                    self.state = CircuitState::Open;
                }
            }
            CircuitState::HalfOpen => {
                self.state = CircuitState::Open;
                self.half_open_attempted = false;
            }
            CircuitState::Open => {}
        }
    }
}

impl Default for ProviderCircuitBreaker {
    fn default() -> Self {
        Self::new(5, Duration::from_secs(30))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn circuit_opens_after_n_failures() {
        let mut cb = ProviderCircuitBreaker::new(3, Duration::from_secs(30));
        assert_eq!(cb.state(), CircuitState::Closed);
        assert!(cb.allow_request());

        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Closed);
        assert!(cb.allow_request());

        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
        assert!(!cb.allow_request());
    }

    #[test]
    fn half_open_after_timeout() {
        let mut cb = ProviderCircuitBreaker::new(2, Duration::from_millis(10));
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);

        std::thread::sleep(Duration::from_millis(15));

        assert!(cb.allow_request());
        assert_eq!(cb.state(), CircuitState::HalfOpen);

        // Only one test request allowed in HalfOpen
        assert!(!cb.allow_request());
    }

    #[test]
    fn half_open_success_closes_circuit() {
        let mut cb = ProviderCircuitBreaker::new(2, Duration::from_millis(10));
        cb.record_failure();
        cb.record_failure();
        std::thread::sleep(Duration::from_millis(15));

        assert!(cb.allow_request());
        cb.record_success();
        assert_eq!(cb.state(), CircuitState::Closed);
        assert!(cb.allow_request());
    }

    #[test]
    fn half_open_failure_reopens_circuit() {
        let mut cb = ProviderCircuitBreaker::new(2, Duration::from_millis(10));
        cb.record_failure();
        cb.record_failure();
        std::thread::sleep(Duration::from_millis(15));

        assert!(cb.allow_request());
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
        assert!(!cb.allow_request());
    }

    #[test]
    fn success_resets_failure_count() {
        let mut cb = ProviderCircuitBreaker::new(3, Duration::from_secs(30));
        cb.record_failure();
        cb.record_failure();
        cb.record_success();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Closed);
    }
}
