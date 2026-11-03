//! M2d-C round 5 (amendment H1 with C1, C3, C6; pre-registered before the
//! API exists): a per-mission halt cell. The broker's table gives the same
//! cell for the same key to every door of the agent while the broker lives;
//! another agent's partition never sees it. A run opened with a cell refuses
//! every admission once the cell is engaged, and an execution of it sees the
//! cell at its final check even with the run registry poisoned. Other runs
//! of the same door are unaffected. A child of the B2 oracles.

use super::*;
use crate::authority::halt_cell::MAX_HALT_CELLS;
use crate::governed::Lookup;

/// C1: get-or-create, the same cell across doors of one agent; another
/// agent's partition is separate; the cohort is one snapshot; retiring
/// engages first; a poisoned table still answers; the partition is
/// bounded.
#[test]
fn m2d_c_r5_the_halt_table_keeps_one_cell_per_key() {
    let f = Fx::new();
    let cells = f.door.halt_cells();
    let first = cells.cell("journal-a", 1).unwrap();
    let twin = f.gov.issue_agent_door(mother(), f.owner.clone()).unwrap();
    let again = twin.halt_cells().cell("journal-a", 1).unwrap();
    again.engage();
    assert!(first.is_engaged(), "the same cell for a replacement door");
    let other = f
        .gov
        .issue_agent_door(AgentId::new("other").unwrap(), f.owner.clone())
        .unwrap();
    assert!(other.halt_cells().get("journal-a", 1).is_none());
    assert!(!other
        .halt_cells()
        .cell("journal-a", 1)
        .unwrap()
        .is_engaged());
    let second = cells.cell("journal-a", 2).unwrap();
    let elsewhere = cells.cell("journal-b", 1).unwrap();
    let cohort: Vec<u64> = cells
        .cohort("journal-a")
        .into_iter()
        .map(|(m, _)| m)
        .collect();
    assert_eq!(cohort, vec![1, 2]);
    cells.retire("journal-a", 2);
    assert!(second.is_engaged(), "retired engaged");
    assert!(cells.get("journal-a", 2).is_none());
    cells.poison_for_test();
    assert!(
        cells.get("journal-b", 1).is_some(),
        "a poisoned table answers"
    );
    assert!(!elsewhere.is_engaged());
    for n in 0..MAX_HALT_CELLS as u64 {
        if cells.cell("bulk", n).is_err() {
            assert!(n > 0);
            return;
        }
    }
    assert_eq!(class(cells.cell("bulk", u64::MAX)), "capacity");
}

/// H1: a run opened with a cell: engaged, the run admits no proposal and
/// no authorization; another run of the same door, with its own cell,
/// still works.
#[test]
fn m2d_c_r5_an_engaged_cell_refuses_its_run_only() {
    let f = Fx::new();
    let cells = f.door.halt_cells();
    let cell = cells.cell("journal", 7).unwrap();
    let run = f.door.open_run_with_halt(&cell).unwrap();
    f.door.admit_files(run, HASH, LABEL).unwrap();
    let file = |run, name: &str| {
        f.door.agent_action(
            run,
            &Intent::File(FileCreate {
                name: name.into(),
                content: CONTENT.into(),
            }),
        )
    };
    let waiting = match file(run, "a.txt").unwrap() {
        AgentOutcome::AwaitingApproval(view) => view,
        AgentOutcome::Done(_) => panic!("a File waits"),
    };
    cell.engage();
    assert_eq!(class(file(run, "b.txt")), "halted");
    assert!(f
        .gov
        .authorize(waiting.id, &mother(), run, &*f.owner)
        .is_err());
    let other = f
        .door
        .open_run_with_halt(&cells.cell("journal", 8).unwrap())
        .unwrap();
    f.door.admit_files(other, HASH, LABEL).unwrap();
    assert!(file(other, "c.txt").is_ok(), "another mission goes on");
    assert!(matches!(
        f.door.try_commitment(waiting.id).unwrap(),
        Lookup::Mine(_)
    ));
}

/// H1 (and Codex review 4, HIGH 1's poisoned registry): an execution of a
/// run opened with a cell, held before its final check; the run registry
/// poisoned; the cell engaged: nothing is published.
#[test]
fn m2d_c_r5_an_engaged_cell_reaches_the_final_check() {
    let f = Fx::new();
    let cell = f.door.halt_cells().cell("journal", 9).unwrap();
    let run = f.door.open_run_with_halt(&cell).unwrap();
    f.door.admit_files(run, HASH, LABEL).unwrap();
    let AgentOutcome::AwaitingApproval(view) = f
        .door
        .agent_action(
            run,
            &Intent::File(FileCreate {
                name: NAME.into(),
                content: CONTENT.into(),
            }),
        )
        .unwrap()
    else {
        panic!("a File waits");
    };
    f.gov.authorize(view.id, &mother(), run, &*f.owner).unwrap();
    let gate = Gate::new();
    let holding = gate.clone();
    f.gov
        .files()
        .hooks()
        .arm(at::FILE_BEFORE_FINAL, move || holding.hold());
    let (gov, id) = (f.gov.clone(), view.id);
    let worker = std::thread::spawn(move || gov.execute(id, &mother(), run).is_ok());
    gate.reached();
    f.gov.authority().runs().poison_for_test();
    cell.engage();
    gate.open();
    assert!(!worker.join().unwrap(), "refused at its final check");
    assert!(!f.ws.join(NAME).exists());
}
