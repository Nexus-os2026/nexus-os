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
use super::evidence::{
    is_plain, EvidencePhase, EvidenceRecord, EvidenceSink, MAX_DETAIL, MAX_FIELD,
};
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
                "Pointer moves and scrolls: never ask you (R1)".to_string(),
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
    /// Monotonic expiry.
    pub expires_ms: u64,
    /// Wall-clock expiry: a grant also ends when this passes, so time the
    /// machine spends suspended (which the monotonic clock does not count)
    /// does not extend it.
    pub expires_wall_ms: u64,
    pub revoked: bool,
}

impl Grant {
    fn live(&self, monotonic_ms: u64, wall_ms: u64) -> bool {
        !self.revoked && self.expires_ms > monotonic_ms && self.expires_wall_ms > wall_ms
    }
}

/// The owner's grants.
pub struct GrantStore {
    grants: Mutex<Grants>,
    generation: Arc<PolicyGeneration>,
    evidence: Arc<dyn EvidenceSink>,
    clock: Arc<dyn Clock>,
}

/// The grants, and the room held for grants whose issue is being recorded
/// (no lock is held while a record is written).
#[derive(Default)]
struct Grants {
    map: HashMap<GrantId, Grant>,
    reserved: usize,
}

/// Room held for one grant while its issue is recorded: filled by the
/// grant, or given back when dropped unfilled (the record failed or
/// panicked).
struct Room<'a> {
    grants: &'a Mutex<Grants>,
    held: bool,
}

impl Room<'_> {
    fn fill(mut self, grant: Grant) {
        let mut grants = self.grants.lock().expect("grant store");
        grants.reserved = grants.reserved.saturating_sub(1);
        grants.map.insert(grant.id, grant);
        self.held = false;
    }
}

impl Drop for Room<'_> {
    fn drop(&mut self) {
        if self.held {
            if let Ok(mut grants) = self.grants.lock() {
                grants.reserved = grants.reserved.saturating_sub(1);
            }
        }
    }
}

/// Grants last at most a day; the owner grants again for longer work.
pub const MAX_GRANT_TTL: Duration = Duration::from_secs(24 * 3600);

/// Most grants kept (ended ones are pruned first).
const GRANT_CAPACITY: usize = 1024;

impl GrantStore {
    pub(crate) fn new(
        generation: Arc<PolicyGeneration>,
        evidence: Arc<dyn EvidenceSink>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            grants: Mutex::new(Grants::default()),
            generation,
            evidence,
            clock,
        }
    }

    /// Ask the owner, natively, to grant `scope` for `ttl` (at most a day).
    /// Only a confirmed request creates a grant.
    pub(crate) fn request(
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
        let wall = self.clock.wall_ms();
        let ttl_ms = u64::try_from(ttl.as_millis()).unwrap_or(u64::MAX);
        let grant = Grant {
            id,
            scope: scope.clone(),
            created_wall_ms: wall,
            expires_ms: now.saturating_add(ttl_ms),
            expires_wall_ms: wall.saturating_add(ttl_ms),
            revoked: false,
        };
        // Room first, so that a grant recorded as issued is one that exists;
        // held, not inserted, while the issue is recorded with the lock
        // released.
        {
            let mut grants = self.grants.lock().expect("grant store");
            if grants.map.len() + grants.reserved >= GRANT_CAPACITY {
                grants.map.retain(|_, grant| grant.live(now, wall));
                if grants.map.len() + grants.reserved >= GRANT_CAPACITY {
                    return Err(AuthorityError::Capacity);
                }
            }
            grants.reserved += 1;
        }
        let room = Room {
            grants: &self.grants,
            held: true,
        };
        // Evidence first: an unrecorded grant is never issued.
        self.record(EvidencePhase::GrantIssued, Some(id), &scope)?;
        room.fill(grant);
        Ok(id)
    }

    /// Revoke a grant. Every unconsumed commitment and approval is
    /// invalidated (the policy generation moves).
    pub fn revoke(&self, id: GrantId) -> Result<(), AuthorityError> {
        let scope = {
            let mut grants = self.grants.lock().expect("grant store");
            let grant = grants
                .map
                .get_mut(&id)
                .ok_or(AuthorityError::UnknownGrant)?;
            grant.revoked = true;
            grant.scope.clone()
        };
        self.generation.bump();
        self.record(EvidencePhase::GrantRevoked, Some(id), &scope)?;
        Ok(())
    }

    /// Take and release the grants' lock once (tests).
    #[cfg(test)]
    pub(crate) fn probe_locks(&self) {
        drop(self.grants.lock().expect("grant store"));
    }

    /// The room held for grants whose issue is being recorded (tests).
    #[cfg(test)]
    pub(crate) fn rooms_held(&self) -> usize {
        self.grants.lock().expect("grant store").reserved
    }

    /// The grant, if it exists, is not revoked and has not expired.
    pub fn live(&self, id: GrantId) -> Option<Grant> {
        let (now, wall) = (self.clock.monotonic_ms(), self.clock.wall_ms());
        self.live_at(id, now, wall)
    }

    /// The same, at the given clock readings (taken by a caller that holds
    /// a lock, so that no clock is read under it).
    pub(crate) fn live_at(&self, id: GrantId, monotonic_ms: u64, wall_ms: u64) -> Option<Grant> {
        self.grants
            .lock()
            .expect("grant store")
            .map
            .get(&id)
            .filter(|grant| grant.live(monotonic_ms, wall_ms))
            .cloned()
    }

    /// Every live grant of `kind`.
    pub fn live_of(&self, kind: CapabilityKind) -> Vec<Grant> {
        let (now, wall) = (self.clock.monotonic_ms(), self.clock.wall_ms());
        let mut live: Vec<Grant> = self
            .grants
            .lock()
            .expect("grant store")
            .map
            .values()
            .filter(|grant| grant.scope.kind() == kind && grant.live(now, wall))
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
            .map
            .values()
            .cloned()
            .collect();
        all.sort_by_key(|grant| grant.created_wall_ms);
        all
    }

    /// Write one record. Never called with the grants' lock held.
    fn record(
        &self,
        phase: EvidencePhase,
        id: Option<GrantId>,
        scope: &GrantScope,
    ) -> Result<(), AuthorityError> {
        let mut record =
            EvidenceRecord::new(phase, self.clock.wall_ms(), self.generation.current());
        record.kind = Some(scope.kind());
        // Every line of the scope, each in its own bounded field (cut into
        // pieces if long), so that no flag is lost to one joined, truncated
        // sentence.
        let lines = scope.describe();
        record.target = lines.first().cloned();
        if let Some(id) = id {
            record.detail.push(("grant".into(), id.to_string()));
        }
        let pieces = lines.iter().flat_map(|line| {
            let chars: Vec<char> = line.chars().collect();
            chars
                .chunks(MAX_FIELD - 16)
                .map(|piece| piece.iter().collect::<String>())
                .collect::<Vec<_>>()
        });
        for (index, piece) in pieces.take(MAX_DETAIL - 1).enumerate() {
            record.detail.push((format!("scope.{}", index + 1), piece));
        }
        self.evidence
            .record(&record)
            .map_err(|_| AuthorityError::EvidenceUnavailable)
    }
}
