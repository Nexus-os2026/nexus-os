//! SB-3E S1 (design 4.2): the broker's route seams. Compiled only with
//! `--cfg mother_route_seams`, which only the route lane's wrapper sets; the
//! shipped build has none. The writers `at` and `note` are crate-private and
//! called only at the audited sites of design 4.4 and the two store sites;
//! the harness may only `arm` and `drain_trace`. `at` returns nothing: it
//! records, may pause, or begins an unwind, and never returns into the route.
#[cfg(not(target_os = "linux"))]
compile_error!("mother_route_seams: the route seams are Linux-only");

use crate::authority::ids::RunId;
use crate::host_session::SessionIdentity;
use nexus_kernel::inference::RequestKind;
use std::fs::File;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

/// S1's marker (SB3E-R9): the first statement of each writer's real body.
const M: &str = "nexus-mother-route-seam-S1";
/// The bounded wait of every pause (design 3.5).
const PATIENCE: Duration = Duration::from_secs(10);
/// Every point a harness may arm: the broker rows of design 4.4 and S1's
/// two store points.
pub(crate) const POINTS: [&str; 35] = [
    "reg.reserve",
    "model.reserved",
    "model.record.before",
    "model.install.before",
    "model.install.after",
    "reg.ready",
    "identity",
    "bind.entry",
    "model.bind.checked",
    "bind.done",
    "attempt.slot",
    "model.attempt.reserved",
    "model.attempt.granted",
    "model.attempt.checked",
    "model.gate.before",
    "model.gate.checked",
    "call.gate",
    "model.call.sampled",
    "model.call.observed",
    "model.call.admitted",
    "door.cancel_run",
    "door.halt",
    "ingress.emergency",
    "ingress.accept",
    "model.attempt.record.before",
    "model.settle.before",
    "complete",
    "close.obligations",
    "producer.join",
    "close.seal",
    "close.mother_end",
    "close.primary",
    "close.delivery",
    "store.commit",
    "store.commit.result",
];
/// The per-request gate entries; each one advances `request_ordinal`.
const GATES: [&str; 2] = ["model.gate.before", "call.gate"];

static ARMED: AtomicBool = AtomicBool::new(false);
static FAILED: AtomicBool = AtomicBool::new(false);
static STATE: Mutex<State> = Mutex::new(State {
    tick: 0,
    entries: Vec::new(),
    pending: Vec::new(),
    ordinals: Vec::new(),
});

type Key = (
    Option<SessionIdentity>,
    Option<u64>,
    Option<RunId>,
    Option<u32>,
);
struct State {
    tick: u64,
    entries: Vec<Entry>,
    pending: Vec<Pending>,
    ordinals: Vec<(Key, u64)>,
}
struct Pending {
    id: u64,
    point: &'static str,
    ack: SyncSender<Entry>,
    pause: Option<(Receiver<()>, &'static str, Arc<File>)>,
}

/// The identity a site knows (design 2.3); unknown fields stay `None`.
#[derive(Clone, Copy, Default)]
pub(crate) struct Ctx<'a> {
    session: Option<SessionIdentity>,
    request: Option<u64>,
    run: Option<RunId>,
    attempt: Option<u32>,
    kind: Option<&'static str>,
    phase: Option<&'a str>,
    commitment: Option<&'a str>,
}
impl<'a> Ctx<'a> {
    pub(crate) fn session(session: SessionIdentity) -> Self {
        let session = Some(session);
        Self {
            session,
            ..Self::default()
        }
    }
    pub(crate) fn request(self, request: u64) -> Self {
        let request = Some(request);
        Self { request, ..self }
    }
    pub(crate) fn run(self, run: RunId) -> Self {
        Self {
            run: Some(run),
            ..self
        }
    }
    pub(crate) fn attempt(self, attempt: u32) -> Self {
        let attempt = Some(attempt);
        Self { attempt, ..self }
    }
    pub(crate) fn kind(self, kind: RequestKind) -> Self {
        let chat = kind == RequestKind::Chat;
        let kind = Some(if chat { "chat" } else { "metadata" });
        Self { kind, ..self }
    }
    /// A store append: its kind (`ordinary` or `owed_flush`) and record.
    pub(crate) fn store(kind: &'static str, phase: &'a str, commitment: Option<&'a str>) -> Self {
        let (kind, phase) = (Some(kind), Some(phase));
        Self {
            kind,
            phase,
            commitment,
            ..Self::default()
        }
    }
}

