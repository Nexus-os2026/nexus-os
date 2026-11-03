//! The assessment slot (§3.2, §3.2a): a pure state machine with a private
//! epoch token, the acknowledgement states and single-use consumption. The
//! owning component (T3, T3b, T4) holds it under its lock; no lock spans
//! inference or a record write.
//!
//! Every slot holds a private epoch. It moves on once when the one
//! assessment begins and once when the slot becomes `Spent`, which
//! invalidates every outstanding token. Each step checks, in order, its
//! token's epoch and the exact state it needs, then (for the steps after a
//! record write) the record, then the binding recomputed from live state;
//! any failure makes the slot `Spent`. `Spent` is absorbing: from it no
//! step changes anything.

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

    /// `Spent` for `reason`, one epoch on, unless already `Spent`.
    fn spend(&mut self, reason: SpentReason) -> Refused {
        if matches!(self.state, SlotState::Spent { .. }) {
            return Refused::AlreadySpent;
        }
        self.state = SlotState::Spent { reason };
        self.epoch = self.epoch.saturating_add(1);
        Refused::Spent(reason)
    }

    /// Begins the assessment, under the owning lock. Only an `Unassessed`
    /// slot starts one; any other begin is a repeat, which spends the slot.
    pub fn begin(&mut self, binding: [u8; 32]) -> Begin {
        match self.state {
            SlotState::Unassessed => {
                self.state = SlotState::Pending;
                self.epoch = self.epoch.saturating_add(1);
                Begin::Started(AssessmentToken {
                    epoch: self.epoch,
                    binding,
                })
            }
            SlotState::Spent { .. } => Begin::AlreadySpent,
            _ => {
                self.spend(SpentReason::Repeat);
                Begin::Repeat
            }
        }
    }

    /// Settle step 1, under the lock, before the unlocked record write: the
    /// token's epoch is current, the state is `Pending` and the binding is
    /// current. Only a notes slot settles as `NeedsOwner`; for any other
    /// slot it is a terminal restriction.
    pub fn settle_begin(
        &mut self,
        token: AssessmentToken,
        outcome: Outcome,
        live: &[u8; 32],
    ) -> Result<SettleToken, Refused> {
        if token.epoch != self.epoch || self.state != SlotState::Pending {
            return Err(self.spend(SpentReason::Invalidated));
        }
        if let (Outcome::NeedsOwner(reason), false) = (outcome, self.kind == SlotKind::Notes) {
            return Err(self.spend(SpentReason::Restricted(reason)));
        }
        if token.binding != *live {
            return Err(self.spend(SpentReason::Stale));
        }
        self.state = SlotState::Settling;
        Ok(SettleToken {
            epoch: token.epoch,
            binding: token.binding,
            outcome,
        })
    }

    /// Settle step 2, under the lock again, after the record write: the
    /// epoch is still current, the state is still `Settling`, the record
    /// succeeded and the binding is still current. Only then is `Clear`, or
    /// a notes `AwaitingAck` with the token's epoch, installed.
    pub fn settle_finish(
        &mut self,
        token: SettleToken,
        recorded: bool,
        live: &[u8; 32],
    ) -> Result<Installed, Refused> {
        if token.epoch != self.epoch || self.state != SlotState::Settling {
            return Err(self.spend(SpentReason::Invalidated));
        }
        if !recorded {
            return Err(self.spend(SpentReason::Unrecorded));
        }
        if token.binding != *live {
            return Err(self.spend(SpentReason::Stale));
        }
        let (state, installed) = match token.outcome {
            Outcome::Clear => (
                SlotState::Clear {
                    binding: token.binding,
                },
                Installed::Clear,
            ),
            Outcome::NeedsOwner(reason) if self.kind == SlotKind::Notes => (
                SlotState::AwaitingAck {
                    assessment_epoch: token.epoch,
                    binding: token.binding,
                    reason,
                },
                Installed::AwaitingAck {
                    assessment_epoch: token.epoch,
                },
            ),
            Outcome::NeedsOwner(reason) => {
                return Err(self.spend(SpentReason::Restricted(reason)));
            }
        };
        self.state = state;
        Ok(installed)
    }

    /// A terminal restriction: needs no token and always installs.
    pub fn restrict(&mut self, reason: Reason) -> Restricted {
        match self.spend(SpentReason::Restricted(reason)) {
            Refused::Spent(_) => Restricted::Installed,
            Refused::AlreadySpent => Restricted::AlreadySpent,
        }
    }

    /// Acknowledgement step 1, under the coordinator's admission lock, with
    /// the epoch of the `AwaitingAck` the coordinator displayed: the state
    /// is exactly `AwaitingAck`, its epoch is the slot's and the displayed
    /// one, and the binding is unchanged.
    pub fn ack_begin(
        &mut self,
        displayed_epoch: u32,
        live: &[u8; 32],
    ) -> Result<AckToken, Refused> {
        let SlotState::AwaitingAck {
            assessment_epoch,
            binding,
            ..
        } = self.state
        else {
            return Err(self.spend(SpentReason::Invalidated));
        };
        if self.kind != SlotKind::Notes
            || assessment_epoch != self.epoch
            || displayed_epoch != assessment_epoch
        {
            return Err(self.spend(SpentReason::Invalidated));
        }
        if binding != *live {
            return Err(self.spend(SpentReason::Stale));
        }
        self.state = SlotState::AckRecording {
            assessment_epoch,
            binding,
        };
        Ok(AckToken {
            epoch: assessment_epoch,
            binding,
        })
    }

    /// Acknowledgement step 3, under the lock again, after the Owner's
    /// decision was recorded (step 2, unlocked): still `AckRecording` for
    /// the same epoch and binding, the record succeeded, the binding is
    /// still current.
    pub fn ack_finish(
        &mut self,
        token: AckToken,
        recorded: bool,
        live: &[u8; 32],
    ) -> Result<(), Refused> {
        let recording = SlotState::AckRecording {
            assessment_epoch: token.epoch,
            binding: token.binding,
        };
        if token.epoch != self.epoch || self.state != recording {
            return Err(self.spend(SpentReason::Invalidated));
        }
        if !recorded {
            return Err(self.spend(SpentReason::Unrecorded));
        }
        if token.binding != *live {
            return Err(self.spend(SpentReason::Stale));
        }
        self.state = SlotState::Acknowledged {
            assessment_epoch: token.epoch,
            binding: token.binding,
        };
        Ok(())
    }

    /// The admission check, under the owning lock: prepare and execute for
    /// the Sentinel, completion for the Verifier, the Builder lane's
    /// admission for a note. Only a current `Clear` (read, not consumed)
    /// or a current `Acknowledged` (consumed in this same step, once) is
    /// admitted; anything else spends the slot.
    pub fn admit(&mut self, live: &[u8; 32]) -> Result<Admission, Refused> {
        match self.state {
            SlotState::Clear { binding } => {
                if binding != *live {
                    return Err(self.spend(SpentReason::Stale));
                }
                Ok(Admission::Clear)
            }
            SlotState::Acknowledged {
                assessment_epoch,
                binding,
            } => {
                if assessment_epoch != self.epoch {
                    return Err(self.spend(SpentReason::Invalidated));
                }
                if binding != *live {
                    return Err(self.spend(SpentReason::Stale));
                }
                self.state = SlotState::Consumed;
                Ok(Admission::Acknowledged)
            }
            _ => Err(self.spend(SpentReason::Invalidated)),
        }
    }
}
