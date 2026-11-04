//! Decision 10: exact review-4 schedules and the wider Owner/lookup sweep.
//! These baseline shims use the old routes on 6bfc637a. The inherent optional
//! cell APIs replace them by ordinary Rust method resolution after the fix;
//! the schedules, effect observations and assertions stay unchanged.
use super::*;
use crate::authority::file_session::HaltCell;
use crate::files::{CleanupRetry, UndoReport};

#[allow(dead_code)]
trait BaselineRecoveryRoutes {
    fn files_undo_with_halt(
        &self,
        owner: &dyn ControlConfirmer,
        cell: Option<&HaltCell>,
    ) -> Result<UndoReport, AuthorityError>;
    fn files_retry_cleanup_with_halt(
        &self,
        cell: Option<&HaltCell>,
    ) -> Result<CleanupRetry, AuthorityError>;
    fn resume_with_halt(
        &self,
        owner: &dyn ControlConfirmer,
        cell: Option<&HaltCell>,
    ) -> Result<(), AuthorityError>;
}
impl BaselineRecoveryRoutes for GovernedControl {
    fn files_undo_with_halt(
        &self,
        owner: &dyn ControlConfirmer,
        _: Option<&HaltCell>,
    ) -> Result<UndoReport, AuthorityError> {
        self.files_undo(owner)
    }
    fn files_retry_cleanup_with_halt(
        &self,
        _: Option<&HaltCell>,
    ) -> Result<CleanupRetry, AuthorityError> {
        self.files_retry_cleanup()
    }
    fn resume_with_halt(
        &self,
        owner: &dyn ControlConfirmer,
        _: Option<&HaltCell>,
    ) -> Result<(), AuthorityError> {
        self.resume(owner)
    }
}

fn published_recovery() -> Fx {
    let f = Fx::new();
    let a = f.ready(NAME, CONTENT);
    f.execute(&a).unwrap();
    f.gov.emergency_stop();
    f.gov
        .resume_with_halt(&*f.owner, Some(&f.gov.file_session_halt()))
        .unwrap();
    f
}

fn undo_ingress(point: &'static str) {
    let f = published_recovery();
    let cell = f.gov.file_session_halt();
    let gate = Gate::new();
    let held = gate.clone();
    f.gov.files().hooks().arm(point, move || held.hold());
    let (gov, owner, carried) = (f.gov.clone(), f.owner.clone(), cell.clone());
    let worker = std::thread::spawn(move || gov.files_undo_with_halt(&*owner, Some(&carried)));
    gate.reached();
    cell.engage_emergency(); // repeated ingress, NO Stop worker
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
    // Same permanently engaged cell, but a NEW request/confirmation is valid.
    f.gov.files_undo_with_halt(&*f.owner, Some(&cell)).unwrap();
    assert!(!f.ws.join(NAME).exists());
    assert_eq!(f.owner.undos().len(), 2);
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
fn decision10_undo_dialogue_ingress_requires_new_request() {
    let f = published_recovery();
    let cell = f.gov.file_session_halt();
    let ingress = cell.clone();
    f.owner.during_undo(move || ingress.engage_emergency());
    let result = f.gov.files_undo_with_halt(&*f.owner, Some(&cell));
    assert!(
        f.ws.join(NAME).exists(),
        "dialogue ingress must not remove the file"
    );
    assert!(matches!(result, Err(AuthorityError::Halted)));
    f.gov.files_undo_with_halt(&*f.owner, Some(&cell)).unwrap();
    assert!(!f.ws.join(NAME).exists());
}

#[test]
fn decision10_cleanup_ingress_before_effect_preserves_obligation() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fx::new();
    f.propose(NAME, CONTENT).unwrap();
    let staging = f.state().join("coding-runs").join(&f.staged()[0]);
    let _ = f.door.cancel_run(f.run).unwrap().write();
    std::fs::set_permissions(&staging, std::fs::Permissions::from_mode(0o500)).unwrap();
    f.gov.settle();
    assert_eq!(f.gov.files().kept_for_test(), 1);
    std::fs::set_permissions(&staging, std::fs::Permissions::from_mode(0o700)).unwrap();
    let cell = f.gov.file_session_halt();
    cell.engage_emergency();
    let ingress = cell.clone();
    f.gov
        .files()
        .hooks()
        .arm("files.retry_before_effect", move || {
            ingress.engage_emergency()
        });
    let result = f.gov.files_retry_cleanup_with_halt(Some(&cell));
    assert!(
        staging.exists(),
        "cleanup ingress must preserve the owed staging"
    );
    assert!(matches!(result, Err(AuthorityError::Halted)));
    assert_eq!(f.gov.files().kept_for_test(), 1);
    let retry = f.gov.files_retry_cleanup_with_halt(Some(&cell)).unwrap();
    assert_eq!((retry.settled, retry.still_owed), (1, 0));
    assert!(!staging.exists());
}

