//! Session §2/§3 and amendment B A6/A11/A12. These use the installed test
//! binary, real PTYs, real process death and independently read storage.
#![deny(unsafe_code)]
#![cfg(target_os = "linux")]
mod support;
use nexus_mother::journal::{verify, Event, JournalStore};
use support::{Fixture, Pty};

const EMERGENCY: &str = "Emergency Stop received. Mother will admit no new work, undo or cleanup in this session. An operation already handed to the kernel may still finish, including its internal cleanup. You can view status and evidence, or quit. Restart only reports surviving evidence and uncertainty; it does not clean up or restore undo.";
const RECOVERY: &str = "Recovery report only. Mother has not removed files or staging, restored workspace permission, or restored undo. Some unfinished work may be known only from incomplete evidence.";
const UNCLEAN: &str = "UNCLEAN EXIT: this process did not complete the full clean-close protocol. Some session-end records may already be durable; any unverified recording or resource obligations are listed separately. No cleanup or recording is promised after this process exits.";

fn journal(f: &Fixture) -> Vec<nexus_mother::journal::Entry> {
    let file = nexus_mother::adapter::FileJournal::open(&f.home.join("mother-journal.jsonl"))
        .expect("ordinary journal file can be opened");
    verify(&file.load().expect("complete journal lines")).expect("verified journal chain")
}

#[test]
fn session_native_recovery_child() {
    let Some(home) = std::env::var_os("M2DD_SESSION_NATIVE_REPORT") else {
        return;
    };
    use nexus_governed_control::{
        authority::clock::SystemClock, broker::Vault, evidence_store::PrimaryEvidence,
        files::RecoveryFinding, governed::GovernedControl,
    };
    use std::sync::Arc;
    let home = std::path::PathBuf::from(home);
    let root = home.join("broker");
    // The independent native-ledger probe must not create an extra unclean
    // primary session in the Home whose readiness the product will inspect.
    let probe = home
        .parent()
        .unwrap()
        .join(format!("native-probe-{}", std::process::id()));
    let evidence = PrimaryEvidence::open(&probe.join("evidence.sqlite3"), vec![])
        .expect("isolated primary evidence for read-only native probe");
    let broker = GovernedControl::new(
        &root,
        Vault::Disabled,
        evidence.clone(),
        Arc::new(SystemClock::default()),
    )
    .expect("new broker for read-only kernel recovery");
    let reports = broker
        .files_recovery_report()
        .expect("exact successful native recovery");
    let wanted = std::env::var("M2DD_SESSION_NATIVE_EXPECT").unwrap();
    match wanted.as_str() {
        "pending" => {
            assert_eq!(reports.len(), 1);
            assert_eq!(
                reports[0].findings,
                vec![RecoveryFinding::StagingAwaitingApproval]
            );
        }
        "empty" => assert!(reports.is_empty()),
        "restore" => {
            assert_eq!(reports.len(), 1);
            assert_eq!(reports[0].findings.len(), 1);
            let RecoveryFinding::RestorePreparedWithoutOutcome { files } = &reports[0].findings[0]
            else {
                panic!("exact prepared restore report")
            };
            assert_eq!(files.len(), 1);
            assert_eq!(
                files[0].path_sha256.as_deref(),
                Some("334bf61109a5501f653af79545b3059dbceee263080ac20dcde6075fed514d79")
            );
            assert!(files[0].temp_id.as_ref().is_some_and(|id| id.len() > 10));
        }
        _ => panic!("unregistered native recovery expectation"),
    }
    assert!(matches!(
        evidence.close_session(),
        nexus_governed_control::authority::evidence::EndStatus::Recorded { .. }
    ));
    println!("NATIVE RECOVERY VERIFIED {wanted}");
}

fn native_recovery(f: &Fixture, wanted: &str) {
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", "session_native_recovery_child", "--nocapture"])
        .env("M2DD_SESSION_NATIVE_REPORT", &f.home)
        .env("M2DD_SESSION_NATIVE_EXPECT", wanted);
    let mut p = Pty::start(command);
    p.expect(&format!("NATIVE RECOVERY VERIFIED {wanted}"));
    assert!(p.finish().success(), "native recovery assertions must pass");
}