/// One trace entry (design 2.3). `note` marks a refusing branch's reason.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub tick: u64,
    pub point: &'static str,
    pub note: bool,
    pub session: Option<SessionIdentity>,
    pub request: Option<u64>,
    pub run: Option<RunId>,
    /// The broker sees no invocation identity (only its revocation).
    pub invocation: Option<u64>,
    pub attempt: Option<u32>,
    pub kind: Option<&'static str>,
    pub request_ordinal: Option<u64>,
    pub phase: Option<String>,
    pub commitment: Option<String>,
}

/// What an armed point does. Every arming turns recording on for all points.
pub enum Action {
    /// Record only.
    Trace,
    /// At the first arrival: acknowledge, then block until the [`Held`] is
    /// dropped. A pause not released within 10 s, or whose harness is gone,
    /// is an irrevocable harness failure: `TIMEOUT <cell> <point>` is written
    /// to `result`, then the process aborts (design 3.3).
    Pause {
        cell: &'static str,
        result: Arc<File>,
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
    assert!(POINTS.contains(&point), "not a broker route point: {point}");
    let (ack, acknowledged) = sync_channel(1);
    let (release, released) = sync_channel(1);
    let mut s = state();
    s.tick += 1;
    let id = s.tick;
    let pause = match action {
        Action::Trace => None,
        Action::Pause { cell, result } => Some(Some((released, cell, result))),
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
pub(crate) fn at(point: &'static str, ctx: Ctx<'_>) {
    std::hint::black_box(M.as_bytes());
    if !ARMED.load(Ordering::Relaxed) {
        return;
    }
    fire(point, false, ctx);
}

/// A refusing branch's own reason (design 2.4); never pauses.
pub(crate) fn note(reason: &'static str, ctx: Ctx<'_>) {
    std::hint::black_box(M.as_bytes());
    if !ARMED.load(Ordering::Relaxed) {
        return;
    }
    fire(reason, true, ctx);
}

fn state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(PoisonError::into_inner)
}

fn fire(point: &'static str, note: bool, ctx: Ctx<'_>) {
    let (entry, pending) = {
        let mut s = state();
        s.tick += 1;
        let key = (ctx.session, ctx.request, ctx.run, ctx.attempt);
        let gate = u64::from(!note && GATES.contains(&point));
        let request_ordinal = match s.ordinals.iter_mut().find(|(k, _)| *k == key) {
            Some((_, n)) => {
                *n += gate;
                Some(*n)
            }
            None if gate == 1 => {
                s.ordinals.push((key, 1));
                Some(1)
            }
            None => None,
        };
        let entry = Entry {
            tick: s.tick,
            point,
            note,
            session: ctx.session,
            request: ctx.request,
            run: ctx.run,
            invocation: None,
            attempt: ctx.attempt,
            kind: ctx.kind,
            request_ordinal,
            phase: ctx.phase.map(str::to_owned),
            commitment: ctx.commitment.map(str::to_owned),
        };
        s.entries.push(entry.clone());
        // Only an ordinary append fires the `store.commit` hold (design 4.2.1).
        let eligible = !note && (point != "store.commit" || ctx.kind == Some("ordinary"));
        let found = s.pending.iter().position(|p| eligible && p.point == point);
        (entry, found.map(|i| s.pending.remove(i)))
    };
    let Some(pending) = pending else { return };
    let acknowledged = pending.ack.try_send(entry).is_ok();
    match pending.pause {
        None => panic!("route seam: armed panic at {point}"),
        Some((released, cell, result)) => {
            if !acknowledged || released.recv_timeout(PATIENCE).is_err() {
                FAILED.store(true, Ordering::SeqCst);
                let _ = writeln!(&*result, "TIMEOUT {cell} {point}");
                std::process::abort();
            }
        }
    }
}
