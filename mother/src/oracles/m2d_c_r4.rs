//! M2d-C round 4: oracles for the Architect's settlement design v2 and its
//! binding addendum (`m2d-c-settlement-design.md`), pre-registered before
//! the repair, against the recording port; deterministic (a held port call
//! or a journal prefix, never a sleep). Their real-broker twins are in
//! `adapter/m2d_files_r4.rs`; the broker's own (publication class, pins,
//! the memory stop under poison) are in the broker crate.
//!
//! (1) an unreadable broker still ends every mission; (2) an executing File
//! at a Stop is uncertain, stays pinned and never reopens the mission; (3)
//! a failed acknowledgement is retried, a conclusive one only; (4) every
//! mission is stopped before any snapshot; (5) crashes at every durable
//! boundary (journal prefixes), one indivisible terminal line, and replay
//! refusals; (6) the ordinary path's dispositions; (7) controls; and the
//! addendum's schedules: the memory stop precedes the journal and the owed
//! write, a recording error does not prevent it, and its scope is the port.

use super::{contract_with, test_private, world, RecordingPort, World};
use crate::adapter::p0_support::action;
use crate::coordinator::{Coordinator, CoordinatorError, Lanes};
use crate::effect::{ApprovalStatus, EffectOutcome, PendingAction, Publication};
use crate::evidence::{
    BlockReason, SettleCause, SettlementEnd, SettlementOutcome, StopReason, FILE_UNCERTAIN_NOTICE,
};
use crate::journal::{
    Entry, Event, JournalError, JournalStore, MemoryJournal, MissionId, ReplayViolation,
};
use crate::state::{MissionState, Trigger};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// A mission whose Builder asked for two Files; the first waits.
fn waiting(w: &World) -> MissionId {
    let id = w.approved(contract_with("ASK-BUILDER-FILE ASK-BUILDER-FILE"));
    w.mother.advance(id).unwrap();
    w.mother.advance(id).unwrap();
    w.port.set_pending(Some(action(1)));
    w.port.reply(Ok(EffectOutcome::AwaitingApproval));
    w.mother.advance(id).unwrap();
    assert_eq!(w.mother.state(id), Some(MissionState::AwaitingApproval));
    id
}

fn events_in(journal: &MemoryJournal, id: MissionId) -> Vec<Event> {
    crate::journal::verify(&journal.lines())
        .unwrap()
        .into_iter()
        .filter(|e| e.mission == id)
        .map(|e| e.event)
        .collect()
}

fn events(w: &World, id: MissionId) -> Vec<Event> {
    events_in(&w.journal, id)
}

/// The mission's settlement records: its start (if any) and its terminal
/// line (outcome, end, action).
type Terminal = (SettlementOutcome, SettlementEnd, Option<PendingAction>);

fn started(events: &[Event]) -> Vec<SettleCause> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::SettlementStarted { cause, .. } => Some(*cause),
            _ => None,
        })
        .collect()
}

fn terminal(events: &[Event]) -> Vec<Terminal> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::Transition {
                trigger:
                    Trigger::Settled {
                        outcome,
                        end,
                        action,
                        ..
                    },
            } => Some((outcome.clone(), *end, action.clone())),
            _ => None,
        })
        .collect()
}

fn published() -> Publication {
    Publication::Published {
        recovery_required: false,
        interrupted: vec![],
    }
}

/// A coordinator opened after a crash on `lines` (every in-memory state
/// lost, the journal kept), with a fresh port.
fn crashed(
    w: &World,
    lines: Vec<String>,
) -> (
    Result<Coordinator<RecordingPort>, CoordinatorError>,
    Arc<RecordingPort>,
    MemoryJournal,
) {
    let journal = MemoryJournal::new();
    journal.set_lines(lines);
    let port = Arc::new(RecordingPort::default());
    let opened = Coordinator::open(
        journal.clone(),
        port.clone(),
        Lanes::new(w.lanes.clone()).unwrap(),
        w.clock.clone(),
        test_private(),
    );
    (opened, port, journal)
}

