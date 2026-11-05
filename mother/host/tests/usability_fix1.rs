//! PR #98 review 1: pre-initialization provenance and honest close wording.
#![deny(unsafe_code)]
#![cfg(target_os = "linux")]
mod support;
use nexus_governed_control::authority::approval::*;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};
use support::faults::Fault;
use support::{Fixture, Pty};

const FRESH: &str = "Fresh start: no earlier session in this home; nothing to recover.";
const RECOVERY: &str = "Recovery report only. Mother has not removed files or staging, restored workspace permission, or restored undo. Some unfinished work may be known only from incomplete evidence.";

fn attach(f: &Fixture, fault: &Fault, command: &mut Command) {
    static LIBRARY: OnceLock<PathBuf> = OnceLock::new();
    let lib = LIBRARY.get_or_init(|| {
        let dir = std::env::temp_dir().join(format!("ux-fix1-interpose-{}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        let lib = dir.join("usability_fix1.so");
        let output = Command::new("cc")
            .args([
                "-shared", "-fPIC", "-std=c11", "-Wall", "-Wextra", "-Werror", "-o",
            ])
            .arg(&lib)
            .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/support/usability_fix1.c"))
            .arg("-ldl")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        lib
    });
    fault.attach(f, command);
    command.env("LD_PRELOAD", lib);
}

fn recovery(p: &Pty) {
    assert!(p.output.contains(RECOVERY), "{}", p.output);
    assert!(
        p.output
            .contains("RECOVERY REPORT only; no sweep or workspace restoration\n"),
        "{}",
        p.output
    );
    assert!(!p.output.contains(FRESH), "{}", p.output);
}

struct No;
impl ControlConfirmer for No {
    fn confirm_action(&self, _: &ActionConfirmation) -> bool {
        false
    }
    fn confirm_grant(&self, _: &GrantConfirmation) -> bool {
        false
    }
    fn confirm_resume(&self, _: &ResumeConfirmation) -> bool {
        false
    }
    fn confirm_acknowledgement(&self, _: &AcknowledgementConfirmation) -> bool {
        false
    }
    fn confirm_agent_door(&self, _: &AgentDoorConfirmation) -> bool {
        false
    }
}

#[test]
fn ux_fix1_primary_only_home_child() {
    let Some(path) = std::env::var_os("UX_FIX1_PRIMARY_HOME") else {
        return;
    };
    let home = nexus_mother_host::Home::open(&PathBuf::from(path), Arc::new(No)).unwrap();
    assert!(matches!(
        home.close_session(),
        nexus_governed_control::authority::evidence::EndStatus::Recorded { .. }
    ));
    println!("PRIMARY ONLY SESSION CLOSED");
}

#[test]
fn ux_fix1_primary_session_history_keeps_recovery_disclosures() {
    let f = Fixture::new();
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", "ux_fix1_primary_only_home_child", "--nocapture"])
        .env("UX_FIX1_PRIMARY_HOME", &f.home);
    let mut setup = Pty::start(command);
    setup.expect("PRIMARY ONLY SESSION CLOSED");
    assert!(setup.finish().success());
    assert_eq!(support::evidence::counts(&f), [0, 1, 1]);
    assert!(!f.home.join("mother-journal.jsonl").exists());
    let mut p = Pty::start(f.command("primary history"));
    p.ready();
    assert!(p
        .output
        .contains("mother_verified=true resources_verified=true"));
    recovery(&p);
    p.quit();
    f.empty();
}

#[test]
fn ux_fix1_crash_after_schema_before_session_keeps_recovery_disclosures() {
    let f = Fixture::new();
    let fault = Fault::new(&f, "schema-before-session");
    let mut command = f.command("crash before first session record");
    attach(&f, &fault, &mut command);
    fault.arm();
    let mut crashed = Pty::start(command);
    let receipt = fault.reached();
    assert!(receipt.path.trim_end().ends_with("/broker/p3-evidence"));
    crashed.child.kill().unwrap();
    assert!(!crashed.child.wait().unwrap().success());
    // The independent verifier checks identity, SQLite integrity and chain;
    // zero opened rows proves death preceded the very first session record.
    assert_eq!(support::evidence::counts(&f), [0, 0, 0]);
    assert!(!f.home.join("mother-journal.jsonl").exists());
    let mut p = Pty::start(f.command("restart after schema commit"));
    p.ready();
    recovery(&p);
    p.quit();
    f.empty();
}

#[test]
fn ux_fix1_unrecognized_prior_home_state_keeps_recovery_disclosures() {
    let f = Fixture::new();
    nexus_mother::adapter::create_private(&f.home).unwrap();
    std::fs::write(f.home.join("prior-state"), b"unclassified earlier state").unwrap();
    let mut p = Pty::start(f.command("unproven freshness"));
    p.ready();
    recovery(&p);
    p.quit();
    f.empty();
}

#[test]
fn ux_fix1_delivered_close_frame_cannot_claim_clean_before_late_finalization() {
    let f = Fixture::new();
    let fault = Fault::new(&f, "finalize-after-delivery");
    let mut command = f.command("late finalization");
    attach(&f, &fault, &mut command);
    let mut p = Pty::start(command);
    p.ready();
    fault.arm();
    p.line("quit");
    let receipt = fault.reached();
    assert_eq!(receipt.path.trim_end(), "joined-after-delivery");
    p.expect("SESSION CLOSED recorded\n");
    let delivered = p.output.clone();
    // Deliberate deadline experiment, not readiness synchronization: the
    // receipt proves delivery/join; four more seconds exceeds the fixed
    // three-second budget even if Quit was accepted just before the receipt.
    let held = Instant::now();
    std::thread::sleep(Duration::from_secs(4));
    assert!(held.elapsed() >= Duration::from_secs(4));
    assert!(p.child.try_wait().unwrap().is_none());
    receipt.release();
    assert!(!p.finish().success(), "expired finalization must fail");
    assert_eq!(support::evidence::counts(&f), [0, 1, 1]);
    assert!(
        !delivered.contains("closed cleanly"),
        "premature clean claim delivered: {delivered}"
    );
    assert!(!p.output.contains("closed cleanly"));
    f.empty();
}
