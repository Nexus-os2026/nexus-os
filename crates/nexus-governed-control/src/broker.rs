//! P3-F: the credential broker.
//!
//! Secrets stay in the vault. An operation that needs one gets a lease: an
//! opaque reference bound to the agent, the run, the service, the one origin
//! the secret may be sent to, how it is sent, and an expiry. The lease (never
//! the secret) is part of the commitment's binding. Only while that
//! commitment is executing, and only for its own destination, does the
//! broker read the secret from the vault and hand it to the transport, once;
//! the transport sends it marked sensitive and redacts it from whatever comes
//! back. A lease ends with its commitment, its run, or its expiry.
//!
//! A release consumes its lease and records that before the vault is read,
//! with no lock held while it records. Then it asks again: the run registry
//! whether the run is still live (not cancelled, finished or stopped), the
//! commitment registry whether the commitment still executes under its
//! guard and lists the lease, and the lease table whether the lease is still
//! the one this release consumed and has not expired. It asks before the
//! vault is read and once more after; a refusal reads nothing, or drops the
//! secret read, builds no header, and ends the lease. A cancellation that
//! arrives after that last look is not the broker's to see: egress looks at
//! the run again before every exchange, and the transport watches the run's
//! cancel token.
//!
//! No secret ever reaches a model prompt, a model-visible argument, a
//! frontend payload, the evidence, a URL, a shell string or a process
//! environment: it exists in memory between the vault read and the request
//! header. Nexus's own copies are zeroized on drop; the copies the HTTP and
//! TLS stack makes while sending (the header value, its buffers) are not
//! under its control. A secret the vault would resolve from the process
//! environment is refused: governed credentials come from the vault.

use crate::authority::clock::Clock;
use crate::authority::commitment::{CommitmentRegistry, CommitmentView, ExecutionGuard, LeaseEnd};
use crate::authority::evidence::{EvidencePhase, EvidenceRecord, EvidenceSink};
use crate::authority::ids::{AgentId, LeaseId, RunId};
use crate::authority::policy::PolicyGeneration;
use crate::authority::run::RunRegistry;
use crate::authority::{Authority, AuthorityError};
use crate::egress::destination::Destination;
use crate::egress::transport::SecretHeader;
use crate::egress::ReleaseCredential;
use reqwest::header::{HeaderName, AUTHORIZATION};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use zeroize::Zeroizing;

/// Why a secret could not be read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecretUnavailable {
    /// No vault is configured.
    NotConfigured,
    NotFound,
    /// The vault would resolve it from the process environment.
    Ambient,
    Failed,
}

/// Which vault a control reads credentials from. Only the crate reads it:
/// outside, the vault is a choice, not a reader.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Vault {
    /// The kernel's encrypted vault (`SecretsFacade`), when the owner
    /// enabled it.
    Kernel,
    /// None: every credentialed operation fails closed.
    Disabled,
}

impl Vault {
    pub(crate) fn source(self) -> Arc<dyn SecretSource> {
        match self {
            Vault::Kernel => Arc::new(KernelVault),
            Vault::Disabled => Arc::new(NoVault),
        }
    }
}

/// Where secrets are read from.
pub(crate) trait SecretSource: Send + Sync {
    fn read(&self, scope: &str, name: &str) -> Result<Zeroizing<String>, SecretUnavailable>;
}

/// No vault: every credential is unavailable (operations needing one fail
/// closed).
pub(crate) struct NoVault;

impl SecretSource for NoVault {
    fn read(&self, _scope: &str, _name: &str) -> Result<Zeroizing<String>, SecretUnavailable> {
        Err(SecretUnavailable::NotConfigured)
    }
}

/// The kernel's encrypted vault (`SecretsFacade`), when the owner enabled
/// it. Every read is audited by the facade itself.
pub(crate) struct KernelVault;

impl SecretSource for KernelVault {
    fn read(&self, scope: &str, name: &str) -> Result<Zeroizing<String>, SecretUnavailable> {
        let facade =
            nexus_kernel::secrets::global::try_facade().ok_or(SecretUnavailable::NotConfigured)?;
        match facade.get_secret(
            &nexus_kernel::secrets::SecretAuditCtx::system(),
            scope,
            name,
        ) {
            Ok(resolved) => from_vault(resolved),
            Err(nexus_kernel::secrets::SecretError::NotFound) => Err(SecretUnavailable::NotFound),
            Err(_) => Err(SecretUnavailable::Failed),
        }
    }
}

