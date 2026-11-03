//! TD-O12: the slot state machine (§3.2a; R2-1 and R3-1), one machine for
//! the Sentinel, Verifier and notes slots. Clear, and a notes `AwaitingAck`,
//! are installed only by the one token, with its epoch current at both
//! settle steps and the binding current; after any repeat or restriction
//! nothing is installed again; a terminal restriction always installs and
//! `Spent` is absorbing. The acknowledgement cases of R3-1: a late
//! `NeedsOwner` after a repeat, Stop or other restriction opens no
//! acknowledgement whatever epoch it cites; record success and failure for
//! the `NeedsOwner` and for the acknowledgement; restrictions during the
//! acknowledgement's recording and before consumption; single-use
//! consumption; a wrong epoch or changed binding refused at every step;
//! and the control. Then every interleaving of two assessments, explored
//! exhaustively.

use std::collections::{BTreeSet, HashSet, VecDeque};

use super::support_b::{B0, B1};
use crate::{
    AckToken, Admission, AssessmentToken, Begin, Failure, Installed, Outcome, Reason, Refused,
    Restricted, SettleToken, Slot, SlotKind, SlotState, SpentReason,
};
use SlotKind::{Notes, Sentinel, Verifier};

const KINDS: [SlotKind; 3] = [Sentinel, Verifier, Notes];
const NOTE: Reason = Reason::NotesInstructsAi;
const STOP: Reason = Reason::Td(Failure::Stopped);
const OTHER: Reason = Reason::Td(Failure::Cancelled);
const EPOCHS: [u32; 6] = [0, 1, 2, 3, 4, u32::MAX];

fn spent(reason: SpentReason) -> SlotState {
    SlotState::Spent { reason }
}

fn started(slot: &mut Slot) -> AssessmentToken {
    match slot.begin(B0) {
        Begin::Started(token) => token,
        other => panic!("not started: {other:?}"),
    }
}

/// Up to `Settling`.
fn settling(slot: &mut Slot, outcome: Outcome) -> SettleToken {
    let token = started(slot);
    slot.settle_begin(token, outcome, &B0).expect("step 1")
}

/// The ordinary first `NeedsOwner`, up to `AwaitingAck`; returns the epoch
/// the coordinator displays.
fn awaiting(slot: &mut Slot) -> u32 {
    let token = started(slot);
    let step = slot
        .settle_begin(token, Outcome::NeedsOwner(NOTE), &B0)
        .expect("step 1");
    assert_eq!(slot.state(), SlotState::Settling);
    let installed = slot.settle_finish(step, true, &B0).expect("step 2");
    match slot.state() {
        SlotState::AwaitingAck {
            assessment_epoch,
            binding,
            reason,
        } => {
            assert_eq!((binding, reason), (B0, NOTE));
            assert_eq!(installed, Installed::AwaitingAck { assessment_epoch });
            assessment_epoch
        }
        other => panic!("not awaiting: {other:?}"),
    }
}

/// Up to `Acknowledged`.
fn acknowledged(slot: &mut Slot) {
    let shown = awaiting(slot);
    let ack = slot.ack_begin(shown, &B0).expect("ack step 1");
    slot.ack_finish(ack, true, &B0).expect("ack step 3");
    assert_eq!(
        slot.state(),
        SlotState::Acknowledged {
            assessment_epoch: 1,
            binding: B0
        }
    );
}

#[derive(Clone, Copy, Debug)]
enum Interference {
    Repeat,
    Stop,
    Other,
}

const INTERFERENCES: [Interference; 3] = [
    Interference::Repeat,
    Interference::Stop,
    Interference::Other,
];

fn interfere(slot: &mut Slot, i: Interference) -> SpentReason {
    match i {
        Interference::Repeat => {
            assert!(matches!(slot.begin(B0), Begin::Repeat), "a repeat");
            SpentReason::Repeat
        }
        Interference::Stop => {
            assert_eq!(slot.restrict(STOP), Restricted::Installed);
            SpentReason::Restricted(STOP)
        }
        Interference::Other => {
            assert_eq!(slot.restrict(OTHER), Restricted::Installed);
            SpentReason::Restricted(OTHER)
        }
    }
}

