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
//! released. Evidence-first records are gating records: refused, the
//! transition does not happen. The records of transitions that have taken
//! effect are end records (EL-1 §6): written, owed or uncertain, never a
//! reason to undo what happened.

use super::approval::{ActionConfirmation, ControlConfirmer, R2Approval};
use super::clock::Clock;
use super::effect::{CapabilityKind, EffectClass};
use super::evidence::{
    bounded, is_plain, EndStatus, EvidencePhase, EvidenceRecord, EvidenceSink, Privacy,
    INTERRUPTIONS,
};
use super::halt::RecordBatch;
use super::ids::{AgentId, CommitmentId, Digest, GrantId, LeaseId, RunId};
use super::policy::{GrantStore, PolicyGeneration};
use super::run::{CancelToken, GroupId, RunRegistry};
use super::AuthorityError;
use super::{write_end, write_record};
use std::collections::{BTreeSet, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
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
    /// M2d (Codex review 1 of PR #64, H2): the effect may have happened and
    /// whether it did is not established: recovery is required. Recorded
    /// as the outcome `uncertain`, never as a plain failure or a
    /// cancellation.
    Uncertain,
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
            FailureClass::Uncertain => "uncertain",
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

/// M2d-C round 4 (settlement design v2, rule 1): what a door may conclude
/// of a commitment's publication. Only `ConfirmedNotPublished` says that
/// nothing was written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PublicationClass {
    /// It succeeded: published, with the codes of a cause that ended its
    /// authority after its final check, and whether recovery is required.
    Published {
        recovery_required: bool,
        interrupted: Vec<&'static str>,
    },
    /// Never handed to the kernel's apply (denied, expired, revoked or
    /// failed before it), or refused at or before the final check with the
    /// kernel's proof.
    ConfirmedNotPublished,
    /// Not final: prepared, authorized or executing.
    Pending,
    /// An uncertain outcome, or one ended after its handoff with no proof
    /// either way.
    Uncertain,
}

/// M2d-C round 4: how far an execution got towards the kernel's apply, as
/// its effect reported it (`ExecutionGuard::handing_off`).
const NOT_HANDED_OFF: u8 = 0;
const HANDED_OFF: u8 = 1;
const PROVEN_NOT_PUBLISHED: u8 = 2;

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
    /// A separately accepted approval root, bound only by the session facade.
    /// Retained Run contexts stay immutable and are never refreshed.
    request: Option<super::file_session::HaltCell>,
    agent: AgentId,
    run: RunId,
    /// P0: its run's privacy class, copied when it is created and never
    /// derived again (every record of it is built with it).
    privacy: Privacy,
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
    /// M2d-C: what a success reported as interrupting it (its view's).
    interrupted: Vec<&'static str>,
    /// M2d-C: a success reported recovery required (its view's).
    recovery_required: bool,
    /// M2d-C round 4: ended by `finish` (it had started), with this failure
    /// class, and how far it got towards the apply.
    finished_started: bool,
    failure: Option<FailureClass>,
    handoff: u8,
    pinned: bool,
    /// Immutable original door ownership; duplicate ack never needs a live run.
    pin_owner: Option<GroupId>,
}

impl Entry {
    /// Rule 1: the publication class (see [`PublicationClass`]).
    fn publication(&self) -> PublicationClass {
        match self.state {
            CommitmentState::Prepared
            | CommitmentState::Authorized
            | CommitmentState::Executing => PublicationClass::Pending,
            CommitmentState::Succeeded => PublicationClass::Published {
                recovery_required: self.recovery_required,
                interrupted: self.interrupted.clone(),
            },
            CommitmentState::Denied | CommitmentState::Expired | CommitmentState::Revoked => {
                PublicationClass::ConfirmedNotPublished
            }
            CommitmentState::Failed | CommitmentState::Cancelled => {
                if !self.finished_started {
                    PublicationClass::ConfirmedNotPublished
                } else if self.failure == Some(FailureClass::Uncertain) {
                    PublicationClass::Uncertain
                } else if self.prepared.kind == CapabilityKind::Files && self.handoff != HANDED_OFF
                {
                    PublicationClass::ConfirmedNotPublished
                } else {
                    PublicationClass::Uncertain
                }
            }
        }
    }

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

/// End records of transitions that have taken effect, not yet written.
pub(crate) struct Unwritten(Vec<EvidenceRecord>);

/// Where an ended entry's dues go as soon as they exist: its record, and
/// its leases' ends.
trait Owes {
    fn record(&mut self, record: EvidenceRecord);
    fn leases(&mut self, leases: &[LeaseId]);
}

impl Owes for Deferred {
    fn record(&mut self, record: EvidenceRecord) {
        self.records.push(record);
    }

