//! The exact durable wait publishes a target; its separately accepted Approve
//! root can be transferred once to the exclusively owned Mother controller.
use super::*;
use crate::authority::ids::CommitmentId;
use crate::authority::AuthorityError;
use crate::governed::Lookup;

/// Verified existing records for a retained native action. This is not work
/// authority: publication still checks the original request and owning holds.
pub struct PreparedWait {
    action: receipt::Action,
}

/// One-use ownership transfer of a completed approval's immutable request.
/// No constructor, Clone, serialization or arbitrary context setter exists.
pub struct ApprovalContinuation {
    core: Arc<Core>,
    context: SessionContext,
    target: u64,
    mission: u64,
    effect: u64,
    generation: u64,
}
impl ApprovalContinuation {
    pub fn mission(&self) -> u64 {
        self.mission
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn effect(&self) -> u64 {
        self.effect
    }
}

impl HostSession {
    pub fn approval_continuation(
        &self,
        request: &SessionRequest,
    ) -> Result<ApprovalContinuation, SessionError> {
        if !Arc::ptr_eq(&self.core, &request.core) || request.kind != CommandKind::Approve {
            return Err(SessionError::Restricted);
        }
        let mut pending = self.core.pending.lock().map_err(|_| {
            self.ingress().fail();
            SessionError::Restricted
        })?;
        let pending = pending
            .as_mut()
            .filter(|p| {
                p.identity == request.target
                    && p.approved
                        .as_ref()
                        .is_some_and(|scope| Arc::ptr_eq(scope, &request.scope))
                    && !p.continuation_taken
            })
            .ok_or(SessionError::NoTarget)?;
        let context = SessionContext {
            cell: self.core.cell.scoped(request.scope.clone()),
            scope: request.scope.clone(),
            run: Some((Arc::downgrade(&self.control), pending.run)),
        };
        let mission = pending.mission.ok_or(SessionError::NoTarget)?;
        let effect = pending.effect.ok_or(SessionError::NoTarget)?;
        let generation = pending.generation.ok_or(SessionError::NoTarget)?;
        context.admit(MotherStep::PortProjection)?;
        pending.continuation_taken = true;
        Ok(ApprovalContinuation {
            core: self.core.clone(),
            context,
            target: pending.identity,
            mission,
            effect,
            generation,
        })
    }
}
impl SessionDoor {
    pub fn prepare_wait(
        &self,
        mission: u64,
        effect: u64,
        id: CommitmentId,
    ) -> Result<PreparedWait, SessionFailure> {
        let action = {
            let actions = self.core.actions.lock().map_err(|_| {
                self.context.fail();
                AuthorityError::Halted
            })?;
            actions
                .iter()
                .find(|a| {
                    a.commitment == Some(id)
                        && a.session == self.core.identity
                        && a.door == self.door.origin()
                        && a.door_request == self.origin_request
                        && a.request == self.context.request()
                        && a.mission == Some(mission)
                        && a.effect == Some(effect)
                })
                .cloned()
                .ok_or(AuthorityError::NotMember)?
        };
        let Lookup::Mine(view) = self.door.commitment(id) else {
            return Err(AuthorityError::UnknownCommitment.into());
        };
        if view.run != action.run
            || view.state != crate::authority::commitment::CommitmentState::Prepared
        {
            return Err(AuthorityError::NotMember.into());
        }
        self.core
            .history
            .get()
            .ok_or(AuthorityError::EvidenceUnavailable)?
            .waiting(&action)?;
        Ok(PreparedWait { action })
    }

    /// Memory-only final publication under the Mother's current-generation hold.
    pub fn publish_wait(&self, wait: PreparedWait, generation: u64) -> Result<(), SessionFailure> {
        let a = wait.action;
        if a.session != self.core.identity
            || a.door != self.door.origin()
            || a.door_request != self.origin_request
            || a.request != self.context.request()
        {
            return Err(AuthorityError::NotMember.into());
        }
        let mut pending = self.core.pending.lock().map_err(|_| {
            self.context.fail();
            AuthorityError::Halted
        })?;
        let p = pending
            .as_mut()
            .filter(|p| {
                p.identity == a.preparation
                    && Some(p.commitment) == a.commitment
                    && p.run == a.run
                    && p.mission == a.mission
                    && p.effect == a.effect
                    && p.claimed.is_none()
                    && (p.generation.is_none()
                        || (p.generation == Some(generation)
                            && self.core.pending_context.load(Ordering::SeqCst) == p.identity))
            })
            .ok_or(AuthorityError::NotMember)?;
        self.door.admit_session_request(p.run, &self.context.cell)?;
        p.generation = Some(generation);
        self.core
            .pending_context
            .store(p.identity, Ordering::SeqCst);
        Ok(())
    }

    pub fn continue_approval(
        &self,
        continuation: ApprovalContinuation,
        mission: u64,
        generation: u64,
        effect: u64,
    ) -> Result<Self, SessionFailure> {
        if !Arc::ptr_eq(&self.core, &continuation.core)
            || continuation.mission != mission
            || continuation.generation != generation
            || continuation.effect != effect
        {
            return Err(AuthorityError::NotMember.into());
        }
        let pending = self.core.pending.lock().map_err(|_| {
            self.context.fail();
            AuthorityError::Halted
        })?;
        let p = pending
            .as_ref()
            .filter(|p| {
                p.identity == continuation.target
                    && p.continuation_taken
                    && p.approved
                        .as_ref()
                        .is_some_and(|scope| Arc::ptr_eq(scope, &continuation.context.scope))
            })
            .ok_or(AuthorityError::NotMember)?;
        self.door
            .admit_session_request(p.run, &continuation.context.cell)?;
        Ok(Self {
            door: self.door.clone(),
            context: continuation.context,
            origin_request: self.origin_request,
            core: self.core.clone(),
            kind: DoorKind::Continuation,
        })
    }

    /// The selected lifecycle receives a view of the same original deny-only
    /// atomic word, scoped to this already verified command. No cell is minted.
    pub fn lifecycle_halt(&self) -> HaltCell {
        self.context.cell.clone()
    }
}
