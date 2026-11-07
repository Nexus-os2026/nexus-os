//! SB-3 C1 (SB3C-R1): the public session model route, driven only through
//! the four public surfaces (`register_local_model`, `model_identity`,
//! `admit_inference`, `model_call`). Counters are read through the existing
//! crate-private test accessors. No thread is joined here.
use super::model_tests::{nth, pin, Accounting, Peer};
use super::*;
use crate::authority::approval::{
    AcknowledgementConfirmation, ActionConfirmation, AgentDoorConfirmation, ControlConfirmer,
    GrantConfirmation, MissionConfirmation, ResumeConfirmation,
};
use crate::authority::clock::ManualClock;
use crate::authority::evidence::EndStatus;
use crate::authority::ids::RunId;
use crate::inference::{
    CallDeadline, CallMeter, Charged, InvocationOwner, MeterRefusal, ModelCallOutcome,
    ModelIdentityView, ModelRequest, ResponseSchema,
};
use std::sync::atomic::{AtomicBool, AtomicU32};

const HASH: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const HASH2: &str = "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";
const LABEL: &str = "msn-000001/1/1";
const LABEL2: &str = "msn-000001/1/2";
const REGISTER: [&str; 4] = ["/api/version", "/api/status", "/api/tags", "/api/show"];
const CALL: [&str; 4] = ["/api/version", "/api/status", "/api/tags", "/api/chat"];

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
#[derive(Default)]
struct Meter(AtomicU32);
impl CallMeter for Meter {
    fn charge(&self) -> Result<Charged, MeterRefusal> {
        Ok(Charged {
            call: self.0.fetch_add(1, Ordering::SeqCst) + 1,
        })
    }
    fn settle(&self, _: u32) {}
}
struct Open;
impl CallDeadline for Open {
    fn expired(&self) -> bool {
        false
    }
}
fn request() -> ModelRequest {
    let schema = ResponseSchema::new(r#"{"type":"object"}"#).unwrap();
    ModelRequest::new("SB3C system".into(), "SB3C prompt".into(), schema).unwrap()
}

/// A Bootstrap session, optionally registered through the public R0 on its
/// own fake peer, with Q, its original door D, cached consent and the
/// existing File run R.
struct Pub {
    session: Arc<HostSession>,
    clock: Arc<ManualClock>,
    q: SessionRequest,
    door: SessionDoor,
    run: RunId,
    view: Option<ModelIdentityView>,
    identity: [u8; 32],
    primary: Arc<crate::evidence_store::PrimaryEvidence>,
    peer: Option<Peer>,
    owner: InvocationOwner,
    _keep: (
        crate::authority::file_session::FileAttachment,
        [crate::harness_tests::TempRoot; 2],
    ),
}
impl Pub {
    fn new() -> Self {
        Self::with(true, true)
    }
    fn with(register: bool, files: bool) -> Self {
        assert!(crate::files::make_non_dumpable());
        let root = crate::harness_tests::temp_root("sb3c-public");
        let selected = crate::harness_tests::temp_root("sb3c-workspace");
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
        let peer = register.then(|| Peer::chat(None, None));
        let view = peer
            .as_ref()
            .map(|p| session.register_local_model(&p.endpoint, pin()).unwrap());
        let identity = crate::inference::ModelIdentity::from_spec(&pin())
            .unwrap()
            .digest();
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
            view,
            identity,
            primary,
            peer,
            owner: InvocationOwner::new(),
            _keep: (attachment, [root, selected]),
        }
    }
    fn paths(&self) -> Vec<String> {
        self.peer
            .as_ref()
            .map_or_else(Vec::new, |p| p.paths.lock().unwrap().clone())
    }
    fn records(&self, phase: &str) -> usize {
        let rows = self.primary.store().rows();
        rows.iter().filter(|r| r.phase == phase).count()
    }
    fn accounting(&self) -> Option<(u32, usize)> {
        let control = self.session.control_for_test();
        let runs = control.authority().runs().clone();
        runs.model_accounting(self.run)
    }
    fn bound(&self) -> bool {
        let control = self.session.control_for_test();
        let runs = control.authority().runs().clone();
        runs.inference_admission(self.run).is_some()
    }
    fn admit(&self, door: &SessionDoor) -> Result<(), &'static str> {
        door.admit_inference(self.run, HASH, LABEL, self.identity)
            .map_err(|e| e.class())
    }
    fn call_with(
        &self,
        door: &SessionDoor,
        request: &ModelRequest,
        meter: &Meter,
    ) -> Result<ModelCallOutcome, &'static str> {
        let observer = self.owner.observer();
        door.model_call(self.run, request, meter, &Open, &observer)
            .map_err(|e| e.class())
    }
    fn call(&self, meter: &Meter) -> Result<ModelCallOutcome, &'static str> {
        self.call_with(&self.door, &request(), meter)
    }
    /// The class a call ended with: its refusal, or its transport result.
    fn class(&self, meter: &Meter) -> &'static str {
        match self.call(meter) {
            Err(class) => class,
            Ok(outcome) => outcome
                .result()
                .as_ref()
                .map_or_else(|e| e.class(), |_| "ok"),
        }
    }
    /// Zero model work: no attempt, pin, record, charge or call request.
    fn untouched(&self, case: &str, meter: &Meter, before: usize) {
        assert_eq!(self.accounting(), Some((0, 0)), "{case}: attempts/pins");
        assert_eq!(self.records("model_call"), 0, "{case}: native records");
        assert_eq!(meter.0.load(Ordering::SeqCst), 0, "{case}: charges");
        assert_eq!(self.paths().len(), before, "{case}: call requests");
    }
}

