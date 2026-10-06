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
use crate::authority::ids::{AgentId, CommitmentId, Digest, RunId};
use crate::authority::{Authority, AuthorityError};
use std::collections::HashMap;
use std::sync::Mutex;
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
}

/// An effect prepared by a domain actuator, waiting for its commitment.
pub(crate) trait PendingEffect: Send {
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
    pending: Mutex<HashMap<CommitmentId, Pending>>,
}

/// Most pending effects held at once.
const MAX_PENDING: usize = 1024;
/// Most pending effects one run holds (R2 waiting for the owner, mostly).
const MAX_PENDING_PER_RUN: usize = 32;
/// Pending places only the owner's own commands may take, so that agents
/// filling the rest cannot block the owner.
const OWNER_RESERVE: usize = 64;

impl Control {
    pub(crate) fn new(authority: Authority) -> Self {
        Self {
            authority,
            pending: Mutex::new(HashMap::new()),
        }
    }

    pub(crate) fn authority(&self) -> &Authority {
        &self.authority
    }

    /// Take and release every lock of the pipeline once (tests).
    #[cfg(test)]
    pub(crate) fn probe_locks(&self) {
        drop(self.pending.lock().expect("pending"));
        self.authority.probe_locks();
    }

    /// Commit to a prepared effect for `agent` in `run`. Nothing happens yet.
    pub(crate) fn propose(
        &self,
        agent: &AgentId,
        run: RunId,
        preparation: Preparation,
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
        let limit = if *agent == AgentId::owner_session() {
            MAX_PENDING
        } else {
            MAX_PENDING - OWNER_RESERVE
        };
        let full = |pending: &HashMap<CommitmentId, Pending>| {
            pending.len() >= limit
                || pending.values().filter(|p| p.run == run).count() >= MAX_PENDING_PER_RUN
        };
        if full(&self.pending.lock().expect("pending")) {
            self.prune();
            if full(&self.pending.lock().expect("pending")) {
                return refuse(AuthorityError::Capacity);
            }
        }
        let view = self
            .authority
            .commitments()
            .prepare(agent, run, action, ttl)?;
        self.pending.lock().expect("pending").insert(
            view.id,
            Pending {
                agent: agent.clone(),
                run,
                effect,
            },
        );
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
            let mut pending = self.pending.lock().expect("pending");
            match pending.get(&id) {
                None => return Err(AuthorityError::UnknownCommitment),
                Some(held) if &held.agent != agent => return Err(AuthorityError::WrongAgent),
                Some(held) if held.run != run => return Err(AuthorityError::WrongRun),
                Some(_) => {}
            }
            pending.remove(&id).expect("present").effect
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
        match effect.execute(&guard) {
            Ok(output) => {
                // It happened; a cancellation requested meanwhile is
                // recorded beside the outcome.
                let meta = output.meta.clone();
                guard.finish(Outcome::Succeeded { meta });
                Ok(output)
            }
            Err((class, detail)) => {
                let cancelled = guard.is_cancelled();
                // A cancelled effect keeps what its actuator said: how far
                // it got.
                guard.finish(if cancelled {
                    Outcome::Cancelled {
                        detail: Some(detail),
                    }
                } else {
                    Outcome::Failed { class, detail }
                });
                Err(if cancelled {
                    AuthorityError::RunCancelled
                } else {
                    AuthorityError::Unavailable("the effect failed")
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
        self.pending.lock().expect("pending").remove(&id);
        Ok(())
    }

    /// Cancel a run and drop every pending effect it had.
    pub(crate) fn cancel_run(&self, run: RunId) -> Result<(), AuthorityError> {
        let result = self.authority.cancel_run(run);
        self.pending
            .lock()
            .expect("pending")
            .retain(|_, held| held.run != run);
        result
    }

    /// The owner's emergency stop.
    pub(crate) fn emergency_stop(&self) -> usize {
        let cancelled = self.authority.emergency_stop();
        self.pending.lock().expect("pending").clear();
        cancelled
    }

    /// A run is done: nothing more can be proposed for it, and what it left
    /// pending ends.
    pub(crate) fn finish_run(&self, run: RunId) {
        self.authority.finish_run(run);
        self.pending
            .lock()
            .expect("pending")
            .retain(|_, held| held.run != run);
    }

    fn drop_if_final(&self, id: CommitmentId) {
        if !self.authority.commitments().is_unconsumed(id) {
            self.pending.lock().expect("pending").remove(&id);
        }
    }

    /// End what can no longer start, and drop the effects of every
    /// commitment that has ended.
    pub(crate) fn prune(&self) {
        let commitments = self.authority.commitments();
        commitments.sweep();
        self.pending
            .lock()
            .expect("pending")
            .retain(|id, _| commitments.is_unconsumed(*id));
    }
}

#[cfg(test)]
mod tests;