/// After a refusal: nothing is accepted, whatever epoch is cited.
fn nothing_accepted(slot: &mut Slot, expected: SlotState) {
    let epoch = slot.epoch();
    for e in EPOCHS {
        assert_eq!(
            slot.ack_begin(e, &B0).map(|_| ()),
            Err(Refused::AlreadySpent),
            "epoch {e}"
        );
    }
    assert_eq!(slot.admit(&B0), Err(Refused::AlreadySpent));
    assert!(
        matches!(slot.begin(B0), Begin::AlreadySpent),
        "refused without a new record"
    );
    assert_eq!(slot.restrict(OTHER), Restricted::AlreadySpent);
    assert_eq!((slot.state(), slot.epoch()), (expected, epoch));
}

#[test]
fn td_o12_control_a_first_needs_owner_is_acknowledged_and_consumed_once() {
    let mut slot = Slot::new(Notes);
    assert_eq!(
        (slot.state(), slot.epoch(), slot.kind()),
        (SlotState::Unassessed, 0, Notes)
    );
    let shown = awaiting(&mut slot);
    assert_eq!((shown, slot.epoch()), (1, 1));
    let ack = slot.ack_begin(shown, &B0).expect("accepted");
    assert_eq!(
        slot.state(),
        SlotState::AckRecording {
            assessment_epoch: 1,
            binding: B0
        }
    );
    slot.ack_finish(ack, true, &B0).expect("recorded");
    assert_eq!(
        slot.state(),
        SlotState::Acknowledged {
            assessment_epoch: 1,
            binding: B0
        }
    );
    assert_eq!(slot.admit(&B0), Ok(Admission::Acknowledged));
    assert_eq!((slot.state(), slot.epoch()), (SlotState::Consumed, 1));
    // Single use: a second consumption is refused, and the slot is spent.
    assert_eq!(
        slot.admit(&B0),
        Err(Refused::Spent(SpentReason::Invalidated))
    );
    nothing_accepted(&mut slot, spent(SpentReason::Invalidated));
}

#[test]
fn td_o12_control_clear_needs_the_token_twice() {
    for kind in KINDS {
        let mut slot = Slot::new(kind);
        let token = started(&mut slot);
        assert_eq!((slot.state(), slot.epoch()), (SlotState::Pending, 1));
        let step = slot
            .settle_begin(token, Outcome::Clear, &B0)
            .expect("step 1");
        assert_eq!(slot.state(), SlotState::Settling);
        assert_eq!(slot.settle_finish(step, true, &B0), Ok(Installed::Clear));
        assert_eq!(
            (slot.state(), slot.epoch()),
            (SlotState::Clear { binding: B0 }, 1)
        );
        // Prepare and execute both read it; reading does not consume it.
        assert_eq!(slot.admit(&B0), Ok(Admission::Clear));
        assert_eq!(slot.admit(&B0), Ok(Admission::Clear));
        // A changed binding at execute fails the recheck, for good.
        assert_eq!(slot.admit(&B1), Err(Refused::Spent(SpentReason::Stale)));
        nothing_accepted(&mut slot, spent(SpentReason::Stale));
    }
}

#[test]
fn td_o12_a_repeat_or_restriction_before_step_two_means_never_clear() {
    for kind in KINDS {
        let outcomes: &[Outcome] = if kind == Notes {
            &[Outcome::Clear, Outcome::NeedsOwner(NOTE)]
        } else {
            &[Outcome::Clear]
        };
        for outcome in outcomes {
            for i in INTERFERENCES {
                for between in [false, true] {
                    let mut slot = Slot::new(kind);
                    let token = started(&mut slot);
                    let reason = if between {
                        let step = slot.settle_begin(token, *outcome, &B0).expect("step 1");
                        let reason = interfere(&mut slot, i);
                        assert_eq!(
                            slot.settle_finish(step, true, &B0),
                            Err(Refused::AlreadySpent)
                        );
                        reason
                    } else {
                        let reason = interfere(&mut slot, i);
                        let late = slot.settle_begin(token, *outcome, &B0);
                        assert_eq!(late.map(|_| ()), Err(Refused::AlreadySpent));
                        reason
                    };
                    assert_eq!(
                        (slot.state(), slot.epoch()),
                        (spent(reason), 2),
                        "{kind:?} {outcome:?} {i:?} {between}"
                    );
                    // A late NeedsOwner opens no acknowledgement, for any epoch.
                    nothing_accepted(&mut slot, spent(reason));
                }
            }
        }
    }
}

