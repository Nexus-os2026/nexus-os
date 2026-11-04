//! D3, real publication interruption, unknown evidence and exact Owner notice.
use super::*;
use crate::adapter::lite_clock::{past_the_start, LiteHome};
use nexus_governed_control::authority::clock::Clock;
use nexus_governed_control::authority::commitment::{CommitmentState, PublicationClass};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    mpsc,
};
const CASE: &str = "NEXUS_LITE_RECOGNITION";
#[test]
fn lite_recognition_scenarios() {
    for case in [
        "pending",
        "denied",
        "unknown",
        "withdrawal-failure",
        "interrupted",
        "deadline-empty",
        "deadline-published",
        "deadline-executing",
    ] {
        let out = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "adapter::lite::recognition::lite_recognition_child",
                "--nocapture",
            ])
            .env(CASE, case)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{case}: {}\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(String::from_utf8_lossy(&out.stdout).contains(&format!("lite-recognition:{case}")));
    }
}
#[test]
fn lite_recognition_child() {
    let Ok(case) = std::env::var(CASE) else {
        return;
    };
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp = std::env::temp_dir().join(format!("lite-recognition-{nanos}"));
    create_private(&temp).unwrap();
    let home = LiteHome::open(&temp.join("home")).unwrap();
    let ws = temp.join("workspace");
    create_private(&ws).unwrap();
    let info = home.control.register_workspace(&ws, &[]).unwrap();
    let owner = FilesOwner::default();
    home.control
        .request_grant(
            &GrantRequest::Files { workspace: info.id },
            Duration::from_secs(86400),
            &owner,
        )
        .unwrap();
    let port = Arc::new(port_for(&home.control, true).with_workspace(info.name.clone()));
    let journal = MemoryJournal::default();
    let m = Arc::new(
        Coordinator::open(
            journal.clone(),
            port.clone(),
            Lanes::new(files_lanes()).unwrap(),
            Arc::new(SystemClock::default()),
            home.private(),
        )
        .unwrap()
        .into_file_session()
        .unwrap(),
    );
    let mut contract = contract_with("MKFILE:one.txt MKFILE:two.txt");
    contract.requested_folder = Some(info.name);
    let id = m.draft(contract).unwrap();
    m.submit(id).unwrap();
    answering(&Yes, || m.request_consent(id)).unwrap();
    if case.starts_with("deadline-") {
        let clock = home.clock.clone();
        port.in_broker_action.arm(0, move || {
            // The first reading is the port's asked_at. Move both clocks at
            // the next reading, before the broker creates its commitment.
            let reads = AtomicUsize::new(0);
            let advance = clock.clone();
            clock.arm(
                move || reads.fetch_add(1, Ordering::SeqCst) == 1,
                move || advance.advance(Duration::from_secs(30)),
            );
        });
    }
    for _ in 0..3 {
        m.advance(id).unwrap();
    }
    let pending = port.pending(id).unwrap();
    let run = RunId::parse(&pending.run).unwrap();
    let commitment = CommitmentId::parse(&pending.commitment).unwrap();
    assert_eq!(m.state(id).unwrap(), Some(MissionState::AwaitingApproval));
    let mut worker = None;
    let mut release = None;
    match case.as_str() {
        "pending" => {
            let snapshot = port.file_publication(id).unwrap().unwrap();
            assert_eq!(snapshot.publication, PublicationClass::Pending);
            m.advance(id).unwrap();
            assert_eq!(m.state(id).unwrap(), Some(MissionState::AwaitingApproval));
        }
        "denied" => {
            assert!(home
                .control
                .authorize(commitment, &mother_agent(), run, &crate::oracles::No)
                .is_err());
            assert_eq!(
                port.file_publication(id).unwrap().unwrap().publication,
                PublicationClass::ConfirmedNotPublished
            );
            m.advance(id).unwrap();
            assert_eq!(
                m.state(id).unwrap(),
                Some(MissionState::Denied),
                "proved native denial remains Denied"
            );
            assert_eq!(port.acks_tried(), 1);
        }
        "withdrawal-failure" => {
            assert!(!home.evidence.health().latched);
            let uncertain = home.control.status().halt_records.uncertain;
            let seen = home.gate.count("run_cancelled");
            home.gate.arm_with("run_cancelled", Some(run), || {
                panic!("withdrawal record projection interrupted")
            });
            let result = m.stop(id);
            assert_eq!(
                home.gate.count("run_cancelled"),
                seen + 1,
                "real withdrawal write reached the forced boundary"
            );
            assert_eq!(
                home.control.status().halt_records.uncertain,
                uncertain + 1,
                "native batch counts the interrupted record as uncertain"
            );
            assert_eq!(
                result,
                Err(CoordinatorError::Withdrawal(
                    crate::evidence::BrokerClass::EvidenceUnavailable
                )),
                "failed native withdrawal recording reaches the typed File caller"
            );
            assert!(m.cell_for_test().is_engaged());
            assert!(!ws.join("one.txt").exists());
        }
        "unknown" => {
            port.runs.lock().unwrap().waiting.remove(&id);
            m.advance(id).unwrap();
            assert_eq!(
                m.reason(id).unwrap().as_deref(),
                Some("file_publication_uncertain")
            );
            assert_eq!(m.notice(id).unwrap(), Some(FILE_UNCERTAIN_NOTICE));
            assert_eq!(port.acks_tried(), 0);
            assert!(m.close().is_err());
        }
        "interrupted" | "deadline-published" | "deadline-executing" => {
            home.control
                .authorize(commitment, &mother_agent(), run, &owner)
                .unwrap();
            if case == "interrupted" {
                let broker = home.control.clone();
                home.clock.arm(past_the_start(&home, run), move || {
                    broker.cancel_run(run).unwrap()
                });
            }
            if case == "deadline-executing" {
                let (entered_tx, entered_rx) = mpsc::sync_channel(1);
                let (release_tx, release_rx) = mpsc::sync_channel(1);
                home.clock.arm(past_the_start(&home, run), move || {
                    entered_tx.send(()).unwrap();
                    release_rx.recv_timeout(Duration::from_secs(10)).unwrap();
                });
                let broker = home.control.clone();
                worker = Some(std::thread::spawn(move || {
                    broker.execute(commitment, &mother_agent(), run)
                }));
                entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
                release = Some(release_tx);
                assert_eq!(
                    port.file_publication(id).unwrap().unwrap().state,
                    Some(CommitmentState::Executing)
                );
            } else {
                home.control
                    .execute(commitment, &mother_agent(), run)
                    .unwrap();
                if case == "interrupted" {
                    assert!(!home.clock.disarm());
                }
                assert_eq!(
                    std::fs::read_to_string(ws.join("one.txt")).unwrap(),
                    content_of("MKFILE", "one.txt").unwrap()
                );
            }
        }
        "deadline-empty" => {}
        _ => panic!("unknown case"),
    }
    if case.starts_with("deadline-") {
        assert!(!port.in_broker_action.armed());
        assert!(
            !home.clock.disarm(),
            "deadline gap and executing hold both fired"
        );
        let left = pending
            .deadline_mono_ms
            .saturating_sub(home.clock.monotonic_ms());
        home.clock.advance(Duration::from_millis(left + 1));
        let snapshot = port.file_publication(id).unwrap().unwrap();
        let nexus_governed_control::governed::Lookup::Mine(view) = port.door.commitment(commitment)
        else {
            panic!("owned action")
        };
        assert!(
            view.expires_wall_ms > home.clock.wall_ms(),
            "Mother deadline precedes live broker deadline"
        );
        let result = m.advance(id).unwrap();
        if case == "deadline-executing" {
            assert_eq!(snapshot.publication, PublicationClass::Pending);
            assert_eq!(
                m.reason(id).unwrap().as_deref(),
                Some("file_publication_uncertain")
            );
            assert_eq!(result.explanation.as_deref(), Some(FILE_UNCERTAIN_NOTICE));
            assert_eq!(m.notice(id).unwrap(), Some(FILE_UNCERTAIN_NOTICE));
            assert_eq!(port.acks_tried(), 0);
        } else if case == "deadline-published" {
            assert_eq!(
                m.reason(id).unwrap().as_deref(),
                Some("file_publication_interrupted:approval_deadline")
            );
        } else {
            assert_eq!(m.reason(id).unwrap().as_deref(), Some("approval_expired"));
        }
        assert_eq!(result.to, MissionState::Blocked);
    }
    if case == "interrupted" {
        m.advance(id).unwrap();
        assert_eq!(
            m.reason(id).unwrap().as_deref(),
            Some("file_publication_interrupted:cancelled")
        );
    }
    if let Some(tx) = release {
        tx.send(()).unwrap();
    }
    if let Some(worker) = worker {
        let _ = worker.join().unwrap();
    }
    let entries = crate::journal::verify(&journal.lines()).unwrap();
    assert!(
        !entries.iter().any(|e| matches!(
            e.event,
            Event::Transition {
                trigger: Trigger::ActionApproved { .. }
            }
        )),
        "no ordinary success from any negative schedule"
    );
    let publications = entries
        .iter()
        .filter(|e| {
            matches!(
                &e.event,
                Event::Transition {
                    trigger: Trigger::Settled {
                        outcome: SettlementOutcome::Published { .. },
                        ..
                    }
                }
            )
        })
        .count();
    assert_eq!(
        publications,
        usize::from(case == "interrupted" || case == "deadline-published"),
        "settlement publication is counted too"
    );
    assert!(!ws.join("two.txt").exists());
    println!("lite-recognition:{case}");
}
