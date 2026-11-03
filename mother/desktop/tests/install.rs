//! Decision 2, finding 5: the install script installs this checkout's
//! binary or refuses. Run from another directory with a relative
//! `CARGO_TARGET_DIR`, where a stale `target/release/nexus-mother` waits,
//! it must refuse, and install nothing.
//!
//! Decision 3: Unix-only (it uses Unix permissions and process groups),
//! and its cleanup is the live harness's native, bounded group kill.

#![cfg(unix)]

#[path = "live/harness.rs"]
mod harness;
#[path = "../../../kernel/tests/process_leader/mod.rs"]
mod process_leader;

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

const DEADLINE: Duration = Duration::from_secs(120);
/// The script's one final cleanup: its group's signal and its reap.
const REAP: Duration = Duration::from_secs(10);
/// How long the stderr reader may take to finish after that cleanup.
const DRAIN: Duration = Duration::from_secs(10);

/// The script's end, once observed: the held leader's one final cleanup
/// (within `reap`), and the text its stderr reader collected (within
/// `drain`). Either failing is the test's failure, reported, never waited on
/// forever.
fn end_script(
    leader: process_leader::Leader,
    reader: JoinHandle<String>,
    reap: Duration,
    drain: Duration,
) -> Result<(ExitStatus, String), String> {
    // 4cb3fbeb's order, unchanged: the reader is joined, without a bound,
    // before the cleanup's result is checked.
    let _ = drain;
    let ended = leader.finish(reap);
    let stderr = reader.join().unwrap_or_default();
    let ended = ended.map_err(|e| format!("cleaning up the install script failed: {e}"))?;
    Ok((ended, stderr))
}

