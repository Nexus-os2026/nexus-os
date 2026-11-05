//! Decision 10: exact review-4 schedules and the wider Owner/lookup sweep.
//! These baseline shims use the old routes on 6bfc637a. The inherent optional
//! cell APIs replace them by ordinary Rust method resolution after the fix;
//! the schedules, effect observations and assertions stay unchanged.
use super::*;
use crate::host_session::{CommandKind, HostSession, SessionDoor, SessionSeed};

struct HostFx {
    gov: Arc<GovernedControl>,
    session: Arc<HostSession>,
    door: Arc<SessionDoor>,
    attachment: Arc<crate::authority::file_session::FileAttachment>,
    owner: Arc<Owner>,
    evidence: Arc<MemoryEvidence>,
    run: RunId,
    ws: PathBuf,
    sequence: AtomicUsize,
    _root: TempRoot,
    _dirs: Dirs,
}
impl HostFx {
    fn new() -> Self {
        assert!(make_non_dumpable());
        let root = temp_root("session-d10");
        let dirs = Dirs::new();
        let ws = dirs.0.join("home/ws");
        let owner = Owner::new();
        let evidence = Arc::new(MemoryEvidence::new(4096));
        let primary = crate::evidence_store::PrimaryEvidence::open(
            &root.0.path().join("p3-evidence/evidence.sqlite3"),
            vec![evidence.clone()],
        )
        .unwrap();
        let session = Arc::new(
            HostSession::open(
                SessionSeed::new().unwrap(),
                root.0.path(),
                primary,
                Arc::new(ManualClock::default()),
                owner.clone(),
            )
            .unwrap(),
        );
        session.bootstrap_workspace(&ws, &[]).unwrap();
        session.finish_bootstrap().unwrap();
        let request = session.ingress().accept(CommandKind::Run, 1).unwrap();
        let door = Arc::new(session.open_file_door(&request).unwrap());
        let attachment = Arc::new(door.file_port().unwrap().attach().unwrap());
        door.claim_file_session(&attachment).unwrap();
        door.confirm_mission(&MissionConfirmation {
            contract_hash: HASH.into(),
            request: LABEL.into(),
            summary: vec!["One file".into()],
            page_facts: None,
        })
        .unwrap();
        let run = door.open_file_run(&attachment).unwrap();
        door.admit_files(run, HASH, LABEL).unwrap();
        Self {
            gov: session.control_for_test(),
            session,
            door,
            attachment,
            owner,
            evidence,
            run,
            ws,
            sequence: AtomicUsize::new(1),
            _root: root,
            _dirs: dirs,
        }
    }
    fn request(
        &self,
        kind: CommandKind,
    ) -> Result<crate::host_session::SessionRequest, AuthorityError> {
        let seq = self.sequence.fetch_add(1, Ordering::SeqCst) as u64 + 1;
        self.session
            .ingress()
            .accept(kind, seq)
            .map_err(|error| match error {
                crate::host_session::SessionError::Unavailable => {
                    AuthorityError::Unavailable("host lift is unavailable")
                }
                _ => AuthorityError::Halted,
            })
    }
    fn propose(&self, name: &str, content: &str) -> Result<CommitmentView, AuthorityError> {
        match self
            .door
            .create_file(
                self.run,
                &FileCreate {
                    name: name.into(),
                    content: content.into(),
                },
            )
            .map_err(AuthorityError::from)?
        {
            AgentOutcome::AwaitingApproval(view) => Ok(view),
            _ => panic!("separate approval required"),
        }
    }
    fn ready(&self, name: &str, content: &str) -> CommitmentView {
        self.propose(name, content).unwrap()
    }
    fn execute(&self, view: &CommitmentView) -> Result<EffectOutput, AuthorityError> {
        assert_eq!(self.session.pending_for_test(), Some(view.id));
        self.session
            .approve_file(&self.request(CommandKind::Approve)?)
            .map_err(Into::into)
    }
    fn undo(&self) -> Result<crate::files::UndoReport, AuthorityError> {
        self.session
            .undo(&self.request(CommandKind::Undo)?)
            .map_err(Into::into)
    }
    fn cleanup(&self) -> Result<crate::files::CleanupRetry, AuthorityError> {
        self.session
            .cleanup(&self.request(CommandKind::Cleanup)?)
            .map_err(Into::into)
    }
    fn lift(&self) -> Result<(), AuthorityError> {
        self.request(CommandKind::Lift).map(|_| ())
    }
    fn state(&self) -> PathBuf {
        self.gov.files().state_dir_for_test()
    }
    fn staged(&self) -> Vec<String> {
        let path = self.state().join("coding-runs");
        if path.exists() {
            entries(&path)
        } else {
            vec![]
        }
    }
}