/// A resolved secret, only if it came from the owner's vault: one the
/// facade found in the process environment is ambient and refused.
pub(crate) fn from_vault(
    resolved: nexus_kernel::secrets::ResolvedSecret,
) -> Result<Zeroizing<String>, SecretUnavailable> {
    match resolved.source {
        nexus_kernel::secrets::ResolvedFrom::Env => Err(SecretUnavailable::Ambient),
        _ => Ok(resolved.value),
    }
}

/// How a credential travels: in `Authorization` after a scheme word
/// (`Bearer`, `Bot`), or as the whole value of one named lower-case header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placement {
    Authorization(&'static str),
    Header(&'static str),
}

/// A credential an operation may use (code-defined, never from a request).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CredentialSpec {
    /// Shown to the owner.
    pub service: &'static str,
    /// The vault scope and name it is stored under.
    pub scope: &'static str,
    pub name: &'static str,
    pub placement: Placement,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LeaseState {
    Issued,
    Released,
    Ended,
}

struct Lease {
    agent: AgentId,
    run: RunId,
    spec: CredentialSpec,
    origin: String,
    deadline_ms: u64,
    state: LeaseState,
}

/// The leases; the commitment registry ends them with their commitments.
#[derive(Default)]
struct LeaseTable {
    leases: Mutex<Leases>,
}

/// The leases, and the room held for leases whose issue is being recorded
/// (no lock is held while a record is written).
#[derive(Default)]
struct Leases {
    map: HashMap<LeaseId, Lease>,
    reserved: usize,
}

/// Room held for one lease while its issue is recorded: filled by the
/// lease, or given back when dropped unfilled (the record failed or
/// panicked).
struct Room<'a> {
    table: &'a LeaseTable,
    held: bool,
}

impl Room<'_> {
    fn fill(mut self, id: LeaseId, lease: Lease) {
        let mut leases = self.table.leases.lock().expect("leases");
        leases.reserved = leases.reserved.saturating_sub(1);
        leases.map.insert(id, lease);
        self.held = false;
    }
}

impl Drop for Room<'_> {
    fn drop(&mut self) {
        if self.held {
            if let Ok(mut leases) = self.table.leases.lock() {
                leases.reserved = leases.reserved.saturating_sub(1);
            }
        }
    }
}

impl LeaseEnd for LeaseTable {
    fn end(&self, lease: LeaseId) {
        if let Some(lease) = self.leases.lock().expect("leases").map.get_mut(&lease) {
            lease.state = LeaseState::Ended;
        }
    }
}

/// Most leases kept (ended ones are pruned first).
const LEASE_CAPACITY: usize = 4096;
/// The longest a lease lives.
pub const MAX_LEASE_TTL: Duration = Duration::from_secs(15 * 60);

/// The broker.
pub struct CredentialBroker {
    table: Arc<LeaseTable>,
    source: Arc<dyn SecretSource>,
    commitments: CommitmentRegistry,
    runs: Arc<RunRegistry>,
    clock: Arc<dyn Clock>,
    evidence: Arc<dyn EvidenceSink>,
    generation: Arc<PolicyGeneration>,
}

impl CredentialBroker {
    /// A broker over `source`, wired so the authority's commitments end
    /// their leases.
    pub(crate) fn new(authority: &Authority, source: Arc<dyn SecretSource>) -> Arc<Self> {
        let table = Arc::new(LeaseTable::default());
        authority.commitments().set_lease_end(table.clone());
        Arc::new(Self {
            table,
            source,
            commitments: authority.commitments().clone(),
            runs: authority.runs().clone(),
            clock: authority.clock().clone(),
            evidence: authority.evidence().clone(),
            generation: authority.generation().clone(),
        })
    }

    /// Write one record. Never called with the lease table's lock held.
    fn record(&self, record: EvidenceRecord) -> Result<(), AuthorityError> {
        self.evidence
            .record(&record)
            .map_err(|_| AuthorityError::EvidenceUnavailable)
    }

