//! The action commitment: the one backend-owned record every governed
//! real-world effect passes through before it happens.
//!
//! States: `Prepared` → `Authorized` → `Executing` → `Succeeded` / `Failed`
//! / `Cancelled`, with the fail-closed exits `Denied`, `Expired` and
//! `Revoked`. A commitment binds the agent, the run, the capability kind,
//! the effect class, the operation, the canonical target identity, the
//! normalized parameter digest, the grants and credential leases it relies
//! on, the policy generation, its deadline and whether it needs native
//! approval; the binding digest covers all of them. It is created only from
//! a backend-prepared action, consumed exactly once by `begin` (after the
//! target is revalidated), and always finalized, on success and on failure.

use super::approval::{ActionConfirmation, ControlConfirmer, R2Approval};
use super::clock::Clock;
use super::effect::{CapabilityKind, EffectClass};
use super::evidence::{bounded, is_plain, EvidencePhase, EvidenceRecord, EvidenceSink};
use super::ids::{AgentId, CommitmentId, Digest, GrantId, LeaseId, RunId};
use super::policy::{GrantStore, PolicyGeneration};
use super::run::{CancelToken, RunRegistry};
use super::AuthorityError;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// The canonical identity of what an action acts on, as the domain resolved
/// it (a destination, a window, an executable). `display` is bounded text
/// for the owner and evidence; `digest` is what revalidation must match.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TargetIdentity {
    pub display: String,
    pub digest: Digest,
}

/// A backend-prepared action: what a domain actuator resolved from an
/// intent. Only the actuators build these; nothing deserializes one.
#[derive(Clone, Debug)]
pub struct PreparedAction {
    pub kind: CapabilityKind,
    pub class: EffectClass,
    pub operation: &'static str,
    pub target: TargetIdentity,
    /// The digest of the normalized parameters.
    pub parameters: Digest,
    /// Grants the action relies on; each must stay live until it starts.
    pub grants: Vec<GrantId>,
    /// Credential leases the action will use (references, never secrets).
    pub leases: Vec<LeaseId>,
    /// Bounded lines for the owner's confirmation (no secrets).
    pub summary: Vec<String>,
}

/// Where a commitment is in its life.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommitmentState {
    Prepared,
    Authorized,
    Executing,
    Succeeded,
    Failed,
    Cancelled,
    Denied,
    Expired,
    Revoked,
}

impl CommitmentState {
    pub fn as_str(self) -> &'static str {
        match self {
            CommitmentState::Prepared => "prepared",
            CommitmentState::Authorized => "authorized",
            CommitmentState::Executing => "executing",
            CommitmentState::Succeeded => "succeeded",
            CommitmentState::Failed => "failed",
            CommitmentState::Cancelled => "cancelled",
            CommitmentState::Denied => "denied",
            CommitmentState::Expired => "expired",
            CommitmentState::Revoked => "revoked",
        }
    }

    fn is_final(self) -> bool {
        !matches!(
            self,
            CommitmentState::Prepared | CommitmentState::Authorized | CommitmentState::Executing
        )
    }
}

/// Why a governed effect failed, as a class (detail stays bounded).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FailureClass {
    /// The actuator reported an error.
    Actuator,
    /// The target no longer matched at execution.
    TargetChanged,
    /// A deadline was reached.
    Timeout,
    /// A size, count or step bound was reached.
    Bounds,
    /// A transport (network, protocol) failure.
    Transport,
    /// The platform cannot perform it.
    Unavailable,
    /// The execution guard was dropped without a result.
    Abandoned,
    /// The authority refused to start it (not authorized, no longer live,
    /// or its start could not be recorded).
    Refused,
}

impl FailureClass {
    pub fn as_str(self) -> &'static str {
        match self {
            FailureClass::Actuator => "actuator",
            FailureClass::TargetChanged => "target_changed",
            FailureClass::Timeout => "timeout",
            FailureClass::Bounds => "bounds",
            FailureClass::Transport => "transport",
            FailureClass::Unavailable => "unavailable",
            FailureClass::Abandoned => "abandoned",
            FailureClass::Refused => "refused",
        }
    }
}

