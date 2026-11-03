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
//!
//! Decision 2: these oracles plant Unix symlinks and permissions, so they
//! are Unix-only (L3); the helper runs in an owned child process, killed
//! and reaped with everything it started if it overruns (L2). Decision 3:
//! the group is signalled natively through the kernel's `nix`, which the
//! kernel has on Linux and macOS only, so the oracles are gated to those.

#![cfg(any(target_os = "linux", target_os = "macos"))]

#[path = "desktop_tripwire/mod.rs"]
mod desktop_tripwire;

use desktop_tripwire::findings;
use std::io::Read;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

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

/// The helper role's variables: the root to check, and (the L2 plant) a
/// file to write a long-running grandchild's process id to before hanging.
const HELPER_ROOT: &str = "NEXUS_G0_HELPER_ROOT";
const HELPER_HANG: &str = "NEXUS_G0_HELPER_HANG";

/// How long a killed helper's group may take to be reaped.
const REAP_WITHIN: Duration = Duration::from_secs(10);

/// The helper role. A no-op unless `findings_within` re-runs this test
/// binary for it, in its own process group: then it prints the helper's
/// findings, one per line.
#[test]
fn g0_helper_role() {
    let Some(root) = std::env::var_os(HELPER_ROOT) else {
        return;
    };
    if let Some(pidfile) = std::env::var_os(HELPER_HANG) {
        let mut sleeper = Command::new("sleep").arg("600").spawn().unwrap();
        std::fs::write(&pidfile, sleeper.id().to_string()).unwrap();
        let _ = sleeper.wait();
    }
    for finding in findings(Path::new(&root)) {
        println!("G0-FINDING {}", finding.replace('\n', " "));
    }
    println!("G0-HELPER-DONE");
}

/// The process group a cleanup may signal, from the pid of a child this
/// test spawned in its own group. Never 0 or 1 (which would signal every
/// process this user owns), never out of range, never this test's own
/// group.
fn checked_pgid(pid: u32) -> Result<i32, String> {
    let pgid =
        i32::try_from(pid).map_err(|_| format!("refusing process group {pid}: out of range"))?;
    if pgid <= 1 {
        return Err(format!(
            "refusing process group {pgid}: never a broadcast or init's group"
        ));
    }
    if pgid == nix::unistd::getpgrp().as_raw() {
        return Err(format!(
            "refusing process group {pgid}: it is this test's own"
        ));
    }
    Ok(pgid)
}

/// What a group signal's result means: delivered, or the group no longer
/// exists (ESRCH: gone). Any other error is a failure.
fn signal_outcome(result: Result<(), nix::errno::Errno>, pgid: i32) -> Result<(), String> {
    match result {
        Ok(()) | Err(nix::errno::Errno::ESRCH) => Ok(()),
        Err(error) => Err(format!("signalling process group {pgid} failed: {error}")),
    }
}

/// SIGKILL to the owned process group of the child `pid`, natively, its
/// result checked.
fn signal_group(pid: u32) -> Result<(), String> {
    let pgid = checked_pgid(pid)?;
    let result = nix::sys::signal::killpg(
        nix::unistd::Pid::from_raw(pgid),
        nix::sys::signal::Signal::SIGKILL,
    );
    signal_outcome(result, pgid)
}

/// Kill the owned child's whole process group, then reap the child; the
/// whole cleanup, signal included, is bounded by one deadline, `within`.
fn kill_and_reap_within(child: &mut Child, within: Duration) -> Result<(), String> {
    let deadline = Instant::now() + within;
    signal_group(child.id())?;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return Ok(()),
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(20));
            }
            Ok(None) => return Err(format!("the killed child was not reaped within {within:?}")),
            Err(e) => return Err(format!("reaping the killed child: {e}")),
        }
    }
}

/// Kill the owned child's whole process group, then reap the child, within
/// `REAP_WITHIN`.
fn kill_and_reap(child: &mut Child) {
    kill_and_reap_within(child, REAP_WITHIN).unwrap_or_else(|e| panic!("{e}"));
}

