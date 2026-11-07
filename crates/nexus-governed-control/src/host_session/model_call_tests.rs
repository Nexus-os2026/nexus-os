//! SB-3A frozen oracles: deny-only invocation observation and the private
//! original-door R2 binding. No ticket, charge, transport or public route.
use super::model_tests::{pin, Peer};
use super::*;
use crate::authority::approval::{
    AcknowledgementConfirmation, ActionConfirmation, AgentDoorConfirmation, ControlConfirmer,
    GrantConfirmation, MissionConfirmation, ResumeConfirmation,
};
use crate::authority::clock::ManualClock;
use crate::authority::ids::{GrantId, RunId};
use crate::authority::run::{InferenceAdmission, RunRegistry};
use crate::inference::{InvocationOwner, ModelIdentity};
use std::sync::atomic::AtomicBool;

const HASH: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const HASH2: &str = "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";
const LABEL: &str = "msn-000001/1/1";
const LABEL2: &str = "msn-000001/1/2";

struct Owner;
impl ControlConfirmer for Owner {
    fn confirm_action(&self, _: &ActionConfirmation) -> bool {
        false
    }
    fn confirm_grant(&self, _: &GrantConfirmation) -> bool {
        true
    }
    fn confirm_resume(&self, _: &ResumeConfirmation) -> bool {
        false
    }
    fn confirm_acknowledgement(&self, _: &AcknowledgementConfirmation) -> bool {
        false
    }
    fn confirm_mission(&self, _: &MissionConfirmation) -> bool {
        true
    }
    fn confirm_agent_door(&self, _: &AgentDoorConfirmation) -> bool {
        true
    }
}
fn mission(hash: &str, label: &str) -> MissionConfirmation {
    MissionConfirmation {
        contract_hash: hash.into(),
        request: label.into(),
        summary: vec!["One file".into()],
        page_facts: None,
    }
}

/// Original Bootstrap registration, Q, D with its retained grant, cached
/// native consent and the existing File run R: every R2 prerequisite.
struct Bind {
    session: Arc<HostSession>,
    clock: Arc<ManualClock>,
    q: SessionRequest,
    door: SessionDoor,
    run: RunId,
    identity: [u8; 32],
    primary: Arc<crate::evidence_store::PrimaryEvidence>,
    _keep: (
        Option<Peer>,
        crate::authority::file_session::FileAttachment,
        [crate::harness_tests::TempRoot; 2],
    ),
}
impl Bind {
    fn new() -> Self {
        Self::with(true, true)
    }
    fn with(register: bool, files: bool) -> Self {
        Self::make(register.then(|| Peer::chat(None, None)), None, files)
    }
    /// SB-3B: registered on `peer`, or on a peer another process owns.
    fn make(peer: Option<Peer>, endpoint: Option<&str>, files: bool) -> Self {
        assert!(crate::files::make_non_dumpable());
        let root = crate::harness_tests::temp_root("sb3a-bind");
        let selected = crate::harness_tests::temp_root("sb3a-workspace");
        let workspace = selected.0.path().join("selected/workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        let primary = crate::evidence_store::PrimaryEvidence::open(
            &root.0.path().join("p3-evidence/evidence.sqlite3"),
            vec![],
        )
        .unwrap();
        let clock = Arc::new(ManualClock::default());
        let session = Arc::new(
            HostSession::open(
                SessionSeed::new().unwrap(),
                root.0.path(),
                primary.clone(),
                clock.clone(),
                Arc::new(Owner),
            )
            .unwrap(),
        );
        session.bootstrap_workspace(&workspace, &[]).unwrap();
        let endpoint = (peer.as_ref().map(|p| p.endpoint.clone())).or(endpoint.map(str::to_owned));
        let identity = match &endpoint {
            Some(endpoint) => {
                let view = session.register_model_private(endpoint, &pin()).unwrap();
                hex::decode(view.identity).unwrap().try_into().unwrap()
            }
            None => ModelIdentity::from_spec(&pin()).unwrap().digest(),
        };
        session.finish_bootstrap().unwrap();
        let q = session.ingress().accept(CommandKind::Run, 1).unwrap();
        let door = session.open_file_door(&q).unwrap();
        let attachment = door.file_port().unwrap().attach().unwrap();
        door.claim_file_session(&attachment).unwrap();
        door.confirm_mission(&mission(HASH, LABEL)).unwrap();
        let run = door.open_file_run(&attachment).unwrap();
        if files {
            door.admit_files(run, HASH, LABEL).unwrap();
        }
        Self {
            session,
            clock,
            q,
            door,
            run,
            identity,
            primary,
            _keep: (peer, attachment, [root, selected]),
        }
    }
    fn peer(&self) -> &Peer {
        self._keep.0.as_ref().expect("this fixture's own peer")
    }
    fn bind_as(&self, door: &SessionDoor, hash: &str, label: &str) -> Result<(), &'static str> {
        door.bind_inference_private(self.run, hash, label, self.identity)
            .map_err(|e| e.class())
    }
    fn bind(&self) -> Result<(), &'static str> {
        self.bind_as(&self.door, HASH, LABEL)
    }
    fn control(&self) -> Arc<crate::governed::GovernedControl> {
        self.session.control_for_test()
    }
    fn runs(&self) -> Arc<RunRegistry> {
        self.control().authority().runs().clone()
    }
    fn bound(&self) -> Option<InferenceAdmission> {
        self.runs().inference_admission(self.run)
    }
    fn grant(&self) -> GrantId {
        match self.door.kind {
            DoorKind::Original { file_grant } => file_grant,
            DoorKind::Continuation => unreachable!("an original door"),
        }
    }
    /// Same D, another context or kind: one association varies.
    fn door_with(&self, context: SessionContext, kind: DoorKind) -> SessionDoor {
        SessionDoor {
            door: self.door.door.clone(),
            origin_request: self.door.origin_request,
            context,
            core: self.door.core.clone(),
            kind,
        }
    }
    fn unchanged(&self, case: &str) {
        assert_eq!(
            self.runs().model_accounting(self.run),
            Some((0, 0)),
            "{case}: no attempt or model pin"
        );
    }
    /// F12: exactly one component differs from a valid binding.
    fn vary(&self, case: &str) -> Result<(), &'static str> {
        let door = match case {
            "s" => {
                let other = SessionSeed::new().unwrap();
                let word = self.session.core.cell.session_word();
                other.core.cell.session_compare(0, word).unwrap();
                let context = SessionContext {
                    cell: other.core.cell.scoped(self.q.scope.clone()),
                    scope: self.q.scope.clone(),
                    run: None,
                };
                self.door_with(context, self.door.kind)
            }
            "q" => {
                let q2 = self.session.ingress().accept(CommandKind::Run, 2).unwrap();
                self.door_with(self.session.context(&q2).unwrap(), self.door.kind)
            }
            "continuation" => self.door_with(self.door.context(), DoorKind::Continuation),
            "groups" | "opened_by" | "view_id" | "door_cell" | "seq" | "kind" | "time" | "life" => {
                self.runs().tamper_for_test(self.run, case);
                return self.bind();
            }
            "unknown_run" => {
                return self
                    .door
                    .bind_inference_private(RunId::fresh(), HASH, LABEL, self.identity)
                    .map_err(|e| e.class())
            }
            "consent" => return self.bind_as(&self.door, HASH, LABEL2),
            "identity" => {
                return self
                    .door
                    .bind_inference_private(self.run, HASH, LABEL, [7; 32])
                    .map_err(|e| e.class())
            }
            "expired_q" => {
                self.session.expire_request_for_test(self.q.sequence());
                return self.bind();
            }
            _ => return self.bind(),
        };
        self.bind_as(&door, HASH, LABEL)
    }
}

