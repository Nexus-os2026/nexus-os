//! The assessment slot (§3.2, §3.2a): a pure state machine with a private
//! epoch token, the acknowledgement states and single-use consumption. The
//! owning component (T3, T3b, T4) holds it under its lock; no lock spans
//! inference or a record write. (T1b oracle commit: interface stub.)

use std::fmt;

use crate::lane::Reason;

/// Which lane the slot serves. Only a notes slot can hold an
/// acknowledgeable `NeedsOwner`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SlotKind {
    Sentinel,
    Verifier,
    Notes,
}

/// Why a slot is `Spent`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SpentReason {
    /// A second assessment began.
    Repeat,
    /// A terminal restriction: a restrictive result, Stop, cancellation,
    /// revocation, expiry or a failed recheck.
    Restricted(Reason),
    /// A token, step or acknowledgement whose epoch or state did not match.
    Invalidated,
    /// The binding recomputed from live state changed.
    Stale,
    /// A record write failed.
    Unrecorded,
}

/// The slot's state. `Spent` is absorbing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SlotState {
    Unassessed,
    Pending,
    Settling,
    Clear {
        binding: [u8; 32],
    },
    Spent {
        reason: SpentReason,
    },
    AwaitingAck {
        assessment_epoch: u32,
        binding: [u8; 32],
        reason: Reason,
    },
    AckRecording {
        assessment_epoch: u32,
        binding: [u8; 32],
    },
    Acknowledged {
        assessment_epoch: u32,
        binding: [u8; 32],
    },
    Consumed,
}

/// The one assessment a slot admits. Not `Clone`, `Copy` or serializable;
/// `Debug` shows nothing of it.
pub struct AssessmentToken {
    epoch: u32,
    binding: [u8; 32],
}

/// What settle step 1 hands to step 2.
pub struct SettleToken {
    epoch: u32,
    binding: [u8; 32],
    outcome: Outcome,
}

/// What acknowledgement step 1 hands to step 3.
pub struct AckToken {
    epoch: u32,
    binding: [u8; 32],
}

macro_rules! redacted_debug {
    ($($name:ident),*) => {$(
        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(concat!(stringify!($name), "(..)"))
            }
        }
    )*};
}
redacted_debug!(AssessmentToken, SettleToken, AckToken);

// Test-only: tokens with given contents and their contents, for the model
// checker and the wrong-epoch cases. Absent from every other build.
#[cfg(test)]
impl AssessmentToken {
    pub(crate) fn forge(epoch: u32, binding: [u8; 32]) -> Self {
        Self { epoch, binding }
    }
    pub(crate) fn parts(&self) -> (u32, [u8; 32]) {
        (self.epoch, self.binding)
    }
}

#[cfg(test)]
impl SettleToken {
    pub(crate) fn forge(epoch: u32, binding: [u8; 32], outcome: Outcome) -> Self {
        Self {
            epoch,
            binding,
            outcome,
        }
    }
    pub(crate) fn parts(&self) -> (u32, [u8; 32], Outcome) {
        (self.epoch, self.binding, self.outcome)
    }
}

#[cfg(test)]
impl AckToken {
    pub(crate) fn forge(epoch: u32, binding: [u8; 32]) -> Self {
        Self { epoch, binding }
    }
    pub(crate) fn parts(&self) -> (u32, [u8; 32]) {
        (self.epoch, self.binding)
    }
}

/// What an assessment settles as. Every restrictive result instead goes
/// through [`Slot::restrict`], which needs no token.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Clear,
    /// A notes `NeedsOwner`, which the Owner may acknowledge.
    NeedsOwner(Reason),
}

#[derive(Debug)]
pub enum Begin {
    /// `Unassessed` → `Pending`: the one assessment this slot admits.
    Started(AssessmentToken),
    /// A repeat: the slot is now `Spent { Repeat }` and every outstanding
    /// token is invalid. The caller records it and cancels any call in
    /// flight.
    Repeat,
    /// Already `Spent`: refused without a new record.
    AlreadySpent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Installed {
    Clear,
    AwaitingAck { assessment_epoch: u32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Restricted {
    Installed,
    AlreadySpent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Admission {
    Clear,
    /// A consumed acknowledgement: single use.
    Acknowledged,
}

/// A refused step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refused {
    /// The slot was already `Spent`; nothing changed.
    AlreadySpent,
    /// This step's checks failed; the slot is now `Spent` for this reason,
    /// which the caller records.
    Spent(SpentReason),
}

/// One assessment slot: a Sentinel run's, a verification's, or a notes
/// screen's for one mission, contract hash and note index.
#[derive(Debug)]
pub struct Slot {
    kind: SlotKind,
    state: SlotState,
    epoch: u32,
}

#[cfg(test)]
impl Clone for Slot {
    fn clone(&self) -> Self {
        Self { ..*self }
    }
}

impl Slot {
    pub const fn new(kind: SlotKind) -> Self {
        Self {
            kind,
            state: SlotState::Unassessed,
            epoch: 0,
        }
    }

    /// Test-only: a slot in any state (the model checker).
    #[cfg(test)]
    pub(crate) fn forged(kind: SlotKind, state: SlotState, epoch: u32) -> Self {
        Self { kind, state, epoch }
    }

    pub const fn kind(&self) -> SlotKind {
        self.kind
    }
    pub const fn state(&self) -> SlotState {
        self.state
    }
    pub const fn epoch(&self) -> u32 {
        self.epoch
    }

    /// Begins the assessment, under the owning lock.
    pub fn begin(&mut self, _binding: [u8; 32]) -> Begin {
        Begin::AlreadySpent
    }

    /// Settle step 1, under the lock, before the unlocked record write.
    pub fn settle_begin(
        &mut self,
        _token: AssessmentToken,
        _outcome: Outcome,
        _live: &[u8; 32],
    ) -> Result<SettleToken, Refused> {
        Err(Refused::AlreadySpent)
    }

    /// Settle step 2, under the lock again, after the record write.
    pub fn settle_finish(
        &mut self,
        _token: SettleToken,
        _recorded: bool,
        _live: &[u8; 32],
    ) -> Result<Installed, Refused> {
        Err(Refused::AlreadySpent)
    }

    /// A terminal restriction: needs no token and always installs.
    pub fn restrict(&mut self, _reason: Reason) -> Restricted {
        Restricted::AlreadySpent
    }

    /// Acknowledgement step 1, under the coordinator's admission lock, with
    /// the epoch of the `AwaitingAck` the coordinator displayed.
    pub fn ack_begin(
        &mut self,
        _displayed_epoch: u32,
        _live: &[u8; 32],
    ) -> Result<AckToken, Refused> {
        Err(Refused::AlreadySpent)
    }

    /// Acknowledgement step 3, under the lock again, after the Owner's
    /// decision was recorded (step 2, unlocked).
    pub fn ack_finish(
        &mut self,
        _token: AckToken,
        _recorded: bool,
        _live: &[u8; 32],
    ) -> Result<(), Refused> {
        Err(Refused::AlreadySpent)
    }

    /// The admission check, under the owning lock: prepare and execute for
    /// the Sentinel, completion for the Verifier, the Builder lane's
    /// admission for a note (which consumes an acknowledgement).
    pub fn admit(&mut self, _live: &[u8; 32]) -> Result<Admission, Refused> {
        Err(Refused::AlreadySpent)
    }
}
