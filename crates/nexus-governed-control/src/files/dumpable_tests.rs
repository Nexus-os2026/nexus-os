//! M2d-C (the Architect's decision 1): `files::make_non_dumpable`, the host's
//! startup step for Files, leaves the process non-dumpable: `PR_GET_DUMPABLE`
//! reads 0 after the call. Read in a held child of this test binary, never
//! in the parent (other tests here make the parent non-dumpable); the child
//! starts dumpable (the control). The call only restricts the process; it
//! grants nothing. Pre-registered before the function exists.

use nexus_kernel::emergency_stop::{ChildEnd, GuardedChild, HaltArea, StopAuthority};
use std::io::BufRead as _;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

const MODE: &str = "NEXUS_M2D_C_DUMPABLE_CHILD";
const WAIT: Duration = Duration::from_secs(30);

/// `PR_GET_DUMPABLE`, read directly.
#[allow(unsafe_code)]
fn dumpable() -> libc::c_int {
    // SAFETY: PR_GET_DUMPABLE takes no further argument and only reads a
    // flag of this process.
    unsafe { libc::prctl(libc::PR_GET_DUMPABLE) }
}

/// The child side; a no-op unless run by the oracle below.
#[test]
#[ignore]
fn m2d_c_dumpable_child() {
    if std::env::var(MODE).is_err() {
        return;
    }
    println!("BEFORE {}", dumpable());
    println!("MADE {}", super::make_non_dumpable());
    println!("AFTER {}", dumpable());
}

fn line(lines: &mpsc::Receiver<String>, prefix: &str) -> String {
    loop {
        let line = lines
            .recv_timeout(WAIT)
            .unwrap_or_else(|_| panic!("the child never said {prefix}"));
        // The harness may print its own text before ours on a line.
        if let Some((_, rest)) = line.split_once(&format!("{prefix} ")) {
            return rest.trim().to_string();
        }
    }
}

/// A fresh child of this binary starts dumpable (1); after the call it
/// says it is non-dumpable, and `PR_GET_DUMPABLE` reads 0.
#[test]
fn m2d_c_make_non_dumpable_leaves_the_process_non_dumpable() {
    let authority = StopAuthority::new();
    let watch = authority.admit().unwrap();
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            "files::dumpable_tests::m2d_c_dumpable_child",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(MODE, "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let (child, pipes) = GuardedChild::spawn(&mut command, &watch, HaltArea::Coding).unwrap();
    let stdout = pipes.stdout.unwrap();
    let (send, lines) = mpsc::channel();
    std::thread::spawn(move || {
        for line in std::io::BufReader::new(stdout)
            .lines()
            .map_while(Result::ok)
        {
            if send.send(line).is_err() {
                break;
            }
        }
    });
    assert_eq!(line(&lines, "BEFORE"), "1", "the child must start dumpable");
    assert_eq!(line(&lines, "MADE"), "true");
    assert_eq!(line(&lines, "AFTER"), "0");
    assert!(matches!(child.finish(), ChildEnd::Exited(status) if status.success()));
}
