//! The one pipeline every governed effect passes through.
//!
//! A domain actuator turns a typed intent into a `PreparedAction` (what the
//! owner and the evidence see, and what the commitment binds) and a pending
//! effect (the typed payload it will use, held here in backend memory and
//! never handed out). The control then runs the commitment lifecycle:
//!
//! `propose` → `prepare` (+ native approval for R2) → `authorize` →
//! `execute`: revalidate the target, check the payload's parameter digest,
//! consume the commitment once, perform the bounded effect under the guard,
//! finalize.
//!
//! There is no other way to reach an actuator's effect: the pending effects
//! are private to this module, keyed by commitment, and taken out exactly
//! once by `execute`.

use crate::authority::approval::ControlConfirmer;
use crate::authority::commitment::{
    CommitmentView, ExecutionGuard, FailureClass, Outcome, PreparedAction,
};
use crate::authority::evidence::ExecutionEvidence;
use crate::authority::ids::{AgentId, CommitmentId, Digest, RunId};
use crate::authority::run::RunClass;
use crate::authority::{Authority, AuthorityError, Stopped};
use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

/// What an executed effect returns to its caller: data only.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EffectOutput {
    /// Bounded text (a page's text, a tool's output, a response body).
    pub text: Option<String>,
    /// Bounded bytes kept in memory (a capture for analysis).
    pub bytes: Option<Vec<u8>>,
    /// Bounded, redacted metadata (also recorded as evidence).
    pub meta: Vec<(String, String)>,
    /// The effect's evidence: its start record, and what became of its end
    /// record (EL-1 §8). Set by the pipeline when the effect has ended.
    pub evidence: Option<ExecutionEvidence>,
}

/// An effect prepared by a domain actuator, waiting for its commitment.
pub(crate) trait PendingEffect: Send {
    fn bind_commitment(&self, _id: CommitmentId) -> Result<(), AuthorityError> {
        Ok(())
    }

    /// Resolve the target again, immediately before the effect, and return
    /// its identity digest (it must equal the committed one).
    fn revalidate(&self) -> Result<Digest, AuthorityError>;
    /// The digest of the exact parameters `execute` will use.
    fn parameters(&self) -> Digest;
    /// Perform the bounded effect. The guard's cancel token must be observed
    /// between bounded steps.
    fn execute(
        self: Box<Self>,
        guard: &ExecutionGuard,
    ) -> Result<EffectOutput, (FailureClass, String)>;
}

/// A domain's preparation: the action to commit to, and the effect. Only
/// the crate's own actuators build one, and only the pipeline consumes it.
pub(crate) struct Preparation {
    pub(crate) action: PreparedAction,
    pub(crate) effect: Box<dyn PendingEffect>,
    /// How long the commitment may wait to start.
    pub(crate) ttl: Duration,
}

/// A proposed effect, held for its owner until it is executed or ends.
struct Pending {
    agent: AgentId,
    run: RunId,
    effect: Box<dyn PendingEffect>,
}

/// The pipeline.
pub(crate) struct Control {
    authority: Authority,
    pending: Mutex<PendingTable>,
}

/// The pending effects, and the places held for proposals being prepared
/// (counted with them, so that a place is checked and taken at once).
#[derive(Default)]
struct PendingTable {
    held: HashMap<CommitmentId, Pending>,
    reserved: usize,
    reserved_by_run: HashMap<RunId, usize>,
}

impl PendingTable {
    fn give_back(&mut self, run: RunId) {
        self.reserved = self.reserved.saturating_sub(1);
        if let Some(count) = self.reserved_by_run.get_mut(&run) {
            *count -= 1;
            if *count == 0 {
                self.reserved_by_run.remove(&run);
            }
        }
    }

    /// Take out every effect `keep` refuses (dropped by the caller, outside
    /// the lock).
    fn take_unless(&mut self, keep: impl Fn(&CommitmentId, &Pending) -> bool) -> Vec<Pending> {
        let ids: Vec<CommitmentId> = self
            .held
            .iter()
            .filter(|(id, held)| !keep(id, held))
            .map(|(id, _)| *id)
            .collect();
        ids.into_iter()
            .filter_map(|id| self.held.remove(&id))
            .collect()
    }
}

