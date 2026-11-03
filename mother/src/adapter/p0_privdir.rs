//! P0-A (Codex review 2 of PR #56, two MEDIUMs; oracles added before their
//! fix):
//! - a directory the store created stays owed a durable link until its
//!   parent's sync succeeds: a retry after a failed ancestor sync syncs
//!   that ancestor again before it succeeds (never inferred from the
//!   directory existing);
//! - every put validates the store's directory and the kind directory
//!   (a directory, not a link, no access for others), whatever the store
//!   remembers about their durability.

use super::p0_support::*;
use super::private_file::seam;
use super::FilePrivate;
use crate::private::{ContentKind, PrivateError, PrivateStore};

/// Opening `base/a/b/private` creates `a`, then the sync of `base` (the
/// link of `a`) fails: the open fails. The retry finds `a` existing; it
/// must still sync `base` before it succeeds.
#[test]
fn p0_privdir_a_retry_syncs_the_ancestor_whose_sync_failed() {
    let base = fresh_dir("privdir-retry");
    let root = base.join("a").join("b").join("private");
    let _ = seam::take_synced();
    seam::fail_sync_of(Some(base.clone()));
    let first = FilePrivate::open(&root);
    seam::fail_sync_of(None);
    assert!(first.is_err(), "the open succeeded with `a` not linked");
    assert!(base.join("a").exists(), "the partial creation happened");

    let _ = seam::take_synced();
    let store = FilePrivate::open(&root).expect("the retry opens");
    let synced = seam::take_synced();
    assert!(
        synced.contains(&base),
        "the retry did not sync the ancestor whose sync failed: {synced:?}"
    );
    store.put(ContentKind::Contract, b"{}").unwrap();
    let _ = std::fs::remove_dir_all(&base);
}

/// After a successful put, the kind directory replaced by a link to a
/// directory outside the store: the next put is refused and writes nothing
/// there.
#[cfg(unix)]
#[test]
fn p0_privdir_a_linked_kind_directory_is_refused_on_every_put() {
    let dir = fresh_dir("privdir-link");
    let root = dir.join("private");
    let store = FilePrivate::open(&root).unwrap();
    store.put(ContentKind::Contract, b"one").unwrap();
    let outside = dir.join("outside");
    super::create_private(&outside).unwrap();
    std::fs::rename(root.join("contract"), dir.join("contract-moved")).unwrap();
    std::os::unix::fs::symlink(&outside, root.join("contract")).unwrap();
    assert_eq!(
        store.put(ContentKind::Contract, b"two"),
        Err(PrivateError::NotPrivate)
    );
    assert_eq!(
        std::fs::read_dir(&outside).unwrap().count(),
        0,
        "a put wrote through the link"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// After a successful put, the kind directory (and, separately, the
/// store's directory) opened to others: the next put is refused.
#[cfg(unix)]
#[test]
fn p0_privdir_a_relaxed_directory_is_refused_on_every_put() {
    use std::os::unix::fs::PermissionsExt;
    let relax = |path: &std::path::Path| {
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap()
    };

    let dir = fresh_dir("privdir-kind");
    let root = dir.join("private");
    let store = FilePrivate::open(&root).unwrap();
    store.put(ContentKind::Contract, b"one").unwrap();
    relax(&root.join("contract"));
    assert_eq!(
        store.put(ContentKind::Contract, b"two"),
        Err(PrivateError::NotPrivate),
        "the kind directory"
    );
    let _ = std::fs::remove_dir_all(&dir);

    let dir = fresh_dir("privdir-root");
    let root = dir.join("private");
    let store = FilePrivate::open(&root).unwrap();
    store.put(ContentKind::Contract, b"one").unwrap();
    relax(&root);
    assert_eq!(
        store.put(ContentKind::Contract, b"two"),
        Err(PrivateError::NotPrivate),
        "the store's directory"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