fn fresh(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "nexus-install-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn i1_a_relative_cargo_target_dir_from_elsewhere_installs_nothing_stale() {
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("install.sh");
    // The caller's directory, holding a stale binary at the relative path.
    let elsewhere = fresh("elsewhere");
    let stale = elsewhere.join("target/release/nexus-mother");
    std::fs::create_dir_all(stale.parent().unwrap()).unwrap();
    std::fs::write(&stale, "#!/bin/sh\necho stale\n").unwrap();
    std::fs::set_permissions(&stale, std::fs::Permissions::from_mode(0o755)).unwrap();
    let home = fresh("home");
    // The toolchain stays where it is; only HOME (the install target) moves.
    let real_home = std::env::var("HOME").expect("HOME");
    let cargo_home = std::env::var("CARGO_HOME").unwrap_or_else(|_| format!("{real_home}/.cargo"));
    let rustup_home =
        std::env::var("RUSTUP_HOME").unwrap_or_else(|_| format!("{real_home}/.rustup"));
    let mut leader = process_leader::Leader::spawn(
        Command::new("bash")
            .arg(&script)
            .current_dir(&elsewhere)
            .env("HOME", &home)
            .env("CARGO_HOME", cargo_home)
            .env("RUSTUP_HOME", rustup_home)
            .env("CARGO_TARGET_DIR", "target")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped()),
    )
    .expect("bash runs the install script");
    let mut stderr_pipe = leader.take_stderr().unwrap();
    let reader = std::thread::spawn(move || {
        let mut text = String::new();
        let _ = std::io::Read::read_to_string(&mut stderr_pipe, &mut text);
        text
    });
    // Observed without reaping; then the held leader's one final cleanup,
    // its group's signal and its reap, within one deadline.
    let until = Instant::now() + DEADLINE;
    let finished = loop {
        if leader.exited().expect("observing the install script") {
            break true;
        }
        if Instant::now() > until {
            break false;
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let (ended, stderr) = end_script(leader, reader, REAP, DRAIN).unwrap_or_else(|e| panic!("{e}"));
    let status = finished.then_some(ended);
    let installed = home.join(".local/bin/nexus-mother");
    let what = std::fs::read(&installed).ok();
    let _ = std::fs::remove_dir_all(&elsewhere);
    let _ = std::fs::remove_dir_all(&home);
    let status = status.expect("the install script did not finish within its deadline");
    assert!(
        what.as_deref() != Some(b"#!/bin/sh\necho stale\n".as_slice()),
        "the script installed the caller's stale binary"
    );
    assert!(
        what.is_none(),
        "the script installed something it should have refused"
    );
    assert!(!status.success(), "the script did not refuse ({status})");
    assert!(
        stderr.contains("CARGO_TARGET_DIR") && stderr.contains("absolute"),
        "the refusal does not say why: {stderr}"
    );
}

// ── Codex re-check 4: the end's cleanup error, and its bounded reader ────

/// A held leader that sleeps, writing nothing to the stderr it is given.
fn sleeper(stderr: std::io::PipeWriter) -> process_leader::Leader {
    process_leader::Leader::spawn(
        Command::new("sleep")
            .arg("60")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(stderr),
    )
    .expect("sleep runs")
}

/// `end_script` for a stand-in script whose stderr a second held leader, the
/// holder, keeps open, run on its own thread and waited for at most `bound`
/// (`None`: it had not returned). `inject`: every held signal of the end is
/// reported as failed (the leader module's test-only seam). Then the
/// holder's own final cleanup closes the pipe's last write end, which
/// releases a reader still waiting on it.
fn end_with_stderr_held_open(
    inject: bool,
    drain: Duration,
    bound: Duration,
) -> Option<Result<(ExitStatus, String), String>> {
    let (mut read_end, write_end) = std::io::pipe().expect("a pipe");
    let script = sleeper(write_end.try_clone().expect("a second write end"));
    let holder = sleeper(write_end);
    let reader = std::thread::spawn(move || {
        let mut text = String::new();
        let _ = std::io::Read::read_to_string(&mut read_end, &mut text);
        text
    });
    let (sent, received) = std::sync::mpsc::channel();
    let ending = std::thread::spawn(move || {
        let ended = if inject {
            let (ended, signalled) = process_leader::recording_with_failed_signals(|| {
                end_script(script, reader, REAP, drain)
            });
            assert!(
                signalled.len() == 1 && signalled.iter().all(|s| s.held && s.leader_present),
                "only the held script's group may be signalled: {signalled:?}"
            );
            ended
        } else {
            end_script(script, reader, REAP, drain)
        };
        let _ = sent.send(ended);
    });
    let ended = received.recv_timeout(bound).ok();
    let held = holder.finish(REAP);
    let until = Instant::now() + REAP;
    while !ending.is_finished() && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(10));
    }
    held.unwrap_or_else(|e| panic!("the holder's final cleanup failed: {e}"));
    assert!(
        ending.is_finished(),
        "the end never returned, even after the holder's cleanup"
    );
    if let Err(panic) = ending.join() {
        std::panic::resume_unwind(panic);
    }
    ended
}

// The recording seam reads /proc.
#[cfg(target_os = "linux")]
#[test]
fn i2_a_failed_cleanup_is_reported_at_once_while_stderr_stays_open() {
    // The final signal is reported as failed while the script's stderr
    // stays open: the cleanup error must reach the test within five
    // seconds, before any wait on the reader (whose own bound here is a
    // minute).
    let ended = end_with_stderr_held_open(true, Duration::from_secs(60), Duration::from_secs(5))
        .expect(
        "the cleanup error was not reported within 5 s: the end waited on the open stderr first",
    );
    let error = ended.expect_err("a failed final signal was reported as a clean end");
    assert!(
        error.contains("cleaning up the install script failed") && error.contains("injected"),
        "the end did not report the cleanup error: {error}"
    );
}

#[test]
fn i3_stderr_still_open_after_the_cleanup_fails_within_its_bound() {
    // The script's group is signalled and its leader reaped, but the
    // holder keeps its stderr open: the reader's join has a bound (one
    // second here), and overrunning it is a failure, never a wait forever.
    let ended = end_with_stderr_held_open(false, Duration::from_secs(1), Duration::from_secs(10))
        .expect("the end still waited on the open stderr after 10 s");
    let error = ended.expect_err("a stderr left open was reported as a clean end");
    assert!(
        error.contains("stderr") && error.contains("still open"),
        "the end did not report the open stderr: {error}"
    );
}
