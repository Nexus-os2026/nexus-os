//! P2-V1-R3B-I1 fixture controls for the custody core (`support/custody/`):
//! the same owner survives failed and exhausted cleanup, reporting, refused
//! shutdown and explicit recovery; admission is reserved, acknowledged and
//! linearized against cancellation; an unknown native outcome stays pending
//! until its own bound completion; expected injected conditions stay apart
//! from real failures, and no recovery clears a failure; dependencies are
//! released only after native completion; evidence is reserved before
//! admission and every acknowledgement binds one exact record; replayed,
//! stale and conflicting events act on nothing.
//!
//! These are not live evidence. No process, cgroup, scope, user manager, bus,
//! socket, file or record storage is touched: every adapter is an in-process
//! stand-in, every instant is injected, and the only threads are this test's
//! own, ordered by channels and barriers (a timeout only ever fails a control
//! that would otherwise hang). Native owners are witnesses whose drop is
//! logged, so each control can tell whether custody ever let one go.

#[path = "support/custody/mod.rs"]
mod custody;

use std::collections::{HashMap, VecDeque};
use std::sync::mpsc;
use std::sync::{Arc, Barrier, Mutex};
use std::thread;
use std::time::Duration;

use custody::*;

const GENERATION: Generation = Generation::new([0x11; 16]);
const OTHER: Generation = Generation::new([0x22; 16]);

/// Small bounds for the controls ([`Config::LIVE`] is the live harness's).
const TEST: Config = Config {
    automatic_attempts: 3,
    recovery_budget: 4,
    recovery_spacing_millis: 10,
    lease_millis: 1_000,
    record_capacity: 128,
    control_reserve: 2,
    receipt_limit: 4,
    unexpected_limit: 2,
    failure_detail_limit: 8,
    detail_chars: 64,
    incident_limit: 4,
};

/// How long a control waits for another thread before failing: a watchdog,
/// never a proof of ordering.
const WATCHDOG: Duration = Duration::from_secs(30);

type DropLog = Arc<Mutex<Vec<u64>>>;

fn logged(log: &DropLog) -> Vec<u64> {
    log.lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
}

/// A stand-in native owner: a token whose drop is logged. With no kind, its
/// own declaration panics.
struct Witness {
    token: u64,
    kind: Option<SlotKind>,
    drops: DropLog,
}

impl Drop for Witness {
    fn drop(&mut self) {
        self.drops
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(self.token);
    }
}

impl Resource for Witness {
    fn kind(&self) -> SlotKind {
        match self.kind {
            Some(kind) => kind,
            None => panic!("injected owner declaration panic"),
        }
    }
}

/// The native stand-in: it produces an owner only for an admitted
/// operation's ticket, and records every owner it produced.
struct Native {
    drops: DropLog,
    produced: Vec<(u64, ActionId)>,
}

impl Native {
    fn produce(&mut self, ticket: &OpTicket) -> Witness {
        self.produce_as(ticket, Some(ticket.kind()))
    }

    fn produce_as(&mut self, ticket: &OpTicket, kind: Option<SlotKind>) -> Witness {
        let token = self.produced.len() as u64 + 1;
        self.produced.push((token, ticket.action()));
        Witness {
            token,
            kind,
            drops: Arc::clone(&self.drops),
        }
    }
}

/// What one scripted cleanup attempt establishes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Plan {
    /// Every fact: a tree's subtree, reaping and output; a removal.
    Confirm,
    /// Nothing.
    Fail,
    /// The subtree is gone; the direct child is not reaped.
    SubtreeOnly,
    /// The direct child is reaped; the subtree is not gone.
    ReapedOnly,
    /// The tree ended natively; its output is still pending.
    OutputPending,
    /// The attempt panics.
    Panic,
}

fn report(kind: SlotKind, plan: Plan) -> CleanupReport {
    let fact = |confirmed: bool| {
        if confirmed {
            Observed::Confirmed
        } else {
            Observed::StillPending
        }
    };
    match kind {
        SlotKind::Process => {
            let (subtree, reaped, output) = match plan {
                Plan::Confirm => (true, true, true),
                Plan::SubtreeOnly => (true, false, false),
                Plan::ReapedOnly => (false, true, false),
                Plan::OutputPending => (true, true, false),
                Plan::Fail | Plan::Panic => (false, false, false),
            };
            CleanupReport {
                subtree: fact(subtree),
                reaped: fact(reaped),
                output: if output {
                    OutputObserved::Complete
                } else {
                    OutputObserved::StillPending
                },
                removed: Observed::NotLooked,
            }
        }
        SlotKind::Workspace | SlotKind::Fixture => CleanupReport {
            removed: fact(plan == Plan::Confirm),
            ..CleanupReport::NOTHING
        },
    }
}

/// A scripted cleanup adapter: per kind, its next plans in order, then the
/// default. It records every attempt as (token, kind, plan).
struct Scripted {
    default: Plan,
    script: HashMap<SlotKind, VecDeque<Plan>>,
    calls: Vec<(u64, SlotKind, Plan)>,
}

impl Scripted {
    fn always(default: Plan) -> Self {
        Self {
            default,
            script: HashMap::new(),
            calls: Vec::new(),
        }
    }

    fn first(mut self, kind: SlotKind, plans: &[Plan]) -> Self {
        self.script
            .entry(kind)
            .or_default()
            .extend(plans.iter().copied());
        self
    }

    fn attempts_on(&self, token: u64) -> usize {
        self.calls.iter().filter(|call| call.0 == token).count()
    }
}

impl Cleanup<Witness> for Scripted {
    fn attempt(&mut self, owner: &mut Witness, entry: &EntryView) -> CleanupReport {
        let plan = self
            .script
            .get_mut(&entry.kind)
            .and_then(VecDeque::pop_front)
            .unwrap_or(self.default);
        self.calls.push((owner.token, entry.kind, plan));
        if plan == Plan::Panic {
            panic!("injected cleanup adapter panic");
        }
        report(entry.kind, plan)
    }
}

/// A cleanup adapter that stalls inside its first attempt until released.
struct Stalling {
    inner: Scripted,
    entered: mpsc::Sender<()>,
    release: mpsc::Receiver<()>,
    stalled: bool,
}

impl Cleanup<Witness> for Stalling {
    fn attempt(&mut self, owner: &mut Witness, entry: &EntryView) -> CleanupReport {
        if !self.stalled {
            self.stalled = true;
            self.entered.send(()).expect("the control side is waiting");
            self.release
                .recv_timeout(WATCHDOG)
                .expect("released by the control side");
        }
        self.inner.attempt(owner, entry)
    }
}

/// The recorder stand-in: it keeps what it was given, in memory. Nothing here
/// is durable; each control acknowledges records itself.
#[derive(Default)]
struct Journal {
    submitted: Vec<RecordIntent>,
}

impl RecordSink for Journal {
    fn submit(&mut self, intent: &RecordIntent) {
        self.submitted.push(intent.clone());
    }
}

/// A recorder that stalls inside its first submission until released.
struct StallingSink {
    inner: Journal,
    entered: mpsc::Sender<()>,
    release: mpsc::Receiver<()>,
    stalled: bool,
}

impl RecordSink for StallingSink {
    fn submit(&mut self, intent: &RecordIntent) {
        if !self.stalled {
            self.stalled = true;
            self.entered.send(()).expect("the control side is waiting");
            self.release
                .recv_timeout(WATCHDOG)
                .expect("released by the control side");
        }
        self.inner.submit(intent);
    }
}

/// A recorder that panics on every submission.
struct PanickingSink;

impl RecordSink for PanickingSink {
    fn submit(&mut self, _intent: &RecordIntent) {
        panic!("injected recorder panic");
    }
}

/// The records layer's stand-in: it answers only what it was given.
struct Validator {
    answers: Vec<(IncidentBinding, ValidatedDisposition)>,
    panics: bool,
}

impl DispositionValidator for Validator {
    fn validate(&self, binding: &IncidentBinding) -> Option<ValidatedDisposition> {
        if self.panics {
            panic!("injected validator panic");
        }
        self.answers
            .iter()
            .find(|(asked, _)| asked == binding)
            .map(|(_, answer)| *answer)
    }
}

/// Run `f` on its own thread and wait for its answer, so that a blocked
/// control side fails the control instead of hanging it.
fn answered<T: Send + 'static>(what: &str, f: impl FnOnce() -> T + Send + 'static) -> T {
    let (sender, receiver) = mpsc::channel();
    let worker = thread::spawn(move || {
        let _ = sender.send(f());
    });
    let answer = receiver
        .recv_timeout(WATCHDOG)
        .unwrap_or_else(|_| panic!("{what} blocked behind a stalled adapter"));
    worker.join().expect("the watchdog worker");
    answer
}

/// One custody under test with its stand-ins, an injected clock and the
/// owners handed back to the test.
struct Lab {
    config: Config,
    custody: Option<Custody<Witness>>,
    control: Control,
    cap: Option<LeaseCap>,
    journal: Journal,
    native: Native,
    drops: DropLog,
    clock: u64,
    acked: usize,
    seq: u64,
    returned: Vec<Witness>,
}

impl Lab {
    fn new(config: Config) -> Self {
        Self::with(config, GENERATION, Vec::new())
    }

    fn with(config: Config, generation: Generation, prior: Vec<PriorIncident>) -> Self {
        let drops = DropLog::default();
        let (custody, control) =
            Custody::new(config, generation, prior, Tick(0)).expect("a valid configuration");
        Self {
            config,
            custody: Some(custody),
            control,
            cap: None,
            journal: Journal::default(),
            native: Native {
                drops: Arc::clone(&drops),
                produced: Vec::new(),
            },
            drops,
            clock: 0,
            acked: 0,
            seq: 0,
            returned: Vec::new(),
        }
    }

    fn c(&mut self) -> &mut Custody<Witness> {
        self.custody.as_mut().expect("the custody is open")
    }

    fn now(&mut self) -> Tick {
        self.clock += 1;
        Tick(self.clock)
    }

    fn later(&mut self, millis: u64) -> Tick {
        self.clock += millis;
        Tick(self.clock)
    }

    fn snap(&self) -> Arc<Snapshot> {
        self.control.status().snapshot
    }

    fn dropped(&self) -> Vec<u64> {
        logged(&self.drops)
    }

    fn flush(&mut self) {
        let now = self.now();
        let custody = self.custody.as_mut().expect("the custody is open");
        custody
            .flush_records(&mut self.journal, now)
            .expect("the journal accepts every record");
    }

    /// Acknowledge submitted records in order, up to (not including) `end`.
    fn ack_until(&mut self, end: usize) {
        while self.acked < end {
            let ack = RecordAck::of(&self.journal.submitted[self.acked]);
            let now = self.now();
            assert_eq!(self.c().acknowledge(&ack, now), AckOutcome::Acknowledged);
            self.acked += 1;
        }
    }

    /// Flush, then acknowledge everything submitted.
    fn ack_all(&mut self) {
        self.flush();
        self.ack_until(self.journal.submitted.len());
    }

    /// Flush and acknowledge in order, stopping at the first record custody
    /// does not accept (after a recording failure).
    fn try_ack_all(&mut self) {
        self.flush();
        while self.acked < self.journal.submitted.len() {
            let ack = RecordAck::of(&self.journal.submitted[self.acked]);
            let now = self.now();
            if self.c().acknowledge(&ack, now) != AckOutcome::Acknowledged {
                return;
            }
            self.acked += 1;
        }
    }

    fn start(&mut self) -> Tick {
        let now = self.now();
        let cap = self.c().start_run(now).expect("the run starts");
        self.cap = Some(cap);
        self.ack_all();
        now
    }

    fn begin(&mut self, case: u32, expectation: Expectation) {
        let now = self.now();
        self.c()
            .begin_case(CaseId(case), expectation, now)
            .expect("the case begins");
    }

    /// Reserve, make the start record durable, and admit.
    fn admit(&mut self, kind: SlotKind) -> OpTicket {
        let now = self.now();
        let reservation = self.c().reserve(kind, now).expect("the action is reserved");
        self.ack_all();
        let now = self.now();
        self.c()
            .admit(reservation, now)
            .expect("the action is admitted")
    }

    /// Admit, and deposit the owner the operation produced (created, or
    /// retained by its own finalization).
    fn deposit(&mut self, kind: SlotKind, retained: bool) -> (EntryId, u64) {
        let ticket = self.admit(kind);
        let owner = self.native.produce(&ticket);
        let token = owner.token;
        let outcome = if retained {
            NativeOutcome::Retained(owner)
        } else {
            NativeOutcome::Created(owner)
        };
        let now = self.now();
        match self.c().complete(&ticket, outcome, now) {
            Ok(Completion::Deposited(entry)) => (entry, token),
            other => panic!("the owner was not deposited: {other:?}"),
        }
    }

    fn create(&mut self, kind: SlotKind) -> (EntryId, u64) {
        self.deposit(kind, false)
    }

    fn end_case(&mut self, cleanup: &mut Scripted) -> CaseOutcome {
        let now = self.now();
        self.c().end_case(cleanup, now).expect("the case ends")
    }

    fn serve(&mut self, request: Request, cleanup: &mut Scripted, now: Tick) -> RequestOutcome {
        self.control.submit(request).expect("the queue is empty");
        let (served, outcome) = self.c().serve(cleanup, now).expect("a request is queued");
        assert_eq!(served, request);
        outcome
    }

