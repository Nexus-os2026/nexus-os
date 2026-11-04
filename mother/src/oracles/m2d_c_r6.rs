//! M2d-C round 6 (R6-1, R6-3 with X3, R6-4, X2″), against the recording
//! port, pre-registered before the repair. The real-broker twins are in
//! `adapter/m2d_files_r6.rs`.
//!
//! - X3: a mission whose cell is engaged (here through the port's own
//!   table, as the broker's emergency ingress engages it) is non-runnable
//!   but never reported durably terminal until the first recording-capable
//!   touch records its terminal for the cell's first cause; repeated
//!   touches are idempotent; pending recordings are drained on a healthy
//!   open and before a clean close; a settling mission's replayed cause is
//!   the cell's.
//! - R6-3: the terminal is the cause's: the Owner's Stop of a mission
//!   whose cell the time budget engaged ends `budget_time`.
//! - R6-4: `stop_all` is typed on poison, never `Ok(0)`; the source guard
//!   over the Stop-path functions and their helpers.
//! - X2″: production refuses an identity-less store (the fallback exists
//!   under `cfg(test)` only): a source guard.

use super::{contract_with, world, World};
use crate::coordinator::CoordinatorError;
use crate::effect::EffectPort;
use crate::lane::Lane;
use crate::journal::{Event, MissionId};
use crate::state::{MissionState, Trigger};
use nexus_governed_control::authority::halt_cell::HaltCause;

fn engage(w: &World, id: MissionId, cause: HaltCause) {
    w.port
        .halt_cells()
        .get(w.mother.journal_identity(), id.0)
        .expect("the mission's cell")
        .engage_with(cause);
}

fn cause(w: &World, id: MissionId) -> Option<HaltCause> {
    w.port
        .halt_cells()
        .get(w.mother.journal_identity(), id.0)
        .and_then(|c| c.cause())
}

fn stops(w: &World, id: MissionId) -> Vec<String> {
    w.mother
        .journal()
        .unwrap()
        .into_iter()
        .filter(|e| e.mission == id)
        .filter_map(|e| match e.event {
            Event::Transition {
                trigger: Trigger::Stop { reason },
            } => Some(reason.code().to_string()),
            Event::Transition {
                trigger: Trigger::Block { reason },
            } => Some(format!("block:{}", reason.as_str())),
            _ => None,
        })
        .collect()
}

/// X3 / R6-1: an engaged, unrecorded mission is non-runnable but not
/// terminal; its first touch records the cause's terminal (the emergency's
/// Stop) and nothing else; touched again, the one record stands.
#[test]
fn m2d_c_r6_the_first_touch_records_the_cell_s_cause_once() {
    let w = world();
    let id = w.mission_in(MissionState::Building);
    let calls = w.keyed_calls();
    engage(&w, id, HaltCause::Emergency);
    assert_eq!(w.mother.state(id), Some(MissionState::Building), "not terminal");
    assert_eq!(stops(&w, id), Vec::<String>::new(), "not recorded yet");
    assert_eq!(w.mother.advance(id), Err(CoordinatorError::Stopped));
    assert_eq!(w.mother.state(id), Some(MissionState::Stopped));
    assert_eq!(stops(&w, id), vec!["emergency_stop".to_string()]);
    assert_eq!(w.keyed_calls(), calls, "no lane ran");
    assert_eq!(w.mother.advance(id), Err(CoordinatorError::Stopped));
    assert_eq!(stops(&w, id), vec!["emergency_stop".to_string()], "once");
    // Every other entry point is a touch too: consent of a waiting draft.
    let drafted = w.mission_in(MissionState::AwaitingConsent);
    engage(&w, drafted, HaltCause::Emergency);
    assert_eq!(
        crate::adapter::m2c_support::consent(&w.mother, drafted, &super::Yes),
        Err(CoordinatorError::Stopped)
    );
    assert_eq!(stops(&w, drafted), vec!["emergency_stop".to_string()]);
}

