//! Immutable ordinary terminal intent, published before its atomic halt tag.
//! A competing touch completes the same intent. No caller waits for another
//! caller's journal or broker I/O; only the small immutable value is installed
//! through OnceLock. The one File slot is never reused in this process.
use super::*;
use crate::effect::FileSnapshot;
use nexus_governed_control::authority::commitment::{CommitmentState, PublicationClass};
use nexus_governed_control::authority::file_session::{HaltCause, HaltTag};
use std::num::NonZeroU16;
use std::sync::OnceLock;

const TERMINAL_TAG: u16 = u16::MAX - 4;

pub(super) enum FileTerminal {
    Failure {
        id: MissionId,
        trigger: Trigger,
    },
    NonFile {
        id: MissionId,
        trigger: Trigger,
    },
    Nonpublication {
        id: MissionId,
        generation: u64,
        trigger: Trigger,
    },
    Publication {
        id: MissionId,
        effect: u64,
        action: Option<PendingAction>,
        read: SettlementOutcome,
        generation: u64,
        receipt: Option<nexus_governed_control::files::FileReceipt>,
    },
    Deadline {
        id: MissionId,
        effect: u64,
        pending: PendingAction,
        generation: u64,
        resolved: OnceLock<FileSnapshot>,
    },
}

pub(super) fn is_terminal_intent(reason: (HaltCause, Option<NonZeroU16>)) -> bool {
    reason == (HaltCause::Fence, NonZeroU16::new(TERMINAL_TAG))
}

impl<P: EffectPort> Coordinator<P> {
    pub(super) fn file_latch_terminal(&self, intent: FileTerminal) -> Result<(), CoordinatorError> {
        let file = self.file.as_ref().ok_or(CoordinatorError::Stopped)?;
        // This initializer only moves already-owned data. It performs no I/O,
        // callback or mission/port lock. The tag cannot be seen without the
        // immutable intent; a competing Stop may still win the atomic latch.
        file.terminal.get_or_init(|| intent);
        file.cell.engage_tagged(HaltTag::Client {
            cause: HaltCause::Fence,
            reason: NonZeroU16::new(TERMINAL_TAG).expect("nonzero terminal tag"),
        });
        Ok(())
    }

    pub(super) fn file_finish_intent(
        &self,
        id: MissionId,
    ) -> Result<MissionState, CoordinatorError> {
        let intent = self
            .file
            .as_ref()
            .and_then(|file| file.terminal.get())
            .ok_or(CoordinatorError::Stopped)?;
        match intent {
            FileTerminal::Failure { id: owner, trigger } if *owner == id => self.file_record_end(
                id,
                SettleCause::Ended(BlockReason::PhaseInterrupted),
                Some(trigger.clone()),
            ),
            FileTerminal::NonFile { id: owner, trigger } if *owner == id => {
                self.record_non_file_end(id, trigger.clone())
            }
            FileTerminal::Nonpublication {
                id: owner,
                generation,
                trigger,
            } if *owner == id => self.file_record_nonpublication(id, *generation, trigger.clone()),
            FileTerminal::Publication {
                id: owner,
                effect,
                action,
                read,
                generation,
                receipt,
            } if *owner == id => self.file_record_terminal(
                id,
                *effect,
                action.clone(),
                (read.clone(), *receipt),
                Some(*generation),
            ),
            FileTerminal::Deadline {
                id: owner,
                effect,
                pending,
                generation,
                resolved,
            } if *owner == id => {
                self.file_finish_deadline(id, *effect, pending, *generation, resolved)
            }
            _ => Err(CoordinatorError::Stopped),
        }
    }

    fn file_finish_deadline(
        &self,
        id: MissionId,
        effect: u64,
        pending: &PendingAction,
        generation: u64,
        resolved: &OnceLock<FileSnapshot>,
    ) -> Result<MissionState, CoordinatorError> {
        let withdrawal = if resolved.get().is_none() {
            // D3: withdraw BEFORE observing publication. Racing completions
            // may help, but only the first installed snapshot is ever recorded.
            // Never hold an initialization lock across this broker operation.
            let withdrawn = self
                .port
                .withdraw(id)
                .map_err(|r| CoordinatorError::Withdrawal(r.class));
            let snapshot = self
                .port
                .file_publication_for(id, effect)
                .ok()
                .flatten()
                .filter(|s| s.action == *pending)
                .unwrap_or(FileSnapshot {
                    action: pending.clone(),
                    publication: PublicationClass::Uncertain,
                    state: None,
                    receipt: None,
                });
            let _ = resolved.set(snapshot);
            withdrawn
        } else {
            Ok(())
        };
        let snapshot = resolved.get().ok_or(CoordinatorError::Stopped)?;
        let state = if snapshot.publication == PublicationClass::ConfirmedNotPublished {
            let trigger = if snapshot.state == Some(CommitmentState::Denied) {
                Trigger::ActionDenied { effect }
            } else {
                Trigger::ApprovalLapsed {
                    effect,
                    reason: LapseReason::ApprovalExpired,
                }
            };
            self.file_record_nonpublication(id, generation, trigger)
        } else {
            let action = (!snapshot.action.commitment.is_empty()).then(|| snapshot.action.clone());
            self.file_record_terminal(
                id,
                effect,
                action,
                (
                    lite_settle::outcome(snapshot.publication.clone()),
                    snapshot.receipt,
                ),
                Some(generation),
            )
        }?;
        withdrawal?;
        Ok(state)
    }
}
