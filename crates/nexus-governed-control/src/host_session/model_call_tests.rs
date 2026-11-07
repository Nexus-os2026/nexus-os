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
        let clock = Arc::new(ManualClock::default());
        let session = Arc::new(
            HostSession::open(
                SessionSeed::new().unwrap(),
                root.0.path(),
                primary,
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
