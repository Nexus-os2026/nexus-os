//! Independent primitive-outcome product for spec §6.1 and B A3/A7/A10/A12.
//! This module deliberately has no production imports. Ordering and physical
//! observations are supplied by fixtures; a product result is never an input.
use super::model::{Answer, Cause, Mission, Record, MISSIONS, TERMINALS};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dispatch {
    None,
    LaneReserved,
    LaneEntered,
    ApprovalPending,
    Executing,
    Settling,
    LateResult,
}

pub const DISPATCH: [Dispatch; 7] = [
    Dispatch::None,
    Dispatch::LaneReserved,
    Dispatch::LaneEntered,
    Dispatch::ApprovalPending,
    Dispatch::Executing,
    Dispatch::Settling,
    Dispatch::LateResult,
];

/// Literal physical histories. Absence does not erase a prior publication.
/// Each row is (handoff, ever published, present now, conclusive, recovery).
pub const HISTORIES: [(bool, bool, bool, bool, bool); 8] = [
    (false, false, false, true, false),
    (true, false, false, false, false),
    (true, false, false, true, false),
    (true, false, false, true, true),
    (true, true, true, true, false),
    (true, true, true, true, true),
    (true, true, false, true, false),
    (true, true, false, false, true),
];

pub const ANSWERS: [Answer; 6] = [
    Answer::NotAsked,
    Answer::Awaiting,
    Answer::Declined,
    Answer::Affirmed,
    Answer::Invalidated,
    Answer::ProtocolFailed,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Expected {
    pub terminal: Option<Mission>,
    pub ever_published: bool,
    pub present: bool,
    pub uncertain: bool,
    pub interrupted: bool,
    pub recovery: bool,
    pub ordinary_approved: bool,
    pub owner_denied: bool,
    pub may_ack: bool,
    pub resource_debt: bool,
}

/// A scheduled primitive observation, not a production status/classifier.
pub struct Facts {
    pub mission: Mission,
    pub cause: Option<Cause>,
    pub first: Option<Mission>,
    pub dispatch: Dispatch,
    pub history: usize,
    pub interruption_ordered: bool,
    pub answer: Answer,
    pub exact_terminal_record: Record,
    pub exact_retained_action: bool,
    pub resource_debt: bool,
}

pub fn expected(f: Facts) -> Expected {
    let row = MISSIONS
        .iter()
        .position(|state| *state == f.mission)
        .unwrap();
    // Intentionally written independently of a production Cause::every.
    let column = f.cause.map(|cause| match cause {
        Cause::OwnerStop => 0,
        Cause::Emergency => 1,
        Cause::Budget => 2,
        Cause::DoorFence => 3,
        Cause::Denial => 4,
        Cause::LaneFailure => 5,
        Cause::VerifierFailure => 6,
        Cause::MissingVerdict => 7,
    });
    let (handed, ever, present, conclusive, recovery) = HISTORIES[f.history];
    // Executing is never itself a nonpublication proof. A conclusively
    // observed late result can refine the physical facts without re-running
    // the mission's terminal transition.
    let uncertain = !conclusive;
    let terminal = f.first.or_else(|| {
        if row >= 8 {
            Some(f.mission)
        } else {
            column.and_then(|column| TERMINALS[row][column])
        }
    });
    Expected {
        terminal,
        ever_published: ever,
        present,
        uncertain,
        interrupted: f.interruption_ordered,
        recovery,
        ordinary_approved: handed
            && matches!(f.dispatch, Dispatch::Settling)
            && present
            && conclusive
            && !recovery
            && !f.interruption_ordered
            && f.first.is_none()
            && f.cause.is_none()
            && row < 8
            && f.answer == Answer::Affirmed,
        owner_denied: f.answer == Answer::Declined
            && !handed
            && conclusive
            && terminal == Some(Mission::Denied)
            && f.exact_terminal_record == Record::Recorded,
        may_ack: f.exact_retained_action
            && conclusive
            && terminal.is_some()
            && f.exact_terminal_record == Record::Recorded,
        resource_debt: f.resource_debt,
    }
}