#[test]
fn session_normal_pending_quit_settles_before_durable_end_without_publication() {
    let f = Fixture::new();
    let mut p = Pty::start(f.command("cancel at Quit\n"));
    p.ready();
    p.mission();
    let staging = f.home.join("broker/files/coding-runs");
    assert!(!support::privacy::files_under(&staging, None).is_empty());
    p.quit();
    assert_eq!(support::evidence::counts(&f), [0, 1, 1]);
    f.empty();
    assert!(support::privacy::files_under(&staging, None).is_empty());
    let entries = journal(&f);
    assert!(matches!(
        entries.last().map(|e| &e.event),
        Some(Event::SessionClosed)
    ));
    assert!(entries
        .iter()
        .any(|e| matches!(&e.event, Event::Transition { trigger }
        if matches!(trigger, nexus_mother::state::Trigger::Stop { .. }
            | nexus_mother::state::Trigger::Settled { .. }))));
    assert!(!entries
        .iter()
        .any(|e| matches!(&e.event, Event::Transition { trigger }
        if matches!(trigger, nexus_mother::state::Trigger::ActionApproved { .. }))));
}

#[test]
fn session_idle_emergency_can_close_with_exact_notice_and_recorded_end() {
    let f = Fixture::new();
    let mut p = Pty::start(f.command("unused\n"));
    p.ready();
    p.line("stop");
    p.expect("STOP RECORDING COMPLETED");
    p.quit();
    assert_eq!(support::evidence::counts(&f), [1, 1, 1]);
    assert!(p.output.contains(EMERGENCY));
    // Oracle amendment U3, Owner: "Make these 5 fixes don't keep pending."
    // Old fresh-home assertion: RECOVERY ("Recovery report only. Mother has not
    // removed files or staging, restored workspace permission, or restored undo.
    // Some unfinished work may be known only from incomplete evidence.").
    // New fresh-home assertion: "Fresh start: no earlier session in this home; nothing to recover."
    // Restart assertions for homes with history below keep RECOVERY exactly.
    assert!(p
        .output
        .contains("Fresh start: no earlier session in this home; nothing to recover."));
    let entries = journal(&f);
    assert!(matches!(
        entries.last().map(|e| &e.event),
        Some(Event::SessionClosed)
    ));
    f.empty();
}

#[test]
fn session_lost_pending_inventory_is_report_only_on_two_new_process_starts() {
    let f = Fixture::new();
    let mut p = Pty::start(f.command("surviving candidate\n"));
    p.ready();
    p.mission();
    let staging = f.home.join("broker/files/coding-runs");
    let before = support::privacy::files_under(&staging, None);
    assert!(
        !before.is_empty(),
        "exact FILE PENDING boundary has real staging"
    );
    p.child.kill().unwrap();
    assert!(!p.child.wait().unwrap().success());
    println!("PROCESS DEATH verified killed_reaped=true");
    drop(p);
    for restart in 1..=2 {
        native_recovery(&f, "pending");
        let mut next = Pty::start(f.command("must never retry\n"));
        next.expect("RECOVERY REPORT");
        next.line("run");
        next.expect("REFUSED");
        assert!(
            !next.output.contains("Answer code:"),
            "restart {restart} asks for no authority"
        );
        next.line("undo");
        next.expect("REFUSED");
        next.line("cleanup");
        next.expect("REFUSED");
        next.line("quit");
        assert!(
            !next.finish().success(),
            "unresolved durable history cannot close cleanly"
        );
        assert!(next.output.contains(RECOVERY));
        assert!(next.output.contains(UNCLEAN));
        assert!(!next.output.contains("SESSION CLOSED recorded"));
        assert_eq!(support::privacy::files_under(&staging, None), before);
        f.empty();
    }
}

#[test]
fn session_torn_mother_history_is_rejected_before_workspace_or_consent() {
    let f = Fixture::new();
    nexus_mother::adapter::create_private(&f.home).unwrap();
    std::fs::write(f.home.join("mother-journal.jsonl"), b"{\"format\":").unwrap();
    for _ in 0..2 {
        native_recovery(&f, "empty");
        let mut p = Pty::start(f.command("must not stage\n"));
        p.expect("RECOVERY FAILED mother_history");
        assert!(!p.output.contains("Answer code:"));
        assert!(!p.output.contains("HANDS READY"));
        assert!(!p.finish().success());
        f.empty();
        assert_eq!(
            std::fs::read(f.home.join("mother-journal.jsonl")).unwrap(),
            b"{\"format\":"
        );
    }
}