#[test]
fn sb3_invocation_owner_is_deny_only() {
    let owner = InvocationOwner::new();
    let (a, b) = (owner.observer(), owner.observer().clone());
    assert!(!owner.is_revoked() && !a.is_revoked() && !b.is_revoked());
    owner.revoke();
    owner.revoke();
    assert!(
        owner.is_revoked() && a.is_revoked() && b.is_revoked(),
        "revocation is final, idempotent and seen by every observer"
    );
    let fresh = InvocationOwner::new();
    assert!(
        !fresh.observer().is_revoked(),
        "a fresh owner is never an old one"
    );
    let orphan = InvocationOwner::new().observer();
    assert!(orphan.is_revoked(), "owner Drop revokes every observer");
    // Closed surface: no reset, setter, accessor, Default, From, Clone or serde.
    let source = include_str!("../inference.rs");
    let surface = source
        .split("/// SB-3 (O1): one lane invocation")
        .nth(1)
        .unwrap()
        .split("/// A call the runs lock")
        .next()
        .unwrap();
    let fns: Vec<&str> = surface
        .lines()
        .filter_map(|l| l.trim().strip_prefix("pub fn "))
        .map(|l| l.split('(').next().unwrap())
        .collect();
    assert_eq!(
        fns,
        ["new", "revoke", "observer", "is_revoked", "is_revoked"]
    );
    assert_eq!(surface.matches("#[derive").count(), 1);
    assert!(surface.contains("#[derive(Clone)]\npub struct InvocationObserver"));
    assert!(surface.contains("impl Drop for InvocationOwner"));
    for banned in [
        "impl Default",
        "impl From",
        "Serialize",
        "store(false",
        "fn reset",
    ] {
        assert!(!surface.contains(banned), "closed surface: {banned}");
    }
}

#[test]
fn sb3_inference_binding_is_exact_and_idempotent() {
    let f = Bind::new();
    assert_eq!(f.bind(), Ok(()), "valid original binding");
    let first = f.bound().expect("one binding");
    assert_eq!(
        (
            first.contract_hash.as_str(),
            first.request.as_str(),
            first.identity
        ),
        (HASH, LABEL, f.identity)
    );
    assert_eq!(f.bind(), Ok(()), "identical live binding is idempotent");
    assert_eq!(f.bound(), Some(first.clone()));
    f.unchanged("idempotent");
    for (hash, label) in [(HASH2, LABEL), (HASH, LABEL2)] {
        f.door.confirm_mission(&mission(hash, label)).unwrap();
        assert_eq!(
            f.bind_as(&f.door, hash, label),
            Err("already_admitted"),
            "changed sticky {hash} {label}"
        );
        assert_eq!(f.bound(), Some(first.clone()), "sticky binding unchanged");
    }
    for (case, class) in [
        ("s", "not_member"),
        ("q", "not_member"),
        ("groups", "not_member"),
        ("opened_by", "not_member"),
        ("door_cell", "not_member"),
        ("view_id", "not_member"),
        ("unknown_run", "unknown_run"),
        ("files", "not_admitted"),
        ("consent", "no_consent"),
        ("seq", "no_consent"),
        ("kind", "no_consent"),
        ("time", "no_consent"),
        ("life", "no_consent"),
        ("identity", "model_identity_mismatch"),
        ("expired_q", "halted"),
        ("continuation", "halted"),
        ("unregistered", "model_not_registered"),
    ] {
        let f = Bind::with(case != "unregistered", case != "files");
        assert_eq!(f.vary(case), Err(class), "R2 isolated {case}");
        assert_eq!(f.bound(), None, "R2 {case} binds nothing");
        f.unchanged(case);
    }
}