/// How an execution ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// It happened; `meta` is bounded, redacted result metadata.
    Succeeded {
        meta: Vec<(String, String)>,
    },
    Failed {
        class: FailureClass,
        detail: String,
    },
    Cancelled,
}

/// A commitment, for display (holds no authority).
#[derive(Clone, Debug)]
pub struct CommitmentView {
    pub id: CommitmentId,
    pub agent: AgentId,
    pub run: RunId,
    pub kind: CapabilityKind,
    pub class: EffectClass,
    pub operation: &'static str,
    pub target: String,
    pub summary: Vec<String>,
    pub state: CommitmentState,
    pub requires_approval: bool,
    pub created_wall_ms: u64,
    pub expires_wall_ms: u64,
    pub binding_short: String,
}

struct Entry {
    agent: AgentId,
    run: RunId,
    prepared: PreparedAction,
    generation: u64,
    deadline_ms: u64,
    created_wall_ms: u64,
    expires_wall_ms: u64,
    binding: Digest,
    state: CommitmentState,
    approval: Option<Digest>,
}

impl Entry {
    fn view(&self, id: CommitmentId) -> CommitmentView {
        CommitmentView {
            id,
            agent: self.agent.clone(),
            run: self.run,
            kind: self.prepared.kind,
            class: self.prepared.class,
            operation: self.prepared.operation,
            target: self.prepared.target.display.clone(),
            summary: self.prepared.summary.clone(),
            state: self.state,
            requires_approval: self.prepared.class.requires_native_approval(),
            created_wall_ms: self.created_wall_ms,
            expires_wall_ms: self.expires_wall_ms,
            binding_short: self.binding.short(),
        }
    }
}

/// Ends credential leases when their commitment ends (the broker).
pub trait LeaseEnd: Send + Sync {
    fn end(&self, lease: LeaseId);
}

struct NoLeases;
impl LeaseEnd for NoLeases {
    fn end(&self, _lease: LeaseId) {}
}

pub(crate) struct Inner {
    entries: Mutex<HashMap<CommitmentId, Entry>>,
    generation: Arc<PolicyGeneration>,
    grants: Arc<GrantStore>,
    runs: Arc<RunRegistry>,
    evidence: Arc<dyn EvidenceSink>,
    clock: Arc<dyn Clock>,
    leases: Mutex<Arc<dyn LeaseEnd>>,
}

/// The backend's commitments.
#[derive(Clone)]
pub struct CommitmentRegistry(Arc<Inner>);

/// The longest a commitment may wait to start.
pub const MAX_COMMITMENT_TTL: Duration = Duration::from_secs(15 * 60);
/// Most commitments kept (finished ones are pruned first).
const CAPACITY: usize = 4096;

fn binding_of(
    id: CommitmentId,
    agent: &AgentId,
    run: RunId,
    prepared: &PreparedAction,
    generation: u64,
    deadline_ms: u64,
) -> Digest {
    let mut grants: Vec<[u8; 16]> = prepared.grants.iter().map(|g| *g.as_bytes()).collect();
    grants.sort();
    let mut leases: Vec<[u8; 16]> = prepared.leases.iter().map(|l| *l.as_bytes()).collect();
    leases.sort();
    let grants: Vec<u8> = grants.concat();
    let leases: Vec<u8> = leases.concat();
    Digest::of(
        "nexus.p3.commitment.binding.v1",
        &[
            id.as_bytes(),
            agent.as_str().as_bytes(),
            run.as_bytes(),
            prepared.kind.as_str().as_bytes(),
            prepared.class.as_str().as_bytes(),
            prepared.operation.as_bytes(),
            prepared.target.digest.as_bytes(),
            prepared.parameters.as_bytes(),
            &grants,
            &leases,
            &generation.to_be_bytes(),
            &deadline_ms.to_be_bytes(),
        ],
    )
}

impl CommitmentRegistry {
    pub(crate) fn new(
        generation: Arc<PolicyGeneration>,
        grants: Arc<GrantStore>,
        runs: Arc<RunRegistry>,
        evidence: Arc<dyn EvidenceSink>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self(Arc::new(Inner {
            entries: Mutex::new(HashMap::new()),
            generation,
            grants,
            runs,
            evidence,
            clock,
            leases: Mutex::new(Arc::new(NoLeases)),
        }))
    }

