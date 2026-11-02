//! M2c-B (PR-B) oracles for the broker-owned halt, pre-registered before the
//! halt exists (design v3 §4, §5, §6, §7.2, §11 C1, C2, C4, C5): B1–B18 and
//! R1–R5, plus the halted-door cases PR-A left for the halt (A4 and A6-e
//! through B17, A7-c, and plant (i) through B7).
//!
//! Every race is a forced order: a named broker hook (`test_hooks`) blocks a
//! thread at that place until the test releases it, and a test-held gate
//! blocks the evidence store inside its mutex (`Fault::Stall`). Never a
//! stress loop or a fixed sleep. Every thread is joined through a channel
//! with a bounded wait, which only detects a deadlock; every pause is
//! released when its guard drops. After each schedule `probe_locks` checks
//! that no lock was left held or poisoned (unless the schedule poisoned it).
//! Each refusal has a positive control on the same door.

use super::{AgentDoor, AgentOutcome, GovernedControl, GrantRequest, Intent, Lookup};
use crate::authority::approval::{
    AcknowledgementConfirmation, ActionConfirmation, AgentDoorConfirmation, ControlConfirmer,
    GrantConfirmation, MissionConfirmation, MissionResumeConfirmation, ResumeConfirmation,
};
use crate::authority::clock::{Clock, ManualClock, SystemClock};
use crate::authority::commitment::{
    CommitmentState, ExecutionGuard, FailureClass, PreparedAction, TargetIdentity,
};
use crate::authority::effect::{CapabilityKind, EffectClass};
use crate::authority::evidence::{EvidencePhase, EvidenceRecord, EvidenceSink, MemoryEvidence};
use crate::authority::halt::{HaltReason, RecordBatch};
use crate::authority::ids::{AgentId, CommitmentId, Digest, GrantId, RunId};
use crate::authority::policy::GrantScope;
use crate::authority::run::RunOrigin;
use crate::authority::AuthorityError;
use crate::broker::{CredentialSpec, Placement, SecretSource, SecretUnavailable};
use crate::control::{EffectOutput, PendingEffect, Preparation};
use crate::egress::destination::Destination;
use crate::egress::{EgressIntent, ReleaseCredential};
use crate::evidence_store::tests::Scratch;
use crate::evidence_store::{Fault, PrimaryEvidence};
use crate::harness_tests::{temp_root, Reply, TestServer, Yes};
use crate::test_hooks::{at, Gate};
use std::any::Any;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use zeroize::Zeroizing;

/// A bounded join: it detects a deadlock and never makes a test pass.
const JOIN: Duration = Duration::from_secs(10);

// ---------------------------------------------------------------- owner

/// The Owner: yes to doors, grants, actions and resumes; no to the rest.
struct Owner;
impl ControlConfirmer for Owner {
    fn confirm_action(&self, _: &ActionConfirmation) -> bool {
        true
    }
    fn confirm_grant(&self, _: &GrantConfirmation) -> bool {
        true
    }
    fn confirm_resume(&self, _: &ResumeConfirmation) -> bool {
        true
    }
    fn confirm_acknowledgement(&self, _: &AcknowledgementConfirmation) -> bool {
        true
    }
    fn confirm_mission(&self, _: &MissionConfirmation) -> bool {
        true
    }
    fn confirm_agent_door(&self, _: &AgentDoorConfirmation) -> bool {
        true
    }
    fn confirm_held_mission(&self, _: &MissionResumeConfirmation) -> bool {
        true
    }
}

/// The Owner, counting the door, mission and held-mission dialogs.
#[derive(Default)]
struct Counting {
    doors: AtomicUsize,
    missions: AtomicUsize,
    helds: AtomicUsize,
}
impl ControlConfirmer for Counting {
    fn confirm_action(&self, _: &ActionConfirmation) -> bool {
        false
    }
    fn confirm_grant(&self, _: &GrantConfirmation) -> bool {
        false
    }
    fn confirm_resume(&self, _: &ResumeConfirmation) -> bool {
        false
    }
    fn confirm_acknowledgement(&self, _: &AcknowledgementConfirmation) -> bool {
        false
    }
    fn confirm_mission(&self, _: &MissionConfirmation) -> bool {
        self.missions.fetch_add(1, Ordering::SeqCst);
        true
    }
    fn confirm_agent_door(&self, _: &AgentDoorConfirmation) -> bool {
        self.doors.fetch_add(1, Ordering::SeqCst);
        true
    }
    fn confirm_held_mission(&self, _: &MissionResumeConfirmation) -> bool {
        self.helds.fetch_add(1, Ordering::SeqCst);
        true
    }
}

/// A vault with one token.
struct Vault;
impl SecretSource for Vault {
    fn read(&self, _: &str, _: &str) -> Result<Zeroizing<String>, SecretUnavailable> {
        Ok(Zeroizing::new("token-m2c".into()))
    }
}

fn mother() -> AgentId {
    AgentId::new("mother").unwrap()
}

const HASH: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn mission_request(label: &str) -> MissionConfirmation {
    MissionConfirmation {
        contract_hash: HASH.into(),
        request: label.into(),
        summary: vec!["A one-page site".into()],
    }
}

fn held_request(label: &str) -> MissionResumeConfirmation {
    MissionResumeConfirmation {
        mission: "m-1".into(),
        contract_hash: HASH.into(),
        request: label.into(),
        held_reason: "unclean_shutdown".into(),
        summary: vec!["A one-page site".into()],
    }
}

// ---------------------------------------------------------------- pauses

/// A thread held at a named broker hook until released (or until this
/// guard drops).
struct Pause {
    entered: Receiver<()>,
    release: Option<Sender<()>>,
}

impl Pause {
    /// Wait until a thread is held at the hook.
    fn reached(&self) {
        self.entered
            .recv_timeout(JOIN)
            .expect("the paused thread never reached its hook");
    }

    fn release(mut self) {
        self.release.take();
    }
}

impl Drop for Pause {
    fn drop(&mut self) {
        self.release.take();
    }
}

/// Hold the next thread that reaches `at` on `gov`.
fn pause_at(gov: &GovernedControl, at: &'static str) -> Pause {
    let (entered, reached) = channel();
    let (release, released) = channel::<()>();
    gov.hooks().arm(at, move || {
        let _ = entered.send(());
        // Released by a send or by the guard's drop; bounded either way,
        // and longer than a bounded join, so that a wait on it shows.
        let _ = released.recv_timeout(3 * JOIN);
    });
    Pause {
        entered: reached,
        release: Some(release),
    }
}