    /// The next explicit recovery attempt, once its spacing has passed.
    fn retry(&mut self, cleanup: &mut Scripted) -> Response {
        self.seq += 1;
        let request = Request {
            generation: self.c().generation(),
            seq: self.seq,
            op: RequestOp::Retry {
                epoch: self.snap().recovery.epoch,
            },
        };
        let now = self.later(self.config.recovery_spacing_millis);
        match self.serve(request, cleanup, now) {
            RequestOutcome::Executed(response) => response,
            other => panic!("the retry was not executed: {other:?}"),
        }
    }

    fn request_shutdown(&mut self) -> ShutdownDecision {
        self.seq += 1;
        let request = Request {
            generation: self.c().generation(),
            seq: self.seq,
            op: RequestOp::Shutdown,
        };
        let now = self.now();
        match self.serve(request, &mut Scripted::always(Plan::Fail), now) {
            RequestOutcome::Executed(Response::Shutdown(decision)) => decision,
            other => panic!("the shutdown request was not executed: {other:?}"),
        }
    }

    /// Owners whose end was confirmed, by token, now back with the test.
    fn released(&mut self) -> Vec<u64> {
        let released = self.c().take_released();
        let tokens = released
            .iter()
            .map(|released| released.owner.token)
            .collect();
        self.returned
            .extend(released.into_iter().map(|released| released.owner));
        tokens
    }

    /// Close; when custody refuses, the same custody stays open.
    fn close(&mut self) -> Option<Closed<Witness>> {
        let now = self.now();
        let custody = self.custody.take().expect("the custody is open");
        match custody.close(now) {
            Ok(closed) => Some(closed),
            Err(custody) => {
                self.custody = Some(*custody);
                None
            }
        }
    }

    fn records(&self) -> Vec<RecordKind> {
        self.journal
            .submitted
            .iter()
            .map(|intent| intent.kind)
            .collect()
    }
}

/// The one held entry.
fn only_entry(snapshot: &Snapshot) -> &EntryView {
    match snapshot.entries.as_slice() {
        [entry] => entry,
        entries => panic!("expected one held entry, found {}", entries.len()),
    }
}

fn count(snapshot: &Snapshot, class: FailureClass) -> u32 {
    snapshot.failure_counts[class.index()]
}

fn is_refused_with(decision: &ShutdownDecision, wanted: Unresolved) -> bool {
    matches!(decision, ShutdownDecision::Refused(unresolved) if unresolved.contains(&wanted))
}

fn only_evidence_pending(decision: &ShutdownDecision) -> bool {
    matches!(
        decision,
        ShutdownDecision::Refused(unresolved)
            if matches!(unresolved.as_slice(), [Unresolved::EvidencePending(_)])
    )
}

/// H1. The same owner survives a failed cleanup, its automatic budget and
/// the recovery budget; only destroying the custody value itself drops it.
#[test]
fn h01_same_owner_survives_failed_cleanup_and_exhausted_budgets() {
    let mut lab = Lab::new(Config {
        recovery_budget: 2,
        ..TEST
    });
    lab.start();
    lab.begin(1, Expectation::Clean);
    let (entry, token) = lab.create(SlotKind::Process);
    let mut cleanup = Scripted::always(Plan::Fail);

    let outcome = lab.end_case(&mut cleanup);
    assert!(
        lab.dropped().is_empty(),
        "[owner-retained] a failed cleanup let go of its owner"
    );
    assert_eq!(
        outcome,
        CaseOutcome {
            case: CaseId(1),
            passed: false,
            resolved: false,
            stopped: true
        }
    );
    assert_eq!(
        cleanup.attempts_on(token),
        3,
        "the automatic budget is spent, never exceeded"
    );
    let snapshot = lab.snap();
    assert_eq!(snapshot.phase, RunPhase::RecoveryRequired);
    let held = only_entry(&snapshot);
    assert_eq!(
        (
            held.entry,
            held.phase,
            held.automatic_attempts,
            held.first_attempt_confirmed,
            held.failure_recorded
        ),
        (entry, EntryPhase::CleanupRequired, 3, Some(false), true)
    );

    for attempt in 1..=2 {
        assert_eq!(
            lab.retry(&mut cleanup),
            Response::Retried {
                attempt,
                resolved: false,
                epoch: u64::from(attempt)
            }
        );
    }
    assert_eq!(
        lab.retry(&mut cleanup),
        Response::RetryRefused(Refusal::BudgetExhausted)
    );
    assert_eq!(
        cleanup.attempts_on(token),
        5,
        "a refused retry attempts nothing"
    );
    let snapshot = lab.snap();
    let held = only_entry(&snapshot);
    assert_eq!(
        (held.entry, held.explicit_attempts),
        (entry, 2),
        "an exhausted budget removes nothing"
    );

    assert!(is_refused_with(
        &lab.c().shutdown_decision(),
        Unresolved::OwnersHeld(1)
    ));
    assert!(lab.close().is_none(), "close hands the same custody back");
    let now = lab.now();
    assert_eq!(lab.c().lend(entry, now, |owner| owner.token), Ok(token));
    assert!(
        lab.dropped().is_empty(),
        "[owner-retained] the owner was let go"
    );

    // Destroying the custody value itself, which this module does not claim
    // to survive, is the only way the owner goes.
    let drops = Arc::clone(&lab.drops);
    drop(lab);
    assert_eq!(logged(&drops), vec![token]);
}

/// H2. Reporting, snapshots, refusals and lending cannot consume the owner;
/// a panicking borrower or cleanup adapter unwinds through its own frames
/// only.
#[test]
fn h02_reporting_snapshots_and_lending_cannot_consume_the_owner() {
    let mut lab = Lab::new(TEST);
    lab.start();
    lab.begin(1, Expectation::Clean);
    let (entry, token) = lab.create(SlotKind::Process);

    let mut kept = Vec::new();
    for _ in 0..64 {
        let status = lab.control.status();
        assert_eq!(status.snapshot.entries.len(), 1);
        kept.push(status.snapshot);
        kept.push(lab.c().snapshot());
        assert!(matches!(
            lab.c().shutdown_decision(),
            ShutdownDecision::Refused(_)
        ));
    }
    drop(kept);
    assert!(
        lab.dropped().is_empty(),
        "[owner-retained] reporting consumed the owner"
    );

    let now = lab.now();
    let panicked = lab
        .c()
        .lend(entry, now, |_| -> u64 { panic!("injected borrower panic") });
    assert_eq!(panicked, Err(LendError::Panicked));
    let now = lab.now();
    assert_eq!(lab.c().lend(entry, now, |owner| owner.token), Ok(token));
    assert!(
        lab.dropped().is_empty(),
        "[owner-retained] a panicking borrower dropped the owner"
    );

    let mut cleanup = Scripted::always(Plan::Confirm).first(SlotKind::Process, &[Plan::Panic]);
    let outcome = lab.end_case(&mut cleanup);
    assert!(
        lab.dropped().is_empty(),
        "[owner-retained] a panicking adapter dropped the owner"
    );
    assert_eq!(
        (outcome.passed, outcome.resolved, outcome.stopped),
        (false, true, false)
    );
    assert_eq!(cleanup.attempts_on(token), 2);
    assert_eq!(lab.released(), vec![token], "the same owner is released");
    assert_eq!(count(&lab.snap(), FailureClass::UnexpectedCleanup), 1);
}

/// H3. A later explicit recovery resolves the run without erasing the
/// earlier failure, and durable completion still needs every record.
#[test]
fn h03_explicit_recovery_resolves_without_erasing_earlier_failure() {
    let mut lab = Lab::new(TEST);
    lab.start();
    lab.begin(1, Expectation::Clean);
    let (_, token) = lab.create(SlotKind::Process);
    let mut failing = Scripted::always(Plan::Fail);
    lab.end_case(&mut failing);
    assert_eq!(
        lab.retry(&mut failing),
        Response::Retried {
            attempt: 1,
            resolved: false,
            epoch: 1
        }
    );
    assert!(lab.released().is_empty());
    let mut confirming = Scripted::always(Plan::Confirm);
    assert_eq!(
        lab.retry(&mut confirming),
        Response::Retried {
            attempt: 2,
            resolved: true,
            epoch: 2
        }
    );
    assert_eq!(
        lab.released(),
        vec![token],
        "the same owner, released by the explicit recovery"
    );

    let snapshot = lab.snap();
    assert_eq!(
        (snapshot.phase, snapshot.verdict),
        (RunPhase::Complete, Verdict::Failed),
        "recovery never turns a failure into a pass"
    );
    assert_eq!(snapshot.recovery.resolved_by, Some(2));
    assert_eq!(count(&snapshot, FailureClass::UnexpectedCleanup), 1);
    assert!(snapshot.failures.iter().any(|failure| {
        failure.class == FailureClass::UnexpectedCleanup && failure.case == Some(CaseId(1))
    }));
    assert_eq!(
        snapshot.evidence.reserved, 0,
        "every reserved record was issued or released"
    );

    lab.flush();
    let records = lab.records();
    assert!(records.contains(&RecordKind::RecoveryAttempt {
        attempt: 1,
        resolved: false
    }));
    assert!(records.contains(&RecordKind::RecoveryAttempt {
        attempt: 2,
        resolved: true
    }));
    assert_eq!(
        records.last(),
        Some(&RecordKind::RunEnded {
            verdict: Verdict::Failed,
            resolved_by: Some(2)
        })
    );

    assert!(only_evidence_pending(&lab.c().shutdown_decision()));
    lab.ack_all();
    assert_eq!(lab.c().shutdown_decision(), ShutdownDecision::Permitted);
    let closed = lab.close().expect("closes once resolved and durable");
    assert_eq!(
        (closed.verdict, closed.resolved_by, closed.durable),
        (Verdict::Failed, Some(2), true)
    );
    assert_eq!(
        closed.failure_counts[FailureClass::UnexpectedCleanup.index()],
        1
    );
}

/// H4. Shutdown is refused while native or required evidence state is
/// unresolved; the request closes admission, its first refusal becomes a
/// control fact, and nothing is released by it.
#[test]
fn h04_shutdown_is_refused_while_native_or_evidence_state_is_unresolved() {
    let mut lab = Lab::new(TEST);
    lab.start();
    lab.begin(1, Expectation::Clean);
    let (_, token) = lab.create(SlotKind::Process);

    let decision = lab.request_shutdown();
    for wanted in [
        Unresolved::RunActive,
        Unresolved::CaseActive,
        Unresolved::OwnersHeld(1),
    ] {
        assert!(
            is_refused_with(&decision, wanted),
            "{wanted:?} in {decision:?}"
        );
    }
    assert!(
        lab.dropped().is_empty(),
        "[owner-retained] a shutdown request let go of an owner"
    );
    let status = lab.control.status();
    assert_eq!(
        status
            .control
            .admission
            .closure
            .map(|closure| closure.reason),
        Some(CancelReason::Shutdown),
        "a shutdown request closes admission"
    );
    assert_eq!(
        status.snapshot.evidence.control_left, 0,
        "the closure and the first refusal are control facts"
    );

    lab.end_case(&mut Scripted::always(Plan::Fail));
    let decision = lab.request_shutdown();
    assert!(is_refused_with(&decision, Unresolved::RecoveryRequired));
    assert!(is_refused_with(&decision, Unresolved::OwnersHeld(1)));
    assert!(lab.close().is_none());
    lab.flush();
    let refusals = lab
        .records()
        .iter()
        .filter(|kind| {
            **kind
                == RecordKind::Control {
                    fact: ControlFact::ShutdownRefused,
                }
        })
        .count();
    assert_eq!(refusals, 1, "only the first refusal is recorded");

    assert_eq!(
        lab.retry(&mut Scripted::always(Plan::Confirm)),
        Response::Retried {
            attempt: 1,
            resolved: true,
            epoch: 1
        }
    );
    let decision = lab.request_shutdown();
    assert!(only_evidence_pending(&decision), "{decision:?}");
    lab.ack_all();
    assert_eq!(lab.request_shutdown(), ShutdownDecision::Permitted);
    let closed = lab.close().expect("closes once nothing is unresolved");
    assert!(closed.durable);
    let released: Vec<u64> = closed
        .released
        .iter()
        .map(|released| released.owner.token)
        .collect();
    assert_eq!(released, vec![token]);

    // An unknown outcome alone refuses shutdown too.
    let mut lab = Lab::new(TEST);
    lab.start();
    lab.begin(1, Expectation::Clean);
    let ticket = lab.admit(SlotKind::Process);
    let now = lab.now();
    assert!(matches!(
        lab.c().complete(&ticket, NativeOutcome::Unknown, now),
        Ok(Completion::StillUnknown)
    ));
    let decision = lab.request_shutdown();
    assert!(
        is_refused_with(&decision, Unresolved::OutcomeUnknown),
        "{decision:?}"
    );
    lab.end_case(&mut Scripted::always(Plan::Confirm));
    assert!(is_refused_with(
        &lab.c().shutdown_decision(),
        Unresolved::OutcomeUnknown
    ));
    assert!(lab.close().is_none());
}

