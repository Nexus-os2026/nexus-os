//! P0-A (Codex review 3 of PR #56, two MEDIUMs on directory durability;
//! oracles added before the Architect's replacement rule): every put makes
//! durable the link of every directory Mother may have created above the
//! blob, whoever created it and however the store's path was spelled.
//! Here `base` is open to others (0755), so the directories Mother creates
//! start at `base/a`, and `base` is the parent whose sync makes `a`'s link
//! durable.

use super::p0_support::*;
use super::private_file::seam;
use super::FilePrivate;
use crate::private::{ContentKind, PrivateStore};
use std::path::{Path, PathBuf};

/// `base`, not private (0755), so that `a` is the highest directory Mother
/// creates.
#[cfg(unix)]
fn open_base(label: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let base = fresh_dir(label);
    std::fs::set_permissions(&base, std::fs::Permissions::from_mode(0o755)).unwrap();
    base
}

/// (a) Opener A creates `base/a` and is paused there, before anything else
/// it does. Opener B opens `base/a/b/private` and puts: B's put must sync
/// `base` before it returns, though B did not create `a`.
#[cfg(unix)]
#[test]
fn p0_chain_a_put_syncs_the_link_of_a_directory_another_opener_created() {
    let _scenario = seam::pause_scenario();
    let base = open_base("chain-race");
    let a = base.join("a");
    seam::pause_after_creating(Some(a.clone()));
    let root_a = a.join("x").join("private");
    let opener_a = std::thread::spawn(move || FilePrivate::open(&root_a).map(|_| ()));
    assert!(seam::wait_paused(), "A paused just after creating a");

    let store_b = FilePrivate::open(&a.join("b").join("private"));
    let _ = seam::take_synced();
    let put = store_b
        .as_ref()
        .map_err(|e| *e)
        .and_then(|s| s.put(ContentKind::Contract, b"{}"));
    let synced = seam::take_synced();
    seam::release();
    let _ = opener_a.join();
    assert!(put.is_ok(), "B's put: {put:?}");
    assert!(
        synced.contains(&base.canonicalize().unwrap()),
        "B's put returned before the link of `a` (A's) was synced: {synced:?}"
    );
    let _ = std::fs::remove_dir_all(&base);
}

/// Restores the working directory however the test ends.
struct Cwd(PathBuf);

impl Drop for Cwd {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.0);
    }
}

/// (b) An open through the absolute path creates `a`, and its sync of
/// `base` fails. The retry spells the same store otherwise (relative to
/// another working directory; through `x/..`): its put must sync `base`.
#[cfg(unix)]
#[test]
fn p0_chain_a_retry_by_another_spelling_syncs_the_ancestor() {
    let fail_first = |base: &Path| {
        let root = base.join("a").join("b").join("private");
        seam::fail_sync_of(Some(base.to_path_buf()));
        let first = FilePrivate::open(&root);
        seam::fail_sync_of(None);
        assert!(first.is_err(), "the open succeeded with `a` not linked");
        assert!(base.join("a").exists());
    };

    // Relative to another working directory.
    let base = open_base("chain-cwd");
    fail_first(&base);
    let (put, synced) = {
        let _back = Cwd(std::env::current_dir().unwrap());
        std::env::set_current_dir(&base).unwrap();
        let store = FilePrivate::open(Path::new("a/b/private")).unwrap();
        let _ = seam::take_synced();
        let put = store.put(ContentKind::Contract, b"{}");
        (put, seam::take_synced())
    };
    assert!(put.is_ok(), "{put:?}");
    assert!(
        synced.contains(&base.canonicalize().unwrap()),
        "the retry's put (relative path) did not sync base: {synced:?}"
    );
    let _ = std::fs::remove_dir_all(&base);

    // Through `x/..`.
    let base = open_base("chain-dotdot");
    fail_first(&base);
    std::fs::create_dir(base.join("x")).unwrap();
    let store = FilePrivate::open(
        &base
            .join("x")
            .join("..")
            .join("a")
            .join("b")
            .join("private"),
    )
    .unwrap();
    let _ = seam::take_synced();
    let put = store.put(ContentKind::Contract, b"{}");
    let synced = seam::take_synced();
    assert!(put.is_ok(), "{put:?}");
    assert!(
        synced.contains(&base.canonicalize().unwrap()),
        "the retry's put (x/..) did not sync base: {synced:?}"
    );
    let _ = std::fs::remove_dir_all(&base);
}