/// The order in which named moments happened, across threads.
#[derive(Clone, Default)]
struct Order(Arc<Mutex<Vec<&'static str>>>);

impl Order {
    fn mark(&self, what: &'static str) {
        self.0.lock().unwrap().push(what);
    }

    fn at(&self, gov: &GovernedControl, at: &'static str, what: &'static str) {
        let order = self.clone();
        gov.hooks().arm(at, move || order.mark(what));
    }

    fn index(&self, what: &str) -> usize {
        self.0
            .lock()
            .unwrap()
            .iter()
            .position(|w| *w == what)
            .unwrap_or_else(|| panic!("{what} never happened: {:?}", self.0.lock().unwrap()))
    }
}

/// Run `f` on a thread; its result comes back through a bounded join.
fn spawn<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Receiver<T> {
    let (done, result) = channel();
    std::thread::spawn(move || {
        let _ = done.send(f());
    });
    result
}

fn join<T>(result: &Receiver<T>) -> T {
    result.recv_timeout(JOIN).expect("a thread deadlocked")
}

// ---------------------------------------------------------------- effects

fn target() -> Digest {
    Digest::of("m2c.halt.target", &[b"here"])
}

fn parameters() -> Digest {
    Digest::of("m2c.halt.params", &[b"p"])
}

/// A counting effect: how often it ran, and whether it saw its run's
/// cancellation at its first check. It can hold its revalidation (before
/// `begin`) or its execution (while Executing) at a gate.
#[derive(Clone, Default)]
struct Counter {
    ran: Arc<AtomicUsize>,
    saw_cancelled: Arc<AtomicBool>,
}

struct Effect {
    counter: Counter,
    hold_revalidation: Option<Gate>,
    hold_execution: Option<Gate>,
}

impl PendingEffect for Effect {
    fn revalidate(&self) -> Result<Digest, AuthorityError> {
        if let Some(gate) = &self.hold_revalidation {
            gate.hold();
        }
        Ok(target())
    }

    fn parameters(&self) -> Digest {
        parameters()
    }

    fn execute(
        self: Box<Self>,
        guard: &ExecutionGuard,
    ) -> Result<EffectOutput, (FailureClass, String)> {
        self.counter.ran.fetch_add(1, Ordering::SeqCst);
        if let Some(gate) = &self.hold_execution {
            gate.hold();
        }
        if guard.is_cancelled() {
            self.counter.saw_cancelled.store(true, Ordering::SeqCst);
            return Err((
                FailureClass::Actuator,
                "cancelled at its first check".into(),
            ));
        }
        Ok(EffectOutput::default())
    }
}

// ---------------------------------------------------------------- fixture

/// A front door, the Owner's perception grant, and a world server with a
/// GET/POST grant.
struct Fx {
    gov: Arc<GovernedControl>,
    /// Every record, in order (the sink, or the store's projection).
    recent: Arc<MemoryEvidence>,
    primary: Option<Arc<PrimaryEvidence>>,
    clock: Arc<ManualClock>,
    grant: GrantId,
    server: TestServer,
    _keep: Box<dyn Any>,
}

/// Over in-memory evidence.
fn fx() -> Fx {
    let root = temp_root("m2c-halt");
    let recent = Arc::new(MemoryEvidence::new(100_000));
    let clock = Arc::new(ManualClock::default());
    let gov = Arc::new(
        GovernedControl::with_secrets(
            root.0.path(),
            Arc::new(Vault),
            recent.clone(),
            clock.clone(),
        )
        .unwrap(),
    );
    finish(gov, recent, None, clock, Box::new(root))
}

/// Over a primary evidence store at `scratch` (opened again by R1–R5).
fn fx_store(scratch: &Scratch) -> Fx {
    let recent = Arc::new(MemoryEvidence::new(100_000));
    let primary = PrimaryEvidence::open(&scratch.store(), vec![recent.clone()]).unwrap();
    let clock = Arc::new(ManualClock::default());
    let gov = Arc::new(
        GovernedControl::with_secrets(
            &scratch.path("runtime"),
            Arc::new(Vault),
            primary.clone(),
            clock.clone(),
        )
        .unwrap(),
    );
    finish(gov, recent, Some(primary), clock, Box::new(()))
}

fn finish(
    gov: Arc<GovernedControl>,
    recent: Arc<MemoryEvidence>,
    primary: Option<Arc<PrimaryEvidence>>,
    clock: Arc<ManualClock>,
    keep: Box<dyn Any>,
) -> Fx {
    let server = TestServer::start(|_| Reply::ok("done"));
    let mut grant = None;
    if !gov.status().emergency_stopped {
        gov.request_grant(
            &GrantRequest::Egress {
                origin: server.origin(),
                methods: vec!["GET".into(), "POST".into()],
                allow_private: true,
            },
            Duration::from_secs(3600),
            &Yes::new(true),
        )
        .unwrap();
        grant = Some(
            gov.authority()
                .grants()
                .request(
                    GrantScope::Perception {
                        display: "agent".into(),
                    },
                    Duration::from_secs(3600),
                    &Owner,
                )
                .unwrap(),
        );
    }
    Fx {
        gov,
        recent,
        primary,
        clock,
        grant: grant
            .unwrap_or_else(|| GrantId::parse("grt-00000000000000000000000000000000").unwrap()),
        server,
        _keep: keep,
    }
}

impl Fx {
    fn door(&self) -> AgentDoor {
        self.gov
            .issue_agent_door(mother(), Arc::new(Owner))
            .expect("the Owner said yes to the door")
    }

    fn get(&self, path: &str) -> Intent {
        Intent::Request(EgressIntent {
            method: "GET".into(),
            url: format!("{}{path}", self.server.origin()),
            headers: vec![],
            body: None,
        })
    }

    fn post(&self, path: &str) -> Intent {
        Intent::Request(EgressIntent {
            method: "POST".into(),
            url: format!("{}{path}", self.server.origin()),
            headers: vec![],
            body: Some("x".into()),
        })
    }

    fn reached(&self, path: &str) -> usize {
        self.server
            .received()
            .iter()
            .filter(|r| r.path == path)
            .count()
    }

    /// Commit to a counting effect in `run` (an R1 perception action under
    /// the Owner's grant). Nothing happens yet.
    fn propose(&self, run: RunId, effect: Effect) -> Result<CommitmentId, AuthorityError> {
        self.gov
            .control
            .propose(
                &mother(),
                run,
                Preparation {
                    action: PreparedAction {
                        kind: CapabilityKind::Perception,
                        class: EffectClass::R1,
                        operation: "m2c.observe",
                        target: TargetIdentity {
                            display: "m2c target".into(),
                            digest: target(),
                        },
                        parameters: parameters(),
                        grants: vec![self.grant],
                        leases: vec![],
                        summary: vec!["m2c".into()],
                    },
                    effect: Box::new(effect),
                    ttl: Duration::from_secs(60),
                },
            )
            .map(|view| view.id)
    }

    fn counting(&self, run: RunId, counter: &Counter) -> CommitmentId {
        self.propose(
            run,
            Effect {
                counter: counter.clone(),
                hold_revalidation: None,
                hold_execution: None,
            },
        )
        .unwrap()
    }

    fn authorize(&self, id: CommitmentId, run: RunId) -> Result<(), AuthorityError> {
        self.gov.control.authorize(id, &mother(), run, &Owner)
    }

    fn execute(&self, id: CommitmentId, run: RunId) -> Result<EffectOutput, AuthorityError> {
        self.gov.control.execute(id, &mother(), run)
    }

    fn state(&self, id: CommitmentId) -> CommitmentState {
        self.gov.authority().commitments().view(id).unwrap().state
    }

    fn cancelled(&self, run: RunId) -> bool {
        self.gov.authority().runs().view(run).unwrap().cancelled
    }

    fn phases(&self) -> Vec<EvidencePhase> {
        self.recent.records().iter().map(|r| r.phase).collect()
    }

    fn count(&self, phase: EvidencePhase) -> usize {
        self.phases().iter().filter(|p| **p == phase).count()
    }

    fn records(&self, phase: EvidencePhase) -> Vec<EvidenceRecord> {
        self.recent
            .records()
            .into_iter()
            .filter(|r| r.phase == phase)
            .collect()
    }
}

fn halt(door: &AgentDoor) -> RecordBatch {
    door.halt(HaltReason::JournalFailed)
}

/// A batch's phases, in order (it is not written).
fn batch_phases(batch: &RecordBatch) -> Vec<EvidencePhase> {
    batch.records().iter().map(|r| r.phase).collect()
}

// ---------------------------------------------------------------- B1, B2

/// B1: a `complete()` paused holding the commitments lock, before its
/// `runs.check`, while the halt runs. The halt's first step completes, but
/// it cannot return before the paused call releases the lock; then the call
/// is refused (Halted), the commitment is Revoked, the effect never ran.
#[test]
fn m2c_b1_the_runs_barrier_refuses_a_check_after_it() {
    let f = fx();
    let door = f.door();
    let run = door.open_run().unwrap();
    let counter = Counter::default();
    let id = f.counting(run, &counter);
    f.authorize(id, run).unwrap();
    let order = Order::default();
    let paused = pause_at(&f.gov, at::COMPLETE_BEFORE_CHECK);
    let gov = f.gov.clone();
    let executing = spawn(move || gov.control.execute(id, &mother(), run));
    paused.reached();
    let halted_after_runs = pause_at(&f.gov, at::HALT_AFTER_RUNS);
    order.at(&f.gov, at::HALT_ENTRIES_TAKEN, "halt took entries");
    let door = Arc::new(door);
    let halting = {
        let door = door.clone();
        spawn(move || halt(&door))
    };
    halted_after_runs.reached();
    halted_after_runs.release();
    order.mark("complete released");
    paused.release();
    let executed = join(&executing);
    assert_eq!(
        executed.map(|_| ()).unwrap_err().cause(),
        &AuthorityError::Halted,
        "refused by the halted group"
    );
    let batch = join(&halting);
    assert!(order.index("complete released") < order.index("halt took entries"));
    assert_eq!(counter.ran.load(Ordering::SeqCst), 0);
    assert_eq!(f.state(id), CommitmentState::Revoked);
    assert!(f.cancelled(run));
    let _ = batch.write();
    f.gov.probe_locks();
}

/// B2: paused after `runs.check` passed and before `finalize`: the halt
/// does not return during the pause; after release the commitment is
/// Executing before the halt returns, its token is set at return, and the
/// effect sees the cancellation at its first check.
#[test]
fn m2c_b2_the_entries_barrier_waits_for_a_checked_complete() {
    let f = fx();
    let door = Arc::new(f.door());
    let run = door.open_run().unwrap();
    let counter = Counter::default();
    let gate = Gate::new();
    let id = f
        .propose(
            run,
            Effect {
                counter: counter.clone(),
                hold_revalidation: None,
                hold_execution: Some(gate.clone()),
            },
        )
        .unwrap();
    f.authorize(id, run).unwrap();
    let order = Order::default();
    let paused = pause_at(&f.gov, at::COMPLETE_CHECKED);
    order.at(&f.gov, at::COMPLETE_FINALIZED, "executing");
    let gov = f.gov.clone();
    let executing = spawn(move || gov.control.execute(id, &mother(), run));
    paused.reached();
    order.at(&f.gov, at::HALT_ENTRIES_TAKEN, "halt took entries");
    let halted_after_runs = pause_at(&f.gov, at::HALT_AFTER_RUNS);
    let halting = {
        let door = door.clone();
        spawn(move || halt(&door))
    };
    halted_after_runs.reached();
    halted_after_runs.release();
    paused.release();
    let batch = join(&halting);
    assert!(order.index("executing") < order.index("halt took entries"));
    assert_eq!(f.state(id), CommitmentState::Executing);
    assert!(f.cancelled(run), "the token is set when the halt returns");
    gate.open();
    let executed = join(&executing);
    assert!(executed.is_err());
    assert_eq!(counter.ran.load(Ordering::SeqCst), 1);
    assert!(counter.saw_cancelled.load(Ordering::SeqCst));
    let _ = batch.write();
    f.gov.probe_locks();
}

// ---------------------------------------------------------------- B3

/// B3: the start record is held by a stalled store: the halt returns while
/// the store is stalled (it does no I/O); the start then completes, and is
/// refused; the commitment is revoked with `interrupted=start`.
#[test]
fn m2c_b3_a_start_held_in_the_store_is_refused() {
    let scratch = Scratch::new();
    let f = fx_store(&scratch);
    let door = Arc::new(f.door());
    let run = door.open_run().unwrap();
    let counter = Counter::default();
    let id = f.counting(run, &counter);
    f.authorize(id, run).unwrap();
    let stall = Gate::new();
    f.primary
        .as_ref()
        .unwrap()
        .store()
        .plant_fault(Fault::Stall(stall.clone()));
    let gov = f.gov.clone();
    let executing = spawn(move || gov.control.execute(id, &mother(), run));
    stall.reached();
    let batch = halt(&door);
    stall.open();
    let executed = join(&executing);
    assert_eq!(
        executed.map(|_| ()).unwrap_err().cause(),
        &AuthorityError::Halted
    );
    assert_eq!(counter.ran.load(Ordering::SeqCst), 0);
    assert_eq!(f.state(id), CommitmentState::Revoked);
    let revoked = f.records(EvidencePhase::Revoked);
    assert!(revoked.iter().any(|r| r
        .detail
        .iter()
        .any(|(k, v)| k == "interrupted" && v == "start")));
    let _ = batch.write();
    f.gov.probe_locks();
}

// ---------------------------------------------------------------- B4

/// B4 (Codex R2-8): opening a run against a halt.
#[test]
fn m2c_b4_opening_a_run_meets_the_halt() {
    let f = fx();
    // Insertion, then the halt, then RunOpened: refused.
    let door = Arc::new(f.door());
    let paused = pause_at(&f.gov, at::OPEN_INSERTED);
    let opening = {
        let door = door.clone();
        spawn(move || door.open_run())
    };
    paused.reached();
    let batch = halt(&door);
    paused.release();
    assert_eq!(join(&opening), Err(AuthorityError::Halted));
    let _ = batch.write();

    // The post-record check passed, then the halt, then the return: the
    // run comes back permanently cancelled, and every call on it refuses.
    let door = Arc::new(f.door());
    let paused = pause_at(&f.gov, at::OPEN_CHECKED);
    let opening = {
        let door = door.clone();
        spawn(move || door.open_run())
    };
    paused.reached();
    let batch = halt(&door);
    paused.release();
    let run = join(&opening).expect("the opening passed its check before the halt");
    assert!(f.cancelled(run));
    assert_eq!(
        door.agent_action(run, &f.get("/b4")).unwrap_err(),
        AuthorityError::Halted
    );
    assert_eq!(f.reached("/b4"), 0);
    let _ = batch.write();

    // Halt, then open: refused before any record.
    let door = f.door();
    let opened = f.count(EvidencePhase::RunOpened);
    let batch = halt(&door);
    assert_eq!(door.open_run(), Err(AuthorityError::Halted));
    assert_eq!(f.count(EvidencePhase::RunOpened), opened);
    let _ = batch.write();

    // Positive control: another door opens.
    assert!(f.door().open_run().is_ok());
    f.gov.probe_locks();
}

// ---------------------------------------------------------------- B5

/// B5: a preparation paused before its insertion, and an authorization
/// paused after its gating record, are both refused after the halt.
#[test]
fn m2c_b5_prepare_and_authorize_meet_the_halt() {
    let f = fx();
    let door = Arc::new(f.door());
    let run = door.open_run().unwrap();
    let paused = pause_at(&f.gov, at::PREPARE_BEFORE_INSERT);
    let preparing = {
        let f_gov = f.gov.clone();
        let grant = f.grant;
        spawn(move || {
            let fx = Fx2 { gov: f_gov, grant };
            fx.propose(run)
        })
    };
    paused.reached();
    let batch = halt(&door);
    paused.release();
    assert_eq!(join(&preparing), Err(AuthorityError::Halted));
    let _ = batch.write();

    let door = Arc::new(f.door());
    let run = door.open_run().unwrap();
    let counter = Counter::default();
    let id = f.counting(run, &counter);
    let paused = pause_at(&f.gov, at::AUTHORIZE_RECORDED);
    let gov = f.gov.clone();
    let authorizing = spawn(move || gov.control.authorize(id, &mother(), run, &Owner));
    paused.reached();
    let batch = halt(&door);
    paused.release();
    assert_eq!(join(&authorizing), Err(AuthorityError::Halted));
    assert_eq!(f.state(id), CommitmentState::Revoked);
    assert_eq!(counter.ran.load(Ordering::SeqCst), 0);
    let _ = batch.write();

    // Positive control on a door not halted.
    let door = f.door();
    let run = door.open_run().unwrap();
    let id = f.counting(run, &counter);
    f.authorize(id, run).unwrap();
    f.execute(id, run).unwrap();
    assert_eq!(counter.ran.load(Ordering::SeqCst), 1);
    f.gov.probe_locks();
}

/// What a thread needs to propose (the fixture holds a server).
struct Fx2 {
    gov: Arc<GovernedControl>,
    grant: GrantId,
}

impl Fx2 {
    fn propose(&self, run: RunId) -> Result<CommitmentId, AuthorityError> {
        self.gov
            .control
            .propose(
                &mother(),
                run,
                Preparation {
                    action: PreparedAction {
                        kind: CapabilityKind::Perception,
                        class: EffectClass::R1,
                        operation: "m2c.observe",
                        target: TargetIdentity {
                            display: "m2c target".into(),
                            digest: target(),
                        },
                        parameters: parameters(),
                        grants: vec![self.grant],
                        leases: vec![],
                        summary: vec!["m2c".into()],
                    },
                    effect: Box::new(Effect {
                        counter: Counter::default(),
                        hold_revalidation: None,
                        hold_execution: None,
                    }),
                    ttl: Duration::from_secs(60),
                },
            )
            .map(|view| view.id)
    }
}

// ---------------------------------------------------------------- B6

const SPEC: CredentialSpec = CredentialSpec {
    service: "M2c",
    scope: "http",
    name: "m2c.token",
    placement: Placement::Authorization("Bearer"),
};

/// B6-b (Codex R2-11): a lease issue paused between its record and its
/// insertion, then a halt: the insertion sees the token, no lease is
/// inserted, and the issue returns Halted. The reverse order: the halt's
/// lease step ends an inserted lease.
#[test]
fn m2c_b6_b_a_lease_meets_the_halt_on_either_side() {
    let f = fx();
    let destination = Destination::parse(&f.server.origin()).unwrap();
    let door = Arc::new(f.door());
    let run = door.open_run().unwrap();
    let paused = pause_at(&f.gov, at::LEASE_RECORDED);
    let leasing = {
        let gov = f.gov.clone();
        let destination = destination.clone();
        spawn(move || {
            gov.credentials
                .lease(&mother(), run, SPEC, &destination, Duration::from_secs(60))
        })
    };
    paused.reached();
    let issued_before = f.gov.credentials.issued().len();
    let batch = halt(&door);
    paused.release();
    assert_eq!(join(&leasing), Err(AuthorityError::Halted));
    assert_eq!(
        f.gov.credentials.issued().len(),
        issued_before,
        "no lease inserted"
    );
    let _ = batch.write();

    // The lease is inserted first; the halt ends it.
    let door = f.door();
    let run = door.open_run().unwrap();
    let lease = f
        .gov
        .credentials
        .lease(&mother(), run, SPEC, &destination, Duration::from_secs(60))
        .unwrap();
    assert!(f.gov.credentials.is_live(lease));
    let batch = halt(&door);
    assert!(!f.gov.credentials.is_live(lease));
    let _ = batch.write();
    f.gov.probe_locks();
}

/// An effect that asks the broker for its lease's credential.
struct Releasing {
    gov: Arc<GovernedControl>,
    lease: crate::authority::ids::LeaseId,
    destination: Destination,
    got: Arc<Mutex<Option<Result<(), AuthorityError>>>>,
}

impl PendingEffect for Releasing {
    fn revalidate(&self) -> Result<Digest, AuthorityError> {
        Ok(self.destination.origin_digest())
    }
    fn parameters(&self) -> Digest {
        Digest::of("m2c.release", &[])
    }
    fn execute(
        self: Box<Self>,
        guard: &ExecutionGuard,
    ) -> Result<EffectOutput, (FailureClass, String)> {
        let got = self
            .gov
            .credentials
            .release(self.lease, guard, &self.destination)
            .map(|_| ());
        *self.got.lock().unwrap() = Some(got);
        Ok(EffectOutput::default())
    }
}

/// B6-a: a credential release paused after its record, and after its vault
/// read, with the halt in between: the re-check refuses, the lease is
/// ended, the token is set.
#[test]
fn m2c_b6_a_a_credential_release_meets_the_halt() {
    for at in [at::RELEASE_RECORDED, at::RELEASE_VAULT_READ] {
        let f = fx();
        let destination = Destination::parse(&f.server.origin()).unwrap();
        let door = Arc::new(f.door());
        let run = door.open_run().unwrap();
        let lease = f
            .gov
            .credentials
            .lease(&mother(), run, SPEC, &destination, Duration::from_secs(60))
            .unwrap();
        let got = Arc::new(Mutex::new(None));
        let grant = f
            .gov
            .authority()
            .grants()
            .request(
                GrantScope::Connector {
                    connector: "m2c".into(),
                    account: "test".into(),
                    operations: vec!["m2c.release".into()],
                },
                Duration::from_secs(600),
                &Owner,
            )
            .unwrap();
        let id = f
            .gov
            .control
            .propose(
                &mother(),
                run,
                Preparation {
                    action: PreparedAction {
                        kind: CapabilityKind::Connector,
                        class: EffectClass::R1,
                        operation: "m2c.release",
                        target: TargetIdentity {
                            display: "m2c release".into(),
                            digest: destination.origin_digest(),
                        },
                        parameters: Digest::of("m2c.release", &[]),
                        grants: vec![grant],
                        leases: vec![lease],
                        summary: vec!["m2c".into()],
                    },
                    effect: Box::new(Releasing {
                        gov: f.gov.clone(),
                        lease,
                        destination: destination.clone(),
                        got: got.clone(),
                    }),
                    ttl: Duration::from_secs(60),
                },
            )
            .unwrap()
            .id;
        f.authorize(id, run).unwrap();
        let paused = pause_at(&f.gov, at);
        let gov = f.gov.clone();
        let executing = spawn(move || gov.control.execute(id, &mother(), run));
        paused.reached();
        let batch = halt(&door);
        paused.release();
        let _ = join(&executing);
        let released = got.lock().unwrap().clone().expect("the release was asked");
        assert!(
            released.is_err(),
            "{at}: the re-check refuses after the halt"
        );
        assert!(!f.gov.credentials.is_live(lease), "{at}");
        assert!(f.cancelled(run), "{at}: the token is set");
        let _ = batch.write();
        f.gov.probe_locks();
    }
}

// ---------------------------------------------------------------- B7

/// B7 (Codex R2-7) and plant (i): adoption against a halt.
#[test]
fn m2c_b7_adoption_meets_the_halt() {
    let f = fx();
    // Join, then halt: the adopted run is cancelled.
    let d1 = f.door();
    let run = d1.open_run().unwrap();
    let AgentOutcome::AwaitingApproval(wait) = d1.agent_action(run, &f.post("/b7")).unwrap() else {
        panic!("a POST waits");
    };
    let d2 = f.door();
    d2.adopt(run, wait.id).unwrap();
    let batch = halt(&d2);
    assert!(f.cancelled(run));
    assert_eq!(f.state(wait.id), CommitmentState::Revoked);
    let _ = batch.write();

    // Halt, then adopt on the halted door: refused, nothing changes
    // (plant (i): a halted door's adopt joins anyway).
    let d1 = f.door();
    let run = d1.open_run().unwrap();
    let AgentOutcome::AwaitingApproval(wait) = d1.agent_action(run, &f.post("/b7-i")).unwrap()
    else {
        panic!("a POST waits");
    };
    let d2 = f.door();
    let batch = halt(&d2);
    assert_eq!(d2.adopt(run, wait.id).unwrap_err(), AuthorityError::Halted);
    assert!(!f.cancelled(run), "the run's state is unchanged");
    assert_eq!(f.state(wait.id), CommitmentState::Prepared);
    assert!(matches!(d2.commitment(wait.id), Lookup::NotMine { .. }));
    let _ = batch.write();
    // The run is still D1's alone: D1's halt cancels it.
    let batch = halt(&d1);
    assert!(f.cancelled(run));
    let _ = batch.write();

    // A complete() paused holding entries while an adoption waits for
    // entries (the entries -> runs order), then a halt: no deadlock.
    let d1 = f.door();
    let run = d1.open_run().unwrap();
    let counter = Counter::default();
    let id = f.counting(run, &counter);
    let AgentOutcome::AwaitingApproval(wait) = d1.agent_action(run, &f.post("/b7-e")).unwrap()
    else {
        panic!("a POST waits");
    };
    let d2 = Arc::new(f.door());
    let paused = pause_at(&f.gov, at::COMPLETE_CHECKED);
    let gov = f.gov.clone();
    let authorizing = spawn(move || gov.control.authorize(id, &mother(), run, &Owner));
    paused.reached();
    let adopting = {
        let d2 = d2.clone();
        spawn(move || d2.adopt(run, wait.id).map(|v| v.id))
    };
    let halting = {
        let d2 = d2.clone();
        spawn(move || halt(&d2))
    };
    paused.release();
    let _ = join(&authorizing);
    let adopted = join(&adopting);
    let batch = join(&halting);
    // Whichever came first: an adoption before the halt is cancelled with
    // it; one after is refused.
    match adopted {
        Ok(_) => assert!(f.cancelled(run)),
        Err(e) => assert_eq!(e, AuthorityError::Halted),
    }
    let _ = batch.write();

    // Several memberships: D1's halt cancels a run in D1 and D2, and D2's
    // other runs are unaffected.
    let d1 = f.door();
    let d2 = f.door();
    let shared = d1.open_run().unwrap();
    let AgentOutcome::AwaitingApproval(wait) = d1.agent_action(shared, &f.post("/b7-s")).unwrap()
    else {
        panic!("a POST waits");
    };
    d2.adopt(shared, wait.id).unwrap();
    let own = d2.open_run().unwrap();
    let batch = halt(&d1);
    assert!(f.cancelled(shared));
    assert!(!f.cancelled(own));
    assert!(matches!(
        d2.agent_action(own, &f.get("/b7-own")).unwrap(),
        AgentOutcome::Done(_)
    ));
    let _ = batch.write();

    // Refused with nothing changed: another agent's run, an owner run, a
    // cancelled run, a commitment of another run.
    let d = f.door();
    let other = AgentId::new("another-agent").unwrap();
    let theirs = f.gov.open_run(other.clone(), RunOrigin::AgentGoal).unwrap();
    let AgentOutcome::AwaitingApproval(their) = f
        .gov
        .agent_action(&other, theirs, &f.post("/b7-t"))
        .unwrap()
    else {
        panic!("a POST waits");
    };
    assert!(d.adopt(theirs, their.id).is_err());
    let (_, owners) = f.gov.open_owner_run(vec![]).unwrap();
    assert!(d.adopt(owners, their.id).is_err());
    let mine = d.open_run().unwrap();
    assert!(
        d.adopt(mine, their.id).is_err(),
        "a commitment of another run"
    );
    assert!(!f.cancelled(theirs) && !f.cancelled(owners) && !f.cancelled(mine));
    f.gov.probe_locks();
}

// ---------------------------------------------------------------- B8

/// B8 (§11 C5): execute has taken the effect out of the pending table and
/// is revalidating it, before `begin`; the halt revokes the commitment and
/// finds nothing pending; `begin` then refuses, and the effect never runs.
/// And: the halt removes the effect before execute takes it: unknown, zero
/// effects. And: a proposal paused before its place is filled during the
/// halt is not held, and its place is given back.
#[test]
fn m2c_b8_pending_effects_meet_the_halt() {
    let f = fx();
    let door = f.door();
    let run = door.open_run().unwrap();
    let counter = Counter::default();
    let gate = Gate::new();
    let id = f
        .propose(
            run,
            Effect {
                counter: counter.clone(),
                hold_revalidation: Some(gate.clone()),
                hold_execution: None,
            },
        )
        .unwrap();
    f.authorize(id, run).unwrap();
    let gov = f.gov.clone();
    let executing = spawn(move || gov.control.execute(id, &mother(), run));
    gate.reached();
    let batch = halt(&door);
    assert_eq!(f.state(id), CommitmentState::Revoked);
    gate.open();
    let executed = join(&executing);
    assert!(executed.is_err());
    assert_eq!(counter.ran.load(Ordering::SeqCst), 0);
    let _ = batch.write();

    // Removed before execute takes it.
    let door = f.door();
    let run = door.open_run().unwrap();
    let id = f.counting(run, &counter);
    f.authorize(id, run).unwrap();
    let batch = halt(&door);
    assert_eq!(
        f.execute(id, run).map(|_| ()).unwrap_err(),
        AuthorityError::UnknownCommitment
    );
    assert_eq!(counter.ran.load(Ordering::SeqCst), 0);
    let _ = batch.write();

    // A proposal paused before its place is filled.
    let door = Arc::new(f.door());
    let run = door.open_run().unwrap();
    let paused = pause_at(&f.gov, at::PLACE_BEFORE_FILL);
    let preparing = {
        let fx = Fx2 {
            gov: f.gov.clone(),
            grant: f.grant,
        };
        spawn(move || fx.propose(run))
    };
    paused.reached();
    let batch = halt(&door);
    paused.release();
    if let Ok(id) = join(&preparing) {
        assert_ne!(f.state(id), CommitmentState::Prepared);
        assert_eq!(
            f.execute(id, run).map(|_| ()).unwrap_err(),
            AuthorityError::UnknownCommitment,
            "not held"
        );
    }
    assert_eq!(
        f.gov.control.pending_places(),
        0,
        "its place was given back"
    );
    let _ = batch.write();

    // Positive control.
    let door = f.door();
    let run = door.open_run().unwrap();
    let id = f.counting(run, &counter);
    f.authorize(id, run).unwrap();
    f.execute(id, run).unwrap();
    assert_eq!(counter.ran.load(Ordering::SeqCst), 1);
    f.gov.probe_locks();
}

// ---------------------------------------------------------------- B9

/// B9 (Codex R2-11): no I/O, against a real stall, with the halted-door
/// precheck. Unrelated expired work waits (settling would write); another
/// writer holds the store's mutex. The halt returns while the store is
/// stalled; the halted door's open_run refuses without settling;
/// agent_action and adopt refuse. After release, the batch writes
/// door_halted, then run_cancelled, then the revocations.
#[test]
fn m2c_b9_the_halt_does_no_io_against_a_stalled_store() {
    let scratch = Scratch::new();
    let f = fx_store(&scratch);
    // Unrelated work that has expired: settling would end it, recorded.
    let other = f.door();
    let other_run = other.open_run().unwrap();
    let counter = Counter::default();
    let expired = f.counting(other_run, &counter);
    // The door to halt, with a waiting action.
    let door = f.door();
    let run = door.open_run().unwrap();
    let AgentOutcome::AwaitingApproval(wait) = door.agent_action(run, &f.post("/b9")).unwrap()
    else {
        panic!("a POST waits");
    };
    let mine = f.counting(run, &counter);
    f.clock.advance(Duration::from_secs(120));
    // Another writer stalls inside the store.
    let stall = Gate::new();
    f.primary
        .as_ref()
        .unwrap()
        .store()
        .plant_fault(Fault::Stall(stall.clone()));
    let writer = {
        let gov = f.gov.clone();
        spawn(move || {
            gov.authority()
                .record_display(EvidencePhase::DisplayStarted, vec![])
        })
    };
    stall.reached();
    let batch = halt(&door);
    assert_eq!(door.open_run(), Err(AuthorityError::Halted));
    assert_eq!(
        door.agent_action(run, &f.get("/b9-get")).unwrap_err(),
        AuthorityError::Halted
    );
    assert_eq!(
        door.adopt(run, wait.id).unwrap_err(),
        AuthorityError::Halted
    );
    assert_eq!(
        f.gov.authority().commitments().view(expired).unwrap().state,
        CommitmentState::Prepared,
        "nothing settled while the store was stalled"
    );
    stall.open();
    let _ = join(&writer);
    let before = f.recent.records().len();
    let outcome = batch.write();
    let written: Vec<EvidencePhase> = f.recent.records()[before..]
        .iter()
        .map(|r| r.phase)
        .collect();
    assert_eq!(written.first(), Some(&EvidencePhase::DoorHalted));
    assert_eq!(written.get(1), Some(&EvidencePhase::RunCancelled));
    assert!(written[2..].iter().all(|p| *p == EvidencePhase::Revoked));
    assert_eq!(written.len(), 4, "door, run, two revocations: {written:?}");
    assert_eq!(outcome.recorded, 4);
    assert_eq!(f.state(wait.id), CommitmentState::Revoked);
    assert_eq!(f.state(mine), CommitmentState::Revoked);
    f.gov.probe_locks();
}

// ---------------------------------------------------------------- B10

/// The halt counters.
fn counters(f: &Fx) -> crate::governed::HaltRecords {
    f.gov.status().halt_records
}

/// Every record a batch took is accounted for exactly once.
fn balanced(f: &Fx, pushed: u64) {
    let c = counters(f);
    assert_eq!(c.pending, 0, "{c:?}");
    assert_eq!(c.recorded + c.owed + c.uncertain + c.lost, pushed, "{c:?}");
}

/// A door with `members` runs, each with one prepared commitment.
fn members(f: &Fx, door: &AgentDoor, members: usize) -> Vec<(RunId, CommitmentId)> {
    let counter = Counter::default();
    (0..members)
        .map(|_| {
            let run = door.open_run().unwrap();
            (run, f.counting(run, &counter))
        })
        .collect()
}

/// B10-a, B10-c: a written batch counts every record recorded; a dropped
/// one counts them lost, does no I/O, and the halt still refuses.
#[test]
fn m2c_b10_a_c_a_batch_is_written_or_counted_lost() {
    let f = fx();
    let door = f.door();
    members(&f, &door, 2);
    let batch = halt(&door);
    let len = batch.len() as u64;
    assert_eq!(len, 5, "door_halted, two run_cancelled, two revoked");
    let outcome = batch.write();
    assert_eq!(outcome.recorded as u64, len);
    assert_eq!(counters(&f).recorded, len);
    balanced(&f, len);

    let door = f.door();
    members(&f, &door, 1);
    let seen = f.recent.records().len();
    let batch = halt(&door);
    let dropped = batch.len() as u64;
    drop(batch);
    assert_eq!(f.recent.records().len(), seen, "no I/O in Drop");
    assert_eq!(counters(&f).lost, dropped);
    balanced(&f, len + dropped);
    assert_eq!(door.open_run(), Err(AuthorityError::Halted));
    f.gov.probe_locks();
}

/// B10-b: a store that refuses inserts keeps the records owed; a failed
/// COMMIT latches the store, and every later record is uncertain.
#[test]
fn m2c_b10_b_owed_and_uncertain() {
    let scratch = Scratch::new();
    let f = fx_store(&scratch);
    let door = f.door();
    members(&f, &door, 1);
    let batch = halt(&door);
    let store = f.primary.as_ref().unwrap().store();
    store.refuse_inserts(true);
    let outcome = batch.write();
    assert_eq!(outcome.owed, 3);
    store.refuse_inserts(false);

    let door = f.door();
    members(&f, &door, 1);
    let batch = halt(&door);
    store.plant_fault(Fault::FailCommit);
    let outcome = batch.write();
    assert_eq!(outcome.uncertain, 3, "{outcome:?}");
    assert!(f.primary.as_ref().unwrap().health().latched);
    balanced(&f, 6);
}

/// A projection that panics while projecting the `k`-th record of `phase`
/// it sees once armed.
struct PanicOn {
    phase: EvidencePhase,
    k: usize,
    armed: AtomicBool,
    seen: AtomicUsize,
}

impl crate::authority::evidence::EvidenceProjection for PanicOn {
    fn project(&self, record: &EvidenceRecord) {
        if self.armed.load(Ordering::SeqCst) && record.phase == self.phase {
            let n = self.seen.fetch_add(1, Ordering::SeqCst) + 1;
            if n == self.k {
                panic!("a projection panics on record {n} (test)");
            }
        }
    }
}

fn fx_store_with(
    scratch: &Scratch,
    projection: Arc<dyn crate::authority::evidence::EvidenceProjection>,
) -> Fx {
    let recent = Arc::new(MemoryEvidence::new(100_000));
    let primary =
        PrimaryEvidence::open(&scratch.store(), vec![recent.clone(), projection]).unwrap();
    let clock = Arc::new(ManualClock::default());
    let gov = Arc::new(
        GovernedControl::with_secrets(
            &scratch.path("runtime"),
            Arc::new(Vault),
            primary.clone(),
            clock.clone(),
        )
        .unwrap(),
    );
    finish(gov, recent, Some(primary), clock, Box::new(()))
}

/// B10-d: a projection panics on record k of the batch: k is uncertain
/// (committed, then the panic), the rest are lost, the counters balance.
#[test]
fn m2c_b10_d_a_projection_panic_mid_write() {
    let scratch = Scratch::new();
    let panics = Arc::new(PanicOn {
        phase: EvidencePhase::RunCancelled,
        k: 2,
        armed: AtomicBool::new(false),
        seen: AtomicUsize::new(0),
    });
    let f = fx_store_with(&scratch, panics.clone());
    let door = f.door();
    members(&f, &door, 3);
    let batch = halt(&door);
    let len = batch.len() as u64;
    assert_eq!(len, 7);
    panics.armed.store(true, Ordering::SeqCst);
    let written = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| batch.write()));
    assert!(written.is_err());
    let c = counters(&f);
    assert_eq!(c.recorded, 2, "door_halted and the first run_cancelled");
    assert_eq!(c.uncertain, 1, "the one being projected");
    assert_eq!(c.lost, len - 3);
    balanced(&f, len);
}

