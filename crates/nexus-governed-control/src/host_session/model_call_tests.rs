//! SB-3A frozen oracles: deny-only invocation observation and the private
//! original-door R2 binding. No ticket, charge, transport or public route.
use super::model_tests::{pin, Peer};
use super::*;
use crate::authority::approval::{
    AcknowledgementConfirmation, ActionConfirmation, AgentDoorConfirmation, ControlConfirmer,
    GrantConfirmation, MissionConfirmation, ResumeConfirmation,
};
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

/// SB-3B1a: a test clock whose monotonic and wall readings move apart.
#[derive(Default)]
struct SplitClock {
    mono: std::sync::atomic::AtomicU64,
    wall: std::sync::atomic::AtomicU64,
    /// SB3B-R5 (3): runs inside every reading (a blocking clock).
    on_read: std::sync::Mutex<Option<Arc<dyn Fn() + Send + Sync>>>,
}
impl SplitClock {
    fn advance(&self, by: Duration) {
        self.mono_only(by);
        self.wall_only(by);
    }
    fn mono_only(&self, by: Duration) {
        self.mono.fetch_add(by.as_millis() as u64, Ordering::SeqCst);
    }
    fn wall_only(&self, by: Duration) {
        self.wall.fetch_add(by.as_millis() as u64, Ordering::SeqCst);
    }
}
impl SplitClock {
    fn read(&self) {
        let hook = self.on_read.lock().unwrap().clone();
        if let Some(hook) = hook {
            hook();
        }
    }
}
impl crate::authority::clock::Clock for SplitClock {
    fn monotonic_ms(&self) -> u64 {
        self.read();
        self.mono.load(Ordering::SeqCst)
    }
    fn wall_ms(&self) -> u64 {
        self.read();
        1_700_000_000_000 + self.wall.load(Ordering::SeqCst)
    }
}

