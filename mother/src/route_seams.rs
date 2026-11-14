//! SB-3E S5 (design 4.2, 4.4; AM-2 B.1): Mother's route seams, as S1.
//! Compiled only with `--cfg mother_route_seams`, which only the route lane's
//! wrapper sets; the shipped build has none. The writer `at` is
//! crate-private and called only at the audited `mother.*` rows of design
//! 4.4; the harness may only `arm` and `drain_trace`. `at` returns nothing:
//! it records, may pause, or begins an unwind, and never returns into the
//! route. Unlike S1 it touches no host resource itself (Mother's
//! `scan_no_host_access_outside_the_adapter`): a pause's timeout is the
//! harness's own never-returning sink.
#[cfg(not(target_os = "linux"))]
compile_error!("mother_route_seams: the route seams are Linux-only");

use crate::lane::InvocationId;
use nexus_governed_control::authority::ids::RunId;
use nexus_governed_control::host_session::{SessionContext, SessionIdentity};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

/// S5's marker (SB3E-R9): the first statement of the writer's real body.
const M: &str = "nexus-mother-route-seam-S5";
/// The bounded wait of every pause (design 3.5).
const PATIENCE: Duration = Duration::from_secs(10);
/// Every point a harness may arm: the `mother.*` rows of design 4.4.
pub(crate) const POINTS: [&str; 10] = [
    "mother.charge.entry",
    "mother.charge",
    "mother.charge.ok",
    "mother.settle",
    "mother.classify",
    "mother.recheck",
    "mother.lane.prelock",
    "mother.admit",
    "mother.admit.ok",
    "mother.lane.returned",
];

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
    pause: Option<(Receiver<()>, &'static str, Timeout)>,
}

/// The identity a site knows (AM-2 A.3); unknown fields stay `None`.
#[derive(Clone, Copy, Default)]
pub(crate) struct Ctx {
    session: Option<SessionIdentity>,
    request: Option<u64>,
    run: Option<RunId>,
    invocation: Option<InvocationId>,
}
impl Ctx {
    /// The session and request of the broker's request context, if any.
    pub(crate) fn of(context: Option<&SessionContext>) -> Self {
        Self {
            session: context.map(SessionContext::session),
            request: context.map(SessionContext::request),
            ..Self::default()
        }
    }
    pub(crate) fn invocation(self, invocation: InvocationId) -> Self {
        let invocation = Some(invocation);
        Self { invocation, ..self }
    }
    /// `mother.classify` only: it carries the run, and no invocation.
    pub(crate) fn run(self, run: RunId) -> Self {
        let run = Some(run);
        Self { run, ..self }
    }
}

/// One trace entry (AM-2 A.3, B.1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub tick: u64,
    pub point: &'static str,
    pub session: Option<SessionIdentity>,
    pub request: Option<u64>,
    /// Only `mother.classify` carries a run.
    pub run: Option<RunId>,
    pub invocation: Option<InvocationId>,
    /// The recording thread (AM-2 B.1).
    pub thread: std::thread::ThreadId,
    /// When it was recorded (AM-2 B.1: position evidence only).
    pub instant: Instant,
}

/// A pause's timeout: given the `TIMEOUT <cell> <point>` line, the harness
/// records it and ends the process; it cannot return (`!`).
pub type Timeout = fn(&str) -> !;

/// What an armed point does. Every arming turns recording on for all points.
pub enum Action {
    /// Record only.
    Trace,
    /// At the first arrival: acknowledge, then block until the [`Held`] is
    /// dropped. A pause not released within 10 s, or whose harness is gone,
    /// is an irrevocable harness failure: `TIMEOUT <cell> <point>` is handed
    /// to `timeout`, which writes it to the cell's result and aborts (design
    /// 3.3); it never returns into the route.
    Pause {
        cell: &'static str,
        timeout: Timeout,
    },
    /// At the first arrival: acknowledge, then begin an unwind.
    Panic,
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
    assert!(POINTS.contains(&point), "not a Mother route point: {point}");
    let (ack, acknowledged) = sync_channel(1);
    let (release, released) = sync_channel(1);
    let mut s = state();
    s.tick += 1;
    let id = s.tick;
    let pause = match action {
        Action::Trace => None,
        Action::Pause { cell, timeout } => Some(Some((released, cell, timeout))),
        Action::Panic => Some(None),
    };
    if let Some(pause) = pause {
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

/// A route point. Unarmed, this is the marker and one relaxed load.
pub(crate) fn at(point: &'static str, ctx: Ctx) {
    std::hint::black_box(M.as_bytes());
    if !ARMED.load(Ordering::Relaxed) {
        return;
    }
    fire(point, ctx);
}

fn state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(PoisonError::into_inner)
}

fn fire(point: &'static str, ctx: Ctx) {
    let (entry, pending) = {
        let mut s = state();
        s.tick += 1;
        let entry = Entry {
            tick: s.tick,
            point,
            session: ctx.session,
            request: ctx.request,
            run: ctx.run,
            invocation: ctx.invocation,
            thread: std::thread::current().id(),
            instant: Instant::now(),
        };
        s.entries.push(entry.clone());
        let found = s.pending.iter().position(|p| p.point == point);
        (entry, found.map(|i| s.pending.remove(i)))
    };
    let Some(pending) = pending else { return };
    let acknowledged = pending.ack.try_send(entry).is_ok();
    match pending.pause {
        None => panic!("route seam: armed panic at {point}"),
        Some((released, cell, timeout)) => {
            if !acknowledged || released.recv_timeout(PATIENCE).is_err() {
                timeout(&format!("TIMEOUT {cell} {point}"));
            }
        }
    }
}