/// X3: pending recordings are drained before a clean close and on a
/// healthy open: an engaged mission never crosses a session unrecorded.
#[test]
fn m2d_c_r6_pending_recordings_are_drained_at_close_and_open() {
    let w = world();
    let closed = w.mission_in(MissionState::Approved);
    engage(&w, closed, HaltCause::Stop);
    let w = w.restart();
    let entries = w.mother.journal().unwrap();
    let stop = entries
        .iter()
        .position(|e| {
            e.mission == closed
                && matches!(
                    e.event,
                    Event::Transition {
                        trigger: Trigger::Stop { .. }
                    }
                )
        })
        .expect("recorded");
    let close = entries
        .iter()
        .position(|e| matches!(e.event, Event::SessionClosed))
        .expect("closed cleanly");
    assert!(stop < close, "drained before the clean close");
    assert_eq!(w.mother.state(closed), Some(MissionState::Stopped));

    // On open: the cell engaged, the journal reopened without a close.
    let opened = w.mission_in(MissionState::Investigating);
    engage(&w, opened, HaltCause::TimeBudget);
    let again = super::world_with(w.lanes.clone());
    drop(again);
    let reopened = crate::coordinator::Coordinator::open(
        w.journal.clone(),
        w.port.clone(),
        crate::coordinator::Lanes::new(w.lanes.clone()).unwrap(),
        w.clock.clone(),
        super::test_private(),
    )
    .unwrap();
    assert_eq!(reopened.held(opened), None, "recorded, never held");
    assert_eq!(reopened.state(opened), Some(MissionState::Blocked));
    assert_eq!(reopened.reason(opened).as_deref(), Some("budget_time"));
}

/// R6-3: the terminal is the cell's first cause's: the Owner's Stop of a
/// mission whose cell the time budget engaged ends `budget_time`, and the
/// cell keeps that cause.
#[test]
fn m2d_c_r6_the_terminal_is_the_first_cause_s() {
    let w = world();
    let id = w.mission_in(MissionState::Building);
    engage(&w, id, HaltCause::TimeBudget);
    assert_eq!(w.mother.stop(id), Ok(MissionState::Blocked));
    assert_eq!(w.mother.reason(id).as_deref(), Some("budget_time"));
    assert_eq!(stops(&w, id), vec!["block:budget_time".to_string()]);
    assert_eq!(cause(&w, id), None, "retired once terminal");
}

/// A mission whose Builder asked for two Files; the first waits.
fn waiting(w: &World) -> MissionId {
    use crate::adapter::p0_support::action;
    use crate::effect::EffectOutcome;
    let id = w.approved(contract_with("ASK-BUILDER-FILE ASK-BUILDER-FILE"));
    w.mother.advance(id).unwrap();
    w.mother.advance(id).unwrap();
    w.port.set_pending(Some(action(1)));
    w.port.reply(Ok(EffectOutcome::AwaitingApproval));
    w.mother.advance(id).unwrap();
    assert_eq!(w.mother.state(id), Some(MissionState::AwaitingApproval));
    id
}

/// X3: a settling mission's replayed cause is its cell's: reopened with a
/// fresh table (a port of its own) from a journal ending in its
/// `SettlementStarted`, the cell is engaged with the journaled first cause
/// before anything is exposed (seen when the open then fails: the cell
/// stays), and a healthy open settles it uncertain and retires the cell.
#[test]
fn m2d_c_r6_a_replayed_settlement_engages_its_cell_with_its_cause() {
    use crate::journal::{JournalStore, MemoryJournal};
    let w = world();
    let id = waiting(&w);
    w.port.hold_publication();
    let mother = std::sync::Arc::clone(&w.mother);
    let stopping = std::thread::spawn(move || mother.stop(id));
    assert!(w.port.wait_publication_entered());
    // SettlementStarted is durable, the terminal held: the same lines in
    // journals of their own.
    let lines = w.journal.lines();
    w.port.release_publication();
    let _ = stopping.join().unwrap();
    let open = |journal: MemoryJournal| {
        let fresh = std::sync::Arc::new(super::RecordingPort::default());
        let opened = crate::coordinator::Coordinator::open(
            journal.clone(),
            fresh.clone(),
            crate::coordinator::Lanes::new(w.lanes.clone()).unwrap(),
            w.clock.clone(),
            super::test_private(),
        );
        (fresh, opened)
    };
    let failing = MemoryJournal::new();
    failing.set_lines(lines.clone());
    failing.fail_after(0);
    let (fresh, opened) = open(failing.clone());
    assert!(opened.is_err(), "the open's first append fails");
    let identity = failing.identity().unwrap();
    assert_eq!(
        fresh.halt_cells().get(&identity, id.0).and_then(|c| c.cause()),
        Some(HaltCause::Stop),
        "engaged with the journaled first cause before anything was exposed"
    );
    let healthy = MemoryJournal::new();
    healthy.set_lines(lines);
    let (fresh, opened) = open(healthy.clone());
    let again = opened.unwrap();
    assert_eq!(again.state(id), Some(MissionState::Blocked));
    assert_eq!(again.reason(id).as_deref(), Some("file_publication_uncertain"));
    assert!(
        fresh.halt_cells().get(&healthy.identity().unwrap(), id.0).is_none(),
        "retired once terminal"
    );
}

