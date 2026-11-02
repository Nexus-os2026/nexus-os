//! The agent door (Mother Nexus M2c, PR-A): what an agent holds instead of
//! the front door.
//!
//! The front door issues a door only after the owner's native yes, every
//! time ([`GovernedControl::issue_agent_door`]). A door is bound to its
//! agent, to the confirmer its host bound, and to its group: the runs it
//! opened or adopted. Every call on a run first checks, under the runs
//! lock, that the run is this agent's agent run and lists this door's
//! group; later admission steps check the run again as they always did.
//! Nothing reachable from a door leads to the front door, the authority,
//! the issuer or an owner API, and a door cannot be built, cloned or
//! deserialized outside this crate.

use super::{AgentOutcome, GovernedControl, Intent};
use crate::authority::approval::{
    ControlConfirmer, MissionApproval, MissionConfirmation, MissionResumeApproval,
    MissionResumeConfirmation,
};
use crate::authority::clock::Clock;
use crate::authority::commitment::CommitmentView;
use crate::authority::ids::{AgentId, CommitmentId, RunId};
use crate::authority::run::GroupId;
use crate::authority::AuthorityError;
use std::fmt;
use std::sync::Arc;

/// A door's identity: its opaque id, its agent and its group. The run
/// registry holds it weakly: once the door is dropped, its group is
/// reclaimed (no `Drop` logic here).
pub(crate) struct DoorCore {
    pub(crate) id: String,
    pub(crate) agent: AgentId,
    pub(crate) group: GroupId,
}

/// What a door may say about a commitment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Lookup<T> {
    /// A commitment of one of this door's runs.
    Mine(T),
    /// Not this door's: only whether it is settled for this door, which
    /// is when it is final and this door's agent's (another agent's
    /// commitment is never settled for this door).
    NotMine { settled: bool },
    /// The broker holds no such commitment.
    Unknown,
}

/// An agent's door to governed control.
pub struct AgentDoor {
    control: Arc<GovernedControl>,
    core: Arc<DoorCore>,
    confirmer: Arc<dyn ControlConfirmer + Send + Sync>,
}

impl fmt::Debug for AgentDoor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AgentDoor")
            .field("agent", &self.core.agent)
            .field("door", &self.core.id)
            .finish_non_exhaustive()
    }
}

impl AgentDoor {
    /// Only the front door's issue, after the owner's recorded yes.
    pub(crate) fn new(
        control: Arc<GovernedControl>,
        core: Arc<DoorCore>,
        confirmer: Arc<dyn ControlConfirmer + Send + Sync>,
    ) -> Self {
        Self {
            control,
            core,
            confirmer,
        }
    }

    /// The agent this door acts for.
    pub fn agent(&self) -> &AgentId {
        &self.core.agent
    }

    /// Open an agent run for this door's agent; it joins this door's group
    /// as it is inserted.
    pub fn open_run(&self) -> Result<RunId, AuthorityError> {
        self.control.settle();
        self.control
            .authority()
            .open_run_in(self.core.agent.clone(), self.core.group)
    }

    /// The run must be in this door's group.
    fn member(&self, run: RunId) -> Result<(), AuthorityError> {
        self.control
            .authority()
            .runs()
            .check_member(run, &self.core.agent, self.core.group)
    }

    /// An action in one of this door's runs, under the owner's standing
    /// grants (see [`GovernedControl::agent_action`]).
    pub fn agent_action(
        &self,
        run: RunId,
        intent: &Intent,
    ) -> Result<AgentOutcome, AuthorityError> {
        self.member(run)?;
        self.control.agent_action(&self.core.agent, run, intent)
    }

    /// Cancel one of this door's runs (recorded).
    pub fn cancel_run(&self, run: RunId) -> Result<(), AuthorityError> {
        self.member(run)?;
        self.control.cancel_run(run)
    }

    /// One of this door's runs is done: it finishes once nothing of it
    /// waits.
    pub fn finish_when_settled(&self, run: RunId) -> Result<(), AuthorityError> {
        self.member(run)?;
        self.control.finish_when_settled(run);
        Ok(())
    }

    /// Take back a waiting action of this agent's: the commitment must be
    /// this agent's and `run`'s, and `run` a live agent run of this agent
    /// in this registry, with room for one more group. Then `run` joins
    /// this door's group (membership only grows). It never cancels
    /// anything, and any refusal changes nothing.
    pub fn adopt(
        &self,
        run: RunId,
        commitment: CommitmentId,
    ) -> Result<CommitmentView, AuthorityError> {
        let authority = self.control.authority();
        authority.commitments().holding_view(commitment, |view| {
            let view = view.ok_or(AuthorityError::UnknownCommitment)?;
            if view.agent != self.core.agent {
                return Err(AuthorityError::WrongAgent);
            }
            if view.run != run {
                return Err(AuthorityError::WrongRun);
            }
            authority
                .runs()
                .join(run, &self.core.agent, self.core.group)?;
            Ok(view)
        })
    }

    /// What this door may know of a commitment.
    pub fn commitment(&self, id: CommitmentId) -> Lookup<CommitmentView> {
        let authority = self.control.authority();
        let Some(view) = authority.commitments().view(id) else {
            return Lookup::Unknown;
        };
        let agents = view.agent == self.core.agent;
        if agents && authority.runs().is_member(view.run, self.core.group) {
            return Lookup::Mine(view);
        }
        Lookup::NotMine {
            settled: agents && view.state.is_final(),
        }
    }

    /// Whether the owner's emergency stop holds.
    pub fn emergency_stopped(&self) -> bool {
        self.control.authority().runs().is_stopped()
    }

    /// The broker's clock (read-only).
    pub fn clock(&self) -> Arc<dyn Clock> {
        self.control.authority().clock().clone()
    }

    /// Ask the owner, through the confirmer the host bound to this door, to
    /// consent to a mission (see [`crate::authority::Authority::confirm_mission`]);
    /// after the record, an emergency stop or quitting refuses it.
    pub fn confirm_mission(
        &self,
        request: &MissionConfirmation,
    ) -> Result<MissionApproval, AuthorityError> {
        let authority = self.control.authority();
        let approval = authority.confirm_mission(request, &*self.confirmer)?;
        authority.mission_admissible()?;
        Ok(approval)
    }

    /// Ask the owner, through the confirmer the host bound to this door, to
    /// resume a held mission (see
    /// [`crate::authority::Authority::confirm_held_mission`]). The approval
    /// records the owner's yes; it resumes nothing.
    pub fn confirm_held_mission(
        &self,
        request: &MissionResumeConfirmation,
    ) -> Result<MissionResumeApproval, AuthorityError> {
        self.control
            .authority()
            .confirm_held_mission(request, &*self.confirmer)
    }
}