/// The journal's lines up to and including the first line whose event
/// `pick` selects.
fn prefix_through(w: &World, pick: impl Fn(&Event) -> bool) -> Vec<String> {
    let lines = w.journal.lines();
    let entries = crate::journal::verify(&lines).unwrap();
    let at = entries
        .iter()
        .position(|e| pick(&e.event))
        .expect("the boundary");
    lines[..=at].to_vec()
}

fn is_start(e: &Event) -> bool {
    matches!(e, Event::SettlementStarted { .. })
}

fn is_terminal(e: &Event) -> bool {
    matches!(
        e,
        Event::Transition {
            trigger: Trigger::Settled { .. }
        }
    )
}

/// (1) Rule 3/5: a broker that cannot be read: the File's mission still
/// ends (uncertain, the Owner told, nothing acknowledged), and Stop
/// everything completes for another live mission too.
#[test]
fn m2d_c_r4_1_an_unreadable_broker_ends_every_mission() {
    let w = world();
    let id = waiting(&w);
    let other = w.approved(contract_with(""));
    w.port.fail_publication();
    assert_eq!(w.mother.stop_all(), 2);
    assert_eq!(w.mother.state(id), Some(MissionState::Blocked));
    assert_eq!(
        w.mother.reason(id).as_deref(),
        Some("file_publication_uncertain")
    );
    assert_eq!(w.mother.notice(id), Some(FILE_UNCERTAIN_NOTICE));
    assert_eq!(w.mother.state(other), Some(MissionState::Stopped));
    assert!(w.port.is_cancelled(id), "withdrawn before the read");
    assert!(
        w.port.acked().is_empty(),
        "an uncertain outcome is kept pinned"
    );
}

/// (2) Rule 5: an executing File at a Stop is pending: uncertain, Blocked,
/// never acknowledged; an answer that comes later (the publication
/// finished) never reopens the mission, and a reopen does not acknowledge
/// it either.
#[test]
fn m2d_c_r4_2_an_executing_file_at_a_stop_is_uncertain_and_never_reopens() {
    let w = world();
    let id = waiting(&w);
    let calls = w.keyed_calls();
    w.port.set_publication(Some(Publication::Pending));
    assert_eq!(w.mother.stop(id), Ok(MissionState::Blocked));
    let events = events(&w, id);
    assert_eq!(started(&events), vec![SettleCause::Stop(StopReason::Owner)]);
    assert_eq!(
        terminal(&events),
        vec![(
            SettlementOutcome::Uncertain,
            SettlementEnd::Block(BlockReason::FilePublicationUncertain),
            Some(action(1)),
        )]
    );
    assert_eq!(w.mother.notice(id), Some(FILE_UNCERTAIN_NOTICE));
    assert!(w.port.acked().is_empty());
    w.port.approval_reply(ApprovalStatus::Approved);
    assert!(w.mother.advance(id).is_err());
    assert_eq!(w.mother.state(id), Some(MissionState::Blocked));
    assert_eq!(w.port.effects().len(), 1, "File B never sent");
    assert_eq!(w.keyed_calls(), calls, "no later lane");
    let again = w.restart();
    assert_eq!(again.mother.state(id), Some(MissionState::Blocked));
    assert!(w.port.acked().is_empty(), "never acknowledged on reopen");
}

/// (3) Rule 2 / addendum 4: an acknowledgement that fails is retried on
/// the next reopen from the journal (conclusive outcomes only), and the
/// ordinary approval acknowledges its File too.
#[test]
fn m2d_c_r4_3_a_failed_ack_is_retried_from_the_journal() {
    let w = world();
    let id = waiting(&w);
    w.port.set_publication(Some(published()));
    w.port.fail_acks(true);
    assert_eq!(w.mother.stop(id), Ok(MissionState::Blocked));
    assert!(w.port.acked().is_empty());
    w.port.fail_acks(false);
    let _again = w.restart();
    assert_eq!(w.port.acked(), vec![action(1)], "re-acknowledged");

    let w = world();
    let id = waiting(&w);
    w.port.approval_reply(ApprovalStatus::Approved);
    assert_eq!(w.mother.advance(id).unwrap().to, MissionState::Verifying);
    assert_eq!(w.port.acked(), vec![action(1)]);
    assert_eq!(w.port.effects().len(), 2, "File B goes on");
}

