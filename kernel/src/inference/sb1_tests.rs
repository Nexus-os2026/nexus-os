//! SB-1 pre-registered oracles (`docs/security/session-model-bridge-oracles.md`
//! §3): the kernel's per-request admission hook `CallCancel::admit_request`,
//! SB1-K1 … SB1-K9 and the Architect rulings' O2/O3 controls. SB1-S1 is the
//! source guard in `kernel/tests/m2e_transport_guards.rs`.
//!
//! Every fixture here is test-only and kernel-isolated; none ships. `Gate`
//! stands in for a session override: its `cancelled()` is always `None`, and
//! its `admit_request` logs the request, takes its owning hold with
//! `try_lock` (contended or poisoned refuses), observes its predicate (the
//! original generation), optionally pauses, then advances its word by CAS.
//! Ingress bumps the generation without the hold. Evidence is the fake
//! runtime's request log and connection count, joined exchange threads,
//! meter events and dispatch notices; never a production enum alone.

use super::exchange::seams::{self as exchange_seams, AfterConstruction};
use super::*;
use std::thread::JoinHandle;

use RequestKind::{Chat, Metadata};

/// One generation step in the gate's word (`generation << 32 | admitted`).
const GENERATION: u64 = 1 << 32;

/// The requests of a registration and of a call, in order.
#[derive(Debug, Clone, Copy)]
enum Seq {
    Registration,
    Call,
}

impl Seq {
    const ALL: [Seq; 2] = [Seq::Registration, Seq::Call];

    fn paths(self) -> [&'static str; 4] {
        match self {
            Seq::Registration => ["/api/version", "/api/status", "/api/tags", "/api/show"],
            Seq::Call => ["/api/version", "/api/status", "/api/tags", "/api/chat"],
        }
    }

    fn kinds(self) -> [RequestKind; 4] {
        match self {
            Seq::Registration => [Metadata; 4],
            Seq::Call => [Metadata, Metadata, Metadata, Chat],
        }
    }
}

/// Where a pausing gate stands when it hands control to the test.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    /// After the predicate was observed, before the CAS.
    Paused,
    /// After a successful CAS, before the request's dispatch.
    Admitted,
}

struct Pause {
    at: usize,
    stages: Mutex<Sender<Stage>>,
    resume: Mutex<Receiver<()>>,
}

/// The test-only session stand-in.
struct Gate {
    hold: Mutex<()>,
    word: AtomicU64,
    log: Mutex<Vec<(RequestKind, usize)>>,
    server: Log,
    deny_at: Option<usize>,
    refusal: ModelError,
    deny_chat_once: AtomicBool,
    pause: Option<Pause>,
    /// A pause the test never released (the watchdog's refusal).
    abandoned: AtomicBool,
}

impl Gate {
    fn new(fake: &Fake) -> Self {
        Self {
            hold: Mutex::new(()),
            word: AtomicU64::new(0),
            log: Mutex::default(),
            server: fake.log.clone(),
            deny_at: None,
            refusal: ModelError::Cancelled,
            deny_chat_once: AtomicBool::new(false),
            pause: None,
            abandoned: AtomicBool::new(false),
        }
    }

    /// Refuse admission `index` with `refusal`.
    fn deny(mut self, index: usize, refusal: ModelError) -> Self {
        self.deny_at = Some(index);
        self.refusal = refusal;
        self
    }

    fn deny_chat_once(self) -> Self {
        self.deny_chat_once.store(true, Ordering::SeqCst);
        self
    }

    /// Pause admission `at` at both stages; the test answers each stage.
    fn pausing(mut self, at: usize) -> (Self, Receiver<Stage>, Sender<()>) {
        let (stage_tx, stages) = mpsc::channel();
        let (resume, resume_rx) = mpsc::channel();
        self.pause = Some(Pause {
            at,
            stages: Mutex::new(stage_tx),
            resume: Mutex::new(resume_rx),
        });
        (self, stages, resume)
    }

    fn arc(self) -> Arc<Self> {
        Arc::new(self)
    }

    /// Ingress: a new generation, set without the hold.
    fn ingress(&self) {
        self.word.fetch_add(GENERATION, Ordering::SeqCst);
    }

    fn admitted(&self) -> u64 {
        self.word.load(Ordering::SeqCst) % GENERATION
    }

    fn log(&self) -> Vec<(RequestKind, usize)> {
        self.log.lock().unwrap().clone()
    }

