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
    let ended = leader.finish(Duration::from_secs(10));
    let stderr = reader.join().unwrap_or_default();
    let ended = ended.unwrap_or_else(|e| panic!("cleaning up the install script failed: {e}"));
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