/// (4) Rule 4: every mission's first Stop step comes before any snapshot:
/// while the File's snapshot is held, the other mission is stopped already.
#[test]
fn m2d_c_r4_4_every_mission_is_stopped_before_any_snapshot() {
    let w = world();
    let id = waiting(&w);
    let other = w.approved(contract_with(""));
    w.port.set_publication(Some(published()));
    w.port.hold_publication();
    let mother = Arc::clone(&w.mother);
    let stopping = std::thread::spawn(move || mother.stop_all());
    assert!(w.port.wait_publication_entered());
    assert_eq!(w.mother.state(other), Some(MissionState::Stopped));
    w.port.release_publication();
    assert_eq!(stopping.join().unwrap(), 2);
    assert_eq!(
        w.mother.reason(id).as_deref(),
        Some("file_publication_interrupted:owner_stop")
    );
}

/// A journal that notes how often the port's memory stop was engaged at
/// each append.
struct Noting {
    inner: MemoryJournal,
    port: Arc<RecordingPort>,
    seen: Arc<Mutex<Vec<usize>>>,
}

impl JournalStore for Noting {
    fn load(&self) -> Result<Vec<String>, JournalError> {
        self.inner.load()
    }
    fn append(&mut self, line: &str) -> Result<(), JournalError> {
        self.seen.lock().unwrap().push(self.port.halts());
        self.inner.append(line)
    }
    fn poison(&mut self) -> Result<(), JournalError> {
        self.inner.poison()
    }
}

/// Addendum 1: the memory stop is engaged before the Stop's first journal
/// append and before its owed-evidence write; a recording error does not
/// prevent it; and its scope is the port: another mission is stopped with
/// it (`door_halted`), never left runnable. Control: without a File live,
/// the scope is the mission's own.
#[test]
fn m2d_c_r4_5_the_memory_stop_comes_first_and_its_scope_is_the_port() {
    let w = world();
    let port = Arc::new(RecordingPort::default());
    let seen: Arc<Mutex<Vec<usize>>> = Arc::default();
    let journal = MemoryJournal::new();
    let mother = Arc::new(
        Coordinator::open(
            Noting {
                inner: journal.clone(),
                port: port.clone(),
                seen: seen.clone(),
            },
            port.clone(),
            Lanes::new(w.lanes.clone()).unwrap(),
            w.clock.clone(),
            test_private(),
        )
        .unwrap(),
    );
    let w = World {
        port: port.clone(),
        journal,
        clock: w.clock.clone(),
        lanes: w.lanes.clone(),
        keyed: w.keyed.clone(),
        mother,
    };
    let id = waiting(&w);
    let other = w.approved(contract_with(""));
    w.mother.advance(other).unwrap();
    port.set_publication(Some(published()));
    port.set_halt_scope(true);
    let at_owed = Arc::new(AtomicUsize::new(usize::MAX));
    let noted = at_owed.clone();
    let probe = port.clone();
    w.mother
        .hooks
        .before_owed_write
        .arm(0, move || noted.store(probe.halts(), Ordering::SeqCst));
    let appended_before = seen.lock().unwrap().len();
    assert_eq!(w.mother.stop(id), Ok(MissionState::Blocked));
    let at_appends = seen.lock().unwrap()[appended_before..].to_vec();
    assert!(
        !at_appends.is_empty() && at_appends.iter().all(|h| *h == 1),
        "{at_appends:?}"
    );
    assert_eq!(at_owed.load(Ordering::SeqCst), 1, "before the owed write");
    assert_eq!(w.mother.state(other), Some(MissionState::Stopped));
    assert!(events(&w, other).contains(&Event::Transition {
        trigger: Trigger::Stop {
            reason: StopReason::DoorHalted
        }
    }));
    assert!(w.mother.advance(other).is_err());

    // A recording error does not prevent it.
    let w = world();
    let id = waiting(&w);
    w.port.set_halt_scope(true);
    w.journal.fail_after(0);
    let _ = w.mother.stop(id);
    assert_eq!(w.port.halts(), 1);
    assert_ne!(w.mother.state(id), Some(MissionState::AwaitingApproval));

    // Control: no File live at the port, the other mission goes on.
    let w = world();
    let id = waiting(&w);
    let other = w.approved(contract_with(""));
    w.port
        .set_publication(Some(Publication::ConfirmedNotPublished));
    assert_eq!(w.mother.stop(id), Ok(MissionState::Stopped));
    assert_eq!(w.mother.state(other), Some(MissionState::Approved));
}

