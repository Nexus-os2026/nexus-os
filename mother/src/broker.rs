//! Mother's one way to an effect: the existing broker.
//!
//! [`Broker`] names the few calls of `GovernedControl` the coordinator
//! makes, as an agent: open an agent's run, act in it under the Owner's
//! standing grants (an R2 action waits for the Owner's native approval),
//! read a commitment's state, cancel or finish the run, and ask whether the
//! Owner's emergency stop holds. The trait is sealed: outside this crate
//! only `GovernedControl` implements it. Mother never opens an owner run,
//! authorizes, executes, grants, revokes, resumes or raises the Owner's
//! broker dialog.

use nexus_governed_control::authority::commitment::CommitmentState;
use nexus_governed_control::authority::ids::{AgentId, CommitmentId, RunId};
use nexus_governed_control::authority::AuthorityError;
use nexus_governed_control::governed::{GovernedControl, Intent};

/// The agent name Mother acts under in the broker.
pub const MOTHER_AGENT: &str = "mother";

/// Mother's broker identity.
pub fn mother_agent() -> AgentId {
    todo!("M1 implementation follows the oracle commit")
}

/// What an action in the broker came to.
#[derive(Debug, PartialEq, Eq)]
pub enum Acted {
    /// The effect ran; its bounded text output, if any.
    Done { text: Option<String> },
    /// Committed; the Owner approves or denies it natively, outside Mother.
    AwaitingApproval { commitment: CommitmentId },
}

/// The broker refused or ended an action: only its stable class name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Refusal {
    pub class: &'static str,
}

impl Refusal {
    /// Whether the Owner's emergency stop refused it.
    pub fn is_emergency_stop(&self) -> bool {
        todo!("M1 implementation follows the oracle commit")
    }
}

impl From<&AuthorityError> for Refusal {
    fn from(error: &AuthorityError) -> Self {
        let _ = error;
        todo!("M1 implementation follows the oracle commit")
    }
}

mod sealed {
    pub trait Sealed {}
    impl Sealed for nexus_governed_control::governed::GovernedControl {}
    #[cfg(test)]
    impl Sealed for crate::oracles::fake::FakeBroker {}
}

/// The calls Mother makes to the broker.
pub trait Broker: Send + Sync + sealed::Sealed {
    /// Open an agent's run for `agent`.
    fn open_run(&self, agent: &AgentId) -> Result<RunId, Refusal>;
    /// An agent's action under the Owner's standing grants.
    fn act(&self, agent: &AgentId, run: RunId, intent: &Intent) -> Result<Acted, Refusal>;
    /// The state of a commitment, if the broker knows it.
    fn commitment_state(&self, id: CommitmentId) -> Option<CommitmentState>;
    /// Cancel a run: nothing more of it starts; what runs sees its token.
    fn cancel_run(&self, run: RunId) -> Result<(), Refusal>;
    /// The mission no longer acts in `run`: finish it once nothing waits.
    fn finish_when_settled(&self, run: RunId);
    /// Whether the Owner's emergency stop holds.
    fn emergency_stopped(&self) -> bool;
}

impl Broker for GovernedControl {
    fn open_run(&self, agent: &AgentId) -> Result<RunId, Refusal> {
        let _ = agent;
        todo!("M1 implementation follows the oracle commit")
    }

    fn act(&self, agent: &AgentId, run: RunId, intent: &Intent) -> Result<Acted, Refusal> {
        let _ = (agent, run, intent);
        todo!("M1 implementation follows the oracle commit")
    }

    fn commitment_state(&self, id: CommitmentId) -> Option<CommitmentState> {
        let _ = id;
        todo!("M1 implementation follows the oracle commit")
    }

    fn cancel_run(&self, run: RunId) -> Result<(), Refusal> {
        let _ = run;
        todo!("M1 implementation follows the oracle commit")
    }

    fn finish_when_settled(&self, run: RunId) {
        let _ = run;
        todo!("M1 implementation follows the oracle commit")
    }

    fn emergency_stopped(&self) -> bool {
        todo!("M1 implementation follows the oracle commit")
    }
}