/// H5. Cancellation before admission prevents native creation: the action is
/// settled as never admitted, nothing can reopen admission, and nothing
/// native exists.
#[test]
fn h05_cancellation_before_admission_prevents_native_creation() {
    let mut lab = Lab::new(TEST);
    lab.start();
    lab.begin(1, Expectation::Clean);
    let now = lab.now();
    let reservation = lab
        .c()
        .reserve(SlotKind::Process, now)
        .expect("the action is reserved");
    lab.ack_all();
    let now = lab.now();
    let receipt = lab.control.cancel(CancelReason::Requested, now);
    assert!(receipt.first);
    assert_eq!(
        receipt.closure,
        Closure {
            reason: CancelReason::Requested,
            at: now,
            after: 0
        }
    );

    let now = lab.now();
    let refused = lab
        .c()
        .admit(reservation, now)
        .expect_err("admission is closed");
    assert_eq!(refused.refusal, Refusal::AdmissionClosed(receipt.closure));
    assert!(
        refused.reservation.is_none(),
        "a closed admission settles the action"
    );
    assert!(
        lab.native.produced.is_empty(),
        "nothing was created natively"
    );
    let status = lab.control.status();
    assert_eq!(
        status.control.admission,
        AdmissionView {
            admitted: 0,
            closure: Some(receipt.closure)
        }
    );
    assert!(status.snapshot.operation.is_none());
    lab.flush();
    assert!(lab.records().iter().any(|kind| matches!(
        kind,
        RecordKind::ActionSettled {
            how: Settlement::NotAdmitted,
            ..
        }
    )));

    // Nothing reopens it: not status, a stale cancellation, a later
    // reservation or a new case.
    let _ = lab.control.status();
    let now = lab.now();
    assert_eq!(
        lab.control.cancel(CancelReason::Stop, now),
        CancelReceipt {
            closure: receipt.closure,
            first: false
        }
    );
    let now = lab.now();
    assert_eq!(
        lab.c().reserve(SlotKind::Workspace, now).unwrap_err(),
        Refusal::AdmissionClosed(receipt.closure)
    );
    let outcome = lab.end_case(&mut Scripted::always(Plan::Confirm));
    assert!(outcome.resolved && outcome.stopped);
    let now = lab.now();
    assert!(lab
        .c()
        .begin_case(CaseId(2), Expectation::Clean, now)
        .is_err());
    assert_eq!(
        lab.control.status().control.admission,
        AdmissionView {
            admitted: 0,
            closure: Some(receipt.closure)
        }
    );
    assert!(lab.native.produced.is_empty());
}

/// H5 (linearization). Admission and cancellation race on the gate: each
/// round is decided at the one critical section, and both outcomes keep the
/// closure's count exact.
#[test]
fn h05_admission_linearizes_against_concurrent_cancellation() {
    let mut admitted = 0;
    let mut refused = 0;
    for _ in 0..64 {
        let mut lab = Lab::new(TEST);
        lab.start();
        lab.begin(1, Expectation::Clean);
        let now = lab.now();
        let reservation = lab
            .c()
            .reserve(SlotKind::Process, now)
            .expect("the action is reserved");
        lab.ack_all();
        let barrier = Arc::new(Barrier::new(2));
        let control = lab.control.clone();
        let canceller_barrier = Arc::clone(&barrier);
        let cancel_at = lab.now();
        let canceller = thread::spawn(move || {
            canceller_barrier.wait();
            control.cancel(CancelReason::Requested, cancel_at)
        });
        barrier.wait();
        let now = lab.now();
        let result = lab.c().admit(reservation, now);
        let receipt = canceller.join().expect("the canceller");
        match result {
            Ok(ticket) => {
                admitted += 1;
                assert_eq!(
                    receipt.closure.after, 1,
                    "an admission before the closure is counted in it"
                );
                let owner = lab.native.produce(&ticket);
                let token = owner.token;
                let now = lab.now();
                assert!(matches!(
                    lab.c()
                        .complete(&ticket, NativeOutcome::Created(owner), now),
                    Ok(Completion::Deposited(_))
                ));
                lab.end_case(&mut Scripted::always(Plan::Confirm));
                assert_eq!(lab.released(), vec![token]);
            }
            Err(refusal) => {
                refused += 1;
                assert_eq!(refusal.refusal, Refusal::AdmissionClosed(receipt.closure));
                assert_eq!(receipt.closure.after, 0);
                assert!(lab.native.produced.is_empty());
            }
        }
        assert_eq!(
            lab.control.status().control.admission.admitted,
            receipt.closure.after
        );
        assert!(lab.dropped().is_empty());
    }
    assert_eq!(admitted + refused, 64);
}

/// H6. Cancellation after admission still adopts the owner the admitted
/// operation produces later (straight into required cleanup), including a
/// late success after a timeout.
#[test]
fn h06_cancellation_after_admission_still_adopts_the_later_owner() {
    let mut lab = Lab::new(TEST);
    lab.start();
    lab.begin(1, Expectation::Clean);
    let ticket = lab.admit(SlotKind::Process);
    let now = lab.now();
    let receipt = lab.control.cancel(CancelReason::Requested, now);
    assert_eq!(
        receipt.closure.after, 1,
        "the admitted action precedes the closure"
    );

    let owner = lab.native.produce(&ticket);
    let token = owner.token;
    let now = lab.now();
    let Ok(Completion::Deposited(entry)) =
        lab.c()
            .complete(&ticket, NativeOutcome::Created(owner), now)
    else {
        panic!("the late owner was not adopted");
    };
    let snapshot = lab.snap();
    let held = only_entry(&snapshot);
    assert_eq!(
        (held.entry, held.origin, held.phase),
        (entry, Origin::Created, EntryPhase::CleanupRequired),
        "an owner adopted after cancellation is not put to use"
    );
    let outcome = lab.end_case(&mut Scripted::always(Plan::Confirm));
    assert_eq!((outcome.resolved, outcome.stopped), (true, true));
    assert_eq!(lab.released(), vec![token]);
    let snapshot = lab.snap();
    assert_eq!(
        (snapshot.phase, snapshot.verdict),
        (RunPhase::Complete, Verdict::Failed)
    );
    assert_eq!(count(&snapshot, FailureClass::Cancelled), 1);

    // A late success after a timeout and a cancellation is adopted too.
    let mut lab = Lab::new(TEST);
    lab.start();
    lab.begin(1, Expectation::Clean);
    let ticket = lab.admit(SlotKind::Workspace);
    let now = lab.now();
    assert!(matches!(
        lab.c().complete(&ticket, NativeOutcome::Unknown, now),
        Ok(Completion::StillUnknown)
    ));
    let now = lab.now();
    lab.control.cancel(CancelReason::LeaseLost, now);
    let outcome = lab.end_case(&mut Scripted::always(Plan::Confirm));
    assert_eq!((outcome.passed, outcome.resolved), (false, false));
    let owner = lab.native.produce(&ticket);
    let token = owner.token;
    let now = lab.now();
    assert!(matches!(
        lab.c()
            .complete(&ticket, NativeOutcome::Created(owner), now),
        Ok(Completion::Deposited(_))
    ));
    assert_eq!(
        lab.retry(&mut Scripted::always(Plan::Confirm)),
        Response::Retried {
            attempt: 1,
            resolved: true,
            epoch: 1
        }
    );
    assert_eq!(lab.released(), vec![token]);
    assert!(lab.dropped().is_empty());
}

/// H7. Reading status never renews the lease; renewal is a distinct
/// operation that needs the run's lease capability; neither records
/// anything, so neither can exhaust control evidence.
#[test]
fn h07_status_never_renews_the_lease() {
    let config = Config {
        lease_millis: 100,
        ..TEST
    };
    let mut lab = Lab::new(config);
    let started = lab.start();
    let deadline = started.plus(100);
    let evidence = lab.snap().evidence.clone();
    for _ in 0..1_000 {
        let status = lab.control.status();
        assert_eq!(
            status.control.lease,
            LeaseView {
                state: LeaseState::Active { deadline },
                renewals: 0
            },
            "[status-no-renew] reading status renewed the lease"
        );
    }
    lab.flush();
    let after = lab.snap().evidence.clone();
    assert_eq!(
        (after.issued, after.control_left),
        (evidence.issued, evidence.control_left),
        "status records nothing"
    );

    assert_eq!(
        lab.control.check_lease(Tick(deadline.0 - 1)),
        LeaseState::Active { deadline }
    );
    assert_eq!(
        lab.control.check_lease(deadline),
        LeaseState::Lost { at: deadline }
    );
    let closure = lab
        .control
        .status()
        .control
        .admission
        .closure
        .expect("losing the lease closes admission");
    assert_eq!(
        (closure.reason, closure.at),
        (CancelReason::LeaseLost, deadline)
    );
    let cap = lab.cap.take().expect("the run's lease capability");
    assert_eq!(
        lab.control.renew_lease(&cap, deadline),
        Err(LeaseError::Lost)
    );
    lab.clock = deadline.0;
    let now = lab.now();
    assert_eq!(lab.c().observe_control(now), Ok(Some(closure)));
    assert_eq!(
        lab.snap().phase,
        RunPhase::Complete,
        "an idle run stops at its next safe point"
    );

    // Renewal: distinct, authorized, and recording nothing.
    let mut lab = Lab::new(config);
    let started = lab.start();
    let cap = lab.cap.take().expect("the run's lease capability");
    let evidence = lab.snap().evidence.clone();
    let mut deadline = started.plus(100);
    for step in 1..=50 {
        let now = Tick(started.0 + step * 10);
        deadline = lab
            .control
            .renew_lease(&cap, now)
            .expect("renewed before expiry");
        assert_eq!(deadline, now.plus(100));
    }
    assert_eq!(
        lab.control.status().control.lease,
        LeaseView {
            state: LeaseState::Active { deadline },
            renewals: 50
        }
    );
    lab.flush();
    let after = lab.snap().evidence.clone();
    assert_eq!(
        (after.issued, after.control_left),
        (evidence.issued, evidence.control_left),
        "renewal records nothing"
    );
    let mut other = Lab::with(config, OTHER, Vec::new());
    other.start();
    let foreign = other.cap.take().expect("another run's capability");
    assert_eq!(
        lab.control.renew_lease(&foreign, Tick(started.0 + 501)),
        Err(LeaseError::Foreign)
    );
}

/// H8. A stalled cleanup adapter, recorder or borrower does not block status,
/// cancellation, lease renewal or the request queue: no lock is held across
/// an adapter call, and the snapshot shows the call in progress, with its
/// time, as a past observation.
#[test]
fn h08_stalled_adapters_do_not_block_the_control_side() {
    // A stalled cleanup attempt.
    let mut lab = Lab::new(TEST);
    lab.start();
    lab.begin(1, Expectation::Clean);
    let (entry, token) = lab.create(SlotKind::Process);
    let (entered, entered_rx) = mpsc::channel();
    let (release, release_rx) = mpsc::channel();
    let mut stalling = Stalling {
        inner: Scripted::always(Plan::Confirm),
        entered,
        release: release_rx,
        stalled: false,
    };
    let mut custody = lab.custody.take().expect("the custody is open");
    let cap = lab.cap.take().expect("the run's lease capability");
    let at = lab.now();
    let executor = thread::spawn(move || {
        let outcome = custody.end_case(&mut stalling, at);
        (custody, outcome)
    });
    entered_rx
        .recv_timeout(WATCHDOG)
        .expect("the cleanup attempt began");

    let control = lab.control.clone();
    let status = answered("status", move || control.status());
    assert_eq!(
        status.snapshot.busy,
        Some(BusyView {
            what: BusyWhat::Cleanup,
            since: at,
            entry: Some(entry)
        })
    );
    assert_eq!(status.snapshot.at, at);
    assert_eq!(
        only_entry(&status.snapshot).automatic_attempts,
        0,
        "a past observation, not a current fact"
    );
    let control = lab.control.clone();
    let renewal_at = lab.now();
    let (renewed, cap) = answered("renewal", move || {
        (control.renew_lease(&cap, renewal_at), cap)
    });
    assert_eq!(renewed, Ok(renewal_at.plus(TEST.lease_millis)));
    let control = lab.control.clone();
    let request = Request {
        generation: GENERATION,
        seq: 1,
        op: RequestOp::Shutdown,
    };
    assert!(answered("submission", move || control.submit(request)).is_ok());
    let control = lab.control.clone();
    let cancel_at = lab.now();
    let receipt = answered("cancellation", move || {
        control.cancel(CancelReason::Requested, cancel_at)
    });
    assert!(receipt.first);

    release.send(()).expect("the adapter is waiting");
    let (custody, outcome) = executor.join().expect("the execution owner");
    lab.custody = Some(custody);
    lab.cap = Some(cap);
    let outcome = outcome.expect("the case ended");
    assert!(outcome.resolved && outcome.stopped);
    assert_eq!(lab.released(), vec![token]);
    assert!(lab.control.status().snapshot.busy.is_none());
    let now = lab.now();
    let (served, _) = lab
        .c()
        .serve(&mut Scripted::always(Plan::Confirm), now)
        .expect("the request queued during the stall");
    assert_eq!(served, request);

    // A stalled recorder.
    let mut lab = Lab::new(TEST);
    let now = lab.now();
    let cap = lab.c().start_run(now).expect("the run starts");
    let (entered, entered_rx) = mpsc::channel();
    let (release, release_rx) = mpsc::channel();
    let mut sink = StallingSink {
        inner: Journal::default(),
        entered,
        release: release_rx,
        stalled: false,
    };
    let mut custody = lab.custody.take().expect("the custody is open");
    let at = lab.now();
    let executor = thread::spawn(move || {
        let sent = custody.flush_records(&mut sink, at);
        (custody, sink, sent)
    });
    entered_rx
        .recv_timeout(WATCHDOG)
        .expect("the submission began");
    let control = lab.control.clone();
    let status = answered("status", move || control.status());
    assert_eq!(
        status.snapshot.busy,
        Some(BusyView {
            what: BusyWhat::Record,
            since: at,
            entry: None
        })
    );
    assert_eq!(
        status.snapshot.evidence.unsent, 1,
        "the record being submitted is still unsent"
    );
    let control = lab.control.clone();
    let renewal_at = lab.now();
    let (renewed, _cap) = answered("renewal", move || {
        (control.renew_lease(&cap, renewal_at), cap)
    });
    assert!(renewed.is_ok());
    release.send(()).expect("the recorder is waiting");
    let (custody, sink, sent) = executor.join().expect("the execution owner");
    lab.custody = Some(custody);
    assert_eq!(sent, Ok(1));
    assert_eq!(sink.inner.submitted.len(), 1);

    // A stalled borrower.
    let mut lab = Lab::new(TEST);
    lab.start();
    lab.begin(1, Expectation::Clean);
    let (entry, token) = lab.create(SlotKind::Workspace);
    let (entered, entered_rx) = mpsc::channel();
    let (release, release_rx) = mpsc::channel::<()>();
    let mut custody = lab.custody.take().expect("the custody is open");
    let at = lab.now();
    let executor = thread::spawn(move || {
        let seen = custody.lend(entry, at, move |owner| {
            entered.send(()).expect("the control side is waiting");
            release_rx
                .recv_timeout(WATCHDOG)
                .expect("released by the control side");
            owner.token
        });
        (custody, seen)
    });
    entered_rx
        .recv_timeout(WATCHDOG)
        .expect("the borrower began");
    let control = lab.control.clone();
    let status = answered("status", move || control.status());
    assert_eq!(
        status.snapshot.busy,
        Some(BusyView {
            what: BusyWhat::Lend,
            since: at,
            entry: Some(entry)
        })
    );
    release.send(()).expect("the borrower is waiting");
    let (custody, seen) = executor.join().expect("the execution owner");
    lab.custody = Some(custody);
    assert_eq!(seen, Ok(token));
    assert!(lab.dropped().is_empty());
}

