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
//!
//! No lock is held while evidence is recorded, a lease is ended or a clock
//! is read. A transition that must be recorded before it takes effect
//! (approval, authorization, start) first reserves the commitment under the
//! lock: the reservation makes nothing approvable, authorizable or
//! executable. The record is written with the lock released; then, under
//! the lock again, the transition completes only if the same reservation
//! still holds the same commitment, in the same state, still live (run,
//! deadline, policy generation, grants). A cancellation, revocation, denial
//! or expiry that came meanwhile wins and withdraws the reservation, which
//! can never be completed later; its record says which transition it
//! interrupted. A record that fails withdraws the reservation: nothing
//! takes effect. A transition that ends a commitment takes effect under the
//! lock at once; its evidence and lease ends follow once the lock is
//! released.

use super::approval::{ActionConfirmation, ControlConfirmer, R2Approval};
use super::clock::Clock;
use super::effect::{CapabilityKind, EffectClass};
use super::evidence::{bounded, is_plain, EvidencePhase, EvidenceRecord, EvidenceSink};
use super::ids::{AgentId, CommitmentId, Digest, GrantId, LeaseId, RunId};
use super::policy::{GrantStore, PolicyGeneration};
use super::run::{CancelToken, RunRegistry};
use super::AuthorityError;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
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

    pub(crate) fn is_final(self) -> bool {
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
    /// or its start could not be recorded), or to let it take its next step
    /// (its grant was revoked or expired).
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
    /// The run was cancelled during the effect; `detail` says how far the
    /// effect got, when the actuator said.
    Cancelled {
        detail: Option<String>,
    },
}

/// The most summary lines an action may show; one that needs more is
/// refused rather than shortened.
pub const MAX_SUMMARY_LINES: usize = 192;

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
    /// The parameters digest salted with a nonce kept only in memory: what
    /// the binding and the evidence carry, so that no record lets
    /// low-entropy content (typed text, a short message) be recovered by
    /// guessing.
    salted: Digest,
    binding: Digest,
    state: CommitmentState,
    approval: Option<Digest>,
    /// An evidence-first transition under way (its record is being
    /// written): while it is, no other transition starts, and it completes
    /// only with its own token.
    reserved: Option<Reservation>,
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

    fn reserved_by(&self, token: u64) -> bool {
        self.reserved.is_some_and(|r| r.token == token)
    }
}

/// The evidence-first transitions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Transition {
    Approve,
    Authorize,
    Start,
}

impl Transition {
    fn as_str(self) -> &'static str {
        match self {
            Transition::Approve => "approval",
            Transition::Authorize => "authorization",
            Transition::Start => "start",
        }
    }
}

/// A transition under way, identified by a fresh token.
#[derive(Clone, Copy, Debug)]
struct Reservation {
    token: u64,
    transition: Transition,
}

/// The clocks, read before a lock is taken.
#[derive(Clone, Copy)]
struct Now {
    monotonic_ms: u64,
    wall_ms: u64,
}

/// What transitions made under the lock still owe once it is released:
/// their evidence, and the ends of their credential leases.
#[derive(Default)]
struct Deferred {
    records: Vec<EvidenceRecord>,
    leases: Vec<LeaseId>,
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
    /// Issues the reservation tokens.
    transitions: AtomicU64,
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
    salted: &Digest,
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
            salted.as_bytes(),
            &grants,
            &leases,
            &generation.to_be_bytes(),
            &deadline_ms.to_be_bytes(),
        ],
    )
}

/// A reservation held while its record is written. Dropped (the record
/// failed, the sink panicked, or the transition completed or was
/// overtaken), it is withdrawn if it still holds the commitment: the
/// commitment stays as it was before the transition.
struct Reserved<'a> {
    registry: &'a CommitmentRegistry,
    id: CommitmentId,
    token: u64,
}