/// S1 (public route): Bootstrap registration's four requests, then one
/// recorded call's four; one attempt, charge and record; R unchanged.
#[test]
fn sb3c_public_route_executes_one_recorded_call() {
    let p = Pub::new();
    assert_eq!(p.paths(), REGISTER, "registration requests");
    assert_eq!(p.records("model_registered"), 1);
    assert_eq!(
        p.door.model_identity(),
        p.view,
        "R1 returns the registered view"
    );
    assert_eq!(p.admit(&p.door), Ok(()), "R2 binds the original run");
    let meter = Meter::default();
    let outcome = p.call(&meter).expect("an admitted attempt");
    let reply = outcome.result().as_ref().expect("the fake reply");
    assert!(reply.content().contains("sb3b"), "the exact fake reply");
    assert!(matches!(outcome.record(), EndStatus::Recorded { .. }));
    assert_eq!(p.paths()[4..], CALL, "call requests");
    assert_eq!(p.accounting(), Some((1, 0)), "one attempt, pin settled");
    assert_eq!(p.records("model_call"), 1, "one native record");
    assert_eq!(meter.0.load(Ordering::SeqCst), 1, "one charge");
    assert_eq!(p.session.run_count(), 1, "the original File run only");
}

/// S2 (and MP-SB-1b): an unregistered session keeps its default denial;
/// the absent-registration accounting case stays unreachable from it.
#[test]
fn sb3c_unconfigured_session_retains_default_denial() {
    let p = Pub::with(false, true);
    let meter = Meter::default();
    let identity = p.door.model_identity();
    let admitted = p.admit(&p.door);
    let called = p.call(&meter).map(|_| ());
    // MP-SB-1b's named assertion: judged before the route's classes, so a
    // deferred precondition that binds and acquires dies here.
    assert_eq!(p.accounting(), Some((0, 0)), "S2: model_attempts == 0");
    assert_eq!(identity, None);
    assert_eq!(admitted, Err("model_not_registered"));
    assert_eq!(called, Err("model_not_registered"));
    p.untouched("S2", &meter, 0);
    assert!(!p.bound(), "no binding");
    let slot = p.session.close_obligations().unwrap().model;
    assert!(!slot.active, "the model slot never became active");
    assert_eq!(p.session.run_count(), 1, "no new run");
    // Reachability: with every other prerequisite valid, the accounting
    // itself issues a ticket for an absent registration; only the route's
    // registration precondition keeps S2's `model_attempts == 0`.
    let root = crate::harness_tests::temp_root("sb3c-reach");
    let sink = crate::evidence_store::PrimaryEvidence::open(
        &root.0.path().join("p3-evidence/evidence.sqlite3"),
        vec![],
    )
    .unwrap();
    let reach = Accounting::new(sink);
    assert!(
        reach.acquire().is_ok(),
        "absent-registration accounting is reachable"
    );
}

