//! SB-3E S7 (design 4.2, DA-S7R): the host reader's route seam, as S5.
//! Compiled only with `--cfg mother_route_seams`, which only the route lane's
//! wrapper sets; the shipped build has none. The writer `at` is crate-private
//! and called only at the three `host.read` arms of `terminal.rs`
//! `read_input`, on the reader thread after it has processed the value; the
//! harness may only `arm` and `drain_trace`. `at` returns nothing: it records
//! or pauses, and never returns into the reader with a value. It touches no
//! host resource itself: a pause's timeout is the harness's own sink.
#[cfg(not(target_os = "linux"))]
compile_error!("mother_route_seams: the route seams are Linux-only");

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

/// S7's marker (SB3E-R9): the first statement of the writer's real body.
const M: &str = "nexus-mother-route-seam-S7";
/// The bounded wait of every pause (design 3.5).
const PATIENCE: Duration = Duration::from_secs(10);
/// Every point a harness may arm.
pub(crate) const POINTS: [&str; 1] = ["host.read"];

static ARMED: AtomicBool = AtomicBool::new(false);
static STATE: Mutex<State> = Mutex::new(State {
    tick: 0,
    entries: Vec::new(),
    pending: Vec::new(),
});

struct State {
    tick: u64,
    entries: Vec<Entry>,
    pending: Vec<Pending>,
}
struct Pending {
    id: u64,
    point: &'static str,
    ack: SyncSender<Entry>,
    pause: (Receiver<()>, &'static str, Timeout),
}

/// Which arm of `read_input` processed the value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Branch {
    /// `Ok(n)`: `n` bytes went through `route_read`.
    Data,
    /// `Ok(0)`: end of input, after `receive(Eof)`.
    Zero,
    /// A read error, after press and fail.
    Err,
}

/// What the reader knew when it recorded.
#[derive(Clone, Copy)]
pub(crate) struct Read {
    n: usize,
    eot: bool,
    branch: Branch,
}
/// `Ok(n)`: the read's length and whether it held a Ctrl-D.
pub(crate) fn data(bytes: &[u8]) -> Read {
    let (n, eot) = (bytes.len(), bytes.contains(&4));
    Read {
        n,
        eot,
        branch: Branch::Data,
    }
}
/// `Ok(0)` or an error: nothing was read.
pub(crate) fn end(branch: Branch) -> Read {
    Read {
        n: 0,
        eot: false,
        branch,
    }
}

/// One trace entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub tick: u64,
    pub point: &'static str,
    pub n: usize,
    pub eot: bool,
    pub branch: Branch,
    /// The recording thread: the reader's own.
    pub thread: std::thread::ThreadId,
    /// When it was recorded (position evidence only).
    pub instant: Instant,
}

/// A pause's timeout: given the `TIMEOUT <cell> <point>` line, the harness
/// records it and ends the process; it cannot return (`!`).
pub type Timeout = fn(&str) -> !;

/// What an armed point does. Every arming turns recording on.
pub enum Action {
    /// Record only.
    Trace,
    /// At the first arrival: acknowledge, then block the reader until the
    /// [`Held`] is dropped; not released within 10 s, `timeout` ends it.
    Pause {
        cell: &'static str,
        timeout: Timeout,
    },
}

/// One arming. Dropping it releases its pause, or disarms it if it has not
/// fired; that never panics.
pub struct Held {
    pub acknowledged: Receiver<Entry>,
    _release: Release,
}
struct Release(u64, SyncSender<()>);
impl Drop for Release {
    fn drop(&mut self) {
        state().pending.retain(|p| p.id != self.0);
        let _ = self.1.try_send(());
    }
}

/// Arm `point` (one of [`POINTS`]) with `action`.
pub fn arm(point: &'static str, action: Action) -> Held {
    assert!(POINTS.contains(&point), "not a host route point: {point}");
    let (ack, acknowledged) = sync_channel(1);
    let (release, released) = sync_channel(1);
    let mut s = state();
    s.tick += 1;
    let id = s.tick;
    if let Action::Pause { cell, timeout } = action {
        let pause = (released, cell, timeout);
        s.pending.push(Pending {
            id,
            point,
            ack,
            pause,
        });
    }
    ARMED.store(true, Ordering::SeqCst);
    Held {
        acknowledged,
        _release: Release(id, release),
    }
}

/// Every entry recorded so far, in tick order; the trace is emptied.
pub fn drain_trace() -> Vec<Entry> {
    std::mem::take(&mut state().entries)
}

/// A reader point. Unarmed, this is the marker and one relaxed load.
pub(crate) fn at(point: &'static str, read: Read) {
    std::hint::black_box(M.as_bytes());
    if !ARMED.load(Ordering::Relaxed) {
        return;
    }
    fire(point, read);
}

fn state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(PoisonError::into_inner)
}

fn fire(point: &'static str, read: Read) {
    let (entry, pending) = {
        let mut s = state();
        s.tick += 1;
        let entry = Entry {
            tick: s.tick,
            point,
            n: read.n,
            eot: read.eot,
            branch: read.branch,
            thread: std::thread::current().id(),
            instant: Instant::now(),
        };
        s.entries.push(entry.clone());
        let found = s.pending.iter().position(|p| p.point == point);
        (entry, found.map(|i| s.pending.remove(i)))
    };
    let Some(pending) = pending else { return };
    let acknowledged = pending.ack.try_send(entry).is_ok();
    let (released, cell, timeout) = pending.pause;
    if !acknowledged || released.recv_timeout(PATIENCE).is_err() {
        timeout(&format!("TIMEOUT {cell} {point}"));
    }
}