fn published_recovery() -> super::Fx {
    let f = super::Fx::new();
    let a = f.ready(NAME, CONTENT);
    f.execute(&a).unwrap();
    f.gov.emergency_stop();
    f.gov
        .resume_with_halt(&*f.owner, Some(&f.gov.file_session_halt()))
        .unwrap();
    f
}

fn published_normal() -> HostFx {
    let f = HostFx::new();
    let a = f.ready(NAME, CONTENT);
    f.execute(&a).unwrap();
    f
}

fn undo_ingress(point: &'static str) {
    let f = published_normal();
    let cell = f.gov.file_session_halt();
    let gate = Gate::new();
    let held = gate.clone();
    f.gov.files().hooks().arm(point, move || held.hold());
    let session = f.session.clone();
    let request = f.request(CommandKind::Undo).unwrap();
    let worker = std::thread::spawn(move || session.undo(&request).map_err(AuthorityError::from));
    gate.reached();
    cell.engage_emergency(); // first emergency, NO Stop worker
    assert!(!f.gov.stop_snapshot().broker_stopped);
    assert!(!f.gov.stop_snapshot().kernel_stopped);
    gate.open();
    let result = worker.join().unwrap();
    let exists = f.ws.join(NAME).exists();
    eprintln!("{point}: result={result:?}, file_exists={exists}");
    assert!(
        exists,
        "{point}: post-confirmation ingress must not remove the file"
    );
    assert!(
        matches!(result, Err(AuthorityError::Halted)),
        "typed stale ingress refusal"
    );
    assert!(!f.gov.files().write_grant_live_for_test());
    // Host amendment: no fresh restore after the first emergency.
    let asks = f.owner.undos().len();
    assert!(matches!(f.undo(), Err(AuthorityError::Halted)));
    assert!(f.ws.join(NAME).exists());
    assert_eq!(f.owner.undos().len(), asks);
}

#[test]
fn decision10_review4_undo_approved_then_ingress_no_worker() {
    undo_ingress(at::FILE_UNDO_APPROVED);
}

#[test]
fn decision10_undo_final_dispatch_then_ingress_no_worker() {
    undo_ingress(at::FILE_UNDO_RESTORE);
}

#[test]
fn decision10_normal_undo_dialogue_emergency_forbids_new_request() {
    let f = published_normal();
    let cell = f.gov.file_session_halt();
    let ingress = cell.clone();
    f.owner.during_undo(move || ingress.engage_emergency());
    let result = f.undo();
    assert!(
        f.ws.join(NAME).exists(),
        "dialogue ingress must not remove the file"
    );
    assert!(matches!(result, Err(AuthorityError::Halted)));
    let asks = f.owner.undos().len();
    assert!(matches!(f.undo(), Err(AuthorityError::Halted)));
    assert!(f.ws.join(NAME).exists());
    assert_eq!(f.owner.undos().len(), asks);
}

#[test]
fn decision10_cleanup_ingress_before_effect_preserves_obligation() {
    use std::os::unix::fs::PermissionsExt;
    let f = HostFx::new();
    f.propose(NAME, CONTENT).unwrap();
    let staging = f.state().join("coding-runs").join(&f.staged()[0]);
    let _ = f.door.cancel_run(f.run).unwrap().write();
    std::fs::set_permissions(&staging, std::fs::Permissions::from_mode(0o500)).unwrap();
    f.gov.settle();
    assert_eq!(f.gov.files().kept_for_test(), 1);
    std::fs::set_permissions(&staging, std::fs::Permissions::from_mode(0o700)).unwrap();
    let cell = f.gov.file_session_halt();
    let ingress = cell.clone();
    f.gov
        .files()
        .hooks()
        .arm("files.retry_before_effect", move || {
            ingress.engage_emergency()
        });
    let result = f.cleanup();
    assert!(
        staging.exists(),
        "cleanup ingress must preserve the owed staging"
    );
    assert!(matches!(result, Err(AuthorityError::Halted)));
    assert_eq!(f.gov.files().kept_for_test(), 1);
    assert!(matches!(f.cleanup(), Err(AuthorityError::Halted)));
    assert!(staging.exists());
    assert_eq!(f.gov.files().kept_for_test(), 1);
}

