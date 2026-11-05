//! Reuse the frozen settlement producer, replacing only its abort with a
//! parent-controlled receipt barrier. Recovery is a new read-only process;
//! the frozen legacy recovery/adoption fixture is deliberately never called.
use super::*;
use crate::journal::{verify, Event, JournalStore};
use crate::state::Trigger;
use nexus_governed_control::authority::clock::SystemClock;
use nexus_governed_control::authority::evidence::EndStatus;
use nexus_governed_control::broker::Vault;
use nexus_governed_control::evidence_store::PrimaryEvidence;
use nexus_governed_control::files::RecoveryFinding;
use nexus_governed_control::governed::GovernedControl;
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader};
use std::os::unix::fs::MetadataExt;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const REPORT: &str = "adapter::session_death_oracles_tests::session_settlement_report_child";
const POINTS: [(&str, &[&str]); 11] = [
    ("started", &["started_durable", "after_first_step"]),
    (
        "before_write",
        &["started_durable", "terminal_append:before_write"],
    ),
    (
        "before_sync",
        &["started_durable", "terminal_append:before_sync"],
    ),
    ("inflight", &["inflight:apply_prepared"]),
    ("torn", &["started_durable", "terminal_append:torn"]),
    (
        "after_sync",
        &["started_durable", "terminal_append:after_sync"],
    ),
    (
        "fence",
        &["started_append:refused", "after_fence_before_marker"],
    ),
    ("poison", &["started_append:refused", "inside_poison"]),
    (
        "before_ack",
        &["started_durable", "after_terminal_before_ack"],
    ),
    (
        "fence_terminal",
        &[
            "started_durable",
            "terminal_append:refused",
            "inside_fence:fenced=true",
        ],
    ),
    (
        "poison_terminal",
        &[
            "started_durable",
            "terminal_append:refused",
            "inside_poison",
        ],
    ),
];

type Resources = BTreeMap<PathBuf, (bool, u64, u64, Vec<u8>)>;
fn resources(root: &Path) -> Resources {
    fn walk(root: &Path, at: &Path, result: &mut Resources) {
        if !at.exists() {
            return;
        }
        for entry in std::fs::read_dir(at).unwrap() {
            let path = entry.unwrap().path();
            let meta = std::fs::symlink_metadata(&path).unwrap();
            assert!(!meta.file_type().is_symlink());
            let data = if meta.is_dir() {
                vec![]
            } else {
                std::fs::read(&path).unwrap()
            };
            result.insert(
                path.strip_prefix(root).unwrap().to_owned(),
                (meta.is_dir(), meta.dev(), meta.ino(), data),
            );
            if meta.is_dir() {
                walk(root, &path, result)
            }
        }
    }
    let mut result = BTreeMap::new();
    walk(root, root, &mut result);
    result
}

#[test]
fn session_settlement_report_child() {
    let Some(root) = std::env::var_os("M2DD_SETTLEMENT_REPORT") else {
        return;
    };
    let root = PathBuf::from(root);
    let point = std::env::var("M2DD_SETTLEMENT_POINT").unwrap();
    assert!(POINTS.iter().any(|(name, _)| *name == point));
    // Fresh broker state, with a separate memory primary store: no prior
    // session, workspace, grant, door, consent, pin or ack is adopted.
    assert!(nexus_governed_control::files::make_non_dumpable());
    let evidence = PrimaryEvidence::open(
        &root.join(format!("probe-{}/evidence.sqlite3", std::process::id())),
        vec![],
    )
    .unwrap();
    let broker = GovernedControl::new(
        &root.join("broker"),
        Vault::Disabled,
        evidence.clone(),
        Arc::new(SystemClock::default()),
    )
    .unwrap();
    let reports = broker
        .files_recovery_report()
        .expect("native report must succeed, including torn Mother history");
    if point == "inflight" {
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].findings.len(), 1);
        let RecoveryFinding::PreparedWithoutOutcome { files } = &reports[0].findings[0] else {
            panic!("one exact prepared native report")
        };
        assert_eq!(files.len(), 1);
    } else {
        assert!(
            reports.is_empty(),
            "completed native publication has the exact empty apply summary"
        );
    }
    // Validate existing Mother bytes without opening an operational
    // coordinator or writing a new terminal transition on a lost capability.
    let path = root.join("mother-journal.jsonl");
    let bytes = std::fs::read(&path).unwrap();
    if point == "torn" {
        assert!(!bytes.ends_with(b"\n"));
        let journal = FileJournal::open(&path).unwrap();
        assert!(
            journal.load().and_then(|lines| verify(&lines)).is_err(),
            "Mother must reject the torn terminal line"
        );
    } else {
        assert!(bytes.ends_with(b"\n"));
        let lines: Vec<_> = std::str::from_utf8(&bytes)
            .unwrap()
            .lines()
            .map(str::to_owned)
            .collect();
        let records = verify(&lines).unwrap();
        let starts = records
            .iter()
            .filter(|e| matches!(e.event, Event::SettlementStarted { .. }))
            .count();
        assert_eq!(
            starts,
            usize::from(!matches!(point.as_str(), "inflight" | "fence" | "poison"))
        );
        let ends = records
            .iter()
            .filter(|e| {
                matches!(
                    e.event,
                    Event::Transition {
                        trigger: Trigger::Settled { .. }
                    }
                )
            })
            .count();
        assert_eq!(
            ends,
            usize::from(matches!(
                point.as_str(),
                "before_sync" | "after_sync" | "before_ack"
            ))
        );
        // Visibility before_sync is not claimed as durability. The test
        // preserves this distinction in its literal boundary receipt.
    }
    assert_eq!(std::fs::read(path).unwrap(), bytes);
    assert!(matches!(
        evidence.close_session(),
        EndStatus::Recorded { .. }
    ));
    println!("\nSESSION SETTLEMENT REPORT VERIFIED {point}");
}