    /// Install the credential broker's lease table.
    pub(crate) fn set_lease_end(&self, leases: Arc<dyn LeaseEnd>) {
        *self.0.leases.lock().expect("leases") = leases;
    }

    fn record(&self, record: &EvidenceRecord) -> Result<(), AuthorityError> {
        self.0
            .evidence
            .record(record)
            .map_err(|_| AuthorityError::EvidenceUnavailable)
    }

    fn base_record(&self, phase: EvidencePhase, id: CommitmentId, entry: &Entry) -> EvidenceRecord {
        let mut record =
            EvidenceRecord::new(phase, self.0.clock.wall_ms(), self.0.generation.current());
        record.commitment = Some(id.to_string());
        record.agent = Some(entry.agent.to_string());
        record.run = Some(entry.run.to_string());
        record.kind = Some(entry.prepared.kind);
        record.class = Some(entry.prepared.class);
        record.operation = Some(entry.prepared.operation);
        record.target = Some(bounded(&entry.prepared.target.display));
        record.target_digest = Some(entry.prepared.target.digest.to_hex());
        record.parameters_digest = Some(entry.prepared.parameters.to_hex());
        record.approval = entry.approval.map(|a| a.short());
        record
    }

    /// Record a commitment for `agent` in `run`. The run must be active and
    /// every grant the action relies on live. Nothing happens yet.
    pub(crate) fn prepare(
        &self,
        agent: &AgentId,
        run: RunId,
        prepared: PreparedAction,
        ttl: Duration,
    ) -> Result<CommitmentView, AuthorityError> {
        self.0.runs.check(run, agent)?;
        for grant in &prepared.grants {
            if self.0.grants.live(*grant).is_none() {
                return Err(AuthorityError::GrantNotLive);
            }
        }
        // What the owner may be shown must read as exactly what it is.
        if prepared.summary.len() > 12 {
            return Err(AuthorityError::InvalidAction("summary out of bounds"));
        }
        if !is_plain(&prepared.target.display) || !prepared.summary.iter().all(|l| is_plain(l)) {
            return Err(AuthorityError::InvalidAction("display text is not plain"));
        }
        let id = CommitmentId::fresh();
        let now = self.0.clock.monotonic_ms();
        let ttl = ttl.min(MAX_COMMITMENT_TTL);
        let ttl_ms = u64::try_from(ttl.as_millis()).unwrap_or(u64::MAX);
        let deadline_ms = now.saturating_add(ttl_ms);
        let generation = self.0.generation.current();
        let created_wall_ms = self.0.clock.wall_ms();
        let entry = Entry {
            agent: agent.clone(),
            run,
            binding: binding_of(id, agent, run, &prepared, generation, deadline_ms),
            prepared,
            generation,
            deadline_ms,
            created_wall_ms,
            expires_wall_ms: created_wall_ms.saturating_add(ttl_ms),
            state: CommitmentState::Prepared,
            approval: None,
        };
        // Evidence first: an unrecorded commitment is never created.
        let mut record = self.base_record(EvidencePhase::Prepared, id, &entry);
        for lease in entry.prepared.leases.iter().take(4) {
            record.detail.push(("lease".into(), lease.to_string()));
        }
        self.record(&record)?;
        let view = entry.view(id);
        let mut entries = self.0.entries.lock().expect("commitments");
        if entries.len() >= CAPACITY {
            entries.retain(|_, e| !e.state.is_final());
            if entries.len() >= CAPACITY {
                return Err(AuthorityError::Capacity);
            }
        }
        entries.insert(id, entry);
        Ok(view)
    }