/// B10-e: a panic forced inside the halt between its entries step and its
/// lease step: the records made so far are counted lost, the group stays
/// halted, and admission is refused.
#[test]
fn m2c_b10_e_a_panic_inside_the_halt() {
    let f = fx();
    let door = f.door();
    let made = members(&f, &door, 2);
    f.gov
        .hooks()
        .arm(at::HALT_AFTER_ENTRIES, || panic!("inside the halt (test)"));
    let halted = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| halt(&door)));
    assert!(halted.is_err());
    assert_eq!(counters(&f).lost, 5);
    balanced(&f, 5);
    assert_eq!(door.open_run(), Err(AuthorityError::Halted));
    assert_eq!(
        door.agent_action(made[0].0, &f.get("/b10e")).unwrap_err(),
        AuthorityError::Halted
    );
    assert_eq!(f.reached("/b10e"), 0);
    f.gov.probe_locks();
}

/// B10-f: a batch outstanding while the store's acknowledgement and session
/// close are written: both succeed, with no order claimed.
#[test]
fn m2c_b10_f_an_outstanding_batch_blocks_nothing() {
    let scratch = Scratch::new();
    let f = fx_store(&scratch);
    let door = f.door();
    members(&f, &door, 1);
    let batch = halt(&door);
    let primary = f.primary.as_ref().unwrap();
    let covers = primary.store().rows().last().map_or(0, |row| row.seq);
    assert!(primary.acknowledge(covers).is_ok());
    assert!(matches!(
        primary.close_session(),
        crate::authority::evidence::EndStatus::Recorded { .. }
    ));
    let _ = batch.write();
}