    /// Lease `spec` to `agent` in `run`, for requests to `origin` only.
    pub(crate) fn lease(
        &self,
        agent: &AgentId,
        run: RunId,
        spec: CredentialSpec,
        origin: &Destination,
        ttl: Duration,
    ) -> Result<LeaseId, AuthorityError> {
        self.runs.check(run, agent)?;
        let id = LeaseId::fresh();
        let now = self.clock.monotonic_ms();
        let ttl_ms = u64::try_from(ttl.min(MAX_LEASE_TTL).as_millis()).unwrap_or(u64::MAX);
        let mut record = EvidenceRecord::new(
            EvidencePhase::LeaseIssued,
            self.clock.wall_ms(),
            self.generation.current(),
        );
        record.agent = Some(agent.to_string());
        record.run = Some(run.to_string());
        record.target = Some(origin.origin_text());
        record.detail.push(("lease".into(), id.to_string()));
        record.detail.push(("service".into(), spec.service.into()));
        // Room first, so that a lease recorded as issued is one that exists;
        // held, not inserted, while the issue is recorded with the lock
        // released.
        {
            let mut leases = self.table.leases.lock().expect("leases");
            if leases.map.len() + leases.reserved >= LEASE_CAPACITY {
                leases.map.retain(|_, lease| {
                    lease.state == LeaseState::Issued && lease.deadline_ms > now
                });
                if leases.map.len() + leases.reserved >= LEASE_CAPACITY {
                    return Err(AuthorityError::Capacity);
                }
            }
            leases.reserved += 1;
        }
        let room = Room {
            table: &self.table,
            held: true,
        };
        // Evidence first: an unrecorded lease is never issued.
        self.record(record)?;
        room.fill(
            id,
            Lease {
                agent: agent.clone(),
                run,
                spec,
                origin: origin.origin_text(),
                deadline_ms: now.saturating_add(ttl_ms),
                state: LeaseState::Issued,
            },
        );
        Ok(id)
    }

    /// Take and release the lease table's lock once (tests).
    #[cfg(test)]
    pub(crate) fn probe_locks(&self) {
        drop(self.table.leases.lock().expect("leases"));
    }

    /// The room held for leases whose issue is being recorded (tests).
    #[cfg(test)]
    pub(crate) fn rooms_held(&self) -> usize {
        self.table.leases.lock().expect("leases").reserved
    }

    /// Whether a lease could still be released (for display and tests).
    /// End a lease no commitment will list (its preparation failed).
    pub(crate) fn end_lease(&self, lease: LeaseId) {
        self.table.end(lease);
    }

    /// Every lease issued so far (tests).
    #[cfg(test)]
    pub(crate) fn issued(&self) -> Vec<LeaseId> {
        self.table
            .leases
            .lock()
            .expect("leases")
            .map
            .keys()
            .copied()
            .collect()
    }

    pub fn is_live(&self, lease: LeaseId) -> bool {
        let now = self.clock.monotonic_ms();
        self.table
            .leases
            .lock()
            .expect("leases")
            .map
            .get(&lease)
            .is_some_and(|l| l.state == LeaseState::Issued && l.deadline_ms > now)
    }

    /// Whether the release that consumed `lease_id` under `guard` may still
    /// go on, asked again after a step taken with no lock held (its record,
    /// the vault read): the run is live, by the run registry's own answer;
    /// the commitment still executes under `guard` and lists the lease; and
    /// the lease is still this release's (its agent and run, `Released`), its
    /// deadline not passed. Each lock is taken alone and released before the
    /// next. A refusal ends the lease and is returned as the authority gave
    /// it.
    fn still_releasing(
        &self,
        lease_id: LeaseId,
        guard: &ExecutionGuard,
        view: &CommitmentView,
    ) -> Result<(), AuthorityError> {
        let live = self.runs.check(view.run, &view.agent).and_then(|_| {
            match self.commitments.prepared_for(guard) {
                Some(prepared) if prepared.leases.contains(&lease_id) => Ok(()),
                Some(_) => Err(AuthorityError::Closed(
                    "the lease is not bound to this commitment",
                )),
                None => Err(AuthorityError::NotAuthorized),
            }
        });
        let now = self.clock.monotonic_ms();
        let mut leases = self.table.leases.lock().expect("leases");
        let lease = leases.map.get_mut(&lease_id);
        let verdict = live.and_then(|()| match lease.as_deref() {
            None => Err(AuthorityError::UnknownLease),
            Some(held) if held.agent != view.agent => Err(AuthorityError::WrongAgent),
            Some(held) if held.run != view.run => Err(AuthorityError::WrongRun),
            Some(held) if held.state != LeaseState::Released => Err(AuthorityError::NotPending),
            Some(held) if now >= held.deadline_ms => Err(AuthorityError::Expired),
            Some(_) => Ok(()),
        });
        if let (Err(_), Some(lease)) = (&verdict, lease) {
            // Ended, never issued again: no later release reads the vault.
            lease.state = LeaseState::Ended;
        }
        verdict
    }
}

