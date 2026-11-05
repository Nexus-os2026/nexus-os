//! Decision 9: forbid late session adoption, including the two exact review-3
//! interleavings. The legacy owner cancels its fixture ONLY after refusal;
//! this is cleanup, not a claim that the host cell halts an unrelated run.
use super::*;
use crate::authority::file_session::HaltCell;

fn host(f: &Fx, cell: HaltCell) -> Arc<AgentDoor> {
    Arc::new(
        f.gov
            .issue_agent_door_with_halt(mother(), f.owner.clone(), Some(cell))
            .unwrap(),
    )
}
fn no_worker(f: &Fx) {
    assert!(f.gov.file_session_halt().is_engaged());
    assert!(!f.gov.stop_snapshot().broker_stopped);
    assert!(!f.gov.stop_snapshot().kernel_stopped);
}
fn refused(result: &Result<CommitmentView, AuthorityError>) -> bool {
    matches!(result, Err(AuthorityError::NotMember))
}

#[test]
fn decision9_review3_execution_guard_before_adoption() {
    let f = Fx::new();
    let a = f.ready(NAME, CONTENT);
    let door = host(&f, f.gov.file_session_halt());
    let gate = Gate::new();
    let holding = gate.clone();
    f.gov
        .files()
        .hooks()
        .arm(at::FILE_EXECUTE_START, move || holding.hold());
    let (gov, run, id) = (f.gov.clone(), f.run, a.id);
    let worker = std::thread::spawn(move || gov.execute(id, &mother(), run));
    gate.reached(); // execution guard already issued with None
    let adoption = door.adopt(f.run, a.id);
    let member = f
        .gov
        .authority()
        .runs()
        .try_is_member(f.run, door.group_for_test())
        .unwrap();
    let unchanged_cell = f
        .gov
        .authority()
        .runs()
        .session_cell(f.run)
        .unwrap()
        .is_none();
    f.gov.file_session_halt().engage_emergency();
    no_worker(&f);
    // A refused takeover cannot proceed. Explicit legacy-owner cleanup lets
    // the held thread exit; never use the host cell as authority over it.
    if refused(&adoption) {
        let _ = f.door.cancel_run(f.run).unwrap().write();
    }
    gate.open();
    let execution = worker.join().unwrap();
    let published = f.ws.join(NAME).exists();
    eprintln!("FILE_EXECUTE_START: adoption={adoption:?}, member={member}, unchanged_cell={unchanged_cell}, published={published}");
    assert!(
        refused(&adoption),
        "FILE_EXECUTE_START: session door must refuse late legacy adoption"
    );
    assert!(
        !member && unchanged_cell,
        "refusal changes no membership or cell"
    );
    assert!(
        execution.is_err() && !published,
        "refused takeover leaves no publication after legacy-owner cleanup"
    );
}

#[test]
fn decision9_review3_propose_adopt_ingress() {
    let f = Fx::new();
    let a = f.propose(NAME, CONTENT).unwrap();
    let door = host(&f, f.gov.file_session_halt());
    let adoption = Arc::new(Mutex::new(None));
    let captured = adoption.clone();
    let (adopter, run) = (door.clone(), f.run);
    f.gov
        .authority()
        .commitments()
        .hooks()
        .arm(at::PREPARE_BEFORE_INSERT, move || {
            *captured.lock().unwrap() = Some(adopter.adopt(run, a.id));
        });
    let (cell, original, captured) = (f.gov.file_session_halt(), f.door.clone(), adoption.clone());
    f.gov
        .authority()
        .hooks()
        .arm(at::PLACE_BEFORE_FILL, move || {
            cell.engage_emergency(); // B finished preparation with cached None
                                     // As above, terminate the fixture through its ORIGINAL owner only
                                     // after the takeover was refused, before allowing installation.
            if refused(captured.lock().unwrap().as_ref().unwrap()) {
                let _ = original.cancel_run(run).unwrap().write();
            }
        });
    let b = f.propose("b.txt", "B\n");
    no_worker(&f);
    let adoption = adoption.lock().unwrap().take().unwrap();
    let held = f.gov.control.pending_held(f.run);
    let member = f
        .gov
        .authority()
        .runs()
        .try_is_member(f.run, door.group_for_test())
        .unwrap();
    let unchanged_cell = f
        .gov
        .authority()
        .runs()
        .session_cell(f.run)
        .unwrap()
        .is_none();
    eprintln!("PREPARE_BEFORE_INSERT -> PLACE_BEFORE_FILL: adoption={adoption:?}, pending={held}, B={b:?}");
    // Clean up the baseline's installed legacy work too, before asserting.
    let _ = f.door.cancel_run(f.run).unwrap().write();
    assert!(
        refused(&adoption),
        "PREPARE_BEFORE_INSERT: session door must refuse composed legacy adoption"
    );
    assert!(
        !member && unchanged_cell,
        "refusal changes no membership or cached-cell identity"
    );
    assert_eq!(
        held, 0,
        "refused takeover installs nothing after original-owner cleanup"
    );
    assert_eq!(f.gov.control.pending_places(), 0);
    assert!(!f.ws.join("b.txt").exists());
}

