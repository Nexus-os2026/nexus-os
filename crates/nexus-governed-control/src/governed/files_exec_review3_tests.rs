//! Oracles for Codex re-check 2 of PR #64 (HIGH 1 and HIGH 2), pre-registered
//! before the repairs. HIGH 1: an execution already past its revalidation
//! must still refuse once another execution has closed their run for Files
//! (H3's closure, M4′). HIGH 2: an undo refused after its kernel write grant
//! was issued closes that grant, and a closure that fails is surfaced, never
//! silent. (HIGH 3, the kernel write grant's interruption after the final
//! check, has no observable kernel API: reported BLOCKED, not here.) A child
//! of the B2 oracles, for their fixture.

use super::*;

/// HIGH 1: execution B passes its revalidation (and its start), then pauses
/// before it takes the one Files slot. Execution A, of the same run and with
/// the earlier deadline, publishes and reports its commitment's expiry
/// after the handoff, which closes the run for Files. B resumes with its own
/// deadline still live: it refuses, and nothing is published for it.
#[test]
fn m2d_b2_r3_h1_an_execution_past_revalidation_refuses_once_the_run_is_closed() {
    let f = Fx::new();
    let a = f.ready(NAME, CONTENT);
    f.clock.advance(Duration::from_secs(100));
    let b = f.ready("b.txt", "b\n");

    let gate = Gate::new();
    let holding = gate.clone();
    f.gov
        .files()
        .hooks()
        .arm(at::FILE_EXECUTE_START, move || holding.hold());
    let (gov, run, id) = (f.gov.clone(), f.run, b.id);
    let paused = std::thread::spawn(move || gov.execute(id, &mother(), run).is_ok());
    gate.reached();

    // A's deadline (600 s) is reached after its handoff; B's (700 s) is not.
    let clock = f.clock.clone();
    f.gov.files().hooks().arm(at::FILE_AFTER_FINAL, move || {
        clock.advance(Duration::from_secs(500))
    });
    let output = f.execute(&a).expect("A publishes");
    assert!(
        output
            .meta
            .contains(&("interrupted".to_string(), "commitment_expired".to_string())),
        "A reports the interruption and closes the run: {:?}",
        output.meta
    );

    gate.open();
    assert!(
        !paused.join().unwrap(),
        "B refuses: its run was closed for Files meanwhile"
    );
    assert!(!f.ws.join("b.txt").exists(), "nothing published for B");
    assert_eq!(f.entries(), vec![NAME.to_string()]);
}

/// HIGH 2: an undo whose lifetime ends after its kernel write grant was
/// issued (Stop and resume at `files.undo_approved`) is refused with that
/// grant closed in the registry. When the closure itself fails (forced), the
/// undo reports recovery required and the startup report lists the grant as
/// not closed: never silent. A later request issues its own grant and
/// completes.
#[test]
fn m2d_b2_r3_h2_a_refused_undo_closes_its_write_grant_or_says_it_could_not() {
    let f = Fx::new();
    f.create(NAME, CONTENT).unwrap();
    let stop_and_resume = |f: &Fx| {
        let (gov, owner) = (f.gov.clone(), f.owner.clone());
        f.gov.files().hooks().arm(at::FILE_UNDO_APPROVED, move || {
            gov.emergency_stop();
            gov.resume(&*owner).unwrap();
        });
    };

    stop_and_resume(&f);
    assert!(f.gov.files_undo(&*f.owner).is_err(), "the undo is refused");
    assert!(
        !f.gov.files().write_grant_live_for_test(),
        "its issued write grant is closed"
    );
    assert_eq!(f.entries(), vec![NAME.to_string()], "nothing touched");

    f.gov.files().fail_grant_closure_for_test(true);
    stop_and_resume(&f);
    assert_eq!(
        class(f.gov.files_undo(&*f.owner)),
        "recovery_required",
        "a closure that fails is surfaced"
    );
    assert!(
        f.gov.files().write_grant_live_for_test(),
        "the grant really is still live"
    );
    let report = f.gov.files_recovery_report().unwrap();
    assert!(
        report.iter().any(|entry| entry
            .findings
            .contains(&RecoveryFinding::WriteGrantNotClosed)),
        "and reported: {report:?}"
    );
    f.gov.files().fail_grant_closure_for_test(false);

    f.gov
        .files_undo(&*f.owner)
        .expect("a new undo request completes");
    assert!(f.entries().is_empty());
}
