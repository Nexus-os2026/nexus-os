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

use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const DEADLINE: Duration = Duration::from_secs(120);

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
    let mut child = Command::new("bash")
        .arg(&script)
        .current_dir(&elsewhere)
        .env("HOME", &home)
        .env("CARGO_HOME", cargo_home)
        .env("RUSTUP_HOME", rustup_home)
        .env("CARGO_TARGET_DIR", "target")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()
        .expect("bash runs the install script");
    let until = Instant::now() + DEADLINE;
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break Some(status);
        }
        if Instant::now() > until {
            // The whole cleanup, signal and reap, within one deadline.
            if let Err(e) = harness::kill_group_within(&mut child, Duration::from_secs(10)) {
                panic!("cleaning up the install script failed: {e}");
            }
            break None;
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let output = child.wait_with_output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
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