#[test]
fn decision10_host_lift_is_unavailable_before_any_native_record() {
    for emergency in [false, true] {
        let f = HostFx::new();
        if emergency {
            f.gov.emergency_stop();
        }
        let before = f.evidence.records().len();
        let result = f.lift();
        assert!(matches!(result, Err(AuthorityError::Unavailable(_))));
        assert_eq!(
            f.evidence.records().len(),
            before,
            "unsupported lift records no native resume"
        );
    }
}

fn selected() -> HostFx {
    HostFx::new()
}

#[test]
fn decision10_review4_prune_between_admission_and_commitment() {
    let f = selected();
    let runs = f.gov.authority().runs();
    runs.set_capacity_for_test(runs.len_for_test());
    let gate = Gate::new();
    let held = gate.clone();
    f.gov
        .files()
        .hooks()
        .arm("files.before_stage", move || held.hold());
    let staged = Arc::new(AtomicUsize::new(0));
    let seen = staged.clone();
    f.gov.files().hooks().every(at::FILE_STAGED, move || {
        seen.fetch_add(1, Ordering::SeqCst);
    });
    let (door, run) = (f.door.clone(), f.run);
    let worker = std::thread::spawn(move || {
        door.create_file(
            run,
            &FileCreate {
                name: NAME.into(),
                content: CONTENT.into(),
            },
        )
    });
    gate.reached();
    let _ = f.door.cancel_run(f.run).unwrap().write();
    let legacy = f.gov.issue_agent_door(mother(), f.owner.clone()).unwrap();
    let inserted = legacy.open_run(); // capacity pressure tries to prune
    let retained = runs.view(f.run).is_some();
    f.gov.file_session_halt().engage_emergency();
    assert!(!f.gov.stop_snapshot().broker_stopped);
    assert!(!f.gov.stop_snapshot().kernel_stopped);
    gate.open();
    let result = worker.join().unwrap();
    let staged_count = staged.load(Ordering::SeqCst);
    eprintln!("files.before_stage: retained={retained}, legacy={inserted:?}, result={result:?}, staged={staged_count}");
    assert_eq!(
        staged_count, 0,
        "pruned/missing session run must never stage after ingress"
    );
    assert!(result.is_err());
    assert_eq!(f.gov.files().kept_for_test(), 0);
    assert_eq!(f.gov.files().deferred_for_test(), 0);
    assert!(f.staged().is_empty());
}

#[test]
fn decision10_unknown_run_cell_and_stage_fail_closed() {
    let f = HostFx::new();
    let unknown = RunId::fresh();
    assert!(
        matches!(
            f.gov.authority().runs().session_cell(unknown),
            Err(AuthorityError::Halted)
        ),
        "missing cell lookup must fail closed"
    );
    assert!(
        matches!(
            f.gov.authority().runs().admit_file_stage(unknown),
            Err(AuthorityError::Halted)
        ),
        "missing stage lookup must fail closed"
    );
    assert!(f.gov.authority().runs().session_halted(unknown));
}

#[test]
fn decision10_legacy_cell_less_recovery_control() {
    let f = published_recovery();
    f.gov.files_undo(&*f.owner).unwrap();
    assert!(!f.ws.join(NAME).exists());
    assert_eq!(f.gov.files_retry_cleanup().unwrap().still_owed, 0);
}

// Decision 11: hooks are at the broker/kernel handoff, not inside kernel U1.
#[test]
fn decision11_undo_ingress_after_last_broker_check_reports_actual_removal() {
    let f = published_normal();
    let cell = f.gov.file_session_halt();
    let ingress = cell.clone();
    f.gov
        .files()
        .hooks()
        .arm("files.undo_kernel_boundary", move || {
            ingress.engage_emergency()
        });
    let result = f.undo().unwrap();
    assert!(
        !f.ws.join(NAME).exists(),
        "already confirmed kernel handoff may finish"
    );
    assert_eq!(
        result.interrupted,
        Some("emergency_stop"),
        "completed undo must report ingress"
    );
    f.gov.emergency_stop();
    assert!(f
        .evidence
        .records()
        .iter()
        .any(|r| r.phase == EvidencePhase::EmergencyStop));
    assert!(cell.is_engaged());
    assert!(f.door.open_file_run(&f.attachment).is_err());
}