#[test]
fn td_o12_a_failed_record_leaves_spent() {
    for kind in KINDS {
        let mut slot = Slot::new(kind);
        let step = settling(&mut slot, Outcome::Clear);
        assert_eq!(
            slot.settle_finish(step, false, &B0),
            Err(Refused::Spent(SpentReason::Unrecorded))
        );
        nothing_accepted(&mut slot, spent(SpentReason::Unrecorded));
    }
    // A NeedsOwner whose record fails: Spent, no acknowledgement.
    let mut slot = Slot::new(Notes);
    let step = settling(&mut slot, Outcome::NeedsOwner(NOTE));
    assert_eq!(
        slot.settle_finish(step, false, &B0),
        Err(Refused::Spent(SpentReason::Unrecorded))
    );
    assert_eq!(slot.epoch(), 2);
    nothing_accepted(&mut slot, spent(SpentReason::Unrecorded));
    // An acknowledgement whose record fails: Spent, nothing admitted.
    let mut slot = Slot::new(Notes);
    let shown = awaiting(&mut slot);
    let ack = slot.ack_begin(shown, &B0).expect("ack step 1");
    assert_eq!(
        slot.ack_finish(ack, false, &B0),
        Err(Refused::Spent(SpentReason::Unrecorded))
    );
    nothing_accepted(&mut slot, spent(SpentReason::Unrecorded));
}

#[test]
fn td_o12_restrictions_during_recording_and_before_consumption() {
    for i in INTERFERENCES {
        // During the acknowledgement's recording.
        let mut slot = Slot::new(Notes);
        let shown = awaiting(&mut slot);
        let ack = slot.ack_begin(shown, &B0).expect("ack step 1");
        let reason = interfere(&mut slot, i);
        assert_eq!(
            slot.ack_finish(ack, true, &B0),
            Err(Refused::AlreadySpent),
            "{i:?}"
        );
        nothing_accepted(&mut slot, spent(reason));
        // After Acknowledged, before consumption.
        let mut slot = Slot::new(Notes);
        acknowledged(&mut slot);
        let reason = interfere(&mut slot, i);
        assert_eq!(slot.admit(&B0), Err(Refused::AlreadySpent), "{i:?}");
        nothing_accepted(&mut slot, spent(reason));
        // While the NeedsOwner awaits the Owner.
        let mut slot = Slot::new(Notes);
        let shown = awaiting(&mut slot);
        let reason = interfere(&mut slot, i);
        assert_eq!(
            slot.ack_begin(shown, &B0).map(|_| ()),
            Err(Refused::AlreadySpent)
        );
        nothing_accepted(&mut slot, spent(reason));
    }
    // Racing acknowledgements cannot both succeed.
    let mut slot = Slot::new(Notes);
    let shown = awaiting(&mut slot);
    let first = slot.ack_begin(shown, &B0).expect("ack step 1");
    assert_eq!(
        slot.ack_begin(shown, &B0).map(|_| ()),
        Err(Refused::Spent(SpentReason::Invalidated))
    );
    assert_eq!(
        slot.ack_finish(first, true, &B0),
        Err(Refused::AlreadySpent)
    );
    nothing_accepted(&mut slot, spent(SpentReason::Invalidated));
}