/// (5) Rule 6 / addendum 2: a crash at every durable boundary. After the
/// settlement's start: uncertain on reopen, never "not published", never
/// acknowledged. After the terminal line: as it was, and a conclusive one
/// is acknowledged again. Before any start (a wait, an unclean shutdown):
/// held, and the Owner's Stop goes through the settlement, never an
/// ordinary Stop. The terminal line is one line: torn, it is refused; a
/// failed append fences and poisons.
#[test]
fn m2d_c_r4_6_a_crash_at_every_durable_boundary() {
    let w = world();
    let id = waiting(&w);
    w.port.set_publication(Some(published()));
    assert_eq!(w.mother.stop(id), Ok(MissionState::Blocked));

    let (opened, port, journal) = crashed(&w, prefix_through(&w, is_start));
    let mother = opened.expect("reopens");
    assert_eq!(mother.state(id), Some(MissionState::Blocked));
    assert_eq!(
        mother.reason(id).as_deref(),
        Some("file_publication_uncertain")
    );
    let events = events_in(&journal, id);
    assert_eq!(terminal(&events).len(), 1);
    assert_eq!(terminal(&events)[0].0, SettlementOutcome::Uncertain);
    assert!(port.acked().is_empty());

    let (opened, port, _) = crashed(&w, prefix_through(&w, is_terminal));
    let mother = opened.expect("reopens");
    assert_eq!(
        mother.reason(id).as_deref(),
        Some("file_publication_interrupted:owner_stop")
    );
    assert_eq!(port.acked(), vec![action(1)], "re-acknowledged");

    // One line: outcome and end together; torn, the journal is refused.
    let mut lines = prefix_through(&w, is_terminal);
    let last = lines.pop().unwrap();
    assert!(last.contains("\"outcome\"") && last.contains("\"end\""));
    lines.push(last[..last.len() / 2].to_string());
    assert!(crashed(&w, lines).0.is_err(), "a torn terminal line");

    // Before any start: the wait, then an unclean shutdown.
    let w = world();
    let id = waiting(&w);
    let lines = w.journal.lines();
    let (opened, port, journal) = crashed(&w, lines);
    let mother = opened.expect("reopens");
    assert!(mother.held(id).is_some(), "held after the unclean shutdown");
    let stopped = mother.stop(id);
    assert_eq!(stopped, Ok(MissionState::Blocked), "never an ordinary Stop");
    let events = events_in(&journal, id);
    assert_eq!(started(&events), vec![SettleCause::Stop(StopReason::Owner)]);
    assert_eq!(terminal(&events)[0].0, SettlementOutcome::Uncertain);
    assert!(port.acked().is_empty());

    // A failed append of the start: fenced, poisoned, ended in memory.
    let w = world();
    let id = waiting(&w);
    w.port.set_publication(Some(published()));
    w.journal.fail_after(0);
    assert!(w.mother.stop(id).is_err());
    assert_ne!(w.mother.state(id), Some(MissionState::AwaitingApproval));
    assert!(w.journal.is_poisoned());
    assert!(
        w.port.acked().is_empty(),
        "nothing durable, nothing acknowledged"
    );
}

