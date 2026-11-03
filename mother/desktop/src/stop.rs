//! The Stop route (M2b design §6.4 and §11 A1, A3).
//!
//! **The ingress** runs on the GTK thread at each press and is memory only:
//! it advances the Stop epoch (`Desk::cancel_all`, which in M2b-A cancels
//! nothing else: there are no confirmations yet) and reserves a worker slot,
//! under the desk's one small mutex. No I/O, no broker call, no other lock.
//!
//! **The blocking part**, the broker's `emergency_stop()` (kernel and broker
//! memory first, then its evidence records, which may stall on the store),
//! runs on a new worker thread for that press: never a shared queue, so one
//! stalled Stop never holds back the next press's ingress. At most
//! [`MAX_STOP_WORKERS`] workers live at once; a press at the cap still does
//! its ingress and is counted as coalesced.
//!
//! **Quit** joins every worker within a bound. A timed-out join is reported
//! as such, never as a clean shutdown, and Quit then exits non-zero.

use nexus_governed_control::governed::GovernedControl;
use std::sync::Arc;
use std::time::Duration;

/// Live Stop workers allowed at once (A1).
pub const MAX_STOP_WORKERS: usize = 4;

/// The confirmation desk's Stop half. M2b-A has no confirmations, so the
/// desk holds only the epoch; M2b-B's desk plugs in here.
pub struct Desk {
    _private: (),
}

impl Desk {
    pub fn new() -> Arc<Self> {
        unimplemented!("M2b-A PR-2 code")
    }

    /// The current Stop epoch.
    pub fn epoch(&self) -> u64 {
        unimplemented!("M2b-A PR-2 code")
    }

    /// Stop's step 0: memory only. Returns the new epoch.
    pub fn cancel_all(&self) -> u64 {
        unimplemented!("M2b-A PR-2 code")
    }
}

/// What one press did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Press {
    /// The ingress happened, and a new worker runs `emergency_stop()`.
    Dispatched { epoch: u64, worker: u64 },
    /// The ingress happened; the cap was reached, so no new worker.
    Coalesced { epoch: u64 },
    /// The ingress happened; the OS refused a new thread.
    SpawnFailed { epoch: u64 },
}

impl Press {
    pub fn epoch(&self) -> u64 {
        match *self {
            Press::Dispatched { epoch, .. }
            | Press::Coalesced { epoch }
            | Press::SpawnFailed { epoch } => epoch,
        }
    }
}

/// The route's receipts, all in memory.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Receipts {
    pub epoch: u64,
    pub presses: u64,
    pub dispatched: u64,
    pub coalesced: u64,
    pub spawn_failures: u64,
    /// Workers started and not yet finished.
    pub live: usize,
    pub finished: u64,
    /// Workers whose `emergency_stop()` panicked.
    pub panicked: u64,
}

/// How a bounded join ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Joined {
    All { joined: usize },
    TimedOut { still_running: usize },
}

/// The Stop route: every Stop button and shortcut goes through it.
pub struct StopRoute {
    _private: (),
}

impl StopRoute {
    pub fn new(desk: Arc<Desk>, owner: Arc<GovernedControl>) -> Arc<Self> {
        let _ = (desk, owner);
        unimplemented!("M2b-A PR-2 code")
    }

    pub fn desk(&self) -> &Arc<Desk> {
        unimplemented!("M2b-A PR-2 code")
    }

    /// One press: the ingress, then a worker (or the coalescing receipt).
    pub fn press(&self) -> Press {
        unimplemented!("M2b-A PR-2 code")
    }

    pub fn receipts(&self) -> Receipts {
        unimplemented!("M2b-A PR-2 code")
    }

    /// Join every worker started so far, within `within`.
    pub fn join_within(&self, within: Duration) -> Joined {
        let _ = within;
        unimplemented!("M2b-A PR-2 code")
    }
}

/// How Quit ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QuitOutcome {
    /// Every Stop worker joined, and the broker shut down.
    Clean {
        joined: usize,
        runs_cancelled: usize,
    },
    /// Something was still running when the bound passed.
    Unclean {
        stop_workers_running: usize,
        shut_down: bool,
    },
}

impl QuitOutcome {
    pub fn is_clean(&self) -> bool {
        unimplemented!("M2b-A PR-2 code")
    }

    /// 0 only for a clean Quit.
    pub fn exit_code(&self) -> i32 {
        unimplemented!("M2b-A PR-2 code")
    }

    /// What the app says on Quit.
    pub fn report(&self) -> String {
        unimplemented!("M2b-A PR-2 code")
    }
}

/// Quit (design §6.6, A3): join the Stop workers, then shut the broker
/// down, everything within `within`.
pub fn quit(route: &StopRoute, owner: &Arc<GovernedControl>, within: Duration) -> QuitOutcome {
    let _ = (route, owner, within);
    unimplemented!("M2b-A PR-2 code")
}