#[test]
fn td_o12_a_wrong_epoch_or_changed_binding_is_refused_at_every_step() {
    let stale = Err(Refused::Spent(SpentReason::Stale));
    let invalid = Err(Refused::Spent(SpentReason::Invalidated));
    for kind in KINDS {
        // Settle step 1.
        let mut slot = Slot::new(kind);
        let token = started(&mut slot);
        assert_eq!(
            slot.settle_begin(token, Outcome::Clear, &B1).map(|_| ()),
            stale
        );
        nothing_accepted(&mut slot, spent(SpentReason::Stale));
        for e in EPOCHS.into_iter().filter(|e| *e != 1) {
            let mut slot = Slot::new(kind);
            let _real = started(&mut slot);
            let forged = AssessmentToken::forge(e, B0);
            assert_eq!(
                slot.settle_begin(forged, Outcome::Clear, &B0).map(|_| ()),
                invalid,
                "epoch {e}"
            );
            nothing_accepted(&mut slot, spent(SpentReason::Invalidated));
        }
        let mut slot = Slot::new(kind);
        let _real = started(&mut slot);
        let other_binding = AssessmentToken::forge(1, B1);
        assert_eq!(
            slot.settle_begin(other_binding, Outcome::Clear, &B0)
                .map(|_| ()),
            stale
        );
        // Settle step 2.
        let mut slot = Slot::new(kind);
        let step = settling(&mut slot, Outcome::Clear);
        assert_eq!(slot.settle_finish(step, true, &B1).map(|_| ()), stale);
        nothing_accepted(&mut slot, spent(SpentReason::Stale));
        for e in EPOCHS.into_iter().filter(|e| *e != 1) {
            let mut slot = Slot::new(kind);
            let _real = settling(&mut slot, Outcome::Clear);
            let forged = SettleToken::forge(e, B0, Outcome::Clear);
            assert_eq!(
                slot.settle_finish(forged, true, &B0).map(|_| ()),
                invalid,
                "epoch {e}"
            );
            nothing_accepted(&mut slot, spent(SpentReason::Invalidated));
        }
        // Admission of a Clear with a changed binding.
        let mut slot = Slot::new(kind);
        let step = settling(&mut slot, Outcome::Clear);
        slot.settle_finish(step, true, &B0).expect("step 2");
        assert_eq!(slot.admit(&B1).map(|_| ()), stale);
    }
    // Acknowledgement step 1: the displayed epoch, then the binding.
    for e in EPOCHS.into_iter().filter(|e| *e != 1) {
        let mut slot = Slot::new(Notes);
        awaiting(&mut slot);
        assert_eq!(slot.ack_begin(e, &B0).map(|_| ()), invalid, "displayed {e}");
        nothing_accepted(&mut slot, spent(SpentReason::Invalidated));
    }
    let mut slot = Slot::new(Notes);
    let shown = awaiting(&mut slot);
    assert_eq!(slot.ack_begin(shown, &B1).map(|_| ()), stale);
    nothing_accepted(&mut slot, spent(SpentReason::Stale));
    // Acknowledgement step 3.
    let mut slot = Slot::new(Notes);
    let shown = awaiting(&mut slot);
    let ack = slot.ack_begin(shown, &B0).expect("ack step 1");
    assert_eq!(slot.ack_finish(ack, true, &B1), stale);
    nothing_accepted(&mut slot, spent(SpentReason::Stale));
    for (e, b) in [(0, B0), (2, B0), (u32::MAX, B0), (1, B1)] {
        let mut slot = Slot::new(Notes);
        let shown = awaiting(&mut slot);
        let _real = slot.ack_begin(shown, &B0).expect("ack step 1");
        assert_eq!(
            slot.ack_finish(AckToken::forge(e, b), true, &B0),
            invalid,
            "({e}, {b:?})"
        );
        nothing_accepted(&mut slot, spent(SpentReason::Invalidated));
    }
    // Consumption: a changed binding, and an acknowledgement of another epoch.
    let mut slot = Slot::new(Notes);
    acknowledged(&mut slot);
    assert_eq!(slot.admit(&B1).map(|_| ()), stale);
    nothing_accepted(&mut slot, spent(SpentReason::Stale));
    let state = SlotState::Acknowledged {
        assessment_epoch: 1,
        binding: B0,
    };
    let mut slot = Slot::forged(Notes, state, 2);
    assert_eq!(slot.admit(&B0).map(|_| ()), invalid);
}

#[test]
fn td_o12_nothing_but_clear_or_an_acknowledgement_is_admitted() {
    for kind in KINDS {
        let mut slot = Slot::new(kind);
        assert_eq!(
            slot.admit(&B0),
            Err(Refused::Spent(SpentReason::Invalidated)),
            "unassessed: required means required"
        );
        let mut slot = Slot::new(kind);
        let _token = started(&mut slot);
        assert_eq!(
            slot.admit(&B0),
            Err(Refused::Spent(SpentReason::Invalidated)),
            "pending"
        );
        let mut slot = Slot::new(kind);
        let _step = settling(&mut slot, Outcome::Clear);
        assert_eq!(
            slot.admit(&B0),
            Err(Refused::Spent(SpentReason::Invalidated)),
            "settling"
        );
        nothing_accepted(&mut slot, spent(SpentReason::Invalidated));
    }
    // An unacknowledged NeedsOwner, or one being recorded, is not admitted.
    let mut slot = Slot::new(Notes);
    awaiting(&mut slot);
    assert_eq!(
        slot.admit(&B0),
        Err(Refused::Spent(SpentReason::Invalidated))
    );
    let mut slot = Slot::new(Notes);
    let shown = awaiting(&mut slot);
    let _ack = slot.ack_begin(shown, &B0).expect("ack step 1");
    assert_eq!(
        slot.admit(&B0),
        Err(Refused::Spent(SpentReason::Invalidated))
    );
    // A Sentinel or Verifier NeedsOwner is a terminal restriction: nothing
    // can be acknowledged from it.
    for kind in [Sentinel, Verifier] {
        let mut slot = Slot::new(kind);
        let r = Reason::FormatVariant;
        let token = started(&mut slot);
        let late = slot.settle_begin(token, Outcome::NeedsOwner(r), &B0);
        assert_eq!(
            late.map(|_| ()),
            Err(Refused::Spent(SpentReason::Restricted(r)))
        );
        nothing_accepted(&mut slot, spent(SpentReason::Restricted(r)));
        let mut slot = Slot::new(kind);
        let _token = started(&mut slot);
        assert_eq!(slot.restrict(r), Restricted::Installed);
        nothing_accepted(&mut slot, spent(SpentReason::Restricted(r)));
    }
}

