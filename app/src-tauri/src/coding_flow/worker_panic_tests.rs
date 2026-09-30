//! P2-ENTRY-H1-R1 desktop tests for the coding worker thread's panic
//! boundary (`guarded`, which production wraps around all of the worker's
//! governed work, around `CodingRun::guard_worker`). A panicking worker ends
//! its thread normally with the run's authority closed and the display at
//! `recovery_required`: never busy, never applicable, never the payload.

use super::linux::{claim, guarded, RunSlot, WORKER_STOPPED};
use nexus_kernel::coding_run::{
    ApplyError, CodingRun, ConfirmationRequest, FolderPicker, LedgerStore, OwnerConfirmer,
    ProjectRegistry, RecoveryReason, RunId, RunScopes, RunState, ScopeEntry, ScopeSet,
    StagingParent,
};
use nexus_kernel::workspace_authority::{
    WorkspaceAuthorityError, WorkspaceAuthorityRegistry, WorkspaceBinding, WorkspaceGrantId,
};
use nexus_persistence::coding_run_ledger::CodingRunLedger;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Distinctive fragments of the panic payload; nothing shown or recorded may
/// hold any of them.
const PAYLOAD_TOKENS: [&str; 3] = [
    "P2E-H1-R1-PAYLOAD",
    "/home/owner/secret-project",
    "token=hunter2",
];

fn panic_with_payload() {
    panic!(
        "{}: {} {}",
        PAYLOAD_TOKENS[0], PAYLOAD_TOKENS[1], PAYLOAD_TOKENS[2]
    );
}

struct Picker(PathBuf);

impl FolderPicker for Picker {
    fn pick_folder(&self) -> Option<PathBuf> {
        Some(self.0.clone())
    }
}

/// A confirmer that must never be asked.
#[derive(Default)]
struct NeverAsked(AtomicBool);

impl OwnerConfirmer for NeverAsked {
    fn confirm(&self, _request: &ConfirmationRequest) -> bool {
        self.0.store(true, Ordering::SeqCst);
        true
    }
}

struct Case {
    root: PathBuf,
    authority: Arc<WorkspaceAuthorityRegistry>,
    binding: WorkspaceBinding,
    read: WorkspaceGrantId,
    ledger: Arc<CodingRunLedger>,
    slot: Arc<RunSlot>,
    id: RunId,
}

impl Drop for Case {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// A created run over a native-picked project under `root`, holding the run
/// read grant the desktop issues, in the slot production builds.
fn case_in(root: PathBuf) -> Case {
    std::fs::create_dir_all(root.join("project/src")).unwrap();
    std::fs::create_dir_all(root.join("state")).unwrap();
    std::fs::write(root.join("project/src/lib.rs"), "pub fn a() {}\n").unwrap();
    let root = root.canonicalize().unwrap();
    let authority = Arc::new(WorkspaceAuthorityRegistry::new());
    let projects = ProjectRegistry::new(Arc::clone(&authority), &root.join("state"));
    let info = projects.select(&Picker(root.join("project"))).unwrap();
    let binding = WorkspaceBinding {
        agent_id: uuid::Uuid::new_v4(),
        run_id: uuid::Uuid::new_v4(),
    };
    let grant = projects.grant_for_run(info.id, binding).unwrap();
    let read = grant.grant_id();
    let ledger = Arc::new(CodingRunLedger::open(&root.join("ledger.db")).unwrap());
    let scopes = RunScopes::new(
        ScopeSet::new([ScopeEntry::WholeProject]),
        ScopeSet::new([ScopeEntry::WholeProject]),
        ScopeSet::default(),
    )
    .unwrap();
    let run =
        CodingRun::create_for_project(Arc::clone(&ledger) as Arc<dyn LedgerStore>, grant, scopes)
            .unwrap();
    let id = run.id();
    let slot = Arc::new(RunSlot::new(
        run,
        binding,
        info,
        "qwen2.5-coder:7b".to_string(),
    ));
    assert!(authority.resolve(read, binding).is_ok());
    Case {
        root,
        authority,
        binding,
        read,
        ledger,
        slot,
        id,
    }
}

fn case() -> Case {
    case_in(std::env::temp_dir().join(format!("nexus-p2e-r1-{}", uuid::Uuid::new_v4())))
}

/// Run `body` behind the production boundary on a real worker thread.
fn run_worker_thread(slot: &Arc<RunSlot>, body: impl FnOnce(&mut CodingRun) + Send + 'static) {
    let worker = Arc::clone(slot);
    let handle = std::thread::Builder::new()
        .name("nexus-coding-run".to_string())
        .spawn(move || guarded(&worker, body))
        .unwrap();
    assert!(
        handle.join().is_ok(),
        "the panic must never escape the worker thread"
    );
}

fn revoked(
    authority: &WorkspaceAuthorityRegistry,
    grant: WorkspaceGrantId,
    binding: WorkspaceBinding,
) -> bool {
    matches!(
        authority.resolve(grant, binding),
        Err(WorkspaceAuthorityError::RevokedGrant)
    )
}

fn project_bytes(project: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut files = BTreeMap::new();
    let mut pending = vec![project.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else {
                files.insert(path.clone(), std::fs::read(&path).unwrap());
            }
        }
    }
    files
}