/// H9. An expected retained boundary (an injected negative control) passes
/// only when it was observed and its first cleanup attempt confirmed every
/// required fact; a confirmation by a later attempt does not satisfy it, and
/// the expected condition itself is never a failure.
#[test]
fn h09_expected_retained_boundary_needs_its_first_attempt_to_confirm() {
    let mut lab = Lab::new(TEST);
    lab.start();
    lab.begin(1, Expectation::RetainedBoundary);
    let (entry, token) = lab.deposit(SlotKind::Process, true);
    let snapshot = lab.snap();
    assert_eq!(
        snapshot.case.as_ref().map(|case| case.expected_observed),
        Some(true)
    );
    assert_eq!(
        snapshot.failure_counts, [0; 9],
        "the expected condition is not a failure"
    );
    let held = only_entry(&snapshot);
    assert_eq!(
        (held.entry, held.origin, held.phase),
        (entry, Origin::Retained, EntryPhase::CleanupRequired)
    );
    let outcome = lab.end_case(&mut Scripted::always(Plan::Confirm));
    assert_eq!(
        outcome,
        CaseOutcome {
            case: CaseId(1),
            passed: true,
            resolved: true,
            stopped: false
        }
    );
    assert_eq!(lab.released(), vec![token]);
    let now = lab.now();
    assert_eq!(lab.c().finish_run(now), Ok(RunPhase::Complete));
    assert_eq!(lab.snap().verdict, Verdict::Passed);

    for (first, why) in [
        (Plan::Fail, "unconfirmed"),
        (Plan::OutputPending, "output still pending"),
        (Plan::SubtreeOnly, "the child not reaped"),
        (Plan::Panic, "the adapter panicked"),
    ] {
        let mut lab = Lab::new(TEST);
        lab.start();
        lab.begin(1, Expectation::RetainedBoundary);
        let (_, token) = lab.deposit(SlotKind::Process, true);
        let mut cleanup = Scripted::always(Plan::Confirm).first(SlotKind::Process, &[first]);
        let outcome = lab.end_case(&mut cleanup);
        assert_eq!((outcome.passed, outcome.resolved), (false, true), "{why}");
        assert_eq!(cleanup.attempts_on(token), 2, "{why}");
        assert_eq!(lab.released(), vec![token], "{why}");
        let snapshot = lab.snap();
        assert_eq!(
            count(&snapshot, FailureClass::ExpectedConditionUnmet),
            1,
            "{why}"
        );
        assert_eq!(snapshot.cases_failed, 1, "{why}");
    }

    let mut lab = Lab::new(TEST);
    lab.start();
    lab.begin(1, Expectation::RetainedBoundary);
    lab.create(SlotKind::Process);
    let outcome = lab.end_case(&mut Scripted::always(Plan::Confirm));
    assert!(
        !outcome.passed && outcome.resolved,
        "an expected condition that did not occur fails"
    );
    assert_eq!(count(&lab.snap(), FailureClass::ExpectedConditionUnmet), 1);
}

/// H10. An unexpected failure stays failed after its cleanup succeeds by
/// explicit recovery.
#[test]
fn h10_unexpected_failure_stays_failed_after_recovery() {
    let mut lab = Lab::new(TEST);
    lab.start();
    lab.begin(1, Expectation::Clean);
    let (_, token) = lab.deposit(SlotKind::Process, true);
    assert_eq!(count(&lab.snap(), FailureClass::UnexpectedRetained), 1);
    let outcome = lab.end_case(&mut Scripted::always(Plan::Fail));
    assert!(!outcome.passed && !outcome.resolved);
    assert_eq!(
        lab.retry(&mut Scripted::always(Plan::Confirm)),
        Response::Retried {
            attempt: 1,
            resolved: true,
            epoch: 1
        }
    );
    let snapshot = lab.snap();
    assert_eq!(
        (
            count(&snapshot, FailureClass::UnexpectedRetained),
            count(&snapshot, FailureClass::UnexpectedCleanup),
            snapshot.failures.len(),
            snapshot.cases_failed
        ),
        (1, 1, 2, 1),
        "[failure-retained] a successful recovery cleared an earlier failure"
    );
    assert_eq!(
        (snapshot.phase, snapshot.verdict),
        (RunPhase::Complete, Verdict::Failed)
    );
    assert_eq!(lab.released(), vec![token]);
    lab.flush();
    assert_eq!(
        lab.records().last(),
        Some(&RecordKind::RunEnded {
            verdict: Verdict::Failed,
            resolved_by: Some(1)
        })
    );
}

/// H11. A workspace or fixture is released only after the native tree that
/// used it has ended natively (subtree gone and child reaped); a failed
/// removal stays unresolved and blocks completion.
#[test]
fn h11_dependencies_release_only_after_native_completion() {
    for (plan, why) in [
        (Plan::Fail, "nothing confirmed"),
        (Plan::SubtreeOnly, "the child is not reaped"),
        (Plan::ReapedOnly, "the subtree is not gone"),
    ] {
        let mut lab = Lab::new(TEST);
        lab.start();
        lab.begin(1, Expectation::Clean);
        let (workspace, workspace_token) = lab.create(SlotKind::Workspace);
        let (fixture, fixture_token) = lab.create(SlotKind::Fixture);
        let (_, process_token) = lab.create(SlotKind::Process);
        let mut cleanup = Scripted::always(Plan::Confirm).first(SlotKind::Process, &[plan; 3]);
        let outcome = lab.end_case(&mut cleanup);
        assert_eq!(
            cleanup.attempts_on(workspace_token) + cleanup.attempts_on(fixture_token),
            0,
            "[dependency-order] a dependency was released before native completion ({why})"
        );
        assert!(!outcome.resolved, "{why}");
        let snapshot = lab.snap();
        let held: Vec<EntryId> = snapshot.entries.iter().map(|view| view.entry).collect();
        assert!(
            held.contains(&workspace) && held.contains(&fixture),
            "{why}"
        );
        assert_eq!(
            lab.retry(&mut Scripted::always(Plan::Confirm)),
            Response::Retried {
                attempt: 1,
                resolved: true,
                epoch: 1
            }
        );
        assert_eq!(
            lab.released(),
            vec![process_token, workspace_token, fixture_token],
            "the tree first, then what it used ({why})"
        );
    }

    // A tree that ended natively releases its dependencies; its own entry
    // waits for its output.
    let mut lab = Lab::new(TEST);
    lab.start();
    lab.begin(1, Expectation::Clean);
    let (_, workspace_token) = lab.create(SlotKind::Workspace);
    let (process, _) = lab.create(SlotKind::Process);
    let mut cleanup =
        Scripted::always(Plan::Confirm).first(SlotKind::Process, &[Plan::OutputPending; 3]);
    let outcome = lab.end_case(&mut cleanup);
    assert!(!outcome.resolved);
    assert_eq!(lab.released(), vec![workspace_token]);
    let snapshot = lab.snap();
    let held = only_entry(&snapshot);
    assert_eq!(held.entry, process);
    assert!(held.subtree.confirmed() && held.reaped.confirmed());
    assert_eq!(
        held.output,
        OutputFact::Pending,
        "native completion alone does not finish the tree's entry"
    );

    // A failed removal stays unresolved and blocks completion.
    let mut lab = Lab::new(TEST);
    lab.start();
    lab.begin(1, Expectation::Clean);
    let (workspace, workspace_token) = lab.create(SlotKind::Workspace);
    let (_, process_token) = lab.create(SlotKind::Process);
    let mut cleanup = Scripted::always(Plan::Confirm).first(SlotKind::Workspace, &[Plan::Fail; 3]);
    let outcome = lab.end_case(&mut cleanup);
    assert!(!outcome.resolved);
    assert_eq!(lab.released(), vec![process_token]);
    let snapshot = lab.snap();
    assert_eq!(
        (snapshot.phase, only_entry(&snapshot).entry),
        (RunPhase::RecoveryRequired, workspace)
    );
    assert!(is_refused_with(
        &lab.c().shutdown_decision(),
        Unresolved::OwnersHeld(1)
    ));
    assert_eq!(
        lab.retry(&mut Scripted::always(Plan::Confirm)),
        Response::Retried {
            attempt: 1,
            resolved: true,
            epoch: 1
        }
    );
    assert_eq!(lab.released(), vec![workspace_token]);

    // Ending one entry: a dependency waits for the tree.
    let mut lab = Lab::new(TEST);
    lab.start();
    lab.begin(1, Expectation::Clean);
    let (workspace, workspace_token) = lab.create(SlotKind::Workspace);
    let (process, process_token) = lab.create(SlotKind::Process);
    let mut cleanup = Scripted::always(Plan::Confirm);
    let now = lab.now();
    assert_eq!(
        lab.c().end_entry(workspace, &mut cleanup, now),
        Err(Refusal::DependencyUnresolved)
    );
    assert!(cleanup.calls.is_empty());
    let now = lab.now();
    assert_eq!(lab.c().end_entry(process, &mut cleanup, now), Ok(true));
    let now = lab.now();
    assert_eq!(lab.c().end_entry(workspace, &mut cleanup, now), Ok(true));
    assert_eq!(lab.released(), vec![process_token, workspace_token]);
    let outcome = lab.end_case(&mut cleanup);
    assert!(outcome.passed && outcome.resolved);
}