/// SB3A-R2: an idempotent success still requires the File association.
#[test]
fn sb3_idempotent_rebind_revalidates_file_consent() {
    let f = Bind::new();
    assert_eq!(f.bind(), Ok(()));
    let first = f.bound();
    f.runs().tamper_for_test(f.run, "seq");
    assert_eq!(
        f.bind(),
        Err("no_consent"),
        "idempotent rebind revalidates File consent"
    );
    assert_eq!(f.bound(), first, "rebind refusal changes nothing");
    f.unchanged("rebind");
}

#[test]
fn sb3_original_file_grant_remains_required() {
    for case in ["revoked", "expired", "scope", "substitute"] {
        let f = Bind::new();
        let control = f.control();
        let grants = control.authority().grants();
        match case {
            "revoked" => grants.revoke(f.grant()).unwrap(),
            "expired" => f.clock.advance(Duration::from_secs(3600)),
            "scope" => f.runs().tamper_for_test(f.run, "workspace"),
            _ => {
                let other = f.session.grant_for_test(&f.q).unwrap();
                grants.revoke(f.grant()).unwrap();
                assert!(grants.live(other).is_some(), "another live Files grant");
            }
        }
        assert_eq!(f.bind(), Err("grant_not_live"), "G0 {case}");
        assert_eq!(f.bound(), None, "G0 {case} binds nothing");
        f.unchanged(case);
    }
    let f = Bind::new();
    assert_eq!(f.bind(), Ok(()), "the original live grant binds");
}

#[test]
fn sb3_stop_before_binding_installs_nothing() {
    for installed in [false, true] {
        let f = Bind::new();
        if installed {
            assert_eq!(f.bind(), Ok(()));
        }
        let before = f.bound();
        let (ingress, reached) = (f.session.ingress(), Arc::new(AtomicBool::new(false)));
        let seen = reached.clone();
        f.session.core.cell.1.arm("model.bind.checked", move || {
            seen.store(true, Ordering::SeqCst);
            ingress.emergency().unwrap();
        });
        assert_eq!(f.bind(), Err("halted"), "ingress wins the original CAS");
        assert!(reached.load(Ordering::SeqCst), "paused at the actual CAS");
        assert_eq!(f.bound(), before, "installed={installed}: no mutation");
        f.unchanged("R3");
    }
}

#[test]
fn sb3_binding_contention_refuses_without_latching() {
    let failed = |f: &Bind| f.session.core.cell.session_word() & FAILED != 0;
    let f = Bind::new();
    let runs = f.runs();
    let held = runs.model_observer();
    assert_eq!(f.bind(), Err("unavailable"), "run-table contention");
    assert!(!failed(&f), "contention latches nothing (run)");
    drop(held);
    let control = f.control();
    let held = control.authority().grants().hold_for_test();
    assert_eq!(f.bind(), Err("unavailable"), "grant contention");
    drop(held);
    assert!(
        !failed(&f) && f.bound().is_none(),
        "contention latches nothing"
    );
    assert_eq!(f.bind(), Ok(()), "released holds admit");
    for poisoned in ["runs", "grants"] {
        let f = Bind::new();
        let control = f.control();
        let _ = std::thread::scope(|s| {
            s.spawn(|| {
                if poisoned == "runs" {
                    control.authority().runs().panic_holding_the_runs_lock();
                }
                let _held = control.authority().grants().hold_for_test();
                panic!("poison the grant store (test)");
            })
            .join()
        });
        assert_eq!(f.bind(), Err("unavailable"), "{poisoned} poison refuses");
        assert!(failed(&f), "{poisoned} poison latches session failure");
    }
}

// SB-3B frozen oracles: the private R3-R7 call, its actual per-request
// gates (B1 of SB3B-R1). No public route, SessionPort or host wiring.
use crate::authority::clock::Clock;
use crate::authority::evidence::EndStatus;
use crate::inference::{
    CallDeadline, CallMeter, Charged, InvocationObserver, MeterRefusal, ModelCallOutcome,
    ModelRequest, ResponseSchema,
};
use crate::test_hooks::Gate;
use std::sync::atomic::{AtomicU32, AtomicUsize};
use std::sync::Mutex;

const CALL: [&str; 4] = ["/api/version", "/api/status", "/api/tags", "/api/chat"];
const REPLY: &str = r#"{"sb3b":"reply"}"#;
/// The four R6 restriction sinks.
const RESTRICT: [&str; 4] = [
    "emergency_restrict",
    "cancel_run",
    "door_halt",
    "close_restrict",
];