/// R6-4: `stop_all` on a poisoned coordinator is `Err(Poisoned)`, never
/// `Ok(0)`, with no panic; healthy, it is `Ok(n)`.
#[test]
fn m2d_c_r6_stop_all_is_typed_on_poison() {
    let w = world();
    let _live = w.mission_in(MissionState::Approved);
    let other = w.mission_in(MissionState::Draft);
    assert_eq!(w.mother.stop_all(), Ok(2));
    assert_eq!(w.mother.stop_all(), Ok(0));
    assert_eq!(w.mother.state(other), Some(MissionState::Stopped));
    let w = world();
    let id = w.mission_in(MissionState::Approved);
    w.mother.poison_for_test();
    let answer = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| w.mother.stop_all()));
    assert_eq!(answer.expect("never a panic"), Err(CoordinatorError::Poisoned));
    assert_eq!(cause(&w, id), Some(HaltCause::Stop), "engaged all the same");
}

/// A lane that runs a hook once, inside its call, before it answers.
struct Hooked {
    inner: std::sync::Arc<dyn crate::lane::Lane>,
    hook: std::sync::Arc<std::sync::Mutex<Option<Box<dyn FnOnce() + Send>>>>,
}

impl crate::lane::Lane for Hooked {
    fn identity(&self) -> crate::lane::LaneIdentity {
        self.inner.identity()
    }
    fn invoke(&self, input: &crate::lane::LaneInput) -> crate::lane::LaneOutput {
        if let Some(hook) = self.hook.lock().unwrap().take() {
            hook();
        }
        self.inner.invoke(input)
    }
}

/// R6-1 / X5: the entry check alone is not enough: a cell engaged while
/// the call is out of the lock (inside a lane's call) refuses the next
/// boundary: the lane's answer is not kept, no later lane or effect runs,
/// and the call records the cause's terminal.
#[test]
fn m2d_c_r6_a_cell_engaged_mid_call_refuses_the_next_boundary() {
    let hook: std::sync::Arc<std::sync::Mutex<Option<Box<dyn FnOnce() + Send>>>> =
        Default::default();
    let keyed: Vec<std::sync::Arc<super::KeyedLane>> = crate::lane::LaneRole::ALL
        .iter()
        .map(|role| std::sync::Arc::new(super::KeyedLane::new(*role)))
        .collect();
    let lanes: Vec<std::sync::Arc<dyn crate::lane::Lane>> = keyed
        .iter()
        .map(|lane| {
            if Lane::identity(&**lane).role == crate::lane::LaneRole::Sentinel {
                std::sync::Arc::new(Hooked {
                    inner: lane.clone(),
                    hook: hook.clone(),
                }) as std::sync::Arc<dyn crate::lane::Lane>
            } else {
                lane.clone() as std::sync::Arc<dyn crate::lane::Lane>
            }
        })
        .collect();
    let mut w = super::world_with(lanes);
    w.keyed = keyed;
    let id = w.mission_in(MissionState::Investigating);
    let cell = w
        .port
        .halt_cells()
        .get(w.mother.journal_identity(), id.0)
        .unwrap();
    *hook.lock().unwrap() = Some(Box::new(move || cell.engage_with(HaltCause::Emergency)));
    let calls = w.keyed_calls();
    assert_eq!(w.mother.advance(id), Err(CoordinatorError::Stopped));
    assert_eq!(w.keyed_calls(), calls + 1, "the Sentinel alone ran");
    let events: Vec<String> = w
        .mother
        .journal()
        .unwrap()
        .into_iter()
        .filter(|e| e.mission == id)
        .map(|e| match e.event {
            Event::LaneInvoked { role, .. } => format!("invoked:{}", role.as_str()),
            Event::LaneReturned { .. } => "returned".into(),
            Event::Transition {
                trigger: Trigger::Stop { reason },
            } => format!("stop:{}", reason.code()),
            other => format!("{other:?}").split(' ').next().unwrap_or("").to_string(),
        })
        .collect();
    let invoked = events.iter().position(|e| e == "invoked:sentinel").unwrap();
    assert_eq!(
        &events[invoked..],
        &["invoked:sentinel".to_string(), "stop:emergency_stop".to_string()],
        "the answer is not kept; the terminal follows: {events:?}"
    );
    assert_eq!(w.mother.state(id), Some(MissionState::Stopped));
}