/// H12. An unknown remote outcome blocks launch, discard and fresh admission:
/// it stays pending, with the dependencies it may use, until a completion
/// bound to its own ticket resolves it.
#[test]
fn h12_unknown_outcome_blocks_launch_discard_and_fresh_admission() {
    let mut lab = Lab::new(TEST);
    lab.start();
    lab.begin(1, Expectation::Clean);
    let (workspace, workspace_token) = lab.create(SlotKind::Workspace);
    let ticket = lab.admit(SlotKind::Process);
    let now = lab.now();
    let completion = lab.c().complete(&ticket, NativeOutcome::Unknown, now);
    assert!(
        matches!(completion, Ok(Completion::StillUnknown)),
        "[unknown-held] a timeout became an outcome: {completion:?}"
    );
    let now = lab.now();
    assert_eq!(
        lab.c().reserve(SlotKind::Fixture, now).unwrap_err(),
        Refusal::OperationPending,
        "no fresh admission"
    );
    assert!(
        is_refused_with(&lab.request_shutdown(), Unresolved::OutcomeUnknown),
        "no discard"
    );

    let outcome = lab.end_case(&mut Scripted::always(Plan::Confirm));
    assert_eq!(
        (outcome.passed, outcome.resolved, outcome.stopped),
        (false, false, true)
    );
    let snapshot = lab.snap();
    assert!(matches!(
        snapshot.operation.as_ref().map(|op| (op.action, op.phase)),
        Some((action, OpPhase::Unknown { .. })) if action == ticket.action()
    ));
    assert_eq!(
        only_entry(&snapshot).entry,
        workspace,
        "a dependency the unknown operation may use is kept"
    );
    assert_eq!(count(&snapshot, FailureClass::UnknownOutcome), 1);
    let now = lab.now();
    assert!(
        lab.c()
            .begin_case(CaseId(2), Expectation::Clean, now)
            .is_err(),
        "no next case"
    );

    // Retries, another generation's ticket and another operation's proof
    // resolve nothing.
    assert_eq!(
        lab.retry(&mut Scripted::always(Plan::Confirm)),
        Response::Retried {
            attempt: 1,
            resolved: false,
            epoch: 1
        }
    );
    let mut other = Lab::with(TEST, OTHER, Vec::new());
    other.start();
    other.begin(1, Expectation::Clean);
    let foreign = other.admit(SlotKind::Process);
    let now = lab.now();
    let proof = NoEffectProof::for_ticket(&foreign, "another generation");
    assert!(matches!(
        lab.c()
            .complete(&foreign, NativeOutcome::NoEffect(proof), now),
        Err(Rejected::Foreign { owner: None })
    ));
    let now = lab.now();
    let proof = NoEffectProof::for_ticket(&foreign, "another operation");
    assert!(matches!(
        lab.c()
            .complete(&ticket, NativeOutcome::NoEffect(proof), now),
        Err(Rejected::UnboundProof)
    ));
    assert!(matches!(
        lab.snap().operation.as_ref().map(|op| op.phase),
        Some(OpPhase::Unknown { .. })
    ));

    // A late error proving no effect resolves it; recovery then completes.
    let now = lab.now();
    let proof = NoEffectProof::for_ticket(&ticket, "refused before acting");
    assert_eq!(proof.reason(), "refused before acting");
    assert!(matches!(
        lab.c()
            .complete(&ticket, NativeOutcome::NoEffect(proof), now),
        Ok(Completion::NoEffect)
    ));
    assert_eq!(
        lab.retry(&mut Scripted::always(Plan::Confirm)),
        Response::Retried {
            attempt: 2,
            resolved: true,
            epoch: 2
        }
    );
    assert_eq!(lab.released(), vec![workspace_token]);
    assert_eq!(lab.snap().verdict, Verdict::Failed);
    assert!(
        lab.native
            .produced
            .iter()
            .all(|(_, action)| *action != ticket.action()),
        "nothing was launched for it"
    );
}

/// H13. Late, stale and duplicate results cannot act on another operation:
/// they resolve nothing, replace nothing and reopen nothing; an owner they
/// bring is held as unexpected (or returned past the bound), never dropped.
#[test]
fn h13_late_stale_and_duplicate_results_cannot_act_on_another_operation() {
    let mut lab = Lab::new(TEST);
    lab.start();
    lab.begin(1, Expectation::Clean);
    let first = lab.admit(SlotKind::Process);
    let now = lab.now();
    let proof = NoEffectProof::for_ticket(&first, "refused");
    assert!(matches!(
        lab.c()
            .complete(&first, NativeOutcome::NoEffect(proof), now),
        Ok(Completion::NoEffect)
    ));
    let second = lab.admit(SlotKind::Process);
    let in_flight = |lab: &Lab| {
        matches!(
            lab.snap().operation.as_ref(),
            Some(op) if op.action == second.action() && matches!(op.phase, OpPhase::InFlight { .. })
        )
    };

    let stale = [
        NativeOutcome::NoEffect(NoEffectProof::for_ticket(&first, "again")),
        NativeOutcome::Unknown,
        NativeOutcome::Ended(EndFacts {
            subtree: true,
            reaped: true,
            output: OutputObserved::Complete,
            removed: false,
        }),
    ];
    for outcome in stale {
        let now = lab.now();
        assert!(matches!(
            lab.c().complete(&first, outcome, now),
            Err(Rejected::Stale)
        ));
        assert!(in_flight(&lab), "a stale result acted on another operation");
    }
    let now = lab.now();
    let proof = NoEffectProof::for_ticket(&first, "the first operation's proof");
    assert!(matches!(
        lab.c()
            .complete(&second, NativeOutcome::NoEffect(proof), now),
        Err(Rejected::UnboundProof)
    ));
    assert!(in_flight(&lab));

    let late = lab.native.produce(&first);
    let late_token = late.token;
    let now = lab.now();
    assert!(matches!(
        lab.c().complete(&first, NativeOutcome::Created(late), now),
        Ok(Completion::Unexpected(_))
    ));
    assert!(
        in_flight(&lab),
        "a late owner is not put in another operation's place"
    );

    let owner = lab.native.produce(&second);
    let owner_token = owner.token;
    let now = lab.now();
    let Ok(Completion::Deposited(entry)) =
        lab.c()
            .complete(&second, NativeOutcome::Created(owner), now)
    else {
        panic!("the operation's own owner was not deposited");
    };
    let duplicate = lab.native.produce(&second);
    let duplicate_token = duplicate.token;
    let now = lab.now();
    assert!(matches!(
        lab.c()
            .complete(&second, NativeOutcome::Created(duplicate), now),
        Ok(Completion::Unexpected(_))
    ));
    let extra = lab.native.produce(&second);
    let extra_token = extra.token;
    let now = lab.now();
    match lab
        .c()
        .complete(&second, NativeOutcome::Created(extra), now)
    {
        Err(Rejected::CustodyFull { owner }) => {
            assert_eq!(owner.token, extra_token, "the same owner comes back");
            lab.returned.push(owner);
        }
        other => panic!("an owner past the bound must come back: {other:?}"),
    }
    assert!(lab.dropped().is_empty());
    let snapshot = lab.snap();
    let placed: Vec<EntryId> = snapshot
        .entries
        .iter()
        .filter(|view| view.origin == Origin::Created)
        .map(|view| view.entry)
        .collect();
    assert_eq!(placed, vec![entry], "the held owner was not replaced");
    assert_eq!(count(&snapshot, FailureClass::UnexpectedOwner), 2);

    let outcome = lab.end_case(&mut Scripted::always(Plan::Confirm));
    assert!(outcome.resolved && !outcome.passed);
    let mut released = lab.released();
    released.sort_unstable();
    assert_eq!(released, vec![late_token, owner_token, duplicate_token]);
    assert!(lab.dropped().is_empty());
}

/// H13 (evidence). Acknowledgements bind one exact record, in order:
/// conflicting, duplicate, out-of-order, foreign and never-issued ones are
/// rejected and change nothing, so the action cannot start.
#[test]
fn h13_acknowledgements_bind_exact_records_in_order() {
    let mut lab = Lab::new(TEST);
    lab.start();
    lab.begin(1, Expectation::Clean);
    let now = lab.now();
    let reservation = lab
        .c()
        .reserve(SlotKind::Process, now)
        .expect("the action is reserved");
    lab.flush();
    let [first, second, third] = [0, 1, 2].map(|index| lab.journal.submitted[index].clone());
    let now = lab.now();
    let outcomes = [
        lab.c().acknowledge(
            &RecordAck {
                id: second.id,
                digest: [0; 32],
            },
            now,
        ),
        lab.c().acknowledge(&RecordAck::of(&first), now),
        lab.c().acknowledge(
            &RecordAck {
                id: first.id,
                digest: [1; 32],
            },
            now,
        ),
        lab.c().acknowledge(&RecordAck::of(&third), now),
    ];
    assert_eq!(
        outcomes,
        [
            AckOutcome::Conflict,
            AckOutcome::Duplicate,
            AckOutcome::Conflict,
            AckOutcome::OutOfOrder
        ]
    );

    // Another generation's record, and one this custody never issued (from
    // another custody of the same generation).
    let mut other = Lab::with(TEST, OTHER, Vec::new());
    other.start();
    let mut twin = Lab::new(TEST);
    twin.start();
    twin.begin(1, Expectation::Clean);
    twin.create(SlotKind::Process);
    twin.end_case(&mut Scripted::always(Plan::Confirm));
    twin.flush();
    let foreign = RecordAck::of(&other.journal.submitted[0]);
    let unissued = RecordAck::of(&twin.journal.submitted[4]);
    let now = lab.now();
    assert_eq!(lab.c().acknowledge(&foreign, now), AckOutcome::Foreign);
    assert_eq!(lab.c().acknowledge(&unissued, now), AckOutcome::NotIssued);
    assert_eq!(
        lab.snap().evidence.acknowledged,
        1,
        "nothing was acknowledged"
    );

    let now = lab.now();
    let refused = lab.c().admit(reservation, now).unwrap_err();
    assert_eq!(refused.refusal, Refusal::EvidencePending);
    let reservation = refused
        .reservation
        .expect("a pending admission keeps its reservation");
    lab.ack_all();
    let now = lab.now();
    assert!(lab.c().admit(reservation, now).is_ok());
}

/// H14. Capacity is reserved before creation: an action whose start, failure
/// and settlement records cannot all be reserved is refused before anything
/// exists natively; an action starts only once its start record is durable;
/// and an admitted action can always record its worst case.
#[test]
fn h14_capacity_reservation_prevents_unrecordable_admission() {
    // The run (start, end, recovery-required and one record per recovery
    // attempt), a case (start, end) and an action (start, failure,
    // settlement), besides the control reserve.
    let needed = 3 + TEST.recovery_budget + 2 + 3;
    let mut lab = Lab::new(Config {
        record_capacity: TEST.control_reserve + needed - 1,
        ..TEST
    });
    lab.start();
    lab.begin(1, Expectation::Clean);
    let now = lab.now();
    let refusal = lab.c().reserve(SlotKind::Process, now).err();
    assert_eq!(
        refusal,
        Some(Refusal::EvidenceCapacity),
        "[capacity-reserved] an action was reserved without room for all its records"
    );
    assert!(lab.native.produced.is_empty() && lab.snap().operation.is_none());

    let mut lab = Lab::new(Config {
        record_capacity: TEST.control_reserve + needed,
        ..TEST
    });
    lab.start();
    lab.begin(1, Expectation::Clean);
    let now = lab.now();
    let reservation = lab
        .c()
        .reserve(SlotKind::Process, now)
        .expect("exactly enough capacity");
    let now = lab.now();
    let refused = match lab.c().admit(reservation, now) {
        Err(refused) => refused,
        Ok(_) => panic!(
            "[start-acknowledged] an action was admitted before its start record was durable"
        ),
    };
    assert_eq!(
        refused.refusal,
        Refusal::EvidencePending,
        "[start-acknowledged] the refusal names the pending start record"
    );
    assert!(lab.native.produced.is_empty());
    let reservation = refused.reservation.expect("the reservation comes back");
    lab.ack_all();
    let now = lab.now();
    let ticket = lab
        .c()
        .admit(reservation, now)
        .expect("admitted once its start is durable");
    let evidence = lab.snap().evidence.clone();
    assert_eq!(
        evidence.issued + u64::from(evidence.reserved),
        u64::from(needed),
        "everything the action may need is reserved"
    );

    // Its worst case: unconfirmed cleanup, every recovery attempt, the end.
    let owner = lab.native.produce(&ticket);
    let token = owner.token;
    let now = lab.now();
    assert!(matches!(
        lab.c()
            .complete(&ticket, NativeOutcome::Created(owner), now),
        Ok(Completion::Deposited(_))
    ));
    lab.end_case(&mut Scripted::always(Plan::Fail));
    for attempt in 1..TEST.recovery_budget {
        assert_eq!(
            lab.retry(&mut Scripted::always(Plan::Fail)),
            Response::Retried {
                attempt,
                resolved: false,
                epoch: u64::from(attempt)
            }
        );
    }
    let last = TEST.recovery_budget;
    assert_eq!(
        lab.retry(&mut Scripted::always(Plan::Confirm)),
        Response::Retried {
            attempt: last,
            resolved: true,
            epoch: u64::from(last)
        }
    );
    assert_eq!(lab.released(), vec![token]);
    let evidence = lab.snap().evidence.clone();
    assert_eq!(
        (evidence.issued, evidence.reserved),
        (u64::from(needed), 0),
        "every record was recorded within capacity"
    );
    lab.flush();
    assert_eq!(
        lab.records().last(),
        Some(&RecordKind::RunEnded {
            verdict: Verdict::Failed,
            resolved_by: Some(last)
        })
    );
}