#[derive(Default)]
pub(super) struct Meter {
    charges: AtomicU32,
    settles: AtomicU32,
    refuse: AtomicBool,
    at_charge: Mutex<Option<Box<dyn Fn() + Send>>>,
}
impl CallMeter for Meter {
    fn charge(&self) -> Result<Charged, MeterRefusal> {
        if self.refuse.load(Ordering::SeqCst) {
            return Err(MeterRefusal::NotRecorded);
        }
        let call = self.charges.fetch_add(1, Ordering::SeqCst) + 1;
        if let Some(hook) = &*self.at_charge.lock().unwrap() {
            hook();
        }
        Ok(Charged { call })
    }
    fn settle(&self, _: u32) {
        self.settles.fetch_add(1, Ordering::SeqCst);
    }
}
struct Open;
impl CallDeadline for Open {
    fn expired(&self) -> bool {
        false
    }
}
fn request() -> ModelRequest {
    let schema = ResponseSchema::new(r#"{"type":"object"}"#).unwrap();
    ModelRequest::new("SB3B system".into(), "SB3B prompt".into(), schema).unwrap()
}
fn invoke(
    door: &SessionDoor,
    run: RunId,
    request: &ModelRequest,
    meter: &Meter,
    invocation: &InvocationObserver,
) -> Result<ModelCallOutcome, &'static str> {
    door.model_call_private(run, request, meter, &Open, invocation)
        .map_err(|e| e.class())
}
fn class(outcome: &ModelCallOutcome) -> &'static str {
    outcome
        .result()
        .as_ref()
        .map_or_else(|e| e.class(), |_| "ok")
}
fn recorded(outcome: &ModelCallOutcome) -> bool {
    matches!(outcome.record(), EndStatus::Recorded { .. })
}

/// A bound original door (SB-3A's R2), its live invocation and a C1 meter.
struct Call {
    b: Bind,
    owner: Arc<InvocationOwner>,
    meter: Meter,
}
impl Call {
    fn new() -> Self {
        Self::serve(Peer::chat(None, None))
    }
    fn serve(peer: Peer) -> Self {
        Self::bound(Bind::make(Some(peer), None, true))
    }
    fn bound(b: Bind) -> Self {
        assert_eq!(b.bind(), Ok(()), "R2 binding");
        Self {
            b,
            owner: Arc::new(InvocationOwner::new()),
            meter: Meter::default(),
        }
    }
    fn call(&self) -> Result<ModelCallOutcome, &'static str> {
        let observer = self.owner.observer();
        invoke(&self.b.door, self.b.run, &request(), &self.meter, &observer)
    }
    /// The call's own peer requests, after registration's four.
    fn sent(&self) -> Vec<String> {
        self.b.peer().paths.lock().unwrap()[4..].to_vec()
    }
    fn entered(&self, n: usize) {
        for _ in 0..4 + n {
            let (_, closed) = self
                .b
                .peer()
                .events
                .recv_timeout(Duration::from_secs(2))
                .unwrap();
            assert!(!closed, "entered, not closed");
        }
    }
    fn records(&self) -> usize {
        let rows = self.b.primary.store().rows();
        rows.iter().filter(|r| r.phase == "model_call").count()
    }
    fn counts(&self) -> (u32, usize) {
        self.b.runs().model_accounting(self.b.run).unwrap()
    }
    fn charged(&self) -> (u32, u32) {
        let m = &self.meter;
        (
            m.charges.load(Ordering::SeqCst),
            m.settles.load(Ordering::SeqCst),
        )
    }
    fn latched(&self) -> (bool, bool) {
        let core = &self.b.session.core;
        let failed = core.cell.session_word() & FAILED != 0;
        (core.model.uncertain.load(Ordering::SeqCst), failed)
    }
    fn hook(&self, point: &'static str, n: usize, action: impl Fn() + Send + Sync + 'static) {
        super::model_tests::nth(&self.b.session, point, n, action);
    }
    fn hold(&self, point: &'static str) -> Gate {
        let gate = Gate::new();
        let held = gate.clone();
        self.b.session.core.cell.1.arm(point, move || held.hold());
        gate
    }
    fn restriction(&self, kind: &'static str) -> impl Fn() + Send + Sync + 'static {
        let (ingress, door, run) = (
            self.b.session.ingress(),
            self.b.door.door.clone(),
            self.b.run,
        );
        move || match kind {
            "emergency_restrict" => assert!(ingress.emergency().is_ok()),
            "cancel_run" => assert!(door.cancel_run(run).is_ok()),
            "door_halt" => drop(door.halt(crate::authority::halt::HaltReason::JournalFailed)),
            _ => assert!(ingress.accept(CommandKind::Quit, 9).is_ok()),
        }
    }
    fn obligations(&self) -> close::ModelObligations {
        self.b.session.close_obligations().unwrap().model
    }
}

