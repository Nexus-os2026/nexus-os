//! Phase 1.4 Deliverable 2 — cross-session cost ceiling.
//!
//! The autonomous scout has a hard $10 ceiling on Anthropic API spend
//! across the entire 87-page rollout. Spend must persist across driver
//! restarts, so this module owns a JSON file at
//! `~/.nexus/ui-repair/spend.json` that records the running total.
//!
//! Every successful [`CostCeiling::record_spend`] call writes the file
//! atomically before returning, so a crash mid-rollout cannot lose
//! more than the most recent in-flight call.
//!
//! Concurrency is the caller's responsibility. The driver wraps the
//! `CostCeiling` in an `Arc<Mutex<_>>` and serializes all spend
//! decisions through that mutex.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::governance::acl::Acl;

/// Default $10 ceiling for the autonomous scout's Anthropic spend.
pub const DEFAULT_CEILING_USD: f64 = 10.0;

/// On-disk shape of `spend.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SpendFile {
    spent_usd: f64,
}

/// Persistent cost ceiling tracker.
#[derive(Debug)]
pub struct CostCeiling {
    ceiling_usd: f64,
    spent_usd: f64,
    persistence_path: PathBuf,
}

impl CostCeiling {
    /// Construct a fresh ceiling at the given path with the given
    /// limit. Does not touch disk; call [`CostCeiling::load_from_disk`]
    /// or [`CostCeiling::save_to_disk`] explicitly.
    pub fn new_with_path(persistence_path: PathBuf, ceiling_usd: f64) -> Self {
        Self {
            ceiling_usd,
            spent_usd: 0.0,
            persistence_path,
        }
    }

    /// Load the persisted spend from disk. If the file does not exist,
    /// returns a fresh ceiling with `spent_usd == 0.0`. The parent
    /// directory is created if missing.
    pub fn load_from_disk(
        persistence_path: PathBuf,
        ceiling_usd: f64,
    ) -> Result<Self, CostCeilingError> {
        // I-2 Layer 1: route parent-dir creation through the ACL helper
        // so all writes from nexus-ui-repair use the same discipline.
        acl_ensure_parent(&persistence_path)?;
        let spent_usd = if persistence_path.exists() {
            let bytes = std::fs::read(&persistence_path)
                .map_err(|e| CostCeilingError::PersistenceFailure { source: e })?;
            let parsed: SpendFile = serde_json::from_slice(&bytes).map_err(|e| {
                CostCeilingError::PersistenceFailure {
                    source: std::io::Error::new(std::io::ErrorKind::InvalidData, e),
                }
            })?;
            parsed.spent_usd
        } else {
            0.0
        };
        Ok(Self {
            ceiling_usd,
            spent_usd,
            persistence_path,
        })
    }

    /// Persist the current spend to disk. Atomic via write-to-temp +
    /// rename so a crash cannot leave a half-written file.
    pub fn save_to_disk(&self) -> Result<(), CostCeilingError> {
        self.persist_value(self.spent_usd)
    }

    /// Persist an arbitrary `spent_usd` value to the on-disk file
    /// atomically (write-to-temp + rename). Takes the value as a
    /// parameter rather than reading `self.spent_usd` so
    /// [`CostCeiling::record_spend`] can persist the *new* total
    /// **before** mutating `self`, ensuring that a failed save never
    /// leaves the in-memory and on-disk values out of sync.
    fn persist_value(&self, spent_usd: f64) -> Result<(), CostCeilingError> {
        // I-2 Layer 1: route parent-dir creation through the ACL helper.
        acl_ensure_parent(&self.persistence_path)?;
        let body = SpendFile { spent_usd };
        let bytes =
            serde_json::to_vec_pretty(&body).map_err(|e| CostCeilingError::PersistenceFailure {
                source: std::io::Error::new(std::io::ErrorKind::InvalidData, e),
            })?;
        let tmp = self.persistence_path.with_extension("json.tmp");
        std::fs::write(&tmp, &bytes)
            .map_err(|e| CostCeilingError::PersistenceFailure { source: e })?;
        std::fs::rename(&tmp, &self.persistence_path)
            .map_err(|e| CostCeilingError::PersistenceFailure { source: e })?;
        Ok(())
    }

    /// Would spending `cost_usd` more keep us at or below the ceiling?
    pub fn can_afford(&self, cost_usd: f64) -> bool {
        self.spent_usd + cost_usd <= self.ceiling_usd
    }

    /// Record `cost_usd` of new spend and persist immediately. Returns
    /// `CeilingExceeded` without mutating state if the new total would
    /// exceed the ceiling.
    pub fn record_spend(&mut self, cost_usd: f64) -> Result<(), CostCeilingError> {
        if !self.can_afford(cost_usd) {
            return Err(CostCeilingError::CeilingExceeded {
                ceiling: self.ceiling_usd,
                attempted: self.spent_usd + cost_usd,
            });
        }
        // Persist FIRST, then mutate self. If the save fails, the
        // in-memory spent_usd stays at its previous value so a
        // subsequent reload from disk cannot silently undercount.
        let new_total = self.spent_usd + cost_usd;
        self.persist_value(new_total)?;
        self.spent_usd = new_total;
        Ok(())
    }

    /// Current persisted spend, in USD.
    pub fn spent_usd(&self) -> f64 {
        self.spent_usd
    }

    /// Configured ceiling, in USD.
    pub fn ceiling_usd(&self) -> f64 {
        self.ceiling_usd
    }

    /// The persistence file path.
    pub fn persistence_path(&self) -> &Path {
        &self.persistence_path
    }
}

/// Route parent-directory creation for `path` through
/// [`Acl::ensure_parent_dirs`] so every write out of this module uses
/// the Phase 1.2 ACL helper rather than raw `std::fs::create_dir_all`.
///
/// The scout's default ACL allowlist only covers `reports/` and
/// `sessions/`, and the cost-ceiling file lives at the base of
/// `~/.nexus/ui-repair/`, so we construct a path-scoped ACL whose only
/// allowed root is the target's immediate parent. The value-add of
/// routing through `ensure_parent_dirs` is the `..` traversal
/// rejection and canonicalization — the allowlist policy itself is
/// the caller's responsibility.
fn acl_ensure_parent(path: &Path) -> Result<(), CostCeilingError> {
    let parent = match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => return Ok(()),
    };
    let acl = Acl::with_roots(vec![parent]);
    acl.ensure_parent_dirs(path)
        .map_err(|e| CostCeilingError::PersistenceFailure {
            source: std::io::Error::other(e.to_string()),
        })
}

/// Errors raised by [`CostCeiling`].
#[derive(Debug, thiserror::Error)]
pub enum CostCeilingError {
    /// Recording the spend would push the running total past the
    /// configured ceiling. State is not mutated.
    #[error("cost ceiling exceeded: ceiling=${ceiling:.2}, attempted=${attempted:.2}")]
    CeilingExceeded { ceiling: f64, attempted: f64 },

    /// Reading or writing the persistence file failed.
    #[error("cost ceiling persistence failure: {source}")]
    PersistenceFailure {
        #[source]
        source: std::io::Error,
    },
}
