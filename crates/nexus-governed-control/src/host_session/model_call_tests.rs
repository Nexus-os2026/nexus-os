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
impl crate::authority::clock::Clock for SplitClock {
    fn monotonic_ms(&self) -> u64 {
        self.mono.load(Ordering::SeqCst)
    }
    fn wall_ms(&self) -> u64 {
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
        let peer = register.then(|| Peer::new(None, None));
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
