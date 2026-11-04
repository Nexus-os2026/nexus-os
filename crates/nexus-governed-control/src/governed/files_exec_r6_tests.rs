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
    assert!(first.is_engaged(), "retired engaged");
    let carried = cells.cell("carried", 1).unwrap();
    let run = f.door.open_run_with_halt(&carried).unwrap();
    let elsewhere = cells.cell("carried", 9).unwrap();
    assert_eq!(f.door.run_carries(run, &carried), Ok(Some(true)));
    assert_eq!(f.door.run_carries(run, &elsewhere), Ok(Some(false)));
    let unknown = RunId::parse("run-00000000000000000000000000000042").unwrap();
    assert_eq!(f.door.run_carries(unknown, &carried), Ok(None));
}

/// X2″: a replayed reference to a surviving run that does not carry this
/// journal's cell is stopped through the door: every cell the run carries
/// is engaged with cause `Fence` (a first cause kept), the run is
/// cancelled and recorded; another agent's run, an unknown run and an
/// unreadable registry are typed; a run says whether it survives.
#[test]
fn m2d_c_r6_a_surviving_run_without_the_cell_is_stopped() {
    let f = Fx::new();
    let cells = f.door.halt_cells();
    let original = cells.cell("old", 1).unwrap();
    let run = f.door.open_run_with_halt(&original).unwrap();
    assert_eq!(f.door.run_survives(run), Ok(true));
    let twin = f.gov.issue_agent_door(mother(), f.owner.clone()).unwrap();
    let fresh = twin.halt_cells().cell("new", 1).unwrap();
    assert_eq!(twin.run_carries(run, &fresh), Ok(Some(false)));
    let batch = twin.stop_surviving_run(run).expect("stopped");
    assert!(!batch.is_empty(), "the cancellation is recorded");
    let _ = batch.write();
    assert_eq!(original.cause(), Some(HaltCause::Fence));
    assert!(!fresh.is_engaged(), "the replacement's cell is untouched");
    assert!(f.gov.authority().runs().view(run).unwrap().cancelled);
    assert_eq!(f.door.run_survives(run), Ok(false));
    assert_eq!(class(f.door.agent_action(run, &Intent::File(FileCreate {
        name: "x.txt".into(),
        content: "x\n".into(),
    }))), "halted");
    let stopped = cells.cell("old", 2).unwrap();
    stopped.engage_with(HaltCause::Stop);
    let other = f.door.open_run_with_halt(&stopped);
    assert_eq!(class(other), "halted", "an engaged cell opens no run");
    let unknown = RunId::parse("run-00000000000000000000000000000042").unwrap();
    assert_eq!(class(twin.stop_surviving_run(unknown)), "unknown_run");
    assert_eq!(f.door.run_survives(unknown), Ok(false));
    let lonely = f
        .gov
        .issue_agent_door(AgentId::new("lonely").unwrap(), f.owner.clone())
        .unwrap();
    assert_eq!(class(lonely.stop_surviving_run(run)), "wrong_agent");
    f.gov.authority().runs().poison_for_test();
    assert!(twin.stop_surviving_run(run).is_err(), "unreadable");
    assert!(f.door.run_survives(run).is_err(), "unreadable");
}

/// Execute `view` of `run` on another thread, held at `point` by a gate.
fn held_on(
    f: &Fx,
    run: RunId,
    view: &CommitmentView,
    point: &'static str,
) -> (Gate, std::thread::JoinHandle<bool>) {
    let gate = Gate::new();
    let holding = gate.clone();
    f.gov.files().hooks().arm(point, move || holding.hold());
    let (gov, id) = (f.gov.clone(), view.id);
    let worker = std::thread::spawn(move || gov.execute(id, &mother(), run).is_ok());
    gate.reached();
    (gate, worker)
}

/// A File of `run` proposed through `door` and authorized by the Owner.
fn authorized(f: &Fx, door: &AgentDoor, run: RunId, name: &str) -> CommitmentView {
    let AgentOutcome::AwaitingApproval(view) = door
        .agent_action(
            run,
            &Intent::File(FileCreate {
                name: name.into(),
                content: CONTENT.into(),
            }),
        )
        .unwrap()
    else {
        panic!("a Files create waits for the Owner");
    };
    f.gov
        .authorize(view.id, &mother(), run, &*f.owner)
        .unwrap();
    view
}

