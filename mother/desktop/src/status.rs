//! The broker's status, read on a backend thread (never on the GTK thread)
//! and handed to the window on a channel. The window shows the newest
//! reading and its age; a reading older than [`NOT_LIVE_AFTER`] is shown
//! NOT LIVE. The window never sets a state itself.

use nexus_governed_control::governed::GovernedControl;
use nexus_kernel::emergency_stop::StopAuthority;
use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
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
    rx: Receiver<Snapshot>,
    newest: RefCell<Option<Snapshot>>,
    closed: Arc<AtomicBool>,
}

impl StatusFeed {
    /// The newest snapshot received so far, if any.
    pub fn latest(&self) -> Option<Snapshot> {
        let mut newest = self.newest.borrow_mut();
        while let Ok(snapshot) = self.rx.try_recv() {
            *newest = Some(snapshot);
        }
        newest.clone()
    }

    /// Ask the reader to stop.
    pub fn close(&self) {
        self.closed.store(true, Ordering::SeqCst);
    }
}

impl Drop for StatusFeed {
    fn drop(&mut self) {
        self.close();
    }
}

/// Read the broker every `every` on a new thread. The reader ends when
/// the feed is closed or dropped.
pub fn spawn_feed(owner: Arc<GovernedControl>, every: Duration) -> StatusFeed {
    let (tx, rx) = mpsc::channel();
    let closed = Arc::new(AtomicBool::new(false));
    let stop = closed.clone();
    let _reader = std::thread::Builder::new()
        .name("nexus-status".into())
        .spawn(move || {
            let kernel = StopAuthority::global();
            let mut seq = 0;
            while !stop.load(Ordering::SeqCst) {
                seq += 1;
                let status = owner.status();
                let snapshot = Snapshot {
                    seq,
                    taken: Instant::now(),
                    broker_stopped: status.emergency_stopped,
                    kernel_stopped: kernel.is_stopped(),
                    stop_number: kernel.stop_number(),
                    policy_generation: status.policy_generation,
                    live_doors: status.live_doors,
                    doors_issued: status.doors_issued,
                };
                if tx.send(snapshot).is_err() {
                    break;
                }
                std::thread::sleep(every);
            }
        });
    StatusFeed {
        rx,
        newest: RefCell::new(None),
        closed,
    }
}
