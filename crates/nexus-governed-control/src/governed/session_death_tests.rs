//! Real death: the parent receives an exact callback receipt, kills and
//! reaps the producer, then starts two independent read-only brokers. A
//! callback alone is never counted as process-death coverage.
use super::*;
use crate::authority::evidence::MemoryEvidence;
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;

const CHILD: &str = "M2DD_NATIVE_DEATH_CHILD";
const CHILD_TEST: &str = "governed::session_routes::death::session_native_death_child";
const REPORT_TEST: &str = "governed::session_routes::death::session_native_death_report_child";

// Literal durable facts at the boundary, not a production classifier.
const CASES: [(&str, &str, bool, &str); 11] = [
    ("session.after.staging_grant", "run.granted", false, "empty"),
    ("session.after.snapshot", "op.outcome", false, "empty"),
    ("session.after.edit", "op.outcome", false, "empty"),
    (
        "session.after.structural_verify",
        "verify.structural",
        false,
        "awaiting",
    ),
    ("session.after.review", "review.computed", false, "awaiting"),
    (
        "session.after.request_approval",
        "approval.granted",
        false,
        "awaiting",
    ),
    ("files.before_final", "apply.prepared", false, "prepared"),
    ("files.after_final", "apply.prepared", false, "prepared"),
    ("session.after.apply", "apply.completed", true, "empty"),
    (
        "session.after.request_restore",
        "restore.approved",
        true,
        "empty",
    ),
    ("session.after.restore", "restore.completed", false, "empty"),
];

fn root(key: &str) -> TempRoot {
    TempRoot(
        crate::runtime_root::RuntimeRoot::open(&std::path::PathBuf::from(
            std::env::var_os(key).unwrap(),
        ))
        .unwrap(),
    )
}

#[test]
fn session_native_death_child() {
    let Ok(point) = std::env::var(CHILD) else {
        return;
    };
    let point = CASES.iter().find(|case| case.0 == point).unwrap().0;
    let f = Fixture::with_roots(root("M2DD_NATIVE_ROOT"), root("M2DD_NATIVE_DIRS"));
    f.gov.files().hooks().arm(point, move || {
        println!("\nDEATH BOUNDARY {point}");
        std::io::stdout().flush().unwrap();
        let mut byte = [0u8];
        // Parent owns termination. EOF/release is not the requested death.
        let _ = std::io::stdin().read_exact(&mut byte);
        panic!("parent must kill and reap at the exact boundary");
    });
    let pending = f.pending();
    f.approve(pending).unwrap();
    f.gov
        .files_undo_with_halt(&Yes, Some(&f.gov.file_session_halt()))
        .unwrap();
    panic!("registered boundary was not reached: {point}");
}

#[test]
fn session_native_death_report_child() {
    let Some(root) = std::env::var_os("M2DD_NATIVE_REPORT_ROOT") else {
        return;
    };
    let broker = GovernedControl::with_secrets(
        std::path::Path::new(&root),
        Vault::Disabled.source(),
        Arc::new(MemoryEvidence::new(64)),
        Arc::new(ManualClock::default()),
    )
    .unwrap()
    .with_kernel_stop(StopAuthority::new());
    let reports = broker
        .files_recovery_report()
        .expect("native recovery must succeed");
    let expected = std::env::var("M2DD_NATIVE_REPORT_EXPECT").unwrap();
    use crate::files::RecoveryFinding;
    match expected.as_str() {
        // Earlier allocation is separately read from verified native records;
        // an empty legacy apply summary is never called an all-clear report.
        "empty" => assert!(reports.is_empty()),
        "awaiting" => {
            assert_eq!(reports.len(), 1);
            assert_eq!(
                reports[0].findings,
                vec![RecoveryFinding::StagingAwaitingApproval]
            );
        }
        "prepared" => {
            assert_eq!(reports.len(), 1);
            assert_eq!(reports[0].findings.len(), 1);
            let RecoveryFinding::PreparedWithoutOutcome { files } = &reports[0].findings[0] else {
                panic!("exact prepared-without-outcome finding required");
            };
            assert_eq!(files.len(), 1);
            // Independent SHA-256 of length-prefixed path-domain and fixture
            // spelling; no production path/output classifier selects it.
            assert_eq!(
                files[0].path_sha256.as_deref(),
                Some("8ceac3f10ffb70651d6fc273da9b9c077b30a0e1a1543f23701f1c59e4e6457a")
            );
        }
        _ => panic!("unknown report expectation"),
    }
    println!("\nNATIVE REPORT VERIFIED {expected}");
}