impl Drop for Reserved<'_> {
    fn drop(&mut self) {
        // Never under a lock of this thread: no lock is held across a record.
        if let Ok(mut entries) = self.registry.0.entries.lock() {
            if let Some(entry) = entries.get_mut(&self.id) {
                if entry.reserved_by(self.token) {
                    entry.reserved = None;
                }
            }
        }
    }
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
            transitions: AtomicU64::new(0),
        }))
    }

    /// Take and release this registry's locks once (tests).
    #[cfg(test)]
    pub(crate) fn probe_locks(&self) {
        drop(self.0.entries.lock().expect("commitments"));
        drop(self.0.leases.lock().expect("leases"));
    }

    /// Install the credential broker's lease table.
    pub(crate) fn set_lease_end(&self, leases: Arc<dyn LeaseEnd>) {
        *self.0.leases.lock().expect("leases") = leases;
    }

    fn entries(&self) -> MutexGuard<'_, HashMap<CommitmentId, Entry>> {
        self.0.entries.lock().expect("commitments")
    }

    fn now(&self) -> Now {
        Now {
            monotonic_ms: self.0.clock.monotonic_ms(),
            wall_ms: self.0.clock.wall_ms(),
        }
    }

    /// Write one record. Never called with a lock held.
    fn record(&self, record: &EvidenceRecord) -> Result<(), AuthorityError> {
        self.0
            .evidence
            .record(record)
            .map_err(|_| AuthorityError::EvidenceUnavailable)
    }

    /// Pay what transitions made under the lock owe, once it is released:
    /// the credential leases end first (whatever the sink does), then the
    /// records are written.
    fn flush(&self, deferred: Deferred) {
        self.end_leases(&deferred.leases);
        for record in &deferred.records {
            // The transition has taken effect (an end, which fails closed):
            // a record that cannot be written does not undo it.
            let _ = self.record(record);
        }
    }

    fn base_record(
        &self,
        phase: EvidencePhase,
        id: CommitmentId,
        entry: &Entry,
        now: Now,
    ) -> EvidenceRecord {
        let mut record = EvidenceRecord::new(phase, now.wall_ms, self.0.generation.current());
        record.commitment = Some(id.to_string());
        record.agent = Some(entry.agent.to_string());
        record.run = Some(entry.run.to_string());
        record.kind = Some(entry.prepared.kind);
        record.class = Some(entry.prepared.class);
        record.operation = Some(entry.prepared.operation);
        record.target = Some(bounded(&entry.prepared.target.display));
        record.target_digest = Some(entry.prepared.target.digest.to_hex());
        record.parameters_digest = Some(entry.salted.to_hex());
        record.approval = entry.approval.map(|a| a.short());
        record
    }

    /// Reserve `entry` for `transition` (it has none under way).
    fn reserve(&self, entry: &mut Entry, transition: Transition) -> u64 {
        let token = self.0.transitions.fetch_add(1, Ordering::SeqCst) + 1;
        entry.reserved = Some(Reservation { token, transition });
        token
    }

    /// Why a commitment no longer takes the transition it was reserved, or
    /// asked, for: a cancelled run or a stop is not a missing authorization.
    fn ended(&self, entry: &Entry, otherwise: AuthorityError) -> AuthorityError {
        match entry.state {
            CommitmentState::Revoked => self
                .0
                .runs
                .check(entry.run, &entry.agent)
                .err()
                .unwrap_or(AuthorityError::Stale),
            CommitmentState::Expired => AuthorityError::Expired,
            _ => otherwise,
        }
    }

    /// The second half of an evidence-first transition, under the lock
    /// again: it completes (`finalize`) only if `token` still reserves the
    /// commitment, in `expected` state, and the commitment is still live.
    /// Whatever ended it meanwhile wins.
    fn complete<T>(
        &self,
        id: CommitmentId,
        token: u64,
        expected: CommitmentState,
        agent: &AgentId,
        run: RunId,
        finalize: impl FnOnce(&mut Entry, CancelToken) -> T,
    ) -> Result<T, AuthorityError> {
        let mut deferred = Deferred::default();
        let now = self.now();
        let completed = {
            let mut entries = self.entries();
            match entries.get_mut(&id) {
                None => Err(AuthorityError::UnknownCommitment),
                Some(entry) if !entry.reserved_by(token) || entry.state != expected => {
                    Err(self.ended(entry, AuthorityError::NotPending))
                }
                Some(entry) => {
                    // Checked while still reserved: an end it finds records
                    // the transition it interrupted.
                    let live = self.live_check(id, entry, agent, run, now, &mut deferred);
                    entry.reserved = None;
                    live.map(|cancel| finalize(entry, cancel))
                }
            }
        };
        self.flush(deferred);
        completed
    }

    /// Move an entry that has not finished to the final `state`, under the
    /// lock: a transition under way for it is withdrawn (its record says
    /// which), and its evidence (`adjust` completes it) and lease ends are
    /// owed.
    #[allow(clippy::too_many_arguments)]
    fn end_entry(
        &self,
        id: CommitmentId,
        entry: &mut Entry,
        state: CommitmentState,
        phase: EvidencePhase,
        now: Now,
        deferred: &mut Deferred,
        adjust: impl FnOnce(&mut EvidenceRecord),
    ) {
        entry.state = state;
        let interrupted = entry.reserved.take();
        let mut record = self.base_record(phase, id, entry, now);
        if let Some(reservation) = interrupted {
            record
                .detail
                .push(("interrupted".into(), reservation.transition.as_str().into()));
        }
        adjust(&mut record);
        deferred.records.push(record);
        deferred
            .leases
            .extend(entry.prepared.leases.iter().copied());
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
        let leases = prepared.leases.clone();
        let result = self.prepare_leased(agent, run, prepared, ttl);
        if result.is_err() {
            // A refused preparation leaves no credential lease behind.
            self.end_leases(&leases);
        }
        result
    }

    fn prepare_leased(
        &self,
        agent: &AgentId,
        run: RunId,
        prepared: PreparedAction,
        ttl: Duration,
    ) -> Result<CommitmentView, AuthorityError> {
        self.0.runs.check(run, agent)?;
        // Every commitment rests on at least one live grant of its own kind.
        if prepared.grants.is_empty() {
            return Err(AuthorityError::NoCoveringGrant);
        }
        for grant in &prepared.grants {
            match self.0.grants.live(*grant) {
                None => return Err(AuthorityError::GrantNotLive),
                Some(live) if live.scope.kind() != prepared.kind => {
                    return Err(AuthorityError::NoCoveringGrant)
                }
                Some(_) => {}
            }
        }
        // What the owner may be shown must read as exactly what it is, in
        // full: an action that cannot be shown whole is refused, never cut.
        if prepared.summary.len() > MAX_SUMMARY_LINES {
            return Err(AuthorityError::InvalidAction(
                "too much to show the owner in full",
            ));
        }
        if !is_plain(&prepared.target.display) || !prepared.summary.iter().all(|l| is_plain(l)) {
            return Err(AuthorityError::InvalidAction("display text is not plain"));
        }
        let id = CommitmentId::fresh();
        let now = self.now();
        let ttl = ttl.min(MAX_COMMITMENT_TTL);
        let ttl_ms = u64::try_from(ttl.as_millis()).unwrap_or(u64::MAX);
        let deadline_ms = now.monotonic_ms.saturating_add(ttl_ms);
        let generation = self.0.generation.current();
        let created_wall_ms = now.wall_ms;
        let mut nonce = [0u8; 16];
        getrandom::getrandom(&mut nonce)
            .map_err(|_| AuthorityError::Unavailable("no randomness for a commitment"))?;
        let salted = Digest::of(
            "nexus.p3.commitment.parameters.v1",
            &[&nonce, prepared.parameters.as_bytes()],
        );
        let entry = Entry {
            agent: agent.clone(),
            run,
            binding: binding_of(id, agent, run, &prepared, &salted, generation, deadline_ms),
            salted,
            prepared,
            generation,
            deadline_ms,
            created_wall_ms,
            expires_wall_ms: created_wall_ms.saturating_add(ttl_ms),
            state: CommitmentState::Prepared,
            approval: None,
            reserved: None,
        };
        // Evidence first: an unrecorded commitment is never created. It is
        // not in the registry yet, so nothing can act on it meanwhile.
        let mut record = self.base_record(EvidencePhase::Prepared, id, &entry, now);
        for lease in entry.prepared.leases.iter().take(4) {
            record.detail.push(("lease".into(), lease.to_string()));
        }
        self.record(&record)?;
        let view = entry.view(id);
        let mut deferred = Deferred::default();
        let now = self.now();
        let refused = {
            let mut entries = self.entries();
            if entries.len() >= CAPACITY {
                entries.retain(|_, e| !e.state.is_final());
            }
            // The run is checked again under the lock that cancelling and
            // stopping take, so a run ended meanwhile gets no live
            // commitment: its recorded preparation ends as revoked.
            let refused = if entries.len() >= CAPACITY {
                Some(AuthorityError::Capacity)
            } else {
                self.0.runs.check(run, agent).err()
            };
            match refused {
                Some(error) => {
                    let mut entry = entry;
                    self.end_unconsumed(
                        id,
                        &mut entry,
                        CommitmentState::Revoked,
                        &error,
                        now,
                        &mut deferred,
                    );
                    Some(error)
                }
                None => {
                    entries.insert(id, entry);
                    None
                }
            }
        };
        self.flush(deferred);
        match refused {
            Some(error) => Err(error),
            None => Ok(view),
        }
    }

    /// Check what every transition requires of a live entry: it belongs to
    /// `agent` and `run`, the run is active, it has not expired, the policy
    /// generation has not moved and its grants are still live. A failed
    /// check moves it to its final state (expired or revoked), its evidence
    /// owed in `deferred`. Returns the run's cancel token.
    fn live_check(
        &self,
        id: CommitmentId,
        entry: &mut Entry,
        agent: &AgentId,
        run: RunId,
        now: Now,
        deferred: &mut Deferred,
    ) -> Result<CancelToken, AuthorityError> {
        if &entry.agent != agent {
            return Err(AuthorityError::WrongAgent);
        }
        if entry.run != run {
            return Err(AuthorityError::WrongRun);
        }
        let token = match self.0.runs.check(run, agent) {
            Ok(token) => token,
            Err(error) => {
                self.end_unconsumed(id, entry, CommitmentState::Revoked, &error, now, deferred);
                return Err(error);
            }
        };
        // Either clock ends it: the monotonic one does not count the time
        // the machine is suspended, the wall clock does.
        let failure =
            if now.monotonic_ms >= entry.deadline_ms || now.wall_ms >= entry.expires_wall_ms {
                Some((CommitmentState::Expired, AuthorityError::Expired))
            } else if self.0.generation.current() != entry.generation {
                Some((CommitmentState::Revoked, AuthorityError::Stale))
            } else if entry.prepared.grants.iter().any(|grant| {
                self.0
                    .grants
                    .live_at(*grant, now.monotonic_ms, now.wall_ms)
                    .is_none()
            }) {
                Some((CommitmentState::Revoked, AuthorityError::GrantNotLive))
            } else {
                None
            };
        if let Some((state, error)) = failure {
            self.end_unconsumed(id, entry, state, &error, now, deferred);
            return Err(error);
        }
        Ok(token)
    }

    /// Move an unconsumed entry to `Expired` or `Revoked`; its evidence and
    /// lease ends are owed in `deferred`.
    fn end_unconsumed(
        &self,
        id: CommitmentId,
        entry: &mut Entry,
        state: CommitmentState,
        reason: &AuthorityError,
        now: Now,
        deferred: &mut Deferred,
    ) {
        let phase = if state == CommitmentState::Expired {
            EvidencePhase::Expired
        } else {
            EvidencePhase::Revoked
        };
        self.end_entry(id, entry, state, phase, now, deferred, |record| {
            record.failure = Some(reason.class());
            record.cancelled = matches!(
                reason,
                AuthorityError::RunCancelled | AuthorityError::EmergencyStopped
            );
        });
    }

    /// End leases no commitment will list (their proposal was refused), or
    /// whose commitment ended. Never called with a lock held.
    pub(crate) fn end_leases(&self, leases: &[LeaseId]) {
        if leases.is_empty() {
            return;
        }
        let table = self.0.leases.lock().expect("leases").clone();
        for lease in leases {
            table.end(*lease);
        }
    }

    /// Ask the owner, natively, to approve one R2 commitment. The dialog is
    /// shown outside every lock; the commitment is checked again afterwards,
    /// so one that changed, expired or was revoked meanwhile is not approved.
    /// The approval is recorded before it exists (evidence first, under a
    /// reservation).
    pub(crate) fn request_approval(
        &self,
        id: CommitmentId,
        agent: &AgentId,
        run: RunId,
        confirmer: &dyn ControlConfirmer,
    ) -> Result<R2Approval, AuthorityError> {
        let mut deferred = Deferred::default();
        let now = self.now();
        let asked = {
            let mut entries = self.entries();
            match entries.get_mut(&id) {
                None => Err(AuthorityError::UnknownCommitment),
                Some(entry) => self.approval_request(id, entry, agent, run, now, &mut deferred),
            }
        };
        self.flush(deferred);
        let (request, binding) = asked?;
        let confirmed = confirmer.confirm_action(&request);
        let mut deferred = Deferred::default();
        let now = self.now();
        let answered = {
            let mut entries = self.entries();
            match entries.get_mut(&id) {
                None => Err(AuthorityError::UnknownCommitment),
                Some(entry)
                    if entry.state != CommitmentState::Prepared
                        || entry.binding != binding
                        || entry.reserved.is_some() =>
                {
                    Err(AuthorityError::NotPending)
                }
                Some(entry) => self
                    .live_check(id, entry, agent, run, now, &mut deferred)
                    .and_then(|_| {
                        if !confirmed {
                            self.end_entry(
                                id,
                                entry,
                                CommitmentState::Denied,
                                EvidencePhase::ApprovalDeclined,
                                now,
                                &mut deferred,
                                |_| {},
                            );
                            return Err(AuthorityError::Declined);
                        }
                        let token = self.reserve(entry, Transition::Approve);
                        let mut record = self.base_record(EvidencePhase::Approved, id, entry, now);
                        record.approval = Some(binding.short());
                        Ok((token, record))
                    }),
            }
        };
        self.flush(deferred);
        let (token, record) = answered?;
        let _reserved = Reserved {
            registry: self,
            id,
            token,
        };
        // Evidence first: an unrecorded approval is never given.
        self.record(&record)?;
        self.complete(id, token, CommitmentState::Prepared, agent, run, |_, _| ())?;
        Ok(R2Approval::confirmed(id, binding))
    }

    /// What the owner is asked to approve, read under the lock.
    fn approval_request(
        &self,
        id: CommitmentId,
        entry: &mut Entry,
        agent: &AgentId,
        run: RunId,
        now: Now,
        deferred: &mut Deferred,
    ) -> Result<(ActionConfirmation, Digest), AuthorityError> {
        if entry.state != CommitmentState::Prepared || entry.reserved.is_some() {
            return Err(AuthorityError::NotPending);
        }
        self.live_check(id, entry, agent, run, now, deferred)?;
        if !entry.prepared.class.requires_native_approval() {
            return Err(AuthorityError::ApprovalNotApplicable);
        }
        let remaining_ms = entry.deadline_ms.saturating_sub(now.monotonic_ms);
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
        Ok((request, entry.binding))
    }

    /// Authorize a prepared commitment. An R2 commitment needs the native
    /// approval of exactly this commitment (it is consumed); an R0 or R1
    /// commitment takes none. Evidence first: it is authorized only once
    /// that is recorded, under a reservation.
    pub(crate) fn authorize(
        &self,
        id: CommitmentId,
        agent: &AgentId,
        run: RunId,
        approval: Option<R2Approval>,
    ) -> Result<(), AuthorityError> {
        let mut deferred = Deferred::default();
        let now = self.now();
        let reserved = {
            let mut entries = self.entries();
            match entries.get_mut(&id) {
                None => Err(AuthorityError::UnknownCommitment),
                Some(entry) => {
                    self.authorization(id, entry, agent, run, approval, now, &mut deferred)
                }
            }
        };
        self.flush(deferred);
        let (token, approved, record) = reserved?;
        let _reserved = Reserved {
            registry: self,
            id,
            token,
        };
        self.record(&record)?;
        self.complete(
            id,
            token,
            CommitmentState::Prepared,
            agent,
            run,
            |entry, _| {
                entry.approval = approved.or(entry.approval);
                entry.state = CommitmentState::Authorized;
            },
        )
    }

    /// The first half of `authorize`, under the lock: validate and reserve.
    #[allow(clippy::too_many_arguments)]
    fn authorization(
        &self,
        id: CommitmentId,
        entry: &mut Entry,
        agent: &AgentId,
        run: RunId,
        approval: Option<R2Approval>,
        now: Now,
        deferred: &mut Deferred,
    ) -> Result<(u64, Option<Digest>, EvidenceRecord), AuthorityError> {
        if entry.state != CommitmentState::Prepared || entry.reserved.is_some() {
            return Err(AuthorityError::NotPending);
        }
        self.live_check(id, entry, agent, run, now, deferred)?;
        let approved = match (entry.prepared.class.requires_native_approval(), approval) {
            (true, None) => return Err(AuthorityError::ApprovalRequired),
            (false, Some(_)) => return Err(AuthorityError::ApprovalNotApplicable),
            (true, Some(approval)) => {
                if approval.commitment() != id || approval.binding() != &entry.binding {
                    return Err(AuthorityError::ApprovalMismatch);
                }
                Some(*approval.binding())
            }
            (false, None) => None,
        };
        let token = self.reserve(entry, Transition::Authorize);
        let mut record = self.base_record(EvidencePhase::Authorized, id, entry, now);
        record.approval = approved.or(entry.approval).map(|a| a.short());
        Ok((token, approved, record))
    }

    /// Consume an authorized commitment, exactly once, immediately before
    /// its effect. `revalidated` is the target identity the actuator has just
    /// resolved again and `parameters` the digest of the exact parameters it
    /// is about to use; both must equal what was committed. Evidence first:
    /// it starts only once that is recorded, under a reservation.
    pub(crate) fn begin(
        &self,
        id: CommitmentId,
        agent: &AgentId,
        run: RunId,
        revalidated: &Digest,
        parameters: &Digest,
    ) -> Result<ExecutionGuard, AuthorityError> {
        let mut deferred = Deferred::default();
        let now = self.now();
        let reserved = {
            let mut entries = self.entries();
            match entries.get_mut(&id) {
                None => Err(AuthorityError::UnknownCommitment),
                Some(entry) => self.start(
                    id,
                    entry,
                    agent,
                    run,
                    (revalidated, parameters),
                    now,
                    &mut deferred,
                ),
            }
        };
        self.flush(deferred);
        let (token, record) = reserved?;
        let _reserved = Reserved {
            registry: self,
            id,
            token,
        };
        let started_wall_ms = now.wall_ms;
        self.record(&record)?;
        let cancel = self.complete(
            id,
            token,
            CommitmentState::Authorized,
            agent,
            run,
            |entry, cancel| {
                entry.state = CommitmentState::Executing;
                cancel
            },
        )?;
        Ok(ExecutionGuard {
            id,
            registry: self.clone(),
            cancel,
            started_wall_ms,
            finished: false,
        })
    }

    /// The first half of `begin`, under the lock: validate and reserve.
    #[allow(clippy::too_many_arguments)]
    fn start(
        &self,
        id: CommitmentId,
        entry: &mut Entry,
        agent: &AgentId,
        run: RunId,
        (revalidated, parameters): (&Digest, &Digest),
        now: Now,
        deferred: &mut Deferred,
    ) -> Result<(u64, EvidenceRecord), AuthorityError> {
        if entry.state != CommitmentState::Authorized || entry.reserved.is_some() {
            // Say why it cannot start: a cancelled run or a stop is not a
            // missing authorization.
            return Err(self.ended(entry, AuthorityError::NotAuthorized));
        }
        self.live_check(id, entry, agent, run, now, deferred)?;
        let changed = if &entry.prepared.target.digest != revalidated {
            Some(AuthorityError::TargetChanged)
        } else if &entry.prepared.parameters != parameters {
            Some(AuthorityError::ParametersChanged)
        } else {
            None
        };
        if let Some(error) = changed {
            self.end_entry(
                id,
                entry,
                CommitmentState::Failed,
                EvidencePhase::Finished,
                now,
                deferred,
                |record| {
                    record.outcome = Some("failed");
                    record.failure = Some(FailureClass::TargetChanged.as_str());
                    record.detail.push(("reason".into(), error.class().into()));
                },
            );
            return Err(error);
        }
        let token = self.reserve(entry, Transition::Start);
        let mut record = self.base_record(EvidencePhase::Started, id, entry, now);
        record.started_wall_ms = Some(now.wall_ms);
        Ok((token, record))
    }

    fn finish(&self, id: CommitmentId, started_wall_ms: u64, outcome: Outcome, cancelled: bool) {
        let mut deferred = Deferred::default();
        let now = self.now();
        {
            let mut entries = self.entries();
            let Some(entry) = entries.get_mut(&id) else {
                return;
            };
            if entry.state != CommitmentState::Executing {
                return;
            }
            // The outcome is what happened: an effect that completed while
            // its run was being cancelled is recorded as completed, with the
            // cancellation beside it, never hidden as "cancelled".
            let (state, outcome_str, failure) = match &outcome {
                Outcome::Succeeded { .. } => (CommitmentState::Succeeded, "succeeded", None),
                Outcome::Failed { class, .. } => {
                    (CommitmentState::Failed, "failed", Some(class.as_str()))
                }
                Outcome::Cancelled { .. } => (CommitmentState::Cancelled, "cancelled", None),
            };
            self.end_entry(
                id,
                entry,
                state,
                EvidencePhase::Finished,
                now,
                &mut deferred,
                |record| {
                    record.started_wall_ms = Some(started_wall_ms);
                    record.finished_wall_ms = Some(now.wall_ms);
                    record.outcome = Some(outcome_str);
                    record.failure = failure;
                    record.cancelled = cancelled || matches!(outcome, Outcome::Cancelled { .. });
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
                        Outcome::Cancelled { detail } => {
                            if let Some(detail) = detail {
                                record.detail.push(("detail".into(), bounded(detail)))
                            }
                        }
                    }
                },
            );
        }
        // A failure to record the end of an effect that already happened
        // cannot undo it; the start record stands either way.
        self.flush(deferred);
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
        let mut deferred = Deferred::default();
        let now = self.now();
        {
            let mut entries = self.entries();
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
            self.end_entry(
                id,
                entry,
                CommitmentState::Failed,
                EvidencePhase::Finished,
                now,
                &mut deferred,
                |record| {
                    record.outcome = Some("failed");
                    record.failure = Some(class.as_str());
                    record.detail.push(("reason".into(), reason.class().into()));
                },
            );
        }
        self.flush(deferred);
        Ok(())
    }

    /// Deny a prepared or authorized commitment (owner or policy). A
    /// transition under way for it is overtaken.
    pub(crate) fn deny(
        &self,
        id: CommitmentId,
        agent: &AgentId,
        run: RunId,
    ) -> Result<(), AuthorityError> {
        let mut deferred = Deferred::default();
        let now = self.now();
        {
            let mut entries = self.entries();
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
            self.end_entry(
                id,
                entry,
                CommitmentState::Denied,
                EvidencePhase::Denied,
                now,
                &mut deferred,
                |_| {},
            );
        }
        self.flush(deferred);
        Ok(())
    }

    /// End every unconsumed commitment of `run` (it was cancelled or it
    /// finished).
    pub(crate) fn revoke_run(&self, run: RunId, reason: &AuthorityError) {
        let mut deferred = Deferred::default();
        let now = self.now();
        {
            let mut entries = self.entries();
            for (id, entry) in entries.iter_mut() {
                if entry.run == run
                    && matches!(
                        entry.state,
                        CommitmentState::Prepared | CommitmentState::Authorized
                    )
                {
                    self.end_unconsumed(
                        *id,
                        entry,
                        CommitmentState::Revoked,
                        reason,
                        now,
                        &mut deferred,
                    );
                }
            }
        }
        self.flush(deferred);
    }

    /// End every unconsumed commitment (emergency stop).
    pub(crate) fn revoke_all(&self) {
        let mut deferred = Deferred::default();
        let now = self.now();
        {
            let mut entries = self.entries();
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
                        now,
                        &mut deferred,
                    );
                }
            }
        }
        self.flush(deferred);
    }

    /// End every unconsumed commitment that can no longer start (it expired,
    /// its run ended, the policy generation moved or a grant it relies on is
    /// gone), so nothing dead waits as pending. Returns how many ended.
    pub fn sweep(&self) -> usize {
        let mut deferred = Deferred::default();
        let now = self.now();
        let mut ended = 0;
        {
            let mut entries = self.entries();
            for (id, entry) in entries.iter_mut() {
                if matches!(
                    entry.state,
                    CommitmentState::Prepared | CommitmentState::Authorized
                ) {
                    let (agent, run) = (entry.agent.clone(), entry.run);
                    if self
                        .live_check(*id, entry, &agent, run, now, &mut deferred)
                        .is_err()
                    {
                        ended += 1;
                    }
                }
            }
        }
        self.flush(deferred);
        ended
    }

    /// Whether a commitment can still be authorized or started (it is
    /// prepared or authorized; liveness is checked when it is used).
    pub fn is_unconsumed(&self, id: CommitmentId) -> bool {
        self.entries().get(&id).is_some_and(|e| {
            matches!(
                e.state,
                CommitmentState::Prepared | CommitmentState::Authorized
            )
        })
    }

    pub fn view(&self, id: CommitmentId) -> Option<CommitmentView> {
        self.entries().get(&id).map(|e| e.view(id))
    }

    pub fn views_of_run(&self, run: RunId) -> Vec<CommitmentView> {
        let mut views: Vec<CommitmentView> = self
            .entries()
            .iter()
            .filter(|(_, e)| e.run == run)
            .map(|(id, e)| e.view(*id))
            .collect();
        views.sort_by_key(|v| v.created_wall_ms);
        views
    }

    /// Whether an executing commitment is still covered: the policy
    /// generation has not moved and every grant it relies on is still live
    /// (neither revoked nor expired). Effects that run in steps (a browser
    /// session) ask before each one.
    pub fn still_authorized(&self, id: CommitmentId) -> bool {
        self.lapse(id).is_none()
    }

    /// Why an executing commitment is no longer covered, if it is not: its
    /// own grant was revoked or expired, or the policy changed (another
    /// grant was revoked, or every run was stopped).
    pub fn lapse(&self, id: CommitmentId) -> Option<&'static str> {
        let now = self.now();
        let entries = self.entries();
        let Some(e) = entries.get(&id) else {
            return Some("the action is no longer known");
        };
        if e.state != CommitmentState::Executing {
            Some("the action is no longer executing")
        } else if !e.prepared.grants.iter().all(|grant| {
            self.0
                .grants
                .live_at(*grant, now.monotonic_ms, now.wall_ms)
                .is_some()
        }) {
            Some("its grant was revoked or expired")
        } else if self.0.generation.current() != e.generation {
            Some("the policy changed (a grant was revoked or every run was stopped)")
        } else {
            None
        }
    }

    /// The prepared action of a commitment that is executing under `guard`
    /// (actuators read what they committed to, nothing else).
    pub fn prepared_for(&self, guard: &ExecutionGuard) -> Option<PreparedAction> {
        self.entries()
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

    /// Whether the commitment is still covered (see
    /// [`CommitmentRegistry::still_authorized`]).
    pub fn still_authorized(&self) -> bool {
        self.registry.still_authorized(self.id)
    }

    /// Why the commitment is no longer covered, if it is not (see
    /// [`CommitmentRegistry::lapse`]).
    pub fn lapse(&self) -> Option<&'static str> {
        self.registry.lapse(self.id)
    }

    /// The same question, with the run's cancellation, for what outlives one
    /// call (a session's proxy threads): true while the run is not cancelled
    /// and the commitment is still covered.
    pub fn liveness(&self) -> Arc<dyn Fn() -> bool + Send + Sync> {
        let (registry, id, cancel) = (self.registry.clone(), self.id, self.cancel.clone());
        Arc::new(move || !cancel.is_cancelled() && registry.still_authorized(id))
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
