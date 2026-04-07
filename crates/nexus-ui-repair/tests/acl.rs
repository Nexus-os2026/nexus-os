//! I-2 Layer 1 enforcement test. See v1.1 §3.1.
//!
//! All cases live inside a single `#[test]` function because they
//! mutate the process-wide `HOME` environment variable; running them as
//! separate parallel tests would race.
//!
//! Original four cases from Phase 1.1 plus the Phase 1.2 additions:
//!
//!   a) PERMIT: reports/<date>/teams.md
//!   b) PERMIT: sessions/<ses>/audit.jsonl
//!   c) DENY:   /etc/passwd
//!   d) DENY:   ~/.nexus/something_else/file.md
//!   e) DENY:   path with `..` traversal components
//!   f) PERMIT: in-bounds path whose parent does not yet exist
//!   g) DENY:   out-of-bounds path whose parent does not yet exist
//!   h) PERMIT: ensure_parent_dirs succeeds and actually creates the dir
//!   i) DENY:   ensure_parent_dirs refuses to create out-of-bounds dir

use std::path::PathBuf;

use nexus_ui_repair::governance::acl::Acl;
use nexus_ui_repair::Error;
use tempfile::tempdir;

/// Guard that restores the original `HOME` environment variable when
/// dropped. Ensures the test does not pollute subsequent tests in the
/// same binary (unlikely — there is only one test here — but required
/// if a panic mid-test would otherwise leak the tempdir path).
struct HomeGuard {
    original: Option<String>,
}

impl HomeGuard {
    fn set(new_home: &std::path::Path) -> Self {
        let original = std::env::var("HOME").ok();
        std::env::set_var("HOME", new_home);
        Self { original }
    }
}

impl Drop for HomeGuard {
    fn drop(&mut self) {
        match &self.original {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }
    }
}

fn assert_denied(result: nexus_ui_repair::Result<()>, label: &str) {
    match result {
        Err(Error::AclDenied(_)) => {}
        Err(other) => panic!("expected AclDenied for {}, got {:?}", label, other),
        Ok(()) => panic!("{} must be denied", label),
    }
}

#[test]
fn default_scout_acl_enforces_write_roots() {
    let home_dir = tempdir().expect("create temp HOME");
    let home_path = home_dir.path().to_path_buf();

    // Build the two allowed roots so canonicalize() inside check_write
    // can resolve them. (The sandbox is permitted to write to children
    // whose parents don't yet exist, but the *root* must exist for the
    // canonicalization logic to lock down the prefix against symlink
    // tricks; first-run writes are covered by the lexical-fallback path
    // in Acl::check_write and tested separately below.)
    let reports = home_path.join(".nexus").join("ui-repair").join("reports");
    let sessions = home_path.join(".nexus").join("ui-repair").join("sessions");
    let other = home_path.join(".nexus").join("something_else");
    std::fs::create_dir_all(&reports).expect("mkdir reports");
    std::fs::create_dir_all(&sessions).expect("mkdir sessions");
    std::fs::create_dir_all(&other).expect("mkdir other");

    let _guard = HomeGuard::set(&home_path);
    let acl = Acl::default_scout();

    // (a) PERMIT: reports/<date>/teams.md — parent doesn't exist yet
    // but is inside an allowed root.
    let a = reports.join("2026-04-07").join("teams.md");
    acl.check_write(&a)
        .expect("(a) reports/<date>/teams.md must be permitted");

    // (b) PERMIT: sessions/<ses>/audit.jsonl.
    let b = sessions.join("ses_abc").join("audit.jsonl");
    acl.check_write(&b)
        .expect("(b) sessions/<ses>/audit.jsonl must be permitted");

    // (c) DENY: /etc/passwd.
    let c = PathBuf::from("/etc/passwd");
    assert_denied(acl.check_write(&c), "/etc/passwd");

    // (d) DENY: ~/.nexus/something_else/file.md.
    let d = other.join("file.md");
    assert_denied(acl.check_write(&d), "~/.nexus/something_else/file.md");

    // (e) DENY (NEW): path with `..` traversal components. Must be
    // rejected as AclDenied *before* canonicalize has a chance to turn
    // it into an Io error.
    let e = reports
        .join("..")
        .join("..")
        .join("..")
        .join("etc")
        .join("passwd");
    assert_denied(acl.check_write(&e), "path with `..` traversal");

    // (f) PERMIT (NEW): in-bounds write target whose parent doesn't yet
    // exist. The 2026-04-07 directory is not present.
    let f = reports.join("2026-04-07").join("new-page-never-seen.md");
    assert!(!f.parent().unwrap().exists());
    acl.check_write(&f)
        .expect("(f) in-bounds missing-parent path must be permitted");

    // (g) DENY (NEW): out-of-bounds write target whose parent does not
    // yet exist.
    let g = home_path
        .join(".nexus")
        .join("other_thing")
        .join("2026-04-07")
        .join("file.md");
    assert_denied(acl.check_write(&g), "(g) out-of-bounds missing-parent");

    // (h) PERMIT (NEW): ensure_parent_dirs actually creates the parent
    // directory for an in-bounds path.
    let h = reports.join("2026-04-07-ensured").join("page.md");
    assert!(!h.parent().unwrap().exists());
    acl.ensure_parent_dirs(&h)
        .expect("(h) ensure_parent_dirs must succeed in-bounds");
    assert!(
        h.parent().unwrap().exists(),
        "(h) parent directory must exist after ensure_parent_dirs"
    );

    // (i) DENY (NEW): ensure_parent_dirs refuses to create a parent
    // outside the sandbox.
    let i = home_path
        .join(".nexus")
        .join("other_thing_2")
        .join("page.md");
    let i_parent = i.parent().unwrap().to_path_buf();
    assert!(!i_parent.exists());
    assert_denied(
        acl.ensure_parent_dirs(&i),
        "(i) ensure_parent_dirs out-of-bounds",
    );
    assert!(
        !i_parent.exists(),
        "(i) parent directory must NOT be created after denied ensure_parent_dirs"
    );
}