/// R0: registration only in Bootstrap, recorded once before readiness; a
/// second registration and a late one send and record nothing.
#[test]
fn sb3c_registration_is_bootstrap_only_once() {
    let f = super::model_tests::fixture();
    let peer = Peer::chat(None, None);
    let models = |f: &super::model_tests::Fixture| {
        let rows = f.primary.store().rows();
        rows.iter()
            .filter(|r| r.phase == "model_registered")
            .count()
    };
    assert_eq!(models(&f), 0);
    assert!(f
        .session
        .register_local_model(&peer.endpoint, pin())
        .is_ok());
    assert_eq!(models(&f), 1, "one record before readiness");
    assert_eq!(*peer.paths.lock().unwrap(), REGISTER);
    let second = f.session.register_local_model(&peer.endpoint, pin());
    // Judged before the refusal: a re-entered registration dies here.
    assert_eq!(*peer.paths.lock().unwrap(), REGISTER, "second: no request");
    assert_eq!(models(&f), 1, "second: no record");
    assert!(second.is_err(), "second: refused");
    assert!(
        f.session.finish_bootstrap().is_ok(),
        "installed model: ready"
    );
    // Late: after readiness nothing registers.
    let late = super::model_tests::fixture();
    late.session.finish_bootstrap().unwrap();
    let other = Peer::chat(None, None);
    let refused = late.session.register_local_model(&other.endpoint, pin());
    assert!(other.paths.lock().unwrap().is_empty(), "late: no request");
    assert_eq!(models(&late), 0, "late: no record");
    assert!(refused.is_err(), "late: refused");
    // A failed registration cannot produce ready.
    let failed = super::model_tests::fixture();
    let bad = Peer::chat(None, Some(1));
    assert!(failed
        .session
        .register_local_model(&bad.endpoint, pin())
        .is_err());
    assert!(
        failed.session.finish_bootstrap().is_err(),
        "failed: not ready"
    );
}

/// R1: the identity view is read-only and only at a live original door.
#[test]
fn sb3c_identity_is_original_request_read_only() {
    let p = Pub::new();
    let before = (p.paths(), p.records("model_call"), p.accounting());
    for _ in 0..3 {
        assert_eq!(p.door.model_identity(), p.view);
    }
    let after = (p.paths(), p.records("model_call"), p.accounting());
    assert_eq!(before, after, "reads add no request, record or attempt");
    let q2 = p.session.ingress().accept(CommandKind::Run, 2).unwrap();
    let continuation = p.session.activate_for_test(&q2).unwrap();
    assert_eq!(continuation.model_identity(), None, "Continuation");
    let expired = Pub::new();
    expired
        .session
        .expire_request_for_test(expired.q.sequence());
    assert_eq!(expired.door.model_identity(), None, "expired Q");
    let restricted = Pub::new();
    restricted.session.ingress().emergency().unwrap();
    assert_eq!(restricted.door.model_identity(), None, "restriction");
    let old = Pub::new();
    let control = old.session.control_for_test();
    assert!(control.kernel_stop_authority().engage() > 0);
    control.resume_with_halt(&Owner, None).unwrap();
    assert_eq!(old.door.model_identity(), None, "the old watch");
    let poisoned = Pub::new();
    let core = poisoned.session.core.clone();
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _held = core.model.state.lock().unwrap();
        panic!("poison the model slot");
    }));
    assert_eq!(poisoned.door.model_identity(), None, "poison");
    assert_eq!(
        Pub::with(false, true).door.model_identity(),
        None,
        "missing M"
    );
}

/// The private A oracle that varies the consent association fields with
/// its test-only tamper seam (SB3C-R1 Q3): cited, never re-seamed.
const CITED: &str = include_str!("model_call_tests.rs");
const TAMPERED: [&str; 8] = [
    "groups",
    "opened_by",
    "door_cell",
    "view_id",
    "seq",
    "kind",
    "time",
    "life",
];