/// Re-chain `events` for `mission` after `lines` (a crafted journal).
fn chained(lines: &[String], extra: Vec<(MissionId, Event)>) -> Vec<String> {
    let entries = crate::journal::verify(lines).unwrap();
    let (mut seq, mut prev) = (entries.len() as u64, entries.last().unwrap().hash.clone());
    let mut out = lines.to_vec();
    for (mission, event) in extra {
        let entry = Entry::chained(seq, &prev, mission, event);
        prev = entry.hash.clone();
        seq += 1;
        out.push(entry.to_line());
    }
    out
}

fn refused(w: &World, lines: Vec<String>) -> Option<ReplayViolation> {
    match crashed(w, lines).0 {
        Err(CoordinatorError::Journal(JournalError::Inconsistent { code, .. })) => Some(code),
        _ => None,
    }
}

/// Addendum 2: replay validates the exact File and the first cause, and
/// refuses a duplicate or conflicting settlement record.
#[test]
fn m2d_c_r4_7_replay_refuses_conflicting_settlement_records() {
    let w = world();
    let id = waiting(&w);
    let base = w.journal.lines();
    let start = |cause| Event::SettlementStarted { effect: 0, cause };
    let owner = SettleCause::Stop(StopReason::Owner);
    let settled = |end| Event::Transition {
        trigger: Trigger::Settled {
            effect: 0,
            action: Some(action(1)),
            outcome: SettlementOutcome::Published {
                recovery_required: false,
                interrupted: vec![],
            },
            end,
        },
    };
    let ok_end = SettlementEnd::Block(BlockReason::FilePublicationOwnerStop);
    // The control: the record the live coordinator writes replays.
    assert!(crashed(
        &w,
        chained(&base, vec![(id, start(owner)), (id, settled(ok_end))])
    )
    .0
    .is_ok());
    assert_eq!(
        refused(
            &w,
            chained(&base, vec![(id, start(owner)), (id, start(owner))])
        ),
        Some(ReplayViolation::SettlementConflict),
        "a second start"
    );
    assert_eq!(
        refused(
            &w,
            chained(
                &base,
                vec![
                    (id, start(owner)),
                    (id, settled(SettlementEnd::Stop(StopReason::Owner)))
                ]
            )
        ),
        Some(ReplayViolation::SettlementConflict),
        "an end its first cause does not give"
    );
    assert_eq!(
        refused(
            &w,
            chained(
                &base,
                vec![(
                    id,
                    Event::SettlementStarted {
                        effect: 7,
                        cause: owner
                    }
                )]
            )
        ),
        Some(ReplayViolation::SettlementOutOfPlace),
        "another File"
    );
    let mut other = action(1);
    other.commitment = "cmt-00000000000000000000000000000009".into();
    let foreign = Event::Transition {
        trigger: Trigger::Settled {
            effect: 0,
            action: Some(other),
            outcome: SettlementOutcome::Published {
                recovery_required: false,
                interrupted: vec![],
            },
            end: ok_end,
        },
    };
    assert_eq!(
        refused(&w, chained(&base, vec![(id, start(owner)), (id, foreign)])),
        Some(ReplayViolation::SettlementConflict),
        "an action other than the journaled one"
    );
    assert_eq!(
        refused(
            &w,
            chained(
                &base,
                vec![
                    (id, start(owner)),
                    (
                        id,
                        Event::Transition {
                            trigger: Trigger::Stop {
                                reason: StopReason::Owner
                            }
                        }
                    )
                ]
            )
        ),
        Some(ReplayViolation::SettlementConflict),
        "an ordinary Stop after the start"
    );
}

