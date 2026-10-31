//! M0-FIX2 F10 (production construction): the front door, built as
//! production builds it, attaches the evidence of record to the process's
//! stop authority, so a model call the owner's Stop halts is durably
//! recorded there and the record survives reopening the store.
//!
//! This binary drives the process's own stop authority, so it holds
//! exactly one test.
#![cfg(unix)]

use nexus_governed_control::authority::clock::SystemClock;
use nexus_governed_control::broker::Vault;
use nexus_governed_control::evidence_store::PrimaryEvidence;
use nexus_governed_control::governed::GovernedControl;
use nexus_kernel::emergency_stop::{ChildEnd, GuardedChild, HaltArea, StopAuthority};
use std::process::{Command, Stdio};
use std::sync::Arc;

#[test]
fn m0_fix2_f10_the_production_front_door_records_a_halted_model_call() {
    let root = std::env::temp_dir().join(format!(
        "nexus-m0-fix2-f10-halt-audit-{}",
        std::process::id()
    ));
    let store = root.join("p3-evidence").join("evidence.sqlite3");
    let evidence = PrimaryEvidence::open(&store, Vec::new()).unwrap();
    let control = GovernedControl::new(
        &root,
        Vault::Disabled,
        evidence.clone(),
        Arc::new(SystemClock::default()),
    )
    .unwrap();

    // A model call in flight under the process's authority.
    let watch = StopAuthority::global().admit().unwrap();
    // The kernel stop that ends the call: the first engaged after its
    // admission (the owner's Stop engages the kernel stop twice).
    let ending_stop = StopAuthority::global().stop_number() + 1;
    let mut call = Command::new("cat");
    call.stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let (child, mut pipes) = GuardedChild::spawn(&mut call, &watch, HaltArea::ModelCall).unwrap();
    let _held_open = pipes.stdin.take();
    control.emergency_stop();
    assert!(matches!(child.finish(), ChildEnd::Halted(Ok(_))));

    // Reopened: the store qualifies (its chain verifies) and holds the halt.
    drop(control);
    drop(evidence);
    drop(PrimaryEvidence::open(&store, Vec::new()).expect("the store reopens"));
    let db =
        rusqlite::Connection::open_with_flags(&store, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap();
    let bodies: Vec<String> = db
        .prepare("SELECT body FROM record WHERE phase = 'kernel_halt'")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(bodies.len(), 1, "{bodies:?}");
    let halt: serde_json::Value = serde_json::from_str(&bodies[0]).unwrap();
    assert_eq!(halt["detail"]["area"], "model_call", "{halt}");
    assert_eq!(halt["detail"]["stop_number"], ending_stop.to_string());
    assert_eq!(halt["outcome"], "ended", "{halt}");
    drop(db);
    let _ = std::fs::remove_dir_all(&root);
}
