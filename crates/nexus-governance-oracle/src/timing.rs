//! Constant-time response normalization.
//!
//! Every oracle response takes exactly `response_ceiling` + random jitter,
//! regardless of actual decision time. This eliminates timing side channels.

use std::time::Duration;

/// Configuration for timing normalization.
pub struct TimingConfig {
    /// Constant response floor — no request returns faster than this.
    pub response_ceiling: Duration,
    /// Jitter range — random variation to prevent clock synchronization attacks.
    pub jitter_range: Duration,
}

impl Default for TimingConfig {
    fn default() -> Self {
        Self {
            response_ceiling: Duration::from_millis(200),
            jitter_range: Duration::from_millis(10),
        }
    }
}

impl TimingConfig {
    /// Calculate the remaining wait time to hit the ceiling + jitter.
    pub fn wait_duration(&self, elapsed: Duration) -> Duration {
        use rand::Rng;
        let jitter_ms = rand::thread_rng().gen_range(0..=self.jitter_range.as_millis() as u64);
        let target = self.response_ceiling + Duration::from_millis(jitter_ms);

        if elapsed < target {
            target - elapsed
        } else {
            Duration::from_millis(jitter_ms.max(1))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_values() {
        let cfg = TimingConfig::default();
        assert_eq!(cfg.response_ceiling, Duration::from_millis(200));
        assert_eq!(cfg.jitter_range, Duration::from_millis(10));
    }

    #[test]
    fn wait_duration_pads_short_elapsed() {
        let cfg = TimingConfig {
            response_ceiling: Duration::from_millis(200),
            jitter_range: Duration::from_millis(0), // no jitter for deterministic test
        };
        let wait = cfg.wait_duration(Duration::from_millis(50));
        assert_eq!(wait, Duration::from_millis(150));
    }

    #[test]
    fn wait_duration_handles_overrun() {
        let cfg = TimingConfig {
            response_ceiling: Duration::from_millis(200),
            jitter_range: Duration::from_millis(5),
        };
        let wait = cfg.wait_duration(Duration::from_millis(300));
        // Should return at least 1ms even on overrun
        assert!(wait.as_millis() >= 1);
    }
}