/// B10-g (Codex R2-12): a maximal batch (4,096 runs, 4,096 commitments) on
/// a store that refuses inserts: 8,192 records owed, then the latch, then
/// uncertain for the rest; the next door issue is refused (latched).
#[test]
fn m2c_b10_g_a_maximal_batch_meets_the_owed_capacity() {
    let scratch = Scratch::new();
    let f = fx_store(&scratch);
    let door = f.door();
    let mut runs = Vec::new();
    for _ in 0..4096 {
        runs.push(door.open_run().unwrap());
    }
    for run in &runs {
        let action = PreparedAction {
            kind: CapabilityKind::Perception,
            class: EffectClass::R1,
            operation: "m2c.observe",
            target: TargetIdentity {
                display: "m2c target".into(),
                digest: target(),
            },
            parameters: parameters(),
            grants: vec![f.grant],
            leases: vec![],
            summary: vec!["m2c".into()],
        };
        f.gov
            .authority()
            .commitments()
            .prepare(&mother(), *run, action, Duration::from_secs(3600))
            .unwrap();
    }
    let batch = halt(&door);
    assert_eq!(batch.len(), 1 + 4096 + 4096);
    f.primary.as_ref().unwrap().store().refuse_inserts(true);
    let outcome = batch.write();
    assert_eq!(outcome.owed, 8192, "{outcome:?}");
    assert_eq!(outcome.uncertain, 1, "{outcome:?}");
    assert!(f.primary.as_ref().unwrap().health().latched);
    assert_eq!(
        f.gov
            .issue_agent_door(mother(), Arc::new(Owner))
            .unwrap_err(),
        AuthorityError::EvidenceUnavailable
    );
}