/// R2 and the F12 isolated refusals through the public route, with a
/// route column (SB3C-R1 Q4): binding → `halted`, call gate →
/// `model_timeout` for an expired Q.
#[test]
fn sb3c_public_binding_is_exact_and_idempotent() {
    let p = Pub::new();
    assert_eq!(p.admit(&p.door), Ok(()));
    assert_eq!(
        p.admit(&p.door),
        Ok(()),
        "identical live binding is idempotent"
    );
    let meter = Meter::default();
    p.untouched("idempotent", &meter, 4);
    for (hash, label) in [(HASH2, LABEL), (HASH, LABEL2)] {
        p.door.confirm_mission(&mission(hash, label)).unwrap();
        let changed = p.door.admit_inference(p.run, hash, label, p.identity);
        assert_eq!(changed.map_err(|e| e.class()), Err("already_admitted"));
    }
    // Binding route, one real component varied at a time.
    for (case, class) in [
        ("s", "unknown_run"),
        ("q", "not_member"),
        ("d", "not_member"),
        ("unknown_run", "unknown_run"),
        ("files", "not_admitted"),
        ("consent", "no_consent"),
        ("identity", "model_identity_mismatch"),
        ("grant", "grant_not_live"),
        ("expired_q", "halted"),
        ("continuation", "halted"),
        ("unregistered", "model_not_registered"),
    ] {
        let f = Pub::with(case != "unregistered", case != "files");
        // SB3C-R4 (2): observed before the operation, compared after; the
        // second session (S) is the receiving fixture.
        let other = (case == "s").then(Pub::new);
        let seen = |p: &Pub| (p.paths().len(), p.records("model_call"), p.accounting());
        let (before, other_before) = (f.paths().len(), other.as_ref().map(seen));
        let got = match case {
            "s" => (other.as_ref().unwrap().door).admit_inference(f.run, HASH, LABEL, f.identity),
            "q" => {
                let q2 = f.session.ingress().accept(CommandKind::Run, 2).unwrap();
                let door = f.session.open_file_door(&q2).unwrap();
                door.admit_inference(f.run, HASH, LABEL, f.identity)
            }
            // SB3C-R4 (1), real objects: another door issued for the same Q.
            "d" => {
                let d2 = f.session.open_file_door(&f.q).unwrap();
                d2.admit_inference(f.run, HASH, LABEL, f.identity)
            }
            "unknown_run" => f
                .door
                .admit_inference(RunId::fresh(), HASH, LABEL, f.identity),
            "consent" => f.door.admit_inference(f.run, HASH, LABEL2, f.identity),
            "identity" => f.door.admit_inference(f.run, HASH, LABEL, [7; 32]),
            "grant" => {
                f.clock.advance(Duration::from_secs(3600));
                f.door.admit_inference(f.run, HASH, LABEL, f.identity)
            }
            "expired_q" => {
                f.session.expire_request_for_test(f.q.sequence());
                f.door.admit_inference(f.run, HASH, LABEL, f.identity)
            }
            "continuation" => {
                let q2 = f.session.ingress().accept(CommandKind::Run, 2).unwrap();
                let door = f.session.activate_for_test(&q2).unwrap();
                door.admit_inference(f.run, HASH, LABEL, f.identity)
            }
            _ => f.door.admit_inference(f.run, HASH, LABEL, f.identity),
        };
        assert_eq!(got.map_err(|e| e.class()), Err(class), "binding {case}");
        assert!(!f.bound(), "binding {case}: nothing bound");
        f.untouched(case, &Meter::default(), before);
        if let Some(other) = &other {
            let after = Some(seen(other));
            assert_eq!(after, other_before, "binding s: the receiving session");
            assert!(!other.bound(), "binding s: the receiver binds nothing");
        }
    }
    // SB3C-R4 (1): an R opened by another request cannot exist while R is
    // attached (one File port per session), so `opened_by` stays cited.
    let f = Pub::new();
    let q2 = f.session.ingress().accept(CommandKind::Run, 2).unwrap();
    let other = f.session.open_file_door(&q2).unwrap();
    assert!(
        other.file_port().is_err(),
        "no second File port: no foreign R"
    );
    // The consent-association fields: cited to A's private tamper oracle.
    assert!(CITED.contains("fn sb3_inference_binding_is_exact_and_idempotent()"));
    for case in TAMPERED {
        let row = format!("(\"{case}\", \"");
        assert!(CITED.contains(&row), "cited private row {case}");
    }
    // Call-gate route.
    for (case, class) in [
        ("expired_q", "model_timeout"),
        ("conversation", "model_config_mismatch"),
        ("continuation", "halted"),
    ] {
        let f = Pub::new();
        assert_eq!(f.admit(&f.door), Ok(()));
        let meter = Meter::default();
        let got = match case {
            "expired_q" => {
                let (session, q) = (f.session.clone(), f.q.sequence());
                nth(&f.session, "model.call.gate", 0, move || {
                    session.expire_request_for_test(q)
                });
                f.class(&meter)
            }
            "conversation" => {
                let conversation = nexus_kernel::inference::Conversation::new(
                    "SB3C system".into(),
                    "SB3C task".into(),
                )
                .unwrap();
                let schema = ResponseSchema::new(r#"{"type":"object"}"#).unwrap();
                let request = ModelRequest::conversation(&conversation, schema).unwrap();
                f.call_with(&f.door, &request, &meter)
                    .map(|_| "ok")
                    .unwrap_or_else(|c| c)
            }
            _ => {
                let q2 = f.session.ingress().accept(CommandKind::Run, 2).unwrap();
                let door = f.session.activate_for_test(&q2).unwrap();
                f.call_with(&door, &request(), &meter)
                    .map(|_| "ok")
                    .unwrap_or_else(|c| c)
            }
        };
        assert_eq!(got, class, "call gate {case}");
        assert_eq!(
            meter.0.load(Ordering::SeqCst),
            0,
            "call gate {case}: no charge"
        );
        assert!(
            !f.paths()[4..].contains(&"/api/chat".to_string()),
            "{case}: no chat"
        );
    }
    // The ninth attempt on one run refuses before any request.
    let f = Pub::new();
    assert_eq!(f.admit(&f.door), Ok(()));
    let meter = Meter::default();
    for n in 1..=8 {
        assert_eq!(f.class(&meter), "ok", "attempt {n}");
    }
    let sent = f.paths().len();
    assert_eq!(
        f.call(&meter).map(|_| ()),
        Err("model_over_budget"),
        "ninth"
    );
    assert_eq!(f.paths().len(), sent, "ninth: no request");
    assert_eq!(meter.0.load(Ordering::SeqCst), 8, "ninth: no charge");
}

/// R3: an ingress at the actual binding CAS wins; nothing is installed.
#[test]
fn sb3c_stop_before_public_binding_installs_nothing() {
    let p = Pub::new();
    let (ingress, reached) = (p.session.ingress(), Arc::new(AtomicBool::new(false)));
    let seen = reached.clone();
    p.session.core.cell.1.arm("model.bind.checked", move || {
        seen.store(true, Ordering::SeqCst);
        ingress.emergency().unwrap();
    });
    assert_eq!(
        p.admit(&p.door),
        Err("halted"),
        "ingress wins the original CAS"
    );
    assert!(reached.load(Ordering::SeqCst), "paused at the actual CAS");
    assert!(!p.bound(), "nothing installed");
}

/// R4 and G0: Continuation and conversation never call; the original File
/// grant stays required at the binding and at the call.
#[test]
fn sb3c_public_continuation_conversation_and_grant() {
    let p = Pub::new();
    let q2 = p.session.ingress().accept(CommandKind::Run, 2).unwrap();
    let continuation = p.session.activate_for_test(&q2).unwrap();
    let meter = Meter::default();
    assert_eq!(continuation.model_identity(), None);
    assert_eq!(p.admit(&continuation), Err("halted"));
    let called = p.call_with(&continuation, &request(), &meter).map(|_| ());
    assert_eq!(called, Err("halted"));
    p.untouched("Continuation", &meter, 4);
    // G0 at the call: the grant lapses after binding; no request is sent.
    let g = Pub::new();
    assert_eq!(g.admit(&g.door), Ok(()));
    g.clock.advance(Duration::from_secs(3600));
    let meter = Meter::default();
    assert_eq!(g.class(&meter), "grant_not_live", "G0 call");
    g.untouched("G0 call", &meter, 4);
    // Positive: the valid original grant reaches chat.
    let ok = Pub::new();
    assert_eq!(ok.admit(&ok.door), Ok(()));
    assert_eq!(
        ok.class(&Meter::default()),
        "ok",
        "valid grant reaches chat"
    );
}

/// SB3C-R4 (3): public G0 at each of the four call gates: the original
/// grant revoked or expired there, or another live grant present, refuses
/// that request and every successor; the attempt is still recorded.
#[test]
fn sb3c_public_grant_is_required_at_every_call_gate() {
    for case in ["revoked", "expired", "substitute"] {
        for i in 0..4 {
            let f = Pub::new();
            assert_eq!(f.admit(&f.door), Ok(()));
            let DoorKind::Original { file_grant } = f.door.kind else {
                unreachable!("an original door")
            };
            let (control, clock) = (f.session.control_for_test(), f.clock.clone());
            if case == "substitute" {
                let other = f.session.grant_for_test(&f.q).unwrap();
                let live = control.authority().grants().live(other).is_some();
                assert!(live, "another live grant");
            }
            nth(&f.session, "model.call.gate", i, move || match case {
                "expired" => clock.advance(Duration::from_secs(3600)),
                _ => control.authority().grants().revoke(file_grant).unwrap(),
            });
            let before = f.paths().len();
            assert_eq!(
                f.class(&Meter::default()),
                "model_cancelled",
                "G0 {i} {case}"
            );
            let sent = f.paths()[before..].to_vec();
            assert_eq!(sent, CALL[..i], "G0 gate {i} {case}: it and successors 0");
            assert_eq!(f.records("model_call"), 1, "G0 gate {i} {case}: recorded");
        }
    }
}
