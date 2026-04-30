//! Bug AK Phase 1 — credential vault facade.
//!
//! See `docs/architecture/decisions/0004_credential_vault_facade.md`
//! for the full design. This module ships the foundation: a
//! `SecretsFacade` with four pluggable `SecretBackend`s (EnvVar,
//! OSKeyring, SqliteEnvelope, Memory) and a per-scope lookup order
//! gated by `CredentialFacadeConfig.env_override_providers`.
//!
//! No callers wire into this module yet — Commit 1 is foundation.
//! Commits 2-5 migrate consumers (SocialConfig, http_connector,
//! nexus-swarm LLM providers, Tauri commands).
//!
//! Trait shape: SYNC. Every backend's underlying API (rusqlite,
//! `keyring` crate, std env) is sync; an async wrapper would be a
//! tokio runtime tax with no I/O benefit. The ADR's API sketch
//! showed `async fn` for forward-compatibility with future
//! network-backed stores; this Phase 1 ships sync. If a future
//! Redis or remote-vault backend is added, the trait can flip
//! async then.

pub mod backend_env;
pub mod backend_keyring;
pub mod backend_memory;
pub mod backend_sqlite;
pub mod global;
pub mod migrate;

#[cfg(test)]
mod tests;

use crate::config::CredentialFacadeConfig;
use backend_env::EnvBackend;
use backend_keyring::KeyringBackendAdapter;
use backend_memory::MemoryBackend;
use backend_sqlite::SqliteEnvelopeBackend;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use thiserror::Error;
pub use zeroize::Zeroizing;

/// Errors surfaced by every `SecretBackend` and the `SecretsFacade`.
#[derive(Debug, Error)]
pub enum SecretError {
    #[error("secret not found")]
    NotFound,
    #[error("backend is read-only")]
    BackendReadOnly,
    #[error("backend not configured: {0}")]
    BackendNotConfigured(String),
    #[error("decryption failed")]
    DecryptionFailed,
    #[error("encryption failed")]
    EncryptionFailed,
    #[error("storage error: {0}")]
    Storage(String),
    #[error("crypto error: {0}")]
    Crypto(String),
}

/// Identity tag for the backend that resolved a `get_secret` call.
/// Recorded in the audit row alongside the capability string so a
/// future ledger replay (Bug AK-2) can spot a credential that
/// resolved from an unexpected source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolvedFrom {
    Env,
    Keyring,
    Sqlite,
    Memory,
}

impl ResolvedFrom {
    pub fn as_str(&self) -> &'static str {
        match self {
            ResolvedFrom::Env => "env",
            ResolvedFrom::Keyring => "keyring",
            ResolvedFrom::Sqlite => "config",
            ResolvedFrom::Memory => "memory",
        }
    }
}

/// SYNC pluggable storage trait. See module docs for rationale.
pub trait SecretBackend: Send + Sync {
    fn id(&self) -> ResolvedFrom;
    fn get(&self, scope: &str, name: &str) -> Result<Zeroizing<String>, SecretError>;
    fn set(&self, scope: &str, name: &str, value: Zeroizing<String>) -> Result<(), SecretError>;
    fn delete(&self, scope: &str, name: &str) -> Result<(), SecretError>;
    fn list(&self, scope: &str) -> Result<Vec<String>, SecretError>;
}

/// Result of a successful resolve, carrying the source for auditing.
#[derive(Debug)]
pub struct ResolvedSecret {
    pub value: Zeroizing<String>,
    pub source: ResolvedFrom,
}

/// Top-level credential vault. Holds typed backend Arcs (so
/// migration can reach the `SqliteEnvelopeBackend` directly without
/// downcasting through `dyn SecretBackend`) plus the dedup state for
/// the once-per-(provider, process) startup diagnostic.
pub struct SecretsFacade {
    env: Arc<EnvBackend>,
    keyring: Arc<KeyringBackendAdapter>,
    sqlite: Option<Arc<SqliteEnvelopeBackend>>,
    memory: Arc<MemoryBackend>,
    env_override_providers: Vec<String>,
    resolve_log_seen: Mutex<HashSet<String>>,
}

impl SecretsFacade {
    /// Construct from typed backend Arcs and the
    /// `CredentialFacadeConfig` knob. Chain order is computed
    /// per-call from `env_override_providers`. `sqlite` is
    /// optional so test fixtures can build a Memory-only facade
    /// without opening a NexusDatabase.
    pub fn new(
        env: Arc<EnvBackend>,
        keyring: Arc<KeyringBackendAdapter>,
        sqlite: Option<Arc<SqliteEnvelopeBackend>>,
        memory: Arc<MemoryBackend>,
        config: &CredentialFacadeConfig,
    ) -> Self {
        Self {
            env,
            keyring,
            sqlite,
            memory,
            env_override_providers: config.env_override_providers.clone(),
            resolve_log_seen: Mutex::new(HashSet::new()),
        }
    }