/// Original Bootstrap registration, Q, D with its retained grant, cached
/// native consent and the existing File run R: every R2 prerequisite.
struct Bind {
    session: Arc<HostSession>,
    clock: Arc<SplitClock>,
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
        Self::make(register.then(|| Peer::chat(None, None)), files)
    }
    /// SB-3B1b: registered on `peer` (its chat answered, its requests held).
    fn make(peer: Option<Peer>, files: bool) -> Self {
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
        let clock = Arc::new(SplitClock::default());
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
        let identity = match &peer {
            Some(p) => {
                let view = session.register_model_private(&p.endpoint, &pin()).unwrap();
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
    fn peer(&self) -> &Peer {
        self._keep.0.as_ref().expect("this fixture's own peer")
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

// SB-3B1a frozen oracles (SB3B-R3): R3 attempt admission through the
// private original-door route. No transport, meter, gate or public route.
use crate::inference::{ModelRequest, ResponseSchema};

fn request() -> ModelRequest {
    let schema = ResponseSchema::new(r#"{"type":"object"}"#).unwrap();
    ModelRequest::new("SB3B system".into(), "SB3B prompt".into(), schema).unwrap()
}

/// A bound original door (SB-3A R2) and its live invocation.
struct Admit {
    b: Bind,
    owner: Arc<InvocationOwner>,
    binding: Option<InferenceAdmission>,
}
impl Admit {
    fn new() -> Self {
        Self::of(Bind::new())
    }
    fn of(b: Bind) -> Self {
        Self {
            b,
            owner: Arc::new(InvocationOwner::new()),
            binding: None,
        }
    }
    fn bound() -> Self {
        let mut f = Self::new();
        assert_eq!(f.b.bind(), Ok(()), "R2 binding");
        f.binding = f.b.bound();
        f
    }
    fn on(
        &self,
        door: &SessionDoor,
        request: &ModelRequest,
    ) -> Result<model::Attempt, &'static str> {
        let observer = self.owner.observer();
        let attempt = door.admit_attempt_private(self.b.run, request, &observer);
        attempt.map_err(|e| e.class())
    }
    fn admit(&self) -> Result<model::Attempt, &'static str> {
        self.on(&self.b.door, &request())
    }
    /// One recorded result, then settlement of the exact pin.
    fn finish(&self, mut attempt: model::Attempt, failed: bool) -> Result<(), SessionError> {
        let result = if failed {
            Err(crate::inference::ModelError::Cancelled)
        } else {
            Ok(())
        };
        let control = self.b.control();
        assert!(matches!(
            attempt.record_return(control.authority(), result),
            Ok(EndStatus::Recorded { .. })
        ));
        attempt.complete()
    }
    fn counts(&self) -> (u32, usize) {
        self.b.runs().model_accounting(self.b.run).unwrap()
    }
    fn calls(&self) -> Vec<serde_json::Value> {
        let rows = self.b.primary.store().rows();
        let rows = rows.iter().filter(|r| r.phase == "model_call");
        rows.map(|r| serde_json::from_str(&r.body).unwrap())
            .collect()
    }
    /// Pre-ticket refusal: no attempt, pin, record or active slot (F8), and
    /// the retained binding unchanged (SB3B-R4 5).
    fn nothing(&self, case: &str) {
        let (attempts, pins, binding) = self.b.runs().model_ledger_for_test(self.b.run);
        assert_eq!(
            format!("{binding:?}"),
            format!("{:?}", self.binding),
            "{case}: binding"
        );
        assert_eq!(binding, self.binding, "{case}: binding kept");
        assert_eq!((attempts, pins.len()), (0, 0), "{case}: no attempt or pin");
        assert_eq!(self.calls().len(), 0, "{case}: no record");
        assert!(
            !self.b.session.core.model.snapshot().unwrap().active,
            "{case}: slot"
        );
    }
    fn failed(&self) -> bool {
        self.b.session.core.cell.session_word() & FAILED != 0
    }
}
use crate::authority::evidence::EndStatus;

/// G0 (R3) and F8: only the original live grant admits an attempt.
#[test]
fn sb3_attempt_admission_requires_the_original_grant() {
    let f = Admit::bound();
    let attempt = f.admit().expect("the original live grant admits");
    assert_eq!(f.counts(), (1, 1), "ticket, ordinal and pin together");
    assert!(f.finish(attempt, false).is_ok() && f.counts() == (1, 0));
    for case in ["revoked", "expired", "scope", "substitute"] {
        let f = Admit::bound();
        let (control, grant) = (f.b.control(), f.b.grant());
        match case {
            "revoked" => control.authority().grants().revoke(grant).unwrap(),
            "expired" => f.b.clock.advance(Duration::from_secs(3600)),
            "scope" => f.b.runs().tamper_for_test(f.b.run, "workspace"),
            _ => {
                let other = f.b.session.grant_for_test(&f.b.q).unwrap();
                control.authority().grants().revoke(grant).unwrap();
                assert!(control.authority().grants().live(other).is_some());
            }
        }
        assert_eq!(f.admit().map(|_| ()), Err("grant_not_live"), "R3 {case}");
        f.nothing(case);
    }
    // R3 grant contention refuses without latching; poison latches failure.
    let f = Admit::bound();
    let control = f.b.control();
    let held = control.authority().grants().hold_for_test();
    assert_eq!(
        f.admit().map(|_| ()),
        Err("unavailable"),
        "R3 grant contention"
    );
    drop(held);
    f.nothing("grant contention");
    assert!(!f.failed(), "contention latches nothing");
    let _ = std::thread::scope(|s| {
        s.spawn(|| {
            let _held = control.authority().grants().hold_for_test();
            panic!("poison the grant store (test)");
        })
        .join()
    });
    assert_eq!(f.admit().map(|_| ()), Err("unavailable"), "R3 grant poison");
    assert!(f.failed(), "grant poison latches session failure");
    let f = Admit::bound();
    let runs = f.b.runs();
    let _ = std::thread::spawn(move || runs.panic_holding_the_runs_lock()).join();
    assert_eq!(
        f.admit().map(|_| ()),
        Err("unavailable"),
        "R3 run-table poison"
    );
    assert!(f.failed(), "run-table poison latches session failure");
}

/// R3: the live invocation is required before acquisition and is the last
/// predicate before the attempt CAS; a later revocation is the entered one.
#[test]
fn sb3_attempt_admission_observes_invocation_last() {
    let f = Admit::bound();
    assert!(f.admit().is_ok(), "positive control");
    let f = Admit::bound();
    f.owner.revoke();
    assert_eq!(
        f.admit().map(|_| ()),
        Err("halted"),
        "revoked before acquisition"
    );
    f.nothing("revoked");
    // After the route's check, before the run-table hold: R3 refuses.
    let f = Admit::bound();
    let owner = f.owner.clone();
    f.b.session
        .core
        .cell
        .1
        .arm("model.attempt.reserved", move || owner.revoke());
    assert_eq!(
        f.admit().map(|_| ()),
        Err("halted"),
        "R3 observes the invocation"
    );
    f.nothing("R3 revoked");
    // Both fail: the grant precedes the observer (observer last).
    let f = Admit::bound();
    let (owner, control, grant) = (f.owner.clone(), f.b.control(), f.b.grant());
    f.b.session
        .core
        .cell
        .1
        .arm("model.attempt.reserved", move || {
            owner.revoke();
            control.authority().grants().revoke(grant).unwrap();
        });
    assert_eq!(
        f.admit().map(|_| ()),
        Err("grant_not_live"),
        "observer is last"
    );
    f.nothing("dual");
    // After the observer read: ingress wins the CAS; a revocation does not.
    let f = Admit::bound();
    let ingress = f.b.session.ingress();
    let armed = move || assert!(ingress.emergency().is_ok());
    f.b.session.core.cell.1.arm("model.attempt.checked", armed);
    assert_eq!(f.admit().map(|_| ()), Err("halted"), "ingress wins the CAS");
    f.nothing("CAS");
    let f = Admit::bound();
    let owner = f.owner.clone();
    f.b.session
        .core
        .cell
        .1
        .arm("model.attempt.checked", move || owner.revoke());
    assert!(
        f.admit().is_ok(),
        "the read admitted it: the entered interval"
    );
    assert_eq!(f.counts(), (1, 1));
}

/// R4 (attempt) and F12: each isolated refusal before any ticket.
#[test]
fn sb3_private_attempt_refuses_before_ticket() {
    let f = Admit::bound();
    assert!(f.admit().is_ok(), "the original door admits");
    for (case, class) in [
        ("continuation", "halted"),
        ("conversation", "model_config_mismatch"),
        ("q", "not_member"),
        ("expired_q", "halted"),
        ("unregistered", "model_not_registered"),
    ] {
        let f = match case {
            "unregistered" => Admit::of(Bind::with(false, true)),
            _ => Admit::bound(),
        };
        let refused = match case {
            "continuation" => {
                let door = f.b.door_with(f.b.door.context(), DoorKind::Continuation);
                f.on(&door, &request())
            }
            "conversation" => {
                let schema = ResponseSchema::new(r#"{"type":"object"}"#).unwrap();
                let opening = nexus_kernel::inference::Conversation::new("s".into(), "u".into());
                let request = ModelRequest::conversation(&opening.unwrap(), schema).unwrap();
                f.on(&f.b.door, &request)
            }
            "q" => {
                let q2 = f.b.session.ingress().accept(CommandKind::Run, 2).unwrap();
                let door =
                    f.b.door_with(f.b.session.context(&q2).unwrap(), f.b.door.kind);
                f.on(&door, &request())
            }
            "expired_q" => {
                f.b.session.expire_request_for_test(f.b.q.sequence());
                f.admit()
            }
            _ => f.admit(),
        };
        assert_eq!(refused.map(|_| ()), Err(class), "{case}");
        f.nothing(case);
    }
}

/// A1: attempts 1-8 on one run, failed ones included; the ninth refuses.
#[test]
fn sb3_attempt_ceiling_is_eight_on_the_original_run() {
    let f = Admit::bound();
    for n in 1..=8u32 {
        let attempt = f.admit().expect("attempt within the ceiling");
        assert_eq!(f.counts(), (n, 1), "attempt {n} counted and pinned");
        assert!(f.finish(attempt, n % 3 == 0).is_ok(), "attempt {n} settled");
        assert_eq!(f.counts(), (n, 0), "attempt {n} never refunded");
    }
    let records = f.calls().len();
    assert_eq!(
        f.admit().map(|_| ()),
        Err("model_over_budget"),
        "ninth attempt"
    );
    assert_eq!(
        (f.counts(), f.calls().len()),
        ((8, 0), records),
        "ninth: nothing"
    );
}

/// A3 and F7: one exact record and pin across cancel, finish and capacity
/// pressure; a substituted ordinal (same cardinality) never settles.
#[test]
fn sb3_attempt_record_and_pin_are_exact() {
    let f = Admit::bound();
    let mut attempt = f.admit().expect("admitted attempt");
    assert_eq!(
        f.b.runs().model_pins_for_test(f.b.run),
        [1],
        "the exact ordinal"
    );
    let runs = f.b.runs();
    assert!(f.b.door.door.cancel_run(f.b.run).is_ok());
    runs.finish(f.b.run);
    runs.set_capacity_for_test(1);
    let other = crate::authority::ids::AgentId::new("other").unwrap();
    let (origin, class) = (
        crate::authority::run::RunOrigin::AgentGoal,
        crate::authority::run::RunClass::Agent,
    );
    assert!(
        runs.open(other, origin, class, 2).is_err(),
        "capacity pressure"
    );
    let control = f.b.control();
    let cancelled = Err(crate::inference::ModelError::Cancelled);
    assert!(matches!(
        attempt.record_return(control.authority(), cancelled),
        Ok(EndStatus::Recorded { .. })
    ));
    assert_eq!(
        runs.model_pins_for_test(f.b.run),
        [1],
        "pin held until settlement"
    );
    assert!(attempt.complete().is_ok(), "settled");
    assert_eq!(f.counts(), (1, 0), "only the model pin released");
    let (calls, rows) = (f.calls(), f.b.primary.store().rows());
    let door = rows
        .iter()
        .find(|r| r.phase == "agent_door_opened")
        .unwrap();
    let door: serde_json::Value = serde_json::from_str(&door.body).unwrap();
    assert_eq!(calls.len(), 1, "exactly one record");
    let call = &calls[0];
    let run = f.b.run.to_string();
    assert_eq!(
        (call["run"].as_str(), call["agent"].as_str()),
        (Some(&*run), Some("mother"))
    );
    for (key, value) in [
        ("door", door["detail"]["door"].as_str().unwrap().to_owned()),
        ("contract_hash", HASH.into()),
        ("request", LABEL.into()),
        ("identity", hex::encode(f.b.identity)),
        ("call", "1".into()),
        ("outcome", "cancelled".into()),
    ] {
        assert_eq!(
            call["detail"][key].as_str(),
            Some(&*value),
            "correlation {key}"
        );
    }
    // F7: the owned ordinal replaced by another, same cardinality.
    let f = Admit::bound();
    let attempt = f.admit().expect("admitted attempt");
    f.b.runs().tamper_for_test(f.b.run, "pin");
    assert_eq!(
        f.b.runs().model_pins_for_test(f.b.run),
        [101],
        "substituted"
    );
    assert!(
        f.finish(attempt, false).is_err(),
        "a substituted pin never settles"
    );
    assert_eq!(f.counts(), (1, 1), "debt retained");
    let o = f.b.session.core.model.snapshot().unwrap();
    assert!(o.active && o.uncertain && f.failed(), "slot debt latched");
}

/// SB3B-R4 (2): R3 reads the invocation after the grant, then the CAS: a
/// revocation published between them refuses (no early cached read).
#[test]
fn sb3_attempt_observer_is_read_after_the_grant() {
    let f = Admit::bound();
    assert!(f.admit().is_ok(), "positive control");
    let f = Admit::bound();
    let owner = f.owner.clone();
    f.b.session
        .core
        .cell
        .1
        .arm("model.attempt.granted", move || owner.revoke());
    assert_eq!(f.admit().map(|_| ()), Err("halted"), "read after the grant");
    f.nothing("granted");
}

/// SB3B-R4 (3): R3 contention latches nothing; poison latches failure;
/// neither issues an attempt, pin, record or binding change.
#[test]
fn sb3_attempt_contention_and_poison_latch_exactly() {
    let f = Admit::bound();
    assert!(f.admit().is_ok(), "positive control");
    let latched = |f: &Admit| {
        (
            f.b.session.core.model.uncertain.load(Ordering::SeqCst),
            f.failed(),
        )
    };
    for held in ["grants", "runs"] {
        let f = Admit::bound();
        let (control, runs) = (f.b.control(), f.b.runs());
        let refused = match held {
            "grants" => {
                let _hold = control.authority().grants().hold_for_test();
                f.admit().map(|_| ())
            }
            _ => {
                let _hold = runs.model_observer();
                f.admit().map(|_| ())
            }
        };
        assert_eq!(refused, Err("unavailable"), "R3 {held} contention");
        assert_eq!(
            latched(&f),
            (false, false),
            "{held}: contention latches nothing"
        );
        f.nothing(held);
    }
    for poisoned in ["grants", "runs"] {
        let f = Admit::bound();
        let (control, runs) = (f.b.control(), f.b.runs());
        let _ = std::thread::spawn(move || {
            if poisoned == "runs" {
                runs.panic_holding_the_runs_lock();
            }
            let _hold = control.authority().grants().hold_for_test();
            panic!("poison the grant store (test)");
        })
        .join();
        assert_eq!(
            f.admit().map(|_| ()),
            Err("unavailable"),
            "R3 {poisoned} poison"
        );
        assert!(f.failed(), "{poisoned}: poison latches session failure");
        f.nothing(poisoned);
    }
}

/// SB3B-R4 (4): each grant clock and the workspace identity independently.
#[test]
fn sb3_attempt_grant_clocks_and_identity_are_independent() {
    let f = Admit::bound();
    assert!(f.admit().is_ok(), "positive control");
    for case in ["monotonic", "wall", "identity"] {
        let f = Admit::bound();
        match case {
            "monotonic" => f.b.clock.mono_only(Duration::from_secs(3600)),
            "wall" => f.b.clock.wall_only(Duration::from_secs(3600)),
            _ => f.b.runs().tamper_for_test(f.b.run, "identity"),
        }
        assert_eq!(f.admit().map(|_| ()), Err("grant_not_live"), "R3 {case}");
        f.nothing(case);
    }
}

/// SB3B-R5 (1) as amended by SB3B-R6 (2): a revocation started after the
/// grant observation cannot complete before Q's CAS; it is refused or lands
/// after the CAS (the entered interval). The bounded wait only bounds the
/// test: a slow revoker can only let a mutant survive, never fail correct
/// code.
#[test]
fn sb3_attempt_grant_revocation_cannot_precede_the_cas() {
    let f = Admit::bound();
    assert!(f.admit().is_ok(), "positive control");
    let f = Admit::bound();
    let (control, grant) = (f.b.control(), f.b.grant());
    let revoked_before_cas = Arc::new(AtomicBool::new(false));
    let seen = revoked_before_cas.clone();
    let holder = Arc::new(std::sync::Mutex::new(None));
    let keep = holder.clone();
    f.b.session
        .core
        .cell
        .1
        .arm("model.attempt.granted", move || {
            let (done, finished) = std::sync::mpsc::channel();
            *keep.lock().unwrap() = Some(std::thread::spawn(move || {
                let _ = control.authority().grants().revoke(grant);
                let _ = done.send(());
            }));
            let completed = finished.recv_timeout(Duration::from_secs(2)).is_ok();
            seen.store(completed, Ordering::SeqCst);
        });
    let admitted = f.admit();
    holder
        .lock()
        .unwrap()
        .take()
        .expect("schedule reached")
        .join()
        .unwrap();
    let before = revoked_before_cas.load(Ordering::SeqCst);
    assert!(
        !(before && admitted.is_ok()),
        "a grant revoked before the CAS admitted"
    );
    let grants = f.b.control();
    assert!(
        grants.authority().grants().live(f.b.grant()).is_none(),
        "revocation landed"
    );
    if admitted.is_err() {
        f.nothing("revoked before the CAS");
    }
}

/// SB3B-R5 (2): the retained SB-2b rollback contract. A pre-ticket refusal
/// whose rollback completion hold expires latches uncertainty and failure;
/// no ticket, pin or record exists.
#[test]
fn sb3_attempt_rollback_hold_expiry_latches_uncertainty() {
    let f = Admit::bound();
    f.b.control()
        .authority()
        .grants()
        .revoke(f.b.grant())
        .unwrap();
    let core = Arc::downgrade(&f.b.session.core);
    let (release, held) = std::sync::mpsc::channel::<()>();
    let held = std::sync::Mutex::new(Some(held));
    let holder = Arc::new(std::sync::Mutex::new(None));
    let keep = holder.clone();
    f.b.session
        .core
        .cell
        .1
        .arm("model.attempt.refused", move || {
            let (core, held) = (
                core.upgrade().unwrap(),
                held.lock().unwrap().take().unwrap(),
            );
            let (ready, wait) = std::sync::mpsc::channel();
            *keep.lock().unwrap() = Some(std::thread::spawn(move || {
                let _hold = core.model.state.lock().unwrap();
                ready.send(()).unwrap();
                let _ = held.recv_timeout(Duration::from_secs(5));
            }));
            wait.recv_timeout(Duration::from_secs(1)).unwrap();
        });
    let refused = f.admit().map(|_| ());
    let _ = release.send(());
    holder
        .lock()
        .unwrap()
        .take()
        .expect("refusal reached")
        .join()
        .unwrap();
    assert_eq!(refused, Err("unavailable"), "the rollback hold expired");
    let uncertain = f.b.session.core.model.uncertain.load(Ordering::SeqCst);
    assert!(uncertain && f.failed(), "uncertainty and failure latched");
    let (attempts, pins, binding) = f.b.runs().model_ledger_for_test(f.b.run);
    assert_eq!(
        (attempts, pins.len(), f.calls().len()),
        (0, 0, 0),
        "no ticket"
    );
    assert_eq!(binding, f.binding, "binding kept");
}

/// F11 (SB3B-R1 2, SB3B-R4 1): every B1a seam is reachable only under
/// `#[cfg(test)]` on its own statement or an enclosing item, judged on code
/// with comments (nested included) and literal contents masked.
#[test]
fn sb3_b1a_test_seams_are_test_only() {
    // Regression controls: the two crafted leaks of the B1a re-check.
    let sibling = "fn production() {\n    #[cfg(test)] let marker = { 0 };\n    consume(probe(None, \"unused\"));\n}\n";
    let nested = "/* outer /* inner */ #[cfg(test)] */\npub(crate) fn probe(cell: u8) {}\n";
    let gated = "#[cfg(test)]\nfn probe() {}\nfn f() {\n    #[cfg(test)]\n    probe();\n}\n";
    assert_eq!(
        leaks(sibling, &["probe"]),
        1,
        "a sibling's attribute never governs"
    );
    assert_eq!(leaks(nested, &["probe"]), 1, "a nested comment is masked");
    assert_eq!(leaks(gated, &["probe"]), 0, "gated statement and item");
    // SB3B-R5 (4): escaped and byte character literals never open a string.
    for literal in [r#"'\"'"#, r#"'\''"#, r#"'\\'"#, r#"b'\"'"#, r#"b'\''"#] {
        let escaped = format!(
            "fn production() {{\n    let _c = {literal};\n    probe(None, \"unused\");\n}}\n"
        );
        assert_eq!(leaks(&escaped, &["probe"]), 1, "escaped literal {literal}");
    }
    let sources = [
        ("model.rs", include_str!("model.rs")),
        ("run.rs", include_str!("../authority/run.rs")),
        (
            "file_session.rs",
            include_str!("../authority/file_session.rs"),
        ),
        ("host_session.rs", include_str!("../host_session.rs")),
    ];
    let seams = [
        "probe",
        "tamper_for_test",
        "model_pins_for_test",
        "model_ledger_for_test",
        "model_call_tests",
    ];
    let mut found = 0;
    for (file, source) in sources {
        found += seam_sites(source, &seams).len();
        assert_eq!(
            leaks(source, &seams),
            0,
            "{file}: a seam reachable outside cfg(test)"
        );
    }
    assert!(found >= 8, "the seams were scanned: {found}");
}
fn seam_sites(source: &str, seams: &[&str]) -> Vec<usize> {
    let code = mask(source);
    let word = |i: usize| {
        code.as_bytes()
            .get(i)
            .is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'_')
    };
    let mut sites = Vec::new();
    for seam in seams {
        for (at, _) in code.match_indices(seam) {
            if !word(at + seam.len()) && !at.checked_sub(1).is_some_and(word) {
                sites.push(at);
            }
        }
    }
    sites
}
fn leaks(source: &str, seams: &[&str]) -> usize {
    let code = mask(source);
    seam_sites(source, seams)
        .into_iter()
        .filter(|&at| !test_only(&code, at))
        .count()
}
/// Comments (nested block comments included) and the contents of string
/// and char literals become spaces; line structure is kept.
fn mask(source: &str) -> String {
    let b = source.as_bytes();
    let mut out = b.to_vec();
    let mut i = 0;
    let blank = |out: &mut Vec<u8>, from: usize, to: usize| {
        for c in &mut out[from..to.min(b.len())] {
            if *c != b'\n' {
                *c = b' ';
            }
        }
    };
    while i < b.len() {
        if b[i..].starts_with(b"//") {
            let end = b[i..]
                .iter()
                .position(|&c| c == b'\n')
                .map_or(b.len(), |n| i + n);
            blank(&mut out, i, end);
            i = end;
        } else if b[i..].starts_with(b"/*") {
            let (mut depth, mut j) = (1, i + 2);
            while j < b.len() && depth > 0 {
                if b[j..].starts_with(b"/*") {
                    (depth, j) = (depth + 1, j + 2);
                } else if b[j..].starts_with(b"*/") {
                    (depth, j) = (depth - 1, j + 2);
                } else {
                    j += 1;
                }
            }
            blank(&mut out, i, j);
            i = j;
        } else if b[i] == b'r'
            && (b[i + 1..].starts_with(b"#") || b[i + 1..].starts_with(b"\""))
            && (i == 0 || !b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'b')
        {
            let hashes = b[i + 1..].iter().take_while(|&&c| c == b'#').count();
            if b.get(i + 1 + hashes) != Some(&b'"') {
                i += 1;
                continue;
            }
            let close = format!("\"{}", "#".repeat(hashes));
            let start = i + 2 + hashes;
            let end = source[start..].find(&close).map_or(b.len(), |n| start + n);
            blank(&mut out, start, end);
            i = end + close.len();
        } else if b[i] == b'"' {
            let mut j = i + 1;
            while j < b.len() && b[j] != b'"' {
                j += if b[j] == b'\\' { 2 } else { 1 };
            }
            blank(&mut out, i + 1, j);
            i = j + 1;
        } else if b[i] == b'\'' && b.get(i + 1) == Some(&b'\\') && i + 3 <= b.len() {
            // An escaped char literal ('\'', '\"', '\\', '\u{..}'; b'..' alike).
            let end = b[i + 3..]
                .iter()
                .position(|&c| c == b'\'')
                .map_or(b.len(), |n| i + 3 + n);
            blank(&mut out, i + 1, end);
            i = end + 1;
        } else if b[i] == b'\'' && b.get(i + 2) == Some(&b'\'') {
            blank(&mut out, i + 1, i + 2);
            i += 3;
        } else {
            i += 1;
        }
    }
    String::from_utf8(out).unwrap()
}
/// The statement or item holding `at`, or an item or block enclosing it,
/// carries `#[cfg(test)]`. Statement starts are found at the same nesting;
/// an enclosing parenthesis never borrows a sibling statement's attribute.
fn test_only(code: &str, at: usize) -> bool {
    let b = code.as_bytes();
    let mut end = at;
    loop {
        let (mut depth, mut i) = (0usize, end);
        let start = loop {
            if i == 0 {
                break 0;
            }
            i -= 1;
            match b[i] {
                b')' | b']' | b'}' if depth > 0 || b[i] != b'}' => depth += 1,
                b'(' | b'[' | b'{' if depth > 0 => depth -= 1,
                b'(' | b'[' => {}
                b'}' | b'{' | b';' | b',' => break i + 1,
                _ => {}
            }
        };
        if code[start..].trim_start().starts_with("#[cfg(test)]") {
            return true;
        }
        // Up to the block that encloses this statement.
        let (mut depth, mut i) = (0usize, start);
        loop {
            if i == 0 {
                return false;
            }
            i -= 1;
            match b[i] {
                b'}' => depth += 1,
                b'{' if depth == 0 => break,
                b'{' => depth -= 1,
                _ => {}
            }
        }
        end = i;
    }
}

// SB-3B frozen oracles: the private R3-R7 call, its actual per-request
// gates (B1 of SB3B-R1). No public route, SessionPort or host wiring.
use crate::authority::clock::Clock;
use crate::inference::{
    CallDeadline, CallMeter, Charged, InvocationObserver, MeterRefusal, ModelCallOutcome,
};
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
    /// F10b: runs on entry, before the C1 charge is taken.
    at_entry: Mutex<Option<Box<dyn Fn() + Send>>>,
}
impl CallMeter for Meter {
    fn charge(&self) -> Result<Charged, MeterRefusal> {
        if let Some(hook) = &*self.at_entry.lock().unwrap() {
            hook();
        }
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
/// Every B1b oracle first shows the no-interference call succeeding.
fn positive(why: &str) {
    let ok = Call::new().call().is_ok_and(|o| o.result().is_ok());
    assert!(ok, "{why}");
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
        Self::bound(Bind::make(Some(peer), true))
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
    fn hold(&self, point: &'static str) -> crate::test_hooks::Gate {
        let gate = crate::test_hooks::Gate::new();
        let held = gate.clone();
        self.b.session.core.cell.1.arm(point, move || held.hold());
        gate
    }
    fn obligations(&self) -> close::ModelObligations {
        self.b.session.close_obligations().unwrap().model
    }
}

/// At call gate `i`, another thread takes the run table until the attempt
/// returns (benign contention); deny-only polling inputs stay live.
type Holder = Arc<Mutex<Option<std::thread::JoinHandle<()>>>>;
fn contend(f: &Call, i: usize) -> (Arc<AtomicUsize>, Holder) {
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

/// Request `i` entered and held open at the peer; `act` runs then.
fn held(i: usize, act: impl FnOnce(&Call)) -> (Call, ModelCallOutcome, Duration) {
    let f = Call::serve(Peer::chat(Some(4 + i), None));
    let (outcome, elapsed) = std::thread::scope(|s| {
        let (door, run, meter, observer) = (&f.b.door, f.b.run, &f.meter, f.owner.observer());
        let call = s.spawn(move || invoke(door, run, &request(), meter, &observer));
        f.entered(i + 1);
        assert!(f.obligations().active, "held: slot active");
        let start = Instant::now();
        act(&f);
        let closed = f.b.peer().events.recv_timeout(Duration::from_secs(1));
        assert_eq!(closed, Ok((CALL[i].into(), true)), "peer saw closure {i}");
        let outcome = call.join().unwrap().expect("ticketed attempt");
        (outcome, start.elapsed())
    });
    (f, outcome, elapsed)
}

/// G4: the no-interference call succeeds; an entered request has no successor.
#[test]
fn sb3_entered_request_grants_no_successor() {
    if watched("sb3_entered_request_grants_no_successor") {
        return;
    }
    let f = Call::new();
    let outcome = f.call().expect("admitted private call");
    let reply = outcome.result().as_ref().expect("transport success");
    assert_eq!(reply.content(), REPLY, "exact fake reply");
    assert!(recorded(&outcome), "one recorded result");
    assert_eq!(f.sent(), CALL, "call paths");
    assert_eq!((f.counts(), f.charged(), f.records()), ((1, 0), (1, 1), 1));
    for i in 0..4 {
        let (f, outcome, _) = held(i, |f| f.restriction("emergency_restrict")());
        assert_eq!(f.sent(), CALL[..=i], "entered {i} once; successors 0");
        assert_eq!(attempts(&f), i + 1, "{i}: exact admission attempts");
        assert!(outcome.result().is_err() && recorded(&outcome), "{i}");
        assert_eq!(
            f.charged().0,
            u32::from(i == 3),
            "{i}: charge only before chat"
        );
    }
    println!("{HELD_DONE} sb3_entered_request_grants_no_successor");
}

/// G5: each held request ends on every restriction sink and is joined.
#[test]
fn sb3_held_request_cancels_and_joins() {
    if watched("sb3_held_request_cancels_and_joins") {
        return;
    }
    let outcome = Call::new().call().expect("admitted private call");
    assert!(outcome.result().is_ok(), "no-interference call");
    for i in 0..4 {
        for kind in RESTRICT {
            let (f, outcome, elapsed) = held(i, |f| f.restriction(kind)());
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
    println!("{HELD_DONE} sb3_held_request_cancels_and_joins");
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
}

/// G2: a restriction after construction, before the actual gate.
#[test]
fn sb3_pre_gate_ingress_sends_no_request() {
    positive("positive control");
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
            assert_eq!(attempts(&f), i + 1, "{kind} {i}: exact admission attempts");
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
    positive("positive control");
    for i in 0..4 {
        let f = Call::new();
        let ingress = f.b.session.ingress();
        f.hook("model.call.observed", i, move || {
            assert!(ingress.emergency().is_ok())
        });
        let outcome = f.call().expect("ticketed attempt");
        assert_eq!(f.sent(), CALL[..i], "ingress wins the CAS at {i}");
        assert!(outcome.result().is_err() && recorded(&outcome), "{i}");
        assert_eq!(
            (attempts(&f), count(&f, "admitted"), notices(&f)),
            (i + 1, i, 0),
            "{i}: one admission per gate, no refreshed retry, no notice"
        );
    }
}

/// L2 with F5 (broker level; composed zeros carried to C/D): revocation
/// after the observer read, before the CAS: at most the selected request,
/// a Recorded outcome, no File effect, the revocation visible at return.
#[test]
fn sb3_revocation_between_observer_read_and_cas_is_entered_only() {
    positive("positive control");
    for i in 0..4 {
        let f = Call::new();
        let owner = f.owner.clone();
        f.hook("model.call.observed", i, move || owner.revoke());
        let file = crate::files::FileCreate {
            name: "b1c.txt".into(),
            content: "pending\n".into(),
        };
        let proposed = f.b.door.create_file(f.b.run, &file).unwrap();
        let crate::governed::AgentOutcome::AwaitingApproval(view) = proposed else {
            panic!("a populated commitment awaiting approval");
        };
        // The actual fields (Debug redacts target and summary to sizes and
        // truncates id and run to 8 hex digits).
        let snapshot = || match f.b.door.commitment(view.id) {
            crate::governed::Lookup::Mine(v) => {
                let fields = format!("{v:?}");
                Some((fields, v.id, v.run, v.target, v.summary, v.binding_short))
            }
            _ => None,
        };
        let content = snapshot().filter(|s| s.0.contains("Prepared"));
        let populated = content.is_some_and(|s| !s.3.is_empty() && !s.4.is_empty());
        assert!(populated, "{i}: a populated, prepared commitment");
        let (before, steps, held) = (broker(&f), file_steps(&f), snapshot());
        let outcome = f.call().expect("ticketed attempt");
        let sent = f.sent();
        assert!(
            sent.len() <= i + 1 && CALL[..sent.len()] == sent[..],
            "{i}: at most the selected request; successors 0"
        );
        assert!(recorded(&outcome), "{i}: the affected outcome is Recorded");
        assert!(f.owner.observer().is_revoked(), "{i}: revocation visible");
        assert_eq!(
            broker(&f),
            before,
            "{i}: inventory and commitments unchanged"
        );
        assert_eq!(steps.load(Ordering::SeqCst), 0, "{i}: File dispatch 0");
        assert_eq!(
            snapshot(),
            held,
            "{i}: the populated commitment is unchanged"
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
    positive("positive control");
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
    one_correlated_record(&f, &outcome);
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
        one_correlated_record(&f, &outcome);
    }
}

/// G0 (every call gate): only the original live grant admits.
#[test]
fn sb3_original_file_grant_remains_required_per_request() {
    positive("original grant reaches chat");
    for case in ["revoked", "expired", "scope", "substitute"] {
        for i in 0..4 {
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
            f.hook("model.call.gate", i, vary);
            let outcome = f.call().expect("ticketed attempt");
            assert_eq!(
                f.sent(),
                CALL[..i],
                "gate {i} {case}: boundary and successors 0"
            );
            assert_eq!(class(&outcome), "model_cancelled", "{i} {case}");
            assert!(recorded(&outcome), "{i} {case}");
        }
    }
}

/// T1: the original Q, mission clocks, backstop and watch are never renewed.
#[test]
fn sb3_original_deadlines_and_watch_are_not_renewed() {
    positive("positive control");
    for (case, expected) in [
        ("q", "model_timeout"), // SB3B-R7 (1): any expiry.
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
            "backstop" => {
                session
                    .core
                    .model
                    .backstop_skew
                    .fetch_add(181_000, Ordering::SeqCst);
            }
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

// SB3B-R2 gap oracles for B1b (findings 1-5, 7, 8, 11), from the sheet.
fn trace(f: &Call) -> Vec<&'static str> {
    f.b.session.core.model.trace.lock().unwrap().clone()
}
fn count(f: &Call, what: &str) -> usize {
    trace(f).iter().filter(|t| t.starts_with(what)).count()
}
/// Dispatch notices in this call's native record (none may exist).
fn notices(f: &Call) -> usize {
    let rows = f.b.primary.store().rows();
    let calls = rows.iter().filter(|r| r.phase == "model_call");
    calls.filter(|r| r.body.contains("sent_mono_ms")).count()
}
/// Every admission attempt (admitted or refused), counted inside admission.
fn attempts(f: &Call) -> usize {
    let trace = trace(f);
    let attempt = |t: &&&str| **t == "admitted" || t.starts_with("refused:");
    trace.iter().filter(attempt).count()
}
/// Direct File-dispatch observation: each File commitment step and each
/// File staging or execution start (SB3B-R13 (O)), counted.
fn file_steps(f: &Call) -> Arc<AtomicUsize> {
    use crate::test_hooks::at::{AUTHORIZE_RECORDED, COMPLETE_BEFORE_CHECK, PREPARE_BEFORE_INSERT};
    use crate::test_hooks::at::{FILE_EXECUTE_START, FILE_STAGED};
    let (steps, hooks) = (Arc::new(AtomicUsize::new(0)), f.b.control());
    for at in [
        PREPARE_BEFORE_INSERT,
        AUTHORIZE_RECORDED,
        COMPLETE_BEFORE_CHECK,
    ] {
        let step = steps.clone();
        let fire = move || {
            step.fetch_add(1, Ordering::SeqCst);
        };
        hooks.authority().commitments().hooks().every(at, fire);
    }
    for at in [FILE_STAGED, FILE_EXECUTE_START] {
        let step = steps.clone();
        let fire = move || {
            step.fetch_add(1, Ordering::SeqCst);
        };
        hooks.files().hooks().every(at, fire);
    }
    steps
}
/// The broker-visible File side: inventory, the commitment registry (size
/// and pending target) and every non-model evidence record in full.
type Broker = (
    String,
    usize,
    Option<String>,
    Vec<(String, Option<String>, String)>,
);
fn broker(f: &Call) -> Broker {
    let s = &f.b.session;
    let rows = f.b.primary.store().rows();
    let other = rows.iter().filter(|r| !r.phase.starts_with("model_"));
    let pending = s
        .pending_for_test()
        .map(|id| format!("{:?}", f.b.door.commitment(id)));
    (
        format!("{:?}", s.inventory().unwrap().entries),
        f.b.control().authority().commitments().len_for_test(),
        pending,
        other
            .map(|r| (r.phase.clone(), r.commitment.clone(), r.body.clone()))
            .collect(),
    )
}
fn refusals(f: &Call) -> Vec<&'static str> {
    let trace = trace(f);
    trace
        .into_iter()
        .filter(|t| t.starts_with("refused"))
        .collect()
}
/// One R5 predicate made false at a gate, with every earlier one still true.
fn fault(f: &Call, kind: &'static str) -> Box<dyn Fn() + Send + Sync> {
    let (session, q, control, runs) = (
        f.b.session.clone(),
        f.b.q.sequence(),
        f.b.control(),
        f.b.runs(),
    );
    let (run, grant, owner, primary) = (f.b.run, f.b.grant(), f.owner.clone(), f.b.primary.clone());
    let (release, held) = std::sync::mpsc::channel::<()>();
    let held = Mutex::new(Some(held));
    if kind == "health_none" {
        let core = &f.b.session.core.cell.1;
        core.arm("model.attempt.return", move || {
            let _ = release.send(());
        });
    }
    Box::new(move || match kind {
        "q" => session.expire_request_for_test(q),
        "backstop" => {
            session
                .core
                .model
                .backstop_skew
                .fetch_add(181_000, Ordering::SeqCst);
        }
        "watch" => assert!(control.kernel_stop_authority().engage() > 0),
        "health" => {
            primary.store().simulate_crash(); // latched: unhealthy
            assert!(primary
                .try_health()
                .is_some_and(|h| h.owed > 0 || h.latched));
        }
        "health_none" => {
            let (store, held) = (primary.clone(), held.lock().unwrap().take().unwrap());
            let (ready, wait) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let _held = store.store().hold();
                ready.send(()).unwrap();
                let _ = held.recv_timeout(Duration::from_secs(3));
            });
            wait.recv_timeout(Duration::from_secs(1)).unwrap();
            assert!(primary.try_health().is_none(), "try_health None");
        }
        "qdead" => assert!(session.ingress().emergency().is_ok()),
        "host" => runs.tamper_for_test(run, "view_id"),
        "unpin" => runs.tamper_for_test(run, "unpin"),
        "member" => runs.tamper_for_test(run, "groups"),
        "sticky" => runs.tamper_for_test(run, "sticky"),
        "pin" => runs.tamper_for_test(run, "pin"),
        "grant" => control.authority().grants().revoke(grant).unwrap(),
        _ => owner.revoke(),
    })
}
/// (fault, class, the step the trace must show refusing).
const FAULTS: [(&str, &str, &str); 13] = [
    ("q", "model_timeout", "refused:1"), // SB3B-R7 (1): any expiry.
    ("qdead", "model_cancelled", "refused:1"),
    ("backstop", "model_timeout", "refused:1"),
    ("watch", "model_stopped", "refused:2"),
    ("health", "model_cancelled", "refused:3"),
    ("health_none", "model_cancelled", "refused:3"),
    ("member", "model_cancelled", "refused:4"),
    ("host", "model_cancelled", "refused:4"),
    ("unpin", "model_cancelled", "refused:4"),
    ("sticky", "model_cancelled", "refused:4"),
    ("pin", "model_cancelled", "refused:4"),
    ("grant", "model_cancelled", "refused:5"),
    ("observer", "model_cancelled", "refused:7"),
];

/// F2 + F8: each R5 predicate, falsified alone after the ticket and before
/// each gate, refuses with its literal class at its own step.
#[test]
fn sb3_r5_each_predicate_refuses_literally() {
    positive("positive control");
    for (kind, expected, step) in FAULTS {
        for i in 0..4 {
            let f = Call::new();
            f.hook("model.call.gate", i, fault(&f, kind));
            let outcome = f.call().expect("ticketed attempt");
            assert_eq!(class(&outcome), expected, "{kind} at gate {i}");
            assert_eq!(
                f.sent(),
                CALL[..i],
                "{kind} at gate {i}: boundary and successors 0"
            );
            assert_eq!(refusals(&f), [step], "{kind} at gate {i}: its own step");
            assert_eq!(
                f.charged().0,
                u32::from(i == 3),
                "{kind} {i}: no new charge"
            );
        }
    }
    // Chat latch: an uncharged chat admission and a second (spent) one.
    for seam in ["early", "replay"] {
        let f = Call::new();
        let core = &f.b.session.core.model;
        let latch = if seam == "early" {
            &core.chat_early
        } else {
            &core.chat_replay
        };
        latch.store(true, Ordering::SeqCst);
        let outcome = f.call().expect("ticketed attempt");
        assert!(outcome.result().is_ok(), "{seam}: the real chat proceeds");
        let trace = trace(&f);
        let at = trace
            .iter()
            .position(|t| *t == seam)
            .expect("seam exercised");
        assert_eq!(
            trace[at + 1..at + 3],
            ["refused:6", "probe:model_cancelled"],
            "{seam}: the latch refuses literally"
        );
        assert_eq!(f.sent(), CALL, "{seam}: exact peer sequence, one chat");
        assert_eq!(f.charged(), (1, 1), "{seam}: exactly one charge");
    }
}

/// F2 (SB3B-R8 3): contention at each R5 hold (run table, grant store) and
/// each gate refuses and latches nothing; poison latches both.
#[test]
fn sb3_r5_holds_contend_and_poison_exactly() {
    positive("positive control");
    for hold in ["runs", "grants"] {
        for poisoned in [false, true] {
            for i in 0..4 {
                let f = Call::new();
                let (runs, control) = (f.b.runs(), f.b.control());
                let (release, rx) = std::sync::mpsc::channel::<()>();
                let rx = Mutex::new(Some(rx));
                let holder: Holder = Arc::new(Mutex::new(None));
                let keep = holder.clone();
                f.hook("model.call.gate", i, move || {
                    let (runs, control) = (runs.clone(), control.clone());
                    let rx = rx.lock().unwrap().take().unwrap();
                    let (ready, wait) = std::sync::mpsc::channel();
                    *keep.lock().unwrap() = Some(std::thread::spawn(move || {
                        let grants = control.authority().grants();
                        let _runs = (hold == "runs").then(|| runs.model_observer());
                        let _grants = (hold == "grants").then(|| grants.hold_for_test());
                        ready.send(()).unwrap();
                        if poisoned {
                            std::panic::resume_unwind(Box::new("poison (test)"));
                        }
                        let _ = rx.recv_timeout(Duration::from_secs(3));
                    }));
                    wait.recv_timeout(Duration::from_secs(1)).unwrap();
                    if poisoned {
                        let joined = keep_join(&keep);
                        assert!(joined, "{hold} poisoned");
                    }
                });
                f.b.session
                    .core
                    .cell
                    .1
                    .arm("model.attempt.return", move || {
                        let _ = release.send(());
                    });
                // Latched at the refusal: read at return, before settlement.
                let (seen, core) = (
                    Arc::new(Mutex::new(None)),
                    Arc::downgrade(&f.b.session.core),
                );
                let keep_seen = seen.clone();
                f.hook("model.attempt.return", 0, move || {
                    let core = core.upgrade().unwrap();
                    let failed = core.cell.session_word() & FAILED != 0;
                    *keep_seen.lock().unwrap() =
                        Some((core.model.uncertain.load(Ordering::SeqCst), failed));
                });
                let outcome = f.call().expect("ticketed attempt");
                if let Some(h) = holder.lock().unwrap().take() {
                    let _ = h.join();
                }
                let what = format!("{hold} poisoned={poisoned} gate {i}");
                assert_eq!(class(&outcome), "model_cancelled", "{what}");
                assert_eq!(f.sent(), CALL[..i], "{what}: boundary and successors 0");
                assert_eq!(f.latched(), (poisoned, poisoned), "{what}: latches");
                let refused = *seen.lock().unwrap();
                assert_eq!(
                    refused,
                    Some((poisoned, poisoned)),
                    "{what}: latched at refusal"
                );
            }
        }
    }
}
/// Joins a poisoning holder (its panic is the poison) inside the gate.
fn keep_join(keep: &Holder) -> bool {
    let handle = keep.lock().unwrap().take();
    handle.is_some_and(|h| h.join().is_err())
}

/// F1: for each adjacent pair of R5 predicates both false, the earlier one
/// refuses (ordered observation trace, codes coincide from step 3).
#[test]
fn sb3_r5_predicate_order_is_exact() {
    positive("positive control");
    for (first, second, step) in [
        ("q", "watch", "refused:1"),
        ("watch", "health", "refused:2"),
        ("health", "member", "refused:3"),
        ("member", "grant", "refused:4"),
    ] {
        let f = Call::new();
        let (a, b) = (fault(&f, second), fault(&f, first));
        f.hook("model.call.gate", 1, move || {
            a();
            b();
        });
        let _ = f.call().expect("ticketed attempt");
        assert_eq!(refusals(&f), [step], "{first} before {second}");
    }
    // Chat latch pairs, at the spent second admission (replay seam).
    for (first, step) in [("grant", "refused:5"), ("latch", "refused:6")] {
        let f = Call::new();
        f.b.session
            .core
            .model
            .chat_replay
            .store(true, Ordering::SeqCst);
        let late = fault(
            &f,
            if first == "grant" {
                "grant"
            } else {
                "observer"
            },
        );
        f.b.session.core.cell.1.arm("model.call.replay", late);
        assert!(
            f.call().is_ok_and(|o| o.result().is_ok()),
            "{first}: real chat"
        );
        let trace = trace(&f);
        let at = trace.iter().position(|t| *t == "replay").expect("replayed");
        assert_eq!(trace[at + 1], step, "{first} precedes the next predicate");
    }
}

/// F3: contention refuses at the override while a cancellation poll taken
/// under that contention returns None; no dispatch notice; one admission
/// per gate (no local retry); the session transport passes no observer.
#[test]
fn sb3_contended_gate_polls_live_and_notifies_nothing() {
    positive("positive control");
    let inference = include_str!("../inference.rs");
    let transport = inference
        .split("pub(crate) fn session_transport(")
        .nth(1)
        .and_then(|body| body.split("\n}\n").next())
        .expect("the session transport");
    assert!(
        transport.contains("observer: None,") && transport.contains("transport.call("),
        "the session transport's own context has no dispatch observer"
    );
    for i in 0..4 {
        let f = Call::new();
        let (runs, core) = (f.b.runs(), Arc::downgrade(&f.b.session.core));
        let (release, rx) = std::sync::mpsc::channel::<()>();
        let rx = Mutex::new(Some(rx));
        let holder: Holder = Arc::new(Mutex::new(None));
        let keep = holder.clone();
        f.b.session.core.cell.1.every("model.call.polled", move || {
            let core = core.upgrade().unwrap();
            let trace = core.model.trace.lock().unwrap().clone();
            let admitted = trace.iter().filter(|t| **t == "admitted").count();
            let Some(rx) = (admitted == i).then(|| rx.lock().unwrap().take()).flatten() else {
                return;
            };
            let (runs, (ready, wait)) = (runs.clone(), std::sync::mpsc::channel());
            *keep.lock().unwrap() = Some(std::thread::spawn(move || {
                let _hold = runs.model_observer();
                ready.send(()).unwrap();
                let _ = rx.recv_timeout(Duration::from_secs(3));
            }));
            wait.recv_timeout(Duration::from_secs(1)).unwrap();
            core.model.trace.lock().unwrap().push("contended");
        });
        f.b.session
            .core
            .cell
            .1
            .arm("model.attempt.return", move || {
                let _ = release.send(());
            });
        let outcome = f.call().expect("ticketed attempt");
        holder
            .lock()
            .unwrap()
            .take()
            .expect("contended")
            .join()
            .unwrap();
        let trace = trace(&f);
        let from = trace.iter().position(|t| *t == "contended").unwrap();
        let polled = trace[from..].iter().position(|t| *t == "poll:none");
        let refused = trace[from..].iter().position(|t| *t == "refused:hold");
        assert!(
            polled.is_some_and(|p| refused.is_some_and(|r| p < r)),
            "{i}: a poll under contention returned None before the refusal"
        );
        assert!(
            !trace.contains(&"poll:cancelled"),
            "{i}: polling stayed None"
        );
        assert_eq!(count(&f, "gate:"), i + 1, "{i}: no retry");
        assert_eq!(notices(&f), 0, "{i}: no dispatch notice");
        assert_eq!(f.sent(), CALL[..i], "{i}: affected and successors 0");
        assert_eq!(
            (class(&outcome), f.latched()),
            ("model_cancelled", (false, false))
        );
    }
}

/// F4: each mission clock alone; a fixed backstop; a second attempt that
/// crosses the original deadline; Stop and resume inside one attempt.
#[test]
fn sb3_call_clocks_backstop_and_watch_are_fixed() {
    positive("positive control");
    for clock in ["monotonic", "wall"] {
        let f = Call::new();
        let c = f.b.clock.clone();
        let far = 1 << 50;
        let (mono, wall) = match clock {
            "monotonic" => (c.monotonic_ms() + 1000, far),
            _ => (far, c.wall_ms() + 1000),
        };
        f.b.door
            .context()
            .narrow_mission_deadlines(c.clone(), mono, wall)
            .unwrap();
        assert!(
            f.call().is_ok_and(|o| o.result().is_ok()),
            "{clock}: attempt one"
        );
        let advance = move |c: &SplitClock, ms| match clock {
            "monotonic" => c.mono_only(Duration::from_millis(ms)),
            _ => c.wall_only(Duration::from_millis(ms)),
        };
        // A deadline renewed at attempt two would end 1,600 ms out; the
        // original ends at 1,000 ms; gate 1 of attempt two is at 1,200 ms.
        advance(&c, 600);
        f.hook("model.call.gate", 1, move || advance(&c, 600));
        let outcome = f.call().expect("attempt two starts before the deadline");
        assert_eq!(
            class(&outcome),
            "model_timeout",
            "{clock} crosses at gate 1"
        );
        assert_eq!(f.sent()[4..], CALL[..1], "{clock}: attempt two prefix");
    }
    // The backstop is captured once per attempt and never extended.
    let f = Call::new();
    let skew = f.b.session.core.model.backstop_skew.clone();
    let gates = AtomicUsize::new(0);
    f.b.session.core.cell.1.every("model.call.gate", move || {
        match gates.fetch_add(1, Ordering::SeqCst) {
            1 => skew.fetch_add(179_000, Ordering::SeqCst),
            2 => skew.fetch_add(2_000, Ordering::SeqCst),
            _ => 0,
        };
    });
    let outcome = f.call().expect("ticketed attempt");
    assert_eq!(
        class(&outcome),
        "model_timeout",
        "the backstop was not extended"
    );
    assert_eq!(f.sent(), CALL[..2], "gate 2 refuses after 181 s");
    assert!(
        f.call().is_ok_and(|o| o.result().is_ok()),
        "a new attempt, a new backstop"
    );
    // Stop and resume before the next gate of the same attempt.
    let f = Call::new();
    let control = f.b.control();
    f.hook("model.call.gate", 1, move || {
        assert!(control.kernel_stop_authority().engage() > 0);
        control.resume_with_halt(&Owner, None).unwrap();
    });
    let outcome = f.call().expect("ticketed attempt");
    assert!(
        !f.b.control().stop_snapshot().kernel_stopped,
        "resumed natively"
    );
    assert_eq!(class(&outcome), "model_stopped", "the retained watch");
    let reread = f.b.session.core.model.registered_identity_for_test();
    let door = &f.b.door;
    let rebind = door.bind_inference_private(f.b.run, HASH, LABEL, reread.expect("installed M"));
    assert_eq!(
        rebind.map_err(|e| e.class()),
        Err("halted"),
        "a re-read identity restores nothing"
    );
    assert_eq!(f.call().map(|_| ()), Err("halted"), "no renewed authority");
}

/// F7 (SB3B-R3 2): a successful, durably recorded call whose exact pin
/// cannot settle is Uncertain, never Recorded; the result stays separate.
#[test]
fn sb3_unsettled_recorded_call_is_uncertain() {
    positive("positive control");
    let f = Call::new();
    let (runs, run) = (f.b.runs(), f.b.run);
    f.b.session
        .core
        .cell
        .1
        .arm("model.attempt.return", move || {
            runs.tamper_for_test(run, "pin")
        });
    let outcome = f.call().expect("ticketed attempt");
    assert!(
        matches!(outcome.record(), EndStatus::Uncertain),
        "never Recorded"
    );
    assert!(outcome.result().is_ok(), "transport result kept separately");
    assert_eq!(f.records(), 1, "exactly one durable record");
    let o = f.obligations();
    assert!(
        o.active && o.uncertain && f.latched() == (true, true),
        "slot debt"
    );
}

/// F8: pre-ticket refusals through the call charge and meter nothing.
#[test]
fn sb3_call_r3_refusals_charge_nothing() {
    positive("positive control");
    for (case, class) in [
        ("grant", "grant_not_live"),
        ("observer", "halted"),
        ("ninth", "model_over_budget"),
        ("q", "halted"),
        ("member", "not_member"),
        ("sticky", "no_consent"),
    ] {
        let f = Call::new();
        match case {
            "grant" | "q" | "member" | "sticky" => fault(&f, case)(),
            "observer" => f.owner.revoke(),
            _ => (0..8).for_each(|_| drop(f.call().unwrap())),
        }
        let before = (f.sent().len(), f.charged(), f.records());
        assert_eq!(f.call().map(|_| ()), Err(class), "{case}");
        assert_eq!(
            (f.sent().len(), f.charged(), f.records()),
            before,
            "{case}: 0/0/0"
        );
    }
}

/// SB3B-R5 (3), SB3B-R8 (9), SB3B-R11 P1: no clock is read under a gate's
/// hold. That covers the clocks (blocking), the caller's
/// `CallDeadline::expired()`, `Instant` in scope admission ("scope.clock")
/// and the grant-poison failure path ("close.failed.published"). While a
/// clock blocks, both holds are free and the call still completes.
#[test]
fn sb3_call_gate_reads_clocks_and_deadline_before_any_hold() {
    positive("positive control");
    for schedule in ["plain", "poison", "exhausted"] {
        let poison = schedule == "poison";
        let f = Call::new();
        let c = f.b.clock.clone();
        let (mono, wall) = (c.monotonic_ms() + 600_000, c.wall_ms() + 600_000);
        let context = f.b.door.context();
        context
            .narrow_mission_deadlines(c.clone(), mono, wall)
            .unwrap();
        let held = Arc::new(Mutex::new(Vec::new()));
        let late = Arc::new(Mutex::new(Vec::new()));
        let (runs, run, control) = (f.b.runs(), f.b.run, f.b.control());
        let (seen, keep, window) = (held.clone(), late.clone(), Arc::new(AtomicBool::new(false)));
        let open = window.clone();
        let check: Arc<dyn Fn(&'static str) + Send + Sync> = Arc::new(move |what| {
            if !open.load(Ordering::SeqCst) {
                return;
            }
            let runs = runs.clone();
            let table = std::thread::spawn(move || runs.model_accounting(run)).join();
            let (control, (tx, rx)) = (control.clone(), std::sync::mpsc::channel());
            let store = std::thread::spawn(move || {
                drop(control.authority().grants().hold_for_test()); // Released, then free.
                let _ = tx.send(());
            });
            let free = rx.recv_timeout(Duration::from_secs(2)).is_ok();
            keep.lock().unwrap().push(store);
            if table.is_err() || !free {
                seen.lock().unwrap().push((what, table.is_err(), !free));
            }
        });
        let reads = Arc::new(AtomicUsize::new(0));
        let (count, clock_check) = (reads.clone(), check.clone());
        *c.on_read.lock().unwrap() = Some(Arc::new(move || {
            count.fetch_add(1, Ordering::SeqCst);
            clock_check("clock");
        }));
        let hooks = &f.b.session.core.cell.1;
        for (at, what) in [
            ("scope.clock", "instant"),
            ("close.failed.published", "fail"),
        ] {
            let check = check.clone();
            hooks.every(at, move || check(what));
        }
        let (on, gates, poisoner) = (window.clone(), AtomicUsize::new(0), f.b.control());
        let scope = f.b.door.context.scope.clone();
        hooks.every("model.call.gate", move || {
            on.store(true, Ordering::SeqCst);
            let gate = gates.fetch_add(1, Ordering::SeqCst);
            if gate == 1 && schedule == "exhausted" {
                scope.next_step.store(u64::MAX, Ordering::SeqCst); // SB3B-R13 (P1)
            }
            if poison && gate == 1 {
                let control = poisoner.clone();
                let _ = std::thread::spawn(move || {
                    let _held = control.authority().grants().hold_for_test();
                    std::panic::resume_unwind(Box::new("poison the grant store (test)"));
                })
                .join();
            }
        });
        let off = window.clone();
        hooks.arm("model.attempt.return", move || {
            off.store(false, Ordering::SeqCst)
        });
        let deadline = Probing(check, AtomicUsize::new(0));
        let observer = f.owner.observer();
        let door = &f.b.door;
        let outcome = door.model_call_private(run, &request(), &f.meter, &deadline, &observer);
        window.store(false, Ordering::SeqCst);
        for store in std::mem::take(&mut *late.lock().unwrap()) {
            store.join().unwrap();
        }
        assert!(
            reads.load(Ordering::SeqCst) >= 4,
            "the gates read the clocks"
        );
        assert!(
            deadline.1.load(Ordering::SeqCst) >= 1,
            "the deadline is read"
        );
        assert_eq!(
            *held.lock().unwrap(),
            [],
            "(what, run table, grant store) held"
        );
        let outcome = outcome.expect("ticketed attempt");
        let expected = if schedule == "plain" {
            "ok"
        } else {
            "model_cancelled"
        };
        let failed = f.b.session.core.cell.session_word() & FAILED != 0;
        assert!(
            failed || schedule == "plain",
            "{schedule}: the failure latch holds"
        );
        assert_eq!(
            class(&outcome),
            expected,
            "a blocking clock delays, never refuses"
        );
    }
}
/// A caller deadline that checks, at every reading, that no hold is held.
struct Probing(Arc<dyn Fn(&'static str) + Send + Sync>, AtomicUsize);
impl CallDeadline for Probing {
    fn expired(&self) -> bool {
        self.1.fetch_add(1, Ordering::SeqCst);
        (self.0)("deadline");
        false
    }
}

/// SB3B-R5 (1) at R5, as corrected by SB3B-R8 (8). A revocation starts at
/// a gate's observer read, and the revoker is joined before judging. A
/// test seam observes each successful CAS admission (`model.call.admitted`
/// fires right after the CAS, under the holds). The invariant is that no
/// CAS admission follows a completed revocation. A slow revoker only
/// removes evidence; it can never fail correct code.
#[test]
fn sb3_call_grant_revocation_cannot_precede_the_cas() {
    positive("positive control");
    for i in 0..4 {
        let f = Call::new();
        let (control, grant) = (f.b.control(), f.b.grant());
        let done = Arc::new(AtomicBool::new(false));
        let (finished, after) = (done.clone(), done.clone());
        let holder: Holder = Arc::new(Mutex::new(None));
        let keep = holder.clone();
        f.hook("model.call.observed", i, move || {
            let (control, finished) = (control.clone(), finished.clone());
            let (tx, rx) = std::sync::mpsc::channel();
            *keep.lock().unwrap() = Some(std::thread::spawn(move || {
                let _ = control.authority().grants().revoke(grant);
                finished.store(true, Ordering::SeqCst);
                let _ = tx.send(());
            }));
            let _ = rx.recv_timeout(Duration::from_secs(2));
        });
        let invalid = Arc::new(AtomicUsize::new(0));
        let flagged = invalid.clone();
        f.b.session
            .core
            .cell
            .1
            .every("model.call.admitted", move || {
                if after.load(Ordering::SeqCst) {
                    flagged.fetch_add(1, Ordering::SeqCst);
                }
            });
        let outcome = f.call().expect("ticketed attempt");
        holder
            .lock()
            .unwrap()
            .take()
            .expect("gate reached")
            .join()
            .unwrap();
        assert_eq!(
            invalid.load(Ordering::SeqCst),
            0,
            "{i}: a CAS admission followed a completed revocation"
        );
        assert!(done.load(Ordering::SeqCst), "{i}: the revoker was joined");
        let grants = f.b.control();
        assert!(
            grants.authority().grants().live(f.b.grant()).is_none(),
            "{i}: revocation landed"
        );
        assert!(recorded(&outcome), "{i}: one recorded result");
    }
}

/// SB3B-R13 (P2): mission limits installed after a gate sampled none (so
/// no reading exists for them) refuse that gate's CAS; never fabricated time.
#[test]
fn sb3_limits_without_a_sample_fail_closed() {
    positive("positive control");
    for i in 0..4 {
        let f = Call::new();
        let (context, clock) = (f.b.door.context(), f.b.clock.clone());
        f.hook("model.call.observed", i, move || {
            let installed = context.narrow_mission_deadlines(clock.clone(), 1, 1);
            assert!(installed.is_ok(), "already-expired, positive limits");
        });
        let outcome = f.call().expect("ticketed attempt");
        assert_eq!(
            f.sent(),
            CALL[..i],
            "{i}: limits without a sample refuse the CAS"
        );
        assert!(recorded(&outcome), "{i}: recorded");
    }
}

/// SB3B-R11 P2: a slow metadata or chat request crossing the original Q,
/// either mission clock or the backstop ends `model_timeout`, recorded so.
#[test]
fn sb3_slow_request_crossing_a_deadline_times_out() {
    if watched("sb3_slow_request_crossing_a_deadline_times_out") {
        return;
    }
    positive("positive control");
    for i in [0, 3] {
        for kind in ["q", "wall", "monotonic", "backstop"] {
            let (f, outcome, _) = held(i, move |f| match kind {
                "q" => f.b.session.expire_request_for_test(f.b.q.sequence()),
                "backstop" => drop(
                    f.b.session
                        .core
                        .model
                        .backstop_skew
                        .fetch_add(181_000, Ordering::SeqCst),
                ),
                _ => {
                    let c = f.b.clock.clone();
                    let (m, w) = (c.monotonic_ms() + 1_000, c.wall_ms() + 1_000);
                    f.b.door
                        .context()
                        .narrow_mission_deadlines(c.clone(), m, w)
                        .unwrap();
                    let by = Duration::from_secs(2);
                    if kind == "wall" {
                        c.wall_only(by)
                    } else {
                        c.mono_only(by)
                    }
                }
            });
            assert_eq!(
                class(&outcome),
                "model_timeout",
                "{kind} during request {i}"
            );
            let rows = f.b.primary.store().rows();
            let call = rows.iter().rfind(|r| r.phase == "model_call").unwrap();
            let timed_out = recorded(&outcome) && call.body.contains("\"timeout\"");
            assert!(timed_out, "{kind} {i}: recorded model_timeout");
        }
    }
    println!("{HELD_DONE} sb3_slow_request_crossing_a_deadline_times_out");
}

/// F11 for B1b (SB3B-R8 10, SB3B-R11 O3): every production module of
/// host_session (a cfg(test) declaration in host_session.rs skips one),
/// host_session.rs, run.rs and inference.rs are scanned for every B1b seam
/// and helper; each is declared and used; removing a field's
/// `#[cfg(test)]` or adding a production use turns the guard RED.
#[test]
fn sb3_b1b_test_seams_are_test_only() {
    let fields = ["trace", "backstop_skew", "chat_replay", "chat_early"];
    let helpers = [
        "SkewClock",
        "b1b_note",
        "b1b_latch_probe",
        "b1b_gate_entry",
        "b1b_gate_exit",
    ];
    let model = include_str!("model.rs");
    for seam in fields.iter().chain(&helpers) {
        let sites = seam_sites(model, &[seam]).len();
        assert!(sites >= 2, "{seam}: declared and used ({sites})");
    }
    let mut seams: Vec<&str> = fields.iter().chain(&helpers).copied().collect();
    seams.push("registered_identity_for_test"); // read by tests only
    seams.extend(["expired_request", "expire_request_for_test"]); // SB3B-R18 Q seam
    let parent = include_str!("../host_session.rs");
    let (code, mut scanned) = (mask(parent), 0);
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/src/host_session");
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_stem().unwrap().to_string_lossy().into_owned();
        let at = code.find(&format!("mod {name};")).expect("declared module");
        if !test_only(&code, at) {
            let source = std::fs::read_to_string(&path).unwrap();
            assert_eq!(
                leaks(&source, &seams),
                0,
                "{name}.rs: a B1b seam outside cfg(test)"
            );
            scanned += 1;
        }
    }
    assert_eq!(scanned, 7, "every production host_session module scanned");
    let run = include_str!("../authority/run.rs");
    for (name, source) in [("host_session.rs", parent), ("run.rs", run)] {
        assert_eq!(
            leaks(source, &seams),
            0,
            "{name}: a B1b seam outside cfg(test)"
        );
    }
    let inference = include_str!("../inference.rs");
    assert_eq!(
        leaks(inference, &seams),
        0,
        "inference.rs: a B1b seam outside cfg(test)"
    );
    for seam in fields {
        let declared = format!("    #[cfg(test)]\n    pub(super) {seam}:");
        assert_eq!(model.matches(&declared).count(), 1, "{seam} declaration");
        let uncfg = model.replace(&declared, &format!("    pub(super) {seam}:"));
        assert!(leaks(&uncfg, &[seam]) >= 1, "{seam}: cfg removal is RED");
    }
    let declared = "    #[cfg(test)]\n    expired_request:";
    assert_eq!(parent.matches(declared).count(), 1, "Q seam declaration");
    let uncfg = parent.replace(declared, "    expired_request:");
    assert!(
        leaks(&uncfg, &["expired_request"]) >= 1,
        "Q seam: cfg removal is RED"
    );
    for seam in &seams {
        let used = format!("{model}\nfn production() {{\n    {seam}();\n}}\n");
        assert!(
            leaks(&used, &[seam]) >= 1,
            "{seam}: a production use is RED"
        );
    }
}

// SB-3B1e: test-only evidence carries (SB3B-R3, SB3B-R15).

/// F10a: the marker a held schedule's child prints when its body ends.
const HELD_DONE: &str = "SB3B1E HELD BODY DONE";
/// F10a: a held schedule runs only in a child process of this test binary,
/// under one 30 s deadline with kill and wait. The parent requires the
/// child's success AND the body's marker, so a skipped body cannot pass.
/// Returns true in the parent (the schedule ran in the child).
fn watched(name: &str) -> bool {
    use std::io::Read;
    const CHILD: &str = "SB3B1E_HELD_CHILD";
    if std::env::var(CHILD).is_ok_and(|child| child == name) {
        return false;
    }
    struct Owned(std::process::Child);
    impl Drop for Owned {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let test = format!("host_session::model_call_tests::{name}");
    let mut child = Owned(
        std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &test, "--nocapture", "--test-threads=1"])
            .env(CHILD, name)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let mut stdout = child.0.stdout.take().unwrap();
    let reader = std::thread::spawn(move || {
        let mut out = String::new();
        let _ = stdout.read_to_string(&mut out);
        out
    });
    let until = Instant::now() + Duration::from_secs(30);
    let status = loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            break status;
        }
        assert!(
            Instant::now() < until,
            "{name}: the held child exceeded 30 s"
        );
        std::thread::sleep(Duration::from_millis(5)); // Polls completion only.
    };
    let out = reader.join().unwrap();
    let marker = out.contains(&format!("{HELD_DONE} {name}"));
    assert!(
        status.success() && marker,
        "{name}: child success and its marker"
    );
    true
}

/// A2 and F6 (SB3B-R2 6): while one operation holds the slot at
/// registration, metadata, chat, recording or owner completion, a second
/// acquisition refuses before its ticket: attempts/records/charges 0/0/0.
#[test]
fn sb3_concurrent_operation_refuses_before_ticket() {
    if watched("sb3_concurrent_operation_refuses_before_ticket") {
        return;
    }
    positive("positive control");
    // Registration precedes READY: no original door exists to call through.
    let r = super::model_tests::fixture();
    let p = Peer::new(None, None);
    let gate = r.session.core.cell.1.clone();
    let paused = crate::test_hooks::Gate::new();
    let pause = paused.clone();
    gate.arm("model.reserved", move || pause.hold());
    std::thread::scope(|s| {
        let registration = s.spawn(|| r.session.register_model_private(&p.endpoint, &pin()));
        paused.reached();
        let q = r.session.ingress().accept(CommandKind::Run, 1);
        let no_door = q.is_err() || r.session.open_file_door(&q.unwrap()).is_err();
        assert!(no_door, "registration: no door to acquire through");
        let rows = r.primary.store().rows();
        let records = rows.iter().filter(|row| row.phase == "model_call").count();
        let slot = r.session.core.model.snapshot().expect("slot observable");
        assert_eq!(
            (slot.operation, records, slot.record),
            ("registration", 0, "pending"),
            "registration held: attempts/records/charges 0/0/0"
        );
        paused.open();
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
            let after = (f.counts(), f.records(), f.charged().0);
            assert_eq!(after, before, "{what}: 0/0/0");
            match &gate {
                Some(gate) => gate.open(),
                None => f.restriction("emergency_restrict")(),
            }
            assert!(call.join().unwrap().is_ok_and(|o| recorded(&o)), "{what}");
        });
    }
    println!("{HELD_DONE} sb3_concurrent_operation_refuses_before_ticket");
}

/// F9 (SB3B-R2 9): exactly one native result record, carrying the admitted
/// attempt's correlation and the returned result's own code.
fn one_correlated_record(f: &Call, outcome: &ModelCallOutcome) {
    let rows = f.b.primary.store().rows();
    let json = |phase: &str| -> Vec<serde_json::Value> {
        let rows = rows.iter().filter(|row| row.phase == phase);
        rows.map(|row| serde_json::from_str(&row.body).unwrap())
            .collect()
    };
    let (calls, doors) = (json("model_call"), json("agent_door_opened"));
    assert_eq!(calls.len(), 1, "exactly one native result record");
    let door = doors.last().expect("door record")["detail"]["door"].clone();
    let code = match outcome.result() {
        Err(crate::authority::AuthorityError::ModelCall(error)) => error.code(),
        _ => "ok",
    };
    for (key, value) in [
        ("door", door.as_str().unwrap().to_owned()),
        ("contract_hash", HASH.into()),
        ("request", LABEL.into()),
        ("identity", hex::encode(f.b.identity)),
        ("call", "1".into()),
        ("outcome", code.into()),
    ] {
        let found = calls[0]["detail"][key].as_str();
        assert_eq!(found, Some(&*value), "record correlation {key}");
    }
}

/// R4 at the call level (moved from B1b): a continuation and a
/// conversation never call; nothing is ticketed, sent or charged.
#[test]
fn sb3_private_call_refuses_continuation_and_conversation() {
    positive("the original door calls");
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
    let nothing = (f.counts(), f.sent().len(), f.charged().0);
    assert_eq!(nothing, ((0, 0), 0, 0), "before ticket");
}

/// G0 at R3 through the call (moved from B1b): without the original live
/// grant nothing is ticketed, sent or charged; another grant never stands in.
#[test]
fn sb3_original_file_grant_required_at_r3_through_the_call() {
    positive("original grant reaches chat");
    for case in ["revoked", "expired", "scope", "substitute"] {
        let f = Call::new();
        let control = f.b.control();
        if case == "substitute" {
            let other = f.b.session.grant_for_test(&f.b.q).unwrap();
            let live = control.authority().grants().live(other).is_some();
            assert!(live, "another live grant");
        }
        match case {
            "expired" => f.b.clock.advance(Duration::from_secs(3600)),
            "scope" => f.b.runs().tamper_for_test(f.b.run, "workspace"),
            _ => control.authority().grants().revoke(f.b.grant()).unwrap(),
        }
        assert_eq!(f.call().map(|_| ()), Err("grant_not_live"), "R3 {case}");
        let nothing = (f.counts(), f.sent().len(), f.charged().0);
        assert_eq!(nothing, ((0, 0), 0, 0), "R3 {case}: no ticket");
    }
}

/// SB-2a registration-gate contention (moved from B1b): each of the four
/// registration gates refuses under contention and latches nothing.
#[test]
fn sb3_registration_gate_contention_latches_nothing() {
    for i in 0..4 {
        let r = super::model_tests::fixture();
        let p = Peer::new(None, None);
        let core = Arc::downgrade(&r.session.core);
        let (tx, rx) = std::sync::mpsc::channel::<()>();
        let rx = Mutex::new(Some(rx));
        super::model_tests::nth(&r.session, "model.gate.before", i, move || {
            let (core, rx) = (core.upgrade().unwrap(), rx.lock().unwrap().take().unwrap());
            let (ready, wait) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let _held = core.model.state.lock().unwrap();
                ready.send(()).unwrap();
                let _ = rx.recv_timeout(Duration::from_secs(3));
            });
            wait.recv_timeout(Duration::from_secs(1)).unwrap();
        });
        // The gate has refused; release before completion's own bounded hold.
        let release = Mutex::new(Some(tx));
        r.session
            .core
            .cell
            .1
            .arm("model.complete.contended", move || {
                let _ = release.lock().unwrap().take().map(|tx| tx.send(()));
            });
        let refused = r.session.register_model_private(&p.endpoint, &pin());
        assert!(refused.is_err(), "registration gate {i}: contended");
        let uncertain = r.session.core.model.uncertain.load(Ordering::SeqCst);
        let failed = r.session.core.cell.session_word() & FAILED != 0;
        assert_eq!((uncertain, failed), (false, false), "registration gate {i}");
    }
}

// SB-3B1e F10b (SB3B-R15, R16, R18): the schedule matrix. Its literal rows
// are `model-bridge-cases.tsv` stage `SB-3B1e-M`, one per (side, class).
const CASES: &str = include_str!("../../../../mother/host/tests/session/model-bridge-cases.tsv");
const SIDES: [&str; 9] = [
    "r3_reserve:before",
    "r3_reserve:after",
    "r3_cas:before",
    "r3_cas:after",
    "charge:before",
    "charge:after",
    "r5_observer:before",
    "r5_cas:before",
    "r5_cas:after",
];
const CLASSES: [&str; 15] = [
    "emergency",
    "cancel_run",
    "door_halt",
    "quit",
    "session_failure",
    "q_expiry",
    "mission_wall",
    "mission_mono",
    "native_halt",
    "run_finish",
    "grant_revocation",
    "grant_expiry",
    "evidence_failure",
    "contention",
    "poison",
];
const STATES: [&str; 8] = [
    "bootstrap",
    "emergency_state",
    "failed",
    "closing_normal",
    "closing_restricted",
    "sealed",
    "closed_clean",
    "closed_unclean",
];
/// SB3B-R18 (1): a mission deadline lapsed at the CAS by narrowing is
/// refused `model_cancelled`; asserted as it is now, until SB3B-B1f.
const KNOWN_DEFECT: [&str; 2] = [
    "r5_cas:before/mission_wall_narrowed",
    "r5_cas:before/mission_mono_narrowed",
];
/// SB3B-R18 (2): the Q seam ignores the sample; no evidence until B1f.
const DEFERRED: [&str; 1] = ["r5_cas:before/q_expiry"];
const MATRIX: &str = "sb3_schedule_matrix_is_judged_exactly";
type Row = (&'static str, &'static str, &'static str, &'static str);
/// The matrix rows: (side, class, test or tag, expected).
fn matrix() -> Vec<Row> {
    let rows = CASES
        .lines()
        .map(|line| line.split('\t').collect::<Vec<_>>());
    let rows = rows.filter(|r| r.len() == 5 && r[4] == "SB-3B1e-M");
    rows.map(|r| (r[2], r[0].rsplit('/').next().unwrap(), r[1], r[3]))
        .collect()
}
/// A class (or executed state) made true where `side` puts it.
/// The contention holder and its release, owned by the schedule.
type Keep = Arc<
    Mutex<(
        Option<std::thread::JoinHandle<()>>,
        Option<std::sync::mpsc::Sender<()>>,
    )>,
>;
fn release(keep: &Keep) {
    let (holder, release) = std::mem::take(&mut *keep.lock().unwrap());
    drop(release);
    if let Some(holder) = holder {
        holder.join().unwrap();
    }
}
fn matrix_fault(f: &Call, class: &'static str, keep: &Keep) -> Arc<dyn Fn() + Send + Sync> {
    let (control, run, clock, session) = (
        f.b.control(),
        f.b.run,
        f.b.clock.clone(),
        f.b.session.clone(),
    );
    let (context, runs, keep) = (f.b.door.context(), f.b.runs(), keep.clone());
    match class {
        "emergency" => Arc::new(f.restriction("emergency_restrict")),
        "cancel_run" | "door_halt" => Arc::new(f.restriction(class)),
        "quit" => Arc::new(f.restriction("close_restrict")),
        "closing_restricted" => {
            let (emergency, quit) = (f.restriction("emergency_restrict"), f.restriction("quit"));
            Arc::new(move || {
                emergency();
                quit();
                assert_eq!(session.ingress().state(), SessionState::ClosingRestricted);
            })
        }
        "sealed" | "closed_clean" | "closed_unclean" => Arc::new(move || {
            // SB3B-R18 (3): set on the session word, in test only.
            let quit = session.ingress().accept(CommandKind::Quit, 9);
            assert!(quit.is_ok(), "closing");
            let word = session.core.cell.session_word();
            let next = match class {
                "sealed" => (word | SEALED) & !READY,
                "closed_clean" => word | SEALED,
                _ => word | SEALED | FAILED,
            };
            assert!(session.core.cell.session_compare(word, next).is_ok());
            let state = match class {
                "sealed" => SessionState::SealedFinalizing,
                "closed_clean" => SessionState::ClosedClean,
                _ => SessionState::ClosedUnclean,
            };
            assert_eq!(session.ingress().state(), state, "{class}");
        }),
        "session_failure" => Arc::new(move || context.fail()),
        "mission_wall" => Arc::new(move || clock.wall_only(Duration::from_secs(2))),
        "mission_mono" => Arc::new(move || clock.mono_only(Duration::from_secs(2))),
        "mission_wall_narrowed" | "mission_mono_narrowed" => Arc::new(move || {
            let far = 1 << 50;
            let (m, w) = match class {
                "mission_wall_narrowed" => (far, clock.wall_ms()),
                _ => (clock.monotonic_ms(), far),
            };
            context
                .narrow_mission_deadlines(clock.clone(), m, w)
                .unwrap();
        }),
        "run_finish" => Arc::new(move || control.finish_run(run)),
        "grant_expiry" => Arc::new(move || clock.advance(Duration::from_secs(3600))),
        "contention" | "poison" => Arc::new(move || {
            let (runs, (ready, wait), poison) =
                (runs.clone(), std::sync::mpsc::channel(), class == "poison");
            let (release, held) = std::sync::mpsc::channel::<()>();
            let holder = std::thread::spawn(move || {
                let _hold = runs.model_observer();
                ready.send(()).unwrap();
                if poison {
                    std::panic::resume_unwind(Box::new("poison the run table (test)"));
                }
                let _ = held.recv_timeout(Duration::from_secs(5)); // Until released.
            });
            wait.recv_timeout(Duration::from_secs(5)).unwrap();
            if poison {
                assert!(holder.join().is_err(), "run table poisoned");
            } else {
                *keep.lock().unwrap() = (Some(holder), Some(release));
            }
        }),
        _ => Arc::from(fault(
            f,
            match class {
                "q_expiry" => "q",
                "native_halt" => "watch",
                "grant_revocation" => "grant",
                _ => "health", // evidence_failure
            },
        )),
    }
}

/// One schedule: `class` lands at `side` (gate `i` of the R5 sides). A
/// fault that needs a hold the call owns at that point runs on its own
/// thread and is joined at the next point that holds nothing.
fn schedule(
    class: &'static str,
    side: &str,
    i: usize,
) -> (Call, Result<ModelCallOutcome, &'static str>) {
    let f = Call::new();
    if let Some(kind) = class.strip_prefix("mission_") {
        let (c, far) = (f.b.clock.clone(), 1 << 50);
        let (m, w) = match kind {
            "mono" => (c.monotonic_ms() + 1000, far),
            "wall" => (far, c.wall_ms() + 1000),
            _ => (far, far), // Narrowed at the side.
        };
        f.b.door
            .context()
            .narrow_mission_deadlines(c.clone(), m, w)
            .unwrap();
    }
    let keep: Keep = Arc::default();
    let (act, landed, pending) = (
        matrix_fault(&f, class, &keep),
        Arc::new(AtomicBool::new(false)),
        Holder::default(),
    );
    let (point, held) = match side {
        "r3_reserve:before" => ("", false),
        "r3_reserve:after" => ("model.attempt.reserved", false),
        "r3_cas:before" => ("model.attempt.checked", true),
        "r3_cas:after" => ("model.call.polled", false), // The first observation.
        "charge:before" => ("meter.entry", false),
        "charge:after" => ("meter.charged", false),
        "r5_observer:before" => ("model.call.gate", false),
        "r5_sample:after" => ("model.call.sampled", false), // B1f: before the hold
        "r5_cas:before" => ("model.call.observed", true),
        _ => ("model.call.admitted", true),
    };
    let serial = held
        && matches!(
            class,
            "cancel_run"
                | "door_halt"
                | "run_finish"
                | "grant_revocation"
                | "contention"
                | "poison"
        );
    let (word, core) = (
        f.b.session.core.cell.session_word(),
        f.b.session.core.clone(),
    );
    let (done, later) = (landed.clone(), pending.clone());
    let fire: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
        let (act, done) = (act.clone(), done.clone());
        let run = move || {
            act();
            done.store(true, Ordering::SeqCst);
        };
        if !serial {
            return run();
        }
        *later.lock().unwrap() = Some(std::thread::spawn(run));
        if class == "door_halt" {
            // Its cell engagement precedes its run-table part: wait for it.
            let until = Instant::now() + Duration::from_secs(5);
            while core.cell.session_word() == word {
                assert!(Instant::now() < until, "door halt engaged");
                std::thread::yield_now();
            }
        }
    });
    let join = move || {
        if let Some(fault) = pending.lock().unwrap().take() {
            fault.join().unwrap();
        }
    };
    let hooks = &f.b.session.core.cell.1;
    for at in ["model.call.gate", "model.call.polled"] {
        let join = join.clone();
        hooks.every(at, join);
    }
    let (end, kept) = (join.clone(), keep.clone());
    hooks.every("model.attempt.return", move || {
        end();
        release(&kept);
    });
    let (meter, at) = (join.clone(), fire.clone());
    let metered: Box<dyn Fn() + Send> = Box::new(move || {
        meter();
        at();
    });
    match point {
        "" => fire(),
        "meter.entry" => *f.meter.at_entry.lock().unwrap() = Some(metered),
        "meter.charged" => *f.meter.at_charge.lock().unwrap() = Some(metered),
        _ => f.hook(point, i, move || fire()),
    }
    let outcome = f.call();
    join();
    release(&keep);
    assert!(
        landed.load(Ordering::SeqCst),
        "{side}/{class} {i}: the fault landed"
    );
    (f, outcome)
}