    /// Check what every transition requires of a live entry: it belongs to
    /// `agent` and `run`, the run is active, it has not expired, the policy
    /// generation has not moved and its grants are still live. A failed
    /// check moves it to its final state (expired or revoked). Returns the
    /// run's cancel token.
    fn live_check(
        &self,
        id: CommitmentId,
        entry: &mut Entry,
        agent: &AgentId,
        run: RunId,
    ) -> Result<CancelToken, AuthorityError> {
        if &entry.agent != agent {
            return Err(AuthorityError::WrongAgent);
        }
        if entry.run != run {
            return Err(AuthorityError::WrongRun);
        }
        let token = match self.0.runs.check(run, agent) {
            Ok(token) => Some(token),
            Err(error) => {
                self.end_unconsumed(id, entry, CommitmentState::Revoked, &error);
                return Err(error);
            }
        };
        let failure = if self.0.clock.monotonic_ms() >= entry.deadline_ms {
            Some((CommitmentState::Expired, AuthorityError::Expired))
        } else if self.0.generation.current() != entry.generation {
            Some((CommitmentState::Revoked, AuthorityError::Stale))
        } else if entry
            .prepared
            .grants
            .iter()
            .any(|grant| self.0.grants.live(*grant).is_none())
        {
            Some((CommitmentState::Revoked, AuthorityError::GrantNotLive))
        } else {
            None
        };
        if let Some((state, error)) = failure {
            self.end_unconsumed(id, entry, state, &error);
            return Err(error);
        }
        Ok(token.expect("checked"))
    }

    /// Move an unconsumed entry to `Expired` or `Revoked`, evidenced, and
    /// release its leases.
    fn end_unconsumed(
        &self,
        id: CommitmentId,
        entry: &mut Entry,
        state: CommitmentState,
        reason: &AuthorityError,
    ) {
        entry.state = state;
        let phase = if state == CommitmentState::Expired {
            EvidencePhase::Expired
        } else {
            EvidencePhase::Revoked
        };
        let mut record = self.base_record(phase, id, entry);
        record.failure = Some(reason.class());
        record.cancelled = matches!(
            reason,
            AuthorityError::RunCancelled | AuthorityError::EmergencyStopped
        );
        let _ = self.record(&record);
        self.release_leases(entry);
    }

    fn release_leases(&self, entry: &Entry) {
        let leases = self.0.leases.lock().expect("leases").clone();
        for lease in &entry.prepared.leases {
            leases.end(*lease);
        }
    }

    /// Ask the owner, natively, to approve one R2 commitment. The dialog is
    /// shown outside every lock; the commitment is checked again afterwards,
    /// so one that changed, expired or was revoked meanwhile is not approved.
    pub(crate) fn request_approval(
        &self,
        id: CommitmentId,
        agent: &AgentId,
        run: RunId,
        confirmer: &dyn ControlConfirmer,
    ) -> Result<R2Approval, AuthorityError> {
        let (request, binding) = {
            let mut entries = self.0.entries.lock().expect("commitments");
            let entry = entries
                .get_mut(&id)
                .ok_or(AuthorityError::UnknownCommitment)?;
            if entry.state != CommitmentState::Prepared {
                return Err(AuthorityError::NotPending);
            }
            self.live_check(id, entry, agent, run)?;
            if !entry.prepared.class.requires_native_approval() {
                return Err(AuthorityError::ApprovalNotApplicable);
            }
            let remaining_ms = entry
                .deadline_ms
                .saturating_sub(self.0.clock.monotonic_ms());
            let request = ActionConfirmation {
                commitment: id.to_string(),
                class: entry.prepared.class,
                kind: entry.prepared.kind,
                operation: entry.prepared.operation.to_string(),
                target: bounded(&entry.prepared.target.display),
                agent: entry.agent.to_string(),
                run: entry.run.to_string(),
                summary: entry.prepared.summary.iter().map(|l| bounded(l)).collect(),
                expires_in_secs: remaining_ms / 1000,
                binding_short: entry.binding.short(),
            };
            (request, entry.binding)
        };
        let confirmed = confirmer.confirm_action(&request);
        let mut entries = self.0.entries.lock().expect("commitments");
        let entry = entries
            .get_mut(&id)
            .ok_or(AuthorityError::UnknownCommitment)?;
        if entry.state != CommitmentState::Prepared || entry.binding != binding {
            return Err(AuthorityError::NotPending);
        }
        self.live_check(id, entry, agent, run)?;
        if !confirmed {
            entry.state = CommitmentState::Denied;
            let record = self.base_record(EvidencePhase::ApprovalDeclined, id, entry);
            let _ = self.record(&record);
            self.release_leases(entry);
            return Err(AuthorityError::Declined);
        }
        let mut record = self.base_record(EvidencePhase::Approved, id, entry);
        record.approval = Some(binding.short());
        self.record(&record)?;
        Ok(R2Approval::confirmed(id, binding))
    }