#[test]
fn decision9_legacy_adoption_control() {
    let f = Fx::new();
    let a = f.propose(NAME, CONTENT).unwrap();
    let door = f.gov.issue_agent_door(mother(), f.owner.clone()).unwrap();
    door.adopt(f.run, a.id).unwrap();
    assert!(f
        .gov
        .authority()
        .runs()
        .try_is_member(f.run, door.group_for_test())
        .unwrap());
    assert!(f
        .gov
        .authority()
        .runs()
        .session_cell(f.run)
        .unwrap()
        .is_none());
    f.authorize(&a).unwrap();
    f.execute(&a).unwrap();
    assert_eq!(std::fs::read_to_string(f.ws.join(NAME)).unwrap(), CONTENT);
}

#[test]
fn decision9_same_owner_same_cell_is_idempotent() {
    let f = Fx::new();
    let door = host(&f, f.gov.file_session_halt());
    let run = door.open_run().unwrap();
    f.gov
        .authority()
        .runs()
        .join(run, &mother(), door.group_for_test())
        .unwrap();
    let token = f.gov.authority().runs().check(run, &mother()).unwrap();
    f.gov.file_session_halt().engage_emergency();
    assert!(token.is_cancelled());
}

#[test]
fn decision9_original_door_cannot_retrofit_a_cell() {
    let f = Fx::new();
    // Even existing membership is insufficient: this run opened without it.
    let before = f.gov.authority().runs().check(f.run, &mother()).unwrap();
    f.gov
        .authority()
        .runs()
        .attach_file_cell(f.door.group_for_test(), f.gov.file_session_halt())
        .unwrap();
    let result = f
        .gov
        .authority()
        .runs()
        .join(f.run, &mother(), f.door.group_for_test());
    assert!(
        matches!(result, Err(AuthorityError::NotMember)),
        "same group is not proof of the original session cell"
    );
    assert!(f
        .gov
        .authority()
        .runs()
        .session_cell(f.run)
        .unwrap()
        .is_none());
    assert!(!before.is_cancelled());
}

#[test]
fn decision9_other_door_same_or_different_cell_refused() {
    for different in [false, true] {
        let f = Fx::new();
        let source_cell = f.gov.file_session_halt();
        let source = host(&f, source_cell.clone());
        let run = source.open_run().unwrap();
        let other_fixture = Fx::new();
        let cell = if different {
            other_fixture.gov.file_session_halt()
        } else {
            source_cell.clone()
        };
        let door = host(&f, cell);
        let result = f
            .gov
            .authority()
            .runs()
            .join(run, &mother(), door.group_for_test());
        assert!(
            matches!(result, Err(AuthorityError::NotMember)),
            "another door/cell cannot join the run: different={different}"
        );
        assert!(!f
            .gov
            .authority()
            .runs()
            .try_is_member(run, door.group_for_test())
            .unwrap());
        assert!(f
            .gov
            .authority()
            .runs()
            .session_cell(run)
            .unwrap()
            .unwrap()
            .same(&source_cell));
    }
}