/// F10b (SB3B-R16 (c)): each class fault at each probe point. `pre:C`
/// refuses before the ticket; `at:C:S` is judged at that side's own gate
/// (step S, or `poll` for the transport's deny-only guard) with no further
/// admission; `next:C:S` lets at most the selected request overlap and the
/// next gate (or the guard) refuses. An entered chat has no next gate.
fn judge(
    f: &Call,
    (side, class, i): (&str, &str, usize),
    expected: &str,
    outcome: Result<ModelCallOutcome, &str>,
) {
    let what = format!("{side}/{class} gate {i}");
    let mut e = expected.splitn(3, ':');
    let (when, literal, step) = (e.next().unwrap(), e.next().unwrap(), e.next().unwrap_or(""));
    if when == "pre" {
        assert_eq!(
            outcome.map(|_| ()),
            Err(literal),
            "{what}: refused before the ticket"
        );
        let nothing = (f.sent().len(), f.charged(), f.records());
        assert_eq!(
            nothing,
            (0, (0, 0), 0),
            "{what}: no request, charge or record"
        );
        if class != "poison" {
            assert_eq!(f.counts(), (0, 0), "{what}: no attempt or pin");
        }
        return;
    }
    assert!(matches!(when, "at" | "next"), "{what}: {expected}");
    let outcome = outcome.unwrap_or_else(|e| panic!("{what}: refused before the ticket ({e})"));
    let base = match &side[..2] {
        "r3" => 0,
        "ch" => 3,
        _ => i,
    };
    let (entered, gated) = (when == "next", step != "poll");
    let (sent, chat) = (f.sent(), entered && i == 3);
    let admitted = count(f, "admitted");
    assert_eq!(
        admitted,
        base + usize::from(entered),
        "{what}: the next gate refuses"
    );
    let (low, high) = match (entered, gated) {
        (false, _) => (base, base),
        (true, true) => (i + 1, i + 1),
        _ => (i, i + 1),
    };
    let overlap = (low..=high).contains(&sent.len()) && CALL[..sent.len()] == sent[..];
    assert!(
        overlap,
        "{what}: at most one overlapping request ({sent:?})"
    );
    let steps: &[&str] = if gated && !chat { &[step] } else { &[] };
    assert_eq!(refusals(f), steps, "{what}: judged at its step");
    let got = class_of(&outcome);
    let literal = match () {
        () if chat && gated => got == "ok", // The entered chat completes.
        () if sent.len() == 4 => got == literal || got == "model_late",
        () => got == literal,
    };
    assert!(literal, "{what}: the literal class ({got})");
    let (charges, settled) = f.charged();
    let most = u32::from(base == 3 || sent.len() >= 3);
    let exact = entered || charges == most;
    assert!(
        charges == settled && charges <= most && exact,
        "{what}: no new charge"
    );
    if class != "poison" {
        assert_eq!(f.counts().0, 1, "{what}: one attempt");
    }
    if class != "evidence_failure" {
        assert_eq!(f.records(), 1, "{what}: one native record");
    }
    if class == "contention" {
        assert_eq!(
            f.latched(),
            (false, false),
            "{what}: contention latches nothing"
        );
    }
}
fn class_of(outcome: &ModelCallOutcome) -> &'static str {
    class(outcome)
}