impl ReleaseCredential for CredentialBroker {
    fn release(
        &self,
        lease_id: LeaseId,
        guard: &ExecutionGuard,
        destination: &Destination,
    ) -> Result<SecretHeader, AuthorityError> {
        // The commitment executing under this guard must list the lease.
        let prepared = self
            .commitments
            .prepared_for(guard)
            .ok_or(AuthorityError::NotAuthorized)?;
        if !prepared.leases.contains(&lease_id) {
            return Err(AuthorityError::Closed(
                "the lease is not bound to this commitment",
            ));
        }
        let view = self
            .commitments
            .view(guard.commitment())
            .ok_or(AuthorityError::UnknownCommitment)?;
        let now = self.clock.monotonic_ms();
        let spec = {
            let mut leases = self.table.leases.lock().expect("leases");
            let lease = leases
                .map
                .get_mut(&lease_id)
                .ok_or(AuthorityError::UnknownLease)?;
            if lease.agent != view.agent {
                return Err(AuthorityError::WrongAgent);
            }
            if lease.run != view.run {
                return Err(AuthorityError::WrongRun);
            }
            if lease.state != LeaseState::Issued {
                return Err(AuthorityError::NotPending);
            }
            if now >= lease.deadline_ms {
                lease.state = LeaseState::Ended;
                return Err(AuthorityError::Expired);
            }
            if destination.origin_text() != lease.origin {
                return Err(AuthorityError::TargetChanged);
            }
            // One-shot: consumed before the vault is read, whatever follows.
            lease.state = LeaseState::Released;
            lease.spec
        };
        let mut record = EvidenceRecord::new(
            EvidencePhase::CredentialReleased,
            self.clock.wall_ms(),
            self.generation.current(),
        );
        record.commitment = Some(guard.commitment().to_string());
        record.agent = Some(view.agent.to_string());
        record.run = Some(view.run.to_string());
        record.target = Some(destination.origin_text());
        record.detail.push(("lease".into(), lease_id.to_string()));
        record.detail.push(("service".into(), spec.service.into()));
        // Recorded with no lock held: whatever arrived meanwhile (a
        // cancellation, a stop, the lease's expiry) is asked about now,
        // before the vault is read.
        self.record(record)?;
        self.still_releasing(lease_id, guard, &view)?;
        let secret = self.source.read(spec.scope, spec.name);
        // And once more now that the vault has answered: refused, the secret
        // read is dropped (zeroized) and no header is built. Later than
        // this, egress's own look before the exchange and the transport's
        // cancel token are what stop the request.
        self.still_releasing(lease_id, guard, &view)?;
        let secret =
            secret.map_err(|_| AuthorityError::Unavailable("the credential is not available"))?;
        // A token is visible ASCII; anything else could break the header.
        if secret.is_empty() || !secret.bytes().all(|b| (0x21..0x7f).contains(&b)) {
            return Err(AuthorityError::Unavailable(
                "the credential is not a plain token",
            ));
        }
        let (name, value) = match spec.placement {
            Placement::Authorization(scheme) => (
                AUTHORIZATION,
                Zeroizing::new(format!("{scheme} {}", *secret)),
            ),
            Placement::Header(header) => (
                HeaderName::from_bytes(header.as_bytes())
                    .map_err(|_| AuthorityError::Unavailable("credential header is not valid"))?,
                Zeroizing::new(secret.to_string()),
            ),
        };
        Ok(SecretHeader {
            name,
            value,
            token: Zeroizing::new(secret.to_string()),
        })
    }
}

#[cfg(test)]
pub(crate) mod tests;