    /// Authorize a prepared commitment. An R2 commitment needs the native
    /// approval of exactly this commitment (it is consumed); an R0 or R1
    /// commitment takes none.
    pub(crate) fn authorize(
        &self,
        id: CommitmentId,
        agent: &AgentId,
        run: RunId,
        approval: Option<R2Approval>,
    ) -> Result<(), AuthorityError> {
        let mut entries = self.0.entries.lock().expect("commitments");
        let entry = entries
            .get_mut(&id)
            .ok_or(AuthorityError::UnknownCommitment)?;
        if entry.state != CommitmentState::Prepared {
            return Err(AuthorityError::NotPending);
        }
        self.live_check(id, entry, agent, run)?;
        match (entry.prepared.class.requires_native_approval(), approval) {
            (true, None) => return Err(AuthorityError::ApprovalRequired),
            (false, Some(_)) => return Err(AuthorityError::ApprovalNotApplicable),
            (true, Some(approval)) => {
                if approval.commitment() != id || approval.binding() != &entry.binding {
                    return Err(AuthorityError::ApprovalMismatch);
                }
                entry.approval = Some(*approval.binding());
            }
            (false, None) => {}
        }
        entry.state = CommitmentState::Authorized;
        let record = self.base_record(EvidencePhase::Authorized, id, entry);
        self.record(&record)?;
        Ok(())
    }

    /// Consume an authorized commitment, exactly once, immediately before
    /// its effect. `revalidated` is the target identity the actuator has just
    /// resolved again and `parameters` the digest of the exact parameters it
    /// is about to use; both must equal what was committed.
    pub(crate) fn begin(
        &self,
        id: CommitmentId,
        agent: &AgentId,
        run: RunId,
        revalidated: &Digest,
        parameters: &Digest,
    ) -> Result<ExecutionGuard, AuthorityError> {
        let mut entries = self.0.entries.lock().expect("commitments");
        let entry = entries
            .get_mut(&id)
            .ok_or(AuthorityError::UnknownCommitment)?;
        if entry.state != CommitmentState::Authorized {
            // Say why it cannot start: a cancelled run or a stop is not a
            // missing authorization.
            return Err(match entry.state {
                CommitmentState::Revoked => self
                    .0
                    .runs
                    .check(entry.run, &entry.agent)
                    .err()
                    .unwrap_or(AuthorityError::Stale),
                CommitmentState::Expired => AuthorityError::Expired,
                _ => AuthorityError::NotAuthorized,
            });
        }
        let token = self.live_check(id, entry, agent, run)?;
        let changed = if &entry.prepared.target.digest != revalidated {
            Some(AuthorityError::TargetChanged)
        } else if &entry.prepared.parameters != parameters {
            Some(AuthorityError::ParametersChanged)
        } else {
            None
        };
        if let Some(error) = changed {
            entry.state = CommitmentState::Failed;
            let mut record = self.base_record(EvidencePhase::Finished, id, entry);
            record.outcome = Some("failed");
            record.failure = Some(FailureClass::TargetChanged.as_str());
            record.detail.push(("reason".into(), error.class().into()));
            let _ = self.record(&record);
            self.release_leases(entry);
            return Err(error);
        }
        let started = self.0.clock.wall_ms();
        let mut record = self.base_record(EvidencePhase::Started, id, entry);
        record.started_wall_ms = Some(started);
        // Evidence first: nothing starts unrecorded.
        self.record(&record)?;
        entry.state = CommitmentState::Executing;
        Ok(ExecutionGuard {
            id,
            registry: self.clone(),
            cancel: token,
            started_wall_ms: started,
            finished: false,
        })
    }