#[test]
fn sb3_schedule_matrix_is_judged_exactly() {
    positive("positive control");
    let mut schedules = 0;
    for (side, class, test, expected) in matrix() {
        let known = test == "OPEN:SB3B-B1f" && KNOWN_DEFECT.contains(&&*format!("{side}/{class}"));
        if test != MATRIX && !known {
            continue;
        }
        for i in 0..if side.starts_with("r5") { 4 } else { 1 } {
            let (f, outcome) = schedule(class, side, i);
            judge(&f, (side, class, i), expected, outcome);
            schedules += 1;
        }
    }
    assert_eq!(schedules, 295, "every executed cell ran");
}

/// F10b validator (SB3B-R15 (2), R16 (a), R18): every matrix cell exactly
/// once; each cites an existing test fn, a closed-list N/A (LIFECYCLE names
/// its earlier denial oracle), an IDENTICAL_POINT whose cell cites one, or
/// a B1f tag on exactly the listed cells. Anything else fails.
#[test]
fn sb3_schedule_matrix_cells_are_complete() {
    const NA: [&str; 7] = [
        "BOOTSTRAP_NO_MISSION",
        "PREBIND_NO_RUN_OR_CONSENT",
        "NO_INVOCATION_AT_REGISTRATION",
        "METADATA_HAS_NO_CHARGE",
        "SETTLEMENT_HAS_NO_NEW_WORK_ADMISSION",
        "LIFECYCLE_DENIES_BEFORE_THIS_SINK",
        "LEGACY_FINALIZATION_ONLY",
    ];
    let source = [
        include_str!("model_call_tests.rs"),
        include_str!("model_tests.rs"),
    ]
    .concat();
    let exists = |name: &str| {
        let word = !name.is_empty()
            && name
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
        word && source.contains(&format!("#[test]\nfn {name}()"))
    };
    let rows = matrix();
    let cell = |r: &Row| format!("{}/{}", r.0, r.1);
    let cites = |at: &str| rows.iter().find(|r| cell(r) == at).map(|r| r.2);
    let all = CLASSES.iter().chain(&STATES);
    let mut want: Vec<String> = SIDES
        .iter()
        .flat_map(|s| all.clone().map(move |c| format!("{s}/{c}")))
        .collect();
    want.extend(KNOWN_DEFECT.map(String::from));
    let mut got: Vec<String> = rows.iter().map(cell).collect();
    want.sort();
    got.sort();
    assert_eq!(got, want, "exactly the matrix cells, each once");
    for r in &rows {
        let (at, test) = (cell(r), r.2);
        let ok = if let Some(na) = test.strip_prefix("N/A:") {
            let (reason, oracle) = na
                .strip_suffix(')')
                .and_then(|n| n.split_once('('))
                .unwrap_or((na, ""));
            let named = oracle.is_empty() && reason != NA[5] || exists(oracle);
            NA.contains(&reason) && named
        } else if let Some(other) = test.strip_prefix("IDENTICAL_POINT:") {
            other != at && !r.3.is_empty() && cites(other).is_some_and(exists)
        } else {
            match test {
                "OPEN:SB3B-B1f" => KNOWN_DEFECT.contains(&&*at),
                "DEFERRED:SB3B-B1f" => DEFERRED.contains(&&*at),
                _ => exists(test),
            }
        };
        assert!(ok, "{at}: {test} is no existing oracle or allowed tag");
    }
    for (cells, tag) in [
        (&KNOWN_DEFECT[..], "OPEN:SB3B-B1f"),
        (&DEFERRED[..], "DEFERRED:SB3B-B1f"),
    ] {
        assert!(
            cells.iter().all(|c| cites(c) == Some(tag)),
            "{tag}: the listed cells"
        );
    }
    assert!(exists(MATRIX) && !exists("sb3_no_such_oracle"), "controls");
}

/// SB3B-R18 (B1f): a Q or mission deadline found lapsed under the hold is
/// an expiry, `model_timeout`, at step 1 (after the pre-hold check, before
/// the hold) and at the CAS (step 8). A narrowed mission deadline is judged
/// there from the pre-hold sample; a Q deadline passing after the sample is
/// invisible to it (the seam is sample-faithful) and ends at the next poll.
#[test]
fn sb3_deadline_lapsed_under_the_hold_times_out() {
    positive("positive control");
    for (side, step) in [
        ("r5_cas:before", "refused:8"),
        ("r5_sample:after", "refused:1"),
    ] {
        for class in ["mission_wall_narrowed", "mission_mono_narrowed", "q_expiry"] {
            let expected = match class {
                "q_expiry" => "next:model_timeout:poll".to_owned(),
                _ => format!("at:model_timeout:{step}"),
            };
            for i in 0..4 {
                let (f, outcome) = schedule(class, side, i);
                judge(&f, (side, class, i), &expected, outcome);
            }
        }
    }
}
