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
//! No secret ever reaches a model prompt, a model-visible argument, a
//! frontend payload, the evidence, a URL, a shell string or a process
//! environment: it exists in memory between the vault read and the request
//! header. Nexus's own copies are zeroized on drop; the copies the HTTP and
//! TLS stack makes while sending (the header value, its buffers) are not
//! under its control. A secret the vault would resolve from the process
//! environment is refused: governed credentials come from the vault.

use crate::authority::clock::Clock;
use crate::authority::commitment::{CommitmentRegistry, ExecutionGuard, LeaseEnd};
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

/// Where secrets are read from.
pub trait SecretSource: Send + Sync {
    fn read(&self, scope: &str, name: &str) -> Result<Zeroizing<String>, SecretUnavailable>;
}

/// No vault: every credential is unavailable (operations needing one fail
/// closed).
pub struct NoVault;

impl SecretSource for NoVault {
    fn read(&self, _scope: &str, _name: &str) -> Result<Zeroizing<String>, SecretUnavailable> {
        Err(SecretUnavailable::NotConfigured)
    }
}

/// The kernel's encrypted vault (`SecretsFacade`), when the owner enabled
/// it. Every read is audited by the facade itself.
pub struct KernelVault;

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
    leases: Mutex<HashMap<LeaseId, Lease>>,
}

impl LeaseEnd for LeaseTable {
    fn end(&self, lease: LeaseId) {
        if let Some(lease) = self.leases.lock().expect("leases").get_mut(&lease) {
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
    pub fn new(authority: &Authority, source: Arc<dyn SecretSource>) -> Arc<Self> {
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
        // Evidence first: an unrecorded lease is never issued.
        self.record(record)?;
        let mut leases = self.table.leases.lock().expect("leases");
        if leases.len() >= LEASE_CAPACITY {
            leases.retain(|_, lease| lease.state == LeaseState::Issued && lease.deadline_ms > now);
            if leases.len() >= LEASE_CAPACITY {
                return Err(AuthorityError::Capacity);
            }
        }
        leases.insert(
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

    /// Whether a lease could still be released (for display and tests).
    pub fn is_live(&self, lease: LeaseId) -> bool {
        let now = self.clock.monotonic_ms();
        self.table
            .leases
            .lock()
            .expect("leases")
            .get(&lease)
            .is_some_and(|l| l.state == LeaseState::Issued && l.deadline_ms > now)
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
        let spec = {
            let mut leases = self.table.leases.lock().expect("leases");
            let lease = leases
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
            if self.clock.monotonic_ms() >= lease.deadline_ms {
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
        self.record(record)?;
        let secret = self
            .source
            .read(spec.scope, spec.name)
            .map_err(|_| AuthorityError::Unavailable("the credential is not available"))?;
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