#[test]
fn decision10_recovery_lift_ingress_after_record() {
    let f = Fx::new();
    f.gov.emergency_stop();
    let cell = f.gov.file_session_halt();
    let ingress = cell.clone();
    f.gov
        .authority()
        .hooks()
        .arm("resume.before_lift", move || ingress.engage_emergency());
    let result = f.gov.resume_with_halt(&*f.owner, Some(&cell));
    assert!(
        f.gov.stop_snapshot().broker_stopped,
        "ingress after resume record must not lift broker Stop"
    );
    assert!(f.gov.stop_snapshot().kernel_stopped);
    assert!(matches!(result, Err(AuthorityError::Halted)));
    f.gov.resume_with_halt(&*f.owner, Some(&cell)).unwrap();
    assert!(!f.gov.stop_snapshot().broker_stopped);
    assert!(!f.gov.stop_snapshot().kernel_stopped);
    assert!(cell.is_engaged(), "lift never reopens the host session");
}

fn selected() -> Fx {
    let mut f = Fx::new();
    let attached = f.door.file_port().unwrap().attach().unwrap();
    f.door.claim_file_session(&attached).unwrap();
    f.run = f.door.open_file_run(&attached).unwrap();
    f.door.admit_files(f.run, HASH, LABEL).unwrap();
    f
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
        door.agent_action(
            run,
            &Intent::File(FileCreate {
                name: NAME.into(),
                content: CONTENT.into(),
            }),
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
    let f = Fx::new();
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
    let f = published_recovery();
    let cell = f.gov.file_session_halt();
    let ingress = cell.clone();
    f.gov
        .files()
        .hooks()
        .arm("files.undo_kernel_boundary", move || {
            ingress.engage_emergency()
        });
    let result = f.gov.files_undo_with_halt(&*f.owner, Some(&cell)).unwrap();
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
    assert!(f.door.open_run().is_err());
}

#[test]
fn decision11_cleanup_ingress_after_handoff_preserves_actual_outcome() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fx::new();
    f.propose(NAME, CONTENT).unwrap();
    let staging = f.state().join("coding-runs").join(&f.staged()[0]);
    let _ = f.door.cancel_run(f.run).unwrap().write();
    std::fs::set_permissions(&staging, std::fs::Permissions::from_mode(0o500)).unwrap();
    f.gov.settle();
    assert_eq!(f.gov.files().kept_for_test(), 1);
    std::fs::set_permissions(&staging, std::fs::Permissions::from_mode(0o700)).unwrap();
    let cell = f.gov.file_session_halt();
    cell.engage_emergency();
    let ingress = cell.clone();
    f.gov
        .files()
        .hooks()
        .arm(at::FILE_DISCARD, move || ingress.engage_emergency());
    let retry = f.gov.files_retry_cleanup_with_halt(Some(&cell)).unwrap();
    assert_eq!((retry.settled, retry.still_owed), (1, 0));
    assert!(!staging.exists());
    f.gov.emergency_stop();
    assert!(f
        .evidence
        .records()
        .iter()
        .any(|r| r.phase == EvidencePhase::EmergencyStop));
    assert!(f.door.open_run().is_err());
}

#[test]
fn decision11_lift_ingress_at_and_after_kernel_call_reengages_native() {
    for point in ["resume.kernel_boundary", "resume.kernel_returned"] {
        let f = Fx::new();
        f.gov.emergency_stop();
        let cell = f.gov.file_session_halt();
        let ingress = cell.clone();
        f.gov
            .authority()
            .hooks()
            .arm(point, move || ingress.engage_emergency());
        let result = f.gov.resume_with_halt(&*f.owner, Some(&cell));
        assert!(
            f.gov.stop_snapshot().kernel_stopped,
            "{point}: native halt re-engaged before return"
        );
        assert!(f.gov.stop_snapshot().broker_stopped);
        assert!(matches!(result, Err(AuthorityError::Halted)));
        assert!(cell.is_engaged());
        assert!(f.door.open_run().is_err());
        assert!(
            f.evidence
                .records()
                .iter()
                .filter(|r| r.phase == EvidencePhase::EmergencyStop)
                .count()
                >= 2
        );
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
    let f = Fx::new();
    f.propose(NAME, CONTENT).unwrap();
    let staging = f.state().join("coding-runs").join(&f.staged()[0]);
    f.gov.emergency_stop();
    let cell = f.gov.file_session_halt();
    f.gov.resume_with_halt(&*f.owner, Some(&cell)).unwrap();
    assert_eq!(f.gov.files().deferred_for_test(), 1);
    let ingress = cell.clone();
    f.gov
        .files()
        .hooks()
        .arm(at::FILE_CLEANUP, move || ingress.engage_emergency());
    let result = f.gov.files_undo_with_halt(&*f.owner, Some(&cell));
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
    // A fresh request still performs the inherited deferred drain, even
    // though there is no published file eligible for undo.
    assert!(matches!(
        f.gov.files_undo_with_halt(&*f.owner, Some(&cell)),
        Err(AuthorityError::Unavailable(_))
    ));
    assert!(!staging.exists());
    assert_eq!(f.gov.files().deferred_for_test(), 0);
}
