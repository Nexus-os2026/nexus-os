//! Policy generation and owner grants.
//!
//! A grant is backend state created only after the owner's native
//! confirmation of exactly what it allows. Revoking a grant, an emergency
//! stop or any policy change bumps the policy generation, which invalidates
//! every commitment and approval not yet consumed (they record the
//! generation they were made under).

use super::approval::{ControlConfirmer, GrantConfirmation};
use super::clock::Clock;
use super::effect::CapabilityKind;
use super::evidence::{is_plain, EvidencePhase, EvidenceRecord, EvidenceSink};
use super::ids::{Digest, GrantId};
use super::AuthorityError;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// The policy generation: a counter that only grows.
#[derive(Default)]
pub struct PolicyGeneration(AtomicU64);

impl PolicyGeneration {
    pub fn current(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }

    /// Invalidate every commitment and approval made under an earlier
    /// generation.
    pub(crate) fn bump(&self) -> u64 {
        self.0.fetch_add(1, Ordering::SeqCst) + 1
    }
}

/// What a grant allows. Each domain interprets its own scope; the authority
/// core only tracks liveness.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GrantScope {
    /// P3-A: run the catalog tool `tool`, whose executable at `executable`
    /// had the identity `identity` when the owner granted it.
    Tool {
        tool: String,
        executable: String,
        identity: Digest,
    },
    /// P3-B: requests to exactly one destination.
    Egress {
        scheme: String,
        host: String,
        port: u16,
        /// Upper-case methods; empty means none.
        methods: Vec<String>,
        /// Whether a private, loopback or link-local address may be reached
        /// (never granted in production except by explicit owner policy).
        allow_private: bool,
    },
    /// P3-C: browser sessions limited to these origins, with the browser
    /// at `executable` pinned to `identity` when the owner granted it.
    Browser {
        /// `scheme://host:port`, canonical.
        origins: Vec<String>,
        downloads: bool,
        executable: String,
        identity: Digest,
    },
    /// P3-D: observing the agent display.
    Perception { display: String },
    /// P3-E: input to the agent display. `session_r1` lets a tightly scoped
    /// orchestrated session run its click and key steps as R1 under this
    /// grant (bounded by `max_steps`).
    Input {
        display: String,
        max_steps: u32,
        session_r1: bool,
    },
    /// P3-F: connector operations for one account.
    Connector {
        connector: String,
        account: String,
        operations: Vec<String>,
    },
}

impl GrantScope {
    pub fn kind(&self) -> CapabilityKind {
        match self {
            GrantScope::Tool { .. } => CapabilityKind::Tool,
            GrantScope::Egress { .. } => CapabilityKind::Egress,
            GrantScope::Browser { .. } => CapabilityKind::Browser,
            GrantScope::Perception { .. } => CapabilityKind::Perception,
            GrantScope::Input { .. } => CapabilityKind::Input,
            GrantScope::Connector { .. } => CapabilityKind::Connector,
        }
    }

    /// The lines the owner reads before granting it.
    pub fn describe(&self) -> Vec<String> {
        match self {
            GrantScope::Tool {
                tool,
                executable,
                identity,
            } => vec![
                format!("Run the tool \"{tool}\""),
                format!("Executable: {executable} (identity {})", identity.short()),
            ],
            GrantScope::Egress {
                scheme,
                host,
                port,
                methods,
                allow_private,
            } => vec![
                format!("Network requests to {scheme}://{host}:{port}"),
                format!("Methods: {}", methods.join(", ")),
                if *allow_private {
                    "Private, local or loopback addresses: allowed".to_string()
                } else {
                    "Private, local or loopback addresses: refused".to_string()
                },
            ],
            GrantScope::Browser {
                origins,
                downloads,
                executable,
                identity,
            } => vec![
                format!("Browser sessions limited to: {}", origins.join(", ")),
                format!(
                    "Downloads: {}",
                    if *downloads {
                        "kept inside the session, deleted when it ends"
                    } else {
                        "refused"
                    }
                ),
                format!("Browser: {executable} (identity {})", identity.short()),
            ],
            GrantScope::Perception { display } => {
                vec![format!("Observe the isolated agent display {display}")]
            }
            GrantScope::Input {
                display,
                max_steps,
                session_r1,
            } => vec![
                format!("Mouse and keyboard on the isolated agent display {display}"),
                format!("At most {max_steps} steps"),
                if *session_r1 {
                    "Clicks, drags and keys: without asking you each time (R1)".to_string()
                } else {
                    "Every click, drag and key: asks for your approval (R2)".to_string()
                },
            ],
            GrantScope::Connector {
                connector,
                account,
                operations,
            } => vec![
                format!(
                    "Connector \"{connector}\" (your label \"{account}\"), with its one stored credential"
                ),
                format!("Operations: {}", operations.join(", ")),
            ],
        }
    }
}