#[test]
fn session_eof_with_pending_resources_retains_them_and_reports_unclean() {
    for input in [b"\x04".as_slice(), b"unfinished\x04"] {
        let f = Fixture::new();
        let mut p = Pty::start(f.command("retained on EOF\n"));
        p.ready();
        p.mission();
        let staging = f.home.join("broker/files/coding-runs");
        let before = support::privacy::files_under(&staging, None);
        assert!(!before.is_empty());
        p.send(input);
        assert!(!p.finish().success());
        assert!(p.output.contains(UNCLEAN));
        assert!(!p.output.contains("SESSION CLOSED recorded"));
        assert_eq!(support::privacy::files_under(&staging, None), before);
        f.empty();
    }
}

fn identities(
    root: &std::path::Path,
) -> std::collections::BTreeMap<std::path::PathBuf, (u64, u64, bool, Vec<u8>)> {
    use std::os::unix::fs::MetadataExt;
    fn walk(
        root: &std::path::Path,
        path: &std::path::Path,
        out: &mut std::collections::BTreeMap<std::path::PathBuf, (u64, u64, bool, Vec<u8>)>,
    ) {
        for entry in std::fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            let meta = std::fs::symlink_metadata(&path).unwrap();
            assert!(!meta.file_type().is_symlink());
            out.insert(
                path.strip_prefix(root).unwrap().to_owned(),
                (
                    meta.dev(),
                    meta.ino(),
                    meta.is_dir(),
                    if meta.is_dir() {
                        vec![]
                    } else {
                        std::fs::read(&path).unwrap()
                    },
                ),
            );
            if meta.is_dir() {
                walk(root, &path, out);
            }
        }
    }
    let mut out = std::collections::BTreeMap::new();
    walk(root, root, &mut out);
    out
}

#[test]
fn session_death_after_durable_restore_preparation_has_two_exact_native_reports() {
    let f = Fixture::new();
    let fault = support::faults::Fault::new(&f, "restore-prepared");
    let mut command = f.command("restore death fixture\n");
    fault.attach(&f, &mut command);
    let mut p = Pty::start(command);
    p.ready();
    p.create();
    fault.arm();
    p.line("undo");
    p.yes();
    let receipt = fault.reached();
    assert_eq!(receipt.path.trim_end(), f.workspace.to_str().unwrap());
    let staging = f.home.join("broker/files/coding-runs");
    let before = (identities(&f.workspace), identities(&staging));
    assert_eq!(
        std::fs::read(f.workspace.join("hello.txt")).unwrap(),
        b"restore death fixture\n"
    );
    p.child.kill().unwrap();
    assert!(!p.child.wait().unwrap().success());
    drop(p);
    for _ in 0..2 {
        native_recovery(&f, "restore");
        assert_eq!((identities(&f.workspace), identities(&staging)), before);
    }
    println!("PROCESS DEATH point=restore.prepared killed_reaped=true fresh_reports=2");
}

fn death_in_cleanup(point: &'static str) {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new();
    let fault = support::faults::Fault::new(&f, point);
    let mut command = f.command("cleanup death fixture\n");
    fault.attach(&f, &mut command);
    let mut p = Pty::start(command);
    p.ready();
    p.mission();
    let staging = f.home.join("broker/files/coding-runs");
    let dirs: Vec<_> = std::fs::read_dir(&staging)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.is_dir())
        .collect();
    assert_eq!(dirs.len(), 1);
    // The old baseline attempted cleanup during denial. The session route
    // never does so: explicitly establish that same failed-cleanup prefix
    // before the independently armed target handoff.
    std::fs::set_permissions(&dirs[0], std::fs::Permissions::from_mode(0o500)).unwrap();
    p.line("approve");
    p.code();
    p.line("no");
    p.expect("ACTION DENIED");
    p.line("cleanup");
    p.expect("still_owed=1");
    assert!(
        dirs[0].is_dir(),
        "failed explicit cleanup retains the exact directory"
    );
    std::fs::set_permissions(&dirs[0], std::fs::Permissions::from_mode(0o700)).unwrap();
    fault.arm();
    p.line("cleanup");
    let receipt = fault.reached();
    assert!(receipt.path.contains("/coding-runs/"));
    let before = identities(&staging);
    assert!(
        !before.is_empty(),
        "native call still owns its staging directory"
    );
    p.child.kill().unwrap();
    assert!(!p.child.wait().unwrap().success());
    drop(p);
    for _ in 0..2 {
        native_recovery(&f, "empty");
        assert_eq!(identities(&staging), before);
        f.empty();
    }
    println!("PROCESS DEATH point={point} killed_reaped=true fresh_reports=2 native_summary=empty_not_all_clear");
}

