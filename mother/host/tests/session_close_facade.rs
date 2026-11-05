//! A6: the real facade owns producer debt and the ordered final obligations.
#![deny(unsafe_code)]
#![cfg(target_os = "linux")]
mod support;
use nexus_governed_control::authority::approval::*;
use nexus_governed_control::authority::clock::SystemClock;
use nexus_governed_control::evidence_store::PrimaryEvidence;
use nexus_governed_control::host_session::{
    close::{FinalStatus, ProducerKind},
    CommandKind, HostSession, SessionError, SessionSeed, SessionState,
};
use nexus_mother::adapter::{create_private, FileJournal};
use nexus_mother::journal::{Entry, Event, JournalStore, GENESIS, SESSION};
use std::sync::Arc;
use std::time::{Duration, Instant};
struct No;
impl ControlConfirmer for No {
    fn confirm_action(&self, _: &ActionConfirmation) -> bool {
        false
    }
    fn confirm_grant(&self, _: &GrantConfirmation) -> bool {
        true
    }
    fn confirm_agent_door(&self, _: &AgentDoorConfirmation) -> bool { true }
    fn confirm_resume(&self, _: &ResumeConfirmation) -> bool {
        false
    }
    fn confirm_acknowledgement(&self, _: &AcknowledgementConfirmation) -> bool {
        false
    }
}
fn child(case: &str) {
    let fixture = support::Fixture::new();
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "session_close_facade_enforces_the_complete_order",
            "--nocapture",
        ])
        .env("M2DD_CLOSE_FACADE", case)
        .env("M2DD_CLOSE_ROOT", &fixture.home)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    while child.try_wait().unwrap().is_none() {
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("bounded close child: {case}");
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let result = child.wait_with_output().unwrap();
    assert!(
        result.status.success(),
        "{case}: {}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(
        String::from_utf8_lossy(&result.stdout).contains(&format!("CLOSE FACADE VERIFIED {case}"))
    );
    let expected_end = u64::from(matches!(
        case,
        "normal" | "idle_emergency" | "producer" | "ingress_after_seal" | "retained_door" | "pending_batch" | "foreign_end"
    ));
    let expected_stop = u64::from(case == "idle_emergency");
    assert_eq!(
        support::evidence::counts(&fixture),
        [expected_stop, 1, expected_end],
        "independently verified primary records for {case}"
    );
    if case != "incomplete_history" {
        let raw = std::fs::read_to_string(fixture.home.join("mother-journal.jsonl")).unwrap();
        let entries =
            nexus_mother::journal::verify(&raw.lines().map(str::to_owned).collect::<Vec<_>>())
                .unwrap();
        assert_eq!(
            entries
                .iter()
                .filter(|entry| matches!(entry.event, Event::SessionClosed))
                .count(),
            expected_end as usize + usize::from(case == "missing_primary")
        );
    }
}
#[test]
fn session_close_facade_enforces_the_complete_order() {
    let Ok(case) = std::env::var("M2DD_CLOSE_FACADE") else {
        for case in [
            "normal",
            "idle_emergency",
            "producer",
            "lost_producer",
            "pending_stop",
            "incomplete_history",
            "ingress_after_seal",
            "missing_mother",
            "missing_primary",
            "lost_finalizer",
            "retained_door",
        ] {
            child(case);
        }
        return;
    };
    assert!(nexus_governed_control::files::make_non_dumpable());
    let root = std::path::PathBuf::from(std::env::var_os("M2DD_CLOSE_ROOT").unwrap());
    create_private(&root).unwrap();
    let seed = SessionSeed::new().unwrap();
    let ingress = seed.ingress();
    let producer = ingress.producer(ProducerKind::Input).unwrap();
    let evidence =
        PrimaryEvidence::open(&root.join("broker/p3-evidence/evidence.sqlite3"), vec![]).unwrap();
    let session = HostSession::open_with_history(
        seed,
        &root.join("broker"),
        &root,
        evidence.clone(),
        Arc::new(SystemClock::default()),
        Arc::new(No),
    )
    .unwrap();
    if case != "source_unchecked" { assert!(session.startup_report().ready()); }
    let retained = if matches!(case.as_str(), "retained_door" | "pending_batch" | "lost_batch") {
        let workspace = root.join("workspace");
        create_private(&workspace).unwrap();
        session.bootstrap_workspace(&workspace, &[]).unwrap();
        session.finish_bootstrap().unwrap();
        let run_request = ingress.accept(CommandKind::Run, 1).unwrap();
        let door = session.open_file_door(&run_request).unwrap();
        let attachment = door.file_port().unwrap().attach().unwrap();
        door.claim_file_session(&attachment).unwrap();
        let run = door.open_file_run(&attachment).unwrap();
        Some((door, run))
    } else { None };
    let mut batch = if matches!(case.as_str(), "pending_batch" | "lost_batch") {
        let (door, run) = retained.as_ref().unwrap();
        Some(door.cancel_run(*run).unwrap())
    } else { None };
    if case == "lost_batch" { drop(batch.take()); }
    let mut journal = FileJournal::open(&root.join("mother-journal.jsonl")).unwrap();
    let opening = Entry::chained(0, GENESIS, SESSION, Event::SessionOpened);
    journal.append(&opening.to_line()).unwrap();
    if matches!(case.as_str(), "idle_emergency" | "pending_stop") {
        let event = ingress.emergency().unwrap();
        if case == "idle_emergency" {
            session.record_stop(&event).unwrap();
        }
    }
    let request = ingress.accept(CommandKind::Quit, 2).unwrap();
    let deadline = request.deadline();
    let repeated = ingress.accept(CommandKind::Quit, 3).unwrap();
    assert_eq!(
        repeated.deadline(),
        deadline,
        "repeated Quit must not restart shutdown"
    );
    assert_eq!(repeated.close_identity(), request.close_identity());
    session.settle_close_authority(&request).unwrap();
    assert!(
        session.seal_close(&request).is_err(),
        "unjoined input producer blocks seal after authority closure"
    );
    if case == "lost_producer" {
        drop(producer);
        assert!(session.seal_close(&request).is_err());
        assert_eq!(session.state(), SessionState::ClosingRestricted);
    } else {
        producer.joined();
        if case == "producer" {
            assert!(
                ingress.producer(ProducerKind::Work).is_err(),
                "closing cannot create another work producer"
            );
        }
        session.settle_close_authority(&request).unwrap();
        if case == "incomplete_history" {
            use std::io::Write;
            std::fs::OpenOptions::new()
                .append(true)
                .open(root.join("mother-journal.jsonl"))
                .unwrap()
                .write_all(b"{torn")
                .unwrap();
        }
        if case == "pending_batch" {
            assert!(session.seal_close(&request).is_err(), "an unwritten native batch is independent record debt");
            assert_eq!(batch.take().unwrap().write().recorded, 1);
        }
        if matches!(case.as_str(), "pending_stop" | "incomplete_history" | "lost_batch" | "source_unchecked") {
            assert!(session.seal_close(&request).is_err());
        } else {
            let seal = session.seal_close(&request).unwrap();
            assert_eq!(session.state(), SessionState::SealedFinalizing);
            assert_eq!(ingress.emergency(), Err(SessionError::SessionEnded));
            assert!(matches!(
                ingress.accept(CommandKind::Status, 4),
                Err(SessionError::SessionEnded)
            ));
            assert!(session.seal_close(&request).is_err(), "one finalizer only");
            if case == "foreign_end" {
                assert!(seal.begin_mother_end(SessionSeed::new().unwrap().identity()).is_err());
            }
            if let Some((door, run)) = &retained {
                assert!(door.cancel_run(*run).is_err(), "a retained door cannot create settlement records after seal");
                assert!(door.finish_when_settled(*run).is_err(), "no retained post-seal finishing route");
            }
            if case == "lost_finalizer" {
                drop(seal);
                assert_eq!(session.state(), SessionState::ClosedUnclean);
            } else if case == "missing_mother" {
                assert!(session.close_primary(&seal).is_err());
                assert!(!seal.finish_delivery());
            } else {
                seal.begin_mother_end(session.identity()).unwrap();
                assert_eq!(session.close_records().mother, FinalStatus::Unknown);
                let ending = Entry::chained(1, &opening.hash, SESSION, Event::SessionClosed);
                journal.append(&ending.to_line()).unwrap();
                session.observe_mother_end(&seal).unwrap();
                assert_eq!(session.close_records().mother, FinalStatus::Recorded);
                if case == "missing_primary" {
                    assert!(!seal.finish_delivery());
                } else {
                    session.close_primary(&seal).unwrap();
                    assert_eq!(session.close_records().primary, FinalStatus::Recorded);
                    assert!(seal.finish_delivery());
                    assert_eq!(session.state(), SessionState::ClosedClean);
                }
            }
        }
    }
    println!("CLOSE FACADE VERIFIED {case}");
}

#[test]
fn session_close_requires_prior_sources_and_retains_native_record_debt() {
    for case in ["source_unchecked", "pending_batch", "lost_batch", "foreign_end"] { child(case); }
}