/// At call gate `i`, another thread takes the run table until the attempt
/// returns (benign contention); deny-only polling inputs stay live.
fn contend(
    f: &Call,
    i: usize,
) -> (
    Arc<AtomicUsize>,
    Arc<Mutex<Option<std::thread::JoinHandle<()>>>>,
) {
    let (gates, holder) = (Arc::new(AtomicUsize::new(0)), Arc::new(Mutex::new(None)));
    let (runs, core, scope) = (
        f.b.runs(),
        Arc::downgrade(&f.b.session.core),
        f.b.door.context.scope.clone(),
    );
    let (release, rx) = std::sync::mpsc::channel::<()>();
    let rx = Mutex::new(Some(rx));
    let (count, threads) = (gates.clone(), holder.clone());
    f.b.session.core.cell.1.every("model.call.gate", move || {
        if count.fetch_add(1, Ordering::SeqCst) != i {
            return;
        }
        let (runs, rx) = (runs.clone(), rx.lock().unwrap().take().unwrap());
        let (tx, ready) = std::sync::mpsc::channel();
        *threads.lock().unwrap() = Some(std::thread::spawn(move || {
            let _hold = runs.model_observer();
            tx.send(()).unwrap();
            let _ = rx.recv_timeout(Duration::from_secs(3));
        }));
        ready.recv_timeout(Duration::from_secs(1)).unwrap();
        let core = core.upgrade().unwrap();
        assert!(
            scope.live(core.cell.session_word()) && !core.model.uncertain.load(Ordering::SeqCst)
        );
    });
    f.b.session
        .core
        .cell
        .1
        .arm("model.attempt.return", move || {
            let _ = release.send(());
        });
    (gates, holder)
}

/// Request `i` entered and held open at the peer; `restrict` runs then.
fn held(i: usize, restrict: &'static str) -> (Call, ModelCallOutcome, Duration) {
    let f = Call::serve(Peer::chat(Some(4 + i), None));
    let (outcome, elapsed) = std::thread::scope(|s| {
        let (door, run, meter, observer) = (&f.b.door, f.b.run, &f.meter, f.owner.observer());
        let call = s.spawn(move || invoke(door, run, &request(), meter, &observer));
        f.entered(i + 1);
        assert!(f.obligations().active, "held: slot active");
        let start = Instant::now();
        f.restriction(restrict)();
        let closed = f.b.peer().events.recv_timeout(Duration::from_secs(1));
        assert_eq!(
            closed,
            Ok((CALL[i].into(), true)),
            "peer saw closure {i} {restrict}"
        );
        let outcome = call.join().unwrap().expect("ticketed attempt");
        (outcome, start.elapsed())
    });
    (f, outcome, elapsed)
}

/// G4: the no-interference call succeeds; an entered request has no successor.
#[test]
fn sb3_entered_request_grants_no_successor() {
    let f = Call::new();
    let outcome = f.call().expect("admitted private call");
    let reply = outcome.result().as_ref().expect("transport success");
    assert_eq!(reply.content(), REPLY, "exact fake reply");
    assert!(recorded(&outcome), "one recorded result");
    assert_eq!(f.sent(), CALL, "call paths");
    assert_eq!((f.counts(), f.charged(), f.records()), ((1, 0), (1, 1), 1));
    for i in 0..4 {
        let (f, outcome, _) = held(i, "emergency_restrict");
        assert_eq!(f.sent(), CALL[..=i], "entered {i} once; successors 0");
        assert!(outcome.result().is_err() && recorded(&outcome), "{i}");
        assert_eq!(
            f.charged().0,
            u32::from(i == 3),
            "{i}: charge only before chat"
        );
    }
}

/// G5: each held request ends on every restriction sink and is joined.
#[test]
fn sb3_held_request_cancels_and_joins() {
    let outcome = Call::new().call().expect("admitted private call");
    assert!(outcome.result().is_ok(), "no-interference call");
    for i in 0..4 {
        for kind in RESTRICT {
            let (f, outcome, elapsed) = held(i, kind);
            assert!(
                elapsed < Duration::from_secs(1),
                "{i} {kind}: joined in {elapsed:?}"
            );
            assert_eq!(f.sent(), CALL[..=i], "{i} {kind}: no retry or successor");
            assert!(
                outcome.result().is_err() && recorded(&outcome),
                "{i} {kind}"
            );
            assert_eq!(f.counts(), (1, 0), "{i} {kind}: settled");
        }
    }
}

/// G1: the actual override at each call request, under its owning hold.
#[test]
fn sb3_call_override_required_at_each_request() {
    let f = Call::new();
    let gates = Arc::new(AtomicUsize::new(0));
    let seen = gates.clone();
    f.b.session.core.cell.1.every("model.call.gate", move || {
        seen.fetch_add(1, Ordering::SeqCst);
    });
    assert!(
        f.call().is_ok_and(|o| o.result().is_ok()),
        "positive control"
    );
    assert_eq!(
        gates.load(Ordering::SeqCst),
        4,
        "real override at all four requests"
    );
    for i in 0..4 {
        let f = Call::new();
        let (gates, holder) = contend(&f, i);
        let outcome = f.call().expect("ticketed attempt");
        let holder = holder.lock().unwrap().take();
        holder.expect("boundary reached").join().unwrap();
        assert_eq!(
            class(&outcome),
            "model_cancelled",
            "contended gate {i} refuses"
        );
        assert_eq!(
            f.sent(),
            CALL[..i],
            "peer prefix {i}: affected and successors 0"
        );
        assert_eq!(gates.load(Ordering::SeqCst), i + 1, "no successor gate {i}");
        assert_eq!(
            f.latched(),
            (false, false),
            "benign contention latches nothing {i}"
        );
        assert!(recorded(&outcome) && f.counts() == (1, 0), "{i}");
    }
    let f = Call::new();
    let runs = f.b.runs();
    f.b.session.core.cell.1.arm("model.call.gate", move || {
        let _ = std::thread::spawn(move || runs.panic_holding_the_runs_lock()).join();
    });
    let outcome = f.call().expect("ticketed attempt");
    assert_eq!(
        (class(&outcome), f.sent().len()),
        ("model_cancelled", 0),
        "poison refuses"
    );
    assert_eq!(
        f.latched(),
        (true, true),
        "poison latches uncertainty and failure"
    );
}