#[test]
fn td_o12_a_terminal_restriction_always_installs_and_spent_is_absorbing() {
    // Every live state, reached through the API.
    let reach = |kind: SlotKind, n: usize| -> Option<Slot> {
        let mut slot = Slot::new(kind);
        if n == 0 {
            return Some(slot);
        }
        let notes = kind == Notes;
        let outcome = if n >= 4 {
            Outcome::NeedsOwner(NOTE)
        } else {
            Outcome::Clear
        };
        if n >= 4 && !notes {
            return None;
        }
        let token = started(&mut slot);
        if n == 1 {
            return Some(slot);
        }
        let step = slot.settle_begin(token, outcome, &B0).expect("step 1");
        if n == 2 {
            return Some(slot);
        }
        slot.settle_finish(step, true, &B0).expect("step 2");
        if n <= 4 {
            return Some(slot);
        }
        let ack = slot.ack_begin(1, &B0).expect("ack step 1");
        if n == 5 {
            return Some(slot);
        }
        slot.ack_finish(ack, true, &B0).expect("ack step 3");
        if n == 6 {
            return Some(slot);
        }
        assert_eq!(slot.admit(&B0), Ok(Admission::Acknowledged));
        Some(slot)
    };
    let mut reached = 0;
    for kind in KINDS {
        for n in 0..8 {
            let Some(mut slot) = reach(kind, n) else {
                continue;
            };
            reached += 1;
            let before = slot.epoch();
            assert_eq!(
                slot.restrict(STOP),
                Restricted::Installed,
                "{kind:?} {:?}",
                slot.state()
            );
            assert_eq!(
                (slot.state(), slot.epoch()),
                (spent(SpentReason::Restricted(STOP)), before + 1)
            );
        }
    }
    assert_eq!(reached, 3 * 4 + 4);
    // From every Spent, no operation changes anything, even with tokens
    // that carry the current epoch and binding.
    let reasons = [
        SpentReason::Repeat,
        SpentReason::Restricted(STOP),
        SpentReason::Restricted(NOTE),
        SpentReason::Invalidated,
        SpentReason::Stale,
        SpentReason::Unrecorded,
    ];
    for kind in KINDS {
        for reason in reasons {
            let state = spent(reason);
            let mut slot = Slot::forged(kind, state, 2);
            let refused = Err(Refused::AlreadySpent);
            assert!(matches!(slot.begin(B0), Begin::AlreadySpent));
            for outcome in [Outcome::Clear, Outcome::NeedsOwner(NOTE)] {
                assert_eq!(
                    slot.settle_begin(AssessmentToken::forge(2, B0), outcome, &B0)
                        .map(|_| ()),
                    refused
                );
                assert_eq!(
                    slot.settle_finish(SettleToken::forge(2, B0, outcome), true, &B0)
                        .map(|_| ()),
                    refused
                );
            }
            assert_eq!(slot.restrict(STOP), Restricted::AlreadySpent);
            for e in EPOCHS {
                assert_eq!(slot.ack_begin(e, &B0).map(|_| ()), refused);
                assert_eq!(slot.ack_finish(AckToken::forge(e, B0), true, &B0), refused);
            }
            assert_eq!(slot.admit(&B0).map(|_| ()), refused);
            assert_eq!(
                (slot.state(), slot.epoch()),
                (state, 2),
                "{kind:?} {reason:?}"
            );
        }
    }
}

