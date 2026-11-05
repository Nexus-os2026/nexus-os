//! Review 1, mechanism I/F12: journal durability never owns global action metadata.
use super::*;
use crate::host_session::outcome::{Acknowledgement, RecordingStatus};
use crate::test_hooks::Gate;
use std::sync::mpsc;

fn terminal_record(f: &Fixture, id: CommitmentId) {
    use sha2::{Digest, Sha256};
    let records = [
        (0, serde_json::json!({"event": "session_opened"})),
        (
            1,
            serde_json::json!({"event": "approval_pending", "effect": 1,
            "pending": {"run": f.run.to_string(), "commitment": id.to_string()}}),
        ),
        (
            1,
            serde_json::json!({"event": "transition", "trigger": {
            "trigger": "action_denied", "effect": 1}}),
        ),
    ];
    let mut previous = "0".repeat(64);
    let mut text = String::new();
    for (sequence, (mission, event)) in records.iter().enumerate() {
        let event = serde_json::to_string(event).unwrap();
        let mut digest = Sha256::new();
        digest.update(b"nexus-mother-journal-v2\n");
        for part in [
            previous.as_bytes(),
            &[2],
            &(sequence as u64).to_be_bytes(),
            &(*mission as u64).to_be_bytes(),
            event.as_bytes(),
        ] {
            digest.update((part.len() as u64).to_be_bytes());
            digest.update(part);
        }
        let hash = hex::encode(digest.finalize());
        text.push_str(&format!("{{\"format\":2,\"seq\":{sequence},\"prev\":\"{previous}\",\"mission\":{mission},\"event\":{event},\"hash\":\"{hash}\"}}\n"));
        previous = hash;
    }
    let path = f._root.0.path().join("mother-journal.jsonl");
    std::fs::write(&path, text).unwrap();
    std::fs::File::open(path).unwrap().sync_all().unwrap();
    std::fs::File::open(f._root.0.path())
        .unwrap()
        .sync_all()
        .unwrap();
}

#[test]
fn fix1_i_ack_sync_does_not_hold_session_action_inventory() {
    for point in [
        "ack.terminal.file_sync",
        "ack.terminal.parent_sync",
        "lite.before_ack",
    ] {
        let f = Arc::new(Fixture::new());
        let id = match f
            .door
            .create_file_effect(
                f.run,
                &FileCreate {
                    name: "ack.txt".into(),
                    content: "ack fixture\n".into(),
                },
                1,
            )
            .unwrap()
        {
            AgentOutcome::AwaitingApproval(view) => view.id,
            _ => panic!("separate approval required"),
        };
        f.gov.deny(id, f.door.agent(), f.run).unwrap();
        terminal_record(&f, id);
        let gate = Gate::new();
        let held = gate.clone();
        if point == "lite.before_ack" {
            f.gov.hooks().arm(point, move || held.hold());
        } else {
            f.gov.file_session_halt().1.arm(point, move || held.hold());
        }
        let ack_fixture = f.clone();
        let ack = std::thread::spawn(move || ack_fixture.door.ack_commitment(id));
        gate.reached();
        assert!(f.gov.authority().commitments().pinned_for_test(id));
        let (observed_tx, observed_rx) = mpsc::channel();
        let observed_fixture = f.clone();
        let observer = std::thread::spawn(move || {
            let facts = observed_fixture
                ._session
                .operation_observation(&observed_fixture.request);
            observed_tx.send(facts).unwrap();
        });
        let (reserved_tx, reserved_rx) = mpsc::channel();
        let reservation_fixture = f.clone();
        let reservation = std::thread::spawn(move || {
            reserved_tx
                .send(reservation_fixture.propose_content("another.txt", "next\n"))
                .unwrap();
        });
        let observed = observed_rx.recv_timeout(Duration::from_secs(2));
        let reserved = reserved_rx.recv_timeout(Duration::from_secs(2));
        let pinned_while_held = f.gov.authority().commitments().pinned_for_test(id);
        // Always release and join, including the baseline defect path.
        gate.open();
        let ack_result = ack.join().unwrap();
        observer.join().unwrap();
        reservation.join().unwrap();
        println!("F12/{point}: observation_during_sync={} reservation_during_sync={} exact_pin_retained={} ack_completed={}",
            observed.is_ok(), reserved.is_ok(), pinned_while_held, ack_result.is_ok());
        assert!(
            observed.is_ok(),
            "F12: observation blocked on acknowledgement journal synchronization at {point}"
        );
        assert!(
            reserved.is_ok(),
            "F12: action reservation blocked on acknowledgement journal synchronization at {point}"
        );
        assert!(
            reserved.unwrap().is_ok(),
            "available capacity must admit the second real preparation"
        );
        let observed = observed.unwrap().unwrap();
        let first = observed
            .actions
            .iter()
            .find(|action| action.commitment == Some(id))
            .unwrap();
        assert_eq!(first.acknowledgement, Acknowledgement::Pending);
        assert_eq!(
            first.mother,
            if point == "lite.before_ack" {
                RecordingStatus::Recorded
            } else {
                RecordingStatus::Unknown
            }
        );
        assert!(
            pinned_while_held,
            "the original pin cannot release before verified durable receipt"
        );
        assert!(ack_result.is_ok());
        assert!(!f.gov.authority().commitments().pinned_for_test(id));
        let facts = f._session.operation_observation(&f.request).unwrap();
        let first = facts
            .actions
            .iter()
            .find(|action| action.commitment == Some(id))
            .unwrap();
        assert_eq!(first.acknowledgement, Acknowledgement::Verified);
        assert_eq!(first.mother, RecordingStatus::Recorded);
        assert_eq!(first.mother_sequence, Some(2));
        assert!(
            f.door.ack_commitment(id).is_ok(),
            "only the exact completed receipt is idempotent"
        );
    }
}