/// B10-h (§11 C2): a panic inside `revoke_where` after its first
/// revocation record exists and before it returns: that record is counted
/// lost, the counters balance, the group stays halted, admission refused.
#[test]
fn m2c_b10_h_a_panic_inside_revoke_where_loses_nothing_uncounted() {
    let f = fx();
    let door = f.door();
    let made = members(&f, &door, 1);
    let counter = Counter::default();
    f.counting(made[0].0, &counter);
    f.gov.hooks().arm(at::REVOKE_WHERE_FIRST, || {
        panic!("inside revoke_where (test)")
    });
    let halted = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| halt(&door)));
    assert!(halted.is_err());
    let c = counters(&f);
    assert_eq!(
        c.lost, 3,
        "door_halted, run_cancelled and the first revocation: {c:?}"
    );
    balanced(&f, 3);
    assert_eq!(door.open_run(), Err(AuthorityError::Halted));
    assert_eq!(
        door.agent_action(made[0].0, &f.get("/b10h")).unwrap_err(),
        AuthorityError::Halted
    );
    // The commitments lock was poisoned on purpose: no probe.
}

// ---------------------------------------------------------------- B11

/// B11: each lock the halt takes, poisoned on its own: the halt completes
/// every step, and admission is refused.
#[test]
fn m2c_b11_the_halt_completes_under_each_poisoned_lock() {
    for lock in ["runs", "entries", "lease slot", "leases", "pending"] {
        let f = fx();
        let door = Arc::new(f.door());
        let made = members(&f, &door, 1);
        let destination = Destination::parse(&f.server.origin()).unwrap();
        let _lease = f
            .gov
            .credentials
            .lease(
                &mother(),
                made[0].0,
                SPEC,
                &destination,
                Duration::from_secs(60),
            )
            .unwrap();
        let gov = f.gov.clone();
        let poisoner = std::thread::spawn(move || match lock {
            "runs" => gov.authority().runs().panic_holding_the_runs_lock(),
            "entries" => gov.authority().commitments().panic_holding_entries(),
            "lease slot" => gov.authority().commitments().panic_holding_lease_slot(),
            "leases" => gov.credentials.panic_holding_leases(),
            _ => gov.control.panic_holding_pending(),
        });
        assert!(poisoner.join().is_err(), "{lock}: poisoned");
        let halting = {
            let door = door.clone();
            spawn(move || {
                let batch = halt(&door);
                batch_phases(&batch)
            })
        };
        let phases = join(&halting);
        assert_eq!(phases.first(), Some(&EvidencePhase::DoorHalted), "{lock}");
        assert!(phases.contains(&EvidencePhase::RunCancelled), "{lock}");
        assert!(phases.contains(&EvidencePhase::Revoked), "{lock}");
        let open = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| door.open_run()));
        assert!(
            open.map_or(true, |r| r.is_err()),
            "{lock}: admission refused"
        );
        let acting = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            door.agent_action(made[0].0, &f.get("/b11"))
        }));
        assert!(acting.map_or(true, |r| r.is_err()), "{lock}");
        assert_eq!(f.reached("/b11"), 0, "{lock}");
    }
}