/// The helper's findings for `root`, run in an owned child process (the
/// test binary's helper role, in its own process group) under `deadline`.
/// On overrun the group, the helper's cargo child included, is killed and
/// reaped: nothing stays detached.
fn findings_within(
    root: &Path,
    deadline: Duration,
    hang: Option<&Path>,
) -> Result<Vec<String>, String> {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "g0_helper_role",
            "--exact",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(HELPER_ROOT, root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .process_group(0);
    if let Some(pidfile) = hang {
        command.env(HELPER_HANG, pidfile);
    }
    let mut child = command
        .spawn()
        .map_err(|e| format!("the helper did not start: {e}"))?;
    let mut stdout = child.stdout.take().unwrap();
    let reader = std::thread::spawn(move || {
        let mut text = String::new();
        let _ = stdout.read_to_string(&mut text);
        text
    });
    let until = Instant::now() + deadline;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < until => std::thread::sleep(Duration::from_millis(20)),
            Ok(None) => {
                kill_and_reap(&mut child);
                return Err(format!("the tripwire did not return within {deadline:?}"));
            }
            Err(e) => {
                kill_and_reap(&mut child);
                return Err(format!("waiting for the helper: {e}"));
            }
        }
    };
    // The child exited; its pipe closes with its group.
    kill_and_reap(&mut child);
    let text = reader.join().unwrap_or_default();
    // libtest prints `test g0_helper_role ... ` with no newline before the
    // role's own output, so the markers are found anywhere in a line.
    if !status.success() || !text.lines().any(|l| l.contains("G0-HELPER-DONE")) {
        return Err(format!("the helper failed ({status}):\n{text}"));
    }
    Ok(text
        .lines()
        .filter_map(|l| l.split_once("G0-FINDING ").map(|(_, finding)| finding))
        .map(str::to_string)
        .collect())
}

