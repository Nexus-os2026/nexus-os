//! Observation only. These facts never authorize a call or discharge resources.
use super::*;
use std::sync::atomic::AtomicU8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RestoreOutcome {
    NotEntered,
    Pending,
    Completed,
    Refused,
    Uncertain,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GrantClosure {
    NotIssued,
    Unclosed,
    Closed,
    Unknown,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RestoreObservation {
    pub request: u64,
    pub target: Option<u64>,
    pub handed_off: bool,
    pub outcome: RestoreOutcome,
    pub grant: GrantClosure,
    pub interrupted: bool,
}
#[derive(Debug, Default)]
pub(super) struct RestoreFacts {
    outcome: AtomicU8,
    grant: AtomicU8,
}
impl RestoreFacts {
    pub(crate) fn entered(&self) {
        self.outcome.store(1, Ordering::SeqCst);
    }
    pub(crate) fn returned(&self, result: RestoreOutcome) {
        self.outcome.store(
            match result {
                RestoreOutcome::Completed => 2,
                RestoreOutcome::Refused => 3,
                _ => 4,
            },
            Ordering::SeqCst,
        );
    }
    pub(crate) fn grant(&self, state: GrantClosure) {
        self.grant.store(
            match state {
                GrantClosure::NotIssued => 0,
                GrantClosure::Unclosed => 1,
                GrantClosure::Closed => 2,
                GrantClosure::Unknown => 3,
            },
            Ordering::SeqCst,
        );
    }
}
impl SessionRequest {
    pub fn restore_observation(&self) -> RestoreObservation {
        let outcome = match self.scope.restore.outcome.load(Ordering::SeqCst) {
            0 => RestoreOutcome::NotEntered,
            1 => RestoreOutcome::Pending,
            2 => RestoreOutcome::Completed,
            3 => RestoreOutcome::Refused,
            _ => RestoreOutcome::Uncertain,
        };
        RestoreObservation {
            request: self.sequence,
            target: self.context(),
            handed_off: outcome != RestoreOutcome::NotEntered,
            outcome,
            grant: match self.scope.restore.grant.load(Ordering::SeqCst) {
                0 => GrantClosure::NotIssued,
                1 => GrantClosure::Unclosed,
                2 => GrantClosure::Closed,
                _ => GrantClosure::Unknown,
            },
            interrupted: self.core.cell.session_word() / EPOCH_STEP != self.generation(),
        }
    }
}
impl RequestScope {
    pub(crate) fn restore_entered(&self) {
        self.restore.entered();
    }
    pub(crate) fn restore_returned(&self, outcome: RestoreOutcome) {
        self.restore.returned(outcome);
    }
    pub(crate) fn restore_grant(&self, state: GrantClosure) {
        self.restore.grant(state);
    }
}

pub(super) struct UndoClaim {
    core: Arc<Core>,
    returned: bool,
}
impl UndoClaim {
    pub(super) fn take(core: &Arc<Core>, target: u64) -> Result<Self, SessionFailure> {
        core.undo_context
            .compare_exchange(target, 0, Ordering::SeqCst, Ordering::SeqCst)
            .map_err(|_| crate::authority::AuthorityError::Halted)?;
        Ok(Self {
            core: core.clone(),
            returned: false,
        })
    }
    pub(super) fn returned(mut self) {
        self.returned = true;
    }
}
impl Drop for UndoClaim {
    fn drop(&mut self) {
        if !self.returned {
            SessionIngress {
                core: self.core.clone(),
            }
            .fail();
        }
    }
}