// ---------------------------------------------------------------- B12

/// B12: the halt's scope, and the halted door method by method.
#[test]
fn m2c_b12_scope_and_the_halted_door() {
    let f = fx();
    let owner = Arc::new(Counting::default());
    let door = f.gov.issue_agent_door(mother(), owner.clone()).unwrap();
    let run = door.open_run().unwrap();
    let AgentOutcome::AwaitingApproval(wait) = door.agent_action(run, &f.post("/b12")).unwrap()
    else {
        panic!("a POST waits");
    };
    let other = f.door();
    let other_run = other.open_run().unwrap();
    let (_, owners) = f.gov.open_owner_run(vec![]).unwrap();
    let plain = f.gov.open_run(mother(), RunOrigin::AgentGoal).unwrap();
    let batch = halt(&door);
    let _ = batch.write();

    // Other doors, owner runs and the front door's own runs: unaffected.
    assert!(!f.cancelled(other_run) && !f.cancelled(owners) && !f.cancelled(plain));
    assert!(matches!(
        other.agent_action(other_run, &f.get("/b12-other")).unwrap(),
        AgentOutcome::Done(_)
    ));
    assert!(f.gov.open_run(mother(), RunOrigin::AgentGoal).is_ok());

    // The halted door, method by method.
    let before = (
        owner.missions.load(Ordering::SeqCst),
        owner.helds.load(Ordering::SeqCst),
    );
    assert_eq!(door.open_run(), Err(AuthorityError::Halted));
    assert_eq!(
        door.agent_action(run, &f.get("/b12-x")).unwrap_err(),
        AuthorityError::Halted
    );
    assert_eq!(
        door.adopt(run, wait.id).unwrap_err(),
        AuthorityError::Halted
    );
    assert_eq!(door.finish_when_settled(run), Err(AuthorityError::Halted));
    assert_eq!(
        door.confirm_mission(&mission_request("m-1/1/1"))
            .unwrap_err(),
        AuthorityError::Halted
    );
    assert_eq!(
        door.confirm_held_mission(&held_request("m-1/2/2"))
            .unwrap_err(),
        AuthorityError::Halted
    );
    assert_eq!(
        (
            owner.missions.load(Ordering::SeqCst),
            owner.helds.load(Ordering::SeqCst)
        ),
        before,
        "refused before any dialog"
    );
    let cancelled = door.cancel_run(run).expect("cancel_run is allowed");
    assert!(cancelled.is_empty(), "nothing left to cancel");
    let again = halt(&door);
    assert!(again.is_empty(), "a second halt makes no transition");
    let _ = (cancelled.write(), again.write());
    assert!(!door.emergency_stopped());
    let _ = door.clock().wall_ms();
    assert!(matches!(door.commitment(wait.id), Lookup::Mine(_)));
    assert_eq!(f.reached("/b12-x"), 0);

    // The Owner's emergency stop and native resume lift no door halt.
    f.gov.emergency_stop();
    f.gov.resume(&Owner).unwrap();
    assert!(!f.gov.status().emergency_stopped);
    assert_eq!(door.open_run(), Err(AuthorityError::Halted));
    assert!(f.door().open_run().is_ok(), "a new door works");
    assert_eq!(f.gov.status().doors_halted, 1);
    f.gov.probe_locks();
}

// ---------------------------------------------------------------- B13