    fn stage(&self, index: usize, stage: Stage) -> Result<(), ModelError> {
        let Some(pause) = self.pause.as_ref().filter(|pause| pause.at == index) else {
            return Ok(());
        };
        let _ = pause.stages.lock().unwrap().send(stage);
        // Bounded: a pause the test never releases refuses.
        if pause.resume.lock().unwrap().recv_timeout(PATIENCE).is_err() {
            self.abandoned.store(true, Ordering::SeqCst);
            return Err(ModelError::Cancelled);
        }
        Ok(())
    }
}

impl CallCancel for Gate {
    fn cancelled(&self) -> Option<CancelCause> {
        None
    }

    fn admit_request(&self, kind: RequestKind) -> Result<(), ModelError> {
        let index = {
            let mut log = self.log.lock().unwrap();
            log.push((kind, self.server.lock().unwrap().len()));
            log.len() - 1
        };
        let Ok(_hold) = self.hold.try_lock() else {
            return Err(ModelError::Cancelled);
        };
        let seen = self.word.load(Ordering::SeqCst);
        if seen / GENERATION != 0 {
            return Err(ModelError::Cancelled);
        }
        if self.deny_at == Some(index) {
            return Err(self.refusal);
        }
        if kind == Chat && self.deny_chat_once.swap(false, Ordering::SeqCst) {
            return Err(ModelError::Cancelled);
        }
        self.stage(index, Stage::Paused)?;
        self.word
            .compare_exchange(seen, seen + 1, Ordering::SeqCst, Ordering::SeqCst)
            .map_err(|_| ModelError::Cancelled)?;
        self.stage(index, Stage::Admitted)
    }
}

/// The ordinary caller's cancellation (the default hook) of a `Ctx`.
struct CtxCancel(Arc<Ctx>);

impl CallCancel for CtxCancel {
    fn cancelled(&self) -> Option<CancelCause> {
        self.0.cancel.cancelled()
    }
}

/// Every dispatch notice.
#[derive(Default)]
struct Notices(Mutex<Vec<u64>>);

impl Notices {
    fn count(&self) -> usize {
        self.0.lock().unwrap().len()
    }
}

impl CallObserver for Notices {
    fn dispatched(&self, sent_mono_ms: u64) {
        self.0.lock().unwrap().push(sent_mono_ms);
    }
}

/// A meter that engages the caller's stop right after its charge.
struct StopOnCharge(Arc<Ctx>);

impl CallMeter for StopOnCharge {
    fn charge(&self) -> Result<Charged, MeterRefusal> {
        let charged = self.0.meter.charge();
        self.0.controller.engage();
        charged
    }

    fn settle(&self, call: u32) {
        self.0.meter.settle(call);
    }
}

/// The seam running `action` after the construction of request `index`.
fn at_request(index: usize, action: impl Fn() + Send + Sync + 'static) -> AfterConstruction {
    let seen = AtomicUsize::new(0);
    Arc::new(move |_kind| {
        if seen.fetch_add(1, Ordering::SeqCst) == index {
            action();
        }
    })
}

/// A fake runtime, a caller, and for a call a transport verified (with the
/// ordinary cancellation) whose requests are then forgotten.
struct Rig {
    seq: Seq,
    fake: Fake,
    ctx: Arc<Ctx>,
    transport: Option<Arc<VerifiedOllama>>,
    from: usize,
    connections: usize,
    notices: Arc<Notices>,
}

impl Rig {
    fn new(seq: Seq) -> Self {
        let fake = Fake::healthy();
        let ctx = Ctx::new();
        let transport = match seq {
            Seq::Registration => None,
            Seq::Call => Some(verified(&fake, &ctx)),
        };
        let (from, connections) = (fake.paths().len(), fake.connections());
        Self {
            seq,
            fake,
            ctx,
            transport,
            from,
            connections,
            notices: Arc::default(),
        }
    }

    fn gate(&self) -> Gate {
        Gate::new(&self.fake)
    }

