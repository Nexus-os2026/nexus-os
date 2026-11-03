//! P0-A (Codex review 1 of PR #56, MEDIUM; oracle added before its fix):
//! the private store's directories are durably linked before `put`
//! returns. On a first `put`, every directory the store created and its
//! parent are synced; a directory sync that fails makes `put` fail, and
//! the reference is then never journaled. Observed through the test-only
//! seam over the store's directory syncs (`private_file::seam`), which
//! records every sync and can fail one.

use super::p0_support::*;
use super::private_file::seam;
use super::FilePrivate;
use crate::coordinator::CoordinatorError;
use crate::journal::MissionId;
use crate::oracles::contract_with;
use crate::private::{ContentKind, PrivateStore};
use crate::state::MissionState;

fn no_effects(_: MissionId) -> Vec<crate::effect::EffectRequest> {
    vec![]
}

/// Every directory a first `put` needed (here three missing levels above
/// the store, the store and its kind directory) is synced, and so is its
/// parent, before `put` returns; the blob's own directory is synced last,
/// after its rename.
#[test]
fn p0_dirsync_a_first_put_makes_every_new_directory_durable() {
    let dir = fresh_dir("dirsync");
    let a = dir.join("a");
    let b = a.join("b");
    let root = b.join("private");
    let _ = seam::take_synced();
    let store = FilePrivate::open(&root).unwrap();
    store.put(ContentKind::Contract, b"{}").unwrap();
    let synced = seam::take_synced();
    let kind = root.join("contract");
    for (new, parent) in [(&a, &dir), (&b, &a), (&root, &b), (&kind, &root)] {
        assert!(
            synced.contains(new),
            "a new directory not synced: {}: {synced:?}",
            new.display()
        );
        assert!(
            synced.contains(parent),
            "the parent of a new directory not synced: {}: {synced:?}",
            parent.display()
        );
    }
    assert_eq!(synced.last(), Some(&kind), "the blob's directory, last");

    // A second kind: its new directory and the store's.
    store.put(ContentKind::PausedPhase, b"[]").unwrap();
    let synced = seam::take_synced();
    let rest = root.join("paused_phase");
    assert!(
        synced.contains(&rest) && synced.contains(&root),
        "{synced:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// A failed directory sync fails `put`, so the coordinator journals no
/// reference; once the sync succeeds again the next `put` syncs the
/// directory's link and only then is the reference journaled.
#[test]
fn p0_dirsync_a_failed_directory_sync_fails_put_and_nothing_is_journaled() {
    let w = P0World::new("dirsync-fail", p0_lanes("r", no_effects));
    let private = w.private_dir();
    let _ = seam::take_synced();
    // The parent of the kind directory the first put creates.
    seam::fail_sync_of(Some(private.clone()));
    let drafted = w.mother.draft(contract_with("dirsync"));
    seam::fail_sync_of(None);
    assert!(
        matches!(drafted, Err(CoordinatorError::Private(_))),
        "a put whose directory link could not be synced succeeded: {drafted:?}"
    );
    assert!(w.mother.missions().is_empty());
    assert!(
        !w.journal_lines().iter().any(|l| l.contains("\"drafted\"")),
        "a reference journaled for content whose directory is not durable"
    );

    let _ = seam::take_synced();
    let id = w.draft(contract_with("dirsync"));
    let synced = seam::take_synced();
    assert!(
        synced.contains(&private) && synced.contains(&private.join("contract")),
        "the retry synced the directory's link: {synced:?}"
    );
    assert_eq!(w.mother.state(id), Some(MissionState::Draft));
}
