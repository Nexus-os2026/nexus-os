//! Oracles for the Architect's H1 decision (Option B) on Codex review 1 of
//! PR #64, Codex's broker table rows 2-6 (`m2d-b2-h1-decision-codex.md`),
//! pre-registered before the repair. The Stop records stay synchronous in
//! the calling worker. What is proved here is that the halt is engaged,
//! observably and in memory, before any blocking dependency: the evidence
//! store, a publication, a cleanup. Dropping a pending effect only
//! transfers ownership. Files observes the engaged halt independently of
//! recording. Stop and resume revive nothing old.
//!
//! Every race is a forced order: the real evidence store held inside its
//! mutex (`Fault::Stall`), or a named hook. A child of the B2 oracles, for
//! their fixture.

use super::*;
use crate::authority::commitment::CommitmentState;
use crate::evidence_store::tests::Scratch;
use crate::evidence_store::{Fault, PrimaryEvidence};

/// The bound within which the engaged halt, or a refusal, must be seen.
const BOUND: Duration = Duration::from_secs(1);

/// Wait until `cond` holds, failing after `BOUND` (readiness polling: the
/// order itself is forced by what the test holds).
fn within(what: &str, mut cond: impl FnMut() -> bool) {
    let deadline = Instant::now() + BOUND;
    while !cond() {
        assert!(Instant::now() < deadline, "{what}: not within 1 s");
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// The B2 fixture on the real evidence store (its writes projected to
/// `f.evidence`).
struct OnStore {
    f: Fx,
    primary: Arc<PrimaryEvidence>,
    _scratch: Scratch,
}

fn on_store() -> OnStore {
    assert!(make_non_dumpable(), "the host's startup step");
    let scratch = Scratch::new();
    let recent = Arc::new(MemoryEvidence::new(100_000));
    let primary = PrimaryEvidence::open(&scratch.store(), vec![recent.clone()]).unwrap();
    let root = temp_root("m2d-b2-r2");
    let clock = Arc::new(ManualClock::default());
    let stop = StopAuthority::new();
    let gov = Arc::new(
        GovernedControl::with_secrets(
            root.0.path(),
            Vault::Disabled.source(),
            primary.clone(),
            clock.clone(),
        )
        .unwrap()
        .with_kernel_stop(stop.clone()),
    );
    let dirs = Dirs::new();
    let ws = dirs.0.join("home/ws");
    let info = gov
        .register_workspace_in(&ws, &dirs.exclusions())
        .expect("an empty workspace");
    let owner = Owner::new();
    let grant = gov
        .request_grant(&GrantRequest::Files { workspace: info.id }, LONG, &*owner)
        .expect("the Owner grants Files");
    let door = Arc::new(gov.issue_agent_door(mother(), owner.clone()).unwrap());
    door.confirm_mission(&MissionConfirmation {
        contract_hash: HASH.into(),
        request: LABEL.into(),
        summary: vec!["A one-page site for Rosa's bakery".into()],
    })
    .expect("the Owner consents");
    let run = door.open_run().unwrap();
    door.admit_files(run, HASH, LABEL).unwrap();
    OnStore {
        f: Fx {
            gov,
            evidence: recent,
            clock,
            stop,
            owner,
            door,
            run,
            grant,
            ws,
            _dirs: dirs,
            root,
        },
        primary,
        _scratch: scratch,
    }
}

/// The engaged halt, in memory: the kernel stop and the broker's stop in
/// force, `run` cancelled, `pending` revoked, the policy generation moved
/// past `generation`.
fn halted(f: &Fx, run: RunId, pending: CommitmentId, generation: u64) -> bool {
    let authority = f.gov.authority();
    f.stop.is_stopped()
        && authority.runs().is_stopped()
        && authority
            .runs()
            .view(run)
            .is_some_and(|view| view.cancelled)
        && authority
            .commitments()
            .view(pending)
            .is_some_and(|view| view.state == CommitmentState::Revoked)
        && authority.policy_generation() > generation
}

/// Count every call that reaches `point` (from now on).
fn counting(f: &Fx, point: &'static str) -> Arc<AtomicUsize> {
    let count = Arc::new(AtomicUsize::new(0));
    let counted = count.clone();
    f.gov.files().hooks().every(point, move || {
        counted.fetch_add(1, Ordering::SeqCst);
    });
    count
}

/// A second run of the fixture's door, admitted, with a staged create
/// whose effect is then dropped by the run's cancellation: a run handed
/// over to the deferred collection.
fn deferred_run(f: &Fx) {
    let other = f.door.open_run().unwrap();
    f.door.admit_files(other, HASH, LABEL).unwrap();
    let staged = f.door.agent_action(
        other,
        &Intent::File(FileCreate {
            name: "deferred.txt".into(),
            content: "d\n".into(),
        }),
    );
    assert!(matches!(staged, Ok(AgentOutcome::AwaitingApproval(_))));
    let _ = f.door.cancel_run(other).unwrap().write();
}

// ---------------------------------------------------------------- row 2

/// Row 2: an independent writer holds the real evidence store inside a
/// stalled append before Stop is entered. Within 1 s, with storage still
/// blocked, the halt is observed engaged in memory: the kernel and broker
/// stops, the run cancelled, its commitment revoked, the generation moved.
/// The Stop call itself may still be recording.
#[test]
fn m2d_b2_r2_row2_the_halt_is_engaged_while_another_writer_holds_the_store() {
    let s = on_store();
    let f = &s.f;
    let pending = f.propose(NAME, CONTENT).unwrap();
    let generation = f.gov.authority().policy_generation();
    let stall = Gate::new();
    s.primary.store().plant_fault(Fault::Stall(stall.clone()));
    let door = f.door.clone();
    let writer = std::thread::spawn(move || {
        let _ = door.open_run();
    });
    stall.reached();
    let gov = f.gov.clone();
    let stopping = std::thread::spawn(move || {
        gov.emergency_stop();
    });
    within("the engaged halt while the store is held", || {
        halted(f, f.run, pending.id, generation)
    });
    assert!(!stopping.is_finished(), "storage is still blocked");
    stall.open();
    writer.join().unwrap();
    stopping.join().unwrap();
    assert!(f
        .evidence
        .records()
        .iter()
        .any(|r| r.phase == EvidencePhase::EmergencyStop));
}

// ---------------------------------------------------------------- row 3

/// Row 3: Stop held inside its own synchronous record write. Without
/// releasing that write, the halt is observed engaged; a publication held
/// at its final check, released, refuses the handoff (observed at the real
/// refusal boundary, before the execution's own cleanup and end record);
/// and an undo returns `emergency_stopped` within 1 s without its dialog
/// or any cleanup.
#[test]
fn m2d_b2_r2_row3_stop_held_in_its_own_write_refuses_the_handoff_and_the_undo() {
    let s = on_store();
    let f = &s.f;
    f.create("first.txt", "1\n").unwrap();
    let view = f.ready(NAME, CONTENT);
    // After the last proposal (a proposal cleans what is deferred).
    deferred_run(f);
    let generation = f.gov.authority().policy_generation();
    let (refused_tx, refused) = mpsc::channel();
    f.gov
        .files()
        .hooks()
        .arm(at::FILE_HANDOFF_REFUSED, move || {
            refused_tx.send(()).unwrap()
        });
    let (gate, worker) = held(f, &view, at::FILE_BEFORE_FINAL);
    let stall = Gate::new();
    s.primary.store().plant_fault(Fault::Stall(stall.clone()));
    let gov = f.gov.clone();
    let stopping = std::thread::spawn(move || {
        gov.emergency_stop();
    });
    stall.reached();
    within("the engaged halt while Stop records", || {
        f.stop.is_stopped()
            && f.gov.authority().runs().is_stopped()
            && f.gov.authority().policy_generation() > generation
    });
    let discards = counting(f, at::FILE_DISCARD);
    let cleanups = counting(f, at::FILE_CLEANUP);
    gate.open();
    refused
        .recv_timeout(BOUND)
        .expect("the handoff is refused while Stop still records");
    assert!(!f.ws.join(NAME).exists(), "nothing published");
    within("the refused execution's own cleanup", || {
        discards.load(Ordering::SeqCst) >= 1
    });
    assert_eq!(cleanups.load(Ordering::SeqCst), 0);
    let (dialogs, discarded) = (f.owner.undos().len(), discards.load(Ordering::SeqCst));
    let started = Instant::now();
    assert_eq!(class(f.gov.files_undo(&*f.owner)), "emergency_stopped");
    assert!(started.elapsed() < BOUND, "the undo returns within 1 s");
    assert_eq!(f.owner.undos().len(), dialogs, "no dialog");
    assert_eq!(cleanups.load(Ordering::SeqCst), 0, "no cleanup");
    assert_eq!(
        discards.load(Ordering::SeqCst),
        discarded,
        "no staging removal"
    );
    assert!(!stopping.is_finished(), "Stop is still in its write");

    stall.open();
    stopping.join().unwrap();
    assert!(!worker.join().unwrap());
    assert!(f.ws.join("first.txt").exists());
}

// ---------------------------------------------------------------- row 4

/// Row 4: a publication and an actual staging cleanup, each stalled on its
/// own. In each case Stop engages and returns before the blocker is
/// released; Stop's own path waits for neither, and a cleanup in progress
/// holds no lock Stop needs (Stop hands a newly dropped effect to the
/// deferred collection meanwhile).
#[test]
fn m2d_b2_r2_row4_a_stalled_publication_or_cleanup_does_not_hold_stop() {
    // The publication held just after its final check.
    let f = Fx::new();
    let view = f.ready(NAME, CONTENT);
    let (gate, worker) = held(&f, &view, at::FILE_AFTER_FINAL);
    let started = Instant::now();
    f.gov.emergency_stop();
    assert!(started.elapsed() < BOUND, "Stop returned");
    assert!(f.stop.is_stopped() && f.gov.authority().runs().is_stopped());
    gate.open();
    assert!(
        worker.join().unwrap(),
        "the one publication after the handoff"
    );

    // The staging cleanup of a handed-over run, held in progress.
    let f = Fx::new();
    let pending = f.propose(NAME, CONTENT).unwrap();
    // After the last proposal (a proposal cleans what is deferred).
    deferred_run(&f);
    let cleanup = Gate::new();
    let holding = cleanup.clone();
    f.gov
        .files()
        .hooks()
        .arm(at::FILE_DISCARD, move || holding.hold());
    let gov = f.gov.clone();
    let settling = std::thread::spawn(move || gov.settle());
    cleanup.reached();
    let started = Instant::now();
    f.gov.emergency_stop();
    assert!(
        started.elapsed() < BOUND,
        "Stop returned during the cleanup"
    );
    assert!(f.stop.is_stopped() && f.gov.authority().runs().is_stopped());
    assert_eq!(
        f.gov
            .authority()
            .commitments()
            .view(pending.id)
            .unwrap()
            .state,
        CommitmentState::Revoked
    );
    assert_eq!(
        f.gov.files().deferred_for_test(),
        1,
        "the dropped effect was handed over while the cleanup ran"
    );
    cleanup.open();
    settling.join().unwrap();
}

// ---------------------------------------------------------------- row 5

/// Row 5: Stop ends a staged, unexecuted Files effect with every cleanup
/// attempt counted: none happens on Stop's path, the staging remains and
/// the run is owned by the deferred collection. A later cleanup that fails
/// keeps it owned and reported; the Owner's retry then removes it.
#[test]
fn m2d_b2_r2_row5_stop_transfers_a_pending_effect_without_cleanup() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fx::new();
    let discards = counting(&f, at::FILE_DISCARD);
    let cleanups = counting(&f, at::FILE_CLEANUP);
    let _pending = f.propose(NAME, CONTENT).unwrap();
    let staging = f.state().join("coding-runs").join(&f.staged()[0]);
    f.gov.emergency_stop();
    assert_eq!(
        discards.load(Ordering::SeqCst),
        0,
        "no removal on Stop's path"
    );
    assert_eq!(
        cleanups.load(Ordering::SeqCst),
        0,
        "no cleanup on Stop's path"
    );
    assert!(staging.exists(), "the staging remains");
    assert_eq!(
        f.gov.files().deferred_for_test(),
        1,
        "ownership transferred"
    );

    std::fs::set_permissions(&staging, std::fs::Permissions::from_mode(0o500)).unwrap();
    f.gov.settle();
    assert!(
        discards.load(Ordering::SeqCst) >= 1,
        "the cleanup was tried"
    );
    assert!(staging.exists());
    assert_eq!(f.gov.files().kept_for_test(), 1, "still owned");
    let listed = |gov: &GovernedControl| {
        gov.files_recovery_report().unwrap().iter().any(|entry| {
            entry
                .findings
                .contains(&RecoveryFinding::StagingNotDiscarded)
        })
    };
    assert!(listed(&f.gov), "reported");
    std::fs::set_permissions(&staging, std::fs::Permissions::from_mode(0o700)).unwrap();
    let retry = f.gov.files_retry_cleanup().unwrap();
    assert_eq!((retry.settled, retry.still_owed), (1, 0));
    assert!(!staging.exists(), "the retry removed it");
    assert!(!listed(&f.gov));
}

// ---------------------------------------------------------------- row 6

/// Row 6: D2's orderings and lifetimes. A Stop before the handoff
/// prevents the publication, and Stop then resume revives neither it nor
/// an authorized one that had not started. A Stop after the handoff lets
/// only that publication complete, and the run's next Files request is
/// refused even after the resume. An undo whose dialog spans Stop then
/// resume is refused; a new request completes.
#[test]
fn m2d_b2_r2_row6_stop_and_resume_revive_neither_the_old_publication_nor_the_old_undo() {
    let f = Fx::new();
    let view = f.ready(NAME, CONTENT);
    let (gate, worker) = held(&f, &view, at::FILE_BEFORE_FINAL);
    f.gov.emergency_stop();
    f.gov.resume(&*f.owner).unwrap();
    gate.open();
    assert!(!worker.join().unwrap(), "refused before its handoff");
    assert!(f.execute(&view).is_err(), "not revived");
    assert!(!f.ws.join(NAME).exists());

    let g = Fx::new();
    let view = g.ready(NAME, CONTENT);
    g.gov.emergency_stop();
    g.gov.resume(&*g.owner).unwrap();
    assert!(
        g.execute(&view).is_err(),
        "an authorized one is not revived"
    );
    assert!(!g.ws.join(NAME).exists());

    let h = Fx::new();
    let view = h.ready(NAME, CONTENT);
    Cause::EmergencyStop.arm(&h, at::FILE_AFTER_FINAL);
    h.execute(&view)
        .expect("the one publication after the handoff");
    assert_eq!(h.entries(), vec![NAME.to_string()]);
    h.gov.resume(&*h.owner).unwrap();
    assert!(
        h.propose("next.txt", "n\n").is_err(),
        "no next mission action"
    );
    let (gov, owner) = (h.gov.clone(), h.owner.clone());
    h.owner.during_undo(move || {
        gov.emergency_stop();
        gov.resume(&*owner).unwrap();
    });
    assert!(h.gov.files_undo(&*h.owner).is_err(), "the old undo request");
    assert_eq!(h.entries(), vec![NAME.to_string()]);
    h.gov
        .files_undo(&*h.owner)
        .expect("a new undo request after the resume");
    assert!(h.entries().is_empty());
}