/// G2: a restriction after construction, before the actual gate.
#[test]
fn sb3_pre_gate_ingress_sends_no_request() {
    assert!(
        Call::new().call().is_ok_and(|o| o.result().is_ok()),
        "positive control"
    );
    for i in 0..4 {
        for kind in RESTRICT {
            let f = Call::new();
            f.hook("model.call.gate", i, f.restriction(kind));
            let outcome = f.call().expect("ticketed attempt");
            assert_eq!(
                f.sent(),
                CALL[..i],
                "{kind} before gate {i}: affected and successors 0"
            );
            assert!(
                outcome.result().is_err() && recorded(&outcome),
                "{i} {kind}"
            );
            assert_eq!(
                f.charged().0,
                u32::from(i == 3),
                "{i} {kind}: prior charge kept, chat 0"
            );
        }
    }
}

/// G3: predicates pass; ingress wins the actual original-snapshot CAS.
#[test]
fn sb3_paused_before_cas_ingress_wins() {
    assert!(
        Call::new().call().is_ok_and(|o| o.result().is_ok()),
        "positive control"
    );
    for i in 0..4 {
        let f = Call::new();
        let ingress = f.b.session.ingress();
        f.hook("model.call.observed", i, move || {
            assert!(ingress.emergency().is_ok())
        });
        let outcome = f.call().expect("ticketed attempt");
        assert_eq!(
            f.sent(),
            CALL[..i],
            "ingress wins the CAS at {i}; no refreshed retry"
        );
        assert!(outcome.result().is_err() && recorded(&outcome), "{i}");
    }
}

/// L2: revocation after the observer read, before the CAS: entered only.
#[test]
fn sb3_revocation_between_observer_read_and_cas_is_entered_only() {
    assert!(
        Call::new().call().is_ok_and(|o| o.result().is_ok()),
        "positive control"
    );
    for i in 0..4 {
        let f = Call::new();
        let owner = f.owner.clone();
        f.hook("model.call.observed", i, move || owner.revoke());
        let outcome = f.call().expect("ticketed attempt");
        assert_eq!(
            f.sent(),
            CALL[..=i],
            "at most selected request {i}; successors 0"
        );
        assert_eq!(
            outcome.result().is_ok(),
            i == 3,
            "{i}: only the entered chat"
        );
    }
    // R3: revoked after the route's check, before the attempt CAS: no ticket.
    let f = Call::new();
    let owner = f.owner.clone();
    f.hook("model.attempt.reserved", 0, move || owner.revoke());
    assert_eq!(f.call().map(|_| ()), Err("halted"), "R3 observer last");
    assert_eq!(
        (f.counts(), f.sent().len()),
        ((0, 0), 0),
        "R3: no ticket or request"
    );
}

/// C2: charged, then the actual invocation is revoked: no chat.
#[test]
fn sb3_invalidation_after_charge_prevents_chat() {
    assert!(
        Call::new().call().is_ok_and(|o| o.result().is_ok()),
        "positive control"
    );
    let f = Call::new();
    let owner = f.owner.clone();
    *f.meter.at_charge.lock().unwrap() = Some(Box::new(move || owner.revoke()));
    let outcome = f.call().expect("ticketed attempt");
    assert_eq!(f.sent(), CALL[..3], "charged; chat 0");
    assert_eq!(f.charged(), (1, 1), "one charge, settled, never refunded");
    assert!(
        outcome.result().is_err() && recorded(&outcome),
        "one recorded obligation"
    );
    assert_eq!(f.counts(), (1, 0));
    // Failed metadata consumes one attempt and no charge; a refused charge sends no chat.
    for (f, bad) in [
        (Call::serve(Peer::chat(None, Some(6))), true),
        (Call::new(), false),
    ] {
        f.meter.refuse.store(!bad, Ordering::SeqCst);
        let outcome = f.call().expect("ticketed attempt");
        assert_eq!((f.counts().0, f.charged().0), (1, 0), "bad={bad}: 1/0/0");
        assert_eq!(f.sent(), CALL[..3], "bad={bad}: no chat");
        assert!(outcome.result().is_err() && recorded(&outcome));
    }
}

/// A1: eight attempts on one run, failed ones included; the ninth refuses.
#[test]
fn sb3_same_run_attempt_ceiling_is_eight() {
    let f = Call::new();
    for n in 1..=8u32 {
        f.meter.refuse.store(n == 2, Ordering::SeqCst);
        let outcome = f.call().expect("attempt within the ceiling");
        assert_eq!(outcome.result().is_ok(), n != 2, "attempt {n}");
        assert_eq!(f.counts(), (n, 0), "attempt {n} counted, never refunded");
    }
    let before = (f.sent().len(), f.charged(), f.records());
    assert_eq!(
        f.call().map(|_| ()),
        Err("model_over_budget"),
        "ninth attempt"
    );
    assert_eq!(
        (f.sent().len(), f.charged(), f.records()),
        before,
        "ninth: 0/0/0"
    );
    assert_eq!(f.counts(), (8, 0), "no refund");
}