/// H15. Physical cleanup and recording failure stay distinct: the owner's
/// end stays confirmed and the owner is not recreated, the settlement stays
/// not durable, admission stays closed and completion is never claimed
/// durable.
#[test]
fn h15_physical_cleanup_and_recording_failure_stay_distinct() {
    let mut lab = Lab::new(TEST);
    lab.start();
    lab.begin(1, Expectation::Clean);
    let (entry, token) = lab.create(SlotKind::Process);
    let outcome = lab.end_case(&mut Scripted::always(Plan::Confirm));
    assert!(outcome.passed && outcome.resolved && !outcome.stopped);
    assert_eq!(lab.released(), vec![token], "physically ended and released");

    lab.flush();
    let settled = lab
        .journal
        .submitted
        .iter()
        .position(|intent| matches!(intent.kind, RecordKind::ActionSettled { .. }))
        .expect("the settlement was issued");
    lab.ack_until(settled);
    let settlement = lab.journal.submitted[settled].clone();
    let now = lab.now();
    assert_eq!(
        lab.c().record_failed(settlement.id, now),
        AckOutcome::FailureRecorded
    );

    let snapshot = lab.snap();
    assert!(
        snapshot.entries.is_empty(),
        "the physical end stays confirmed"
    );
    let settling: Vec<_> = snapshot
        .settling
        .iter()
        .map(|view| (view.entry, view.record))
        .collect();
    assert_eq!(
        settling,
        vec![(Some(entry), settlement.id)],
        "the settlement is not durable"
    );
    assert_eq!(
        snapshot.evidence.failed.map(|(id, _)| id),
        Some(settlement.id)
    );
    assert_eq!(count(&snapshot, FailureClass::RecordFailed), 1);
    assert_eq!(
        (snapshot.phase, snapshot.verdict),
        (RunPhase::Complete, Verdict::Failed)
    );
    let closure = lab
        .control
        .status()
        .control
        .admission
        .closure
        .expect("a recording failure closes admission");
    assert_eq!(closure.reason, CancelReason::RecordFailed);
    let now = lab.now();
    assert!(matches!(
        lab.c().begin_case(CaseId(2), Expectation::Clean, now),
        Err(Refusal::AdmissionClosed(_))
    ));
    let now = lab.now();
    assert_eq!(
        lab.c().acknowledge(&RecordAck::of(&settlement), now),
        AckOutcome::LedgerFailed,
        "nothing makes it durable later"
    );
    assert_eq!(
        lab.c().shutdown_decision(),
        ShutdownDecision::PermittedWithoutDurableCompletion
    );
    let closed = lab.close().expect("nothing native is unresolved");
    assert!(!closed.durable);
    assert_eq!(closed.verdict, Verdict::Failed);
    assert!(closed.released.is_empty());
    assert_eq!(lab.native.produced.len(), 1, "the owner was not recreated");
    assert!(lab.dropped().is_empty());
}

/// H16. Refused deposits and admissions return or retain ownership: another
/// generation's ticket or an earlier instant returns the same owner; an owner
/// of another kind, or one whose own declaration panics, is held as
/// unexpected; an admission refusal returns or settles its reservation.
#[test]
fn h16_refused_deposits_and_admissions_return_or_retain_ownership() {
    let mut lab = Lab::new(TEST);
    lab.start();
    lab.begin(1, Expectation::Clean);
    let ticket = lab.admit(SlotKind::Process);

    let mut other = Lab::with(TEST, OTHER, Vec::new());
    other.start();
    other.begin(1, Expectation::Clean);
    let foreign = other.admit(SlotKind::Process);
    let owner = lab.native.produce(&foreign);
    let foreign_token = owner.token;
    let now = lab.now();
    match lab
        .c()
        .complete(&foreign, NativeOutcome::Created(owner), now)
    {
        Err(rejected @ Rejected::Foreign { .. }) => {
            let owner = rejected.into_owner().expect("the owner comes back");
            assert_eq!(owner.token, foreign_token);
            lab.returned.push(owner);
        }
        other => panic!("another generation's ticket was accepted: {other:?}"),
    }

    let owner = lab.native.produce(&ticket);
    let token = owner.token;
    let owner = match lab
        .c()
        .complete(&ticket, NativeOutcome::Created(owner), Tick(0))
    {
        Err(Rejected::ClockRegression { owner: Some(owner) }) => owner,
        other => panic!("an earlier instant was accepted: {other:?}"),
    };
    assert_eq!(owner.token, token);
    assert!(
        matches!(
            lab.snap().operation.as_ref().map(|op| op.phase),
            Some(OpPhase::InFlight { .. })
        ),
        "the operation is still open"
    );
    let now = lab.now();
    assert!(matches!(
        lab.c()
            .complete(&ticket, NativeOutcome::Created(owner), now),
        Ok(Completion::Deposited(_))
    ));

    // An owner whose own declaration panics is held, as a process tree.
    let workspace = lab.admit(SlotKind::Workspace);
    let undeclared = lab.native.produce_as(&workspace, None);
    let undeclared_token = undeclared.token;
    let now = lab.now();
    let Ok(Completion::Unexpected(held)) =
        lab.c()
            .complete(&workspace, NativeOutcome::Created(undeclared), now)
    else {
        panic!("an owner whose declaration panicked was not held");
    };
    let snapshot = lab.snap();
    let view = snapshot
        .entries
        .iter()
        .find(|view| view.entry == held)
        .expect("held");
    assert_eq!(
        (view.kind, view.origin),
        (SlotKind::Process, Origin::Unexpected)
    );

    // Another custody's reservation is returned unused.
    let mut twin = Lab::new(TEST);
    twin.start();
    twin.begin(1, Expectation::Clean);
    let now = twin.now();
    let reservation = twin
        .c()
        .reserve(SlotKind::Fixture, now)
        .expect("the action is reserved");
    let now = lab.now();
    let refused = lab.c().admit(reservation, now).unwrap_err();
    assert_eq!(refused.refusal, Refusal::NotReserved);
    assert!(refused.reservation.is_some(), "the reservation comes back");
    assert!(lab.dropped().is_empty());

    let outcome = lab.end_case(&mut Scripted::always(Plan::Confirm));
    assert!(outcome.resolved && !outcome.passed);
    let mut released = lab.released();
    released.sort_unstable();
    assert_eq!(released, vec![token, undeclared_token]);

    // An owner of another kind is held as unexpected too.
    let mut lab = Lab::new(TEST);
    lab.start();
    lab.begin(1, Expectation::Clean);
    let workspace = lab.admit(SlotKind::Workspace);
    let wrong = lab.native.produce_as(&workspace, Some(SlotKind::Fixture));
    let wrong_token = wrong.token;
    let now = lab.now();
    assert!(matches!(
        lab.c()
            .complete(&workspace, NativeOutcome::Created(wrong), now),
        Ok(Completion::Unexpected(_))
    ));
    let snapshot = lab.snap();
    assert_eq!(
        (only_entry(&snapshot).kind, only_entry(&snapshot).origin),
        (SlotKind::Fixture, Origin::Unexpected)
    );
    assert!(snapshot.operation.is_none());
    let outcome = lab.end_case(&mut Scripted::always(Plan::Confirm));
    assert!(outcome.resolved && !outcome.passed);
    assert_eq!(lab.released(), vec![wrong_token]);
    assert!(lab.dropped().is_empty());
}

/// H17. No normal API path exits, aborts, leaks or drops unresolved
/// ownership: the core has no such construct, an entry leaves its group only
/// through release, and every public operation on a custody holding an
/// unresolved owner and an unknown operation leaves both where they are.
#[test]
fn h17_no_api_path_exits_aborts_leaks_or_drops_unresolved_ownership() {
    let sources = [
        ("core.rs", include_str!("support/custody/core.rs")),
        ("model.rs", include_str!("support/custody/model.rs")),
        ("mod.rs", include_str!("support/custody/mod.rs")),
    ];
    let forbidden = [
        "process::",
        "abort(",
        "exit(",
        "mem::forget",
        "Box::leak",
        "ManuallyDrop",
        "into_raw",
        "impl Drop",
        "Drop for",
        "static mut",
        "OnceLock",
        "LazyLock",
        "thread_local!",
        "unsafe",
        "Rc<",
    ];
    for (name, source) in sources {
        for pattern in forbidden {
            assert!(!source.contains(pattern), "{name} contains {pattern:?}");
        }
        for line in source.lines() {
            let code = line.trim_start();
            assert!(
                !code.starts_with("static ") && !code.starts_with("pub static "),
                "{name} declares a global: {line}"
            );
        }
    }
    let core = sources[0].1;
    assert_eq!(
        core.matches("group.take(").count(),
        1,
        "one path takes an entry out of its group"
    );
    let release = core.find("fn finish(").expect("the release path");
    let release_end = release + core[release..].find("\n    }\n").expect("its end");
    let take = core.find("group.take(").expect("the take");
    assert!(
        release < take && take < release_end,
        "an entry leaves its group only through release"
    );

    let mut lab = Lab::new(TEST);
    lab.start();
    lab.begin(1, Expectation::Clean);
    let (entry, token) = lab.create(SlotKind::Process);
    let ticket = lab.admit(SlotKind::Workspace);
    let now = lab.now();
    assert!(matches!(
        lab.c().complete(&ticket, NativeOutcome::Unknown, now),
        Ok(Completion::StillUnknown)
    ));
    let mut failing = Scripted::always(Plan::Fail);
    lab.end_case(&mut failing);
    let held = |lab: &Lab, step: &str| {
        let snapshot = lab.snap();
        assert!(lab.dropped().is_empty(), "{step} let go of an owner");
        let entries: Vec<EntryId> = snapshot.entries.iter().map(|view| view.entry).collect();
        assert_eq!(entries, vec![entry], "{step}");
        assert!(
            matches!(
                snapshot.operation.as_ref().map(|op| op.phase),
                Some(OpPhase::Unknown { .. })
            ),
            "{step}"
        );
    };
    held(&lab, "ending the case");

    let _ = lab.control.status();
    let _ = lab.c().snapshot();
    let _ = lab.c().shutdown_decision();
    held(&lab, "reporting");
    assert!(lab.close().is_none());
    held(&lab, "a refused close");
    let now = lab.now();
    assert_eq!(lab.c().lend(entry, now, |owner| owner.token), Ok(token));
    held(&lab, "lending");
    let now = lab.now();
    lab.c()
        .record_assertion_failure("an observation failed", now)
        .expect("recorded");
    let now = lab.now();
    assert_eq!(lab.c().observe_control(now), Ok(None));
    let now = lab.now();
    assert!(lab.c().finish_run(now).is_err());
    let now = lab.now();
    assert!(lab
        .c()
        .begin_case(CaseId(2), Expectation::Clean, now)
        .is_err());
    let now = lab.now();
    assert!(lab.c().reserve(SlotKind::Fixture, now).is_err());
    let now = lab.now();
    assert!(lab.c().end_entry(entry, &mut failing, now).is_err());
    let now = lab.now();
    assert!(lab.c().end_case(&mut failing, now).is_err());
    held(&lab, "refused operations");
    assert!(matches!(
        lab.retry(&mut failing),
        Response::Retried {
            resolved: false,
            ..
        }
    ));
    held(&lab, "a failed recovery attempt");
    let now = lab.now();
    assert!(matches!(
        lab.c().flush_records(&mut PanickingSink, now),
        Err(FlushError::SinkPanicked { sent: 0 })
    ));
    held(&lab, "a panicking recorder");
    assert!(matches!(
        lab.request_shutdown(),
        ShutdownDecision::Refused(_)
    ));
    held(&lab, "a shutdown request");
    lab.ack_all();
    assert!(lab.released().is_empty());
    let now = lab.now();
    let silent = Validator {
        answers: Vec::new(),
        panics: false,
    };
    assert!(lab
        .c()
        .apply_disposition(&IncidentBinding::new([9; 32]), &silent, now)
        .is_err());
    held(&lab, "acknowledgements, release and disposition");
    let now = lab.now();
    lab.control.cancel(CancelReason::Stop, now);
    let cap = lab.cap.take().expect("the run's lease capability");
    let now = lab.now();
    let _ = lab.control.renew_lease(&cap, now);
    let _ = lab.control.check_lease(Tick(u64::MAX));
    held(&lab, "control operations");
    let past = Tick(0);
    assert_eq!(
        lab.c().lend(entry, past, |owner| owner.token),
        Err(LendError::ClockRegression)
    );
    assert!(lab.c().end_case(&mut failing, past).is_err());
    assert!(lab.c().observe_control(past).is_err());
    assert!(matches!(
        lab.c().complete(&ticket, NativeOutcome::Unknown, past),
        Err(Rejected::ClockRegression { owner: None })
    ));
    held(&lab, "earlier instants");
    let now = lab.now();
    assert!(lab.c().observe_control(now).is_ok());
    lab.flush();
    let next = lab.journal.submitted[lab.acked].id;
    let now = lab.now();
    assert_eq!(
        lab.c().record_failed(next, now),
        AckOutcome::FailureRecorded
    );
    held(&lab, "a recording failure");
}

/// The adversarial events: every way the control side, the native
/// stand-in, the recorder and the case can act next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Event {
    Admit,
    Cancel,
    Timeout,
    Late,
    LateNone,
    EndFail,
    EndOk,
    StaleAck,
    RecordFail,
    RetryOk,
    RetryFail,
    AckAll,
    Close,
}

const EVENTS: [Event; 13] = [
    Event::Admit,
    Event::Cancel,
    Event::Timeout,
    Event::Late,
    Event::LateNone,
    Event::EndFail,
    Event::EndOk,
    Event::StaleAck,
    Event::RecordFail,
    Event::RetryOk,
    Event::RetryFail,
    Event::AckAll,
    Event::Close,
];

const ADVERSARIAL: Config = Config {
    automatic_attempts: 2,
    recovery_budget: 3,
    recovery_spacing_millis: 10,
    lease_millis: 1_000_000,
    record_capacity: 48,
    control_reserve: 2,
    receipt_limit: 2,
    unexpected_limit: 1,
    failure_detail_limit: 4,
    detail_chars: 32,
    incident_limit: 0,
};

/// States a sequence reached (to prove the sequences exercise them).
#[derive(Debug, Default, Clone, Copy)]
struct Coverage {
    adopted_after_cancel: bool,
    unknown_resolved: bool,
    stray_held: bool,
    stray_returned: bool,
    record_failed: bool,
    recovered: bool,
    reopened: bool,
    closed: bool,
}

