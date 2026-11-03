//! Oracles for H3's kernel write-grant half (Codex re-check 2 of PR #64,
//! HIGH 3; binding §11 M4′), on M2d-K2's `write_grant_at_close`,
//! pre-registered before the broker reads it. A kernel write grant that
//! lapsed after the final check is reported beside a truthful publication
//! and ends Files for the run. A refused attempt, whose grant K2 also reports
//! lapsed, never reports it as an interruption. A child of the B2 oracles,
//! for their fixture.
//!
//! Not injectable from the broker, so covered by K2's kernel oracles
//! instead:
//! - the write grant's expiry: its 120 s lifetime is system time, and the
//!   kernel's short-lived grant (`grant_for_apply_lasting`) is `cfg(test)`
//!   inside the kernel crate;
//! - an undo's grant lapsing after U1: the kernel's restore takes no
//!   observer, and its only hook after U1 is crate-private, so no broker
//!   test can order a lapse between U1 and the kernel's close.
//!
//! The undo path is held instead to the same reader as the publication path
//! by a source guard.

use super::*;

/// The kernel write grant revoked just after the final check: the one
/// publication completes, the result and the evidence report
/// ("interrupted", "write_grant_revoked"), and the run asks for nothing
/// more, though its Files grant and its run are still live.
#[test]
fn m2d_b2_r4_a_a_write_grant_revoked_after_the_handoff_is_reported_and_ends_files() {
    let f = Fx::new();
    let view = f.ready(NAME, CONTENT);
    Cause::WriteGrant.arm(&f, at::FILE_AFTER_FINAL);
    let output = f.execute(&view).expect("the one publication completes");
    assert_eq!(f.entries(), vec![NAME.to_string()]);
    let interrupted = ("interrupted".to_string(), "write_grant_revoked".to_string());
    assert!(output.meta.contains(&interrupted), "{:?}", output.meta);
    assert!(
        f.finished(view.id).detail.contains(&interrupted),
        "in the evidence too"
    );
    assert!(f
        .gov
        .authority()
        .runs()
        .view(f.run)
        .is_some_and(|v| !v.cancelled));
    assert!(
        f.propose("next.txt", "n\n").is_err(),
        "no further Files request for the run"
    );
}

/// A refused attempt (the write grant revoked before the final check),
/// whose grant K2 reports as lapsed at its close, never reports it as an
/// interruption, and does not end Files for the run: a new request is
/// accepted.
#[test]
fn m2d_b2_r4_b_a_refused_attempt_never_reports_the_write_grant() {
    let f = Fx::new();
    let view = f.ready(NAME, CONTENT);
    Cause::WriteGrant.arm(&f, at::FILE_BEFORE_FINAL);
    let refused = f.execute(&view).expect_err("refused at its final check");
    assert!(f.entries().is_empty());
    let mut text = format!("{refused:?} {refused}");
    for record in f.records_of(view.id) {
        text.push_str(&format!("{record:?}\n{}\n", record.to_json()));
    }
    assert!(!text.contains("write_grant_revoked"), "{text}");
    assert!(
        f.propose("next.txt", "n\n").is_ok(),
        "Files is not closed by a refusal"
    );
}

/// The undo path reads the same lapse the publication path reads, once
/// its restore completed: a source guard (not a proof), since no broker
/// test can order a lapse after U1.
#[test]
fn m2d_b2_r4_c_a_completed_undo_reads_the_write_grant_at_close() {
    let source = include_str!("../files/mod.rs");
    let undo = source.find("pub(crate) fn undo(").expect("Files::undo");
    let body = &source[undo..];
    let end = body.find("\n    }\n").expect("its end");
    let body = &body[..end];
    assert!(
        body.contains("write_grant_lapse(&applied.run)"),
        "a completed undo reads the write grant at its close"
    );
    let reads = source.matches("write_grant_lapse(").count();
    assert!(
        reads >= 3,
        "the reader, the publication and the undo: {reads}"
    );
}