/// C5 (review 5, MEDIUM 5), at the broker: a token captured before its
/// final check is reached (1) through a replacement: a twin door of the
/// same agent finds the same cell by the same key and engages it; (3)
/// through X2″'s stop of a surviving run whose adoption D4(a) refused. In
/// each the held execution's final check refuses and nothing is published.
/// (A permitted adoption never carries a Files token: D4(a) refuses the
/// adoption of any run with a pinned Files obligation, from its proposal
/// on; the cell an adoption attaches is covered by the round-5 adoption
/// oracles.)
#[test]
fn m2d_c_r6_a_token_held_before_its_final_check_sees_the_cell_engaged_later() {
    // (1) Replacement: the same key names the same cell.
    let f = Fx::new();
    let cells = f.door.halt_cells();
    let original = cells.cell("journal", 1).unwrap();
    let run = f.door.open_run_with_halt(&original).unwrap();
    f.door.admit_files(run, HASH, LABEL).unwrap();
    let view = authorized(&f, &f.door, run, "replaced.txt");
    let (gate, worker) = held_on(&f, run, &view, at::FILE_BEFORE_FINAL);
    let twin = f.gov.issue_agent_door(mother(), f.owner.clone()).unwrap();
    let found = twin.halt_cells().cell("journal", 1).unwrap();
    assert!(found.same(&original), "the original cell, by its key");
    found.engage_with(HaltCause::Stop);
    gate.open();
    assert!(!worker.join().unwrap(), "(1) refused at the final check");
    assert!(!f.ws.join("replaced.txt").exists());

    // (3) The adoption refused (authorized, D4(a)): the replacement's cell
    // never reaches the run; X2″ stops the run itself.
    let f = Fx::new();
    let original = f.door.halt_cells().cell("journal", 3).unwrap();
    let run = f.door.open_run_with_halt(&original).unwrap();
    f.door.admit_files(run, HASH, LABEL).unwrap();
    let view = authorized(&f, &f.door, run, "surviving.txt");
    let (gate, worker) = held_on(&f, run, &view, at::FILE_BEFORE_FINAL);
    let twin = f.gov.issue_agent_door(mother(), f.owner.clone()).unwrap();
    let fresh = twin.halt_cells().cell("copied", 3).unwrap();
    assert_eq!(class(twin.adopt_with_halt(run, view.id, &fresh)), "halted");
    assert_eq!(twin.run_carries(run, &fresh), Ok(Some(false)));
    let _ = twin.stop_surviving_run(run).unwrap().write();
    gate.open();
    assert!(!worker.join().unwrap(), "(3) refused at the final check");
    assert!(!f.ws.join("surviving.txt").exists());
    assert_eq!(original.cause(), Some(HaltCause::Fence));
}

/// The body of `fn name` in `source` (to its closing brace at the
/// item's indentation).
fn body<'s>(source: &'s str, name: &str) -> &'s str {
    let start = source
        .find(&format!("fn {name}("))
        .or_else(|| source.find(&format!("fn {name}<")))
        .unwrap_or_else(|| panic!("no fn {name}"));
    let rest = &source[start..];
    let indent = source[..start]
        .rsplit('\n')
        .next()
        .map_or(0, |line| line.len() - line.trim_start().len());
    let close = format!("\n{}}}", " ".repeat(indent));
    &rest[..rest.find(&close).unwrap_or(rest.len())]
}

/// R6-1's source guard: every run admission goes through the one shared
/// check, directly or through `live_member` / `checked`.
#[test]
fn m2d_c_r6_no_run_admission_bypasses_the_shared_check() {
    let source = include_str!("../authority/run.rs");
    for name in ["checked", "live_member", "join_with"] {
        assert!(
            body(source, name).contains("run_admissible("),
            "{name} bypasses the shared check"
        );
    }
    for name in ["admissible", "admit_model_call"] {
        assert!(
            body(source, name).contains("live_member("),
            "{name} bypasses the shared check"
        );
    }
    for name in ["admit_files", "admit_inference"] {
        assert!(
            body(source, name).contains("self.admissible("),
            "{name} bypasses the shared check"
        );
    }
    for name in ["check", "check_retaining"] {
        assert!(
            body(source, name).contains("Self::checked("),
            "{name} bypasses the shared check"
        );
    }
    assert!(
        body(source, "open_in_with").contains("is_engaged"),
        "an opening with an engaged cell"
    );
}