    fn chain_order(&self, scope: &str) -> [ResolvedFrom; 4] {
        let env_first = self.env_override_providers.iter().any(|p| p == scope);
        if env_first {
            [
                ResolvedFrom::Env,
                ResolvedFrom::Keyring,
                ResolvedFrom::Sqlite,
                ResolvedFrom::Memory,
            ]
        } else {
            [
                ResolvedFrom::Keyring,
                ResolvedFrom::Env,
                ResolvedFrom::Sqlite,
                ResolvedFrom::Memory,
            ]
        }
    }

    fn backend(&self, kind: ResolvedFrom) -> Option<&dyn SecretBackend> {
        match kind {
            ResolvedFrom::Env => Some(&*self.env),
            ResolvedFrom::Keyring => Some(&*self.keyring),
            ResolvedFrom::Sqlite => self.sqlite.as_deref().map(|b| b as &dyn SecretBackend),
            ResolvedFrom::Memory => Some(&*self.memory),
        }
    }

    /// Bug AK Commit 2: typed access to the SqliteEnvelope backend
    /// for `migrate_config_to_vault`. Returns `None` when the facade
    /// was built without a sqlite backend (test fixtures).
    pub(crate) fn sqlite_backend(&self) -> Option<&Arc<SqliteEnvelopeBackend>> {
        self.sqlite.as_ref()
    }

    /// Walk the configured chain. First backend that returns Ok wins.
    /// Backends returning `BackendNotConfigured` are skipped silently
    /// (e.g. OSKeyring on a server without a keyring daemon).
    /// `NotFound` continues the chain. Any other error stops the
    /// walk and surfaces.
    pub fn get_secret(&self, scope: &str, name: &str) -> Result<ResolvedSecret, SecretError> {
        for kind in self.chain_order(scope) {
            let Some(backend) = self.backend(kind) else {
                continue;
            };
            match backend.get(scope, name) {
                Ok(value) => {
                    self.maybe_log_first_resolve(scope, kind);
                    return Ok(ResolvedSecret {
                        value,
                        source: kind,
                    });
                }
                Err(SecretError::NotFound) => continue,
                Err(SecretError::BackendNotConfigured(_)) => continue,
                Err(other) => return Err(other),
            }
        }
        Err(SecretError::NotFound)
    }

    /// Write to the first writable backend in chain order. Read-only
    /// backends (EnvVar) are skipped; the next backend in chain
    /// receives the write.
    pub fn set_secret(
        &self,
        scope: &str,
        name: &str,
        value: Zeroizing<String>,
    ) -> Result<(), SecretError> {
        for kind in self.chain_order(scope) {
            let Some(backend) = self.backend(kind) else {
                continue;
            };
            match backend.set(scope, name, Zeroizing::new(value.clone().to_string())) {
                Ok(()) => return Ok(()),
                Err(SecretError::BackendReadOnly) => continue,
                Err(SecretError::BackendNotConfigured(_)) => continue,
                Err(other) => return Err(other),
            }
        }
        Err(SecretError::BackendNotConfigured(
            "no writable backend in chain".into(),
        ))
    }

    fn for_each_backend<F>(&self, mut f: F) -> Result<(), SecretError>
    where
        F: FnMut(&dyn SecretBackend) -> Result<(), SecretError>,
    {
        for kind in [
            ResolvedFrom::Env,
            ResolvedFrom::Keyring,
            ResolvedFrom::Sqlite,
            ResolvedFrom::Memory,
        ] {
            if let Some(backend) = self.backend(kind) {
                match f(backend) {
                    Ok(())
                    | Err(SecretError::NotFound)
                    | Err(SecretError::BackendReadOnly)
                    | Err(SecretError::BackendNotConfigured(_)) => {}
                    Err(other) => return Err(other),
                }
            }
        }
        Ok(())
    }

    /// Delete from every backend that has the entry. Returns Ok
    /// even if no backend held it (idempotent).
    pub fn delete_secret(&self, scope: &str, name: &str) -> Result<(), SecretError> {
        self.for_each_backend(|backend| backend.delete(scope, name))
    }

    /// Union of names across all backends in the scope.
    pub fn list_secrets(&self, scope: &str) -> Result<Vec<String>, SecretError> {
        let mut acc: HashSet<String> = HashSet::new();
        self.for_each_backend(|backend| match backend.list(scope) {
            Ok(names) => {
                for n in names {
                    acc.insert(n);
                }
                Ok(())
            }
            Err(other) => Err(other),
        })?;
        let mut out: Vec<String> = acc.into_iter().collect();
        out.sort();
        Ok(out)
    }

    fn maybe_log_first_resolve(&self, scope: &str, source: ResolvedFrom) {
        // Memory backend resolves are not logged (test fixture).
        if matches!(source, ResolvedFrom::Memory) {
            return;
        }
        let key = scope.to_string();
        let mut guard = self
            .resolve_log_seen
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        if guard.insert(key) {
            // ADR 0004: INFO once per (provider, process). Use eprintln
            // to avoid pulling tracing into kernel/Cargo.toml; matches
            // the existing kernel log idiom (record_idempotency
            // eviction failures, etc.).
            eprintln!("credential resolved: {} source={}", scope, source.as_str());
        }
    }
}
