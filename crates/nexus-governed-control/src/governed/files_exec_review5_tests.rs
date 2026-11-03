//! Oracles for Codex re-check 3 of PR #64 (the two MEDIUMs of HIGH 2's undo
//! grant closure on unwind), with the Architect's single design ("armed for
//! the grant's whole life, outcome always recorded"), pre-registered before
//! the repair. A panic inside the undo's guarded restore region (hook
//! `files.undo_restore`, in the same scope as the restore call) still
//! closes the undo's kernel write grant; when that closure fails, the
//! recovery report lists it; the normal path is unchanged. A child of the B2
//! oracles, for their fixture.

use super::*;
use std::panic::{catch_unwind, AssertUnwindSafe};

/// Run an undo whose guarded restore region panics; the panic is caught.
fn undo_that_panics(f: &Fx) {
    f.gov.files().hooks().arm(at::FILE_UNDO_RESTORE, || {
        panic!("a panic inside the undo's guarded restore region (test)")
    });
    let (gov, owner) = (f.gov.clone(), f.owner.clone());
    let unwound = catch_unwind(AssertUnwindSafe(move || {
        let _ = gov.files_undo(&*owner);
    }));
    assert!(unwound.is_err(), "the undo unwound");
}

fn listed_not_closed(f: &Fx) -> bool {
    f.gov.files_recovery_report().unwrap().iter().any(|entry| {
        entry
            .findings
            .contains(&RecoveryFinding::WriteGrantNotClosed)
    })
}

/// (a) A panic inside the guarded restore region: afterwards the undo's
/// kernel write grant no longer resolves.
#[test]
fn m2d_b2_r5_a_an_unwinding_undo_still_closes_its_write_grant() {
    let f = Fx::new();
    f.create(NAME, CONTENT).unwrap();
    undo_that_panics(&f);
    assert!(
        !f.gov.files().write_grant_live_for_test(),
        "the grant is closed after the unwind"
    );
    assert!(!listed_not_closed(&f), "and its closure was established");
}

/// (b) The same panic with the closure forced to fail: the recovery report
/// lists the grant as not closed (never silent), and it really is still
/// live.
#[test]
fn m2d_b2_r5_b_an_unwinding_undo_whose_closure_fails_is_reported() {
    let f = Fx::new();
    f.create(NAME, CONTENT).unwrap();
    f.gov.files().fail_grant_closure_for_test(true);
    undo_that_panics(&f);
    f.gov.files().fail_grant_closure_for_test(false);
    assert!(
        f.gov.files().write_grant_live_for_test(),
        "the forced failure left it live"
    );
    assert!(listed_not_closed(&f), "the recovery report lists it");
}

/// (c) Control: the normal path is unchanged: the undo completes, its grant
/// no longer resolves, and nothing is reported.
#[test]
fn m2d_b2_r5_c_the_normal_undo_is_unchanged() {
    let f = Fx::new();
    f.create(NAME, CONTENT).unwrap();
    let report = f.gov.files_undo(&*f.owner).expect("the undo completes");
    assert_eq!(report.interrupted, None);
    assert!(f.entries().is_empty(), "the file was removed");
    assert!(!f.gov.files().write_grant_live_for_test());
    assert!(!listed_not_closed(&f));
}