/// A2: one operation; a second acquisition refuses before its ticket.
#[test]
fn sb3_concurrent_operation_refuses_before_ticket() {
    assert!(
        Call::new().call().is_ok_and(|o| o.result().is_ok()),
        "positive control"
    );
    // Registration precedes READY: no original door exists to call through.
    let r = super::model_tests::fixture();
    let p = Peer::new(None, None);
    let gate = Gate::new();
    let paused = gate.clone();
    r.session
        .core
        .cell
        .1
        .arm("model.reserved", move || paused.hold());
    std::thread::scope(|s| {
        let registration = s.spawn(|| r.session.register_model_private(&p.endpoint, &pin()));
        gate.reached();
        let q = r.session.ingress().accept(CommandKind::Run, 1);
        assert!(
            q.is_err() || r.session.open_file_door(&q.unwrap()).is_err(),
            "no door"
        );
        gate.open();
        assert!(registration.join().unwrap().is_ok());
    });
    for what in ["metadata", "chat", "recording", "owner_completion"] {
        let f = match what {
            "metadata" => Call::serve(Peer::chat(Some(4), None)),
            "chat" => Call::serve(Peer::chat(Some(7), None)),
            _ => Call::new(),
        };
        let gate = match what {
            "recording" => Some(f.hold("model.attempt.record.before")),
            "owner_completion" => Some(f.hold("model.settle.before")),
            _ => None,
        };
        std::thread::scope(|s| {
            let (door, run, meter, observer) = (&f.b.door, f.b.run, &f.meter, f.owner.observer());
            let call = s.spawn(move || invoke(door, run, &request(), meter, &observer));
            match &gate {
                Some(gate) => gate.reached(),
                None => f.entered(if what == "metadata" { 1 } else { 4 }),
            }
            let before = (f.counts(), f.records(), f.charged().0);
            assert!(f.call().is_err(), "{what}: second acquisition refused");
            assert_eq!(
                (f.counts(), f.records(), f.charged().0),
                before,
                "{what}: 0/0/0"
            );
            match &gate {
                Some(gate) => gate.open(),
                None => f.restriction("emergency_restrict")(),
            }
            assert!(call.join().unwrap().is_ok_and(|o| recorded(&o)), "{what}");
        });
    }
}

/// A3: cancel, finish and capacity pressure keep one exact record and pin.
#[test]
fn sb3_attempt_record_retains_original_correlation() {
    assert!(
        Call::new().call().is_ok_and(|o| o.result().is_ok()),
        "positive control"
    );
    let f = Call::serve(Peer::chat(Some(7), None));
    let pins = Arc::new(Mutex::new(None));
    let (runs, run, seen) = (f.b.runs(), f.b.run, pins.clone());
    f.b.session.core.cell.1.arm("model.settle.before", move || {
        *seen.lock().unwrap() = runs.model_accounting(run);
    });
    std::thread::scope(|s| {
        let (door, meter, observer) = (&f.b.door, &f.meter, f.owner.observer());
        let call = s.spawn(move || invoke(door, run, &request(), meter, &observer));
        f.entered(4);
        let runs = f.b.runs();
        drop(f.b.door.door.cancel_run(run).unwrap());
        runs.finish(run);
        runs.set_capacity_for_test(1);
        let other = crate::authority::ids::AgentId::new("other").unwrap();
        let origin = (
            crate::authority::run::RunOrigin::AgentGoal,
            crate::authority::run::RunClass::Agent,
        );
        assert!(
            runs.open(other, origin.0, origin.1, 2).is_err(),
            "capacity pressure"
        );
        assert!(call.join().unwrap().is_ok_and(|o| recorded(&o)), "recorded");
    });
    assert_eq!(
        *pins.lock().unwrap(),
        Some((1, 1)),
        "exact pin held until settlement"
    );
    assert_eq!(f.counts(), (1, 0), "only the model pin released");
    let rows = f.b.primary.store().rows();
    let json = |phase: &str| -> Vec<serde_json::Value> {
        let rows = rows.iter().filter(|r| r.phase == phase);
        rows.map(|r| serde_json::from_str(&r.body).unwrap())
            .collect()
    };
    let (calls, doors) = (json("model_call"), json("agent_door_opened"));
    assert_eq!(
        (calls.len(), doors.len()),
        (1, 1),
        "exactly one attempt record"
    );
    let (call, door) = (&calls[0], &doors[0]["detail"]["door"]);
    assert_eq!(
        (call["run"].as_str(), call["agent"].as_str()),
        (Some(&*run.to_string()), Some("mother"))
    );
    for (key, value) in [
        ("door", door.as_str().unwrap().to_owned()),
        ("contract_hash", HASH.into()),
        ("request", LABEL.into()),
        ("identity", hex::encode(f.b.identity)),
        ("call", "1".into()),
        ("outcome", "cancelled".into()),
    ] {
        assert_eq!(
            call["detail"][key].as_str(),
            Some(&*value),
            "retained correlation {key}"
        );
    }
}