#[test]
fn session_death_at_native_cleanup_handoff_never_sweeps_on_restart() {
    death_in_cleanup("cleanup");
}

#[test]
fn session_death_after_one_internal_cleanup_item_preserves_the_remaining_resources() {
    death_in_cleanup("cleanup-after-item");
}

fn death_during_close(point: &'static str, torn: bool) {
    let f = Fixture::new();
    let fault = support::faults::Fault::new(&f, point);
    let mut command = f.command("conclusive file before partial close\n");
    fault.attach(&f, &mut command);
    let mut p = Pty::start(command);
    p.ready();
    p.create();
    fault.arm();
    p.line("quit");
    let receipt = fault.reached();
    assert!(receipt.path.ends_with("/mother-journal.jsonl\n"));
    let path = f.home.join("mother-journal.jsonl");
    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(!bytes.ends_with(b"\n"), torn);
    if !torn {
        // Read bytes only: FileJournal::open synchronizes and would change
        // the before-sync experiment's durability facts.
        let lines: Vec<String> = std::str::from_utf8(&bytes)
            .unwrap()
            .lines()
            .map(str::to_owned)
            .collect();
        let entries = verify(&lines).unwrap();
        assert!(matches!(
            entries.last().map(|entry| &entry.event),
            Some(Event::SessionClosed)
        ));
    }
    p.child.kill().unwrap();
    assert!(!p.child.wait().unwrap().success());
    println!("PROCESS DEATH verified killed_reaped=true");
    drop(p);
    // Primary close is still outstanding; Mother's completed/partial write
    // cannot establish the full close proof.
    let [_, opened, closed] = support::evidence::counts(&f);
    assert_eq!((opened, closed), (1, 0));
    let staging = f.home.join("broker/files/coding-runs");
    let retained = support::privacy::files_under(&staging, None);
    for restart in 1..=2 {
        native_recovery(&f, "empty");
        let mut next = Pty::start(f.command("cannot adopt closed history\n"));
        if torn {
            next.expect("RECOVERY FAILED mother_history");
            assert!(!next.output.contains("HANDS READY"));
            assert!(!next.finish().success());
            assert_eq!(std::fs::read(&path).unwrap(), bytes);
        } else {
            next.expect("RECOVERY REPORT");
            next.line("run");
            next.expect("REFUSED");
            next.line("undo");
            next.expect("REFUSED");
            next.line("quit");
            assert!(!next.finish().success());
        }
        assert!(
            !next.output.contains("Answer code:"),
            "restart {restart} asks for no prior authority"
        );
        assert_eq!(support::privacy::files_under(&staging, None), retained);
        assert_eq!(
            std::fs::read(f.workspace.join("hello.txt")).unwrap(),
            b"conclusive file before partial close\n"
        );
    }
}

#[test]
fn session_death_after_mother_close_before_primary_close_is_not_clean() {
    death_during_close("mother-close-after-sync", false);
}

#[test]
fn session_death_before_mother_close_sync_preserves_unknown_end() {
    death_during_close("mother-close-before-sync", false);
}

#[test]
fn session_torn_close_requires_successful_native_report_and_mother_rejection() {
    death_during_close("mother-close-torn", true);
}

#[test]
fn session_death_with_pending_stop_record_is_uncovered_on_restart() {
    let f = Fixture::new();
    let fault = support::faults::Fault::new(&f, "evidence");
    let mut command = f.command("no mission\n");
    fault.attach(&f, &mut command);
    let mut p = Pty::start(command);
    p.ready();
    fault.arm();
    p.line("stop");
    let receipt = fault.reached();
    assert!(receipt.path.contains("/p3-evidence/evidence.sqlite3"));
    p.child.kill().unwrap();
    assert!(!p.child.wait().unwrap().success());
    println!("PROCESS DEATH verified killed_reaped=true");
    drop(p);
    assert_eq!(support::evidence::counts(&f), [0, 1, 0]);
    for _ in 0..2 {
        native_recovery(&f, "empty");
        let mut next = Pty::start(f.command("no lost Stop adoption\n"));
        next.expect("RECOVERY REPORT");
        next.line("run");
        next.expect("REFUSED");
        assert!(!next.output.contains("Answer code:"));
        next.line("quit");
        assert!(!next.finish().success());
        f.empty();
    }
}