/// (6) Addendum 3, the ordinary path: an uncertain, unreadable or unknown
/// outcome ends the mission uncertain (durable terminal line, no ack, no
/// later dispatch, the Owner told); a denial is taken as not published and
/// acknowledged; a clean approval goes on; a valid wait keeps waiting.
#[test]
fn m2d_c_r4_8_the_ordinary_path_dispositions() {
    for status in [
        ApprovalStatus::Uncertain,
        ApprovalStatus::Lapsed {
            class: "approval_outcome_unknown".into(),
        },
        ApprovalStatus::Lapsed {
            class: "port_fenced".into(),
        },
    ] {
        let w = world();
        let id = waiting(&w);
        let calls = w.keyed_calls();
        w.port.approval_reply(status.clone());
        assert_eq!(w.mother.advance(id).unwrap().to, MissionState::Blocked);
        let events = events(&w, id);
        assert!(started(&events).is_empty(), "{status:?}");
        assert_eq!(
            terminal(&events),
            vec![(
                SettlementOutcome::Uncertain,
                SettlementEnd::Block(BlockReason::FilePublicationUncertain),
                Some(action(1)),
            )],
            "{status:?}"
        );
        assert_eq!(w.mother.notice(id), Some(FILE_UNCERTAIN_NOTICE));
        assert!(w.port.acked().is_empty(), "{status:?}: kept pinned");
        assert_eq!(w.port.effects().len(), 1, "{status:?}");
        assert_eq!(w.keyed_calls(), calls, "{status:?}");
    }
    let w = world();
    let id = waiting(&w);
    w.port.approval_reply(ApprovalStatus::Denied);
    assert_eq!(w.mother.advance(id).unwrap().to, MissionState::Denied);
    assert_eq!(w.port.acked(), vec![action(1)]);

    let w = world();
    let id = waiting(&w);
    w.port.approval_reply(ApprovalStatus::Waiting);
    w.mother.advance(id).unwrap();
    assert_eq!(w.mother.state(id), Some(MissionState::AwaitingApproval));
    assert!(w.port.acked().is_empty());
}

/// (7) Controls: a mission with no File stops as before (no settlement
/// record); a File confirmed not published ends as its cause would have,
/// through its settlement, and is acknowledged; the time budget likewise.
#[test]
fn m2d_c_r4_9_controls() {
    let w = world();
    let id = w.approved(contract_with(""));
    w.mother.advance(id).unwrap();
    assert_eq!(w.mother.stop(id), Ok(MissionState::Stopped));
    let recorded = events(&w, id);
    assert!(started(&recorded).is_empty() && terminal(&recorded).is_empty());

    let w = world();
    let id = waiting(&w);
    w.port
        .set_publication(Some(Publication::ConfirmedNotPublished));
    assert_eq!(w.mother.stop(id), Ok(MissionState::Stopped));
    assert_eq!(
        terminal(&events(&w, id)),
        vec![(
            SettlementOutcome::NotPublished,
            SettlementEnd::Stop(StopReason::Owner),
            Some(action(1)),
        )]
    );
    assert_eq!(w.port.acked(), vec![action(1)]);

    let w = world();
    let id = waiting(&w);
    w.port
        .set_publication(Some(Publication::ConfirmedNotPublished));
    w.clock.advance(Duration::from_millis(3_600_001));
    assert_eq!(w.mother.advance(id).unwrap().to, MissionState::Blocked);
    assert_eq!(w.mother.reason(id).as_deref(), Some("budget_time"));
    assert_eq!(started(&events(&w, id)), vec![SettleCause::TimeBudget]);
}

/// Addendum 5: the Owner's notice of an uncertain File, exactly.
#[test]
fn m2d_c_r4_10_the_owner_is_told_exactly() {
    let w = world();
    let id = waiting(&w);
    w.port.set_publication(Some(Publication::Uncertain));
    assert_eq!(w.mother.stop(id), Ok(MissionState::Blocked));
    assert_eq!(
        w.mother.notice(id),
        Some(
            "A file may exist in your folder. A write already handed off may still finish. \
             Mother will not retry or continue this mission."
        )
    );
    let other = w.approved(contract_with(""));
    assert_eq!(w.mother.notice(other), None);
}