#[test]
fn decision11_cleanup_ingress_after_handoff_preserves_actual_outcome() {
    use std::os::unix::fs::PermissionsExt;
    let f = HostFx::new();
    f.propose(NAME, CONTENT).unwrap();
    let staging = f.state().join("coding-runs").join(&f.staged()[0]);
    let _ = f.door.cancel_run(f.run).unwrap().write();
    std::fs::set_permissions(&staging, std::fs::Permissions::from_mode(0o500)).unwrap();
    f.gov.settle();
    assert_eq!(f.gov.files().kept_for_test(), 1);
    std::fs::set_permissions(&staging, std::fs::Permissions::from_mode(0o700)).unwrap();
    let cell = f.gov.file_session_halt();
    let ingress = cell.clone();
    f.gov
        .files()
        .hooks()
        .arm(at::FILE_DISCARD, move || ingress.engage_emergency());
    let retry = f.cleanup().unwrap();
    assert_eq!((retry.settled, retry.still_owed), (1, 0));
    assert!(!staging.exists());
    f.gov.emergency_stop();
    assert!(f
        .evidence
        .records()
        .iter()
        .any(|r| r.phase == EvidencePhase::EmergencyStop));
    assert!(f.door.open_file_run(&f.attachment).is_err());
}

#[test]
fn decision11_host_lift_cannot_reach_a_kernel_handoff() {
    for point in ["resume.kernel_boundary", "resume.kernel_returned"] {
        let f = HostFx::new();
        f.gov.emergency_stop();
        let reached = Arc::new(AtomicUsize::new(0));
        let seen = reached.clone();
        f.gov.authority().hooks().arm(point, move || {
            seen.fetch_add(1, Ordering::SeqCst);
        });
        assert!(matches!(f.lift(), Err(AuthorityError::Unavailable(_))));
        assert_eq!(reached.load(Ordering::SeqCst), 0);
        assert!(f.gov.stop_snapshot().kernel_stopped);
        assert!(f.gov.stop_snapshot().broker_stopped);
    }
}

#[test]
fn decision10_cancelled_session_run_retained_before_commitment_pin() {
    let f = selected();
    let runs = f.gov.authority().runs();
    runs.set_capacity_for_test(runs.len_for_test());
    let _ = f.door.cancel_run(f.run).unwrap().write();
    let legacy = f.gov.issue_agent_door(mother(), f.owner.clone()).unwrap();
    let _ = legacy.open_run();
    assert!(
        runs.view(f.run).is_some(),
        "admitted session run retained before pin"
    );
    assert!(
        matches!(runs.admit_file_stage(f.run), Err(AuthorityError::Halted)),
        "cancelled session run refuses staging even before emergency ingress"
    );
}

#[test]
fn decision11_undo_deferred_cleanup_rechecks_ingress_and_retains_tail() {
    let f = HostFx::new();
    f.propose(NAME, CONTENT).unwrap();
    let staging = f.state().join("coding-runs").join(&f.staged()[0]);
    let _ = f.door.cancel_run(f.run).unwrap().write();
    let cell = f.gov.file_session_halt();
    assert_eq!(f.gov.files().deferred_for_test(), 1);
    let ingress = cell.clone();
    f.gov
        .files()
        .hooks()
        .arm(at::FILE_CLEANUP, move || ingress.engage_emergency());
    assert!(f.undo().is_err(), "no published undo target exists");
    let result = f.cleanup();
    assert!(
        matches!(result, Err(AuthorityError::Halted)),
        "ingress at deferred cleanup must refuse the recovery request: {result:?}"
    );
    assert!(
        staging.exists(),
        "unattempted deferred staging remains owned"
    );
    assert_eq!(f.gov.files().deferred_for_test(), 1);
    assert!(
        !f.gov.stop_snapshot().kernel_stopped,
        "no recording worker ran"
    );
    // No new request can drain after emergency; the unattempted tail stays.
    assert!(matches!(f.undo(), Err(AuthorityError::Halted)));
    assert!(staging.exists());
    assert_eq!(f.gov.files().deferred_for_test(), 1);
}
