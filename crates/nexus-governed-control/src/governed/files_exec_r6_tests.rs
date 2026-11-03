//! M2d-C round 6 (R6-1, R6-3 with X1, X2; pre-registered before the
//! repair): the engaged cell is the only "stopped" fact.
//!
//! - R6-1: every run admission reads the run's cells: `admit_files`,
//!   `admit_inference` and `admit_model_call` too (review 5, MEDIUM 1),
//!   refused with nothing bound and no attempt counted.
//! - R6-3 / X1: the broker's emergency ingress engages every cell (a
//!   mission with no run among them) before anything is recorded, with its
//!   first cause, which survives the Owner's lift and a replacement door; a
//!   later engagement keeps the first cause.
//! - X2: the identity-less fallback's emptiness is the whole agent
//!   partition and its agent runs (unreadable: an error); a journal's anchor
//!   lives while any of its cells does; a run says whether it carries a
//!   cell.
//!
//! A child of the B2 oracles.

use super::*;
use crate::authority::halt_cell::HaltCause;
use crate::authority::run::RunOrigin;

/// R6-1: with the run's cell engaged, each admission route is refused
/// `halted` and binds nothing.
#[test]
fn m2d_c_r6_every_admission_reads_the_cells() {
    let f = Fx::new();
    let cell = f.door.halt_cells().cell("journal", 1).unwrap();
    let run = f.door.open_run_with_halt(&cell).unwrap();
    cell.engage();
    assert_eq!(class(f.door.admit_files(run, HASH, LABEL)), "halted");
    let runs = f.gov.authority().runs();
    let group = f.door.group_for_test();
    assert_eq!(
        class(runs.admit_inference(run, &mother(), group, HASH, LABEL, [7; 32])),
        "halted"
    );
    assert_eq!(
        class(
            runs.admit_model_call(run, &mother(), group, &[7; 32])
                .map(|_| ())
        ),
        "halted"
    );
    assert!(runs.files_admission(run).is_none(), "nothing bound");
}

/// R6-3 / X1: the emergency ingress engages every cell first, a mission
/// with no run included, with its cause; the lift and a replacement door
/// keep it; a later engagement keeps the first cause.
#[test]
fn m2d_c_r6_the_emergency_ingress_engages_every_cell() {
    let f = Fx::new();
    let cells = f.door.halt_cells();
    let runless = cells.cell("journal", 1).unwrap();
    let running = cells.cell("journal", 2).unwrap();
    let _run = f.door.open_run_with_halt(&running).unwrap();
    let stopped = cells.cell("journal", 3).unwrap();
    stopped.engage_with(HaltCause::Stop);
    f.gov.emergency_stop();
    assert_eq!(runless.cause(), Some(HaltCause::Emergency));
    assert_eq!(running.cause(), Some(HaltCause::Emergency));
    assert_eq!(
        stopped.cause(),
        Some(HaltCause::Stop),
        "the first cause stays"
    );
    f.gov.resume(&*f.owner).unwrap();
    let twin = f.gov.issue_agent_door(mother(), f.owner.clone()).unwrap();
    let again = twin.halt_cells().cell("journal", 1).unwrap();
    assert_eq!(again.cause(), Some(HaltCause::Emergency), "after the lift");
    again.engage_with(HaltCause::TimeBudget);
    assert_eq!(runless.cause(), Some(HaltCause::Emergency));
}

/// X2: the fallback's emptiness covers the whole agent partition and the
/// agent's live runs, and is an error when the runs cannot be read.
#[test]
fn m2d_c_r6_the_partition_emptiness_covers_cells_and_runs() {
    let f = Fx::new();
    let other = AgentId::new("lonely").unwrap();
    let door = f
        .gov
        .issue_agent_door(other.clone(), f.owner.clone())
        .unwrap();
    let cells = door.halt_cells();
    assert_eq!(cells.partition_empty(), Ok(true));
    let cell = cells.cell("j", 1).unwrap();
    assert_eq!(cells.partition_empty(), Ok(false), "a live cell");
    cells.retire("j", 1);
    let _ = cell;
    assert_eq!(cells.partition_empty(), Ok(true));
    let _run = f
        .gov
        .authority()
        .open_run(other.clone(), RunOrigin::AgentGoal)
        .unwrap();
    assert_eq!(cells.partition_empty(), Ok(false), "a live cell-less run");
    f.gov.authority().runs().poison_for_test();
    assert!(cells.partition_empty().is_err(), "unreadable");
}

/// X2: a journal's anchor is kept while any of its cells is, dropped with
/// the last; a run says whether it carries a cell.
#[test]
fn m2d_c_r6_the_anchor_lives_with_the_cells_and_runs_name_theirs() {
    let f = Fx::new();
    let cells = f.door.halt_cells();
    let anchor = Arc::new(());
    let first = cells.cell("anchored", 1).unwrap();
    let _second = cells.cell("anchored", 2).unwrap();
    cells.anchor("anchored", Box::new(anchor.clone()));
    assert_eq!(Arc::strong_count(&anchor), 2);
    cells.retire("anchored", 1);
    assert_eq!(Arc::strong_count(&anchor), 2, "one cell remains");
    cells.retire("anchored", 2);
    assert_eq!(Arc::strong_count(&anchor), 1, "dropped with the last");
    let run = f.door.open_run_with_halt(&first).unwrap();
    let elsewhere = cells.cell("anchored", 9).unwrap();
    assert_eq!(f.door.run_carries(run, &first), Ok(Some(true)));
    assert_eq!(f.door.run_carries(run, &elsewhere), Ok(Some(false)));
    let unknown = RunId::parse("run-00000000000000000000000000000042").unwrap();
    assert_eq!(f.door.run_carries(unknown, &first), Ok(None));
}