    fn leases(&mut self, leases: &[LeaseId]) {
        self.leases.extend(leases.iter().copied());
    }
}

/// The dues of revocations made in one hold of the commitments lock: each
/// record goes straight into `records` (a door halt's counted batch, §11
/// C2), the leases are kept to end with no lock held.
struct Revoking<'a, R: EndRecords> {
    records: &'a mut R,
    leases: Vec<LeaseId>,
}

impl<R: EndRecords> Owes for Revoking<'_, R> {
    fn record(&mut self, record: EvidenceRecord) {
        self.records.push(record);
    }

    fn leases(&mut self, leases: &[LeaseId]) {
        self.leases.extend(leases.iter().copied());
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
    preparations: std::sync::atomic::AtomicUsize,
    generation: Arc<PolicyGeneration>,
    grants: Arc<GrantStore>,
    runs: Arc<RunRegistry>,
    evidence: Arc<dyn EvidenceSink>,
    clock: Arc<dyn Clock>,
    leases: Mutex<Arc<dyn LeaseEnd>>,
    /// Issues the reservation tokens.
    transitions: AtomicU64,
    /// The broker's test hooks (tests).
    #[cfg(test)]
    hooks: Arc<crate::test_hooks::Hooks>,
    #[cfg(test)]
    capacity: AtomicU64,
}

/// Capacity reserved before a host action creates any native resource.
/// The count is changed only while the entries hold is owned.
pub(crate) struct PreparationSlot {
    registry: CommitmentRegistry,
    held: bool,
}
impl Drop for PreparationSlot {
    fn drop(&mut self) {
        if self.held {
            // Dropping a reservation cannot lose capacity accounting on
            // poison; it performs no evidence or native cleanup.
            let _entries = self
                .registry
                .0
                .entries
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            self.registry.0.preparations.fetch_sub(1, Ordering::SeqCst);
        }
    }
}

/// Where revocation records go: an unwritten list (the emergency stop), or
/// a counted batch (a door halt: every record is accounted for as soon as
/// it exists, M2c §11 C2).
pub(crate) trait EndRecords {
    fn push(&mut self, record: EvidenceRecord);
}

impl EndRecords for Vec<EvidenceRecord> {
    fn push(&mut self, record: EvidenceRecord) {
        Vec::push(self, record);
    }
}

impl EndRecords for RecordBatch {
    fn push(&mut self, record: EvidenceRecord) {
        RecordBatch::push(self, record);
    }
}

/// The backend's commitments.
#[derive(Clone)]
pub struct CommitmentRegistry(Arc<Inner>);

