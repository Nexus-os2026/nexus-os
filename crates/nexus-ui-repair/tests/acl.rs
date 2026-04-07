//! I-2 enforcement test. The single Phase 1.1 test that does real work.
//!
//! Verifies that `Acl::default_scout()` permits writes under the two
//! whitelisted roots and denies writes anywhere else, with the denials
//! returning `Error::AclDenied` (not `Io` or any other variant).
//!
//! All four cases live inside one `#[test]` function because they
//! mutate the process-wide `HOME` environment variable; running them as
//! separate parallel tests would race.

use std::path::PathBuf;

use nexus_ui_repair::governance::acl::Acl;
use nexus_ui_repair::Error;
use tempfile::tempdir;

#[test]
fn default_scout_acl_enforces_write_roots() {
    let home_dir = tempdir().expect("create temp HOME");
    let home_path = home_dir.path().to_path_buf();

    // Build the four directories the test cares about so canonicalize
    // succeeds against them.
    let reports = home_path.join(".nexus").join("ui-repair").join("reports");
    let sessions = home_path.join(".nexus").join("ui-repair").join("sessions");
    let other = home_path.join(".nexus").join("something_else");
    std::fs::create_dir_all(&reports).expect("mkdir reports");
    std::fs::create_dir_all(&sessions).expect("mkdir sessions");
    std::fs::create_dir_all(&other).expect("mkdir other");

    // Point HOME at the temp dir for the duration of the assertions.
    // Safety: this test is single-threaded with respect to HOME — there
    // are no other tests in this binary that touch the variable.
    std::env::set_var("HOME", &home_path);
    let acl = Acl::default_scout();

    // (a) writes under reports/ are permitted.
    let report_target = reports.join("teams.md");
    acl.check_write(&report_target)
        .expect("reports/ write must be allowed");

    // (b) writes under sessions/ are permitted.
    let session_target = sessions.join("audit.jsonl");
    acl.check_write(&session_target)
        .expect("sessions/ write must be allowed");

    // (c) /etc/passwd must be denied with AclDenied (not any other err).
    let etc_passwd = PathBuf::from("/etc/passwd");
    match acl.check_write(&etc_passwd) {
        Err(Error::AclDenied(p)) => assert_eq!(p, etc_passwd),
        Err(other) => panic!("expected AclDenied for /etc/passwd, got {:?}", other),
        Ok(()) => panic!("/etc/passwd must not be writable"),
    }

    // (d) ~/.nexus/something_else/ must be denied with AclDenied.
    let other_target = other.join("oops.txt");
    match acl.check_write(&other_target) {
        Err(Error::AclDenied(p)) => assert_eq!(p, other_target),
        Err(other) => panic!(
            "expected AclDenied for ~/.nexus/something_else/, got {:?}",
            other
        ),
        Ok(()) => panic!("~/.nexus/something_else/ must not be writable"),
    }
}
