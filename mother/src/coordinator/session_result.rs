//! Observation of exact retained terminal intent. This never performs a touch,
//! transition, acknowledgement or recovery, and cannot establish authority.
use super::*;
use nexus_governed_control::authority::file_session::HaltCause;
use std::sync::atomic::Ordering;

#[derive(Clone, Debug)]
pub enum TerminalIntent {
    None,
    Transition(Trigger),
    Publication {
        effect: u64,
        outcome: SettlementOutcome,
    },
    Deadline {
        effect: u64,
    },
    Halt {
        cause: HaltCause,
        reason: Option<u16>,
    },
    Unknown,
}
#[derive(Clone, Debug)]
pub struct TerminalFacts {
    pub first: TerminalIntent,
    pub state: Option<MissionState>,
    pub settlement: Option<SettleCause>,
    pub recorded_sequence: Option<u64>,
    pub recording_unknown: bool,
}
impl TerminalFacts {
    pub fn first_code(&self) -> &'static str {
        match &self.first {
            TerminalIntent::None => "none",
            TerminalIntent::Transition(trigger) => trigger.name(),
            TerminalIntent::Publication { .. } => "publication",
            TerminalIntent::Deadline { .. } => "deadline",
            TerminalIntent::Halt {
                cause: HaltCause::Stop,
                ..
            } => "stop",
            TerminalIntent::Halt {
                cause: HaltCause::Emergency,
                ..
            } => "emergency",
            TerminalIntent::Halt {
                cause: HaltCause::TimeBudget,
                ..
            } => "budget",
            TerminalIntent::Halt {
                cause: HaltCause::Fence,
                ..
            } => "fence",
            TerminalIntent::Unknown => "unknown",
        }
    }
}
impl<P: EffectPort> FileSession<P> {
    pub fn terminal_facts(&self, id: MissionId) -> Result<TerminalFacts, CoordinatorError> {
        use super::lite_intent::{is_terminal_intent, FileTerminal};
        let c = &self.coordinator;
        let file = c.file.as_ref().ok_or(CoordinatorError::Stopped)?;
        if file.mission.load(Ordering::SeqCst) != id.0 {
            return Err(CoordinatorError::Stopped);
        }
        let observed = c.inner.lock().ok().and_then(|inner| {
            let m = inner.missions.get(&id)?;
            Some((
                m.state,
                m.settlement.map(|(_, cause)| cause),
                inner.store.load(),
            ))
        });
        let state = observed.as_ref().map(|(state, _, _)| *state);
        let settlement = observed.as_ref().and_then(|(_, cause, _)| *cause);
        let entries = observed
            .and_then(|(_, _, lines)| lines.ok())
            .and_then(|lines| verify(&lines).ok());
        let recorded = entries.as_ref().and_then(|entries| {
            if !state.is_some_and(MissionState::is_terminal) {
                return None;
            }
            entries.iter().rev().find_map(|entry| {
                if entry.mission == id {
                    if let Event::Transition { trigger } = &entry.event {
                        return Some((entry.seq, trigger.clone()));
                    }
                }
                None
            })
        });
        let first = if let Some((_, trigger)) = &recorded {
            TerminalIntent::Transition(trigger.clone())
        } else if let Some(reason) = file.cell.reason() {
            if is_terminal_intent(reason) {
                match file.terminal.get() {
                    Some(
                        FileTerminal::Failure { id: owner, trigger }
                        | FileTerminal::NonFile { id: owner, trigger }
                        | FileTerminal::Nonpublication {
                            id: owner, trigger, ..
                        },
                    ) if *owner == id => TerminalIntent::Transition(trigger.clone()),
                    Some(FileTerminal::Publication {
                        id: owner,
                        effect,
                        read,
                        ..
                    }) if *owner == id => TerminalIntent::Publication {
                        effect: *effect,
                        outcome: read.clone(),
                    },
                    Some(FileTerminal::Deadline {
                        id: owner, effect, ..
                    }) if *owner == id => TerminalIntent::Deadline { effect: *effect },
                    _ => TerminalIntent::Unknown,
                }
            } else {
                TerminalIntent::Halt {
                    cause: reason.0,
                    reason: reason.1.map(|r| r.get()),
                }
            }
        } else {
            TerminalIntent::None
        };
        Ok(TerminalFacts {
            first,
            state,
            settlement,
            recorded_sequence: recorded.map(|(seq, _)| seq),
            recording_unknown: entries.is_none(),
        })
    }
}