/// One adversarial run: a lab, every ticket it issued, and what was last
/// observed (for the monotonic invariants).
struct Adversary {
    lab: Lab,
    tickets: Vec<OpTicket>,
    closed: Option<Closed<Witness>>,
    cases: u32,
    last: Arc<Snapshot>,
    last_control: ControlView,
    trail: Vec<Event>,
    coverage: Coverage,
}

impl Adversary {
    fn run(events: &[Event]) -> Coverage {
        let mut lab = Lab::new(ADVERSARIAL);
        lab.start();
        let status = lab.control.status();
        let mut adversary = Self {
            lab,
            tickets: Vec::new(),
            closed: None,
            cases: 0,
            last: status.snapshot,
            last_control: status.control,
            trail: Vec::new(),
            coverage: Coverage::default(),
        };
        for event in events {
            adversary.apply(*event);
        }
        adversary.coverage
    }

    fn apply(&mut self, event: Event) {
        self.trail.push(event);
        if self.closed.is_some() {
            return;
        }
        let open = self
            .last
            .operation
            .as_ref()
            .filter(|op| !matches!(op.phase, OpPhase::Reserved))
            .map(|op| op.action);
        let bound = open.is_some() && self.tickets.last().map(OpTicket::action) == open;
        let resolving = bound && matches!(event, Event::Late | Event::LateNone);
        match event {
            Event::Admit => self.admit(),
            Event::Cancel => {
                let now = self.lab.now();
                self.lab.control.cancel(CancelReason::Requested, now);
            }
            Event::Timeout => self.complete(|_, _| NativeOutcome::Unknown),
            Event::Late => {
                self.complete(|native, ticket| NativeOutcome::Created(native.produce(ticket)))
            }
            Event::LateNone => self.complete(|_, ticket| {
                NativeOutcome::NoEffect(NoEffectProof::for_ticket(ticket, "late refusal"))
            }),
            Event::EndFail => self.end(Plan::Fail),
            Event::EndOk => self.end(Plan::Confirm),
            Event::StaleAck => self.stale_ack(),
            Event::RecordFail => self.record_fail(),
            Event::RetryOk => self.retry(Plan::Confirm),
            Event::RetryFail => self.retry(Plan::Fail),
            Event::AckAll => self.lab.try_ack_all(),
            Event::Close => self.close(),
        }
        if self.closed.is_none() {
            self.lab.released();
        }
        self.check(event, resolving, open);
    }

    fn admit(&mut self) {
        let kinds = [SlotKind::Process, SlotKind::Workspace, SlotKind::Fixture];
        let kind = kinds[self.tickets.len() % kinds.len()];
        if self.lab.snap().case.is_none() {
            self.cases += 1;
            let now = self.lab.now();
            let case = CaseId(self.cases);
            if self
                .lab
                .c()
                .begin_case(case, Expectation::Clean, now)
                .is_err()
            {
                return;
            }
        }
        let now = self.lab.now();
        let Ok(reservation) = self.lab.c().reserve(kind, now) else {
            return;
        };
        self.lab.try_ack_all();
        let now = self.lab.now();
        if let Ok(ticket) = self.lab.c().admit(reservation, now) {
            self.tickets.push(ticket);
        }
    }

    fn complete(&mut self, make: impl FnOnce(&mut Native, &OpTicket) -> NativeOutcome<Witness>) {
        let Some(ticket) = self.tickets.last() else {
            return;
        };
        let outcome = make(&mut self.lab.native, ticket);
        let now = self.lab.now();
        let custody = self.lab.custody.as_mut().expect("the custody is open");
        if let Err(rejected) = custody.complete(ticket, outcome, now) {
            if matches!(rejected, Rejected::CustodyFull { .. }) {
                self.coverage.stray_returned = true;
            }
            if let Some(owner) = rejected.into_owner() {
                self.lab.returned.push(owner);
            }
        }
    }

    fn end(&mut self, plan: Plan) {
        let mut cleanup = Scripted::always(plan);
        let now = self.lab.now();
        if self.lab.snap().case.is_some() {
            self.lab
                .c()
                .end_case(&mut cleanup, now)
                .expect("an active case ends");
        } else {
            let _ = self.lab.c().finish_run(now);
        }
    }

    fn stale_ack(&mut self) {
        self.lab.flush();
        let acknowledged = self.lab.snap().evidence.acknowledged;
        let submitted = &self.lab.journal.submitted;
        let mut acks: Vec<RecordAck> = [
            submitted.first(),
            submitted.get(acknowledged as usize),
            submitted.last(),
        ]
        .into_iter()
        .flatten()
        .map(|intent| {
            let mut digest = intent.digest;
            digest[0] ^= 0xff;
            RecordAck {
                id: intent.id,
                digest,
            }
        })
        .collect();
        acks.extend(submitted.first().map(RecordAck::of));
        for ack in acks {
            let now = self.lab.now();
            let outcome = self.lab.c().acknowledge(&ack, now);
            assert_ne!(
                outcome,
                AckOutcome::Acknowledged,
                "{:?}: a stale or conflicting acknowledgement was accepted",
                self.trail
            );
        }
        assert_eq!(self.lab.snap().evidence.acknowledged, acknowledged);
    }

    fn record_fail(&mut self) {
        self.lab.flush();
        let next = self.lab.snap().evidence.acknowledged as usize;
        if let Some(id) = self.lab.journal.submitted.get(next).map(|intent| intent.id) {
            let now = self.lab.now();
            let _ = self.lab.c().record_failed(id, now);
        }
    }

    fn retry(&mut self, plan: Plan) {
        let mut cleanup = Scripted::always(plan);
        self.lab.seq += 1;
        let request = Request {
            generation: GENERATION,
            seq: self.lab.seq,
            op: RequestOp::Retry {
                epoch: self.lab.snap().recovery.epoch,
            },
        };
        let now = self.lab.later(ADVERSARIAL.recovery_spacing_millis);
        let outcome = self.lab.serve(request, &mut cleanup, now);
        assert!(
            matches!(outcome, RequestOutcome::Executed(_)),
            "{:?}: a fresh request was not executed: {outcome:?}",
            self.trail
        );
    }

    fn close(&mut self) {
        let refused = matches!(
            self.lab.c().shutdown_decision(),
            ShutdownDecision::Refused(_)
        );
        match self.lab.close() {
            Some(closed) => {
                assert!(!refused, "{:?}: closed while refused", self.trail);
                self.closed = Some(closed);
                self.coverage.closed = true;
            }
            None => assert!(refused, "{:?}: not closed while permitted", self.trail),
        }
    }

    fn check(&mut self, event: Event, resolving: bool, open: Option<ActionId>) {
        let trail = &self.trail;
        let status = self.lab.control.status();
        let now = &status.snapshot;
        let before = &self.last;
        assert!(
            self.lab.dropped().is_empty(),
            "{trail:?}: custody let go of an owner"
        );

        // Every produced owner is held, waiting, or back with the caller.
        let produced = self.lab.native.produced.len();
        let returned = self.lab.returned.len();
        match &self.closed {
            Some(closed) => assert_eq!(
                produced,
                returned + closed.released.len(),
                "{trail:?}: owners not conserved at close"
            ),
            None => assert_eq!(
                produced,
                returned + now.entries.len() + now.released_waiting,
                "{trail:?}: owners not conserved"
            ),
        }

        // Admission: one ticket per admission, none after the closure, and
        // the closure never changes.
        let control = status.control;
        if let Some(closure) = self.last_control.admission.closure {
            assert_eq!(
                control.admission.closure,
                Some(closure),
                "{trail:?}: the closure changed"
            );
            assert_eq!(
                control.admission.admitted, self.last_control.admission.admitted,
                "{trail:?}: admitted after the closure"
            );
        }
        if let Some(closure) = control.admission.closure {
            assert!(closure.after <= control.admission.admitted);
        }
        assert_eq!(
            control.admission.admitted,
            self.tickets.len() as u64,
            "{trail:?}: an admission without exactly one ticket"
        );

        // Failures and the verdict only accumulate.
        if before.verdict == Verdict::Failed {
            assert_eq!(
                now.verdict,
                Verdict::Failed,
                "{trail:?}: a failed verdict changed"
            );
        }
        assert!(
            now.failure_counts
                .iter()
                .zip(before.failure_counts.iter())
                .all(|(now, before)| now >= before),
            "{trail:?}: a failure count fell"
        );
        assert!(
            now.failures.starts_with(&before.failures),
            "{trail:?}: a recorded failure changed"
        );

        // Evidence stays within capacity, acknowledgements only advance, and
        // a recording failure is permanent.
        let evidence = &now.evidence;
        assert!(evidence.acknowledged <= evidence.issued);
        assert!(evidence.acknowledged >= before.evidence.acknowledged);
        assert!(evidence.issued >= before.evidence.issued);
        let control_used = u64::from(ADVERSARIAL.control_reserve - evidence.control_left);
        let general = evidence.issued - control_used + u64::from(evidence.reserved);
        assert!(
            general <= u64::from(ADVERSARIAL.record_capacity - ADVERSARIAL.control_reserve),
            "{trail:?}: evidence beyond capacity"
        );
        if before.evidence.failed.is_some() {
            assert_eq!(
                evidence.failed, before.evidence.failed,
                "{trail:?}: a recording failure changed"
            );
        }

        // Phases move only forward, except that an owner arriving after
        // completion requires recovery again.
        let legal = before.phase == now.phase
            || matches!(
                (before.phase, now.phase),
                (
                    RunPhase::Running,
                    RunPhase::RecoveryRequired | RunPhase::Complete
                ) | (RunPhase::RecoveryRequired, RunPhase::Complete)
                    | (
                        RunPhase::Complete,
                        RunPhase::RecoveryRequired | RunPhase::Closed
                    )
            );
        assert!(legal, "{trail:?}: {:?} -> {:?}", before.phase, now.phase);

        // An admitted operation resolves only by its own bound completion.
        if let Some(action) = open {
            let still = now
                .operation
                .as_ref()
                .is_some_and(|after| after.action == action);
            assert!(
                still || resolving,
                "{trail:?}: an admitted operation resolved without its bound completion"
            );
        }

        // A held entry keeps its identity; attempts stay within budgets.
        for entry in &now.entries {
            if let Some(previous) = before
                .entries
                .iter()
                .find(|previous| previous.entry == entry.entry)
            {
                assert_eq!(
                    (
                        previous.action,
                        previous.kind,
                        previous.origin,
                        previous.since
                    ),
                    (entry.action, entry.kind, entry.origin, entry.since),
                    "{trail:?}: a held entry changed identity"
                );
                assert!(entry.automatic_attempts >= previous.automatic_attempts);
                assert!(entry.explicit_attempts >= previous.explicit_attempts);
            }
            assert!(entry.automatic_attempts <= ADVERSARIAL.automatic_attempts);
            assert!(entry.explicit_attempts <= ADVERSARIAL.recovery_budget);
        }
        assert!(now.recovery.attempts <= ADVERSARIAL.recovery_budget);

        // Nothing unresolved may shut down.
        if self.closed.is_none()
            && (!now.entries.is_empty() || now.operation.is_some() || !now.lost.is_empty())
        {
            assert!(
                matches!(
                    self.lab.c().shutdown_decision(),
                    ShutdownDecision::Refused(_)
                ),
                "{trail:?}: shutdown permitted while unresolved"
            );
        }
        assert!(now.published >= before.published);

        let coverage = &mut self.coverage;
        if event == Event::Late
            && resolving
            && control.admission.closure.is_some()
            && now
                .entries
                .iter()
                .any(|entry| Some(entry.action) == open && entry.origin == Origin::Created)
        {
            coverage.adopted_after_cancel = true;
        }
        let was_unknown = before
            .operation
            .as_ref()
            .is_some_and(|op| matches!(op.phase, OpPhase::Unknown { .. }));
        if was_unknown && resolving && now.operation.is_none() {
            coverage.unknown_resolved = true;
        }
        if now
            .entries
            .iter()
            .any(|entry| entry.origin == Origin::Unexpected)
        {
            coverage.stray_held = true;
        }
        if evidence.failed.is_some() {
            coverage.record_failed = true;
        }
        if now.recovery.resolved_by.is_some() {
            coverage.recovered = true;
        }
        if before.phase == RunPhase::Complete && now.phase == RunPhase::RecoveryRequired {
            coverage.reopened = true;
        }
        self.last = Arc::clone(&status.snapshot);
        self.last_control = control;
    }
}