struct OwnedChild(Child);
impl Drop for OwnedChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn wait_receipt(child: &mut OwnedChild, expected: String) -> std::thread::JoinHandle<()> {
    let output = child.0.stdout.take().unwrap();
    let (tx, rx) = mpsc::sync_channel(1);
    let reader = std::thread::spawn(move || {
        let mut collected = String::new();
        let mut received = false;
        for line in BufReader::new(output).lines() {
            let line = line.unwrap();
            if line == expected && !received {
                let _ = tx.send(Ok(()));
                received = true;
            }
            if collected.len() < 16_384 {
                collected.push_str(&line);
                collected.push('\n');
            }
        }
        if !received {
            let _ = tx.send(Err(collected));
        }
    });
    let result = rx.recv_timeout(Duration::from_secs(15));
    if !matches!(result, Ok(Ok(()))) {
        child.0.kill().ok();
        child.0.wait().ok();
        reader.join().unwrap();
        panic!("exact boundary receipt: {result:?}");
    }
    reader
}

fn finish(child: &mut OwnedChild) -> std::process::ExitStatus {
    let until = std::time::Instant::now() + Duration::from_secs(15);
    loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            return status;
        }
        if std::time::Instant::now() >= until {
            child.0.kill().unwrap();
            child.0.wait().unwrap();
            panic!("child did not exit within its one deadline");
        }
        // Poll only process completion; no test ordering depends on a sleep.
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn session_real_death_at_native_boundaries_preserves_resources_on_two_fresh_reports() {
    for (point, required_event, published, report) in CASES {
        let state = temp_root("session-death-state");
        let selected = temp_root("session-death-selected");
        let mut child = OwnedChild(
            Command::new(std::env::current_exe().unwrap())
                .args(["--exact", CHILD_TEST, "--nocapture"])
                .env(CHILD, point)
                .env("M2DD_NATIVE_ROOT", state.0.path())
                .env("M2DD_NATIVE_DIRS", selected.0.path())
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::inherit())
                .spawn()
                .unwrap(),
        );
        let drain = wait_receipt(&mut child, format!("DEATH BOUNDARY {point}"));
        let staging = state.0.path().join("files/coding-runs");
        let workspace = selected.0.path().join("selected/workspace");
        let before = resources(&staging);
        let owner_before = resources(&workspace);
        assert!(!before.is_empty(), "boundary must own actual staging");
        assert_eq!(workspace.join("session.txt").exists(), published);
        let ledger = state.0.path().join("files/ledger.db");
        let records = events(&ledger);
        assert!(
            matching(&records, required_event, None) > 0,
            "{point}: required durable {required_event}"
        );
        child.0.kill().unwrap();
        assert!(!finish(&mut child).success());
        drain.join().unwrap();
        for restart in 1..=2 {
            let mut probe = OwnedChild(
                Command::new(std::env::current_exe().unwrap())
                    .args(["--exact", REPORT_TEST, "--nocapture"])
                    .env("M2DD_NATIVE_REPORT_ROOT", state.0.path())
                    .env("M2DD_NATIVE_REPORT_EXPECT", report)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::inherit())
                    .spawn()
                    .unwrap(),
            );
            let drain = wait_receipt(&mut probe, format!("NATIVE REPORT VERIFIED {report}"));
            assert!(finish(&mut probe).success());
            drain.join().unwrap();
            assert_eq!(
                resources(&staging),
                before,
                "{point}/restart={restart}: staging identity/content"
            );
            assert_eq!(
                resources(&workspace),
                owner_before,
                "{point}/restart={restart}: Owner resources"
            );
            assert_eq!(
                events(&ledger),
                records,
                "report does not invent a native outcome"
            );
        }
        println!("DEATH VERIFIED {point} killed_reaped=true fresh_reports=2 native_event={required_event}");
    }
}