/// The longest a commitment may wait to start.
pub const MAX_COMMITMENT_TTL: Duration = Duration::from_secs(15 * 60);
/// Most commitments kept (finished ones are pruned first): no more can be
/// live at once.
pub(crate) const CAPACITY: usize = 4096;

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
            preparations: std::sync::atomic::AtomicUsize::new(0),
            generation,
            grants,
            runs,
            evidence,
            clock,
            leases: Mutex::new(Arc::new(NoLeases)),
            transitions: AtomicU64::new(0),
            #[cfg(test)]
            hooks: Arc::default(),
            #[cfg(test)]
            capacity: AtomicU64::new(0),
        }))
    }

    /// The broker's test hooks (tests).
    #[cfg(test)]
    pub(crate) fn hooks(&self) -> &Arc<crate::test_hooks::Hooks> {
        &self.0.hooks
    }

    #[cfg(test)]
    fn fire(&self, at: &'static str) {
        self.0.hooks.fire(at);
    }

    /// Take the commitments lock and panic while holding it, which poisons
    /// it (tests, on a thread the test joins).
    #[cfg(test)]
    pub(crate) fn panic_holding_entries(&self) {
        let _entries = self
            .0
            .entries
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        panic!("the commitments lock is poisoned on purpose (test)");
    }

    /// The same for the lease-end slot (tests).
    #[cfg(test)]
    pub(crate) fn panic_holding_lease_slot(&self) {
        let _slot = self.0.leases.lock().unwrap_or_else(PoisonError::into_inner);
        panic!("the lease-end slot is poisoned on purpose (test)");
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

    /// The most entries kept ([`CAPACITY`]; tests may lower it).
    fn capacity(&self) -> usize {
        #[cfg(test)]
        {
            let set = self.0.capacity.load(Ordering::SeqCst);
            if set > 0 {
                return set as usize;
            }
        }
        CAPACITY
    }

    #[cfg(test)]
    pub(crate) fn set_capacity_for_test(&self, capacity: usize) {
        self.0.capacity.store(capacity as u64, Ordering::SeqCst);
    }

    #[cfg(test)]
    pub(crate) fn len_for_test(&self) -> usize {
        self.entries().len()
    }

    #[cfg(test)]
    pub(crate) fn pinned_for_test(&self, id: CommitmentId) -> bool {
        self.entries().get(&id).is_some_and(|entry| entry.pinned)
    }

    /// Exact, original-door-scoped acknowledgement. Only a conclusive File
    /// outcome releases its pin. A duplicate changes no retention count;
    /// ownership is checked even after release and run pruning. Missing ids
    /// release nothing (including a repeat after the released entry was pruned).
    pub(crate) fn ack(
        &self,
        id: CommitmentId,
        agent: &AgentId,
        owner: GroupId,
    ) -> Result<(), AuthorityError> {
        let mut entries = self
            .0
            .entries
            .lock()
            .map_err(|_| AuthorityError::Unavailable("the broker cannot read its records"))?;
        let Some(entry) = entries.get_mut(&id) else {
            return Ok(());
        };
        if &entry.agent != agent {
            return Err(AuthorityError::WrongAgent);
        }
        if entry.pin_owner != Some(owner) {
            return Err(AuthorityError::NotMember);
        }
        if !entry.pinned {
            return Ok(());
        }
        match entry.publication() {
            PublicationClass::Published { .. } | PublicationClass::ConfirmedNotPublished => {}
            PublicationClass::Pending | PublicationClass::Uncertain => {
                return Err(AuthorityError::Unavailable(
                    "the commitment's outcome is not conclusive",
                ));
            }
        }
        self.0.runs.release(entry.run)?;
        entry.pinned = false;
        Ok(())
    }

    fn entries(&self) -> MutexGuard<'_, HashMap<CommitmentId, Entry>> {
        self.0.entries.lock().expect("commitments")
    }

    pub(crate) fn bind_session_request(
        &self,
        id: CommitmentId,
        agent: &AgentId,
        run: RunId,
        request: &super::file_session::HaltCell,
    ) -> Result<(), AuthorityError> {
        let mut entries = self.0.entries.lock().map_err(|_| AuthorityError::Halted)?;
        let entry = entries
            .get_mut(&id)
            .ok_or(AuthorityError::UnknownCommitment)?;
        if &entry.agent != agent
            || entry.run != run
            || !entry.pinned
            || entry.request.is_some()
            || entry.state != CommitmentState::Prepared
            || entry.reserved.is_some()
        {
            return Err(AuthorityError::NotPending);
        }
        let cancel = self.0.runs.check_request(run, agent, Some(request))?;
        cancel.admit_host()?;
        entry.request = Some(request.clone());
        Ok(())
    }

    pub(crate) fn session_request(
        &self,
        id: CommitmentId,
        run: RunId,
    ) -> Result<super::file_session::HaltCell, AuthorityError> {
        let entries = self.0.entries.lock().map_err(|_| AuthorityError::Halted)?;
        let entry = entries.get(&id).ok_or(AuthorityError::UnknownCommitment)?;
        if entry.run != run || !entry.pinned {
            return Err(AuthorityError::WrongRun);
        }
        if entry.state.is_final()
            || entry.generation != self.0.generation.current()
            || entry
                .prepared
                .grants
                .iter()
                .any(|id| self.0.grants.live(*id).is_none())
        {
            return Err(AuthorityError::Stale);
        }
        entry.request.clone().ok_or(AuthorityError::NotAdmitted)
    }

    fn now(&self) -> Now {
        Now {
            monotonic_ms: self.0.clock.monotonic_ms(),
            wall_ms: self.0.clock.wall_ms(),
        }
    }

    /// Write one gating record (evidence first: what it permits happens
    /// only once it is written); returns its sequence number. Never called
    /// with a lock held.
    fn record(&self, record: &EvidenceRecord) -> Result<u64, AuthorityError> {
        write_record(&*self.0.evidence, record).map_err(|_| AuthorityError::EvidenceUnavailable)
    }

    /// Pay what transitions made under the lock owe, once it is released:
    /// the credential leases end first (whatever the sink does), then the
    /// records are written as end records: the transitions have taken
    /// effect (ends, which fail closed), so a record that cannot be written
    /// now is owed or uncertain (the store keeps it, and refuses what comes
    /// next until it is written), never a reason to undo them. Returns what
    /// became of the last record.
    fn flush(&self, deferred: Deferred) -> Option<EndStatus> {
        self.end_leases(&deferred.leases);
        self.write_ends(Unwritten(deferred.records))
    }

    /// Write the end records of transitions that have taken effect.
    pub(crate) fn write_ends(&self, records: Unwritten) -> Option<EndStatus> {
        let mut last = None;
        for record in &records.0 {
            last = Some(write_end(&*self.0.evidence, record));
        }
        last
    }

    fn base_record(
        &self,
        phase: EvidencePhase,
        id: CommitmentId,
        entry: &Entry,
        now: Now,
    ) -> EvidenceRecord {
        let mut record = EvidenceRecord::new(
            phase,
            now.wall_ms,
            self.0.generation.current(),
            entry.privacy,
        );
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
                    #[cfg(test)]
                    self.fire(crate::test_hooks::at::COMPLETE_BEFORE_CHECK);
                    // Checked while still reserved: an end it finds records
                    // the transition it interrupted.
                    let mut live = self.live_check(id, entry, agent, run, now, &mut deferred);
                    #[cfg(test)]
                    if live.is_ok() {
                        self.fire(crate::test_hooks::at::COMPLETE_CHECKED);
                    }
                    // The run token was read before grant validation. Read
                    // its optional deny-only cell again in this same hold,
                    // immediately before approval/authorization/dispatch.
                    if live
                        .as_ref()
                        .is_ok_and(|cancel| cancel.admit_host().is_err())
                    {
                        let reason = match entry.reserved.expect("reserved transition").transition {
                            Transition::Approve => "approval_refused_halted",
                            Transition::Authorize => "authorization_refused_halted",
                            Transition::Start => "execution_dispatch_refused_halted",
                        };
                        self.end_entry(
                            id,
                            entry,
                            CommitmentState::Revoked,
                            EvidencePhase::Revoked,
                            now,
                            &mut deferred,
                            |record| {
                                record.failure = Some("halted");
                                record.cancelled = true;
                                record.detail.push(("reason".into(), reason.into()));
                            },
                        );
                        live = Err(AuthorityError::Halted);
                    }
                    entry.reserved = None;
                    let finalized = live.map(|cancel| finalize(entry, cancel));
                    #[cfg(test)]
                    if finalized.is_ok() {
                        self.fire(crate::test_hooks::at::COMPLETE_FINALIZED);
                    }
                    finalized
                }
            }
        };
        self.flush(deferred);
        completed
    }

    /// Move an entry that has not finished to the final `state`, under the
    /// lock: a transition under way for it is withdrawn (its record says
    /// which), and its evidence (`adjust` completes it) and lease ends are
    /// owed, given to `owed` as soon as they exist.
    #[allow(clippy::too_many_arguments)]
    fn end_entry(
        &self,
        id: CommitmentId,
        entry: &mut Entry,
        state: CommitmentState,
        phase: EvidencePhase,
        now: Now,
        owed: &mut impl Owes,
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
        owed.record(record);
        owed.leases(&entry.prepared.leases);
    }

    pub(crate) fn reserve_preparation(&self) -> Result<PreparationSlot, AuthorityError> {
        let mut entries = self.0.entries.lock().map_err(|_| AuthorityError::Halted)?;
        let reserved = self.0.preparations.load(Ordering::SeqCst);
        if entries.len() + reserved >= self.capacity() {
            entries.retain(|_, entry| !entry.state.is_final() || entry.pinned);
        }
        if entries.len() + reserved >= self.capacity() {
            return Err(AuthorityError::Capacity);
        }
        self.0.preparations.store(reserved + 1, Ordering::SeqCst);
        Ok(PreparationSlot {
            registry: self.clone(),
            held: true,
        })
    }

    pub(crate) fn prepare_reserved(
        &self,
        agent: &AgentId,
        run: RunId,
        prepared: PreparedAction,
        ttl: Duration,
        slot: PreparationSlot,
    ) -> Result<CommitmentView, AuthorityError> {
        if !Arc::ptr_eq(&self.0, &slot.registry.0) || !slot.held {
            return Err(AuthorityError::Halted);
        }
        let leases = prepared.leases.clone();
        let result = self.prepare_leased(agent, run, prepared, ttl, Some(slot));
        if result.is_err() {
            self.end_leases(&leases);
        }
        result
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
        let result = self.prepare_leased(agent, run, prepared, ttl, None);
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
        mut slot: Option<PreparationSlot>,
    ) -> Result<CommitmentView, AuthorityError> {
        self.0.runs.check(run, agent)?;
        // Fixed now, from the class the run was admitted with (a run gone
        // meanwhile is Restricted), and kept for the commitment's life.
        let privacy = self.0.runs.privacy(run);
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
            request: None,
            agent: agent.clone(),
            run,
            privacy,
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
            interrupted: Vec::new(),
            recovery_required: false,
            finished_started: false,
            failure: None,
            handoff: NOT_HANDED_OFF,
            pinned: false,
            pin_owner: None,
        };
        // Evidence first: an unrecorded commitment is never created. It is
        // not in the registry yet, so nothing can act on it meanwhile.
        let mut record = self.base_record(EvidencePhase::Prepared, id, &entry, now);
        for lease in entry.prepared.leases.iter().take(4) {
            record.detail.push(("lease".into(), lease.to_string()));
        }
        self.record(&record)?;
        #[cfg(test)]
        self.fire(crate::test_hooks::at::PREPARE_BEFORE_INSERT);
        let view = entry.view(id);
        let mut deferred = Deferred::default();
        let now = self.now();
        let refused = {
            let mut entries = self.entries();
            if let Some(slot) = slot.as_mut() {
                self.0.preparations.fetch_sub(1, Ordering::SeqCst);
                slot.held = false;
            }
            let reserved = self.0.preparations.load(Ordering::SeqCst);
            if entries.len() + reserved >= self.capacity() {
                entries.retain(|_, e| !e.state.is_final() || e.pinned);
            }
            // The run is checked again under the lock that cancelling and
            // stopping take, so a run ended meanwhile gets no live
            // commitment: its recorded preparation ends as revoked.
            let files = entry.prepared.kind == CapabilityKind::Files;
            let retained = if entries.len() + reserved >= self.capacity() {
                Err(AuthorityError::Capacity)
            } else {
                self.0.runs.check_retaining(run, agent, files)
            };
            match retained {
                Err(error) => {
                    let mut entry = entry;
                    if error == AuthorityError::Halted && self.0.runs.session_halted(run) {
                        self.end_entry(
                            id,
                            &mut entry,
                            CommitmentState::Revoked,
                            EvidencePhase::Revoked,
                            now,
                            &mut deferred,
                            |record| {
                                record.failure = Some("halted");
                                record.cancelled = true;
                                record.detail.push((
                                    "reason".into(),
                                    "commitment_prepare_refused_halted".into(),
                                ));
                            },
                        );
                    } else {
                        self.end_unconsumed(
                            id,
                            &mut entry,
                            CommitmentState::Revoked,
                            &error,
                            now,
                            &mut deferred,
                        );
                    }
                    Some(error)
                }
                Ok((_, owner)) => {
                    let mut entry = entry;
                    entry.pinned = files;
                    entry.pin_owner = owner;
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
        let token = match self
            .0
            .runs
            .check_request(run, agent, entry.request.as_ref())
        {
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
                #[cfg(test)]
                self.fire("decision7.grant_validation");
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
    /// lease ends are owed, given to `owed` as soon as they exist.
    fn end_unconsumed(
        &self,
        id: CommitmentId,
        entry: &mut Entry,
        state: CommitmentState,
        reason: &AuthorityError,
        now: Now,
        owed: &mut impl Owes,
    ) {
        let phase = if state == CommitmentState::Expired {
            EvidencePhase::Expired
        } else {
            EvidencePhase::Revoked
        };
        // Preserve legacy no-cell evidence. Session refusals also identify
        // the particular admission whose earlier evidence could not take effect.
        let route = if *reason == AuthorityError::Halted && self.0.runs.session_halted(entry.run) {
            entry.reserved.map(|r| match r.transition {
                Transition::Approve => "approval_refused_halted",
                Transition::Authorize => "authorization_refused_halted",
                Transition::Start => "execution_dispatch_refused_halted",
            })
        } else {
            None
        };
        self.end_entry(id, entry, state, phase, now, owed, |record| {
            if let Some(route) = route {
                record.detail.push(("reason".into(), route.into()));
            }
            record.failure = Some(reason.class());
            record.cancelled = matches!(
                reason,
                AuthorityError::RunCancelled
                    | AuthorityError::EmergencyStopped
                    | AuthorityError::Halted
            );
        });
        #[cfg(test)]
        self.fire(crate::test_hooks::at::END_UNCONSUMED_RECORDED);
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
        self.session_confirmation_handoff(id, agent, run, binding)?;
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

    fn session_confirmation_handoff(
        &self,
        id: CommitmentId,
        agent: &AgentId,
        run: RunId,
        binding: Digest,
    ) -> Result<(), AuthorityError> {
        let mut deferred = Deferred::default();
        let now = self.now();
        let result = {
            let mut entries = self.entries();
            let entry = entries
                .get_mut(&id)
                .ok_or(AuthorityError::UnknownCommitment)?;
            if entry.request.is_none() {
                return Ok(());
            }
            if entry.binding != binding
                || entry.state != CommitmentState::Prepared
                || entry.reserved.is_some()
            {
                return Err(AuthorityError::NotPending);
            }
            self.live_check(id, entry, agent, run, now, &mut deferred)
                .and_then(|cancel| cancel.admit_host())
        };
        if result.is_err() {
            self.flush(deferred);
        }
        result
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
        #[cfg(test)]
        self.fire(crate::test_hooks::at::AUTHORIZE_RECORDED);
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
        let start = self.record(&record)?;
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
            start,
            finished: false,
            handoff: std::sync::atomic::AtomicU8::new(NOT_HANDED_OFF),
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

    fn finish(
        &self,
        id: CommitmentId,
        started_wall_ms: u64,
        outcome: Outcome,
        cancelled: bool,
        handoff: u8,
    ) -> EndStatus {
        let mut deferred = Deferred::default();
        let now = self.now();
        {
            let mut entries = self.entries();
            // An executing entry is never pruned, and only this ends it: a
            // missing one cannot be claimed recorded.
            let Some(entry) = entries.get_mut(&id) else {
                return EndStatus::Uncertain;
            };
            if entry.state != CommitmentState::Executing {
                return EndStatus::Uncertain;
            }
            // The outcome is what happened: an effect that completed while
            // its run was being cancelled is recorded as completed, with the
            // cancellation beside it, never hidden as "cancelled".
            let (state, outcome_str, failure) = match &outcome {
                Outcome::Succeeded { .. } => (CommitmentState::Succeeded, "succeeded", None),
                Outcome::Failed {
                    class: FailureClass::Uncertain,
                    ..
                } => (
                    CommitmentState::Failed,
                    "uncertain",
                    Some(FailureClass::Uncertain.as_str()),
                ),
                Outcome::Failed { class, .. } => {
                    (CommitmentState::Failed, "failed", Some(class.as_str()))
                }
                Outcome::Cancelled { .. } => (CommitmentState::Cancelled, "cancelled", None),
            };
            // M2d-C round 4 (rule 1): what its publication class reads.
            entry.finished_started = true;
            entry.handoff = handoff;
            entry.failure = match &outcome {
                Outcome::Failed { class, .. } => Some(*class),
                _ => None,
            };
            // M2d-C: the codes the record's detail keeps (its first 16
            // entries), of the registry only, in its order.
            if let Outcome::Succeeded { meta } = &outcome {
                let meta = &meta[..meta.len().min(16)];
                // M2d-C (HIGH 2 of Codex review 1 of PR #78).
                entry.recovery_required = meta
                    .iter()
                    .any(|(k, v)| k == "reason" && v == "recovery_required");
                entry.interrupted = INTERRUPTIONS
                    .iter()
                    .copied()
                    .filter(|code| {
                        meta.iter()
                            .any(|(k, v)| k == "interrupted" && v.as_str() == *code)
                    })
                    .collect();
            }
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
        // cannot undo it; the start record stands either way, and the end
        // record is owed or uncertain.
        self.flush(deferred).unwrap_or(EndStatus::Uncertain)
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

    /// End every unconsumed commitment (emergency stop) and their leases,
    /// at once; their records are returned to be written after the rest of
    /// the stop has taken effect.
    pub(crate) fn revoke_all(&self) -> Unwritten {
        let mut records = Vec::new();
        let entries = self.entries();
        let leases = self.revoke_where_in(
            entries,
            |_| true,
            &AuthorityError::EmergencyStopped,
            &mut records,
        );
        self.end_leases(&leases);
        Unwritten(records)
    }

    /// M2c PR-B, a door halt's second step: under the commitments lock
    /// alone, taken even if poisoned (it also waits for every `complete()`
    /// in progress: the entries barrier), end every unconsumed commitment of
    /// `runs`. Each end record goes into `batch` as soon as it exists (§11
    /// C2). Returns the leases those commitments held, to end with no lock.
    pub(crate) fn revoke_runs(
        &self,
        runs: &BTreeSet<RunId>,
        reason: &AuthorityError,
        batch: &mut RecordBatch,
    ) -> Vec<LeaseId> {
        let entries = self
            .0
            .entries
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        #[cfg(test)]
        self.fire(crate::test_hooks::at::HALT_ENTRIES_TAKEN);
        self.revoke_where_in(entries, |run| runs.contains(&run), reason, batch)
    }

    /// End every unconsumed commitment whose run `pick` names, in the hold
    /// of `entries` given; each record goes straight into `out` as it is
    /// made, with no temporary in between. Returns their leases.
    fn revoke_where_in(
        &self,
        mut entries: MutexGuard<'_, HashMap<CommitmentId, Entry>>,
        pick: impl Fn(RunId) -> bool,
        reason: &AuthorityError,
        out: &mut impl EndRecords,
    ) -> Vec<LeaseId> {
        let now = self.now();
        let mut owed = Revoking {
            records: out,
            leases: Vec::new(),
        };
        #[cfg(test)]
        let mut first = true;
        for (id, entry) in entries.iter_mut() {
            if pick(entry.run)
                && matches!(
                    entry.state,
                    CommitmentState::Prepared | CommitmentState::Authorized
                )
            {
                self.end_unconsumed(*id, entry, CommitmentState::Revoked, reason, now, &mut owed);
                #[cfg(test)]
                if std::mem::take(&mut first) {
                    self.fire(crate::test_hooks::at::REVOKE_WHERE_FIRST);
                }
            }
        }
        owed.leases
    }

    /// M2c PR-B: end `leases` through the lease-end slot, taken even if
    /// poisoned (a halt's third step). Never called with a lock held.
    pub(crate) fn end_leases_even_if_poisoned(&self, leases: &[LeaseId]) {
        if leases.is_empty() {
            return;
        }
        let table = self
            .0
            .leases
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        for lease in leases {
            table.end(*lease);
        }
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
    /// A host-session pending installation refused after prepare. The
    /// commitment may already have ended; no terminal result is overwritten.
    pub(crate) fn refuse_pending_install(&self, id: CommitmentId) {
        let mut deferred = Deferred::default();
        let now = self.now();
        {
            let mut entries = self.entries();
            if let Some(entry) = entries.get_mut(&id).filter(|e| !e.state.is_final()) {
                self.end_entry(
                    id,
                    entry,
                    CommitmentState::Revoked,
                    EvidencePhase::Revoked,
                    now,
                    &mut deferred,
                    |record| {
                        record.failure = Some("halted");
                        record.cancelled = true;
                        record
                            .detail
                            .push(("reason".into(), "pending_effect_refused_halted".into()));
                    },
                );
            }
        }
        self.flush(deferred);
    }

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

    /// M2c: run `f` on the view of `id` while the commitments lock is held
    /// (a door's adoption joins its group inside: entries → runs, the
    /// existing order).
    pub(crate) fn holding_view<T>(
        &self,
        id: CommitmentId,
        f: impl FnOnce(Option<CommitmentView>) -> T,
    ) -> T {
        let entries = self.entries();
        let view = entries.get(&id).map(|e| e.view(id));
        #[cfg(test)]
        self.fire("decision8.adopt_view");
        let result = f(view);
        drop(entries);
        result
    }

    /// Read publication proof and its identity in one registry hold. Poison is
    /// an error, never evidence that no publication occurred.
    pub(crate) fn publication(
        &self,
        id: CommitmentId,
    ) -> Result<Option<(CommitmentView, PublicationClass)>, AuthorityError> {
        let entries = self
            .0
            .entries
            .lock()
            .map_err(|_| AuthorityError::Unavailable("the broker cannot read its records"))?;
        Ok(entries
            .get(&id)
            .map(|entry| (entry.view(id), entry.publication())))
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

    /// M2d (§6.3, Codex D2 condition 4): whether the commitment's deadline
    /// is reached on either clock (reached exactly counts), as `live_check`
    /// decides it before a start. A commitment this registry no longer
    /// knows counts as expired (fail closed).
    pub fn expired(&self, id: CommitmentId) -> bool {
        let now = self.now();
        self.entries()
            .get(&id)
            .is_none_or(|e| now.monotonic_ms >= e.deadline_ms || now.wall_ms >= e.expires_wall_ms)
    }

    /// M2d: whether a grant the commitment relies on has reached its expiry
    /// (rather than been revoked), so that an observer can name the cause.
    pub(crate) fn grant_expired(&self, id: CommitmentId) -> bool {
        let now = self.now();
        let grants = self
            .entries()
            .get(&id)
            .map(|e| e.prepared.grants.clone())
            .unwrap_or_default();
        grants.iter().any(|grant| {
            self.0
                .grants
                .expired_at(*grant, now.monotonic_ms, now.wall_ms)
        })
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
    /// The sequence number of its start record.
    start: u64,
    finished: bool,
    handoff: std::sync::atomic::AtomicU8,
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

    /// The sequence number of the start record (written before it began).
    pub fn start_seq(&self) -> u64 {
        self.start
    }

    pub fn cancel_token(&self) -> &CancelToken {
        &self.cancel
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel.is_cancelled()
    }

    /// M2d-C round 4 (rule 1): the effect is about to hand its change to
    /// the kernel's apply. From here on, an end without proof either way
    /// is uncertain.
    pub(crate) fn handing_off(&self) {
        self.handoff.store(HANDED_OFF, Ordering::SeqCst);
    }

    /// M2d-C round 4: the kernel proved the handed-off change was not
    /// published (refused at or before its final check).
    pub(crate) fn proven_not_published(&self) {
        self.handoff.store(PROVEN_NOT_PUBLISHED, Ordering::SeqCst);
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

    /// M2d: whether the commitment's deadline is reached on either clock
    /// (see [`CommitmentRegistry::expired`]). `still_authorized` does not
    /// check it; a step that must not happen after the deadline (a Files
    /// publication) asks both.
    pub fn expired(&self) -> bool {
        self.registry.expired(self.id)
    }

    /// M2d: whether a grant the commitment relies on has expired (rather
    /// than been revoked).
    pub(crate) fn grant_expired(&self) -> bool {
        self.registry.grant_expired(self.id)
    }

    /// The same question, with the run's cancellation, for what outlives one
    /// call (a session's proxy threads): true while the run is not cancelled
    /// and the commitment is still covered.
    pub fn liveness(&self) -> Arc<dyn Fn() -> bool + Send + Sync> {
        let (registry, id, cancel) = (self.registry.clone(), self.id, self.cancel.clone());
        Arc::new(move || !cancel.is_cancelled() && registry.still_authorized(id))
    }

    /// End the commitment with `outcome`. Whether the run was cancelled
    /// during the effect is recorded beside the outcome. Returns what became
    /// of its end record (EL-1 §8).
    pub fn finish(mut self, outcome: Outcome) -> EndStatus {
        self.finished = true;
        let cancelled = self.cancel.is_cancelled();
        let handoff = self.handoff.load(Ordering::SeqCst);
        self.registry
            .finish(self.id, self.started_wall_ms, outcome, cancelled, handoff)
    }
}

impl Drop for ExecutionGuard {
    fn drop(&mut self) {
        if !self.finished {
            // No caller is left to report to: the store keeps the end
            // record owed, or latched, and the interface shows it.
            self.registry.finish(
                self.id,
                self.started_wall_ms,
                Outcome::Failed {
                    class: FailureClass::Abandoned,
                    detail: "the execution ended without a result".into(),
                },
                self.cancel.is_cancelled(),
                self.handoff.load(Ordering::SeqCst),
            );
        }
    }
}