struct Owned(Child);
impl Drop for Owned {
    fn drop(&mut self) {
        self.0.kill().ok();
        self.0.wait().ok();
    }
}
fn receipt(child: &mut Owned, expected: String) -> std::thread::JoinHandle<()> {
    let out = child.0.stdout.take().unwrap();
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    let reader = std::thread::spawn(move || {
        let mut found = false;
        let mut text = String::new();
        for line in BufReader::new(out).lines() {
            let line = line.unwrap();
            if line == expected && !found {
                tx.send(Ok(())).ok();
                found = true;
            }
            if text.len() < 16384 {
                text.push_str(&line);
                text.push('\n');
            }
        }
        if !found {
            tx.send(Err(text)).ok();
        }
    });
    let observed = rx.recv_timeout(Duration::from_secs(20));
    if !matches!(observed, Ok(Ok(()))) {
        child.0.kill().ok();
        child.0.wait().ok();
        reader.join().unwrap();
        panic!("exact parent-kill barrier: {observed:?}");
    }
    reader
}
fn finish(child: &mut Owned) -> std::process::ExitStatus {
    let end = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            return status;
        }
        if Instant::now() >= end {
            child.0.kill().unwrap();
            child.0.wait().unwrap();
            panic!("bounded probe completion")
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn session_parent_kills_and_reaps_at_every_settlement_boundary_then_reports_twice() {
    let root = std::env::temp_dir().join(format!("m2dd-settlement-death-{}", std::process::id()));
    create_private(&root).unwrap();
    let source = root.join("abort_barrier.c");
    let library = root.join("abort_barrier.so");
    std::fs::write(
        &source,
        br#"
#define _GNU_SOURCE
#include <dlfcn.h>
#include <stdlib.h>
#include <unistd.h>
_Noreturn void abort(void) {
    if (getenv("M2DD_ABORT_BARRIER")) {
        static const char receipt[] = "\nSESSION ABORT BARRIER\n";
        if (write(1, receipt, sizeof(receipt)-1) != (ssize_t)(sizeof(receipt)-1)) _exit(124);
        char release;
        if (read(0, &release, 1) < 0) _exit(125);
        _exit(126);
    }
    void (*real)(void) = dlsym(RTLD_NEXT, "abort");
    real();
    _exit(127);
}
"#,
    )
    .unwrap();
    let compiled = Command::new("cc")
        .args([
            "-shared", "-fPIC", "-std=c11", "-Wall", "-Wextra", "-Werror", "-o",
        ])
        .arg(&library)
        .arg(&source)
        .arg("-ldl")
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    for (point, expected_receipts) in POINTS {
        let home = root.join(point);
        let mut child = Owned(
            Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "adapter::lite_crash::lite_crash_child",
                    "--nocapture",
                    "--test-threads=1",
                ])
                .env("MOTHER_LITE_CHILD", point)
                .env("MOTHER_LITE_PHASE", "1")
                .env("MOTHER_LITE_HOME", &home)
                .env("M2DD_ABORT_BARRIER", "1")
                .env("LD_PRELOAD", &library)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::inherit())
                .spawn()
                .unwrap(),
        );
        let reader = receipt(&mut child, "SESSION ABORT BARRIER".into());
        let observed = std::fs::read_to_string(home.join("receipt")).unwrap();
        assert_eq!(
            observed.lines().collect::<Vec<_>>(),
            expected_receipts,
            "{point}: independently registered exact boundary"
        );
        let workspace =
            PathBuf::from(std::fs::read_to_string(home.join("workspace.path")).unwrap());
        let staging = home.join("broker/files/coding-runs");
        let owner_before = resources(&workspace);
        let stage_before = resources(&staging);
        assert_eq!(workspace.join("crash-a.txt").exists(), point != "inflight");
        assert!(!workspace.join("crash-b.txt").exists());
        assert!(!workspace.join("crash-c.txt").exists());
        let journal_before = std::fs::read(home.join("mother-journal.jsonl")).unwrap();
        child.0.kill().unwrap();
        assert!(!finish(&mut child).success());
        reader.join().unwrap();
        for restart in 1..=2 {
            let mut probe = Owned(
                Command::new(std::env::current_exe().unwrap())
                    .args(["--exact", REPORT, "--nocapture"])
                    .env("M2DD_SETTLEMENT_REPORT", &home)
                    .env("M2DD_SETTLEMENT_POINT", point)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::inherit())
                    .spawn()
                    .unwrap(),
            );
            let reader = receipt(
                &mut probe,
                format!("SESSION SETTLEMENT REPORT VERIFIED {point}"),
            );
            assert!(finish(&mut probe).success());
            reader.join().unwrap();
            assert_eq!(
                resources(&workspace),
                owner_before,
                "{point}/{restart}: Owner resources"
            );
            assert_eq!(
                resources(&staging),
                stage_before,
                "{point}/{restart}: staging resources"
            );
            assert_eq!(
                std::fs::read(home.join("mother-journal.jsonl")).unwrap(),
                journal_before,
                "report cannot repeat terminal transitions"
            );
        }
        println!("SETTLEMENT DEATH VERIFIED {point} killed_reaped=true fresh_reports=2");
    }
}
