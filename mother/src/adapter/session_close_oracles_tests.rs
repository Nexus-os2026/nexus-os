//! A6: settle and acknowledge before seal; reserve exactly one Mother end.
use super::*;
use crate::adapter::{session_support::Fixture, FileJournal};
use crate::oracles::{contract_with, lite_support::files_lanes, m2c_confirmers::answering, Yes};
use nexus_governed_control::host_session::{CommandKind, SessionState};
#[test]
fn session_pending_quit_prepares_terminal_ack_without_recording_an_end() {
    const NAME: &str = "coordinator::lite::session_close_oracles::session_pending_quit_prepares_terminal_ack_without_recording_an_end";
    if std::env::var("M2DD_CLOSE_MOTHER").as_deref() != Ok("pending") {
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", NAME, "--nocapture"])
            .env("M2DD_CLOSE_MOTHER", "pending")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        while child.try_wait().unwrap().is_none() {
            if std::time::Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("bounded Mother close child");
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    assert!(nexus_governed_control::files::make_non_dumpable());
    let home = Fixture::new("mother-close");
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
    let mut contract = contract_with("MKFILE:never.txt");
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
    let quit = home.session.ingress().accept(CommandKind::Quit, 2).unwrap();
    let ending = file.prepare_session_end().unwrap();
    let entries = crate::journal::verify(&home.lines()).unwrap();
    assert!(entries.iter().any(|e| matches!(
        e.event,
        Event::Transition {
            trigger: Trigger::Stop { .. } | Trigger::Settled { .. }
        }
    )));
    assert!(!entries
        .iter()
        .any(|e| matches!(e.event, Event::SessionClosed)));
    assert!(!home.workspace.join("never.txt").exists());
    let inventory = home.session.settle_close_resources(&quit).unwrap();
    assert!(inventory.entries.iter().all(|e| e.discharged));
    home.session.settle_close_authority(&quit).unwrap();
    let seal = home.session.seal_close(&quit).unwrap();
    ending.record(&seal).unwrap();
    home.session.observe_mother_end(&seal).unwrap();
    home.session.close_primary(&seal).unwrap();
    assert!(seal.finish_delivery());
    assert_eq!(home.session.state(), SessionState::ClosedClean);
    let entries = crate::journal::verify(
        &FileJournal::open(&home.dir.join("mother-journal.jsonl"))
            .unwrap()
            .load()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        entries
            .iter()
            .filter(|e| matches!(e.event, Event::SessionClosed))
            .count(),
        1
    );
    assert!(!home.workspace.join("never.txt").exists());
}

#[test]
fn fix2_conclusive_pending_quit_has_no_replay_debt() {
    const TEST: &str =
        "coordinator::lite::session_close_oracles::fix2_conclusive_pending_quit_has_no_replay_debt";
    if std::env::var("M2DD_FIX2_REPLAY").as_deref() != Ok("canonical") {
        let out = super::session_budget_fix1::child_case(TEST, "M2DD_FIX2_REPLAY", "canonical");
        println!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(out.status.success());
        return;
    }
    assert!(nexus_governed_control::files::make_non_dumpable());
    let home = Fixture::canonical("fix2-replay");
    let m = Coordinator::open(
        home.journal(),
        home.port.clone(),
        Lanes::new(files_lanes()).unwrap(),
        home.clock.clone(),
        home.private(),
    )
    .unwrap()
    .into_file_session()
    .unwrap();
    let mut contract = contract_with("MKFILE:never.txt");
    contract.requested_folder = Some(home.session.workspace().unwrap().name);
    let id = m.draft(contract).unwrap();
    m.submit(id).unwrap();
    answering(&Yes, || m.request_consent(id)).unwrap();
    for _ in 0..8 {
        if m.state(id).unwrap() == Some(MissionState::AwaitingApproval) {
            break;
        }
        m.advance(id).unwrap();
    }
    let quit = home.session.ingress().accept(CommandKind::Quit, 2).unwrap();
    let ending = m.prepare_session_end().unwrap();
    let lines = home.lines();
    let entries = verify(&lines).unwrap();
    let settled = entries
        .iter()
        .position(|e| {
            matches!(
                &e.event,
                Event::Transition {
                    trigger: Trigger::Settled {
                        outcome: SettlementOutcome::NotPublished,
                        ..
                    }
                }
            )
        })
        .expect("conclusive native nonpublication");
    for count in 1..=entries.len() {
        let report = inspect_history_lines(&lines[..count]).unwrap();
        if count <= settled && report.missions.iter().any(|(mission, _)| *mission == id) {
            assert!(
                report.unresolved.contains(&id),
                "incomplete prefix retains its debt"
            );
        }
    }
    let report = inspect_history_lines(&lines).unwrap();
    assert!(
        report.unresolved.is_empty(),
        "retained first cause is history, not unresolved settlement: {:?}",
        report.unresolved
    );
    assert_eq!(report.missions, vec![(id, MissionState::Stopped)]);
    assert!(!report.session_end_recorded);
    assert!(!home.workspace.join("never.txt").exists());
    assert!(home
        .session
        .settle_close_resources(&quit)
        .unwrap()
        .entries
        .iter()
        .all(|e| e.discharged));
    home.session.settle_close_authority(&quit).unwrap();
    let seal = home.session.seal_close(&quit).unwrap();
    ending.record(&seal).unwrap();
    home.session.observe_mother_end(&seal).unwrap();
    home.session.close_primary(&seal).unwrap();
    assert!(seal.finish_delivery());
    assert_eq!(home.session.state(), SessionState::ClosedClean);
    println!(
        "FIX2 REPLAY VERIFIED conclusive-pending-quit prefixes={}",
        lines.len()
    );
}

#[path = "session_lifecycle_fix1_tests.rs"]
mod lifecycle_fix1;
#[path = "session_native_record_fix1_tests.rs"]
mod native_record_fix1;
