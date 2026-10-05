//! The session launcher, live: what a session process inherits, and how it
//! ends.

use super::{SessionProcess, SessionSpec};
use crate::harness_tests::temp_root;
use std::os::fd::AsRawFd;
use std::path::PathBuf;
use std::time::{Duration, Instant};

fn shell(script: String, dir: PathBuf, stop_grace: Option<Duration>) -> SessionProcess {
    SessionProcess::launch(SessionSpec {
        program: std::fs::canonicalize("/bin/sh").unwrap(),
        args: vec!["-c".into(), script.into()],
        env: vec![("PATH".into(), "/usr/bin:/bin".into())],
        current_dir: dir,
        stop_grace,
        inherit: vec![],
    })
    .unwrap()
}

/// Only the standard descriptors (and the listed ones) reach a session
/// process: one this process holds without close-on-exec does not.
#[test]
fn a_session_process_inherits_no_other_descriptor() {
    let root = temp_root("launcher");
    let dir = root.0.path().to_path_buf();
    let held = std::fs::File::open("/dev/null").unwrap();
    // A descriptor without close-on-exec, the way a C library might open
    // one, at the lowest free number from 300.
    // SAFETY: F_DUPFD duplicates a descriptor this test owns.
    let leaked = unsafe { libc::fcntl(held.as_raw_fd(), libc::F_DUPFD, 300) };
    assert!(leaked >= 300);
    let out = dir.join("descriptors");
    let mut process = shell(format!("ls /proc/self/fd > {}", out.display()), dir, None);
    let deadline = Instant::now() + Duration::from_secs(10);
    while process.running() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    process.end();
    // SAFETY: closes the descriptor this test made.
    unsafe {
        libc::close(leaked);
    }
    let listed: Vec<i32> = std::fs::read_to_string(&out)
        .unwrap()
        .split_whitespace()
        .map(|fd| fd.parse().unwrap())
        .collect();
    assert!(listed.contains(&1), "{listed:?}");
    assert!(!listed.contains(&leaked), "{listed:?}");
}

/// With a grace the group is asked to stop and the leader may clean up;
/// without one it is killed at once. Either way it ends and is reaped.
#[test]
fn a_grace_lets_the_leader_clean_up_and_none_does_not() {
    for (grace, cleaned) in [(Some(Duration::from_secs(2)), true), (None, false)] {
        let root = temp_root("launcher");
        let dir = root.0.path().to_path_buf();
        let flag = dir.join("cleaned");
        let script = format!(
            "trap 'echo > {}; exit 0' TERM; while :; do sleep 0.05; done",
            flag.display()
        );
        let mut process = shell(script, dir, grace);
        std::thread::sleep(Duration::from_millis(300));
        assert!(process.running());
        process.end();
        assert!(!process.running());
        assert_eq!(flag.exists(), cleaned, "{grace:?}");
    }
}
