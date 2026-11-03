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
//!
//! PR-B: its holder can halt it ([`AgentDoor::halt`]), for good. A halted
//! door refuses `open_run`, `agent_action`, `adopt`, `finish_when_settled`,
//! `confirm_mission` and `confirm_held_mission` with `Halted`, decided under
//! the runs lock before anything is settled, asked or written; it still
//! allows `cancel_run` and `halt` (they only stop effects) and answers
//! `clock`, `emergency_stopped` and `commitment`. Nothing lifts a door halt.
//!
//! M2d-B1 (design v2 §6.1 step 2, §11 H1′): the Owner's yeses at a door's
//! mission and held-mission dialogs are cached in the door's bounded table,
//! a slot reserved before the dialog and filled only if the lifetime it was
//! asked in still holds; [`AgentDoor::admit_files`] binds one of the door's
//! runs, once, to one of those yeses and the Owner's registered workspace.
//! A door registers no workspace and grants nothing.

use super::{AgentOutcome, GovernedControl, HaltReason, Intent, RecordBatch};
use crate::authority::approval::{
    ControlConfirmer, MissionApproval, MissionConfirmation, MissionResumeApproval,
    MissionResumeConfirmation,
};
use crate::authority::clock::Clock;
use crate::authority::commitment::{CommitmentState, CommitmentView};
use crate::authority::effect::CapabilityKind;
use crate::authority::evidence::Privacy;
use crate::authority::halt_cell::{HaltCell, HaltCells};
use crate::authority::ids::{AgentId, CommitmentId, RunId};
use crate::authority::run::{ConsentKind, GroupId};
use crate::authority::AuthorityError;
use crate::files::FileReceipt;
use crate::inert_page::ApprovedFacts;
use crate::inference::{
    Admitted, CallDeadline, CallMeter, ModelCallOutcome, ModelIdentityView, ModelRequest,
};
use std::fmt;
use std::sync::Arc;

