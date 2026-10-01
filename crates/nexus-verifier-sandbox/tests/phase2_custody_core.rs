//! P2-V1-R3B-I1, -R1 and -R2 fixture controls for the custody core
//! (`support/custody/`): the same owner survives failed and exhausted
//! cleanup, reporting, refused shutdown and explicit recovery; admission is
//! reserved, acknowledged and linearized against cancellation, and the first
//! actual failure closes it for good (no next case, no reopening), an
//! expected retained boundary's failed first attempt included, at once; an
//! unknown native outcome stays pending until its own bound completion;
//! expected injected conditions stay apart from actual failures, and output
//! detachment is output loss, never completeness; the terminal commitment is
//! ordered against cancellation and lease expiry (accepted before it, they
//! fail the run; after it, they are late and change nothing); a run is final
//! only when its terminal record is acknowledged, after which its verdict
//! never changes and a late owner is an incident of its own; failed or
//! pending evidence keeps the custody from closing; dependencies are released
//! only after native completion; acknowledgements bind exact records;
//! replayed, stale and conflicting events act on nothing.
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
    late_limit: 2,
    failure_detail_limit: 8,
    detail_chars: 64,
    incident_limit: 4,
};

/// A process operation's own end with every native fact confirmed but its
/// output detached.
const DETACHED_END: EndFacts = EndFacts {
    subtree: true,
    reaped: true,
    output: OutputObserved::Detached,
    removed: false,
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
    /// The tree ended natively; its output was detached (lost), not drained.
    Detached,
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
        SlotKind::Process if plan == Plan::Detached => CleanupReport {
            subtree: Observed::Confirmed,
            reaped: Observed::Confirmed,
            output: OutputObserved::Detached,
            removed: Observed::NotLooked,
        },
        SlotKind::Process => {
            let (subtree, reaped, output) = match plan {
                Plan::Confirm => (true, true, true),
                Plan::SubtreeOnly => (true, false, false),
                Plan::ReapedOnly => (false, true, false),
                Plan::OutputPending => (true, true, false),
                Plan::Fail | Plan::Panic | Plan::Detached => (false, false, false),
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
            removed: fact(matches!(plan, Plan::Confirm | Plan::Detached)),
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

    /// Flush and acknowledge until nothing is left: acknowledging a
    /// completion candidate's last record issues its terminal record, which
    /// is then flushed and acknowledged too.
    fn ack_all(&mut self) {
        loop {
            self.flush();
            if self.acked == self.journal.submitted.len() {
                return;
            }
            self.ack_until(self.journal.submitted.len());
        }
    }

    /// As [`Self::ack_all`], stopping at the first record custody does not
    /// accept (after a recording failure).
    fn try_ack_all(&mut self) {
        loop {
            self.flush();
            if self.acked == self.journal.submitted.len() {
                return;
            }
            while self.acked < self.journal.submitted.len() {
                let ack = RecordAck::of(&self.journal.submitted[self.acked]);
                let now = self.now();
                if self.c().acknowledge(&ack, now) != AckOutcome::Acknowledged {
                    return;
                }
                self.acked += 1;
            }
        }
    }

    fn start(&mut self) -> Tick {
        let now = self.now();
        let cap = self.c().start_run(now).expect("the run starts");
        self.cap = Some(cap);
        self.ack_all();
        now
    }

    /// Begin a case once every earlier record is acknowledged.
    fn begin(&mut self, case: u32, expectation: Expectation) {
        self.try_ack_all();
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

    /// Finish the run and settle its evidence: the phase it ends in.
    fn finish(&mut self) -> RunPhase {
        let now = self.now();
        self.c().finish_run(now).expect("the run finishes");
        self.ack_all();
        self.snap().phase
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

    /// The submitted terminal record, if any: its identity and verdict.
    fn terminal_record(&self) -> Option<(RecordId, Verdict)> {
        self.journal
            .submitted
            .iter()
            .find_map(|intent| match intent.kind {
                RecordKind::RunEnded { verdict, .. } => Some((intent.id, verdict)),
                _ => None,
            })
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

fn fault_count(snapshot: &Snapshot, class: FailureClass) -> u32 {
    snapshot.fault_counts[class.index()]
}

fn is_refused_with(decision: &ShutdownDecision, wanted: Unresolved) -> bool {
    matches!(decision, ShutdownDecision::Refused(unresolved) if unresolved.contains(&wanted))
}

fn closed_by_failure(closure: Option<Closure>) -> Option<FailureClass> {
    match closure?.reason {
        ClosureReason::Failed(class) => Some(class),
        ClosureReason::Cancelled(_) => None,
    }
}

/// General (non-control) records issued.
fn general_issued(snapshot: &Snapshot, config: &Config) -> u64 {
    snapshot.evidence.issued - u64::from(config.control_reserve - snapshot.evidence.control_left)
}

/// R1 finding A (within a case): the first actual failure closes execution
/// admission for good. Nothing further is reserved or admitted in that case
/// (not even an action reserved before the failure), the closure names the
/// failure rather than a cancellation, an operation admitted before it is
/// still adopted for cleanup, and nothing reopens admission after cleanup.
#[test]
fn r01_actual_failure_closes_admission_within_its_case() {
    let mut lab = Lab::new(TEST);
    lab.start();
    lab.begin(1, Expectation::Clean);
    let now = lab.now();
    lab.c()
        .record_assertion_failure("an observation failed", now)
        .expect("recorded");
    let now = lab.now();
    let reserved = lab.c().reserve(SlotKind::Process, now);
    let admission = lab.control.status().control.admission;
    assert!(
        matches!(
            reserved,
            Err(Refusal::AdmissionClosed(Closure {
                reason: ClosureReason::Failed(FailureClass::Assertion),
                ..
            }))
        ),
        "[fail-stop] after an actual failure: reserve -> {reserved:?}; admission {admission:?}"
    );
    assert_eq!(admission.admitted, 0);
    assert!(lab.native.produced.is_empty());

    // An action reserved before the failure is not admitted after it.
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
    lab.c()
        .record_assertion_failure("an observation failed", now)
        .expect("recorded");
    let now = lab.now();
    let refused = lab.c().admit(reservation, now).unwrap_err();
    assert!(
        matches!(refused.refusal, Refusal::AdmissionClosed(_)),
        "[fail-stop] a reservation made before the failure was admitted after it"
    );
    assert!(refused.reservation.is_none(), "the action is settled");
    lab.flush();
    assert!(lab.records().iter().any(|kind| matches!(
        kind,
        RecordKind::ActionSettled {
            how: Settlement::NotAdmitted,
            ..
        }
    )));

    // An operation admitted before the failure still delivers, and its owner
    // is adopted for cleanup; admission stays closed after the cleanup.
    let mut lab = Lab::new(TEST);
    lab.start();
    lab.begin(1, Expectation::Clean);
    let ticket = lab.admit(SlotKind::Process);
    let now = lab.now();
    lab.c()
        .record_assertion_failure("an observation failed", now)
        .expect("recorded");
    let owner = lab.native.produce(&ticket);
    let token = owner.token;
    let now = lab.now();
    let Ok(Completion::Deposited(entry)) =
        lab.c()
            .complete(&ticket, NativeOutcome::Created(owner), now)
    else {
        panic!("the admitted operation's owner was not adopted");
    };
    let snapshot = lab.snap();
    assert_eq!(
        (only_entry(&snapshot).entry, only_entry(&snapshot).phase),
        (entry, EntryPhase::CleanupRequired)
    );
    let outcome = lab.end_case(&mut Scripted::always(Plan::Confirm));
    assert_eq!(
        (outcome.passed, outcome.resolved, outcome.stopped),
        (false, true, true)
    );
    assert_eq!(lab.released(), vec![token]);
    let now = lab.now();
    assert!(lab.c().reserve(SlotKind::Process, now).is_err());
    assert_eq!(
        closed_by_failure(lab.control.status().control.admission.closure),
        Some(FailureClass::Assertion),
        "an actual failure is never recorded as a cancellation"
    );
    assert_eq!(count(&lab.snap(), FailureClass::Cancelled), 0);
    assert!(lab.dropped().is_empty());
}

/// One way an actual failure enters a running case whose owners the case's
/// end can still clean up.
type FailureScenario = (&'static str, Expectation, FailureClass, fn(&mut Lab));

/// R1 finding A (between cases): every actual failure the core classifies,
/// once its case's resources are confirmed cleaned up, still stops the run:
/// no next case begins and nothing further is admitted, after any amount of
/// acknowledged evidence.
#[test]
fn r02_failed_case_cleanup_never_permits_a_next_case() {
    let scenarios: [FailureScenario; 9] = [
        (
            "an assertion",
            Expectation::Clean,
            FailureClass::Assertion,
            |lab| {
                lab.create(SlotKind::Process);
                let now = lab.now();
                lab.c()
                    .record_assertion_failure("an observation failed", now)
                    .expect("recorded");
            },
        ),
        (
            "a panicking borrower",
            Expectation::Clean,
            FailureClass::Assertion,
            |lab| {
                let (entry, _) = lab.create(SlotKind::Process);
                let now = lab.now();
                let panicked = lab
                    .c()
                    .lend(entry, now, |_| -> u64 { panic!("injected borrower panic") });
                assert_eq!(panicked, Err(LendError::Panicked));
            },
        ),
        (
            "an unexpected retained boundary",
            Expectation::Clean,
            FailureClass::UnexpectedRetained,
            |lab| {
                lab.deposit(SlotKind::Process, true);
            },
        ),
        (
            "a misfit owner",
            Expectation::Clean,
            FailureClass::UnexpectedOwner,
            |lab| {
                let ticket = lab.admit(SlotKind::Workspace);
                let owner = lab.native.produce_as(&ticket, Some(SlotKind::Fixture));
                let now = lab.now();
                assert!(matches!(
                    lab.c()
                        .complete(&ticket, NativeOutcome::Created(owner), now),
                    Ok(Completion::Unexpected(_))
                ));
            },
        ),
        (
            "a late owner",
            Expectation::Clean,
            FailureClass::LateOwner,
            |lab| {
                let ticket = lab.admit(SlotKind::Process);
                let now = lab.now();
                let proof = NoEffectProof::for_ticket(&ticket, "refused");
                lab.c()
                    .complete(&ticket, NativeOutcome::NoEffect(proof), now)
                    .expect("resolved");
                let late = lab.native.produce(&ticket);
                let now = lab.now();
                assert!(matches!(
                    lab.c().complete(&ticket, NativeOutcome::Created(late), now),
                    Ok(Completion::Late { .. })
                ));
            },
        ),
        (
            "a detached output",
            Expectation::Clean,
            FailureClass::OutputLost,
            |lab| {
                let ticket = lab.admit(SlotKind::Process);
                let now = lab.now();
                assert!(matches!(
                    lab.c()
                        .complete(&ticket, NativeOutcome::Ended(DETACHED_END), now),
                    Ok(Completion::OutputLost)
                ));
            },
        ),
        (
            "an unmet expected condition",
            Expectation::RetainedBoundary,
            FailureClass::ExpectedConditionUnmet,
            |lab| {
                lab.create(SlotKind::Process);
            },
        ),
        (
            "a recorder adapter panic",
            Expectation::Clean,
            FailureClass::RecorderFault,
            |lab| {
                let now = lab.now();
                assert!(matches!(
                    lab.c().flush_records(&mut PanickingSink, now),
                    Err(FlushError::SinkPanicked { .. })
                ));
            },
        ),
        (
            "a recording failure",
            Expectation::Clean,
            FailureClass::RecordFailed,
            |lab| {
                lab.flush();
                let id = lab.journal.submitted[lab.acked].id;
                let now = lab.now();
                assert_eq!(lab.c().record_failed(id, now), AckOutcome::FailureRecorded);
            },
        ),
    ];
    for (what, expectation, class, inject) in scenarios {
        let mut lab = Lab::new(TEST);
        lab.start();
        lab.begin(1, expectation);
        inject(&mut lab);
        let outcome = lab.end_case(&mut Scripted::always(Plan::Confirm));
        assert!(!outcome.passed && outcome.resolved, "{what}: {outcome:?}");
        assert!(
            outcome.stopped,
            "[no-next-case] after {what}, the run went on past its failed case: {outcome:?}"
        );
        lab.released();
        lab.try_ack_all();
        let admitted = lab.control.status().control.admission.admitted;
        let now = lab.now();
        let next = lab.c().begin_case(CaseId(2), Expectation::Clean, now);
        assert!(
            matches!(
                next,
                Err(Refusal::AdmissionClosed(Closure {
                    reason: ClosureReason::Failed(first),
                    ..
                })) if first == class
            ),
            "[no-next-case] after {what} and its cleanup, the next case -> {next:?}"
        );
        let now = lab.now();
        assert!(lab.c().reserve(SlotKind::Process, now).is_err(), "{what}");
        assert_eq!(
            lab.control.status().control.admission.admitted,
            admitted,
            "{what}"
        );
        assert_ne!(lab.snap().verdict, Verdict::Passed, "{what}");
        assert!(lab.dropped().is_empty(), "{what}");
    }
}

/// R1 finding B: physical cleanup can be confirmed while required evidence
/// failed; the custody then refuses ordinary closure for good, returning the
/// same custody with the failed record, the pending settlement and the
/// verdict intact, and never recreates the ended owner.
#[test]
fn r03_failed_evidence_keeps_the_custody_open() {
    let mut lab = Lab::new(TEST);
    lab.start();
    lab.begin(1, Expectation::Clean);
    let (entry, token) = lab.create(SlotKind::Process);
    let outcome = lab.end_case(&mut Scripted::always(Plan::Confirm));
    assert!(outcome.passed);
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

    let decision = lab.request_shutdown();
    let failed_record = matches!(
        &decision,
        ShutdownDecision::Refused(unresolved) if unresolved.iter().any(|item| matches!(
            item,
            Unresolved::EvidenceFailed { record, .. } if *record == settlement.id
        ))
    );
    assert!(
        failed_record,
        "[evidence-close] the shutdown decision hid the failed evidence: {decision:?}"
    );
    let closed = lab.close();
    assert!(
        closed.is_none(),
        "[evidence-close] closed with failed evidence: {closed:?}"
    );
    // The same custody: the failed record, the settlement that can never be
    // durable and the verdict are all as they were.
    let snapshot = lab.snap();
    assert_eq!(
        snapshot.evidence.failed.map(|(id, _)| id),
        Some(settlement.id)
    );
    let settling: Vec<_> = snapshot
        .settling
        .iter()
        .map(|view| (view.entry, view.record, view.how))
        .collect();
    assert_eq!(
        settling,
        vec![(Some(entry), settlement.id, Settlement::Confirmed)]
    );
    assert_eq!(snapshot.verdict, Verdict::Failed);
    assert!(snapshot.entries.is_empty());
    // A later acknowledgement cannot heal the recorder.
    let now = lab.now();
    assert_eq!(
        lab.c().acknowledge(&RecordAck::of(&settlement), now),
        AckOutcome::LedgerFailed
    );
    assert!(
        lab.close().is_none(),
        "[evidence-close] closed after an unbound acknowledgement"
    );
    assert_eq!(lab.native.produced.len(), 1, "the owner was not recreated");
    assert!(lab.dropped().is_empty());
}

/// R1 finding C: a run's verdict is published as a pass only once its
/// terminal record is acknowledged, and from then on it never changes:
/// ordinary case mutations are refused, a later fault is the custody's (not
/// the run's), and the closed result reports exactly the acknowledged
/// terminal record.
#[test]
fn r04_finalized_verdict_matches_acknowledged_terminal_evidence() {
    let mut lab = Lab::new(TEST);
    lab.start();
    lab.begin(1, Expectation::Clean);
    let ticket = lab.admit(SlotKind::Process);
    let owner = lab.native.produce(&ticket);
    let now = lab.now();
    lab.c()
        .complete(&ticket, NativeOutcome::Created(owner), now)
        .expect("deposited");
    assert!(lab.end_case(&mut Scripted::always(Plan::Confirm)).passed);
    lab.released();
    let now = lab.now();
    assert_eq!(lab.c().finish_run(now), Ok(RunPhase::Candidate));
    let snapshot = lab.snap();
    assert_eq!(
        (snapshot.verdict, snapshot.terminal),
        (Verdict::Pending, None),
        "no pass before the terminal record"
    );
    lab.ack_all();
    let (terminal, recorded) = lab.terminal_record().expect("the terminal record");
    assert_eq!(recorded, Verdict::Passed);
    let snapshot = lab.snap();
    assert_eq!(
        (snapshot.phase, snapshot.verdict),
        (RunPhase::Finalized, Verdict::Passed)
    );

    // Ordinary case mutations are refused, changing nothing.
    let now = lab.now();
    assert_eq!(
        lab.c().record_assertion_failure("a late observation", now),
        Err(Refusal::Phase(RunPhase::Finalized))
    );
    let now = lab.now();
    assert!(lab
        .c()
        .begin_case(CaseId(2), Expectation::Clean, now)
        .is_err());
    let now = lab.now();
    assert!(lab.c().reserve(SlotKind::Process, now).is_err());
    let now = lab.now();
    assert!(lab.c().finish_run(now).is_err());
    assert_eq!(lab.snap().verdict, Verdict::Passed);

    // A late owner after finalization is a fault of the custody: the run's
    // acknowledged verdict stands.
    let late = lab.native.produce(&ticket);
    let late_token = late.token;
    let now = lab.now();
    assert!(matches!(
        lab.c().complete(&ticket, NativeOutcome::Created(late), now),
        Ok(Completion::Late { .. })
    ));
    let snapshot = lab.snap();
    assert_eq!(
        (snapshot.verdict, snapshot.phase),
        (Verdict::Passed, RunPhase::Finalized),
        "[finalized-verdict] a fault after the terminal record changed the run's outcome"
    );
    assert_eq!(fault_count(&snapshot, FailureClass::LateOwner), 1);
    assert_eq!(snapshot.failure_counts, [0; FailureClass::COUNT]);

    // Its recovery and evidence, then the closed result: the acknowledged
    // terminal record and its verdict, with the fault beside them.
    assert!(
        lab.close().is_none(),
        "a held late incident refuses closure"
    );
    assert_eq!(
        lab.retry(&mut Scripted::always(Plan::Confirm)),
        Response::Retried {
            attempt: 1,
            resolved: true,
            epoch: 1
        }
    );
    assert_eq!(lab.released(), vec![late_token]);
    lab.ack_all();
    let issued = lab.snap().evidence.issued;
    let closed = lab.close().expect("closes once nothing is unresolved");
    assert_eq!(
        (closed.terminal, closed.verdict, closed.records),
        (Some(terminal), recorded, issued),
        "[finalized-verdict] the closed result does not match the acknowledged terminal record"
    );
    assert_eq!(closed.fault_counts[FailureClass::LateOwner.index()], 1);
    assert_eq!(lab.snap().verdict, Verdict::Passed);
}

/// R1 finding C: an owner delivered for an operation already retired is a
/// late incident with its own identity and records, never attached to the
/// retired action's lifecycle; before the terminal record it is a failure of
/// the run, after it a fault that leaves the finalized run as recorded; past
/// the bound it is returned, never dropped.
#[test]
fn r05_late_owner_is_a_bound_incident_never_the_retired_action() {
    // Before the terminal record: the run fails and requires recovery.
    let mut lab = Lab::new(TEST);
    lab.start();
    lab.begin(1, Expectation::Clean);
    let ticket = lab.admit(SlotKind::Process);
    let owner = lab.native.produce(&ticket);
    let now = lab.now();
    lab.c()
        .complete(&ticket, NativeOutcome::Created(owner), now)
        .expect("deposited");
    lab.end_case(&mut Scripted::always(Plan::Confirm));
    lab.released();
    let now = lab.now();
    assert_eq!(lab.c().finish_run(now), Ok(RunPhase::Candidate));
    lab.flush();
    let settled_at = lab.journal.submitted.len();
    let late = lab.native.produce(&ticket);
    let late_token = late.token;
    let now = lab.now();
    let Ok(Completion::Late { incident, entry }) =
        lab.c().complete(&ticket, NativeOutcome::Created(late), now)
    else {
        panic!("[late-incident] the late owner was not held as an incident");
    };
    lab.flush();
    let after: Vec<RecordKind> = lab.records()[settled_at..].to_vec();
    let reuses = after.iter().any(|kind| {
        matches!(kind,
            RecordKind::ActionFailed { action } | RecordKind::ActionSettled { action, .. }
                if *action == ticket.action())
    });
    let opened = after.contains(&RecordKind::IncidentOpened {
        incident,
        action: ticket.action(),
        kind: Some(SlotKind::Process),
    });
    assert!(
        opened && !reuses,
        "[late-incident] the late owner's evidence is not its own incident's: {after:?}"
    );
    let snapshot = lab.snap();
    let held = only_entry(&snapshot);
    assert_eq!(
        (held.entry, held.incident, held.action, held.origin),
        (entry, Some(incident), ticket.action(), Origin::Late)
    );
    assert_eq!(snapshot.phase, RunPhase::RecoveryRequired);
    assert_eq!(count(&snapshot, FailureClass::LateOwner), 1);
    assert_eq!(
        lab.retry(&mut Scripted::always(Plan::Confirm)),
        Response::Retried {
            attempt: 1,
            resolved: true,
            epoch: 1
        }
    );
    assert_eq!(lab.released(), vec![late_token]);
    lab.flush();
    assert!(lab.records().contains(&RecordKind::IncidentSettled {
        incident,
        how: Settlement::Confirmed
    }));
    lab.ack_all();
    assert_eq!(
        lab.terminal_record().map(|(_, verdict)| verdict),
        Some(Verdict::Failed)
    );

    // After the terminal record: the finalized run is not rewritten.
    let mut lab = Lab::new(TEST);
    lab.start();
    lab.begin(1, Expectation::Clean);
    let ticket = lab.admit(SlotKind::Process);
    let now = lab.now();
    let proof = NoEffectProof::for_ticket(&ticket, "refused");
    lab.c()
        .complete(&ticket, NativeOutcome::NoEffect(proof), now)
        .expect("resolved");
    lab.end_case(&mut Scripted::always(Plan::Confirm));
    assert_eq!(lab.finish(), RunPhase::Finalized);
    let terminal = lab.terminal_record();
    let late = lab.native.produce(&ticket);
    let now = lab.now();
    let Ok(Completion::Late { incident, .. }) =
        lab.c().complete(&ticket, NativeOutcome::Created(late), now)
    else {
        panic!("[late-incident] a late owner after finalization was not held as an incident");
    };
    let snapshot = lab.snap();
    assert_eq!(
        (snapshot.phase, snapshot.verdict),
        (RunPhase::Finalized, Verdict::Passed)
    );
    assert!(is_refused_with(
        &lab.c().shutdown_decision(),
        Unresolved::LateIncidents(1)
    ));
    lab.flush();
    assert!(lab
        .records()
        .iter()
        .any(|kind| matches!(kind, RecordKind::IncidentOpened { incident: opened, .. } if *opened == incident)));
    assert_eq!(lab.terminal_record(), terminal, "one terminal record");

    // Past the bound: returned, unrecorded, never dropped.
    let mut lab = Lab::new(Config {
        late_limit: 1,
        ..TEST
    });
    lab.start();
    lab.begin(1, Expectation::Clean);
    let ticket = lab.admit(SlotKind::Process);
    let now = lab.now();
    let proof = NoEffectProof::for_ticket(&ticket, "refused");
    lab.c()
        .complete(&ticket, NativeOutcome::NoEffect(proof), now)
        .expect("resolved");
    let first = lab.native.produce(&ticket);
    let now = lab.now();
    assert!(matches!(
        lab.c()
            .complete(&ticket, NativeOutcome::Created(first), now),
        Ok(Completion::Late { .. })
    ));
    let second = lab.native.produce(&ticket);
    let second_token = second.token;
    let now = lab.now();
    match lab
        .c()
        .complete(&ticket, NativeOutcome::Created(second), now)
    {
        Err(Rejected::CustodyFull { owner }) => {
            assert_eq!(owner.token, second_token, "the same owner comes back");
            lab.returned.push(owner);
        }
        other => panic!("[late-incident] an owner past the bound: {other:?}"),
    }
    lab.flush();
    let opened = lab
        .records()
        .iter()
        .filter(|kind| matches!(kind, RecordKind::IncidentOpened { .. }))
        .count();
    assert_eq!(opened, 1, "no incident record for a returned owner");
    assert_eq!(
        count(&lab.snap(), FailureClass::LateOwner),
        2,
        "[late-incident] a returned late owner is still a failure"
    );
    assert!(lab.dropped().is_empty());
}

/// R1 finding D: detached output is output loss, never completeness. In an
/// ordinary clean case it is an actual failure by either route (the
/// operation's own end, or the cleanup of a stored owner); the native
/// resources that genuinely ended are still released, and the settlement
/// record says the output was lost.
#[test]
fn r06_detached_output_is_output_loss_never_a_clean_pass() {
    // Route 1: the operation's own end reports detached output.
    let mut lab = Lab::new(TEST);
    lab.start();
    lab.begin(1, Expectation::Clean);
    let ticket = lab.admit(SlotKind::Process);
    let now = lab.now();
    let completion = lab
        .c()
        .complete(&ticket, NativeOutcome::Ended(DETACHED_END), now);
    let failures = count(&lab.snap(), FailureClass::OutputLost);
    let outcome = lab.end_case(&mut Scripted::always(Plan::Confirm));
    assert!(
        matches!(completion, Ok(Completion::OutputLost)) && failures == 1 && !outcome.passed,
        "[output-lost] the operation's end: {completion:?}, failures {failures}, case {outcome:?}"
    );
    lab.ack_all();
    assert!(lab.records().contains(&RecordKind::ActionSettled {
        action: ticket.action(),
        how: Settlement::OutputLost
    }));
    assert_eq!(
        lab.terminal_record().map(|(_, verdict)| verdict),
        Some(Verdict::Failed)
    );

    // Route 2: cleanup of a stored owner reports detached output.
    let mut lab = Lab::new(TEST);
    lab.start();
    lab.begin(1, Expectation::Clean);
    let (_, token) = lab.create(SlotKind::Process);
    let outcome = lab.end_case(&mut Scripted::always(Plan::Detached));
    let snapshot = lab.snap();
    assert!(
        !outcome.passed && count(&snapshot, FailureClass::OutputLost) == 1,
        "[output-lost] the cleanup route: case {outcome:?}, failures {:?}",
        snapshot.failure_counts
    );
    assert!(outcome.resolved, "a natively ended tree is still released");
    assert_eq!(lab.released(), vec![token]);
    assert_eq!(
        closed_by_failure(lab.control.status().control.admission.closure),
        Some(FailureClass::OutputLost)
    );
    lab.ack_all();
    assert!(lab.records().iter().any(|kind| matches!(
        kind,
        RecordKind::ActionSettled {
            how: Settlement::OutputLost,
            ..
        }
    )));
    assert_eq!(lab.snap().verdict, Verdict::Failed);
}

/// A completion candidate whose case passed and whose earlier evidence is
/// issued but not yet acknowledged: every record but the last is
/// acknowledged, so the next acknowledgement reaches the commitment point.
fn candidate_before_last_ack(lab: &mut Lab) {
    lab.begin(1, Expectation::Clean);
    let (_, token) = lab.create(SlotKind::Process);
    assert!(lab.end_case(&mut Scripted::always(Plan::Confirm)).passed);
    assert_eq!(lab.released(), vec![token]);
    let now = lab.now();
    assert_eq!(lab.c().finish_run(now), Ok(RunPhase::Candidate));
    lab.flush();
    lab.ack_until(lab.journal.submitted.len() - 1);
    let snapshot = lab.snap();
    assert_eq!(
        (snapshot.phase, snapshot.terminal, snapshot.verdict),
        (RunPhase::Candidate, None, Verdict::Pending)
    );
    assert_eq!(lab.control.status().control.admission.committed, None);
}

/// After a cancellation accepted before the commitment: the terminal record
/// failed the run, the cancellation is the run's only failure (no case's,
/// and no test failure), its control fact is recorded once before the
/// terminal record, and the closed result says the same. The capture of
/// what was observed.
fn cancelled_outcome(lab: &mut Lab, closure: Closure) -> (bool, String) {
    lab.ack_all();
    let snapshot = lab.snap();
    let admission = lab.control.status().control.admission;
    let terminal = lab.terminal_record();
    let records = lab.records();
    let facts: Vec<usize> = records
        .iter()
        .enumerate()
        .filter(|(_, kind)| {
            matches!(kind, RecordKind::Control {
                fact: ControlFact::AdmissionClosed { reason, .. }
            } if *reason == closure.reason)
        })
        .map(|(index, _)| index)
        .collect();
    let ended = records
        .iter()
        .position(|kind| matches!(kind, RecordKind::RunEnded { .. }));
    let failures: Vec<(FailureClass, Option<CaseId>)> = snapshot
        .failures
        .iter()
        .map(|failure| (failure.class, failure.case))
        .collect();
    let closed = lab.close();
    let capture = format!(
        "admission {admission:?}; failures {failures:?}; terminal {terminal:?}; published {:?}; \
         control facts at {facts:?}, terminal at {ended:?}; close {closed:?}",
        snapshot.verdict
    );
    let failed = terminal.map(|(_, verdict)| verdict) == Some(Verdict::Failed)
        && failures == vec![(FailureClass::Cancelled, None)]
        && snapshot.verdict == Verdict::Failed
        && admission.closure == Some(closure)
        && admission.committed.is_some()
        && facts.len() == 1
        && ended.is_some_and(|ended| facts[0] < ended)
        && closed
            .as_ref()
            .map(|closed| (closed.terminal, closed.verdict))
            == Some((terminal.map(|(record, _)| record), Verdict::Failed));
    (failed, capture)
}

/// R2 finding A (explicit cancellation): a cancellation accepted while the
/// run is a completion candidate (its earlier evidence unacknowledged, no
/// terminal record yet) is part of the terminal commitment's outcome: the run
/// fails, the cancellation recorded once as its own failure, whether or not
/// the execution owner observed control before the last acknowledgement.
/// Observed, it is the run's failure at once.
#[test]
fn r07_cancellation_accepted_in_candidate_prevents_a_pass() {
    for observe in [false, true] {
        let mut lab = Lab::new(TEST);
        lab.start();
        candidate_before_last_ack(&mut lab);
        let now = lab.now();
        let receipt = lab.control.cancel(CancelReason::Requested, now);
        let closure = Closure {
            reason: ClosureReason::Cancelled(CancelReason::Requested),
            at: now,
            after: 1,
        };
        assert_eq!(
            receipt,
            CancelReceipt::Accepted(closure),
            "observe {observe}"
        );
        if observe {
            let now = lab.now();
            assert_eq!(lab.c().observe_control(now), Ok(Some(closure)));
            let snapshot = lab.snap();
            assert!(
                snapshot.verdict == Verdict::Failed
                    && count(&snapshot, FailureClass::Cancelled) == 1
                    && snapshot.terminal.is_none(),
                "[cancel-observed] an observed cancellation of a candidate is not its failure at \
                 once: published {:?}, failures {:?}",
                snapshot.verdict,
                snapshot.failure_counts
            );
        }
        let (failed, capture) = cancelled_outcome(&mut lab, closure);
        assert!(
            failed,
            "[cancel-before-commit] observe {observe}: receipt {receipt:?}; {capture}"
        );
        assert!(lab.dropped().is_empty());
    }
}

/// R2 finding A (lease expiry): an expiry observed while the run is a
/// completion candidate is a cancellation accepted before the commitment:
/// the run fails, with or without an explicit observation before the last
/// acknowledgement. Reading status never renews or expires the lease;
/// instants are injected, nothing sleeps.
#[test]
fn r08_lease_expiry_observed_in_candidate_prevents_a_pass() {
    for observe in [false, true] {
        let mut lab = Lab::new(TEST);
        let started = lab.start();
        candidate_before_last_ack(&mut lab);
        let deadline = started.plus(TEST.lease_millis);
        for _ in 0..3 {
            assert_eq!(
                lab.control.status().control.lease,
                LeaseView {
                    state: LeaseState::Active { deadline },
                    renewals: 0
                }
            );
        }
        let now = lab.later(TEST.lease_millis);
        assert!(now >= deadline);
        assert_eq!(lab.control.check_lease(now), LeaseState::Lost { at: now });
        let closure = Closure {
            reason: ClosureReason::Cancelled(CancelReason::LeaseLost),
            at: now,
            after: 1,
        };
        assert_eq!(
            lab.control.status().control.admission.closure,
            Some(closure)
        );
        if observe {
            let now = lab.now();
            assert_eq!(lab.c().observe_control(now), Ok(Some(closure)));
            let snapshot = lab.snap();
            assert!(
                snapshot.verdict == Verdict::Failed
                    && count(&snapshot, FailureClass::Cancelled) == 1,
                "[lease-observed] published {:?}, failures {:?}",
                snapshot.verdict,
                snapshot.failure_counts
            );
        }
        let (failed, capture) = cancelled_outcome(&mut lab, closure);
        assert!(failed, "[lease-before-commit] observe {observe}: {capture}");
    }
}

/// R2 finding B: a retained boundary whose first cleanup attempt does not
/// confirm every fact has failed for good at that attempt. Ended through
/// `end_entry` (the next attempt confirming), the expectation is recorded
/// unmet and admission closed before the case ends, so no further native
/// action is admitted in that case; the cleanup still ends and releases the
/// same owner, a dependency in use stays held until its turn, the later
/// success undoes nothing, and the case's end records nothing twice.
#[test]
fn r09_failed_first_attempt_of_a_retained_boundary_closes_admission_at_once() {
    let mut lab = Lab::new(TEST);
    lab.start();
    lab.begin(1, Expectation::RetainedBoundary);
    let (workspace, workspace_token) = lab.create(SlotKind::Workspace);
    let (entry, token) = lab.deposit(SlotKind::Process, true);
    let mut cleanup = Scripted::always(Plan::Confirm).first(SlotKind::Process, &[Plan::Fail]);
    let now = lab.now();
    let ended = lab.c().end_entry(entry, &mut cleanup, now);
    let snapshot = lab.snap();
    let case = snapshot.case.clone().expect("the case is still active");
    let failures: Vec<FailureClass> = snapshot
        .failures
        .iter()
        .map(|failure| failure.class)
        .collect();
    let gate = lab.control.status().control.admission;
    let attempts = cleanup.attempts_on(token);
    let released = lab.released();
    let now = lab.now();
    let reserved = lab.c().reserve(SlotKind::Fixture, now);
    let capture = format!(
        "end_entry {ended:?} after {attempts} attempts, released {released:?}; case {case:?}; \
         failures {failures:?}; gate {gate:?}; reserve {reserved:?}"
    );
    assert!(
        failures == vec![FailureClass::ExpectedConditionUnmet]
            && case.failed
            && case.retained_first_confirmed == Some(false)
            && matches!(
                gate.closure,
                Some(Closure {
                    reason: ClosureReason::Failed(FailureClass::ExpectedConditionUnmet),
                    ..
                })
            )
            && matches!(reserved, Err(Refusal::AdmissionClosed(_))),
        "[expectation-latched] {capture}"
    );
    // The cleanup went on within its budget and released the same owner; the
    // dependency in use is still held, nothing was admitted or produced.
    assert_eq!((ended, attempts, released), (Ok(true), 2, vec![token]));
    let held = only_entry(&snapshot);
    assert_eq!((held.entry, held.phase), (workspace, EntryPhase::Live));
    assert_eq!(gate.admitted, 2);
    assert_eq!(lab.native.produced.len(), 2);
    assert_eq!(
        lab.control.status().control.admission,
        gate,
        "a refused reservation changes nothing"
    );

    let outcome = lab.end_case(&mut Scripted::always(Plan::Confirm));
    assert_eq!(
        (outcome.passed, outcome.resolved, outcome.stopped),
        (false, true, true)
    );
    assert_eq!(lab.released(), vec![workspace_token]);
    let snapshot = lab.snap();
    assert_eq!(
        count(&snapshot, FailureClass::ExpectedConditionUnmet),
        1,
        "[expectation-once] the case's end recorded the latched expectation again: {:?}",
        snapshot.failure_counts
    );
    assert_eq!(lab.control.status().control.admission.closure, gate.closure);
    lab.try_ack_all();
    let now = lab.now();
    assert!(matches!(
        lab.c().begin_case(CaseId(2), Expectation::Clean, now),
        Err(Refusal::AdmissionClosed(_))
    ));
    lab.ack_all();
    assert_eq!(
        lab.terminal_record().map(|(_, verdict)| verdict),
        Some(Verdict::Failed)
    );
    assert_eq!(lab.returned.len(), 2);
    assert!(lab.dropped().is_empty());
}

/// R2 finding A (a cancellation concurrent with the last acknowledgement):
/// the recorder stand-in pauses inside a submission, after the execution
/// owner last looked at control; the control side's cancellation is accepted
/// during the pause; the acknowledgement that follows reaches the commitment
/// point, which sees the cancellation: the run fails, with and without an
/// explicit observation in between. Channels order the threads; nothing
/// sleeps, and the pause is the stand-in's, outside the core.
#[test]
fn r10_cancellation_while_the_recorder_is_paused_precedes_the_commitment() {
    for observe in [false, true] {
        let mut lab = Lab::new(TEST);
        lab.start();
        lab.begin(1, Expectation::Clean);
        let (_, token) = lab.create(SlotKind::Process);
        assert!(lab.end_case(&mut Scripted::always(Plan::Confirm)).passed);
        assert_eq!(lab.released(), vec![token]);
        let now = lab.now();
        assert_eq!(lab.c().finish_run(now), Ok(RunPhase::Candidate));

        let (entered, entered_rx) = mpsc::channel();
        let (release, release_rx) = mpsc::channel();
        let mut sink = StallingSink {
            inner: std::mem::take(&mut lab.journal),
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
            .expect("the recorder holds a pending record");
        let control = lab.control.clone();
        let cancel_at = lab.now();
        let receipt = answered("cancellation", move || {
            control.cancel(CancelReason::Requested, cancel_at)
        });
        let closure = Closure {
            reason: ClosureReason::Cancelled(CancelReason::Requested),
            at: cancel_at,
            after: 1,
        };
        assert_eq!(
            receipt,
            CancelReceipt::Accepted(closure),
            "observe {observe}"
        );
        release.send(()).expect("the recorder is waiting");
        let (custody, sink, sent) = executor.join().expect("the execution owner");
        lab.custody = Some(custody);
        lab.journal = sink.inner;
        assert_eq!(sent, Ok(2), "both pending records were submitted");
        if observe {
            let now = lab.now();
            assert_eq!(lab.c().observe_control(now), Ok(Some(closure)));
        }
        let (failed, capture) = cancelled_outcome(&mut lab, closure);
        assert!(failed, "[cancel-in-flight] observe {observe}: {capture}");
    }
}

/// R2 finding A (finalization winning): once the last earlier
/// acknowledgement made the terminal commitment (its terminal record
/// issued), a cancellation or an observed lease expiry is late: its receipt
/// names the commitment and claims no cancellation, admission is not closed
/// by it, the terminal record and its verdict stay as committed, and no pass
/// is published until that record is acknowledged. A pending or failed
/// terminal record still refuses closure. Repeated acknowledgements,
/// observations, cancellations and status reads leave exactly one terminal
/// record, and a later owner is still a late incident (returned past the
/// bound), never a change of the outcome.
#[test]
fn r11_finalization_winning_makes_a_later_cancellation_late() {
    let mut lab = Lab::new(Config {
        late_limit: 1,
        ..TEST
    });
    lab.start();
    lab.begin(1, Expectation::Clean);
    let ticket = lab.admit(SlotKind::Process);
    let owner = lab.native.produce(&ticket);
    let now = lab.now();
    lab.c()
        .complete(&ticket, NativeOutcome::Created(owner), now)
        .expect("deposited");
    assert!(lab.end_case(&mut Scripted::always(Plan::Confirm)).passed);
    lab.released();
    let now = lab.now();
    assert_eq!(lab.c().finish_run(now), Ok(RunPhase::Candidate));
    lab.flush();
    lab.ack_until(lab.journal.submitted.len());
    let commitment = Commitment {
        at: Tick(lab.clock),
    };
    let snapshot = lab.snap();
    let terminal = snapshot
        .terminal
        .expect("committed: the terminal record is issued");
    assert_eq!(
        (
            snapshot.phase,
            terminal.verdict,
            terminal.acknowledged,
            snapshot.verdict
        ),
        (
            RunPhase::Finalizing,
            Verdict::Passed,
            false,
            Verdict::Pending
        )
    );
    let admission = lab.control.status().control.admission;
    assert_eq!(
        (admission.closure, admission.committed),
        (None, Some(commitment))
    );

    let now = lab.now();
    let receipt = lab.control.cancel(CancelReason::Requested, now);
    let after = lab.control.status().control.admission;
    assert!(
        receipt == CancelReceipt::Late(commitment) && after == admission,
        "[late-cancel] after the commitment: receipt {receipt:?}, admission {after:?}"
    );
    let now = lab.later(TEST.lease_millis);
    assert_eq!(lab.control.check_lease(now), LeaseState::Lost { at: now });
    assert_eq!(
        lab.control.status().control.admission,
        admission,
        "[late-cancel] a lease expiry after the commitment closed admission"
    );
    let cap = lab.cap.take().expect("the run's lease capability");
    let now = lab.now();
    assert_eq!(lab.control.renew_lease(&cap, now), Err(LeaseError::Lost));
    for reason in [
        CancelReason::Stop,
        CancelReason::Shutdown,
        CancelReason::RunBudget,
    ] {
        let now = lab.now();
        assert_eq!(lab.c().observe_control(now), Ok(None));
        let now = lab.now();
        assert_eq!(
            lab.control.cancel(reason, now),
            CancelReceipt::Late(commitment)
        );
        assert_eq!(lab.control.status().control.admission, admission);
    }
    let snapshot = lab.snap();
    assert_eq!(
        (
            snapshot.phase,
            snapshot.verdict,
            snapshot.failure_counts,
            snapshot.cancel_observed
        ),
        (
            RunPhase::Finalizing,
            Verdict::Pending,
            [0; FailureClass::COUNT],
            None
        ),
        "no pass before the terminal record is acknowledged, and no cancellation"
    );
    let decision = lab.c().shutdown_decision();
    assert!(
        is_refused_with(&decision, Unresolved::TerminalPending(terminal.record)),
        "[terminal-ack] {decision:?}"
    );
    assert!(
        lab.close().is_none(),
        "[terminal-ack] closed with the terminal record pending"
    );

    lab.ack_all();
    let terminal_intent = lab
        .journal
        .submitted
        .iter()
        .find(|intent| intent.id == terminal.record)
        .cloned()
        .expect("submitted");
    for _ in 0..3 {
        let now = lab.now();
        assert_eq!(
            lab.c().acknowledge(&RecordAck::of(&terminal_intent), now),
            AckOutcome::Duplicate
        );
        let now = lab.now();
        assert_eq!(lab.c().observe_control(now), Ok(None));
        let _ = lab.control.status();
    }
    lab.flush();
    let terminals: Vec<_> = lab
        .records()
        .into_iter()
        .filter(|kind| matches!(kind, RecordKind::RunEnded { .. }))
        .collect();
    assert_eq!(
        terminals,
        vec![RecordKind::RunEnded {
            verdict: Verdict::Passed,
            resolved_by: None
        }],
        "exactly one terminal record"
    );
    assert!(
        !lab.records()
            .iter()
            .any(|kind| matches!(kind, RecordKind::Control { .. })),
        "a late cancellation is no control fact"
    );
    let snapshot = lab.snap();
    assert_eq!(
        (snapshot.phase, snapshot.verdict),
        (RunPhase::Finalized, Verdict::Passed)
    );

    // Later owners: a late incident, then (past the bound) returned; faults
    // of the custody beside the unchanged outcome.
    let late = lab.native.produce(&ticket);
    let late_token = late.token;
    let now = lab.now();
    assert!(matches!(
        lab.c().complete(&ticket, NativeOutcome::Created(late), now),
        Ok(Completion::Late { .. })
    ));
    let second = lab.native.produce(&ticket);
    let second_token = second.token;
    let now = lab.now();
    match lab
        .c()
        .complete(&ticket, NativeOutcome::Created(second), now)
    {
        Err(Rejected::CustodyFull { owner }) => {
            assert_eq!(owner.token, second_token);
            lab.returned.push(owner);
        }
        other => panic!("a late owner past the bound: {other:?}"),
    }
    let snapshot = lab.snap();
    assert_eq!(
        (
            snapshot.verdict,
            snapshot.failure_counts,
            fault_count(&snapshot, FailureClass::LateOwner)
        ),
        (Verdict::Passed, [0; FailureClass::COUNT], 2)
    );
    assert_eq!(lab.control.status().control.admission, admission);
    assert_eq!(
        lab.retry(&mut Scripted::always(Plan::Confirm)),
        Response::Retried {
            attempt: 1,
            resolved: true,
            epoch: 1
        }
    );
    assert_eq!(lab.released(), vec![late_token]);
    lab.ack_all();
    let closed = lab.close().expect("closes once nothing is unresolved");
    assert_eq!(
        (closed.terminal, closed.verdict),
        (Some(terminal.record), Verdict::Passed)
    );
    assert!(lab.dropped().is_empty());

    // A failing terminal record (after a late cancellation, and of a run a
    // cancellation failed before its commitment): never final, never
    // closed, its verdict as committed.
    for cancel_first in [false, true] {
        let mut lab = Lab::new(TEST);
        lab.start();
        candidate_before_last_ack(&mut lab);
        if cancel_first {
            let now = lab.now();
            assert!(matches!(
                lab.control.cancel(CancelReason::Requested, now),
                CancelReceipt::Accepted(_)
            ));
        }
        let last = lab.journal.submitted.len();
        lab.ack_until(last);
        lab.flush();
        let (record, verdict) = lab.terminal_record().expect("committed");
        if !cancel_first {
            let now = lab.now();
            assert!(matches!(
                lab.control.cancel(CancelReason::Requested, now),
                CancelReceipt::Late(_)
            ));
        }
        // Acknowledge what precedes the terminal record, then fail it.
        let index = lab
            .journal
            .submitted
            .iter()
            .position(|intent| intent.id == record)
            .expect("submitted");
        lab.ack_until(index);
        let now = lab.now();
        assert_eq!(
            lab.c().record_failed(record, now),
            AckOutcome::FailureRecorded
        );
        let snapshot = lab.snap();
        let expected = if cancel_first {
            (Verdict::Failed, Verdict::Failed)
        } else {
            (Verdict::Passed, Verdict::Pending)
        };
        assert_eq!(
            (verdict, snapshot.verdict),
            expected,
            "cancel first {cancel_first}"
        );
        assert_eq!(snapshot.phase, RunPhase::Finalizing);
        assert_eq!(fault_count(&snapshot, FailureClass::RecordFailed), 1);
        let decision = lab.c().shutdown_decision();
        assert!(
            is_refused_with(&decision, Unresolved::TerminalPending(record)),
            "[terminal-ack] cancel first {cancel_first}: {decision:?}"
        );
        assert!(
            lab.close().is_none(),
            "[terminal-ack] closed with a failed terminal record"
        );
        let admission = lab.control.status().control.admission;
        let now = lab.now();
        assert!(matches!(
            lab.control.cancel(CancelReason::Requested, now),
            CancelReceipt::Late(_)
        ));
        assert_eq!(lab.control.status().control.admission, admission);
        assert_eq!(
            lab.records()
                .iter()
                .filter(|kind| matches!(kind, RecordKind::RunEnded { .. }))
                .count(),
            1
        );
    }
}

/// The commitment point and a concurrent cancellation raced on the gate's
/// critical section. This is supplementary: the ordered controls above are
/// the proof, and nothing here depends on which side wins a round. Whichever
/// does, the receipt and the outcome agree: an accepted cancellation failed
/// the run, a late one changed nothing.
#[test]
fn r12_commitment_and_concurrent_cancellation_agree_whichever_wins() {
    let (mut accepted, mut late) = (0, 0);
    for _ in 0..64 {
        let mut lab = Lab::new(TEST);
        lab.start();
        lab.begin(1, Expectation::Clean);
        assert!(lab.end_case(&mut Scripted::always(Plan::Confirm)).passed);
        let now = lab.now();
        assert_eq!(lab.c().finish_run(now), Ok(RunPhase::Candidate));
        lab.flush();
        let last = lab.journal.submitted.len();
        lab.ack_until(last - 1);
        let ack = RecordAck::of(&lab.journal.submitted[last - 1]);
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
        assert_eq!(lab.c().acknowledge(&ack, now), AckOutcome::Acknowledged);
        lab.acked += 1;
        let receipt = canceller.join().expect("the canceller");
        lab.ack_all();
        let snapshot = lab.snap();
        let admission = lab.control.status().control.admission;
        let verdict = lab.terminal_record().map(|(_, verdict)| verdict);
        match receipt {
            CancelReceipt::Accepted(closure) => {
                accepted += 1;
                assert_eq!(
                    (
                        verdict,
                        snapshot.verdict,
                        count(&snapshot, FailureClass::Cancelled),
                        admission.closure
                    ),
                    (Some(Verdict::Failed), Verdict::Failed, 1, Some(closure))
                );
            }
            CancelReceipt::Late(commitment) => {
                late += 1;
                assert_eq!(
                    (
                        verdict,
                        snapshot.verdict,
                        snapshot.failure_counts,
                        admission.closure,
                        admission.committed
                    ),
                    (
                        Some(Verdict::Passed),
                        Verdict::Passed,
                        [0; FailureClass::COUNT],
                        None,
                        Some(commitment)
                    )
                );
            }
            CancelReceipt::AlreadyClosed(closure) => {
                panic!("nothing else closed admission: {closure:?}")
            }
        }
        assert!(lab.close().is_some());
    }
    assert_eq!(accepted + late, 64);
}

/// R2 finding B, by failure mode: a retained boundary's first attempt that
/// leaves its output pending, its direct child unreaped, its subtree
/// unconfirmed or nothing confirmed latches the unmet expectation at once
/// (through `end_entry`), closing admission before the next attempt
/// confirms; an adapter panic, a detached output and exhausted cleanup keep
/// their own failures (which close admission first, as before) beside it,
/// each recorded once.
#[test]
fn r13_every_unconfirmed_first_attempt_latches_its_expectation() {
    let latched = |lab: &mut Lab, first: Plan| {
        lab.start();
        lab.begin(1, Expectation::RetainedBoundary);
        let (entry, token) = lab.deposit(SlotKind::Process, true);
        let mut cleanup = Scripted::always(Plan::Confirm).first(SlotKind::Process, &[first]);
        let now = lab.now();
        let ended = lab.c().end_entry(entry, &mut cleanup, now);
        (ended, cleanup.attempts_on(token), token)
    };
    for (first, why) in [
        (Plan::OutputPending, "output still pending"),
        (Plan::SubtreeOnly, "the direct child unreaped"),
        (Plan::ReapedOnly, "the subtree unconfirmed"),
        (Plan::Fail, "nothing confirmed"),
    ] {
        let mut lab = Lab::new(TEST);
        let (ended, attempts, token) = latched(&mut lab, first);
        let snapshot = lab.snap();
        let failures: Vec<FailureClass> = snapshot
            .failures
            .iter()
            .map(|failure| failure.class)
            .collect();
        let closure = lab.control.status().control.admission.closure;
        assert!(
            failures == vec![FailureClass::ExpectedConditionUnmet]
                && closed_by_failure(closure) == Some(FailureClass::ExpectedConditionUnmet),
            "[expectation-latched] {why}: failures {failures:?}, closure {closure:?}"
        );
        assert_eq!((ended, attempts), (Ok(true), 2), "{why}");
        let now = lab.now();
        assert!(
            matches!(
                lab.c().reserve(SlotKind::Workspace, now),
                Err(Refusal::AdmissionClosed(_))
            ),
            "{why}"
        );
        assert_eq!(lab.released(), vec![token], "{why}");
        let outcome = lab.end_case(&mut Scripted::always(Plan::Confirm));
        assert!(!outcome.passed && outcome.stopped, "{why}");
        assert_eq!(
            count(&lab.snap(), FailureClass::ExpectedConditionUnmet),
            1,
            "{why}"
        );
    }

    for (first, class, attempts, why) in [
        (
            Plan::Panic,
            FailureClass::UnexpectedCleanup,
            2,
            "the adapter panicked",
        ),
        (
            Plan::Detached,
            FailureClass::OutputLost,
            1,
            "the output detached",
        ),
    ] {
        let mut lab = Lab::new(TEST);
        let (ended, made, token) = latched(&mut lab, first);
        assert_eq!((ended, made), (Ok(true), attempts), "{why}");
        let snapshot = lab.snap();
        let failures: Vec<FailureClass> = snapshot
            .failures
            .iter()
            .map(|failure| failure.class)
            .collect();
        assert_eq!(
            failures,
            vec![class, FailureClass::ExpectedConditionUnmet],
            "{why}"
        );
        assert_eq!(
            closed_by_failure(lab.control.status().control.admission.closure),
            Some(class),
            "{why}"
        );
        assert_eq!(lab.released(), vec![token], "{why}");
        lab.end_case(&mut Scripted::always(Plan::Confirm));
        let snapshot = lab.snap();
        assert_eq!(
            (
                count(&snapshot, FailureClass::ExpectedConditionUnmet),
                count(&snapshot, class)
            ),
            (1, 1),
            "{why}"
        );
    }

    // Exhausted: the owner stays held (and admission closed) until explicit
    // recovery ends it; each failure once.
    let mut lab = Lab::new(TEST);
    lab.start();
    lab.begin(1, Expectation::RetainedBoundary);
    let (entry, token) = lab.deposit(SlotKind::Process, true);
    let mut failing = Scripted::always(Plan::Fail);
    let now = lab.now();
    assert_eq!(lab.c().end_entry(entry, &mut failing, now), Ok(false));
    assert_eq!(failing.attempts_on(token), 3);
    let snapshot = lab.snap();
    let failures: Vec<FailureClass> = snapshot
        .failures
        .iter()
        .map(|failure| failure.class)
        .collect();
    assert_eq!(
        failures,
        vec![
            FailureClass::ExpectedConditionUnmet,
            FailureClass::UnexpectedCleanup
        ]
    );
    assert_eq!(only_entry(&snapshot).entry, entry);
    let now = lab.now();
    assert!(matches!(
        lab.c().reserve(SlotKind::Workspace, now),
        Err(Refusal::AdmissionClosed(_))
    ));
    let outcome = lab.end_case(&mut failing);
    assert_eq!((outcome.resolved, outcome.stopped), (false, true));
    assert_eq!(lab.snap().phase, RunPhase::RecoveryRequired);
    assert_eq!(lab.snap().failures.len(), 2, "nothing recorded twice");
    assert!(matches!(
        lab.retry(&mut Scripted::always(Plan::Confirm)),
        Response::Retried { resolved: true, .. }
    ));
    assert_eq!(lab.released(), vec![token]);
    assert_eq!(lab.snap().failures.len(), 2);
    assert!(lab.dropped().is_empty());
}

/// R2 finding B's boundary: an expected retained boundary whose first
/// attempt confirms every fact is still a passing control, and admission
/// stays open for its case's further actions; an ordinary owner whose
/// cleanup is pending at first and confirmed within its budget is no
/// failure at all, in a clean case and as a dependency in a retained-boundary
/// case.
#[test]
fn r14_expected_first_attempt_success_and_ordinary_retries_keep_their_semantics() {
    // The expected first-attempt success, through `end_entry`.
    let mut lab = Lab::new(TEST);
    lab.start();
    lab.begin(1, Expectation::RetainedBoundary);
    let (entry, token) = lab.deposit(SlotKind::Process, true);
    let now = lab.now();
    assert_eq!(
        lab.c()
            .end_entry(entry, &mut Scripted::always(Plan::Confirm), now),
        Ok(true)
    );
    let snapshot = lab.snap();
    assert_eq!(
        snapshot
            .case
            .as_ref()
            .map(|case| (case.retained_first_confirmed, case.failed)),
        Some((Some(true), false))
    );
    assert_eq!(snapshot.failure_counts, [0; FailureClass::COUNT]);
    assert_eq!(lab.control.status().control.admission.closure, None);
    assert_eq!(lab.released(), vec![token]);
    let (_, workspace) = lab.create(SlotKind::Workspace);
    let outcome = lab.end_case(&mut Scripted::always(Plan::Confirm));
    assert!(outcome.passed && !outcome.stopped);
    assert_eq!(lab.released(), vec![workspace]);
    assert_eq!(lab.finish(), RunPhase::Finalized);
    assert_eq!(lab.snap().verdict, Verdict::Passed);

    // Ordinary cleanup, pending then confirmed, in a clean case.
    let mut lab = Lab::new(TEST);
    lab.start();
    lab.begin(1, Expectation::Clean);
    let (entry, token) = lab.create(SlotKind::Process);
    let mut cleanup = Scripted::always(Plan::Confirm).first(SlotKind::Process, &[Plan::Fail]);
    let now = lab.now();
    assert_eq!(lab.c().end_entry(entry, &mut cleanup, now), Ok(true));
    assert_eq!(cleanup.attempts_on(token), 2);
    assert_eq!(lab.snap().failure_counts, [0; FailureClass::COUNT]);
    assert_eq!(lab.control.status().control.admission.closure, None);
    lab.released();
    lab.create(SlotKind::Fixture);
    assert!(lab.end_case(&mut Scripted::always(Plan::Confirm)).passed);
    assert_eq!(lab.finish(), RunPhase::Finalized);
    assert_eq!(lab.snap().verdict, Verdict::Passed);

    // A dependency's ordinary retries in a retained-boundary case.
    let mut lab = Lab::new(TEST);
    lab.start();
    lab.begin(1, Expectation::RetainedBoundary);
    let (workspace, _) = lab.create(SlotKind::Workspace);
    let (entry, _) = lab.deposit(SlotKind::Process, true);
    let now = lab.now();
    assert_eq!(
        lab.c()
            .end_entry(entry, &mut Scripted::always(Plan::Confirm), now),
        Ok(true)
    );
    let mut cleanup = Scripted::always(Plan::Confirm).first(SlotKind::Workspace, &[Plan::Fail]);
    let now = lab.now();
    assert_eq!(lab.c().end_entry(workspace, &mut cleanup, now), Ok(true));
    assert_eq!(lab.snap().failure_counts, [0; FailureClass::COUNT]);
    assert_eq!(lab.control.status().control.admission.closure, None);
    assert!(lab.end_case(&mut Scripted::always(Plan::Confirm)).passed);
    assert_eq!(lab.released().len(), 2);
    assert!(lab.dropped().is_empty());
}

/// R2, the shutdown route against the commitment point: a shutdown request
/// cancels (through the same gate, so necessarily before the commitment)
/// only while the run is running. To a completion candidate it only answers:
/// it neither cancels, commits, finalizes nor fails it, and the run then
/// commits through the one commitment point; after the commitment it
/// changes nothing.
#[test]
fn r15_shutdown_request_is_no_second_finalization_rule() {
    let mut lab = Lab::new(TEST);
    lab.start();
    candidate_before_last_ack(&mut lab);
    let decision = lab.request_shutdown();
    assert!(
        is_refused_with(&decision, Unresolved::RunEnding),
        "{decision:?}"
    );
    let snapshot = lab.snap();
    let admission = lab.control.status().control.admission;
    assert!(
        admission.closure.is_none()
            && admission.committed.is_none()
            && snapshot.phase == RunPhase::Candidate
            && snapshot.verdict == Verdict::Pending
            && snapshot.failure_counts == [0; FailureClass::COUNT],
        "[shutdown-candidate] a shutdown request decided a completion candidate's outcome: \
         admission {admission:?}, phase {:?}, published {:?}, failures {:?}",
        snapshot.phase,
        snapshot.verdict,
        snapshot.failure_counts
    );
    lab.ack_all();
    assert_eq!(
        lab.terminal_record().map(|(_, verdict)| verdict),
        Some(Verdict::Passed)
    );
    assert_eq!(lab.request_shutdown(), ShutdownDecision::Permitted);
    assert_eq!(
        lab.close().map(|closed| closed.verdict),
        Some(Verdict::Passed)
    );

    // After the commitment, before the terminal acknowledgement.
    let mut lab = Lab::new(TEST);
    lab.start();
    candidate_before_last_ack(&mut lab);
    let last = lab.journal.submitted.len();
    lab.ack_until(last);
    let admission = lab.control.status().control.admission;
    assert!(admission.committed.is_some());
    let decision = lab.request_shutdown();
    assert!(
        is_refused_with(&decision, Unresolved::EvidencePending(1)),
        "{decision:?}"
    );
    assert_eq!(lab.control.status().control.admission, admission);
    lab.ack_all();
    assert_eq!(lab.snap().verdict, Verdict::Passed);

    // While running: a cancellation through the same gate.
    let mut lab = Lab::new(TEST);
    lab.start();
    lab.begin(1, Expectation::Clean);
    lab.create(SlotKind::Process);
    assert!(matches!(
        lab.request_shutdown(),
        ShutdownDecision::Refused(_)
    ));
    let closure = lab
        .control
        .status()
        .control
        .admission
        .closure
        .expect("a running run's shutdown request closes admission");
    assert_eq!(
        closure.reason,
        ClosureReason::Cancelled(CancelReason::Shutdown)
    );
    let outcome = lab.end_case(&mut Scripted::always(Plan::Confirm));
    assert!(outcome.stopped);
    lab.released();
    let (failed, capture) = cancelled_outcome(&mut lab, closure);
    assert!(failed, "{capture}");
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
/// only (and is an actual failure).
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
        (false, true, true)
    );
    assert_eq!(cleanup.attempts_on(token), 2);
    assert_eq!(lab.released(), vec![token], "the same owner is released");
    let snapshot = lab.snap();
    assert_eq!(count(&snapshot, FailureClass::UnexpectedCleanup), 1);
    assert_eq!(count(&snapshot, FailureClass::Assertion), 1);
}

/// H3. A later explicit recovery resolves the run without erasing the
/// earlier failure, and the run is final only once its evidence, ending in
/// its terminal record, is acknowledged.
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
        (RunPhase::Candidate, Verdict::Failed),
        "recovery never turns a failure into a pass"
    );
    assert_eq!(snapshot.recovery.resolved_by, Some(2));
    assert_eq!(count(&snapshot, FailureClass::UnexpectedCleanup), 1);
    assert!(snapshot.failures.iter().any(|failure| {
        failure.class == FailureClass::UnexpectedCleanup && failure.case == Some(CaseId(1))
    }));
    let decision = lab.c().shutdown_decision();
    assert!(
        is_refused_with(&decision, Unresolved::RunEnding),
        "{decision:?}"
    );

    lab.ack_all();
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
    let snapshot = lab.snap();
    assert_eq!(snapshot.phase, RunPhase::Finalized);
    assert_eq!(
        snapshot.evidence.reserved, 2,
        "only the two unused recovery attempts stay reserved"
    );
    assert_eq!(lab.c().shutdown_decision(), ShutdownDecision::Permitted);
    let closed = lab.close().expect("closes once resolved and durable");
    assert_eq!(
        (closed.verdict, closed.resolved_by, closed.terminal),
        (
            Verdict::Failed,
            Some(2),
            lab.terminal_record().map(|(id, _)| id)
        )
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
        Some(ClosureReason::Cancelled(CancelReason::Shutdown)),
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
    assert!(
        is_refused_with(&decision, Unresolved::RunEnding),
        "{decision:?}"
    );
    lab.ack_all();
    assert_eq!(lab.request_shutdown(), ShutdownDecision::Permitted);
    let closed = lab.close().expect("closes once nothing is unresolved");
    assert_eq!(closed.terminal, lab.terminal_record().map(|(id, _)| id));
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
    let CancelReceipt::Accepted(closure) = receipt else {
        panic!("the first cancellation before the commitment is accepted: {receipt:?}");
    };
    assert_eq!(
        closure,
        Closure {
            reason: ClosureReason::Cancelled(CancelReason::Requested),
            at: now,
            after: 0
        }
    );

    let now = lab.now();
    let refused = lab
        .c()
        .admit(reservation, now)
        .expect_err("admission is closed");
    assert_eq!(refused.refusal, Refusal::AdmissionClosed(closure));
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
            closure: Some(closure),
            committed: None
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
        CancelReceipt::AlreadyClosed(closure)
    );
    let now = lab.now();
    assert_eq!(
        lab.c().reserve(SlotKind::Workspace, now).unwrap_err(),
        Refusal::AdmissionClosed(closure)
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
            closure: Some(closure),
            committed: None
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
        let CancelReceipt::Accepted(closure) = receipt else {
            panic!("a running run's cancellation is accepted: {receipt:?}");
        };
        match result {
            Ok(ticket) => {
                admitted += 1;
                assert_eq!(
                    closure.after, 1,
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
                assert_eq!(refusal.refusal, Refusal::AdmissionClosed(closure));
                assert_eq!(closure.after, 0);
                assert!(lab.native.produced.is_empty());
            }
        }
        assert_eq!(
            lab.control.status().control.admission.admitted,
            closure.after
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
    assert!(
        matches!(receipt, CancelReceipt::Accepted(closure) if closure.after == 1),
        "the admitted action precedes the closure: {receipt:?}"
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
        (RunPhase::Candidate, Verdict::Failed)
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
        (ClosureReason::Cancelled(CancelReason::LeaseLost), deadline)
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
        RunPhase::Candidate,
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
    assert!(matches!(receipt, CancelReceipt::Accepted(_)), "{receipt:?}");

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
/// fact, complete output included; a confirmation by a later attempt does
/// not satisfy it, and the expected condition itself is never a failure. A
/// passing control lets the next case begin only once its evidence is
/// acknowledged.
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
        snapshot.failure_counts,
        [0; FailureClass::COUNT],
        "the expected condition is not a failure"
    );
    assert_eq!(
        lab.control.status().control.admission.closure,
        None,
        "an expected condition does not close admission"
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
    assert_eq!(lab.control.status().control.admission.closure, None);
    let now = lab.now();
    assert_eq!(
        lab.c().begin_case(CaseId(2), Expectation::Clean, now),
        Err(Refusal::EvidencePending),
        "the next case waits for the control's evidence"
    );
    lab.ack_all();
    let now = lab.now();
    assert_eq!(
        lab.c().begin_case(CaseId(2), Expectation::Clean, now),
        Ok(())
    );
    let outcome = lab.end_case(&mut Scripted::always(Plan::Confirm));
    assert!(outcome.passed);
    assert_eq!(lab.finish(), RunPhase::Finalized);
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
        assert_eq!(
            (outcome.passed, outcome.resolved, outcome.stopped),
            (false, true, true),
            "{why}"
        );
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

    // A first attempt that detaches the output does not confirm every fact.
    let mut lab = Lab::new(TEST);
    lab.start();
    lab.begin(1, Expectation::RetainedBoundary);
    let (_, token) = lab.deposit(SlotKind::Process, true);
    let outcome = lab.end_case(&mut Scripted::always(Plan::Detached));
    assert!(!outcome.passed && outcome.resolved);
    assert_eq!(lab.released(), vec![token]);
    let snapshot = lab.snap();
    assert_eq!(count(&snapshot, FailureClass::ExpectedConditionUnmet), 1);
    assert_eq!(count(&snapshot, FailureClass::OutputLost), 1);

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
        (RunPhase::Candidate, Verdict::Failed)
    );
    assert_eq!(lab.released(), vec![token]);
    lab.ack_all();
    assert_eq!(
        lab.records().last(),
        Some(&RecordKind::RunEnded {
            verdict: Verdict::Failed,
            resolved_by: Some(1)
        })
    );
    assert_eq!(lab.snap().phase, RunPhase::Finalized);
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
/// bring is a late incident (or returned past the bound), never dropped.
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
        Ok(Completion::Late { .. })
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
        Ok(Completion::Late { .. })
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
    assert_eq!(
        count(&snapshot, FailureClass::LateOwner),
        3,
        "two held late owners and one returned are all failures"
    );

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

    let exact = Config {
        record_capacity: TEST.control_reserve + needed,
        ..TEST
    };
    let mut lab = Lab::new(exact);
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
    let snapshot = lab.snap();
    assert_eq!(
        general_issued(&snapshot, &exact) + u64::from(snapshot.evidence.reserved),
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
    lab.ack_all();
    let snapshot = lab.snap();
    assert_eq!(
        (
            general_issued(&snapshot, &exact),
            snapshot.evidence.reserved
        ),
        (u64::from(needed), 0),
        "every record was recorded within capacity"
    );
    assert_eq!(
        lab.records().last(),
        Some(&RecordKind::RunEnded {
            verdict: Verdict::Failed,
            resolved_by: Some(last)
        })
    );
    assert_eq!(snapshot.phase, RunPhase::Finalized);
}

/// H15. Physical cleanup and recording failure stay distinct: the owner's
/// end stays confirmed and the owner is not recreated, the settlement stays
/// not durable, admission stays closed, and closure is refused because
/// required evidence is unresolved.
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
        (snapshot.phase, snapshot.verdict, snapshot.terminal),
        (RunPhase::Candidate, Verdict::Failed, None),
        "no terminal record after failed evidence"
    );
    assert_eq!(
        closed_by_failure(lab.control.status().control.admission.closure),
        Some(FailureClass::RecordFailed)
    );
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
    let decision = lab.c().shutdown_decision();
    assert!(
        is_refused_with(
            &decision,
            Unresolved::EvidenceFailed {
                record: settlement.id,
                at: snapshot.evidence.failed.map(|(_, at)| at).expect("failed")
            }
        ),
        "[evidence-close] {decision:?}"
    );
    assert!(
        lab.close().is_none(),
        "[evidence-close] closed while required evidence failed"
    );
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
    assert!(lab.c().observe_control(now).is_ok());
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
    assert!(matches!(
        lab.retry(&mut failing),
        Response::Retried {
            resolved: false,
            ..
        }
    ));
    lab.flush();
    let next = lab.journal.submitted[lab.acked].id;
    let now = lab.now();
    assert_eq!(
        lab.c().record_failed(next, now),
        AckOutcome::FailureRecorded
    );
    held(&lab, "a recording failure");
    assert!(lab.close().is_none());
    held(&lab, "a refused close after failed evidence");
}

/// The adversarial events: every way the control side, the native
/// stand-in, the recorder and the case can act next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Event {
    Admit,
    /// Admit a process action in a retained-boundary case (begun if none is
    /// active).
    AdmitRetained,
    Cancel,
    /// The lease's deadline passes and the control side observes it.
    LeaseExpire,
    ObserveControl,
    Timeout,
    Late,
    /// The open operation's own finalization leaves a retained owner.
    Retained,
    LateNone,
    LateDetached,
    /// End the first held owner now: its first attempt confirms nothing, the
    /// next everything.
    EndEntry,
    EndFail,
    EndOk,
    EndDetached,
    Assert,
    StaleAck,
    /// Acknowledge only the next record.
    AckOne,
    RecordFail,
    RetryOk,
    RetryFail,
    AckAll,
    Shutdown,
    Close,
}

const EVENTS: [Event; 23] = [
    Event::Admit,
    Event::AdmitRetained,
    Event::Cancel,
    Event::LeaseExpire,
    Event::ObserveControl,
    Event::Timeout,
    Event::Late,
    Event::Retained,
    Event::LateNone,
    Event::LateDetached,
    Event::EndEntry,
    Event::EndFail,
    Event::EndOk,
    Event::EndDetached,
    Event::Assert,
    Event::StaleAck,
    Event::AckOne,
    Event::RecordFail,
    Event::RetryOk,
    Event::RetryFail,
    Event::AckAll,
    Event::Shutdown,
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
    late_limit: 1,
    failure_detail_limit: 4,
    detail_chars: 32,
    incident_limit: 0,
};

/// States a sequence reached (to prove the sequences exercise them).
#[derive(Debug, Default, Clone, Copy)]
struct Coverage {
    adopted_after_cancel: bool,
    unknown_resolved: bool,
    late_held: bool,
    late_returned: bool,
    record_failed: bool,
    recovered: bool,
    fail_stop_refused: bool,
    next_case_refused: bool,
    post_terminal_refused: bool,
    late_after_terminal: bool,
    output_lost: bool,
    evidence_close_refused: bool,
    closed: bool,
    cancel_before_commit: bool,
    lease_before_commit: bool,
    cancel_late: bool,
    expectation_latched: bool,
    retained_confirmed: bool,
}

/// The oracle's own facts, taken from what the custody published (its
/// snapshots, its records and its answers), never from its phases.
#[derive(Debug, Default)]
struct Oracle {
    /// Admissions and the case when an actual failure was first published.
    failed_at: Option<(u64, Option<CaseId>)>,
    /// The acknowledged terminal record and the verdict it carries.
    terminal: Option<(RecordId, Verdict)>,
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
    oracle: Oracle,
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
            oracle: Oracle::default(),
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
        let resolving = bound
            && matches!(
                event,
                Event::Late | Event::Retained | Event::LateNone | Event::LateDetached
            );
        match event {
            Event::Admit => self.admit(Expectation::Clean, None),
            Event::AdmitRetained => {
                self.admit(Expectation::RetainedBoundary, Some(SlotKind::Process))
            }
            Event::Cancel => self.cancel(),
            Event::LeaseExpire => self.expire_lease(),
            Event::ObserveControl => {
                let now = self.lab.now();
                self.lab
                    .c()
                    .observe_control(now)
                    .expect("an instant after every earlier one");
            }
            Event::Timeout => self.complete(|_, _| NativeOutcome::Unknown),
            Event::Late => {
                self.complete(|native, ticket| NativeOutcome::Created(native.produce(ticket)))
            }
            Event::Retained => {
                self.complete(|native, ticket| NativeOutcome::Retained(native.produce(ticket)))
            }
            Event::LateNone => self.complete(|_, ticket| {
                NativeOutcome::NoEffect(NoEffectProof::for_ticket(ticket, "late refusal"))
            }),
            Event::LateDetached => self.complete(|_, _| NativeOutcome::Ended(DETACHED_END)),
            Event::EndEntry => self.end_entry(),
            Event::EndFail => self.end(Plan::Fail),
            Event::EndOk => self.end(Plan::Confirm),
            Event::EndDetached => self.end(Plan::Detached),
            Event::Assert => self.assert(),
            Event::StaleAck => self.stale_ack(),
            Event::AckOne => self.ack_one(),
            Event::RecordFail => self.record_fail(),
            Event::RetryOk => self.request(RequestOp::Retry { epoch: 0 }, Plan::Confirm),
            Event::RetryFail => self.request(RequestOp::Retry { epoch: 0 }, Plan::Fail),
            Event::AckAll => self.lab.try_ack_all(),
            Event::Shutdown => self.request(RequestOp::Shutdown, Plan::Fail),
            Event::Close => self.close(),
        }
        if self.closed.is_none() {
            self.lab.released();
        }
        self.check(event, resolving, open);
    }

    fn admit(&mut self, expectation: Expectation, kind: Option<SlotKind>) {
        let kinds = [SlotKind::Process, SlotKind::Workspace, SlotKind::Fixture];
        let kind = kind.unwrap_or(kinds[self.tickets.len() % kinds.len()]);
        let failed = self.oracle.failed_at.is_some();
        if self.lab.snap().case.is_none() {
            self.lab.try_ack_all();
            self.cases += 1;
            let now = self.lab.now();
            let case = CaseId(self.cases);
            if self.lab.c().begin_case(case, expectation, now).is_err() {
                self.coverage.next_case_refused |= failed;
                return;
            }
        }
        let now = self.lab.now();
        let Ok(reservation) = self.lab.c().reserve(kind, now) else {
            self.coverage.fail_stop_refused |= failed;
            return;
        };
        self.lab.try_ack_all();
        let now = self.lab.now();
        match self.lab.c().admit(reservation, now) {
            Ok(ticket) => self.tickets.push(ticket),
            Err(_) => self.coverage.fail_stop_refused |= failed,
        }
    }

    /// A cancellation, and its receipt checked against the control facts
    /// published just before it: late once committed, else the closure
    /// already made, else a new one.
    fn cancel(&mut self) {
        let before = self.lab.control.status();
        let now = self.lab.now();
        let receipt = self.lab.control.cancel(CancelReason::Requested, now);
        let gate = before.control.admission;
        let expected = match (gate.committed, gate.closure) {
            (Some(commitment), _) => CancelReceipt::Late(commitment),
            (None, Some(closure)) => CancelReceipt::AlreadyClosed(closure),
            (None, None) => CancelReceipt::Accepted(Closure {
                reason: ClosureReason::Cancelled(CancelReason::Requested),
                at: now,
                after: gate.admitted,
            }),
        };
        assert_eq!(
            receipt, expected,
            "{:?}: the receipt does not match the gate",
            self.trail
        );
        let phase = before.snapshot.phase;
        match receipt {
            CancelReceipt::Accepted(_) => {
                self.coverage.cancel_before_commit |= phase == RunPhase::Candidate
            }
            CancelReceipt::Late(_) => self.coverage.cancel_late |= phase == RunPhase::Finalizing,
            CancelReceipt::AlreadyClosed(_) => {}
        }
    }

    /// The lease's deadline passes and is observed: before the commitment,
    /// an active lease's loss closes an open gate as a cancellation; after
    /// it, nothing but the lease changes.
    fn expire_lease(&mut self) {
        let before = self.lab.control.status();
        let now = self.lab.later(ADVERSARIAL.lease_millis);
        let state = self.lab.control.check_lease(now);
        let gate = before.control.admission;
        let active = matches!(before.control.lease.state, LeaseState::Active { .. });
        assert!(matches!(state, LeaseState::Lost { .. }), "{:?}", self.trail);
        let closes = active && gate.committed.is_none() && gate.closure.is_none();
        let expected = if closes {
            Some(Closure {
                reason: ClosureReason::Cancelled(CancelReason::LeaseLost),
                at: now,
                after: gate.admitted,
            })
        } else {
            gate.closure
        };
        let after = self.lab.control.status().control.admission;
        assert_eq!(
            (after.closure, after.committed),
            (expected, gate.committed),
            "{:?}: a lease expiry closed admission wrongly",
            self.trail
        );
        self.coverage.lease_before_commit |= closes && before.snapshot.phase == RunPhase::Candidate;
    }

    /// End the first held owner through `end_entry`.
    fn end_entry(&mut self) {
        let first = self
            .lab
            .snap()
            .entries
            .first()
            .map(|view| (view.entry, view.kind));
        let Some((entry, kind)) = first else {
            return;
        };
        let mut cleanup = Scripted::always(Plan::Confirm).first(kind, &[Plan::Fail]);
        let now = self.lab.now();
        let _ = self.lab.c().end_entry(entry, &mut cleanup, now);
    }

    /// Acknowledge the next record only, if any is issued and acceptable.
    fn ack_one(&mut self) {
        self.lab.flush();
        let Some(ack) = self
            .lab
            .journal
            .submitted
            .get(self.lab.acked)
            .map(RecordAck::of)
        else {
            return;
        };
        let now = self.lab.now();
        if self.lab.c().acknowledge(&ack, now) == AckOutcome::Acknowledged {
            self.lab.acked += 1;
        }
    }

    fn complete(&mut self, make: impl FnOnce(&mut Native, &OpTicket) -> NativeOutcome<Witness>) {
        let Some(ticket) = self.tickets.last() else {
            return;
        };
        let outcome = make(&mut self.lab.native, ticket);
        let now = self.lab.now();
        let custody = self.lab.custody.as_mut().expect("the custody is open");
        match custody.complete(ticket, outcome, now) {
            Ok(Completion::Late { .. }) => {
                self.coverage.late_held = true;
                self.coverage.late_after_terminal |= self.oracle.terminal.is_some();
            }
            Ok(Completion::OutputLost) => self.coverage.output_lost = true,
            Ok(_) => {}
            Err(rejected) => {
                if matches!(rejected, Rejected::CustodyFull { .. }) {
                    self.coverage.late_returned = true;
                }
                if let Some(owner) = rejected.into_owner() {
                    self.lab.returned.push(owner);
                }
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

    fn assert(&mut self) {
        let now = self.lab.now();
        let result = self.lab.c().record_assertion_failure("adversarial", now);
        if self.oracle.terminal.is_some() {
            assert!(
                result.is_err(),
                "{:?}: an assertion was accepted after the terminal record was acknowledged",
                self.trail
            );
            self.coverage.post_terminal_refused = true;
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
            if self.lab.c().record_failed(id, now) == AckOutcome::FailureRecorded {
                self.coverage.record_failed = true;
            }
        }
    }

    fn request(&mut self, op: RequestOp, plan: Plan) {
        let mut cleanup = Scripted::always(plan);
        self.lab.seq += 1;
        let op = match op {
            RequestOp::Retry { .. } => RequestOp::Retry {
                epoch: self.lab.snap().recovery.epoch,
            },
            RequestOp::Shutdown => RequestOp::Shutdown,
        };
        let request = Request {
            generation: GENERATION,
            seq: self.lab.seq,
            op,
        };
        let now = self.lab.later(ADVERSARIAL.recovery_spacing_millis);
        let outcome = self.lab.serve(request, &mut cleanup, now);
        assert!(
            matches!(outcome, RequestOutcome::Executed(_)),
            "{:?}: a fresh request was not executed: {outcome:?}",
            self.trail
        );
        if matches!(
            outcome,
            RequestOutcome::Executed(Response::Retried { resolved: true, .. })
        ) {
            self.coverage.recovered = true;
        }
    }

    fn close(&mut self) {
        let before = self.lab.snap();
        let refused = matches!(
            self.lab.c().shutdown_decision(),
            ShutdownDecision::Refused(_)
        );
        let evidence_unresolved = before.evidence.failed.is_some()
            || before.evidence.acknowledged < before.evidence.issued;
        match self.lab.close() {
            Some(closed) => {
                let trail = &self.trail;
                assert!(!refused, "{trail:?}: closed while refused");
                assert!(
                    !evidence_unresolved,
                    "{trail:?}: closed with evidence pending or failed"
                );
                assert!(
                    before.entries.is_empty() && before.operation.is_none(),
                    "{trail:?}: closed holding native state"
                );
                // The closed result reports exactly the acknowledged
                // terminal record.
                assert_eq!(
                    closed.terminal,
                    self.oracle.terminal.map(|(record, _)| record),
                    "{trail:?}"
                );
                assert_eq!(
                    closed.verdict,
                    self.oracle
                        .terminal
                        .map_or(Verdict::Pending, |(_, verdict)| verdict),
                    "{trail:?}"
                );
                assert_eq!(closed.records, before.evidence.issued, "{trail:?}");
                self.closed = Some(closed);
                self.coverage.closed = true;
            }
            None => {
                assert!(refused, "{:?}: not closed while permitted", self.trail);
                self.coverage.evidence_close_refused |= before.evidence.failed.is_some();
            }
        }
    }

    fn check(&mut self, event: Event, resolving: bool, open: Option<ActionId>) {
        let known_terminal = self.oracle.terminal;
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

        // Admission: one ticket per admission, the closure never changes and
        // nothing is admitted after it.
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
        assert_eq!(
            control.admission.admitted,
            self.tickets.len() as u64,
            "{trail:?}: an admission without exactly one ticket"
        );

        // The terminal commitment: made with the terminal record, never
        // changed, and nothing closes admission after it.
        if let Some(commitment) = self.last_control.admission.committed {
            assert_eq!(
                control.admission.committed,
                Some(commitment),
                "{trail:?}: the commitment changed"
            );
            assert_eq!(
                control.admission.closure, self.last_control.admission.closure,
                "{trail:?}: admission closed after the commitment"
            );
        }
        assert_eq!(
            control.admission.committed.is_some(),
            now.terminal.is_some(),
            "{trail:?}: a commitment without its terminal record, or the reverse"
        );

        // Fail-stop: from the first published actual failure on, nothing more
        // is admitted and no other case begins; a recording failure after the
        // commitment is a fault, and the commitment already ended admission.
        let failed_now = now.failure_counts.iter().any(|count| *count > 0);
        if now.evidence.failed.is_some() {
            assert!(
                control.admission.closure.is_some() || control.admission.committed.is_some(),
                "{trail:?}: failed evidence left admission open"
            );
        }
        match self.oracle.failed_at {
            Some((admitted, case)) => {
                assert_eq!(
                    control.admission.admitted, admitted,
                    "{trail:?}: admitted after an actual failure"
                );
                let current = now.case.as_ref().map(|view| view.case);
                assert!(
                    current.is_none() || current == case,
                    "{trail:?}: a case began after an actual failure"
                );
                assert!(
                    control.admission.closure.is_some(),
                    "{trail:?}: an actual failure left admission open"
                );
            }
            None if failed_now => {
                assert!(
                    control.admission.closure.is_some(),
                    "{trail:?}: an actual failure left admission open"
                );
                self.oracle.failed_at = Some((
                    control.admission.admitted,
                    now.case.as_ref().map(|view| view.case),
                ));
            }
            None => {}
        }

        // Failures and faults only accumulate, and a failed verdict stays.
        if before.verdict == Verdict::Failed {
            assert_eq!(
                now.verdict,
                Verdict::Failed,
                "{trail:?}: a failed verdict changed"
            );
        }
        for (now_counts, before_counts) in [
            (&now.failure_counts, &before.failure_counts),
            (&now.fault_counts, &before.fault_counts),
        ] {
            assert!(
                now_counts
                    .iter()
                    .zip(before_counts.iter())
                    .all(|(now, before)| now >= before),
                "{trail:?}: a failure count fell"
            );
        }
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
        assert!(
            general_issued(now, &ADVERSARIAL) + u64::from(evidence.reserved)
                <= u64::from(ADVERSARIAL.record_capacity - ADVERSARIAL.control_reserve),
            "{trail:?}: evidence beyond capacity"
        );
        if before.evidence.failed.is_some() {
            assert_eq!(
                evidence.failed, before.evidence.failed,
                "{trail:?}: a recording failure changed"
            );
        }

        // Records, as the recorder received them.
        if self.closed.is_none() {
            self.lab.flush();
        }
        let records = &self.lab.journal.submitted;
        let acknowledged = self.lab.snap().evidence.acknowledged as usize;
        let terminals: Vec<(usize, RecordId, Verdict)> = records
            .iter()
            .enumerate()
            .filter_map(|(index, intent)| match intent.kind {
                RecordKind::RunEnded { verdict, .. } => Some((index, intent.id, verdict)),
                _ => None,
            })
            .collect();
        assert!(
            terminals.len() <= 1,
            "{trail:?}: more than one terminal record"
        );
        if let Some(&(index, record, verdict)) = terminals.first() {
            // No case or action starts after the terminal record, and a run
            // with an earlier actual failure or output loss never ends passed.
            assert!(
                records[index + 1..].iter().all(|intent| !matches!(
                    intent.kind,
                    RecordKind::CaseStarted { .. } | RecordKind::ActionStarted { .. }
                )),
                "{trail:?}: work after the terminal record"
            );
            let lost = records[..index].iter().any(|intent| {
                matches!(
                    intent.kind,
                    RecordKind::ActionSettled {
                        how: Settlement::OutputLost,
                        ..
                    } | RecordKind::IncidentOpened { .. }
                        | RecordKind::ActionFailed { .. }
                )
            });
            if lost {
                assert_eq!(
                    verdict,
                    Verdict::Failed,
                    "{trail:?}: a failure or output loss before the terminal record, yet it passed"
                );
            }
            if index < acknowledged {
                match self.oracle.terminal {
                    Some(known) => assert_eq!(known, (record, verdict), "{trail:?}"),
                    None => self.oracle.terminal = Some((record, verdict)),
                }
            }
        }
        // A settled action's lifecycle ends with its settlement; an incident
        // has its own opening record and identity.
        let mut settled: Vec<ActionId> = Vec::new();
        let mut opened: Vec<IncidentId> = Vec::new();
        for intent in records.iter() {
            match intent.kind {
                RecordKind::ActionSettled { action, .. } | RecordKind::ActionFailed { action } => {
                    assert!(
                        !settled.contains(&action),
                        "{trail:?}: a settled action's lifecycle was reused"
                    );
                    if matches!(intent.kind, RecordKind::ActionSettled { .. }) {
                        settled.push(action);
                    }
                }
                RecordKind::IncidentOpened { incident, .. } => {
                    assert!(!opened.contains(&incident), "{trail:?}: incident reused");
                    opened.push(incident);
                }
                RecordKind::IncidentSettled { incident, .. } => {
                    assert!(opened.contains(&incident), "{trail:?}: unopened incident");
                }
                _ => {}
            }
        }
        let now = self.lab.snap();
        // A closure (made only before the commitment) is part of the
        // committed outcome: a run whose admission closed never ends passed,
        // and a cancellation is its failure exactly once; a published pass
        // means admission never closed.
        let cancelled = count(&now, FailureClass::Cancelled);
        assert!(cancelled <= 1, "{trail:?}: a cancellation counted twice");
        if let (Some(&(_, _, verdict)), Some(closure)) =
            (terminals.first(), control.admission.closure)
        {
            assert_eq!(
                verdict,
                Verdict::Failed,
                "{trail:?}: admission closed before the commitment ({closure:?}), yet the run passed"
            );
            if matches!(closure.reason, ClosureReason::Cancelled(_)) {
                assert_eq!(
                    cancelled, 1,
                    "{trail:?}: a cancellation before the commitment is not the run's failure"
                );
            }
        }
        if now.verdict == Verdict::Passed {
            assert_eq!(
                control.admission.closure, None,
                "{trail:?}: a pass published after admission closed"
            );
        }
        // An expected retained boundary whose first attempt did not confirm
        // every fact failed at that attempt: while its case is still active,
        // the unmet expectation is recorded and admission is closed. An
        // expectation is unmet at most once per case.
        if let Some(case) = now.case.as_ref() {
            if case.expectation == Expectation::RetainedBoundary
                && case.retained_first_confirmed == Some(false)
            {
                assert!(
                    count(&now, FailureClass::ExpectedConditionUnmet) >= 1
                        && case.failed
                        && control.admission.closure.is_some(),
                    "{trail:?}: a failed first attempt left its expectation open"
                );
                self.coverage.expectation_latched = true;
            }
        }
        let mut expecting = 0;
        let mut retained_cases: Vec<CaseId> = Vec::new();
        for intent in records.iter() {
            match intent.kind {
                RecordKind::CaseStarted { case, expectation } => {
                    if expectation != Expectation::Clean {
                        expecting += 1;
                    }
                    if expectation == Expectation::RetainedBoundary {
                        retained_cases.push(case);
                    }
                }
                RecordKind::CaseEnded { case, passed: true } if retained_cases.contains(&case) => {
                    self.coverage.retained_confirmed = true
                }
                _ => {}
            }
        }
        assert!(
            count(&now, FailureClass::ExpectedConditionUnmet) <= expecting,
            "{trail:?}: an expectation unmet twice, or one never declared"
        );
        // The published verdict: a pass only with its acknowledged terminal
        // record, and once that record is acknowledged, exactly its verdict.
        if now.verdict == Verdict::Passed {
            assert_eq!(
                self.oracle.terminal.map(|(_, verdict)| verdict),
                Some(Verdict::Passed),
                "{trail:?}: a pass published without its acknowledged terminal record"
            );
        }
        if let Some((record, verdict)) = self.oracle.terminal {
            assert_eq!(
                now.verdict, verdict,
                "{trail:?}: the finalized verdict changed"
            );
            assert_eq!(
                now.terminal
                    .map(|terminal| (terminal.record, terminal.acknowledged)),
                Some((record, true)),
                "{trail:?}"
            );
        }
        if known_terminal.is_some() {
            assert_eq!(
                now.failure_counts, before.failure_counts,
                "{trail:?}: a run failure after the terminal record was acknowledged"
            );
        }

        // Nothing returns to running once it stopped, and a final run stays
        // final.
        if before.phase != RunPhase::Running && before.phase != RunPhase::NotStarted {
            assert_ne!(now.phase, RunPhase::Running, "{trail:?}: the run reopened");
        }
        if before.phase == RunPhase::Finalized {
            assert!(
                matches!(now.phase, RunPhase::Finalized | RunPhase::Closed),
                "{trail:?}"
            );
        }

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
                        previous.incident,
                        previous.kind,
                        previous.origin,
                        previous.since
                    ),
                    (
                        entry.action,
                        entry.incident,
                        entry.kind,
                        entry.origin,
                        entry.since
                    ),
                    "{trail:?}: a held entry changed identity"
                );
                assert!(entry.automatic_attempts >= previous.automatic_attempts);
                assert!(entry.explicit_attempts >= previous.explicit_attempts);
            }
            assert!(entry.automatic_attempts <= ADVERSARIAL.automatic_attempts);
            assert!(entry.explicit_attempts <= ADVERSARIAL.recovery_budget);
        }
        assert!(now.recovery.attempts <= ADVERSARIAL.recovery_budget);

        // Nothing unresolved, pending or failed may close.
        if self.closed.is_none()
            && (!now.entries.is_empty()
                || now.operation.is_some()
                || !now.lost.is_empty()
                || now.evidence.failed.is_some()
                || now.evidence.acknowledged < now.evidence.issued)
        {
            assert!(
                matches!(
                    self.lab.c().shutdown_decision(),
                    ShutdownDecision::Refused(_)
                ),
                "{trail:?}: shutdown permitted while unresolved"
            );
        }

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
        self.last = now;
        self.last_control = control;
    }
}

/// A witness sequence, and the state it must reach.
type Witnessed = (&'static [Event], fn(&Coverage) -> bool);

/// A completion candidate whose case passed, its evidence pending.
const CANDIDATE_PENDING: &[Event] = &[Event::Admit, Event::LateNone, Event::EndOk, Event::EndOk];

/// The same run committed: its terminal record issued, not acknowledged.
const COMMITTED_PENDING: &[Event] = &[
    Event::Admit,
    Event::LateNone,
    Event::EndOk,
    Event::EndOk,
    Event::AckOne,
    Event::AckOne,
];

/// An expected retained boundary held in its active case.
const RETAINED_HELD: &[Event] = &[Event::AdmitRetained, Event::Retained];

/// A run requiring recovery: its owner's cleanup failed.
const RECOVERY_REQUIRED: &[Event] = &[Event::Admit, Event::Late, Event::EndFail];

/// The finite adversarial transition-sequence control: every sequence of
/// four events (admit, in a clean or a retained-boundary case, cancel, lease
/// expiry, control observation, timeout, late results with an owner, a
/// retained one, without one and with detached output, ending an owner now,
/// failing, confirming and detaching case ends, assertions, stale
/// acknowledgements, single and complete acknowledgement, record failures,
/// failing and confirming retries, shutdown and close), every three-event
/// continuation of four prefixes (a completion candidate with evidence
/// pending, a committed run whose terminal record is pending, an expected
/// retained boundary held, a run requiring recovery), a deterministic sample
/// of longer sequences, and witness sequences proving each guarded state is
/// reached. After every event an independent oracle (from published
/// snapshots, records and answers, not phases) checks ownership, fail-stop,
/// commitment, finalization, evidence, late-incident, expectation and
/// output-loss contracts.
#[test]
fn h18_adversarial_transition_sequences_keep_every_invariant() {
    let witnesses: [Witnessed; 18] = [
        (
            &[Event::Admit, Event::Cancel, Event::Late, Event::EndOk],
            |coverage| coverage.adopted_after_cancel,
        ),
        (
            &[Event::Admit, Event::Timeout, Event::LateNone],
            |coverage| coverage.unknown_resolved,
        ),
        (&[Event::Admit, Event::LateNone, Event::Late], |coverage| {
            coverage.late_held
        }),
        (
            &[Event::Admit, Event::LateNone, Event::Late, Event::Late],
            |coverage| coverage.late_returned,
        ),
        (
            &[Event::Admit, Event::Late, Event::EndOk, Event::RecordFail],
            |coverage| coverage.record_failed,
        ),
        (
            &[Event::Admit, Event::Late, Event::EndFail, Event::RetryOk],
            |coverage| coverage.recovered,
        ),
        (&[Event::Admit, Event::Assert, Event::Admit], |coverage| {
            coverage.fail_stop_refused
        }),
        (
            &[
                Event::Admit,
                Event::Late,
                Event::Assert,
                Event::EndOk,
                Event::Admit,
            ],
            |coverage| coverage.next_case_refused,
        ),
        (
            &[
                Event::Admit,
                Event::Late,
                Event::EndOk,
                Event::EndOk,
                Event::AckAll,
                Event::Assert,
            ],
            |coverage| coverage.post_terminal_refused,
        ),
        (
            &[
                Event::Admit,
                Event::Late,
                Event::EndOk,
                Event::EndOk,
                Event::AckAll,
                Event::Late,
            ],
            |coverage| coverage.late_after_terminal,
        ),
        (
            &[Event::Admit, Event::LateDetached, Event::EndOk],
            |coverage| coverage.output_lost,
        ),
        (
            &[
                Event::Admit,
                Event::Late,
                Event::EndOk,
                Event::RecordFail,
                Event::Close,
            ],
            |coverage| coverage.evidence_close_refused,
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
        (
            &[
                Event::Admit,
                Event::LateNone,
                Event::EndOk,
                Event::EndOk,
                Event::Cancel,
                Event::AckAll,
            ],
            |coverage| coverage.cancel_before_commit,
        ),
        (
            &[
                Event::Admit,
                Event::LateNone,
                Event::EndOk,
                Event::EndOk,
                Event::LeaseExpire,
                Event::AckAll,
            ],
            |coverage| coverage.lease_before_commit,
        ),
        (
            &[
                Event::Admit,
                Event::LateNone,
                Event::EndOk,
                Event::EndOk,
                Event::AckOne,
                Event::AckOne,
                Event::Cancel,
                Event::AckOne,
            ],
            |coverage| coverage.cancel_late,
        ),
        (
            &[
                Event::AdmitRetained,
                Event::Retained,
                Event::EndEntry,
                Event::Admit,
            ],
            |coverage| coverage.expectation_latched && coverage.fail_stop_refused,
        ),
        (
            &[Event::AdmitRetained, Event::Retained, Event::EndOk],
            |coverage| coverage.retained_confirmed && !coverage.expectation_latched,
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

    for prefix in [
        CANDIDATE_PENDING,
        COMMITTED_PENDING,
        RETAINED_HELD,
        RECOVERY_REQUIRED,
    ] {
        for a in EVENTS {
            for b in EVENTS {
                for c in EVENTS {
                    let mut events = prefix.to_vec();
                    events.extend([a, b, c]);
                    Adversary::run(&events);
                }
            }
        }
    }

    let mut state = 0x5eed_c0de_u64;
    for _ in 0..6_000 {
        let mut events = [Event::Admit; 10];
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
/// each (before the run starts), bound to exactly that incident; nothing
/// else can, and the incident's own outcome is kept beside its disposition.
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
    assert_eq!(
        lab.control.status().control.admission.closure,
        None,
        "a validator panic before the run is no run failure"
    );
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
        lab.snap().prior,
        vec![
            PriorView {
                binding: unresolved.binding,
                outcome: unresolved.outcome,
                disposition: Some(DispositionReason::OwnerDestroyed)
            },
            PriorView {
                binding: malformed.binding,
                outcome: malformed.outcome,
                disposition: Some(DispositionReason::RecordsMalformed)
            },
            PriorView {
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
    let now = lab.now();
    assert_eq!(
        lab.c().apply_disposition(&resolved.binding, &bound, now),
        Err(Refusal::Phase(RunPhase::Running)),
        "dispositions belong before the run"
    );
}

/// Failure history is bounded: details up to the bound (escaped and
/// truncated), beyond it only classified and counted; nothing clears it, and
/// a failed case stops the run (its successor is refused) however cleanly
/// its owners were resolved.
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
        (false, true, true)
    );
    lab.ack_all();
    let now = lab.now();
    assert!(matches!(
        lab.c().begin_case(CaseId(2), Expectation::Clean, now),
        Err(Refusal::AdmissionClosed(_))
    ));
    let snapshot = lab.snap();
    assert_eq!(
        (
            snapshot.phase,
            snapshot.cases_passed,
            snapshot.cases_failed,
            snapshot.verdict
        ),
        (RunPhase::Finalized, 0, 1, Verdict::Failed)
    );
    assert_eq!(count(&snapshot, FailureClass::Assertion), 10);
    assert_eq!(snapshot.failure_overflow, 7);
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

/// A phase in which the public API is probed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum At {
    NotStarted,
    RunningCase,
    RunningIdle,
    FailedCase,
    /// A retained-boundary case whose first cleanup attempt failed (ended
    /// through `end_entry`; the case is still active).
    FailedExpectation,
    Recovery,
    Candidate,
    Finalizing,
    Finalized,
}

const PHASES: [At; 9] = [
    At::NotStarted,
    At::RunningCase,
    At::RunningIdle,
    At::FailedCase,
    At::FailedExpectation,
    At::Recovery,
    At::Candidate,
    At::Finalizing,
    At::Finalized,
];

/// One probed public call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Call {
    StartRun,
    BeginCase,
    Reserve,
    Assert,
    EndCase,
    FinishRun,
    Lend,
    Retry,
    Close,
    /// The control side's cancellation, accepted only if it closes the gate.
    Cancel,
}

/// A fresh lab in `at`, with an entry id held there (or held earlier).
fn lab_at(at: At) -> (Lab, Option<EntryId>) {
    let mut lab = Lab::new(TEST);
    let mut entry = None;
    if at != At::NotStarted {
        lab.start();
    }
    if at == At::FailedExpectation {
        lab.begin(1, Expectation::RetainedBoundary);
        entry = Some(lab.deposit(SlotKind::Process, true).0);
    } else if !matches!(at, At::NotStarted | At::RunningIdle) {
        lab.begin(1, Expectation::Clean);
        entry = Some(lab.create(SlotKind::Process).0);
    }
    match at {
        At::NotStarted | At::RunningIdle | At::RunningCase => {}
        At::FailedExpectation => {
            let entry = entry.expect("the retained boundary");
            let mut cleanup =
                Scripted::always(Plan::Confirm).first(SlotKind::Process, &[Plan::Fail]);
            let now = lab.now();
            assert_eq!(lab.c().end_entry(entry, &mut cleanup, now), Ok(true));
        }
        At::FailedCase => {
            let now = lab.now();
            lab.c()
                .record_assertion_failure("an observation failed", now)
                .expect("recorded");
        }
        At::Recovery => {
            lab.end_case(&mut Scripted::always(Plan::Fail));
        }
        At::Candidate | At::Finalizing | At::Finalized => {
            lab.end_case(&mut Scripted::always(Plan::Confirm));
            lab.released();
            let now = lab.now();
            lab.c().finish_run(now).expect("the run finishes");
            match at {
                At::Finalizing => {
                    lab.flush();
                    lab.ack_until(lab.journal.submitted.len());
                }
                At::Finalized => lab.ack_all(),
                _ => {}
            }
        }
    }
    let phase = lab.snap().phase;
    let expected = match at {
        At::NotStarted => RunPhase::NotStarted,
        At::RunningCase | At::RunningIdle | At::FailedCase | At::FailedExpectation => {
            RunPhase::Running
        }
        At::Recovery => RunPhase::RecoveryRequired,
        At::Candidate => RunPhase::Candidate,
        At::Finalizing => RunPhase::Finalizing,
        At::Finalized => RunPhase::Finalized,
    };
    assert_eq!(phase, expected, "{at:?}");
    (lab, entry)
}

/// Whether `call` is accepted in `at`.
fn probe(at: At, call: Call) -> bool {
    let (mut lab, entry) = lab_at(at);
    let before = lab.snap();
    let admission = lab.control.status().control.admission;
    let now = lab.now();
    let accepted = match call {
        Call::StartRun => lab.c().start_run(now).is_ok(),
        Call::BeginCase => lab
            .c()
            .begin_case(CaseId(9), Expectation::Clean, now)
            .is_ok(),
        Call::Reserve => lab.c().reserve(SlotKind::Fixture, now).is_ok(),
        Call::Assert => lab.c().record_assertion_failure("probed", now).is_ok(),
        Call::EndCase => lab
            .c()
            .end_case(&mut Scripted::always(Plan::Confirm), now)
            .is_ok(),
        Call::FinishRun => lab.c().finish_run(now).is_ok(),
        Call::Lend => {
            // An entry from a twin custody of the same generation where this
            // phase never held one: refused by phase, or unknown here.
            let entry = entry.unwrap_or_else(|| {
                let mut twin = Lab::new(TEST);
                twin.start();
                twin.begin(1, Expectation::Clean);
                twin.create(SlotKind::Process).0
            });
            lab.c().lend(entry, now, |owner| owner.token).is_ok()
        }
        Call::Retry => {
            let request = Request {
                generation: GENERATION,
                seq: 1,
                op: RequestOp::Retry { epoch: 0 },
            };
            matches!(
                lab.serve(request, &mut Scripted::always(Plan::Confirm), now),
                RequestOutcome::Executed(Response::Retried { .. })
            )
        }
        Call::Close => lab.close().is_some(),
        Call::Cancel => {
            // Its receipt follows the gate as it stood: late once committed,
            // the earlier closure if one was made, else accepted.
            let receipt = lab.control.cancel(CancelReason::Requested, now);
            let expected = match (admission.committed, admission.closure) {
                (Some(commitment), _) => CancelReceipt::Late(commitment),
                (None, Some(closure)) => CancelReceipt::AlreadyClosed(closure),
                (None, None) => CancelReceipt::Accepted(Closure {
                    reason: ClosureReason::Cancelled(CancelReason::Requested),
                    at: now,
                    after: admission.admitted,
                }),
            };
            assert_eq!(receipt, expected, "{at:?}");
            matches!(receipt, CancelReceipt::Accepted(_))
        }
    };
    if !accepted && lab.custody.is_some() {
        // A refusal changes nothing.
        let after = lab.snap();
        assert_eq!(
            (
                after.phase,
                after.failure_counts,
                after.evidence.issued,
                after.entries.len()
            ),
            (
                before.phase,
                before.failure_counts,
                before.evidence.issued,
                before.entries.len()
            ),
            "{call:?} refused in {at:?} changed the custody"
        );
        assert_eq!(
            lab.control.status().control.admission,
            admission,
            "{call:?} in {at:?}"
        );
    }
    accepted
}

/// The public API's phase matrix, checked call by call in fresh labs: what
/// each phase accepts, and that every refusal changes nothing. Terminal
/// phases refuse every case-level mutation; only closure (once final) and
/// reporting remain. A failed first attempt of an expected retained boundary
/// refuses every further native action of its still-active case. A
/// cancellation closes the gate only before the commitment (in a completion
/// candidate too); after an actual failure it finds the gate closed, after
/// the commitment it is late, and neither changes anything.
#[test]
fn h23_public_api_phase_matrix() {
    use At::*;
    let matrix: [(Call, &[At]); 10] = [
        (Call::StartRun, &[NotStarted]),
        (Call::BeginCase, &[RunningIdle]),
        (Call::Reserve, &[RunningCase]),
        (
            Call::Assert,
            &[
                RunningCase,
                RunningIdle,
                FailedCase,
                FailedExpectation,
                Recovery,
                Candidate,
            ],
        ),
        (Call::EndCase, &[RunningCase, FailedCase, FailedExpectation]),
        (Call::FinishRun, &[RunningIdle]),
        (Call::Lend, &[RunningCase, FailedCase, Recovery]),
        (Call::Retry, &[Recovery]),
        (Call::Close, &[NotStarted, Finalized]),
        (
            Call::Cancel,
            &[NotStarted, RunningCase, RunningIdle, Candidate],
        ),
    ];
    for (call, accepting) in matrix {
        for at in PHASES {
            assert_eq!(
                probe(at, call),
                accepting.contains(&at),
                "{call:?} in {at:?}"
            );
        }
    }
}

/// The finalization boundary: a completion candidate issues its terminal
/// record only once every earlier record is acknowledged, the run is final
/// only when that record is acknowledged, no pass is published before, and
/// the closed result, the final snapshot and the acknowledged records name
/// the same outcome. A failure while a candidate is carried by the terminal
/// record; a failing terminal record leaves the run unfinalized and the
/// custody open.
#[test]
fn h24_finalization_boundary_candidate_terminal_and_final() {
    let mut lab = Lab::new(TEST);
    lab.start();
    lab.begin(1, Expectation::Clean);
    lab.create(SlotKind::Process);
    lab.end_case(&mut Scripted::always(Plan::Confirm));
    lab.released();
    let now = lab.now();
    assert_eq!(lab.c().finish_run(now), Ok(RunPhase::Candidate));
    let decision = lab.c().shutdown_decision();
    assert!(
        is_refused_with(&decision, Unresolved::RunEnding),
        "{decision:?}"
    );
    lab.flush();
    let earlier = lab.journal.submitted.len();
    lab.ack_until(earlier - 1);
    let snapshot = lab.snap();
    assert_eq!(
        (snapshot.phase, snapshot.terminal, snapshot.verdict),
        (RunPhase::Candidate, None, Verdict::Pending),
        "no terminal record while an earlier record is pending"
    );
    lab.ack_until(earlier);
    let snapshot = lab.snap();
    let terminal = snapshot.terminal.expect("the terminal record is issued");
    assert_eq!(
        (
            snapshot.phase,
            terminal.verdict,
            terminal.acknowledged,
            snapshot.verdict
        ),
        (
            RunPhase::Finalizing,
            Verdict::Passed,
            false,
            Verdict::Pending
        ),
        "no pass before the terminal record is acknowledged"
    );
    assert!(is_refused_with(
        &lab.c().shutdown_decision(),
        Unresolved::TerminalPending(terminal.record)
    ));
    lab.flush();
    assert_eq!(
        lab.journal
            .submitted
            .last()
            .map(|intent| (intent.id, intent.kind)),
        Some((
            terminal.record,
            RecordKind::RunEnded {
                verdict: Verdict::Passed,
                resolved_by: None
            }
        )),
        "the terminal record is the last"
    );
    lab.ack_all();
    let snapshot = lab.snap();
    assert_eq!(
        (snapshot.phase, snapshot.verdict),
        (RunPhase::Finalized, Verdict::Passed)
    );
    assert_eq!(lab.c().shutdown_decision(), ShutdownDecision::Permitted);
    let records = lab.journal.submitted.len() as u64;
    let closed = lab.close().expect("final and durable");
    assert_eq!(
        (closed.terminal, closed.verdict, closed.records),
        (Some(terminal.record), Verdict::Passed, records)
    );
    let last = lab.snap();
    assert_eq!(
        (last.phase, last.verdict, last.evidence.acknowledged),
        (RunPhase::Closed, Verdict::Passed, records)
    );

    // A failure while a candidate (its case records still pending) is
    // carried by the terminal record.
    let mut lab = Lab::new(TEST);
    lab.start();
    lab.begin(1, Expectation::Clean);
    assert!(lab.end_case(&mut Scripted::always(Plan::Confirm)).passed);
    let now = lab.now();
    assert_eq!(lab.c().finish_run(now), Ok(RunPhase::Candidate));
    let now = lab.now();
    lab.c()
        .record_assertion_failure("observed before the end", now)
        .expect("a candidate's verdict is still open");
    lab.ack_all();
    assert_eq!(
        lab.terminal_record().map(|(_, verdict)| verdict),
        Some(Verdict::Failed)
    );
    assert_eq!(lab.snap().verdict, Verdict::Failed);

    // With nothing pending, the terminal record is issued at once.
    let mut lab = Lab::new(TEST);
    lab.start();
    let now = lab.now();
    assert_eq!(lab.c().finish_run(now), Ok(RunPhase::Finalizing));

    // A failing terminal record: never final, never closed, and the fault
    // does not rewrite the verdict fixed in it.
    let mut lab = Lab::new(TEST);
    lab.start();
    lab.begin(1, Expectation::Clean);
    assert!(lab.end_case(&mut Scripted::always(Plan::Confirm)).passed);
    let now = lab.now();
    assert_eq!(lab.c().finish_run(now), Ok(RunPhase::Candidate));
    lab.flush();
    lab.ack_until(lab.journal.submitted.len());
    lab.flush();
    let (record, verdict) = lab.terminal_record().expect("issued");
    assert_eq!(verdict, Verdict::Passed);
    let now = lab.now();
    assert_eq!(
        lab.c().record_failed(record, now),
        AckOutcome::FailureRecorded
    );
    let snapshot = lab.snap();
    assert_eq!(
        (snapshot.phase, snapshot.verdict),
        (RunPhase::Finalizing, Verdict::Pending)
    );
    assert_eq!(fault_count(&snapshot, FailureClass::RecordFailed), 1);
    assert_eq!(snapshot.failure_counts, [0; FailureClass::COUNT]);
    let decision = lab.c().shutdown_decision();
    assert!(
        is_refused_with(&decision, Unresolved::TerminalPending(record)),
        "{decision:?}"
    );
    assert!(lab.close().is_none());
}

/// A declared output detachment is an expected injected condition: observed
/// once in its own case, it is no failure (its settlement still records the
/// output as lost); undeclared, unobserved or repeated, it is.
#[test]
fn h25_declared_output_detachment_is_expected_and_checked() {
    // By cleanup of a stored owner.
    let mut lab = Lab::new(TEST);
    lab.start();
    lab.begin(1, Expectation::OutputDetached);
    let (_, token) = lab.create(SlotKind::Process);
    let outcome = lab.end_case(&mut Scripted::always(Plan::Detached));
    assert_eq!(
        (outcome.passed, outcome.resolved, outcome.stopped),
        (true, true, false)
    );
    assert_eq!(lab.released(), vec![token]);
    let snapshot = lab.snap();
    assert_eq!(snapshot.failure_counts, [0; FailureClass::COUNT]);
    lab.ack_all();
    assert!(lab.records().iter().any(|kind| matches!(
        kind,
        RecordKind::ActionSettled {
            how: Settlement::OutputLost,
            ..
        }
    )));
    assert_eq!(lab.finish(), RunPhase::Finalized);
    assert_eq!(lab.snap().verdict, Verdict::Passed);

    // By the operation's own end.
    let mut lab = Lab::new(TEST);
    lab.start();
    lab.begin(1, Expectation::OutputDetached);
    let ticket = lab.admit(SlotKind::Process);
    let now = lab.now();
    assert!(matches!(
        lab.c()
            .complete(&ticket, NativeOutcome::Ended(DETACHED_END), now),
        Ok(Completion::OutputLost)
    ));
    assert!(lab.end_case(&mut Scripted::always(Plan::Confirm)).passed);

    // Declared but not observed.
    let mut lab = Lab::new(TEST);
    lab.start();
    lab.begin(1, Expectation::OutputDetached);
    lab.create(SlotKind::Process);
    let outcome = lab.end_case(&mut Scripted::always(Plan::Confirm));
    assert!(!outcome.passed);
    assert_eq!(count(&lab.snap(), FailureClass::ExpectedConditionUnmet), 1);

    // Declared once, observed twice.
    let mut lab = Lab::new(TEST);
    lab.start();
    lab.begin(1, Expectation::OutputDetached);
    let ticket = lab.admit(SlotKind::Process);
    let now = lab.now();
    assert!(matches!(
        lab.c()
            .complete(&ticket, NativeOutcome::Ended(DETACHED_END), now),
        Ok(Completion::OutputLost)
    ));
    lab.create(SlotKind::Process);
    let outcome = lab.end_case(&mut Scripted::always(Plan::Detached));
    assert!(!outcome.passed);
    assert_eq!(count(&lab.snap(), FailureClass::OutputLost), 1);
}

/// One entry point at which the core recognizes an actual failure, and the
/// failure it names.
type EntryPoint = (FailureClass, fn(&mut Lab));

/// Every entry point that recognizes an actual failure closes execution
/// admission itself, naming the failure; an expected condition does not; a
/// validator panic before the run is no run failure (the incident just keeps
/// blocking the start).
#[test]
fn h26_every_failure_entry_point_closes_admission_itself() {
    let points: [EntryPoint; 8] = [
        (FailureClass::Assertion, |lab| {
            let now = lab.now();
            lab.c()
                .record_assertion_failure("observed", now)
                .expect("recorded");
        }),
        (FailureClass::Assertion, |lab| {
            let (entry, _) = lab.create(SlotKind::Process);
            let now = lab.now();
            let _ = lab
                .c()
                .lend(entry, now, |_| -> u64 { panic!("injected borrower panic") });
        }),
        (FailureClass::UnexpectedCleanup, |lab| {
            let (entry, _) = lab.create(SlotKind::Process);
            let now = lab.now();
            let mut cleanup =
                Scripted::always(Plan::Confirm).first(SlotKind::Process, &[Plan::Panic]);
            assert_eq!(lab.c().end_entry(entry, &mut cleanup, now), Ok(true));
        }),
        (FailureClass::UnexpectedRetained, |lab| {
            lab.deposit(SlotKind::Process, true);
        }),
        (FailureClass::UnexpectedOwner, |lab| {
            let ticket = lab.admit(SlotKind::Process);
            let owner = lab.native.produce_as(&ticket, None);
            let now = lab.now();
            lab.c()
                .complete(&ticket, NativeOutcome::Created(owner), now)
                .expect("held");
        }),
        (FailureClass::AuthorityLost, |lab| {
            let ticket = lab.admit(SlotKind::Process);
            let now = lab.now();
            let unconfirmed = EndFacts {
                subtree: false,
                reaped: true,
                output: OutputObserved::Complete,
                removed: false,
            };
            assert!(matches!(
                lab.c()
                    .complete(&ticket, NativeOutcome::Ended(unconfirmed), now),
                Ok(Completion::AuthorityLost)
            ));
        }),
        (FailureClass::OutputLost, |lab| {
            let (entry, _) = lab.create(SlotKind::Process);
            let now = lab.now();
            let mut cleanup = Scripted::always(Plan::Detached);
            assert_eq!(lab.c().end_entry(entry, &mut cleanup, now), Ok(true));
        }),
        (FailureClass::RecorderFault, |lab| {
            let now = lab.now();
            let _ = lab.c().flush_records(&mut PanickingSink, now);
        }),
    ];
    for (class, inject) in points {
        let mut lab = Lab::new(TEST);
        lab.start();
        lab.begin(1, Expectation::Clean);
        inject(&mut lab);
        let closure = lab.control.status().control.admission.closure;
        assert_eq!(closed_by_failure(closure), Some(class), "{class:?}");
        let now = lab.now();
        assert!(
            matches!(
                lab.c().reserve(SlotKind::Fixture, now),
                Err(Refusal::AdmissionClosed(_))
            ),
            "{class:?}"
        );
        assert!(lab.dropped().is_empty(), "{class:?}");
    }

    // Failures recognized at the end of a case close admission the same way.
    for (expectation, class) in [
        (
            Expectation::RetainedBoundary,
            FailureClass::ExpectedConditionUnmet,
        ),
        (Expectation::Clean, FailureClass::UnknownOutcome),
    ] {
        let mut lab = Lab::new(TEST);
        lab.start();
        lab.begin(1, expectation);
        if class == FailureClass::UnknownOutcome {
            lab.admit(SlotKind::Process);
        } else {
            lab.create(SlotKind::Process);
        }
        lab.end_case(&mut Scripted::always(Plan::Confirm));
        assert_eq!(
            closed_by_failure(lab.control.status().control.admission.closure),
            Some(class)
        );
    }

    // An expected condition met is no failure: admission stays open.
    let mut lab = Lab::new(TEST);
    lab.start();
    lab.begin(1, Expectation::RetainedBoundary);
    lab.deposit(SlotKind::Process, true);
    assert!(lab.end_case(&mut Scripted::always(Plan::Confirm)).passed);
    assert_eq!(lab.control.status().control.admission.closure, None);

    // Between cases, a failure stops the run at once: an assertion makes it
    // a (failed) completion candidate, a late owner makes it require
    // recovery; no later call is needed for either.
    let mut lab = Lab::new(TEST);
    lab.start();
    lab.begin(1, Expectation::Clean);
    assert!(lab.end_case(&mut Scripted::always(Plan::Confirm)).passed);
    let now = lab.now();
    lab.c()
        .record_assertion_failure("observed between cases", now)
        .expect("recorded");
    let snapshot = lab.snap();
    assert_eq!(
        (snapshot.phase, snapshot.verdict),
        (RunPhase::Candidate, Verdict::Failed)
    );
    let mut lab = Lab::new(TEST);
    lab.start();
    lab.begin(1, Expectation::Clean);
    let ticket = lab.admit(SlotKind::Process);
    let now = lab.now();
    let proof = NoEffectProof::for_ticket(&ticket, "refused");
    lab.c()
        .complete(&ticket, NativeOutcome::NoEffect(proof), now)
        .expect("resolved");
    assert!(lab.end_case(&mut Scripted::always(Plan::Confirm)).passed);
    let late = lab.native.produce(&ticket);
    let now = lab.now();
    assert!(matches!(
        lab.c().complete(&ticket, NativeOutcome::Created(late), now),
        Ok(Completion::Late { .. })
    ));
    assert_eq!(lab.snap().phase, RunPhase::RecoveryRequired);

    // A validator panic before the run.
    let blocked = PriorIncident {
        binding: IncidentBinding::new([7; 32]),
        outcome: PriorOutcome::Malformed,
    };
    let mut lab = Lab::with(TEST, GENERATION, vec![blocked]);
    let panicking = Validator {
        answers: Vec::new(),
        panics: true,
    };
    let now = lab.now();
    assert_eq!(
        lab.c().apply_disposition(&blocked.binding, &panicking, now),
        Err(Refusal::Undisposable)
    );
    assert_eq!(lab.control.status().control.admission.closure, None);
    let now = lab.now();
    assert_eq!(
        lab.c().start_run(now).unwrap_err(),
        Refusal::PriorUnresolved
    );
}