    fn finish(&self, id: CommitmentId, started_wall_ms: u64, outcome: Outcome, cancelled: bool) {
        let mut entries = self.0.entries.lock().expect("commitments");
        let Some(entry) = entries.get_mut(&id) else {
            return;
        };
        if entry.state != CommitmentState::Executing {
            return;
        }
        // The outcome is what happened: an effect that completed while its
        // run was being cancelled is recorded as completed, with the
        // cancellation beside it, never hidden as "cancelled".
        let (state, outcome_str, failure) = match &outcome {
            Outcome::Succeeded { .. } => (CommitmentState::Succeeded, "succeeded", None),
            Outcome::Failed { class, .. } => {
                (CommitmentState::Failed, "failed", Some(class.as_str()))
            }
            Outcome::Cancelled => (CommitmentState::Cancelled, "cancelled", None),
        };
        entry.state = state;
        let mut record = self.base_record(EvidencePhase::Finished, id, entry);
        record.started_wall_ms = Some(started_wall_ms);
        record.finished_wall_ms = Some(self.0.clock.wall_ms());
        record.outcome = Some(outcome_str);
        record.failure = failure;
        record.cancelled = cancelled || matches!(outcome, Outcome::Cancelled);
        match &outcome {
            Outcome::Succeeded { meta } => {
                record.detail = meta
                    .iter()
                    .take(16)
                    .map(|(k, v)| (bounded(k), bounded(v)))
                    .collect();
            }
            Outcome::Failed { detail, .. } => {
                record.detail.push(("detail".into(), bounded(detail)))
            }
            Outcome::Cancelled => {}
        }
        // A failure to record the end of an effect that already happened
        // cannot undo it; the start record stands either way.
        let _ = self.record(&record);
        self.release_leases(entry);
    }

    /// End a commitment that could not start: its target could not be
    /// revalidated immediately before the effect, or the authority refused
    /// to begin it. It fails without starting, evidenced.
    pub(crate) fn fail_unstarted(
        &self,
        id: CommitmentId,
        agent: &AgentId,
        run: RunId,
        reason: &AuthorityError,
    ) -> Result<(), AuthorityError> {
        let mut entries = self.0.entries.lock().expect("commitments");
        let entry = entries
            .get_mut(&id)
            .ok_or(AuthorityError::UnknownCommitment)?;
        if &entry.agent != agent {
            return Err(AuthorityError::WrongAgent);
        }
        if entry.run != run {
            return Err(AuthorityError::WrongRun);
        }
        if !matches!(
            entry.state,
            CommitmentState::Prepared | CommitmentState::Authorized
        ) {
            return Err(AuthorityError::NotPending);
        }
        let class = match reason {
            AuthorityError::TargetChanged | AuthorityError::ParametersChanged => {
                FailureClass::TargetChanged
            }
            AuthorityError::Unavailable(_) => FailureClass::Unavailable,
            _ => FailureClass::Refused,
        };
        entry.state = CommitmentState::Failed;
        let mut record = self.base_record(EvidencePhase::Finished, id, entry);
        record.outcome = Some("failed");
        record.failure = Some(class.as_str());
        record.detail.push(("reason".into(), reason.class().into()));
        let _ = self.record(&record);
        self.release_leases(entry);
        Ok(())
    }

    /// Deny a prepared or authorized commitment (owner or policy).
    pub(crate) fn deny(
        &self,
        id: CommitmentId,
        agent: &AgentId,
        run: RunId,
    ) -> Result<(), AuthorityError> {
        let mut entries = self.0.entries.lock().expect("commitments");
        let entry = entries
            .get_mut(&id)
            .ok_or(AuthorityError::UnknownCommitment)?;
        if &entry.agent != agent {
            return Err(AuthorityError::WrongAgent);
        }
        if entry.run != run {
            return Err(AuthorityError::WrongRun);
        }
        if !matches!(
            entry.state,
            CommitmentState::Prepared | CommitmentState::Authorized
        ) {
            return Err(AuthorityError::NotPending);
        }
        entry.state = CommitmentState::Denied;
        let record = self.base_record(EvidencePhase::Denied, id, entry);
        let _ = self.record(&record);
        self.release_leases(entry);
        Ok(())
    }

    /// End every unconsumed commitment of `run` (it was cancelled or it
    /// finished).
    pub(crate) fn revoke_run(&self, run: RunId, reason: &AuthorityError) {
        let mut entries = self.0.entries.lock().expect("commitments");
        for (id, entry) in entries.iter_mut() {
            if entry.run == run
                && matches!(
                    entry.state,
                    CommitmentState::Prepared | CommitmentState::Authorized
                )
            {
                self.end_unconsumed(*id, entry, CommitmentState::Revoked, reason);
            }
        }
    }

