//! A5: a root approval names a published wait and is the sole parent of
//! subsequent Mother work. Old contexts remain immutable observations.
use super::*;
use crate::adapter::FileJournal;
use crate::oracles::lite_support::files_lanes;
use crate::oracles::m2c_confirmers::answering;
use crate::oracles::{contract_with, Yes};
use nexus_governed_control::host_session::{CommandKind, SessionError, SessionIngress};
use std::sync::atomic::{AtomicU64, Ordering};

const CASE: &str = "M2DD_APPROVAL_CONTINUATION";
fn child(test: &str, case: &str) {
    let mut process = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            &format!("coordinator::lite::session_continuation_oracles::{test}"),
            "--nocapture",
        ])
        .env(CASE, case)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while process.try_wait().unwrap().is_none() {
        if std::time::Instant::now() >= deadline {
            process.kill().unwrap();
            process.wait().unwrap();
            panic!("bounded continuation child: {case}");
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let output = process.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{case}: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains(&format!("CONTINUATION VERIFIED {case}"))
    );
}

fn waiting(
    home: &crate::adapter::session_support::Fixture,
    store: impl JournalStore + 'static,
) -> (FileSession<crate::adapter::SessionPort>, MissionId) {
    waiting_with(home, store, "MKFILE:session.txt")
}
fn waiting_with(
    home: &crate::adapter::session_support::Fixture,
    store: impl JournalStore + 'static,
    markers: &str,
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
    let mut contract = contract_with(markers);
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
    if std::env::var(CASE).as_deref() != Ok("publication") {
        child(
            "session_approval_target_is_not_published_inside_either_wait_append",
            "publication",
        );
        return;
    }
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
    println!("CONTINUATION VERIFIED publication");
}

#[test]
fn session_approval_continuation_binds_the_new_root_and_refuses_changed_mission() {
    assert!(nexus_governed_control::files::make_non_dumpable());
    let Ok(case) = std::env::var(CASE) else {
        for case in ["control", "stopped", "advanced"] {
            child(
                "session_approval_continuation_binds_the_new_root_and_refuses_changed_mission",
                case,
            );
        }
        return;
    };
    assert!(matches!(case.as_str(), "control" | "stopped" | "advanced"));
    for stop_before_continuation in [case == "stopped"] {
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
        } else if case == "advanced" {
            session.advance(id).unwrap();
            assert_ne!(
                session.state(id).unwrap(),
                Some(MissionState::AwaitingApproval)
            );
        }
        let continued = session.continue_approval(continuation);
        if stop_before_continuation || case == "advanced" {
            assert!(
                continued.is_err(),
                "settlement/changed generation cannot be refreshed by a saved continuation"
            );
            println!("CONTINUATION VERIFIED {case}");
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
        println!("CONTINUATION VERIFIED {case}");
    }
}

#[test]
fn session_approval_two_files_keep_distinct_roots_and_reject_a_queued_replacement() {
    if std::env::var(CASE).as_deref() != Ok("two-files") {
        child(
            "session_approval_two_files_keep_distinct_roots_and_reject_a_queued_replacement",
            "two-files",
        );
        return;
    }
    assert!(nexus_governed_control::files::make_non_dumpable());
    let home = crate::adapter::session_support::Fixture::new("two-cont");
    let (session, id) = waiting_with(
        &home,
        home.journal(),
        "MKFILE:session.txt MKFILE:second.txt",
    );
    let first = home
        .session
        .ingress()
        .accept(CommandKind::Approve, 100)
        .unwrap();
    let queued = home
        .session
        .ingress()
        .accept(CommandKind::Approve, 101)
        .unwrap();
    assert_eq!(first.context(), queued.context());
    home.session.approve_file(&first).unwrap();
    let continuation = home.session.approval_continuation(&first).unwrap();
    assert!(
        home.session.approval_continuation(&first).is_err(),
        "continuation ownership transfers only once"
    );
    let session = session.continue_approval(continuation).unwrap();
    session.advance(id).unwrap();
    assert_eq!(
        session.state(id).unwrap(),
        Some(MissionState::AwaitingApproval)
    );
    assert!(home.workspace.join("session.txt").exists());
    assert!(!home.workspace.join("second.txt").exists());
    assert!(
        home.session.approve_file(&queued).is_err(),
        "accepted old target cannot transfer to replacement"
    );
    assert!(!home.workspace.join("second.txt").exists());
    let second = home
        .session
        .ingress()
        .accept(CommandKind::Approve, 102)
        .unwrap();
    assert_ne!(first.context(), second.context());
    home.session.approve_file(&second).unwrap();
    let session = session
        .continue_approval(home.session.approval_continuation(&second).unwrap())
        .unwrap();
    assert_eq!(session.coordinator.port.context().request(), 102);
    assert_eq!(home.port.context().request(), 1);
    for _ in 0..8 {
        if session.state(id).unwrap() == Some(MissionState::Done) {
            break;
        }
        session.advance(id).unwrap();
    }
    assert_eq!(session.state(id).unwrap(), Some(MissionState::Done));
    assert!(home.workspace.join("second.txt").exists());
    let lines = home.lines();
    for end in 0..=lines.len() {
        let prefix = crate::journal::MemoryJournal::new();
        prefix.set_lines(lines[..end].to_vec());
        crate::coordinator::inspect_history(&prefix).unwrap();
    }
    println!("CONTINUATION VERIFIED two-files");
}
