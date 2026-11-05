//! Fix round 1 F/F08: native filesystem completion and record durability differ.
use super::*;

#[test]
fn fix1_f08_restore_completion_record_failure_keeps_physical_completion() {
    use crate::host_session::outcome::{GrantClosure, RecordingStatus, RestoreOutcome};
    let mut failures = Vec::new();
    for fail_record in [false, true] {
        for interrupt_after_return in [false, true] {
            let f = Fixture::new();
            let id = f.pending();
            f.approve(id).unwrap();
            let target = f._dirs.0.path().join("selected/workspace/session.txt");
            assert_eq!(std::fs::read(&target).unwrap(), b"session oracle\n");
            if fail_record {
                let connection = rusqlite::Connection::open(f.ledger_path()).unwrap();
                connection.execute_batch("CREATE TRIGGER fix1_restore_record_failure BEFORE INSERT ON coding_run_ledger WHEN NEW.event_kind='restore.completed' BEGIN SELECT RAISE(ABORT, 'injected restore record failure'); END;").unwrap();
            }
            let request = f._session.ingress().accept(CommandKind::Undo, 3).unwrap();
            if interrupt_after_return {
                let ingress = f._session.ingress();
                f.gov.files().hooks().arm("session.after.restore", move || {
                    ingress.emergency().unwrap();
                });
            }
            let result = f._session.undo(&request);
            let facts = request.restore_observation();
            let records = events(&f.ledger_path());
            let actual_completion_records = matching(&records, "restore.completed", None);
            let inventory = f._session.inventory().unwrap();
            println!("FIX1-F08 fail_record={fail_record} stop_after_return={interrupt_after_return} target_absent={} completion_records={actual_completion_records} facts={facts:?} returned_ok={} unresolved={}", !target.exists(), result.is_ok(), f._session.resource_obligations().unwrap().unresolved);
            assert!(
                !target.exists(),
                "identity-checked restore physically removed the original file"
            );
            assert_eq!(actual_completion_records, usize::from(!fail_record));
            assert!(facts.handed_off);
            assert_eq!(facts.interrupted, interrupt_after_return);
            assert_eq!(facts.grant, GrantClosure::Closed);
            let kernel_recording = f._session.operation_observation(&request).unwrap().kernel;
            let expected_recording = if fail_record {
                RecordingStatus::Owed
            } else {
                RecordingStatus::Recorded
            };
            if facts.outcome != RestoreOutcome::Completed || kernel_recording != expected_recording
            {
                failures.push(format!("fail_record={fail_record}/stop={interrupt_after_return}: physical={:?}, kernel_recording={kernel_recording:?}", facts.outcome));
            }
            if fail_record {
                assert!(result.is_err());
                assert!(inventory.entries.iter().any(|entry| !entry.discharged));
                assert!(f._session.resource_obligations().unwrap().unresolved > 0);
                assert!(records
                    .iter()
                    .any(|(kind, value)| kind == "apply.recovery_required"
                        && value["completion_unrecorded"] == "restore"));
            }
        }
    }
    assert!(failures.is_empty(), "a failed completion append cannot erase conclusive physical completion or independent record debt: {failures:?}");
}