    /// Run the sequence under `cancel`, with the seam and meter if given (a
    /// call also has the rig's notices as its observer).
    fn start(
        &self,
        cancel: Arc<dyn CallCancel>,
        seam: Option<AfterConstruction>,
        meter: Option<Arc<dyn CallMeter>>,
    ) -> Receiver<Outcome<()>> {
        let (ctx, notices) = (self.ctx.clone(), self.notices.clone());
        let transport = self.transport.clone();
        let endpoint = self.fake.endpoint();
        in_thread(
            move || {
                if let Some(seam) = seam {
                    exchange_seams::set_after_construction(seam);
                }
                match transport {
                    None => VerifiedOllama::verify(
                        endpoint,
                        &spec(),
                        cancel.as_ref(),
                        &ctx.deadline,
                        &ctx.watch,
                    )
                    .map(|_| ()),
                    Some(transport) => {
                        let meter: &dyn CallMeter = match &meter {
                            Some(meter) => meter.as_ref(),
                            None => &ctx.meter,
                        };
                        let call = CallContext {
                            meter,
                            cancel: cancel.as_ref(),
                            deadline: &ctx.deadline,
                            stop: &ctx.watch,
                            observer: Some(notices.as_ref()),
                        };
                        transport.call(&small(), &call).map(|_| ())
                    }
                }
            },
            None,
        )
    }

    fn run(&self, cancel: Arc<dyn CallCancel>, seam: Option<AfterConstruction>) -> Outcome<()> {
        done(&self.start(cancel, seam, None))
    }

    /// The requests the runtime received since the rig was ready.
    fn sent(&self) -> Vec<String> {
        self.fake.paths()[self.from..].to_vec()
    }

    /// The first `n` paths of the sequence.
    fn first(&self, n: usize) -> Vec<String> {
        self.seq.paths()[..n]
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    /// The meter events a sequence refused at request `index` leaves.
    fn charged_before(&self, index: usize) -> Vec<Metered> {
        match (self.seq, index) {
            (Seq::Call, 3) => vec![Metered::Charge(1), Metered::Settle(1)],
            _ => Vec::new(),
        }
    }
}

/// Holds a gate's owning hold from another thread, taken at a seam.
struct Contender {
    gate: Arc<Gate>,
    release: Mutex<Option<Sender<()>>>,
    holder: Mutex<Option<JoinHandle<()>>>,
}

impl Contender {
    fn new(gate: &Arc<Gate>) -> Arc<Self> {
        Arc::new(Self {
            gate: gate.clone(),
            release: Mutex::default(),
            holder: Mutex::default(),
        })
    }

    /// Take the hold on a holder thread and return once it is held.
    fn hold_now(&self) {
        let (held_tx, held) = mpsc::channel();
        let (release, release_rx) = mpsc::channel::<()>();
        let gate = self.gate.clone();
        let holder = std::thread::spawn(move || {
            let _hold = gate.hold.lock().unwrap();
            let _ = held_tx.send(());
            let _ = release_rx.recv_timeout(PATIENCE);
        });
        held.recv_timeout(PATIENCE).expect("the hold was taken");
        *self.release.lock().unwrap() = Some(release);
        *self.holder.lock().unwrap() = Some(holder);
    }