/// The body of `fn name` in `source` (to its closing brace at the item's
/// indentation).
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

/// R6-4's source guard: the Stop-path functions and, transitively, every
/// coordinator function they call take no panicking route: no
/// `with_lock(` (only `try_with_lock(`), no `.expect(`, no `.unwrap(`, no
/// `.lock().expect`.
#[test]
fn m2d_c_r6_no_stop_path_takes_a_panicking_lock() {
    let source = include_str!("../coordinator.rs");
    let roots = [
        "stop",
        "stop_for",
        "barrier",
        "stop_step",
        "after_stop",
        "reconcile",
        "settle_terminal",
        "settle_one",
        "end",
        "end_with",
        "end_plain_block",
        "end_plain",
        "file_unresolved",
        "close_action",
        "stop_all",
        "emergency",
        "record_for",
        "halted_advance",
        "drain_pending",
        "close_ref",
        "budget_spent",
        "engaged",
    ];
    let mut seen: Vec<String> = Vec::new();
    let mut todo: Vec<String> = roots.iter().map(|r| r.to_string()).collect();
    while let Some(name) = todo.pop() {
        if seen.contains(&name) {
            continue;
        }
        let text = body(source, &name);
        for forbidden in ["with_lock(", ".expect(", ".unwrap(", ".lock().expect"] {
            let hits = text
                .match_indices(forbidden)
                .filter(|(at, _)| {
                    forbidden != "with_lock("
                        || !text[..*at].ends_with("try_")
                })
                .count();
            assert_eq!(hits, 0, "{name} takes a panicking route: {forbidden}");
        }
        for call in text.split("self.").skip(1) {
            let callee: String = call
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            if !callee.is_empty()
                && call[callee.len()..].starts_with('(')
                && (source.contains(&format!("fn {callee}("))
                    || source.contains(&format!("fn {callee}<")))
            {
                todo.push(callee);
            }
        }
        seen.push(name);
    }
    assert!(seen.len() > roots.len(), "the closure follows the helpers");
}

/// X2″'s source guard: the identity-less fallback exists under `cfg(test)`
/// only; a non-test build refuses a store with no identity, typed.
#[test]
fn m2d_c_r6_production_refuses_an_identity_less_store() {
    let source = include_str!("../coordinator.rs");
    let open = body(source, "open");
    let fallback = open
        .find("fresh_identity(\"anonymous\")")
        .expect("the test fallback");
    let before = &open[..fallback];
    let guarded = before.rfind("#[cfg(test)]").expect("a cfg(test) guard");
    let refused = before.rfind("#[cfg(not(test))]").expect("the production arm");
    assert!(
        open[refused..].contains("IdentityRefusal::NoIdentity"),
        "the production arm refuses, typed"
    );
    assert!(
        !open[guarded..fallback].contains("#[cfg(not(test))]"),
        "the fallback is under the cfg(test) arm"
    );
    assert_eq!(
        source.matches("fresh_identity(\"anonymous\")").count(),
        1,
        "one fallback"
    );
}