/// A door's identity: its opaque id, its agent and its group. The run
/// registry holds it weakly: once the door is dropped, its group is
/// reclaimed (no `Drop` logic here).
pub(crate) struct DoorCore {
    pub(crate) id: String,
    pub(crate) agent: AgentId,
    pub(crate) group: GroupId,
    /// M2d-C round 4 (rule 3): the door's memory stop, shared with its
    /// group's entry: set without any lock, never cleared.
    pub(crate) halt: Arc<std::sync::atomic::AtomicBool>,
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
        // A halted door settles nothing (settling may write).
        self.not_halted()?;
        self.control.settle();
        #[cfg(test)]
        self.control
            .hooks()
            .fire(crate::test_hooks::at::OPEN_SETTLED);
        let run = self
            .control
            .authority()
            .open_run_in(self.core.agent.clone(), self.core.group)?;
        #[cfg(test)]
        self.control
            .hooks()
            .fire(crate::test_hooks::at::OPEN_CHECKED);
        Ok(run)
    }

    /// M2d-C round 4 (settlement design v2, rule 3): stop this door in
    /// memory, for good, first: no lock, no I/O, no panic. From then on
    /// none of its runs is admitted (open, propose, authorize, execute,
    /// join), and an execution of any of them sees the cancellation at its
    /// final check, whatever state the registries are in. The registry work
    /// (cancelling runs, revoking commitments, their records) is the
    /// caller's next step, which may fail with a typed error. It affects
    /// every run of this door's group: a host that shares one door across
    /// missions stops them all.
    pub fn stop_now(&self) {
        self.core
            .halt
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }

    /// M2d-C round 5 (H1): [`Self::open_run`] with the mission's halt cell,
    /// which the run keeps from its insertion (in the same hold): once the
    /// cell is engaged, no admission of the run passes and an execution of
    /// it sees the cell at its final check.
    pub fn open_run_with_halt(&self, cell: &HaltCell) -> Result<RunId, AuthorityError> {
        self.not_halted()?;
        self.control.settle();
        self.control
            .authority()
            .open_run_in_with(self.core.agent.clone(), self.core.group, cell)
    }

    /// M2d-C round 5 (C1): this agent's partition of the broker's halt
    /// table (taken once, when a port is built; not on a Stop path).
    pub fn halt_cells(&self) -> HaltCells {
        self.control.authority().halt_cells(&self.core.agent)
    }

    /// PR-B: refused once this door is halted (read under the runs lock);
    /// round 4: or stopped in memory (read first, without it).
    fn not_halted(&self) -> Result<(), AuthorityError> {
        if self.core.halt.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(AuthorityError::Halted);
        }
        if self.control.authority().runs().door_halted(self.core.group) {
            return Err(AuthorityError::Halted);
        }
        Ok(())
    }

    /// PR-B: halt this door, for good: every run of its group is cancelled,
    /// their unconsumed commitments revoked, their leases ended and their
    /// pending effects dropped, in memory (no I/O). Once it returns, none of
    /// its runs admits a commitment to Executing, and none joins its group.
    /// The records of what this call did are returned, to write once no lock
    /// is held. Idempotent: a later call makes only what is left to make.
    pub fn halt(&self, reason: HaltReason) -> RecordBatch {
        self.control.halt_door(&self.core, reason)
    }

    /// The run must be in this door's group.
    fn member(&self, run: RunId) -> Result<(), AuthorityError> {
        self.control
            .authority()
            .runs()
            .check_member(run, &self.core.agent, self.core.group, false)
    }

    /// An action in one of this door's runs, under the owner's standing
    /// grants (see [`GovernedControl::agent_action`]).
    pub fn agent_action(
        &self,
        run: RunId,
        intent: &Intent,
    ) -> Result<AgentOutcome, AuthorityError> {
        self.member(run)?;
        #[cfg(test)]
        self.control
            .hooks()
            .fire(crate::test_hooks::at::DOOR_MEMBER_CHECKED);
        self.control.agent_action(&self.core.agent, run, intent)
    }

    /// Cancel one of this door's runs, also once the door is halted (a
    /// cancellation only stops effects). PR-B: in memory; the records of
    /// what this call did are returned, to write once no lock is held.
    pub fn cancel_run(&self, run: RunId) -> Result<RecordBatch, AuthorityError> {
        self.control.authority().runs().check_member(
            run,
            &self.core.agent,
            self.core.group,
            true,
        )?;
        Ok(self.control.cancel_member(run))
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
        self.adopt_inner(run, commitment, None)
    }

    /// M2d-C round 5 (D4(a), C1): [`Self::adopt`] for a mission, attaching
    /// its halt cell to the run in the same hold as the join and the check
    /// below, so every later execution token of the run carries it. Refused,
    /// changing nothing, while any commitment of the run is authorized or
    /// executing, or any Files obligation of it is unacknowledged (such a
    /// run is reachable only through ack-only ownership:
    /// [`Self::ack_commitment`], [`Self::cancel_orphan`]).
    pub fn adopt_with_halt(
        &self,
        run: RunId,
        commitment: CommitmentId,
        cell: &HaltCell,
    ) -> Result<CommitmentView, AuthorityError> {
        self.adopt_inner(run, commitment, Some(cell))
    }

    fn adopt_inner(
        &self,
        run: RunId,
        commitment: CommitmentId,
        cell: Option<&HaltCell>,
    ) -> Result<CommitmentView, AuthorityError> {
        // A halted door says so first (the join checks it again).
        if self.core.halt.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(AuthorityError::Halted);
        }
        let authority = self.control.authority();
        // Round 4 (rule 3): on the recovery path, never a panic.
        authority
            .commitments()
            .try_adopt_hold(commitment, run, |view, blocked| {
                let view = view.ok_or(AuthorityError::UnknownCommitment)?;
                if view.agent != self.core.agent {
                    return Err(AuthorityError::WrongAgent);
                }
                if view.run != run {
                    return Err(AuthorityError::WrongRun);
                }
                // D4(a): the run's membership is closed (it admits only
                // acknowledgement); refused as a halt is, changing nothing
                // (the M2c B7 oracle: an adoption is taken, or refused
                // Halted).
                if blocked {
                    return Err(AuthorityError::Halted);
                }
                authority
                    .runs()
                    .join_with(run, &self.core.agent, self.core.group, cell)?;
                Ok(view)
            })
    }

    /// M2d-C round 5 (D4(a)): ack-only ownership's cancellation: a run of
    /// this door's, or (once the door that held it is gone) an agent run of
    /// this door's agent that no live door lists, is cancelled (its
    /// unconsumed commitments revoked). It only narrows: no consent, no
    /// execution, no view. Never panics.
    pub fn cancel_orphan(&self, run: RunId) -> Result<RecordBatch, AuthorityError> {
        self.control
            .authority()
            .runs()
            .ack_owner(run, &self.core.agent, self.core.group)?;
        Ok(self.control.cancel_member(run))
    }

    /// M2d-C (Codex review 2 of PR #78, HIGH 3): [`AgentDoor::commitment`]
    /// that never panics: a registry the broker cannot read (its lock
    /// poisoned) is `Unavailable`; otherwise the same answer.
    pub fn try_commitment(
        &self,
        id: CommitmentId,
    ) -> Result<Lookup<CommitmentView>, AuthorityError> {
        let unavailable = AuthorityError::Unavailable("the broker cannot read its records");
        let authority = self.control.authority();
        let Some(view) = authority
            .commitments()
            .try_view(id)
            .ok_or(unavailable.clone())?
        else {
            return Ok(Lookup::Unknown);
        };
        let agents = view.agent == self.core.agent;
        let member = authority
            .runs()
            .try_is_member(view.run, self.core.group)
            .ok_or(unavailable)?;
        if agents && member {
            return Ok(Lookup::Mine(view));
        }
        Ok(Lookup::NotMine {
            settled: agents && view.state.is_final(),
        })
    }

    /// M2d-C round 4 (settlement design v2, rule 2; addendum 4): acknowledge
    /// that the caller durably recorded what became of its Files commitment
    /// `id`: its pin (and its run's retention) is released, exactly once.
    /// Only a conclusive outcome is released; a repeated ack changes
    /// nothing. Owner-scoped: the run must be one of this door's, or, once
    /// the door that held it is gone, an agent run of this door's agent no
    /// live door lists (ack-only ownership: it restores no consent, no
    /// execution authority and shows nothing). Also once halted and at
    /// capacity; never panics.
    pub fn ack_commitment(&self, id: CommitmentId) -> Result<(), AuthorityError> {
        let authority = self.control.authority();
        let runs = authority.runs();
        authority.commitments().ack(id, &self.core.agent, |run| {
            runs.ack_owner(run, &self.core.agent, self.core.group)
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
        self.not_halted()?;
        let authority = self.control.authority();
        let runs = authority.runs();
        // The door's holder supplies the labels: only controlled ones, and
        // the record is Restricted (P0-C). M2d-B1: a slot in this door's
        // consent table is reserved just before the Owner is asked.
        // S1a-D+B: the page facts, built before the dialog, come back as the
        // dialog showed them.
        let (approval, recorded_ms, slot, facts) =
            authority.confirm_mission_as(request, &*self.confirmer, Privacy::Restricted, || {
                runs.consent_slot(self.core.group, &self.core.agent)
            })?;
        #[cfg(test)]
        self.control
            .hooks()
            .fire(crate::test_hooks::at::MISSION_RECORDED);
        // After the record: it says the owner said yes; no approval exists
        // if this door was halted, or a stop came, meanwhile.
        self.not_halted()?;
        authority.mission_admissible()?;
        self.cache(
            slot,
            approval.contract_hash(),
            approval.request(),
            (approval.evidence_seq(), recorded_ms),
            (ConsentKind::Mission, facts),
        )?;
        Ok(approval)
    }

    /// M2d-B1 (H1′): cache the Owner's recorded yes in this door's table,
    /// with its record's sequence number and own time, only if the lifetime
    /// it was asked in still holds; otherwise no approval exists. S1a-D+B:
    /// with the page facts its dialog showed (none for a held resume, whose
    /// dialog shows none).
    fn cache(
        &self,
        slot: crate::authority::run::ConsentSlot<'_>,
        contract_hash: &str,
        request: &str,
        (evidence_seq, at_wall_ms): (u64, u64),
        (kind, page_facts): (ConsentKind, Option<ApprovedFacts>),
    ) -> Result<(), AuthorityError> {
        #[cfg(test)]
        self.control
            .hooks()
            .fire(crate::test_hooks::at::CONSENT_BEFORE_FILL);
        slot.fill(
            contract_hash,
            request,
            evidence_seq,
            kind,
            at_wall_ms,
            page_facts,
        )
    }

    /// M2d-B1 (A3, H1′): bind one of this door's runs, once, to the Owner's
    /// yes to exactly `contract_hash` and `request` cached at this door, and
    /// to the workspace the Owner registered (its identity checked now, with
    /// no lock held). The binding is validated and installed in one hold of
    /// the runs lock with the door's life, the run's membership and life,
    /// Stop and closure. The same admission again is the same binding; any
    /// other second admission is refused; any refusal binds nothing.
    pub fn admit_files(
        &self,
        run: RunId,
        contract_hash: &str,
        request: &str,
    ) -> Result<(), AuthorityError> {
        self.not_halted()?;
        let workspace = self.control.files().admissible()?;
        #[cfg(test)]
        self.control
            .hooks()
            .fire(crate::test_hooks::at::ADMIT_BEFORE_BIND);
        self.control
            .authority()
            .runs()
            .admit_files(
                run,
                &self.core.agent,
                self.core.group,
                contract_hash,
                request,
                workspace,
            )
            .map(|_| ())
    }

    /// M2e-B (design v2 §8): the registered local model, if any (its name,
    /// manifest digest and identity digest).
    pub fn model_identity(&self) -> Option<ModelIdentityView> {
        self.control
            .registered_model()
            .map(|model| model.view.clone())
    }

    /// M2e-B (design v2 §8, H1′): bind one of this door's runs, once, to
    /// the Owner's yes to exactly `contract_hash` and `request` cached at
    /// this door, and to the registered model, which must be
    /// `expected_identity` (the identity the Owner consented to). Validated
    /// and installed in one hold of the runs lock with the door's life, the
    /// run's membership and life, Stop and closure (shared with Files
    /// admission). The same admission again is the same binding; any other
    /// second admission is refused; any refusal binds nothing.
    pub fn admit_inference(
        &self,
        run: RunId,
        contract_hash: &str,
        request: &str,
        expected_identity: [u8; 32],
    ) -> Result<(), AuthorityError> {
        self.not_halted()?;
        let model = self
            .control
            .registered_model()
            .ok_or(AuthorityError::ModelNotRegistered)?;
        if model.identity != expected_identity {
            return Err(AuthorityError::ModelIdentityMismatch);
        }
        #[cfg(test)]
        self.control
            .hooks()
            .fire(crate::test_hooks::at::INFERENCE_BEFORE_BIND);
        self.control
            .authority()
            .runs()
            .admit_inference(
                run,
                &self.core.agent,
                self.core.group,
                contract_hash,
                request,
                model.identity,
            )
            .map(|_| ())
    }

    /// M2e-B (design v2 §6 step 3, §8): one model call of one of this
    /// door's runs, charged to `meter` immediately before the chat and
    /// ended by `deadline` (absolute, the caller's), the run's cancellation,
    /// this door's halt, the session's end or the Owner's Stop. Admitted
    /// in one hold of the runs lock (the run this door's live agent run,
    /// admitted for inference to the registered model, under its attempt
    /// ceiling); then no broker lock is held. Refused before any attempt
    /// (nothing charged, sent or recorded) while the evidence store cannot
    /// record or owes a record. Every attempt is recorded (Restricted), and what became of
    /// its record is returned with the transport's result. The reply is
    /// untrusted data.
    pub fn model_call(
        &self,
        run: RunId,
        request: &ModelRequest,
        meter: &dyn CallMeter,
        deadline: &dyn CallDeadline,
    ) -> Result<ModelCallOutcome, AuthorityError> {
        self.not_halted()?;
        let model = self
            .control
            .registered_model()
            .ok_or(AuthorityError::ModelNotRegistered)?;
        // A known evidence failure (a latched store, recovery required, or
        // any record owed) permits no new inference.
        self.control.authority().inference_admissible()?;
        let ticket = self.control.authority().runs().admit_model_call(
            run,
            &self.core.agent,
            self.core.group,
            &model.identity,
        )?;
        Ok(crate::inference::call(
            self.control.authority(),
            self.control.kernel_stop_authority(),
            &model,
            Admitted {
                run,
                agent: &self.core.agent,
                ticket,
            },
            request,
            (meter, deadline),
        ))
    }

    /// S1a-D+B (M3 §6.2): the broker's receipt for a Files create of one of
    /// this door's runs that succeeded (published, possibly overtaken after
    /// its final check); `None` for any other commitment, another door's,
    /// one not yet executed, or one that was denied, expired, lapsed or
    /// failed. Digests and a size only.
    pub fn file_receipt(&self, id: CommitmentId) -> Option<FileReceipt> {
        match self.commitment(id) {
            Lookup::Mine(view)
                if view.kind == CapabilityKind::Files
                    && view.state == CommitmentState::Succeeded =>
            {
                self.control.files().receipt(id)
            }
            Lookup::Mine(_) | Lookup::NotMine { .. } | Lookup::Unknown => None,
        }
    }

    /// The yeses cached at this door (tests).
    #[cfg(test)]
    pub(crate) fn consents_cached(&self) -> usize {
        self.control
            .authority()
            .runs()
            .consents_cached(self.core.group)
    }

    /// Ask the owner, through the confirmer the host bound to this door, to
    /// resume a held mission (see
    /// [`crate::authority::Authority::confirm_held_mission`]). The approval
    /// records the owner's yes; it resumes nothing.
    pub fn confirm_held_mission(
        &self,
        request: &MissionResumeConfirmation,
    ) -> Result<MissionResumeApproval, AuthorityError> {
        self.not_halted()?;
        let runs = self.control.authority().runs();
        let (approval, recorded_ms, slot) = self.control.authority().confirm_held_mission_as(
            request,
            &*self.confirmer,
            Privacy::Restricted,
            || runs.consent_slot(self.core.group, &self.core.agent),
        )?;
        #[cfg(test)]
        self.control
            .hooks()
            .fire(crate::test_hooks::at::HELD_RECORDED);
        // After the record (§2.6 step 6): no approval if halted meanwhile.
        self.not_halted()?;
        self.cache(
            slot,
            approval.contract_hash(),
            approval.request(),
            (approval.evidence_seq(), recorded_ms),
            (ConsentKind::Resume, None),
        )?;
        Ok(approval)
    }
}