/// The display after a caught panic: recovery required, not busy, not
/// applicable or restorable, discardable, the fixed message only.
fn assert_recovery_view(c: &Case, run_state: &str) {
    let view = c.slot.view(c.id);
    assert_eq!(view.stage, "recovery_required");
    assert_eq!(view.message.as_deref(), Some(WORKER_STOPPED));
    assert_eq!(view.run_state, run_state);
    assert!(!view.can_apply && !view.can_restore, "{view:?}");
    assert!(view.can_discard, "an explicit discard stays available");
    assert!(view.review.is_none(), "no stale review is shown");
    let shown = serde_json::to_string(&view).unwrap();
    for token in PAYLOAD_TOKENS {
        assert!(!shown.contains(token), "the payload reached the view");
    }
}

fn assert_no_payload_recorded(c: &Case) {
    for record in c.ledger.verify_run(c.id.ledger_key()).unwrap() {
        for token in PAYLOAD_TOKENS {
            assert!(!record.payload.contains(token), "{}", record.event_kind);
        }
    }
}

/// Apply cannot even be requested: the owner action's claim is refused, and
/// the run cannot be approved (the confirmer is never asked).
fn assert_apply_refused(c: &Case) {
    assert!(claim(&c.slot, &["review"], "applying").is_err());
    let confirmer = NeverAsked::default();
    let mut run = c.slot.run.lock().unwrap();
    assert!(matches!(
        run.request_approval("project", &confirmer),
        Err(ApplyError::InvalidState)
    ));
    assert!(!confirmer.0.load(Ordering::SeqCst));
}

#[test]
fn p2e_r1_d1_a_worker_panic_closes_the_read_grant_and_shows_recovery() {
    let c = case();
    let before = project_bytes(&c.root.join("project"));
    run_worker_thread(&c.slot, |_run| panic_with_payload());
    assert!(!c.slot.run.is_poisoned(), "the run guard never unwound");
    assert!(revoked(&c.authority, c.read, c.binding));
    assert_eq!(
        c.slot.run.lock().unwrap().state(),
        RunState::RecoveryRequired(RecoveryReason::WorkerPanicked)
    );
    assert_recovery_view(&c, "RecoveryRequired(WorkerPanicked)");
    assert_apply_refused(&c);
    assert_no_payload_recorded(&c);
    assert_eq!(project_bytes(&c.root.join("project")), before);
}

#[test]
fn p2e_r1_d2_a_panic_holding_the_display_lock_neither_deadlocks_nor_stays_busy() {
    let c = case();
    let slot = Arc::clone(&c.slot);
    run_worker_thread(&c.slot, move |_run| {
        // Unwinding drops this guard and poisons the display mutex; the
        // recovery update must still get through.
        let _display = slot.display();
        panic_with_payload();
    });
    assert!(revoked(&c.authority, c.read, c.binding));
    assert_recovery_view(&c, "RecoveryRequired(WorkerPanicked)");
}

#[test]
fn p2e_r1_d3_a_body_that_does_not_panic_is_untouched() {
    let c = case();
    run_worker_thread(&c.slot, |run| assert_eq!(run.state(), RunState::Created));
    let view = c.slot.view(c.id);
    assert_eq!((view.stage, view.message), ("preparing", None));
    assert!(c.authority.resolve(c.read, c.binding).is_ok());
    assert_eq!(c.slot.run.lock().unwrap().state(), RunState::Created);
    assert!(!c
        .ledger
        .verify_run(c.id.ledger_key())
        .unwrap()
        .iter()
        .any(|record| record.event_kind == "run.recovery_required"));
}