/// B13 (§5): the halt forced into every interleaving point against the
/// other lock-taking calls, both ways: the call paused at its own hook
/// while the halt runs, and the halt paused between each pair of its steps
/// while the call runs. Each schedule ends with bounded joins and
/// `probe_locks`; none deadlocks.
#[test]
fn m2c_b13_the_lock_order_has_no_cycle() {
    // The calls, paused at their own hooks, while a halt runs whole.
    for at in [
        at::COMPLETE_CHECKED,
        at::PREPARE_BEFORE_INSERT,
        at::PLACE_BEFORE_FILL,
        at::LEASE_RECORDED,
    ] {
        let f = fx();
        let door = Arc::new(f.door());
        let run = door.open_run().unwrap();
        let counter = Counter::default();
        let id = f.counting(run, &counter);
        let paused = pause_at(&f.gov, at);
        let gov = f.gov.clone();
        let grant = f.grant;
        let destination = Destination::parse(&f.server.origin()).unwrap();
        let call = spawn(move || match at {
            at::COMPLETE_CHECKED => gov.control.authorize(id, &mother(), run, &Owner).is_ok(),
            at::LEASE_RECORDED => gov
                .credentials
                .lease(&mother(), run, SPEC, &destination, Duration::from_secs(60))
                .is_ok(),
            _ => Fx2 { gov, grant }.propose(run).is_ok(),
        });
        paused.reached();
        let halting = {
            let door = door.clone();
            spawn(move || halt(&door).write())
        };
        paused.release();
        let _ = join(&call);
        let _ = join(&halting);
        f.gov.probe_locks();
    }

    // The halt, paused between each pair of its steps, while each call
    // runs whole.
    for step in [
        at::HALT_AFTER_RUNS,
        at::HALT_AFTER_ENTRIES,
        at::HALT_AFTER_LEASES,
    ] {
        for call in [
            "prune",
            "revoke_run",
            "stop_all",
            "door cancel_run",
            "adopt",
            "complete",
        ] {
            let f = fx();
            let door = Arc::new(f.door());
            let run = door.open_run().unwrap();
            let AgentOutcome::AwaitingApproval(wait) =
                door.agent_action(run, &f.post("/b13")).unwrap()
            else {
                panic!("a POST waits");
            };
            let counter = Counter::default();
            let id = f.counting(run, &counter);
            let other = Arc::new(f.door());
            let paused = pause_at(&f.gov, step);
            let halting = {
                let door = door.clone();
                spawn(move || halt(&door).write())
            };
            paused.reached();
            let gov = f.gov.clone();
            let (d, o) = (door.clone(), other.clone());
            let running = spawn(move || match call {
                "prune" => gov.settle(),
                "revoke_run" => {
                    let _ = gov.cancel_run(run);
                }
                "stop_all" => {
                    gov.emergency_stop();
                }
                "door cancel_run" => {
                    let _ = d.cancel_run(run).map(RecordBatch::write);
                }
                "adopt" => {
                    let _ = o.adopt(run, wait.id);
                }
                _ => {
                    let _ = gov.control.authorize(id, &mother(), run, &Owner);
                }
            });
            let _ = join(&running);
            paused.release();
            let _ = join(&halting);
            f.gov.probe_locks();
        }
    }

    // A projection that halts the door and presses the emergency stop while
    // it projects a Started record (it runs with no broker lock held).
    let scratch = Scratch::new();
    let doors: Arc<Mutex<Option<(Arc<AgentDoor>, Arc<GovernedControl>)>>> = Default::default();
    struct HaltsOnStart(Arc<Mutex<Option<(Arc<AgentDoor>, Arc<GovernedControl>)>>>);
    impl crate::authority::evidence::EvidenceProjection for HaltsOnStart {
        fn project(&self, record: &EvidenceRecord) {
            if record.phase == EvidencePhase::Started {
                let taken = self.0.lock().unwrap().take();
                if let Some((door, gov)) = taken {
                    let _ = door.halt(HaltReason::JournalFailed).write();
                    gov.emergency_stop();
                }
            }
        }
    }
    let f = fx_store_with(&scratch, Arc::new(HaltsOnStart(doors.clone())));
    let door = Arc::new(f.door());
    let run = door.open_run().unwrap();
    let counter = Counter::default();
    let id = f.counting(run, &counter);
    f.authorize(id, run).unwrap();
    *doors.lock().unwrap() = Some((door.clone(), f.gov.clone()));
    let gov = f.gov.clone();
    let executing = spawn(move || gov.control.execute(id, &mother(), run));
    assert!(join(&executing).is_err());
    assert_eq!(counter.ran.load(Ordering::SeqCst), 0);
    f.gov.probe_locks();
}

// ---------------------------------------------------------------- B15

/// B15 (Codex R2-7): concurrent halts. H1 is paused after its first step; a
/// `complete()` is paused holding the commitments lock; H2 runs whole: it
/// waits at its own entries barrier. Each transition lands in exactly one
/// batch, `door_halted` in H1's.
#[test]
fn m2c_b15_concurrent_halts_each_take_both_barriers() {
    let f = fx();
    let door = Arc::new(f.door());
    let made = members(&f, &door, 2);
    let counter = Counter::default();
    let authorized = f.counting(made[0].0, &counter);
    let order = Order::default();
    // The complete() has passed its run check and holds the commitments
    // lock; then H1 takes its first step and pauses before its second.
    let paused = pause_at(&f.gov, at::COMPLETE_CHECKED);
    let gov = f.gov.clone();
    let run = made[0].0;
    let authorizing = spawn(move || gov.control.authorize(authorized, &mother(), run, &Owner));
    paused.reached();
    let first = pause_at(&f.gov, at::HALT_AFTER_RUNS);
    let h1 = {
        let door = door.clone();
        spawn(move || halt(&door))
    };
    first.reached();
    {
        let order = order.clone();
        f.gov.hooks().every(at::HALT_ENTRIES_TAKEN, move || {
            order.mark("a halt took entries")
        });
    }
    let h2 = {
        let door = door.clone();
        spawn(move || halt(&door))
    };
    order.mark("complete released");
    paused.release();
    let _ = join(&authorizing);
    let b2 = join(&h2);
    first.release();
    let b1 = join(&h1);
    assert!(order.index("complete released") < order.index("a halt took entries"));
    let p1 = batch_phases(&b1);
    let p2 = batch_phases(&b2);
    assert_eq!(
        p1.iter()
            .filter(|p| **p == EvidencePhase::DoorHalted)
            .count(),
        1
    );
    assert!(!p2.contains(&EvidencePhase::DoorHalted));
    let all: Vec<EvidencePhase> = p1.iter().chain(p2.iter()).copied().collect();
    assert_eq!(
        all.iter()
            .filter(|p| **p == EvidencePhase::RunCancelled)
            .count(),
        2
    );
    assert_eq!(
        all.iter().filter(|p| **p == EvidencePhase::Revoked).count(),
        3
    );
    let _ = (b1.write(), b2.write());
    f.gov.probe_locks();
}

// ---------------------------------------------------------------- B16

/// B16 (Codex R2-1): group confinement under a halt.
#[test]
fn m2c_b16_a_halted_group_confines_an_adopting_door() {
    let f = fx();
    let d1 = f.door();
    let run = d1.open_run().unwrap();
    // Positive control: D1's own call works before D1 halts.
    assert!(matches!(
        d1.agent_action(run, &f.get("/b16-ok")).unwrap(),
        AgentOutcome::Done(_)
    ));
    let d2 = Arc::new(f.door());
    assert_eq!(
        d2.agent_action(run, &f.get("/b16-x")).unwrap_err(),
        AuthorityError::NotMember
    );
    let AgentOutcome::AwaitingApproval(wait) = d1.agent_action(run, &f.post("/b16-w")).unwrap()
    else {
        panic!("a POST waits");
    };
    d2.adopt(run, wait.id).unwrap();
    let paused = pause_at(&f.gov, at::DOOR_MEMBER_CHECKED);
    let acting = {
        let d2 = d2.clone();
        let get = f.get("/b16-y");
        spawn(move || d2.agent_action(run, &get).map(|_| ()))
    };
    paused.reached();
    let batch = halt(&d2);
    paused.release();
    assert_eq!(join(&acting), Err(AuthorityError::Halted));
    assert_eq!(f.reached("/b16-x") + f.reached("/b16-y"), 0);
    let _ = batch.write();
    f.gov.probe_locks();
}

// ---------------------------------------------------------------- B17

/// B17 (A4 and A6-e at the broker): a halted door refuses consent and
/// resume before the dialog; a halt between the record and the post-record
/// check gives Halted, the record stands and no approval exists.
#[test]
fn m2c_b17_consent_and_resume_post_record_checks() {
    let f = fx();
    let owner = Arc::new(Counting::default());
    let door = f.gov.issue_agent_door(mother(), owner.clone()).unwrap();
    let batch = halt(&door);
    assert_eq!(
        door.confirm_mission(&mission_request("m-1/1/1"))
            .unwrap_err(),
        AuthorityError::Halted
    );
    assert_eq!(
        door.confirm_held_mission(&held_request("m-1/2/2"))
            .unwrap_err(),
        AuthorityError::Halted
    );
    assert_eq!(owner.missions.load(Ordering::SeqCst), 0);
    assert_eq!(owner.helds.load(Ordering::SeqCst), 0);
    let _ = batch.write();

    for (at, phase) in [
        (at::HELD_RECORDED, EvidencePhase::MissionResumeConfirmed),
        (at::MISSION_RECORDED, EvidencePhase::MissionConsented),
    ] {
        let door = Arc::new(f.door());
        let halted = Arc::new(Mutex::new(None));
        {
            let (door, halted) = (door.clone(), halted.clone());
            f.gov.hooks().arm(at, move || {
                *halted.lock().unwrap() = Some(halt(&door));
            });
        }
        let records = f.count(phase);
        let answer = if at == at::HELD_RECORDED {
            door.confirm_held_mission(&held_request("m-1/3/3"))
                .map(|_| ())
        } else {
            door.confirm_mission(&mission_request("m-1/4/4"))
                .map(|_| ())
        };
        assert_eq!(answer, Err(AuthorityError::Halted), "{at}");
        assert_eq!(f.count(phase), records + 1, "{at}: the record stands");
        let _ = halted.lock().unwrap().take().unwrap().write();
    }
    // Positive control.
    let door = f.door();
    assert!(door.confirm_held_mission(&held_request("m-1/5/5")).is_ok());
    assert!(door.confirm_mission(&mission_request("m-1/6/6")).is_ok());
}

// ---------------------------------------------------------------- B18