/// A live or past grant.
#[derive(Clone, Debug)]
pub struct Grant {
    pub id: GrantId,
    pub scope: GrantScope,
    pub created_wall_ms: u64,
    pub expires_ms: u64,
    pub revoked: bool,
}

/// The owner's grants.
pub struct GrantStore {
    grants: Mutex<HashMap<GrantId, Grant>>,
    generation: Arc<PolicyGeneration>,
    evidence: Arc<dyn EvidenceSink>,
    clock: Arc<dyn Clock>,
}

/// Grants last at most a day; the owner grants again for longer work.
pub const MAX_GRANT_TTL: Duration = Duration::from_secs(24 * 3600);

impl GrantStore {
    pub(crate) fn new(
        generation: Arc<PolicyGeneration>,
        evidence: Arc<dyn EvidenceSink>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            grants: Mutex::new(HashMap::new()),
            generation,
            evidence,
            clock,
        }
    }

    /// Ask the owner, natively, to grant `scope` for `ttl` (at most a day).
    /// Only a confirmed request creates a grant.
    pub fn request(
        &self,
        scope: GrantScope,
        ttl: Duration,
        confirmer: &dyn ControlConfirmer,
    ) -> Result<GrantId, AuthorityError> {
        let ttl = ttl.min(MAX_GRANT_TTL);
        let lines = scope.describe();
        // The owner must read exactly what is granted.
        if !lines.iter().all(|line| is_plain(line)) {
            return Err(AuthorityError::InvalidAction("grant text is not plain"));
        }
        let request = GrantConfirmation {
            kind: scope.kind(),
            lines,
            expires_in_secs: ttl.as_secs(),
        };
        if !confirmer.confirm_grant(&request) {
            self.record(EvidencePhase::GrantDeclined, None, &scope)?;
            return Err(AuthorityError::Declined);
        }
        let id = GrantId::fresh();
        let now = self.clock.monotonic_ms();
        let grant = Grant {
            id,
            scope: scope.clone(),
            created_wall_ms: self.clock.wall_ms(),
            expires_ms: now.saturating_add(u64::try_from(ttl.as_millis()).unwrap_or(u64::MAX)),
            revoked: false,
        };
        self.record(EvidencePhase::GrantIssued, Some(id), &scope)?;
        self.grants.lock().expect("grant store").insert(id, grant);
        Ok(id)
    }

    /// Revoke a grant. Every unconsumed commitment and approval is
    /// invalidated (the policy generation moves).
    pub fn revoke(&self, id: GrantId) -> Result<(), AuthorityError> {
        let scope = {
            let mut grants = self.grants.lock().expect("grant store");
            let grant = grants.get_mut(&id).ok_or(AuthorityError::UnknownGrant)?;
            grant.revoked = true;
            grant.scope.clone()
        };
        self.generation.bump();
        self.record(EvidencePhase::GrantRevoked, Some(id), &scope)?;
        Ok(())
    }

    /// The grant, if it exists, is not revoked and has not expired.
    pub fn live(&self, id: GrantId) -> Option<Grant> {
        let now = self.clock.monotonic_ms();
        self.grants
            .lock()
            .expect("grant store")
            .get(&id)
            .filter(|grant| !grant.revoked && grant.expires_ms > now)
            .cloned()
    }

    /// Every live grant of `kind`.
    pub fn live_of(&self, kind: CapabilityKind) -> Vec<Grant> {
        let now = self.clock.monotonic_ms();
        let mut live: Vec<Grant> = self
            .grants
            .lock()
            .expect("grant store")
            .values()
            .filter(|grant| grant.scope.kind() == kind && !grant.revoked && grant.expires_ms > now)
            .cloned()
            .collect();
        live.sort_by_key(|grant| grant.id);
        live
    }

    /// Every grant, for display.
    pub fn all(&self) -> Vec<Grant> {
        let mut all: Vec<Grant> = self
            .grants
            .lock()
            .expect("grant store")
            .values()
            .cloned()
            .collect();
        all.sort_by_key(|grant| grant.created_wall_ms);
        all
    }

    fn record(
        &self,
        phase: EvidencePhase,
        id: Option<GrantId>,
        scope: &GrantScope,
    ) -> Result<(), AuthorityError> {
        let mut record =
            EvidenceRecord::new(phase, self.clock.wall_ms(), self.generation.current());
        record.kind = Some(scope.kind());
        record.target = Some(scope.describe().join("; "));
        if let Some(id) = id {
            record.detail.push(("grant".into(), id.to_string()));
        }
        self.evidence
            .record(&record)
            .map_err(|_| AuthorityError::EvidenceUnavailable)
    }
}
