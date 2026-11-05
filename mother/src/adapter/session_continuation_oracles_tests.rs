//! A5: a root approval names a published wait and is the sole parent of
//! subsequent Mother work. Old contexts remain immutable observations.
use super::*;
use crate::adapter::FileJournal;
use crate::oracles::lite_support::files_lanes;
use crate::oracles::m2c_confirmers::answering;
use crate::oracles::{contract_with, Yes};
use nexus_governed_control::host_session::{CommandKind, SessionError, SessionIngress};
use std::sync::atomic::{AtomicU64, Ordering};

fn waiting(
    home: &crate::adapter::session_support::Fixture,
    store: impl JournalStore + 'static,
) -> (FileSession<crate::adapter::SessionPort>, MissionId) {
    let session = Coordinator::open(
        store,
        home.port.clone(),
        Lanes::new(files_lanes()).unwrap(),
        home.clock.clone(),
        home.private(),
    )
    .unwrap()
    .into_file_session()
    .unwrap();
    let mut contract = contract_with("MKFILE:session.txt");
    contract.requested_folder = Some(home.session.workspace().unwrap().name);
    let id = session.draft(contract).unwrap();
    session.submit(id).unwrap();
    answering(&Yes, || session.request_consent(id)).unwrap();
    for _ in 0..8 {
        if session.state(id).unwrap() == Some(MissionState::AwaitingApproval) {
            return (session, id);
        }
        session.advance(id).unwrap();
    }
    panic!("real mission never reached its pending File approval");
}

struct ObserveWait {
    journal: FileJournal,
    ingress: SessionIngress,
    sequence: AtomicU64,
    observed: Arc<Mutex<Vec<Result<(), SessionError>>>>,
}
impl JournalStore for ObserveWait {
    fn load(&self) -> Result<Vec<String>, JournalError> {
        self.journal.load()
    }
    fn poison(&mut self) -> Result<(), JournalError> {
        self.journal.poison()
    }
    fn append(&mut self, line: &str) -> Result<(), JournalError> {
        let entry: Entry = serde_json::from_str(line).unwrap();
        let waiting = matches!(
            entry.event,
            Event::ApprovalPending { .. }
                | Event::Transition {
                    trigger: Trigger::AwaitApproval { .. }
                }
        );
        for after in [false, true] {
            if after {
                self.journal.append(line)?;
            }
            if waiting {
                let sequence = self.sequence.fetch_add(1, Ordering::SeqCst);
                self.observed.lock().unwrap().push(
                    self.ingress
                        .accept(CommandKind::Approve, sequence)
                        .map(|_| ()),
                );
            }
        }
        Ok(())
    }
}

#[test]
fn session_approval_target_is_not_published_inside_either_wait_append() {
    assert!(nexus_governed_control::files::make_non_dumpable());
    let home = crate::adapter::session_support::Fixture::new("pending-publication");
    let observed = Arc::new(Mutex::new(Vec::new()));
    let store = ObserveWait {
        journal: home.journal(),
        ingress: home.session.ingress(),
        sequence: AtomicU64::new(2),
        observed: observed.clone(),
    };
    let (_session, _) = waiting(&home, store);
    assert_eq!(
        *observed.lock().unwrap(),
        vec![Err(SessionError::NoTarget); 4],
        "a command cannot bind a wait before both durable records and their projection return"
    );
    let request = home
        .session
        .ingress()
        .accept(CommandKind::Approve, 100)
        .unwrap();
    assert!(
        request.context().is_some(),
        "same real route publishes its exact target after waiting"
    );
    assert!(!home.workspace.join("session.txt").exists());
}

#[test]
fn session_approval_continuation_binds_the_new_root_and_refuses_changed_mission() {
    assert!(nexus_governed_control::files::make_non_dumpable());
    for stop_before_continuation in [false, true] {
        let label = if stop_before_continuation {
            "continuation-stopped"
        } else {
            "continuation-control"
        };
        let home = crate::adapter::session_support::Fixture::new(label);
        let old_context = home.port.context();
        let (session, id) = waiting(&home, home.journal());
        let generation = session
            .coordinator
            .inner
            .lock()
            .unwrap()
            .mission(id)
            .unwrap()
            .generation;
        let request = home
            .session
            .ingress()
            .accept(CommandKind::Approve, 100)
            .unwrap();
        home.session.approve_file(&request).unwrap();
        assert!(home.workspace.join("session.txt").exists());
        let continuation = home.session.approval_continuation(&request).unwrap();
        assert_eq!(continuation.mission(), id.0);
        assert_eq!(continuation.generation(), generation);
        assert_eq!(old_context.request(), 1);
        if stop_before_continuation {
            session.stop(id).unwrap();
        }
        let continued = session.continue_approval(continuation);
        if stop_before_continuation {
            assert!(
                continued.is_err(),
                "settlement/changed generation cannot be refreshed by a saved continuation"
            );
            continue;
        }
        let continued = continued.unwrap();
        assert_eq!(
            continued
                .coordinator
                .inner
                .lock()
                .unwrap()
                .session_context
                .as_ref()
                .unwrap()
                .request(),
            100
        );
        assert_eq!(
            old_context.request(),
            1,
            "retained callbacks keep their original context"
        );
        assert_eq!(
            home.port.context().request(),
            1,
            "retained port is not globally refreshed"
        );
        for _ in 0..8 {
            if continued.state(id).unwrap() == Some(MissionState::Done) {
                break;
            }
            continued.advance(id).unwrap();
        }
        assert_eq!(continued.state(id).unwrap(), Some(MissionState::Done));
        let lines = home.lines();
        for end in 0..=lines.len() {
            let prefix = crate::journal::MemoryJournal::new();
            prefix.set_lines(lines[..end].to_vec());
            crate::coordinator::inspect_history(&prefix).unwrap();
        }
    }
}