#[test]
fn td_o12_tokens_show_nothing() {
    let mut slot = Slot::new(Notes);
    let token = started(&mut slot);
    assert_eq!(format!("{token:?}"), "AssessmentToken(..)");
    let step = slot
        .settle_begin(token, Outcome::NeedsOwner(NOTE), &B0)
        .expect("step 1");
    assert_eq!(format!("{step:#?}"), "SettleToken(..)");
    slot.settle_finish(step, true, &B0).expect("step 2");
    let ack = slot.ack_begin(1, &B0).expect("ack step 1");
    assert_eq!(format!("{ack:?}"), "AckToken(..)");
}

// ---- The exhaustive two-assessment exploration ----

/// What one assessment holds: nothing yet, its token, its settle token, or
/// nothing any more. Contents are the real tokens' (`parts`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Held {
    Idle,
    Token(u32, u8),
    Settling(u32, u8, bool),
    Done,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct World {
    state: SlotState,
    epoch: u32,
    held: [Held; 2],
    ack: Option<(u32, u8)>,
    live: u8,
    restricts: u8,
    changes: u8,
    ack_begins: u8,
    admits: u8,
    tainted: bool,
    consumed: u8,
}

#[derive(Clone, Copy, Debug)]
enum Ev {
    Begin(usize),
    Settle1(usize, bool),
    Finish(usize, bool),
    Restrict,
    Change,
    AckBegin(u32),
    AckFinish(bool),
    Admit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Res {
    Started,
    Repeat,
    BeginRefused,
    Ok,
    Installed(Installed),
    Admitted(Admission),
    RestrictInstalled,
    RestrictRefused,
    Err(Refused),
    Changed,
}

fn binding(live: u8) -> [u8; 32] {
    [B0, B1][usize::from(live)]
}

fn index(b: [u8; 32]) -> u8 {
    [B0, B1]
        .iter()
        .position(|x| *x == b)
        .expect("a binding the harness gave") as u8
}

fn outcome(needs_owner: bool) -> Outcome {
    if needs_owner {
        Outcome::NeedsOwner(NOTE)
    } else {
        Outcome::Clear
    }
}

fn events(w: &World) -> Vec<Ev> {
    let mut evs = Vec::new();
    for x in 0..2 {
        match w.held[x] {
            Held::Idle => evs.push(Ev::Begin(x)),
            Held::Token(..) => evs.extend([Ev::Settle1(x, false), Ev::Settle1(x, true)]),
            Held::Settling(..) => evs.extend([Ev::Finish(x, true), Ev::Finish(x, false)]),
            Held::Done => {}
        }
    }
    if w.restricts < 2 {
        evs.push(Ev::Restrict);
    }
    if w.changes < 1 {
        evs.push(Ev::Change);
    }
    if w.ack_begins < 2 {
        evs.extend((0..=3).map(Ev::AckBegin));
    }
    if w.ack.is_some() {
        evs.extend([Ev::AckFinish(true), Ev::AckFinish(false)]);
    }
    if w.admits < 3 {
        evs.push(Ev::Admit);
    }
    evs
}

fn step(kind: SlotKind, w: &World, ev: Ev) -> (World, Res) {
    let mut slot = Slot::forged(kind, w.state, w.epoch);
    let mut n = *w;
    let live = binding(w.live);
    let res = match ev {
        Ev::Begin(x) => match slot.begin(live) {
            Begin::Started(t) => {
                let (e, b) = t.parts();
                n.held[x] = Held::Token(e, index(b));
                Res::Started
            }
            Begin::Repeat => {
                n.held[x] = Held::Done;
                n.tainted = true;
                Res::Repeat
            }
            Begin::AlreadySpent => {
                n.held[x] = Held::Done;
                Res::BeginRefused
            }
        },
        Ev::Settle1(x, no) => {
            let Held::Token(e, b) = w.held[x] else {
                unreachable!()
            };
            match slot.settle_begin(AssessmentToken::forge(e, binding(b)), outcome(no), &live) {
                Ok(s) => {
                    let (e, b, o) = s.parts();
                    assert_eq!(o, outcome(no));
                    n.held[x] = Held::Settling(e, index(b), no);
                    Res::Ok
                }
                Err(r) => {
                    n.held[x] = Held::Done;
                    Res::Err(r)
                }
            }
        }
        Ev::Finish(x, recorded) => {
            let Held::Settling(e, b, no) = w.held[x] else {
                unreachable!()
            };
            n.held[x] = Held::Done;
            let t = SettleToken::forge(e, binding(b), outcome(no));
            match slot.settle_finish(t, recorded, &live) {
                Ok(i) => Res::Installed(i),
                Err(r) => Res::Err(r),
            }
        }
        Ev::Restrict => {
            n.restricts += 1;
            n.tainted = true;
            match slot.restrict(STOP) {
                Restricted::Installed => Res::RestrictInstalled,
                Restricted::AlreadySpent => Res::RestrictRefused,
            }
        }
        Ev::Change => {
            n.changes += 1;
            n.live = 1;
            Res::Changed
        }
        Ev::AckBegin(shown) => {
            n.ack_begins += 1;
            match slot.ack_begin(shown, &live) {
                Ok(a) => {
                    let (e, b) = a.parts();
                    n.ack = Some((e, index(b)));
                    Res::Ok
                }
                Err(r) => Res::Err(r),
            }
        }
        Ev::AckFinish(recorded) => {
            let (e, b) = n.ack.take().unwrap();
            match slot.ack_finish(AckToken::forge(e, binding(b)), recorded, &live) {
                Ok(()) => Res::Ok,
                Err(r) => Res::Err(r),
            }
        }
        Ev::Admit => {
            n.admits += 1;
            match slot.admit(&live) {
                Ok(a) => {
                    if a == Admission::Acknowledged {
                        n.consumed += 1;
                    }
                    Res::Admitted(a)
                }
                Err(r) => Res::Err(r),
            }
        }
    };
    n.state = slot.state();
    n.epoch = slot.epoch();
    (n, res)
}

fn check(kind: SlotKind, w: &World, ev: Ev, res: Res, n: &World) {
    use SlotState::*;
    let ctx = format!("{kind:?}: {w:?} --{ev:?}--> {res:?} {n:?}");
    let is_spent = |s: SlotState| matches!(s, Spent { .. });
    let live = binding(w.live);
    // Spent is absorbing: nothing changes and nothing succeeds.
    if is_spent(w.state) {
        assert_eq!((n.state, n.epoch), (w.state, w.epoch), "{ctx}");
        let refused = matches!(
            res,
            Res::BeginRefused
                | Res::RestrictRefused
                | Res::Err(Refused::AlreadySpent)
                | Res::Changed
        );
        assert!(refused, "{ctx}");
    }
    // After any repeat or restriction, the slot is Spent for good.
    if n.tainted {
        assert!(is_spent(n.state), "{ctx}");
    }
    match res {
        Res::Err(Refused::Spent(reason)) => {
            assert!(
                !is_spent(w.state) && n.state == Spent { reason } && n.epoch == w.epoch + 1,
                "{ctx}"
            );
        }
        Res::Err(Refused::AlreadySpent) | Res::BeginRefused | Res::RestrictRefused => {
            assert!(is_spent(w.state), "{ctx}");
        }
        Res::Repeat => assert!(
            n.state
                == Spent {
                    reason: SpentReason::Repeat
                }
                && n.epoch == w.epoch + 1,
            "{ctx}"
        ),
        Res::RestrictInstalled => {
            assert!(
                n.state
                    == Spent {
                        reason: SpentReason::Restricted(STOP)
                    }
                    && n.epoch == w.epoch + 1,
                "{ctx}"
            );
        }
        Res::Started => {
            let Ev::Begin(x) = ev else { unreachable!() };
            assert!(
                w.state == Unassessed && n.state == Pending && n.epoch == w.epoch + 1,
                "{ctx}"
            );
            assert_eq!(n.held[x], Held::Token(n.epoch, w.live), "{ctx}");
        }
        _ => {}
    }
    // Every change of state is one the design allows.
    let changed = w.state != n.state;
    if changed && !is_spent(n.state) {
        let allowed = match (w.state, n.state, ev) {
            (Unassessed, Pending, Ev::Begin(_)) => true,
            (Pending, Settling, Ev::Settle1(x, _)) => {
                matches!(w.held[x], Held::Token(e, b) if e == w.epoch && b == w.live)
            }
            (Settling, Clear { binding }, Ev::Finish(x, true)) => {
                binding == live
                    && matches!(w.held[x], Held::Settling(e, b, false) if e == w.epoch && b == w.live)
            }
            (
                Settling,
                AwaitingAck {
                    assessment_epoch,
                    binding,
                    reason,
                },
                Ev::Finish(x, true),
            ) => {
                kind == Notes
                    && (binding, reason) == (live, NOTE)
                    && assessment_epoch == w.epoch
                    && matches!(w.held[x], Held::Settling(e, b, true) if e == w.epoch && b == w.live)
            }
            (
                AwaitingAck {
                    assessment_epoch,
                    binding,
                    ..
                },
                AckRecording {
                    assessment_epoch: e2,
                    binding: b2,
                },
                Ev::AckBegin(shown),
            ) => {
                shown == assessment_epoch
                    && assessment_epoch == w.epoch
                    && binding == live
                    && (e2, b2) == (assessment_epoch, binding)
            }
            (
                AckRecording {
                    assessment_epoch,
                    binding,
                },
                Acknowledged {
                    assessment_epoch: e2,
                    binding: b2,
                },
                Ev::AckFinish(true),
            ) => {
                w.ack == Some((assessment_epoch, w.live))
                    && binding == live
                    && assessment_epoch == w.epoch
                    && (e2, b2) == (assessment_epoch, binding)
            }
            (
                Acknowledged {
                    assessment_epoch,
                    binding,
                },
                Consumed,
                Ev::Admit,
            ) => {
                assessment_epoch == w.epoch
                    && binding == live
                    && res == Res::Admitted(Admission::Acknowledged)
            }
            _ => false,
        };
        assert!(allowed, "{ctx}");
    }
    if !changed && !is_spent(w.state) {
        assert!(
            matches!(res, Res::Admitted(Admission::Clear) | Res::Changed),
            "{ctx}"
        );
    }
    // Admissions: a current Clear, or one acknowledgement, consumed in the same step.
    match res {
        Res::Admitted(Admission::Clear) => assert!(w.state == Clear { binding: live }, "{ctx}"),
        Res::Admitted(Admission::Acknowledged) => assert!(n.state == Consumed, "{ctx}"),
        _ => {}
    }
    assert!(n.consumed <= 1, "{ctx}");
    if kind != Notes {
        assert!(
            !matches!(
                n.state,
                AwaitingAck { .. } | AckRecording { .. } | Acknowledged { .. } | Consumed
            ),
            "{ctx}"
        );
    }
    assert!(
        n.epoch == w.epoch || (n.epoch == w.epoch + 1 && (n.state == Pending || is_spent(n.state))),
        "{ctx}"
    );
}

fn shape(s: SlotState) -> String {
    match s {
        SlotState::Spent { reason } => format!("Spent({reason:?})"),
        other => format!("{other:?}")
            .split([' ', '{'])
            .next()
            .unwrap()
            .to_string(),
    }
}

#[test]
fn td_o12_every_interleaving_of_two_assessments() {
    for kind in KINDS {
        let start = World {
            state: SlotState::Unassessed,
            epoch: 0,
            held: [Held::Idle; 2],
            ack: None,
            live: 0,
            restricts: 0,
            changes: 0,
            ack_begins: 0,
            admits: 0,
            tainted: false,
            consumed: 0,
        };
        let mut seen = HashSet::from([start]);
        let mut queue = VecDeque::from([start]);
        let mut shapes = BTreeSet::new();
        let mut admitted = BTreeSet::new();
        let mut transitions = 0usize;
        while let Some(w) = queue.pop_front() {
            for ev in events(&w) {
                let (n, res) = step(kind, &w, ev);
                check(kind, &w, ev, res, &n);
                transitions += 1;
                shapes.insert(shape(n.state));
                if let Res::Admitted(a) = res {
                    admitted.insert(format!("{a:?}"));
                }
                if seen.insert(n) {
                    queue.push_back(n);
                }
            }
        }
        // The exploration is not vacuous.
        let mut expected: BTreeSet<String> = ["Unassessed", "Pending", "Settling", "Clear"]
            .map(String::from)
            .into();
        for r in [
            SpentReason::Repeat,
            SpentReason::Restricted(STOP),
            SpentReason::Invalidated,
            SpentReason::Stale,
            SpentReason::Unrecorded,
        ] {
            expected.insert(shape(SlotState::Spent { reason: r }));
        }
        let mut admissions = BTreeSet::from(["Clear".to_string()]);
        if kind == Notes {
            expected.extend(
                ["AwaitingAck", "AckRecording", "Acknowledged", "Consumed"].map(String::from),
            );
            admissions.insert("Acknowledged".into());
        } else {
            expected.insert(shape(SlotState::Spent {
                reason: SpentReason::Restricted(NOTE),
            }));
        }
        assert_eq!(shapes, expected, "{kind:?}");
        assert_eq!(admitted, admissions, "{kind:?}");
        assert!(
            seen.len() > 500 && transitions > 5_000,
            "{kind:?}: {} states, {transitions} transitions",
            seen.len()
        );
    }
}
