//! The broker's status, read on a backend thread (never on the GTK thread)
//! and handed to the window on a channel.

use nexus_governed_control::governed::GovernedControl;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// A snapshot older than this is shown NOT LIVE.
pub const NOT_LIVE_AFTER: Duration = Duration::from_secs(3);

/// One reading of the broker and the kernel Stop.
#[derive(Clone, Debug)]
pub struct Snapshot {
    pub seq: u64,
    pub taken: Instant,
    /// `ControlStatus.emergency_stopped`.
    pub broker_stopped: bool,
    /// `StopAuthority::global().is_stopped()`.
    pub kernel_stopped: bool,
    pub stop_number: u64,
    pub policy_generation: u64,
    pub live_doors: usize,
    pub doors_issued: u64,
}

/// The reader thread's output.
pub struct StatusFeed {
    _private: (),
}

impl StatusFeed {
    /// The newest snapshot received so far, if any.
    pub fn latest(&self) -> Option<Snapshot> {
        unimplemented!("M2b-A PR-2 code")
    }

    /// Ask the reader to stop.
    pub fn close(&self) {
        unimplemented!("M2b-A PR-2 code")
    }
}

/// Read the broker every `every` on a new thread.
pub fn spawn_feed(owner: Arc<GovernedControl>, every: Duration) -> StatusFeed {
    let _ = (owner, every);
    unimplemented!("M2b-A PR-2 code")
}
