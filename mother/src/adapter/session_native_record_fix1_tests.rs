//! Fix round 1 A/F01: a failed best-effort cleanup record is independent debt.
use super::*;
use nexus_governed_control::host_session::outcome::Acknowledgement;
use std::sync::atomic::Ordering;

const TEST: &str = "coordinator::lite::session_close_oracles::native_record_fix1::fix1_a_discard_record_failure_keeps_exact_acked_resource_debt";

#[test]
fn fix1_a_discard_record_failure_keeps_exact_acked_resource_debt() {
    let Ok(case) = std::env::var("M2DD_FIX1_A") else {
        for case in ["control", "discard_record_failure"] {
            let mut child = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", TEST, "--nocapture"])
                .env("M2DD_FIX1_A", case)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .unwrap();
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
            while child.try_wait().unwrap().is_none() {
                if std::time::Instant::now() >= deadline {
                    child.kill().unwrap();
                    child.wait().unwrap();
                    panic!("bounded F01 child: {case}");
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            let output = child.wait_with_output().unwrap();
            println!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(output.status.success(), "F01 child {case} failed");
        }
        return;
    };
    assert!(matches!(
        case.as_str(),
        "control" | "discard_record_failure"
    ));
    assert!(nexus_governed_control::files::make_non_dumpable());
    let home = Fixture::new("fix1-discard-record");
    let file = Coordinator::open(
        home.journal(),
        home.port.clone(),
        Lanes::new(files_lanes()).unwrap(),
        home.clock.clone(),
        home.private(),
    )
    .unwrap()
    .into_file_session()
    .unwrap();
    let mut contract = contract_with("MKFILE:declined.txt");
    contract.requested_folder = Some(home.session.workspace().unwrap().name);
    let id = file.draft(contract).unwrap();
    file.submit(id).unwrap();
    answering(&Yes, || file.request_consent(id)).unwrap();
    for _ in 0..8 {
        if file.state(id).unwrap() == Some(MissionState::AwaitingApproval) {
            break;
        }
        file.advance(id).unwrap();
    }
    assert_eq!(
        file.state(id).unwrap(),
        Some(MissionState::AwaitingApproval)
    );
    home.owner.decline.store(true, Ordering::SeqCst);
    let approval = home
        .session
        .ingress()
        .accept(CommandKind::Approve, 2)
        .unwrap();
    assert!(home.session.approve_file(&approval).is_err());
    file.advance(id).unwrap();
    assert_eq!(file.state(id).unwrap(), Some(MissionState::Denied));
    let facts = home.session.operation_observation(&approval).unwrap();
    assert_eq!(facts.actions.len(), 1);
    assert_eq!(facts.actions[0].acknowledgement, Acknowledgement::Verified);
    assert!(facts.actions[0].mother_sequence.is_some());
    assert!(facts.actions[0].mother_hash.is_some());
    let ledger = home.dir.join("broker/files/ledger.db");
    if case == "discard_record_failure" {
        let output = std::process::Command::new("python3").arg("-c").arg(r#"
import sqlite3, sys
con = sqlite3.connect(sys.argv[1])
con.execute("CREATE TRIGGER fix1_discard_record_failure BEFORE INSERT ON coding_run_ledger WHEN NEW.event_kind='staging.discarded' BEGIN SELECT RAISE(ABORT, 'injected discard record failure'); END")
con.commit()
"#).arg(&ledger).output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let quit = home.session.ingress().accept(CommandKind::Quit, 3).unwrap();
    let ending = file.prepare_session_end().unwrap();
    let inventory = home.session.settle_close_resources(&quit).unwrap();
    home.session.settle_close_authority(&quit).unwrap();
    assert_eq!(inventory.entries.len(), 1);
    let entry = &inventory.entries[0];
    assert!(entry.staging_absent);
    assert!(!home.workspace.join("declined.txt").exists());
    let staging = home.dir.join("broker/files/coding-runs");
    assert_eq!(
        std::fs::read_dir(staging).unwrap().count(),
        0,
        "actual physical staging removal completed"
    );
    let output = std::process::Command::new("python3").arg("-c").arg(r#"
import sqlite3, sys
con = sqlite3.connect('file:' + sys.argv[1] + '?mode=ro', uri=True)
print(con.execute("SELECT count(*) FROM coding_run_ledger WHERE event_kind='staging.discarded'").fetchone()[0])
"#).arg(&ledger).output().unwrap();
    assert!(output.status.success());
    let recorded: usize = String::from_utf8(output.stdout)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    let debt = home.session.resource_obligations().unwrap();
    let seal = home.session.seal_close(&quit);
    let expected_recorded = case == "control";
    println!("FIX1-A case={case} physical_staging_absent={} actual_discard_records={recorded} cleanup_recorded={} discharged={} unresolved={} seal_ok={} exact_mother_ack=Verified", entry.staging_absent, entry.cleanup_recorded, entry.discharged, debt.unresolved, seal.is_ok());
    assert_eq!(recorded, usize::from(expected_recorded));
    assert_eq!(entry.cleanup_recorded, expected_recorded, "missing native record remains independent debt after physical removal and exact Mother acknowledgement");
    assert_eq!(entry.discharged, expected_recorded);
    assert_eq!(debt.unresolved, usize::from(!expected_recorded));
    assert_eq!(seal.is_ok(), expected_recorded);
    if let Ok(seal) = seal {
        ending.record(&seal).unwrap();
        home.session.observe_mother_end(&seal).unwrap();
        home.session.close_primary(&seal).unwrap();
        assert!(seal.finish_delivery());
        assert_eq!(home.session.state(), SessionState::ClosedClean);
    }
}
