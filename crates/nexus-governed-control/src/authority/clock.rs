//! Time, injectable so that expiry is testable without sleeping.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// A monotonic clock for deadlines and a wall clock for evidence.
pub trait Clock: Send + Sync {
    /// Monotonic milliseconds since an arbitrary origin.
    fn monotonic_ms(&self) -> u64;
    /// Milliseconds since the Unix epoch.
    fn wall_ms(&self) -> u64;
}

/// The system clocks.
pub struct SystemClock {
    origin: Instant,
}

impl Default for SystemClock {
    fn default() -> Self {
        Self {
            origin: Instant::now(),
        }
    }
}

impl Clock for SystemClock {
    fn monotonic_ms(&self) -> u64 {
        u64::try_from(self.origin.elapsed().as_millis()).unwrap_or(u64::MAX)
    }

    fn wall_ms(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
            .unwrap_or(0)
    }
}

/// A clock that moves only when told to (tests and controls).
#[derive(Default)]
pub struct ManualClock {
    now: AtomicU64,
}

impl ManualClock {
    pub fn advance(&self, by: Duration) {
        let ms = u64::try_from(by.as_millis()).unwrap_or(u64::MAX);
        self.now.fetch_add(ms, Ordering::SeqCst);
    }
}

impl Clock for ManualClock {
    fn monotonic_ms(&self) -> u64 {
        self.now.load(Ordering::SeqCst)
    }

    fn wall_ms(&self) -> u64 {
        1_700_000_000_000 + self.now.load(Ordering::SeqCst)
    }
}
