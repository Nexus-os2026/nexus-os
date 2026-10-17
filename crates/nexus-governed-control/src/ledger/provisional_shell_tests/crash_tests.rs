//! A real process crash on a real file: assumptions A-1 and A-2 of
//! CONTRACT-02-R1 section 4.1, exercised for a killed process. This is not a
//! discharge of either assumption: a process killed by SIGKILL leaves the
//! operating system's page cache intact, so nothing here speaks to a power
//! loss or to a device that does not honour a sync.
//!
//! The parent re-runs this test binary as a child, filtered to the ignored
//! test `crash_child`, which reads its task from the environment. The child
//! reports `READY` on its standard output once it has reached the point to
//! be killed at, and then waits; the parent kills it through its own `Child`
//! handle (never by name or by a guessed process id), reaps it, and opens
//! the file.

use super::*;
use std::io::{BufRead, BufReader, Write as _};
use std::process::{Command, Stdio};
use std::time::Duration;

const TASK: &str = "NEXUS_LEDGER_SHELL_CRASH_TASK";

/// Kills the child once it reports `READY`; its standard output is read on a
/// thread so that the wait for `READY` is bounded.
fn run_child_and_kill(mode: &str, path: &Path) -> String {
    let exe = std::env::current_exe().expect("the test binary");
    let mut child = Command::new(exe)
        .args([
            "--exact",
            "ledger::provisional_shell_tests::crash_tests::crash_child",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(TASK, format!("{mode}\n{}", path.display()))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("the child starts");
    let stdout = child.stdout.take().expect("piped");
    let (tx, rx) = std::sync::mpsc::channel();
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            // libtest may have begun the line with the test's name.
            if let Some(at) = line.find("READY ") {
                let _ = tx.send(line[at + "READY ".len()..].trim().to_string());
                break;
            }
        }
    });
    let ready = rx.recv_timeout(Duration::from_secs(120));
    child.kill().expect("the child is killed");
    let status = child.wait().expect("the child is reaped");
    let _ = reader.join();
    let found = ready.expect("the child reported READY");
    assert!(!status.success(), "the child was killed, not finished");
    found
}

fn report(line: &str) {
    let mut out = std::io::stdout().lock();
    writeln!(out, "READY {line}").unwrap();
    out.flush().unwrap();
}

/// The child's side. Run only by `run_child_and_kill`; without its task it
/// does nothing.
#[test]
#[ignore = "run by the crash tests as a child process"]
fn crash_child() {
    let Ok(task) = std::env::var(TASK) else {
        return;
    };
    let (mode, path) = task.split_once('\n').expect("mode and path");
    let path = Path::new(path);
    let mut store = Store::open(path, LIMITS).expect("the child opens the store");
    assert_eq!(store.open_session(), Opening::Opened);
    let built = store.fact(b"crash", None, b"content");
    let key = built.key();
    let line = format!("{}{}", hex::encode(key.id), hex::encode(key.digest));
    match mode {
        // An insert issued, the transaction neither committed nor rolled back.
        "before-commit" => {
            let staged = [Staged {
                member: *built.record().member(),
                draft: built.draft.clone(),
            }];
            let mut io = AppendIo {
                conn: &store.conn,
                store: store.identity(),
                staged: &staged,
            };
            assert_eq!(Txn::open(&mut io), Read::Got(()));
            assert!(matches!(
                Txn::insert(&mut io, 0, built.record().member()),
                Insert::Inserted(_)
            ));
            report(&line);
        }
        // The core answered Committed: COMMIT returned success.
        "after-commit" => {
            let mut ticket = store.health().arm().unwrap();
            assert_eq!(
                store.append_facts(&mut ticket, &[built]).unwrap(),
                FactOutcome::Committed
            );
            report(&line);
            std::mem::forget(ticket);
        }
        other => panic!("unknown mode {other}"),
    }
    // Wait to be killed; bounded, so a lost parent does not leave it forever.
    for _ in 0..1200 {
        std::thread::sleep(Duration::from_millis(100));
    }
    std::mem::forget(store);
}

fn key_from(line: &str) -> RecordKey {
    let bytes = hex::decode(line).expect("hex");
    RecordKey {
        id: bytes[..32].try_into().unwrap(),
        digest: bytes[32..].try_into().unwrap(),
    }
}

fn fresh_store(scratch: &Scratch) -> PathBuf {
    let path = scratch.path("l.db");
    drop(Store::create(&path, LIMITS).unwrap());
    path
}

/// A-1 for a crash: an insert whose transaction was never committed leaves
/// nothing once the process is gone, and the file opens with its chain whole.
#[test]
fn a_process_killed_before_commit_leaves_nothing() {
    let scratch = Scratch::new();
    let path = fresh_store(&scratch);
    let key = key_from(&run_child_and_kill("before-commit", &path));
    let store = Store::open(&path, LIMITS).expect("the file opens after the crash");
    assert!(matches!(
        by_id(&store.conn, &store.identity(), &key.id),
        Read::Got(None)
    ));
}

/// A-2 for a crash: what the core reported Committed is in the file after the
/// process is killed.
#[test]
fn a_process_killed_after_commit_keeps_the_record() {
    let scratch = Scratch::new();
    let path = fresh_store(&scratch);
    let key = key_from(&run_child_and_kill("after-commit", &path));
    let store = Store::open(&path, LIMITS).expect("the file opens after the crash");
    match by_id(&store.conn, &store.identity(), &key.id) {
        Read::Got(Some(row)) => assert_eq!(row.digest, key.digest),
        other => panic!("{other:?}"),
    }
}