    /// End every unconsumed commitment (emergency stop).
    pub(crate) fn revoke_all(&self) {
        let mut entries = self.0.entries.lock().expect("commitments");
        for (id, entry) in entries.iter_mut() {
            if matches!(
                entry.state,
                CommitmentState::Prepared | CommitmentState::Authorized
            ) {
                self.end_unconsumed(
                    *id,
                    entry,
                    CommitmentState::Revoked,
                    &AuthorityError::EmergencyStopped,
                );
            }
        }
    }

    /// End every unconsumed commitment that can no longer start (it expired,
    /// its run ended, the policy generation moved or a grant it relies on is
    /// gone), so nothing dead waits as pending. Returns how many ended.
    pub fn sweep(&self) -> usize {
        let mut entries = self.0.entries.lock().expect("commitments");
        let mut ended = 0;
        for (id, entry) in entries.iter_mut() {
            if matches!(
                entry.state,
                CommitmentState::Prepared | CommitmentState::Authorized
            ) {
                let (agent, run) = (entry.agent.clone(), entry.run);
                if self.live_check(*id, entry, &agent, run).is_err() {
                    ended += 1;
                }
            }
        }
        ended
    }

    /// Whether a commitment can still be authorized or started (it is
    /// prepared or authorized; liveness is checked when it is used).
    pub fn is_unconsumed(&self, id: CommitmentId) -> bool {
        self.0
            .entries
            .lock()
            .expect("commitments")
            .get(&id)
            .is_some_and(|e| {
                matches!(
                    e.state,
                    CommitmentState::Prepared | CommitmentState::Authorized
                )
            })
    }

    pub fn view(&self, id: CommitmentId) -> Option<CommitmentView> {
        self.0
            .entries
            .lock()
            .expect("commitments")
            .get(&id)
            .map(|e| e.view(id))
    }

    pub fn views_of_run(&self, run: RunId) -> Vec<CommitmentView> {
        let mut views: Vec<CommitmentView> = self
            .0
            .entries
            .lock()
            .expect("commitments")
            .iter()
            .filter(|(_, e)| e.run == run)
            .map(|(id, e)| e.view(*id))
            .collect();
        views.sort_by_key(|v| v.created_wall_ms);
        views
    }

    /// The prepared action of a commitment that is executing under `guard`
    /// (actuators read what they committed to, nothing else).
    pub fn prepared_for(&self, guard: &ExecutionGuard) -> Option<PreparedAction> {
        self.0
            .entries
            .lock()
            .expect("commitments")
            .get(&guard.id)
            .filter(|e| e.state == CommitmentState::Executing)
            .map(|e| e.prepared.clone())
    }
}

/// The one-shot right to perform one committed effect. Not `Clone`; it ends
/// the commitment when finished, and as `Failed (abandoned)` if it is dropped
/// unfinished (a panic or an early return still finalizes).
pub struct ExecutionGuard {
    id: CommitmentId,
    registry: CommitmentRegistry,
    cancel: CancelToken,
    started_wall_ms: u64,
    finished: bool,
}

impl std::fmt::Debug for ExecutionGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ExecutionGuard({:?})", self.id)
    }
}

impl ExecutionGuard {
    pub fn commitment(&self) -> CommitmentId {
        self.id
    }

    pub fn cancel_token(&self) -> &CancelToken {
        &self.cancel
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel.is_cancelled()
    }

    /// End the commitment with `outcome`. Whether the run was cancelled
    /// during the effect is recorded beside the outcome.
    pub fn finish(mut self, outcome: Outcome) {
        self.finished = true;
        let cancelled = self.cancel.is_cancelled();
        self.registry
            .finish(self.id, self.started_wall_ms, outcome, cancelled);
    }
}

impl Drop for ExecutionGuard {
    fn drop(&mut self) {
        if !self.finished {
            self.registry.finish(
                self.id,
                self.started_wall_ms,
                Outcome::Failed {
                    class: FailureClass::Abandoned,
                    detail: "the execution ended without a result".into(),
                },
                self.cancel.is_cancelled(),
            );
        }
    }
}