/// G0 (R3 and every call gate): only the original live grant admits.
#[test]
fn sb3_original_file_grant_remains_required_per_request() {
    assert!(
        Call::new().call().is_ok_and(|o| o.result().is_ok()),
        "original grant reaches chat"
    );
    for case in ["revoked", "expired", "scope", "substitute"] {
        for gate in [None, Some(0), Some(1), Some(2), Some(3)] {
            let f = Call::new();
            let (control, grant, clock, runs, run) = (
                f.b.control(),
                f.b.grant(),
                f.b.clock.clone(),
                f.b.runs(),
                f.b.run,
            );
            if case == "substitute" {
                let other = f.b.session.grant_for_test(&f.b.q).unwrap();
                assert!(
                    control.authority().grants().live(other).is_some(),
                    "another live grant"
                );
            }
            let vary = move || match case {
                "expired" => clock.advance(Duration::from_secs(3600)),
                "scope" => runs.tamper_for_test(run, "workspace"),
                _ => control.authority().grants().revoke(grant).unwrap(),
            };
            let Some(i) = gate else {
                vary();
                assert_eq!(f.call().map(|_| ()), Err("grant_not_live"), "R3 {case}");
                assert_eq!(
                    (f.counts(), f.sent().len()),
                    ((0, 0), 0),
                    "R3 {case}: no ticket"
                );
                continue;
            };
            f.hook("model.call.gate", i, vary);
            let outcome = f.call().expect("ticketed attempt");
            assert_eq!(
                f.sent(),
                CALL[..i],
                "gate {i} {case}: boundary and successors 0"
            );
            assert!(
                outcome.result().is_err() && recorded(&outcome),
                "{i} {case}"
            );
        }
    }
}

/// R4 (private call): a continuation and a conversation never call.
#[test]
fn sb3_private_call_refuses_continuation_and_conversation() {
    assert!(
        Call::new().call().is_ok_and(|o| o.result().is_ok()),
        "the original door calls"
    );
    let f = Call::new();
    let continuation = f.b.door_with(f.b.door.context(), DoorKind::Continuation);
    let observer = f.owner.observer();
    let refused = invoke(&continuation, f.b.run, &request(), &f.meter, &observer);
    assert_eq!(
        refused.map(|_| ()),
        Err("halted"),
        "a continuation never calls"
    );
    let schema = ResponseSchema::new(r#"{"type":"object"}"#).unwrap();
    let opening = nexus_kernel::inference::Conversation::new("s".into(), "u".into()).unwrap();
    let conversation = ModelRequest::conversation(&opening, schema).unwrap();
    let refused = invoke(&f.b.door, f.b.run, &conversation, &f.meter, &observer);
    assert_eq!(
        refused.map(|_| ()),
        Err("model_config_mismatch"),
        "conversation"
    );
    assert_eq!(
        (f.counts(), f.sent().len(), f.charged().0),
        ((0, 0), 0, 0),
        "before ticket"
    );
}

/// T1: the original Q, mission clocks, backstop and watch are never renewed.
#[test]
fn sb3_original_deadlines_and_watch_are_not_renewed() {
    assert!(
        Call::new().call().is_ok_and(|o| o.result().is_ok()),
        "positive control"
    );
    for (case, expected) in [
        ("q", "model_cancelled"),
        ("mission", "model_timeout"),
        ("backstop", "model_timeout"),
        ("watch", "model_stopped"),
    ] {
        let f = Call::new();
        let clock = f.b.clock.clone();
        let (mono, wall) = (clock.monotonic_ms(), clock.wall_ms());
        let limits = f.b.door.context();
        limits
            .narrow_mission_deadlines(clock.clone(), mono + 1000, wall + 1000)
            .unwrap();
        let (session, q, control) = (f.b.session.clone(), f.b.q.sequence(), f.b.control());
        f.hook("model.call.gate", 1, move || match case {
            "q" => session.expire_request_for_test(q),
            "mission" => clock.advance(Duration::from_secs(2)),
            "backstop" => session
                .core
                .model
                .backstop_spent
                .store(true, Ordering::SeqCst),
            _ => assert!(control.kernel_stop_authority().engage() > 0),
        });
        let outcome = f.call().expect("ticketed attempt");
        assert_eq!(
            f.sent(),
            CALL[..1],
            "{case} before gate 1: later gates refuse"
        );
        assert_eq!(class(&outcome), expected, "{case}");
        if case == "backstop" {
            continue; // Each attempt captures its own backstop.
        }
        if case == "watch" {
            f.b.control().resume_with_halt(&Owner, None).unwrap();
            assert!(
                !f.b.control().stop_snapshot().kernel_stopped,
                "native resume"
            );
        }
        assert!(f.call().is_err(), "{case}: call two is not renewed");
        assert_eq!(
            (f.sent().len(), f.counts().0),
            (1, 1),
            "{case}: call two sends nothing"
        );
    }
}

/// SB3B-R1 (2): every B1 test seam is unreachable in a non-test build: the
/// probe function, the backstop seam field and the door_with module are
/// each `#[cfg(test)]`, so a production reference cannot compile.
#[test]
fn sb3_test_seams_are_test_only() {
    let slot = include_str!("model.rs");
    assert_eq!(
        slot.matches("backstop_spent").count(),
        2,
        "one field, one read"
    );
    let field = "    #[cfg(test)]\n    pub(super) backstop_spent: AtomicBool,";
    assert!(slot.contains(field), "backstop seam field");
    let probe = include_str!("../authority/file_session.rs");
    assert!(
        probe.contains("#[cfg(test)]\npub(crate) fn probe("),
        "probe"
    );
    let module = include_str!("../host_session.rs");
    assert!(
        module.contains("#[cfg(test)]\nmod model_call_tests;"),
        "door_with"
    );
}