/// A witness sequence, and the state it must reach.
type Witnessed = (&'static [Event], fn(&Coverage) -> bool);

/// The finite adversarial transition-sequence control: every sequence of
/// four events (admit, cancel, timeout, late result, late refusal, failing
/// and confirming case ends, stale acknowledgement, record failure, failing
/// and confirming retries, acknowledgement and close), a deterministic sample
/// of longer ones, and witness sequences proving each guarded state is
/// reached; after every event, no owner was dropped and every invariant
/// holds.
#[test]
fn h18_adversarial_transition_sequences_keep_every_invariant() {
    let witnesses: [Witnessed; 8] = [
        (
            &[Event::Admit, Event::Cancel, Event::Late, Event::EndOk],
            |coverage| coverage.adopted_after_cancel,
        ),
        (
            &[Event::Admit, Event::Timeout, Event::LateNone],
            |coverage| coverage.unknown_resolved,
        ),
        (&[Event::Admit, Event::LateNone, Event::Late], |coverage| {
            coverage.stray_held
        }),
        (
            &[Event::Admit, Event::LateNone, Event::Late, Event::Late],
            |coverage| coverage.stray_returned,
        ),
        (
            &[Event::Admit, Event::Late, Event::EndOk, Event::RecordFail],
            |coverage| coverage.record_failed,
        ),
        (
            &[Event::Admit, Event::Late, Event::EndFail, Event::RetryOk],
            |coverage| coverage.recovered,
        ),
        (
            &[
                Event::Admit,
                Event::LateNone,
                Event::EndOk,
                Event::EndOk,
                Event::Late,
            ],
            |coverage| coverage.reopened,
        ),
        (
            &[
                Event::Admit,
                Event::Late,
                Event::EndOk,
                Event::EndOk,
                Event::AckAll,
                Event::Close,
            ],
            |coverage| coverage.closed,
        ),
    ];
    for (events, reached) in witnesses {
        assert!(
            reached(&Adversary::run(events)),
            "{events:?} did not reach its state"
        );
    }

    let mut sequences = 0_usize;
    for a in EVENTS {
        for b in EVENTS {
            for c in EVENTS {
                for d in EVENTS {
                    Adversary::run(&[a, b, c, d]);
                    sequences += 1;
                }
            }
        }
    }
    assert_eq!(sequences, EVENTS.len().pow(4));

    let mut state = 0x5eed_c0de_u64;
    for _ in 0..4_000 {
        let mut events = [Event::Admit; 8];
        for event in &mut events {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            *event = EVENTS[(state >> 33) as usize % EVENTS.len()];
        }
        Adversary::run(&events);
    }
}

/// Requests are bound to the generation and to strictly increasing sequence
/// numbers: an exact repeat returns its first response, a conflicting
/// payload, a gap or another generation executes nothing, and an old number
/// whose receipt was evicted is never executed again, even when it could now
/// succeed. A full queue returns the request.
#[test]
fn h19_replayed_evicted_conflicting_and_foreign_requests_execute_nothing() {
    let mut lab = Lab::new(TEST);
    lab.start();
    lab.begin(1, Expectation::Clean);
    let (_, token) = lab.create(SlotKind::Process);
    let mut cleanup = Scripted::always(Plan::Fail);
    lab.end_case(&mut cleanup);
    let retry = |seq, epoch| Request {
        generation: GENERATION,
        seq,
        op: RequestOp::Retry { epoch },
    };

    let now = lab.later(TEST.recovery_spacing_millis);
    assert_eq!(
        lab.serve(retry(1, 0), &mut cleanup, now),
        RequestOutcome::Executed(Response::Retried {
            attempt: 1,
            resolved: false,
            epoch: 1
        })
    );
    let attempts = cleanup.calls.len();
    let now = lab.now();
    assert_eq!(
        lab.serve(retry(1, 0), &mut cleanup, now),
        RequestOutcome::Duplicate(Response::Retried {
            attempt: 1,
            resolved: false,
            epoch: 1
        })
    );
    let now = lab.now();
    assert_eq!(
        lab.serve(retry(1, 1), &mut cleanup, now),
        RequestOutcome::Conflict
    );
    let now = lab.now();
    assert_eq!(
        lab.serve(retry(3, 1), &mut cleanup, now),
        RequestOutcome::Gap
    );
    let now = lab.now();
    let foreign = Request {
        generation: OTHER,
        ..retry(2, 1)
    };
    assert_eq!(
        lab.serve(foreign, &mut cleanup, now),
        RequestOutcome::Foreign
    );
    assert_eq!(cleanup.calls.len(), attempts, "nothing executed");

    // Seq 2 is refused as too soon; four more executed requests evict its
    // receipt.
    let too_soon = retry(2, 1);
    let now = lab.now();
    assert_eq!(
        lab.serve(too_soon, &mut cleanup, now),
        RequestOutcome::Executed(Response::RetryRefused(Refusal::TooSoon))
    );
    for seq in 3..=6 {
        let now = lab.now();
        assert_eq!(
            lab.serve(retry(seq, 0), &mut cleanup, now),
            RequestOutcome::Executed(Response::RetryRefused(Refusal::StaleEpoch))
        );
    }
    let now = lab.later(TEST.recovery_spacing_millis);
    let replayed = lab.serve(too_soon, &mut cleanup, now);
    assert_eq!(
        replayed,
        RequestOutcome::Replayed,
        "[replay-refused] an evicted request executed again: {replayed:?}"
    );
    assert_eq!(
        cleanup.calls.len(),
        attempts,
        "[replay-refused] the replay attempted cleanup"
    );
    assert_eq!(lab.snap().recovery.attempts, 1);

    // A full queue returns the request, neither accepted nor consumed.
    lab.control.submit(retry(7, 1)).expect("queued");
    match lab.control.submit(retry(8, 1)) {
        Err(Busy(returned)) => assert_eq!(returned, retry(8, 1)),
        Ok(()) => panic!("a full queue accepted a request"),
    }
    assert!(lab.control.status().control.queued);
    let now = lab.now();
    let (served, outcome) = lab
        .c()
        .serve(&mut Scripted::always(Plan::Confirm), now)
        .expect("the queued request");
    assert_eq!(served, retry(7, 1));
    assert_eq!(
        outcome,
        RequestOutcome::Executed(Response::Retried {
            attempt: 2,
            resolved: true,
            epoch: 2
        })
    );
    assert_eq!(lab.released(), vec![token]);
}

/// Prior incidents block the run until an external validator dispositions
/// each, bound to exactly that incident; nothing else can, and the incident's
/// own outcome is kept beside its disposition.
#[test]
fn h20_prior_incidents_need_a_bound_external_disposition() {
    let unresolved = PriorIncident {
        binding: IncidentBinding::new([1; 32]),
        outcome: PriorOutcome::Unresolved { outstanding: 2 },
    };
    let malformed = PriorIncident {
        binding: IncidentBinding::new([2; 32]),
        outcome: PriorOutcome::Malformed,
    };
    let resolved = PriorIncident {
        binding: IncidentBinding::new([3; 32]),
        outcome: PriorOutcome::Resolved,
    };
    let mut lab = Lab::with(TEST, GENERATION, vec![unresolved, malformed, resolved]);
    let now = lab.now();
    assert_eq!(
        lab.c().start_run(now).unwrap_err(),
        Refusal::PriorUnresolved
    );

    let misbound = Validator {
        answers: vec![(
            unresolved.binding,
            ValidatedDisposition::new(malformed.binding, DispositionReason::Other),
        )],
        panics: false,
    };
    let silent = Validator {
        answers: Vec::new(),
        panics: false,
    };
    let panicking = Validator {
        answers: Vec::new(),
        panics: true,
    };
    let bound = Validator {
        answers: vec![
            (
                unresolved.binding,
                ValidatedDisposition::new(unresolved.binding, DispositionReason::OwnerDestroyed),
            ),
            (
                malformed.binding,
                ValidatedDisposition::new(malformed.binding, DispositionReason::RecordsMalformed),
            ),
            (
                resolved.binding,
                ValidatedDisposition::new(resolved.binding, DispositionReason::Other),
            ),
        ],
        panics: false,
    };
    for validator in [&misbound, &silent, &panicking] {
        let now = lab.now();
        assert_eq!(
            lab.c()
                .apply_disposition(&unresolved.binding, validator, now),
            Err(Refusal::Undisposable)
        );
    }
    let now = lab.now();
    assert_eq!(
        lab.c().apply_disposition(&resolved.binding, &bound, now),
        Err(Refusal::Undisposable),
        "a resolved incident needs none"
    );
    let now = lab.now();
    assert_eq!(
        lab.c().apply_disposition(&unresolved.binding, &bound, now),
        Ok(())
    );
    let now = lab.now();
    assert_eq!(
        lab.c().start_run(now).unwrap_err(),
        Refusal::PriorUnresolved,
        "every incident needs its own"
    );
    let now = lab.now();
    assert_eq!(
        lab.c().apply_disposition(&malformed.binding, &bound, now),
        Ok(())
    );
    let now = lab.now();
    assert_eq!(
        lab.c().apply_disposition(&malformed.binding, &bound, now),
        Err(Refusal::Undisposable),
        "a disposition applies once"
    );

    assert_eq!(
        lab.snap().incidents,
        vec![
            IncidentView {
                binding: unresolved.binding,
                outcome: unresolved.outcome,
                disposition: Some(DispositionReason::OwnerDestroyed)
            },
            IncidentView {
                binding: malformed.binding,
                outcome: malformed.outcome,
                disposition: Some(DispositionReason::RecordsMalformed)
            },
            IncidentView {
                binding: resolved.binding,
                outcome: resolved.outcome,
                disposition: None
            },
        ]
    );
    lab.start();
    assert_eq!(
        lab.records().first(),
        Some(&RecordKind::RunStarted { dispositioned: 2 })
    );
}

/// Failure history is bounded: details up to the bound (escaped and
/// truncated), beyond it only classified and counted; nothing clears it, and
/// a failed case whose owners are resolved lets the run go on to its next
/// case.
#[test]
fn h21_failure_history_is_bounded_classified_and_never_cleared() {
    let mut lab = Lab::new(Config {
        failure_detail_limit: 3,
        detail_chars: 8,
        ..TEST
    });
    lab.start();
    lab.begin(1, Expectation::Clean);
    for index in 0..10 {
        let now = lab.now();
        lab.c()
            .record_assertion_failure(&format!("assertion {index}\n\u{1b}[31m"), now)
            .expect("recorded");
    }
    let snapshot = lab.snap();
    assert_eq!(
        (
            snapshot.failures.len(),
            snapshot.failure_overflow,
            count(&snapshot, FailureClass::Assertion)
        ),
        (3, 7, 10)
    );
    for failure in &snapshot.failures {
        assert!(!failure.detail.chars().any(char::is_control));
        assert!(failure.detail.chars().count() <= 8 + "...".len());
        assert_eq!(
            (failure.class, failure.case),
            (FailureClass::Assertion, Some(CaseId(1)))
        );
    }
    let outcome = lab.end_case(&mut Scripted::always(Plan::Confirm));
    assert_eq!(
        (outcome.passed, outcome.resolved, outcome.stopped),
        (false, true, false)
    );
    lab.begin(2, Expectation::Clean);
    let (_, token) = lab.create(SlotKind::Fixture);
    let outcome = lab.end_case(&mut Scripted::always(Plan::Confirm));
    assert!(outcome.passed);
    assert_eq!(lab.released(), vec![token]);
    let now = lab.now();
    assert_eq!(lab.c().finish_run(now), Ok(RunPhase::Complete));
    let snapshot = lab.snap();
    assert_eq!(
        (
            snapshot.cases_passed,
            snapshot.cases_failed,
            snapshot.verdict
        ),
        (1, 1, Verdict::Failed)
    );
    assert_eq!(count(&snapshot, FailureClass::Assertion), 10);
}

/// The group's bounds: one open native operation, and one process tree, one
/// workspace and one fixture; each refusal comes before anything is created.
#[test]
fn h22_group_bounds_refuse_before_creation() {
    let mut lab = Lab::new(TEST);
    lab.start();
    lab.begin(1, Expectation::Clean);
    let now = lab.now();
    let reservation = lab
        .c()
        .reserve(SlotKind::Process, now)
        .expect("the action is reserved");
    assert_eq!(reservation.action().generation(), GENERATION);
    let now = lab.now();
    assert_eq!(
        lab.c().reserve(SlotKind::Workspace, now).unwrap_err(),
        Refusal::OperationPending
    );
    lab.ack_all();
    let now = lab.now();
    let ticket = lab.c().admit(reservation, now).expect("admitted");
    let now = lab.now();
    assert_eq!(
        lab.c().reserve(SlotKind::Workspace, now).unwrap_err(),
        Refusal::OperationPending
    );
    let owner = lab.native.produce(&ticket);
    let now = lab.now();
    assert!(matches!(
        lab.c()
            .complete(&ticket, NativeOutcome::Created(owner), now),
        Ok(Completion::Deposited(_))
    ));
    lab.create(SlotKind::Workspace);
    lab.create(SlotKind::Fixture);
    for kind in [SlotKind::Process, SlotKind::Workspace, SlotKind::Fixture] {
        let now = lab.now();
        assert_eq!(
            lab.c().reserve(kind, now).unwrap_err(),
            Refusal::SlotOccupied(kind)
        );
    }
    assert_eq!(lab.native.produced.len(), 3);
    assert_eq!(lab.snap().entries.len(), 3);
    let outcome = lab.end_case(&mut Scripted::always(Plan::Confirm));
    assert!(outcome.passed && outcome.resolved);
    assert_eq!(lab.released().len(), 3);

    assert!(Custody::<Witness>::new(Config::LIVE, GENERATION, Vec::new(), Tick(0)).is_ok());
    let unrecordable = Config {
        control_reserve: 1,
        ..TEST
    };
    assert_eq!(
        Custody::<Witness>::new(unrecordable, GENERATION, Vec::new(), Tick(0)).err(),
        Some(Refusal::Capacity)
    );
}