/// B18 (§11 C1): the halt keeps its own member list. An opening at the
/// registry's capacity, paused after settlement; the halt cancels member R
/// at its first step and pauses; the opening resumes and prunes R (it was
/// cancelled); the halt's later steps still revoke R's commitment, end its
/// lease and drop its pending effect.
#[test]
fn m2c_b18_the_halt_uses_its_member_snapshot() {
    let f = fx();
    let door = Arc::new(f.door());
    let member = door.open_run().unwrap();
    let counter = Counter::default();
    let id = f.counting(member, &counter);
    let destination = Destination::parse(&f.server.origin()).unwrap();
    let lease = f
        .gov
        .credentials
        .lease(
            &mother(),
            member,
            SPEC,
            &destination,
            Duration::from_secs(600),
        )
        .unwrap();
    // Fill the registry to its capacity with live runs of another door.
    let filler = f.door();
    while f.gov.authority().runs().views().len() < 4096 {
        filler.open_run().unwrap();
    }
    let opener = Arc::new(f.door());
    let opening_paused = pause_at(&f.gov, at::OPEN_SETTLED);
    let opening = {
        let opener = opener.clone();
        spawn(move || opener.open_run())
    };
    opening_paused.reached();
    let halt_paused = pause_at(&f.gov, at::HALT_AFTER_RUNS);
    let halting = {
        let door = door.clone();
        spawn(move || halt(&door))
    };
    halt_paused.reached();
    opening_paused.release();
    assert!(
        join(&opening).is_ok(),
        "the opening pruned the cancelled member"
    );
    assert!(
        f.gov.authority().runs().view(member).is_none(),
        "the member left the registry"
    );
    halt_paused.release();
    let batch = join(&halting);
    assert_eq!(f.state(id), CommitmentState::Revoked);
    assert!(!f.gov.credentials.is_live(lease));
    assert_eq!(f.gov.control.pending_held(member), 0);
    assert!(batch_phases(&batch).contains(&EvidencePhase::Revoked));
    let _ = batch.write();
    f.gov.probe_locks();
}

// ---------------------------------------------------------------- A7-c

/// A7-c (PR-A, waiting for the halt): a halt of another door during an
/// issue's dialog, and between its record and its registration, changes
/// nothing about this issue.
#[test]
fn m2c_a7_c_another_door_s_halt_does_not_touch_an_issue() {
    let f = fx();
    for at in [at::ISSUE_ANSWERED, at::ISSUE_RECORDED] {
        let other = Arc::new(f.door());
        let other_run = other.open_run().unwrap();
        let halted = Arc::new(Mutex::new(None));
        {
            let (other, halted) = (other.clone(), halted.clone());
            f.gov.hooks().arm(at, move || {
                *halted.lock().unwrap() = Some(halt(&other));
            });
        }
        let door = f.door();
        let run = door.open_run().expect("the new door works");
        assert!(matches!(
            door.agent_action(run, &f.get("/a7c")).unwrap(),
            AgentOutcome::Done(_)
        ));
        assert!(f.cancelled(other_run), "{at}: the other door was halted");
        let _ = halted.lock().unwrap().take().unwrap().write();
    }
    f.gov.probe_locks();
}

// ---------------------------------------------------------------- R1-R5

/// The store's file as a dead process leaves it: the old connection is
/// dropped with nothing more written, and the old control is let go.
fn crash(f: Fx) {
    f.primary.as_ref().unwrap().store().simulate_crash();
    drop(f);
}

fn door_with(
    f: &Fx,
    confirmer: Arc<dyn ControlConfirmer + Send + Sync>,
) -> Result<AgentDoor, AuthorityError> {
    f.gov.issue_agent_door(mother(), confirmer)
}

struct DoorNo;
impl ControlConfirmer for DoorNo {
    fn confirm_action(&self, _: &ActionConfirmation) -> bool {
        false
    }
    fn confirm_grant(&self, _: &GrantConfirmation) -> bool {
        false
    }
    fn confirm_resume(&self, _: &ResumeConfirmation) -> bool {
        false
    }
    fn confirm_acknowledgement(&self, _: &AcknowledgementConfirmation) -> bool {
        false
    }
}

/// An authorized commitment of `run`, started, its guard abandoned (an
/// action Executing when the process dies: its Started record is durable,
/// its end never comes).
fn executing(f: &Fx, run: RunId) -> CommitmentId {
    let counter = Counter::default();
    let id = f.counting(run, &counter);
    f.authorize(id, run).unwrap();
    let guard = f
        .gov
        .authority()
        .commitments()
        .begin(id, &mother(), run, &target(), &parameters())
        .unwrap();
    std::mem::forget(guard);
    id
}

fn unsettled(f: &Fx) -> Vec<String> {
    f.primary
        .as_ref()
        .unwrap()
        .store()
        .rows()
        .into_iter()
        .filter(|row| row.phase == "unsettled")
        .filter_map(|row| row.commitment)
        .collect()
}

/// R1: a door open, then a crash: the next issue asks; no gives no door,
/// yes a door. With no crash at all, the next issue asks too.
#[test]
fn m2c_r1_every_issue_after_a_crash_asks() {
    let scratch = Scratch::new();
    let f = fx_store(&scratch);
    let owner = Arc::new(Counting::default());
    let first = door_with(&f, owner.clone()).unwrap();
    let second = door_with(&f, owner.clone()).unwrap();
    assert_eq!(
        owner.doors.load(Ordering::SeqCst),
        2,
        "no crash: asked again"
    );
    drop((first, second));
    crash(f);
    let f = fx_store(&scratch);
    assert_eq!(
        door_with(&f, Arc::new(DoorNo)).unwrap_err(),
        AuthorityError::Declined
    );
    let owner = Arc::new(Counting::default());
    let door = door_with(&f, owner.clone()).unwrap();
    assert_eq!(owner.doors.load(Ordering::SeqCst), 1);
    assert!(door.open_run().is_ok());
}

/// R2 (S0): a halt whose batch was dropped, then a crash: no door_halted
/// row; the member action that had started is unsettled; the issue is
/// refused until the Owner acknowledges, then asks.
#[test]
fn m2c_r2_a_lost_batch_leaves_the_start_unsettled() {
    let scratch = Scratch::new();
    let f = fx_store(&scratch);
    let door = f.door();
    let run = door.open_run().unwrap();
    let started = executing(&f, run);
    drop(halt(&door));
    drop(door);
    crash(f);
    let f = fx_store(&scratch);
    let rows = f.primary.as_ref().unwrap().store().rows();
    assert!(!rows.iter().any(|row| row.phase == "door_halted"));
    assert_eq!(unsettled(&f), vec![started.to_string()]);
    assert!(f.gov.status().emergency_stopped);
    assert!(
        door_with(&f, Arc::new(Owner)).is_err(),
        "refused until acknowledged"
    );
    f.gov.resume(&Owner).unwrap();
    let owner = Arc::new(Counting::default());
    assert!(door_with(&f, owner.clone()).is_ok());
    assert_eq!(owner.doors.load(Ordering::SeqCst), 1);
}

/// R3 (S1): a halt whose batch was written up to record k (a projection
/// aborts the write after it), then a crash: exactly the rows up to k are
/// present; the started member action is unsettled.
#[test]
fn m2c_r3_a_prefix_written_leaves_the_rest_to_recovery() {
    let scratch = Scratch::new();
    let panics = Arc::new(PanicOn {
        phase: EvidencePhase::RunCancelled,
        k: 1,
        armed: AtomicBool::new(false),
        seen: AtomicUsize::new(0),
    });
    let f = fx_store_with(&scratch, panics.clone());
    let door = f.door();
    let run = door.open_run().unwrap();
    let other_run = door.open_run().unwrap();
    let started = executing(&f, run);
    let counter = Counter::default();
    let waiting = f.counting(other_run, &counter);
    let batch = halt(&door);
    panics.armed.store(true, Ordering::SeqCst);
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| batch.write())).is_err());
    drop(door);
    crash(f);
    let f = fx_store(&scratch);
    let rows = f.primary.as_ref().unwrap().store().rows();
    let count = |phase: &str| rows.iter().filter(|row| row.phase == phase).count();
    assert_eq!(count("door_halted"), 1);
    assert_eq!(
        count("run_cancelled"),
        1,
        "record k committed before the abort"
    );
    assert!(!rows.iter().any(
        |row| row.phase == "revoked" && row.commitment.as_deref() == Some(&waiting.to_string())
    ));
    assert!(unsettled(&f).contains(&started.to_string()));
    assert!(
        !unsettled(&f).contains(&waiting.to_string()),
        "never started"
    );
}

/// R4 (S2): a halt with everything written, then a crash: the rows are
/// present; the next issue still asks.
#[test]
fn m2c_r4_a_written_halt_still_needs_a_new_yes() {
    let scratch = Scratch::new();
    let f = fx_store(&scratch);
    let door = f.door();
    members(&f, &door, 1);
    let outcome = halt(&door).write();
    assert_eq!(outcome.recorded, 3);
    drop(door);
    crash(f);
    let f = fx_store(&scratch);
    let rows = f.primary.as_ref().unwrap().store().rows();
    for phase in ["door_halted", "run_cancelled", "revoked"] {
        assert!(rows.iter().any(|row| row.phase == phase), "{phase}");
    }
    assert!(unsettled(&f).is_empty());
    let owner = Arc::new(Counting::default());
    assert!(door_with(&f, owner.clone()).is_ok());
    assert_eq!(owner.doors.load(Ordering::SeqCst), 1);
}

/// R5: an unsettled action at the crash: control starts emergency-stopped,
/// and the door's gating record is refused until the Owner acknowledges.
#[test]
fn m2c_r5_recovery_comes_before_any_door() {
    let scratch = Scratch::new();
    let f = fx_store(&scratch);
    let door = f.door();
    let run = door.open_run().unwrap();
    executing(&f, run);
    drop(door);
    crash(f);
    let f = fx_store(&scratch);
    assert!(f.gov.status().emergency_stopped);
    let owner = Arc::new(Counting::default());
    assert!(door_with(&f, owner.clone()).is_err());
    assert_eq!(
        owner.doors.load(Ordering::SeqCst),
        0,
        "refused before the dialog"
    );
    f.gov.resume(&Owner).unwrap();
    assert!(door_with(&f, owner.clone()).is_ok());
}