fn findings_within_deadline(root: &Path) -> Vec<String> {
    findings_within(root, DEADLINE, None).unwrap_or_else(|e| panic!("{e}"))
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

// ── Decision 2, L1: an unrelated dangling link is allowed ──────────────────

#[test]
fn g0_s6_an_unrelated_dangling_symlink_is_allowed_a_forbidden_one_is_not() {
    let root = fixture_copy("dangling");
    assert!(
        !Path::new("/opt/missing").exists(),
        "this control needs /opt/missing to be absent"
    );
    std::fs::create_dir_all(root.0.join("tools/view")).unwrap();
    // Green control: an optional tool not installed here.
    symlink("/opt/missing", root.0.join("tools/optional-sdk")).unwrap();
    // Still red: a forbidden name, dangling and looping.
    symlink("/opt/missing", root.0.join("tools/src-tauri")).unwrap();
    symlink(
        root.0.join("tools/view/src-tauri"),
        root.0.join("tools/view/src-tauri"),
    )
    .unwrap();
    let found = findings_within_deadline(&root.0);
    assert!(
        !found.iter().any(|f| f.contains("optional-sdk")),
        "an unrelated dangling symlink is a false red: {found:?}"
    );
    for forbidden in ["tools/src-tauri", "tools/view/src-tauri"] {
        assert!(
            found
                .iter()
                .any(|f| f.contains(forbidden) && f.contains("src-tauri tree")),
            "{forbidden} escaped the scan: {found:?}"
        );
    }
}

// ── Decision 2, L2: an overrunning helper is killed with its children ──────

#[test]
fn g0_s7_a_helper_past_its_deadline_is_killed_and_reaped_with_its_children() {
    let root = fixture_copy("hang");
    let pidfile = root.0.join("sleeper.pid");
    let started = Instant::now();
    let outcome = findings_within(&root.0, Duration::from_secs(3), Some(&pidfile));
    assert!(outcome.is_err(), "a hung helper returned: {outcome:?}");
    assert!(started.elapsed() < Duration::from_secs(3) + REAP_WITHIN);
    let pid = std::fs::read_to_string(&pidfile).expect("the helper started its grandchild");
    // The grandchild was in the helper's group: it is gone too.
    let deadline = Instant::now() + REAP_WITHIN;
    loop {
        let alive = Command::new("kill")
            .args(["-0", pid.trim()])
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .success();
        if !alive {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "the helper's grandchild {pid} survived"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

// ── Decision 3, item 1: the whole cleanup is bounded and checked ───────────
//
// No oracle here signals a process it did not plant: a failed signal and a
// gone group are judged on the pure decision functions, and the refusal
// of pgid 0 or 1 (which would signal every process this user owns) is
// checked without sending anything.

#[test]
fn g0_s8_a_failed_group_signal_fails_and_only_a_gone_group_is_gone() {
    use nix::errno::Errno;
    for refused in [0, 1, u32::MAX] {
        assert!(
            checked_pgid(refused).is_err(),
            "process group {refused} must never be signalled"
        );
    }
    let own = nix::unistd::getpgrp().as_raw() as u32;
    assert!(checked_pgid(own).is_err(), "never this test's own group");
    assert_eq!(checked_pgid(424242), Ok(424242));
    assert_eq!(signal_outcome(Ok(()), 4242), Ok(()));
    assert_eq!(
        signal_outcome(Err(Errno::ESRCH), 4242),
        Ok(()),
        "a gone group is gone"
    );
    for error in [Errno::EPERM, Errno::EINVAL] {
        assert!(
            signal_outcome(Err(error), 4242).is_err(),
            "a refused signal ({error}) was discarded as success"
        );
    }
}

const CLEANUP_ROLE: &str = "NEXUS_G0_CLEANUP_ROLE";

/// The cleanup role: a no-op unless s9 re-runs this binary for it. It
/// kills and reaps a child it planted within 2 s and says how that went.
#[test]
fn g0_cleanup_role() {
    let Some(pidfile) = std::env::var_os(CLEANUP_ROLE) else {
        return;
    };
    // Its own stdio: it must not hold the role's report pipe open.
    let mut planted = Command::new("sleep")
        .arg("600")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .unwrap();
    std::fs::write(&pidfile, planted.id().to_string()).unwrap();
    match kill_and_reap_within(&mut planted, Duration::from_secs(2)) {
        Ok(()) => println!("G0-CLEANUP reaped"),
        Err(e) => println!("G0-CLEANUP failed: {e}"),
    }
}

#[test]
fn g0_s9_the_whole_cleanup_is_bounded_even_when_kill_would_hang() {
    // A fake `kill` that hangs, first on the role's PATH: an external
    // command can hold the cleanup forever; native signalling cannot.
    let temp = Temp::new("fake-kill");
    let fake = temp.0.join("bin");
    std::fs::create_dir_all(&fake).unwrap();
    std::fs::write(fake.join("kill"), "#!/bin/sh\nexec sleep 600\n").unwrap();
    std::fs::set_permissions(fake.join("kill"), std::fs::Permissions::from_mode(0o755)).unwrap();
    let pidfile = temp.0.join("planted.pid");
    let path = format!(
        "{}:{}",
        fake.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let mut role = Command::new(std::env::current_exe().unwrap())
        .args([
            "g0_cleanup_role",
            "--exact",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(CLEANUP_ROLE, &pidfile)
        .env("PATH", path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .unwrap();
    let mut stdout = role.stdout.take().unwrap();
    let reader = std::thread::spawn(move || {
        let mut text = String::new();
        let _ = stdout.read_to_string(&mut text);
        text
    });
    let until = Instant::now() + Duration::from_secs(20);
    let finished = loop {
        if role.try_wait().unwrap().is_some() {
            break true;
        }
        if Instant::now() > until {
            break false;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    // The role's group (and a hung fake kill in it) never outlives the test.
    let _ = kill_and_reap_within(&mut role, REAP_WITHIN);
    let text = reader.join().unwrap_or_default();
    let reaped = text.lines().any(|l| l.contains("G0-CLEANUP reaped"));
    // The planted child is signalled here only if the role did not reap it
    // (it is then still alive, so its group is still the one planted).
    if !reaped {
        if let Some(pid) = std::fs::read_to_string(&pidfile)
            .ok()
            .and_then(|pid| pid.trim().parse::<u32>().ok())
        {
            let _ = signal_group(pid);
        }
    }
    assert!(
        finished,
        "the cleanup waited on a hung kill command past its deadline"
    );
    assert!(
        reaped,
        "the planted child was not killed and reaped: {text}"
    );
}