#[test]
fn p2e_r1_d4_a_panic_after_a_clean_end_still_shows_recovery() {
    let c = case();
    run_worker_thread(&c.slot, |run| {
        run.cancel().unwrap();
        panic_with_payload();
    });
    // The recorded outcome is kept; the worker still did not finish normally.
    assert_eq!(c.slot.run.lock().unwrap().state(), RunState::Cancelled);
    assert!(revoked(&c.authority, c.read, c.binding));
    assert_recovery_view(&c, "Cancelled");
}

/// End to end with staging authority: the worker stages the project under an
/// identity home, then panics. `HOME` is process state, so this runs in a
/// child process whose identity home is a private temporary directory; the
/// developer's own home is never read or written.
#[test]
fn p2e_r1_d5_a_panic_after_staging_closes_staging_and_read_authority() {
    const CHILD: &str = "NEXUS_P2E_R1_STAGING_CHILD";
    const NAME: &str = "coding_flow::worker_panic_tests::p2e_r1_d5_a_panic_after_staging_closes_staging_and_read_authority";
    const WITNESS: &str = "p2e-r1-d5 staging witness";
    if std::env::var(CHILD).as_deref() == Ok(NAME) {
        let home = PathBuf::from(std::env::var_os("HOME").unwrap());
        let c = case_in(home.join("case"));
        let before = project_bytes(&c.root.join("project"));
        run_worker_thread(&c.slot, |run| {
            let parent = StagingParent::from_identity_home().unwrap();
            run.grant(&parent).unwrap();
            run.snapshot().unwrap();
            panic_with_payload();
        });
        // The staging grant id is only a recorded name; it must not resolve.
        let granted = c
            .ledger
            .verify_run(c.id.ledger_key())
            .unwrap()
            .into_iter()
            .find(|record| record.event_kind == "run.granted")
            .unwrap();
        let facts: serde_json::Value = serde_json::from_str(&granted.payload).unwrap();
        let staging: WorkspaceGrantId =
            serde_json::from_value(facts["staging_grant"].clone()).unwrap();
        assert!(revoked(&c.authority, staging, c.binding));
        assert!(revoked(&c.authority, c.read, c.binding));
        assert_eq!(
            c.slot.run.lock().unwrap().state(),
            RunState::RecoveryRequired(RecoveryReason::WorkerPanicked)
        );
        assert_recovery_view(&c, "RecoveryRequired(WorkerPanicked)");
        assert_apply_refused(&c);
        assert_no_payload_recorded(&c);
        assert_eq!(project_bytes(&c.root.join("project")), before);
        eprintln!("\n{WITNESS}");
        return;
    }
    let home = std::env::temp_dir().join(format!("nexus-p2e-r1-home-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&home).unwrap();
    let home = home.canonicalize().unwrap();
    let (stdout, stderr) = (home.join("stdout"), home.join("stderr"));
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", NAME, "--nocapture", "--test-threads=1"])
        .env(CHILD, NAME)
        .env("HOME", &home)
        .env("NEXUS_ORACLE_EPHEMERAL", "1")
        .env("NEXUS_DB_PATH", ":memory:")
        .env("NEXUS_CONFIG_PATH", home.join("config.toml"))
        .stdin(std::process::Stdio::null())
        .stdout(std::fs::File::create(&stdout).unwrap())
        .stderr(std::fs::File::create(&stderr).unwrap())
        .spawn()
        .unwrap();
    // Bounded: the parent owns the child and kills and reaps it at the deadline.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            let _ = std::fs::remove_dir_all(&home);
            panic!("p2e_r1_d5 child exceeded its deadline");
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    let err = std::fs::read_to_string(&stderr).unwrap();
    let out = std::fs::read_to_string(&stdout).unwrap();
    let _ = std::fs::remove_dir_all(&home);
    assert!(status.success(), "{status}\n{out}\n{err}");
    assert_eq!(
        err.lines().filter(|line| *line == WITNESS).count(),
        1,
        "the child must run the observation exactly once\n{err}"
    );
}
