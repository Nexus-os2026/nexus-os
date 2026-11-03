//! M2b-A PR-1, Architect decision 1 (Codex review 1 of PR #52): G0's tree
//! scan sees symlinked desktop paths and never drops a traversal error.
//! Registered before the fix they judge; the frozen G0 oracles
//! (`desktop_tripwire_oracles.rs`) are unchanged.
//!
//! Each plant is made on a fresh copy of G0's real offline Cargo fixture,
//! never in the repository itself (a checked-in plant would trip the real
//! workspace tripwire), and the whole helper (`findings`: both metadata
//! configurations and the tree) runs on it under a deadline, so a hang
//! fails instead of stalling the suite. The fixture's own `webkit6`
//! finding is always present there, so each oracle asserts its own
//! finding.

#[path = "desktop_tripwire/mod.rs"]
mod desktop_tripwire;

use desktop_tripwire::findings;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

/// Longer than two `cargo metadata` runs of the fixture and a tree walk.
const DEADLINE: Duration = Duration::from_secs(120);

/// A private temporary directory, removed (permissions restored) on drop.
struct Temp(PathBuf);

impl Temp {
    fn new(label: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "nexus-g0-symlink-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        fn open_up(dir: &Path) {
            let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                        open_up(&entry.path());
                    }
                }
            }
        }
        open_up(&self.0);
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).unwrap();
        }
    }
}

/// A copy of G0's real offline Cargo fixture.
fn fixture_copy(label: &str) -> Temp {
    let temp = Temp::new(label);
    let real = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/desktop_tripwire/fixture");
    copy_tree(&real, &temp.0);
    temp
}

/// The helper's findings for `root`, or a failure if it does not return
/// within the deadline.
fn findings_within_deadline(root: &Path) -> Vec<String> {
    let (tx, rx) = mpsc::channel();
    let root = root.to_path_buf();
    std::thread::spawn(move || {
        let _ = tx.send(findings(&root));
    });
    rx.recv_timeout(DEADLINE)
        .unwrap_or_else(|_| panic!("the tripwire did not return within {DEADLINE:?}"))
}

/// A directory outside the fixture, holding a tree.
fn outside_tree(label: &str) -> Temp {
    let outside = Temp::new(label);
    std::fs::create_dir_all(outside.0.join("src")).unwrap();
    std::fs::write(outside.0.join("src/main.rs"), "fn main() {}\n").unwrap();
    outside
}

#[test]
fn g0_s1_a_symlink_named_src_tauri_to_an_outside_tree_is_red() {
    let root = fixture_copy("src-tauri");
    let outside = outside_tree("src-tauri-target");
    std::fs::create_dir_all(root.0.join("tools")).unwrap();
    symlink(&outside.0, root.0.join("tools/src-tauri")).unwrap();
    let found = findings_within_deadline(&root.0);
    assert!(
        found
            .iter()
            .any(|f| f.contains("tools/src-tauri") && f.contains("src-tauri")),
        "a symlinked src-tauri escaped the scan: {found:?}"
    );
}

#[test]
fn g0_s2_a_symlink_named_app_is_red() {
    // Both an `app` pointing at a tree outside the workspace and one whose
    // target is not there at the time of the check.
    for dangling in [false, true] {
        let root = fixture_copy("app");
        let outside = outside_tree("app-target");
        let target = if dangling {
            outside.0.join("gone")
        } else {
            outside.0.clone()
        };
        symlink(&target, root.0.join("app")).unwrap();
        let found = findings_within_deadline(&root.0);
        assert!(
            found.iter().any(|f| f.starts_with("app")),
            "a symlinked app escaped the scan (dangling: {dangling}): {found:?}"
        );
    }
}

#[test]
fn g0_s3_a_symlinked_mother_desktop_is_red() {
    let root = fixture_copy("desktop");
    let outside = outside_tree("desktop-target");
    std::fs::create_dir_all(root.0.join("mother")).unwrap();
    symlink(outside.0.join("gone"), root.0.join("mother/desktop")).unwrap();
    let found = findings_within_deadline(&root.0);
    assert!(
        found.iter().any(|f| f.contains("mother/desktop")),
        "a symlinked mother/desktop escaped the scan: {found:?}"
    );
}

#[test]
fn g0_s4_a_symlink_loop_neither_hangs_nor_passes_silently() {
    let root = fixture_copy("loop");
    std::fs::create_dir_all(root.0.join("tools")).unwrap();
    symlink(root.0.join("tools/loop-b"), root.0.join("tools/loop-a")).unwrap();
    symlink(root.0.join("tools/loop-a"), root.0.join("tools/loop-b")).unwrap();
    // A loop through a parent directory too.
    symlink(&root.0, root.0.join("tools/up")).unwrap();
    let found = findings_within_deadline(&root.0);
    assert!(
        found.iter().any(|f| f.contains("tools/loop-a")),
        "a symlink loop passed silently: {found:?}"
    );
}

#[test]
fn g0_s5_an_unreadable_directory_is_red_with_a_clear_error() {
    let root = fixture_copy("unreadable");
    let sealed = root.0.join("tools/sealed");
    std::fs::create_dir_all(&sealed).unwrap();
    std::fs::write(sealed.join("inside.rs"), "\n").unwrap();
    std::fs::set_permissions(&sealed, std::fs::Permissions::from_mode(0o000)).unwrap();
    assert!(
        std::fs::read_dir(&sealed).is_err(),
        "this plant needs a user who cannot read a 0o000 directory"
    );
    let found = findings_within_deadline(&root.0);
    assert!(
        found
            .iter()
            .any(|f| f.contains("tools/sealed") && f.contains("cannot read")),
        "an unreadable directory was skipped: {found:?}"
    );
}
