//! P0-A (Codex review 4 of PR #56, MEDIUM; oracle added before its fix):
//! the durability chain stops only at a verified boundary, a directory
//! read successfully that is not private to the store's owner. A level
//! whose metadata cannot be read fails the open, so no put is acknowledged
//! and nothing is journaled on a chain cut short.

use super::p0_support::*;
use super::private_file::seam;
use super::FilePrivate;
use crate::coordinator::CoordinatorError;
use crate::journal::MissionId;
use crate::oracles::contract_with;
use std::path::PathBuf;

fn no_effects(_: MissionId) -> Vec<crate::effect::EffectRequest> {
    vec![]
}

/// `base` at 0755: `base/a` is the highest directory Mother creates.
#[cfg(unix)]
fn open_base(label: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let base = fresh_dir(label);
    std::fs::set_permissions(&base, std::fs::Permissions::from_mode(0o755)).unwrap();
    base.canonicalize().unwrap()
}

/// Opener A creates `base/a` and is paused there. Opener B opens
/// `base/a/b/private` while the read of `a`'s metadata fails: B's open
/// must fail (its chain would otherwise end at `a`, and its puts would
/// never sync `base`).
#[cfg(unix)]
#[test]
fn p0_chain_io_a_metadata_failure_fails_the_open() {
    let base = open_base("chain-io");
    let a = base.join("a");
    seam::pause_after_creating(Some(a.clone()));
    let root_a = a.join("x").join("private");
    let opener_a = std::thread::spawn(move || FilePrivate::open(&root_a).map(|_| ()));
    assert!(seam::wait_paused(), "A paused just after creating a");

    seam::fail_metadata_of(Some(a.clone()));
    let opened = FilePrivate::open(&a.join("b").join("private"));
    seam::fail_metadata_of(None);
    seam::release();
    let _ = opener_a.join();
    assert!(
        opened.is_err(),
        "an open whose chain could not be read succeeded (its chain ended at `a`)"
    );
    let _ = std::fs::remove_dir_all(&base);
}

/// Through the coordinator: Mother's home, whose metadata cannot be read
/// while her private store is opened, fails the reopen, and the journal is
/// left byte for byte (no session record, nothing else).
#[test]
fn p0_chain_io_a_metadata_failure_journals_nothing() {
    let mut w = P0World::new("chain-io-journal", p0_lanes("r", no_effects));
    let id = w.draft(contract_with("chain io"));
    w.mother.close_ref().unwrap();
    let before = std::fs::read(w.journal_path()).unwrap();
    let home = w.dir.canonicalize().unwrap();
    seam::fail_metadata_of(Some(home));
    let reopened = w.open();
    seam::fail_metadata_of(None);
    assert!(
        matches!(reopened, Err(CoordinatorError::Private(_))),
        "a reopen whose private store's chain could not be read: {:?}",
        reopened.as_ref().err()
    );
    assert_eq!(
        std::fs::read(w.journal_path()).unwrap(),
        before,
        "the journal changed"
    );
    w.reopen_clean().unwrap();
    assert!(w.mother.state(id).is_some());
}