/// A place held for one proposal while it is prepared: filled by its
/// effect, or given back when dropped (the preparation was refused).
pub(crate) struct Place<'a> {
    control: &'a Control,
    run: RunId,
    held: bool,
    cell: Option<crate::authority::file_session::HaltCell>,
}

impl Place<'_> {
    /// Hold the effect of a prepared commitment; one that ended meanwhile
    /// (its run was cancelled while it was prepared) is not held.
    fn fill(
        mut self,
        id: CommitmentId,
        pending: Pending,
        halt: Option<&crate::authority::file_session::HaltCell>,
    ) -> Result<(), AuthorityError> {
        crate::authority::file_session::check_halt(halt)?;
        #[cfg(test)]
        self.control
            .authority
            .hooks()
            .fire(crate::test_hooks::at::PLACE_BEFORE_FILL);
        let (unheld, refused) = {
            let mut table = self.control.pending();
            table.give_back(self.run);
            self.held = false;
            let unconsumed = self.control.authority.commitments().is_unconsumed(id);
            #[cfg(test)]
            self.control
                .authority
                .hooks()
                .fire("decision8.pending_validated");
            #[cfg(test)]
            crate::authority::file_session::probe(halt, "registry.pending_install.before");
            let refused = crate::authority::file_session::check_halt(halt).err();
            if unconsumed && refused.is_none() {
                table.held.insert(id, pending);
                #[cfg(test)]
                crate::authority::file_session::probe(halt, "registry.pending_install.after");
                (None, None)
            } else {
                (Some(pending), refused)
            }
        };
        // An effect not held is dropped outside the lock.
        drop(unheld);
        match refused {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

impl Drop for Place<'_> {
    fn drop(&mut self) {
        if self.held {
            if let Ok(mut table) = self.control.pending.lock() {
                table.give_back(self.run);
            } else if let Some(cell) = &self.cell {
                cell.fail_session();
            }
        }
    }
}

/// Most pending effects held at once.
const MAX_PENDING: usize = 1024;
/// Most pending effects one run holds (R2 waiting for the owner, mostly).
const MAX_PENDING_PER_RUN: usize = 32;
/// Pending places only the owner's own commands may take, so that agents
/// filling the rest cannot block the owner. An owner command is one whose
/// run the backend opened as the owner's ([`RunClass::Owner`]); an agent's
/// name earns it nothing.
const OWNER_RESERVE: usize = 64;

impl Control {
    pub(crate) fn new(authority: Authority) -> Self {
        Self {
            authority,
            pending: Mutex::new(PendingTable::default()),
        }
    }

    /// Take and release every lock of the pipeline once (tests).
    #[cfg(test)]
    pub(crate) fn probe_locks(&self) {
        drop(self.pending());
        self.authority.probe_locks();
    }

    fn pending(&self) -> MutexGuard<'_, PendingTable> {
        self.pending.lock().expect("pending")
    }

    /// Check for room and take a place for a proposal in `run`, at once,
    /// under the lock.
    fn place(
        &self,
        run: RunId,
        limit: usize,
        request: Option<&crate::authority::file_session::HaltCell>,
    ) -> Result<Place<'_>, AuthorityError> {
        let mut table = self.pending();
        let in_run = table.held.values().filter(|p| p.run == run).count()
            + table.reserved_by_run.get(&run).copied().unwrap_or(0);
        if table.held.len() + table.reserved >= limit || in_run >= MAX_PENDING_PER_RUN {
            return Err(AuthorityError::Capacity);
        }
        #[cfg(test)]
        let cell = self.authority.runs().session_cell(run).ok().flatten();
        #[cfg(test)]
        crate::authority::file_session::probe(cell.as_ref(), "registry.pending_reserve.before");
        crate::authority::file_session::check_halt(request)?;
        table.reserved += 1;
        *table.reserved_by_run.entry(run).or_default() += 1;
        #[cfg(test)]
        crate::authority::file_session::probe(cell.as_ref(), "registry.pending_reserve.after");
        Ok(Place {
            control: self,
            run,
            held: true,
            cell: request.cloned(),
        })
    }

    pub(crate) fn authority(&self) -> &Authority {
        &self.authority
    }

    /// Commit to a prepared effect for `agent` in `run`. Nothing happens yet.
    pub(crate) fn reserve_host_proposal(
        &self,
        run: RunId,
    ) -> Result<(Place<'_>, crate::authority::commitment::PreparationSlot), AuthorityError> {
        self.reserve_host_proposal_request(run, None)
    }
    pub(crate) fn reserve_host_proposal_request(
        &self,
        run: RunId,
        request: Option<&crate::authority::file_session::HaltCell>,
    ) -> Result<(Place<'_>, crate::authority::commitment::PreparationSlot), AuthorityError> {
        self.authority.runs().admit_file_request(run, request)?;
        let limit = match self.authority.runs().class(run) {
            Some(RunClass::Owner) => MAX_PENDING,
            Some(RunClass::Agent) => MAX_PENDING - OWNER_RESERVE,
            None => return Err(AuthorityError::UnknownRun),
        };
        let cell = match request {
            Some(cell) => cell.clone(),
            None => self
                .authority
                .runs()
                .session_cell(run)?
                .ok_or(AuthorityError::Halted)?,
        };
        let place = self.place(run, limit, Some(&cell))?;
        let slot = self.authority.commitments().reserve_preparation(cell)?;
        Ok((place, slot))
    }

    pub(crate) fn propose_reserved(
        &self,
        agent: &AgentId,
        run: RunId,
        preparation: Preparation,
        reservation: (Place<'_>, crate::authority::commitment::PreparationSlot),
    ) -> Result<CommitmentView, AuthorityError> {
        self.propose_with_reservation(agent, run, preparation, Some(reservation))
    }

    pub(crate) fn propose(
        &self,
        agent: &AgentId,
        run: RunId,
        preparation: Preparation,
    ) -> Result<CommitmentView, AuthorityError> {
        self.propose_with_reservation(agent, run, preparation, None)
    }

    fn propose_with_reservation(
        &self,
        agent: &AgentId,
        run: RunId,
        preparation: Preparation,
        reservation: Option<(Place<'_>, crate::authority::commitment::PreparationSlot)>,
    ) -> Result<CommitmentView, AuthorityError> {
        let Preparation {
            action,
            effect,
            ttl,
        } = preparation;
        // A refused proposal leaves no credential lease behind.
        let refuse = |error: AuthorityError| {
            self.authority.commitments().end_leases(&action.leases);
            Err(error)
        };
        if effect.parameters() != action.parameters {
            return refuse(AuthorityError::InvalidAction(
                "the prepared parameters do not match the effect",
            ));
        }
        // The owner's reserve is for the owner's own command runs, as the
        // backend opened them; never for a name.
        let limit = match self.authority.runs().class(run) {
            Some(RunClass::Owner) => MAX_PENDING,
            Some(RunClass::Agent) => MAX_PENDING - OWNER_RESERVE,
            None => return refuse(AuthorityError::UnknownRun),
        };
        let halt = match reservation
            .as_ref()
            .and_then(|(place, _)| place.cell.clone())
        {
            Some(cell) => Some(cell),
            None => match self.authority.runs().session_cell(run) {
                Ok(cell) => cell,
                Err(error) => return refuse(error),
            },
        };
        if let Err(error) = crate::authority::file_session::check_halt(halt.as_ref()) {
            return refuse(error);
        }
        let (place, slot) = match reservation {
            Some((place, slot)) if std::ptr::eq(place.control, self) && place.run == run => {
                (place, Some(slot))
            }
            Some(_) => return refuse(AuthorityError::Halted),
            None => {
                let place = match self.place(run, limit, halt.as_ref()) {
                    Ok(place) => place,
                    Err(AuthorityError::Capacity) => {
                        self.prune();
                        match self.place(run, limit, halt.as_ref()) {
                            Ok(place) => place,
                            Err(error) => return refuse(error),
                        }
                    }
                    Err(error) => return refuse(error),
                };
                (place, None)
            }
        };
        let view = match slot {
            Some(slot) => self
                .authority
                .commitments()
                .prepare_reserved(agent, run, action, ttl, slot)?,
            None => self
                .authority
                .commitments()
                .prepare(agent, run, action, ttl)?,
        };
        if let Err(error) = effect.bind_commitment(view.id) {
            self.authority.commitments().refuse_pending_install(view.id);
            return Err(error);
        }
        if let Err(error) = place.fill(
            view.id,
            Pending {
                agent: agent.clone(),
                run,
                effect,
            },
            halt.as_ref(),
        ) {
            // No pending effect was installed. End the earlier commitment
            // truthfully outside the pending-table hold, keeping its pin/ack
            // bookkeeping under the existing File lifecycle rules.
            self.authority.commitments().refuse_pending_install(view.id);
            return Err(error);
        }
        Ok(view)
    }

    /// Authorize a commitment: R0 and R1 directly (their grants were checked
    /// when they were prepared and are checked again here), R2 only after the
    /// owner's native approval of exactly this commitment.
    pub(crate) fn authorize(
        &self,
        id: CommitmentId,
        agent: &AgentId,
        run: RunId,
        confirmer: &dyn ControlConfirmer,
    ) -> Result<(), AuthorityError> {
        let commitments = self.authority.commitments();
        let view = commitments
            .view(id)
            .ok_or(AuthorityError::UnknownCommitment)?;
        let result = if view.requires_approval {
            commitments
                .request_approval(id, agent, run, confirmer)
                .and_then(|approval| commitments.authorize(id, agent, run, Some(approval)))
        } else {
            commitments.authorize(id, agent, run, None)
        };
        if result.is_err() {
            self.drop_if_final(id);
        }
        result
    }

    /// Perform an authorized commitment, once.
    pub(crate) fn execute(
        &self,
        id: CommitmentId,
        agent: &AgentId,
        run: RunId,
    ) -> Result<EffectOutput, AuthorityError> {
        let effect = {
            let mut pending = self.pending();
            match pending.held.get(&id) {
                None => return Err(AuthorityError::UnknownCommitment),
                Some(held) if &held.agent != agent => return Err(AuthorityError::WrongAgent),
                Some(held) if held.run != run => return Err(AuthorityError::WrongRun),
                Some(_) => {}
            }
            pending.held.remove(&id).expect("present").effect
        };
        let target = match effect.revalidate() {
            Ok(target) => target,
            Err(error) => {
                // The target is gone or changed: the commitment fails unstarted.
                let _ = self
                    .authority
                    .commitments()
                    .fail_unstarted(id, agent, run, &error);
                return Err(error);
            }
        };
        let parameters = effect.parameters();
        let guard = match self
            .authority
            .commitments()
            .begin(id, agent, run, &target, &parameters)
        {
            Ok(guard) => guard,
            Err(error) => {
                // The effect has been taken out, so the commitment ends here
                // too (a no-op if the refusal already ended it).
                let _ = self
                    .authority
                    .commitments()
                    .fail_unstarted(id, agent, run, &error);
                return Err(error);
            }
        };
        let start = guard.start_seq();
        match effect.execute(&guard) {
            Ok(mut output) => {
                // It happened; a cancellation requested meanwhile is
                // recorded beside the outcome. Its result says what became
                // of its end record: a completed effect whose end is owed or
                // uncertain is reported so, never as plain success.
                let meta = output.meta.clone();
                let end = guard.finish(Outcome::Succeeded { meta });
                output.evidence = Some(ExecutionEvidence { start, end });
                Ok(output)
            }
            Err((class, detail)) => {
                let cancelled = guard.is_cancelled();
                // M2d (Codex review 1 of PR #64, H2): an effect that may have
                // happened is never reported as cancelled or as a plain
                // failure: it is uncertain, recovery required.
                let uncertain = class == FailureClass::Uncertain;
                // A cancelled effect keeps what its actuator said: how far
                // it got.
                let end = guard.finish(if cancelled && !uncertain {
                    Outcome::Cancelled {
                        detail: Some(detail),
                    }
                } else {
                    Outcome::Failed { class, detail }
                });
                Err(AuthorityError::Ended {
                    cause: Box::new(if uncertain {
                        AuthorityError::RecoveryRequired
                    } else if cancelled {
                        AuthorityError::RunCancelled
                    } else {
                        AuthorityError::Unavailable("the effect failed")
                    }),
                    evidence: ExecutionEvidence { start, end },
                })
            }
        }
    }

    /// Deny a pending commitment and drop its effect.
    pub(crate) fn deny(
        &self,
        id: CommitmentId,
        agent: &AgentId,
        run: RunId,
    ) -> Result<(), AuthorityError> {
        self.authority.commitments().deny(id, agent, run)?;
        let dropped = self.pending().held.remove(&id);
        drop(dropped);
        Ok(())
    }

    /// Cancel a run and drop every pending effect it had (outside the
    /// lock, as every dropped effect is).
    pub(crate) fn cancel_run(&self, run: RunId) -> Result<(), AuthorityError> {
        let result = self.authority.cancel_run(run);
        let dropped = self.pending().take_unless(|_, held| held.run != run);
        drop(dropped);
        result
    }

    /// The owner's emergency stop, its records written at once (tests; the
    /// front door also stops the agent display before any record).
    #[cfg(test)]
    pub(crate) fn emergency_stop(&self) -> usize {
        let stopped = self.stop_effects(None);
        let cancelled = stopped.cancelled;
        self.authority.write_stop_records(stopped);
        cancelled
    }

    /// The effects of an emergency stop, its records not yet written: every
    /// run cancelled, every unconsumed commitment ended, the generation
    /// moved, every pending effect dropped.
    pub(crate) fn close_ready(&self) -> bool {
        self.stop_metadata_known()
            && self
                .pending
                .lock()
                .is_ok_and(|p| p.held.is_empty() && p.reserved == 0 && p.reserved_by_run.is_empty())
    }

    pub(crate) fn stop_metadata_known(&self) -> bool {
        !self.pending.is_poisoned()
            && self.authority.runs().stop_metadata_known()
            && self.authority.commitments().stop_metadata_known()
    }

    pub(crate) fn stop_effects(&self, reason: Option<&'static str>) -> Stopped {
        let stopped = self.authority.stop_effects(reason);
        let dropped = self.pending().take_unless(|_, _| false);
        drop(dropped);
        stopped
    }

    /// A run is done: nothing more can be proposed for it, and what it left
    /// pending ends.
    pub(crate) fn finish_run(&self, run: RunId) {
        self.authority.finish_run(run);
        let dropped = self.pending().take_unless(|_, held| held.run != run);
        drop(dropped);
    }

    /// M2c PR-B, a door halt's last step: under the pending table alone,
    /// taken even if poisoned, take out every effect of `runs`; they are
    /// dropped once the lock is released.
    pub(crate) fn drop_pending_of(&self, runs: &std::collections::BTreeSet<RunId>) {
        let dropped = self
            .pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take_unless(|_, held| !runs.contains(&held.run));
        drop(dropped);
    }

    /// Places held for proposals being prepared (tests).
    #[cfg(test)]
    pub(crate) fn pending_places(&self) -> usize {
        self.pending().reserved
    }

    /// Effects held for `run` (tests).
    #[cfg(test)]
    pub(crate) fn pending_held(&self, run: RunId) -> usize {
        self.pending()
            .held
            .values()
            .filter(|held| held.run == run)
            .count()
    }

    /// Take the pending table's lock and panic while holding it (tests).
    #[cfg(test)]
    pub(crate) fn panic_holding_pending(&self) {
        let _table = self
            .pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        panic!("the pending table is poisoned on purpose (test)");
    }

    fn drop_if_final(&self, id: CommitmentId) {
        if !self.authority.commitments().is_unconsumed(id) {
            let dropped = self.pending().held.remove(&id);
            drop(dropped);
        }
    }

    /// End what can no longer start, and drop the effects of every
    /// commitment that has ended.
    pub(crate) fn prune(&self) {
        let commitments = self.authority.commitments();
        commitments.sweep();
        let dropped = self
            .pending()
            .take_unless(|id, _| commitments.is_unconsumed(*id));
        drop(dropped);
    }
}

#[cfg(test)]
mod tests;