    /// Release the hold; whether one was taken.
    fn release(&self) -> bool {
        let Some(release) = self.release.lock().unwrap().take() else {
            return false;
        };
        let _ = release.send(());
        let holder = self.holder.lock().unwrap().take().expect("a holder");
        holder.join().expect("the holder ended");
        true
    }
}

// ── SB1-K1: the default ─────────────────────────────────────────────────────

#[test]
fn sb1_default_hook_admits_live_and_refuses_cancelled() {
    let live = Flag::default();
    let cancelled = Flag(AtomicBool::new(true));
    for kind in [Metadata, Chat] {
        assert_eq!(live.admit_request(kind), Ok(()), "{kind:?}");
        assert_eq!(
            cancelled.admit_request(kind),
            Err(ModelError::Cancelled),
            "{kind:?}"
        );
    }
}

// ── SB1-K2: once per request, with its kind ─────────────────────────────────

#[test]
fn sb1_hook_runs_once_per_request_with_its_kind() {
    for seq in Seq::ALL {
        let rig = Rig::new(seq);
        let gate = rig.gate().arc();
        let outcome = rig.run(gate.clone(), None);
        assert_eq!(outcome.code(), Ok(()), "{seq:?}");
        assert_eq!(outcome.live, 0, "{seq:?}: joined");
        let log: Vec<(RequestKind, usize)> = gate
            .log()
            .into_iter()
            .map(|(kind, seen)| (kind, seen - rig.from))
            .collect();
        let expected: Vec<(RequestKind, usize)> = seq.kinds().into_iter().zip(0..4).collect();
        assert_eq!(log, expected, "{seq:?}: one admission per request");
        assert_eq!(rig.sent(), rig.first(4), "{seq:?}");
        assert_eq!(gate.admitted(), 4, "{seq:?}");
        assert_eq!(outcome.exchanges, 4, "{seq:?}: hook count == request count");
    }
}

// ── SB1-K3: registration metadata refused at each gate ──────────────────────

#[test]
fn sb1_registration_metadata_refused_at_each_gate_sends_nothing_further() {
    for i in 0..4 {
        let rig = Rig::new(Seq::Registration);
        let gate = rig.gate().deny(i, ModelError::Cancelled).arc();
        let outcome = rig.run(gate.clone(), None);
        assert_eq!(outcome.code(), Err(ModelError::Cancelled), "admission {i}");
        assert_eq!(rig.fake.paths().len(), i, "admission {i}: requests");
        assert_eq!(rig.fake.connections(), i, "admission {i}: connections");
        assert_eq!(outcome.live, 0, "admission {i}: joined");
        assert_eq!(gate.log().len(), i + 1, "admission {i}: hook calls");
    }
}

// ── SB1-K4: call metadata refused at each gate ──────────────────────────────

#[test]
fn sb1_call_metadata_refused_at_each_gate_charges_and_sends_nothing() {
    for i in 0..3 {
        let rig = Rig::new(Seq::Call);
        let gate = rig.gate().deny(i, ModelError::Cancelled).arc();
        let outcome = rig.run(gate.clone(), None);
        assert_eq!(outcome.code(), Err(ModelError::Cancelled), "admission {i}");
        assert_eq!(rig.sent(), rig.first(i), "admission {i}: requests");
        assert_eq!(rig.fake.count("/api/chat"), 0, "admission {i}");
        assert_eq!(rig.fake.connections(), rig.connections + i, "admission {i}");
        assert_eq!(rig.ctx.meter.events(), NOTHING, "admission {i}: no charge");
        assert_eq!(outcome.live, 0, "admission {i}: joined");
        assert_eq!(gate.log().len(), i + 1, "admission {i}: hook calls");
    }
}

// ── SB1-K5: chat refused at its gate, after the charge ──────────────────────

#[test]
fn sb1_chat_refused_at_its_gate_after_the_charge_sends_and_notifies_nothing() {
    let rig = Rig::new(Seq::Call);
    let gate = rig.gate().deny(3, ModelError::Cancelled).arc();
    let outcome = rig.run(gate.clone(), None);
    assert_eq!(outcome.code(), Err(ModelError::Cancelled));
    assert_eq!(rig.sent(), rig.first(3));
    assert_eq!(rig.fake.count("/api/chat"), 0);
    assert_eq!(
        rig.ctx.meter.events(),
        [Metered::Charge(1), Metered::Settle(1)],
        "charged, settled, not refunded"
    );
    assert_eq!(rig.notices.count(), 0, "no dispatch notice");
    assert_eq!(gate.log().last().map(|(kind, _)| *kind), Some(Chat));
    assert_eq!(outcome.live, 0);

    // Positive control: the same observer is notified of an admitted chat
    // where the host clock can be read.
    let rig = Rig::new(Seq::Call);
    let outcome = rig.run(rig.gate().arc(), None);
    assert_eq!(outcome.code(), Ok(()));
    assert_eq!(rig.fake.count("/api/chat"), 1);
    if cfg!(any(target_os = "linux", target_os = "macos")) {
        assert_eq!(rig.notices.count(), 1, "an admitted chat is notified");
    }
}

// ── SB1-K6: ingress between construction and admission ──────────────────────

#[test]
fn pre_gate_ingress_request_count_eq_zero() {
    for seq in Seq::ALL {
        for i in 0..4 {
            let rig = Rig::new(seq);
            let gate = rig.gate().arc();
            let ingress = gate.clone();
            let seam = at_request(i, move || ingress.ingress());
            let outcome = rig.run(gate.clone(), Some(seam));
            let what = format!("{seq:?} request {i}");
            assert_eq!(outcome.code(), Err(ModelError::Cancelled), "{what}");
            assert_eq!(rig.sent(), rig.first(i), "{what}: that and later = 0");
            assert_eq!(rig.notices.count(), 0, "{what}: notices");
            assert_eq!(rig.ctx.meter.events(), rig.charged_before(i), "{what}");
            assert_eq!(gate.log().len(), i + 1, "{what}: admission reached");
            assert_eq!(outcome.live, 0, "{what}: joined");
        }
    }
}

// ── SB1-K7: MP-SB-6-default control ─────────────────────────────────────────

#[test]
fn session_override_required_at_each_request() {
    for seq in Seq::ALL {
        for i in 0..4 {
            let rig = Rig::new(seq);
            let gate = rig.gate().arc();
            let contender = Contender::new(&gate);
            let at = contender.clone();
            let seam = at_request(i, move || at.hold_now());
            let outcome = rig.run(gate.clone(), Some(seam));
            let held = contender.release();
            let what = format!("{seq:?} request {i}");
            assert!(held, "{what}: the owning hold was contended");
            assert_eq!(outcome.code(), Err(ModelError::Cancelled), "{what}");
            assert_eq!(rig.sent(), rig.first(i), "{what}: that and later = 0");
            assert_eq!(rig.notices.count(), 0, "{what}: notices");
            assert_eq!(gate.admitted(), i as u64, "{what}: no CAS under contention");
            assert_eq!(outcome.live, 0, "{what}: joined");
        }
    }
}

// ── SB1-K8: paused after the predicate, before the CAS ──────────────────────

/// Answer the gate's stages until the sequence returns: `schedule` maps
/// each stage to whether the test performs ingress before resuming it.
fn drive(
    rig: &Rig,
    gate: &Arc<Gate>,
    stages: &Receiver<Stage>,
    resume: &Sender<()>,
    schedule: impl Fn(Stage) -> bool,
) -> (Outcome<()>, Vec<Stage>) {
    let rx = rig.start(gate.clone(), None, None);
    let watchdog = std::time::Instant::now() + PATIENCE;
    let mut seen = Vec::new();
    loop {
        assert!(
            std::time::Instant::now() < watchdog,
            "the schedule completed"
        );
        match stages.recv_timeout(Duration::from_millis(50)) {
            Ok(stage) => {
                seen.push(stage);
                if schedule(stage) {
                    gate.ingress();
                }
                resume.send(()).unwrap();
            }
            Err(_) => {
                if let Ok(outcome) = rx.try_recv() {
                    while let Ok(stage) = stages.try_recv() {
                        seen.push(stage);
                    }
                    return (outcome, seen);
                }
            }
        }
        assert!(seen.len() <= 2, "at most two stages");
    }
}

#[test]
fn sb1_paused_before_cas_ingress_wins_dispatches_nothing() {
    for seq in Seq::ALL {
        for i in 0..4 {
            let what = format!("{seq:?} request {i}");
            // No interference: the full sequence.
            let rig = Rig::new(seq);
            let (gate, stages, resume) = rig.gate().pausing(i);
            let gate = gate.arc();
            let (outcome, seen) = drive(&rig, &gate, &stages, &resume, |_| false);
            assert_eq!(outcome.code(), Ok(()), "{what}: no interference");
            assert_eq!(seen, [Stage::Paused, Stage::Admitted], "{what}");
            assert_eq!(rig.sent(), rig.first(4), "{what}: no interference");

            // Ingress wins the race to the CAS.
            let rig = Rig::new(seq);
            let (gate, stages, resume) = rig.gate().pausing(i);
            let gate = gate.arc();
            let (outcome, seen) = drive(&rig, &gate, &stages, &resume, |stage| {
                stage == Stage::Paused
            });
            assert_eq!(outcome.code(), Err(ModelError::Cancelled), "{what}");
            assert_eq!(seen, [Stage::Paused], "{what}: the CAS failed");
            assert_eq!(rig.sent(), rig.first(i), "{what}: that and later = 0");
            assert_eq!(rig.notices.count(), 0, "{what}: notices");
            assert_eq!(gate.admitted(), i as u64, "{what}");
            assert!(!gate.abandoned.load(Ordering::SeqCst), "{what}");
            assert_eq!(outcome.live, 0, "{what}: joined");
        }
    }
}

#[test]
fn sb1_paused_before_cas_sink_wins_admits_only_that_request() {
    for seq in Seq::ALL {
        for i in 0..4 {
            let what = format!("{seq:?} request {i}");
            let rig = Rig::new(seq);
            let (gate, stages, resume) = rig.gate().pausing(i);
            let gate = gate.arc();
            let (outcome, seen) = drive(&rig, &gate, &stages, &resume, |stage| {
                stage == Stage::Admitted
            });
            assert_eq!(seen, [Stage::Paused, Stage::Admitted], "{what}");
            assert_eq!(rig.sent(), rig.first(i + 1), "{what}: exactly that one");
            assert_eq!(gate.admitted(), i as u64 + 1, "{what}");
            if i < 3 {
                assert_eq!(outcome.code(), Err(ModelError::Cancelled), "{what}");
                assert_eq!(gate.log().len(), i + 2, "{what}: the next refused");
            } else {
                // The entered last request completes; nothing follows it.
                assert_eq!(outcome.code(), Ok(()), "{what}");
                assert_eq!(gate.log().len(), 4, "{what}");
            }
            if matches!(seq, Seq::Call)
                && i == 3
                && cfg!(any(target_os = "linux", target_os = "macos"))
            {
                assert_eq!(rig.notices.count(), 1, "{what}: the one entered chat");
            } else {
                assert_eq!(rig.notices.count(), 0, "{what}: notices");
            }
            assert!(!gate.abandoned.load(Ordering::SeqCst), "{what}");
            assert_eq!(outcome.live, 0, "{what}: joined");
        }
    }
}

// ── SB1-K9: no kernel retry of a refused chat ───────────────────────────────

#[test]
fn attempt_chat_count_le_one() {
    let rig = Rig::new(Seq::Call);
    let gate = rig.gate().deny_chat_once().arc();
    let outcome = rig.run(gate.clone(), None);
    assert_eq!(outcome.code(), Err(ModelError::Cancelled));
    let chats = gate.log().iter().filter(|(kind, _)| *kind == Chat).count();
    assert_eq!(chats, 1, "the hook was called once for the chat");
    assert_eq!(rig.fake.count("/api/chat"), 0);
    assert_eq!(rig.sent(), rig.first(3));
    assert_eq!(
        rig.ctx.meter.events(),
        [Metered::Charge(1), Metered::Settle(1)]
    );
}

// ── Ruling O2: a cancel during construction is now refused (stricter) ───────

/// Before SB-1 a cancel arriving during a request's synchronous construction
/// could still be notified and sent, then end the call `Cancelled`. The
/// default hook now refuses it before the notice and the send: the result
/// code is unchanged, the window is narrower.
#[test]
fn sb1_o2_cancel_during_construction_is_stricter_no_notice_no_send() {
    for seq in Seq::ALL {
        for i in 0..4 {
            let rig = Rig::new(seq);
            let ctx = rig.ctx.clone();
            let seam = at_request(i, move || ctx.cancel.0.store(true, Ordering::SeqCst));
            let outcome = rig.run(Arc::new(CtxCancel(rig.ctx.clone())), Some(seam));
            let what = format!("{seq:?} request {i}");
            assert_eq!(
                outcome.code(),
                Err(ModelError::Cancelled),
                "{what}: same code"
            );
            assert_eq!(rig.sent(), rig.first(i), "{what}: no send");
            assert_eq!(rig.notices.count(), 0, "{what}: no notice");
            assert_eq!(rig.ctx.meter.events(), rig.charged_before(i), "{what}");
            assert_eq!(outcome.live, 0, "{what}: joined");
        }
    }
}

// ── Ruling O3: a hook's Stopped is the pre-dispatch check's Stopped ─────────

#[test]
fn sb1_o3_hook_stopped_matches_the_pre_dispatch_check() {
    type Observed = (Result<(), ModelError>, usize, usize, Vec<Metered>, usize);
    let observe = |rig: &Rig, outcome: &Outcome<()>| -> Observed {
        assert_eq!(outcome.live, 0, "joined");
        (
            outcome.code(),
            rig.ctx.stop.halts().len(),
            rig.fake.count("/api/chat"),
            rig.ctx.meter.events(),
            rig.notices.count(),
        )
    };

    // The hook refuses the chat with Stopped.
    let rig = Rig::new(Seq::Call);
    let gate = rig.gate().deny(3, ModelError::Stopped).arc();
    let hook = observe(&rig, &rig.run(gate.clone(), None));
    assert_eq!(gate.log().len(), 4, "the hook refused the chat");

    // The existing pre-dispatch check: a Stop engaged after the charge.
    let rig = Rig::new(Seq::Call);
    let meter: Arc<dyn CallMeter> = Arc::new(StopOnCharge(rig.ctx.clone()));
    let cancel = Arc::new(CtxCancel(rig.ctx.clone()));
    let check = observe(&rig, &done(&rig.start(cancel, None, Some(meter))));

    let expected: Observed = (
        Err(ModelError::Stopped),
        0,
        0,
        vec![Metered::Charge(1), Metered::Settle(1)],
        0,
    );
    assert_eq!(check, expected, "the pre-dispatch check");
    assert_eq!(hook, check, "no extra record_stop, same outcome");
}
