//! OSKeyring backend.
//!
//! Phase 1 ships:
//!   - Module-private `KeyringBackend` trait (NOT public — no API
//!     obligation to external crates).
//!   - `MockKeyring` test impl behind `#[cfg(test)]`. Used by the
//!     migration tests in `kernel/src/secrets/tests.rs`.
//!   - Stub production impl `OsKeyring` that returns
//!     `BackendNotConfigured`. The real `keyring` v3-backed impl
//!     ships in Commit 4 alongside the nexus-swarm provider
//!     re-routing — Commit 4 is the first commit where the OS
//!     keyring path actually has a caller. Adding `keyring` to
//!     `kernel/Cargo.toml` requires explicit dep approval; surfaced
//!     in the Commit 1 report under DEPS_NEEDED.
//!
//! Why a trait at all (vs feature-gating the whole backend): the
//! `MockKeyring` impl is used by Phase 1 tests (Refinement D's
//! keyring-miss-falls-through scenarios), so we need a swappable
//! shape from day one. The trait is module-private to keep the
//! choice flexible without locking in an external API.

use super::{ResolvedFrom, SecretBackend, SecretError};
#[cfg(test)]
use std::collections::HashMap;
#[cfg(test)]
use std::sync::Mutex;
use zeroize::Zeroizing;

pub(crate) trait KeyringBackend: Send + Sync {
    fn get(&self, scope: &str, name: &str) -> Result<Zeroizing<String>, SecretError>;
    fn set(&self, scope: &str, name: &str, value: Zeroizing<String>) -> Result<(), SecretError>;
    fn delete(&self, scope: &str, name: &str) -> Result<(), SecretError>;
    fn list(&self, scope: &str) -> Result<Vec<String>, SecretError>;
}

/// Production keyring backend. Phase 1 STUB — returns
/// `BackendNotConfigured` for every call. The chain skips
/// configured-not-actually-wired backends silently, so this is a
/// safe placeholder until Commit 4 swaps it for a `keyring` v3
/// impl. See module header for rationale.
pub(crate) struct OsKeyring;

impl OsKeyring {
    pub(crate) fn new() -> Self {
        Self
    }
}

impl KeyringBackend for OsKeyring {
    fn get(&self, _scope: &str, _name: &str) -> Result<Zeroizing<String>, SecretError> {
        Err(SecretError::BackendNotConfigured(
            "OS keyring impl pending Commit 4 (keyring crate dep)".into(),
        ))
    }

    fn set(&self, _scope: &str, _name: &str, _value: Zeroizing<String>) -> Result<(), SecretError> {
        Err(SecretError::BackendNotConfigured(
            "OS keyring impl pending Commit 4 (keyring crate dep)".into(),
        ))
    }

    fn delete(&self, _scope: &str, _name: &str) -> Result<(), SecretError> {
        Err(SecretError::BackendNotConfigured(
            "OS keyring impl pending Commit 4 (keyring crate dep)".into(),
        ))
    }

    fn list(&self, _scope: &str) -> Result<Vec<String>, SecretError> {
        Ok(Vec::new())
    }
}

/// Module-private mock keyring used by tests. Stores entries in a
/// `Mutex<HashMap<(scope, name), value>>`.
#[cfg(test)]
#[derive(Default)]
pub(crate) struct MockKeyring {
    inner: Mutex<HashMap<(String, String), String>>,
}

#[cfg(test)]
impl MockKeyring {
    pub(crate) fn new() -> Self {
        Self::default()
    }
}

#[cfg(test)]
impl KeyringBackend for MockKeyring {
    fn get(&self, scope: &str, name: &str) -> Result<Zeroizing<String>, SecretError> {
        let guard = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        match guard.get(&(scope.to_string(), name.to_string())) {
            Some(v) => Ok(Zeroizing::new(v.clone())),
            None => Err(SecretError::NotFound),
        }
    }

    fn set(&self, scope: &str, name: &str, value: Zeroizing<String>) -> Result<(), SecretError> {
        let mut guard = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        guard.insert((scope.to_string(), name.to_string()), value.to_string());
        Ok(())
    }

    fn delete(&self, scope: &str, name: &str) -> Result<(), SecretError> {
        let mut guard = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        guard.remove(&(scope.to_string(), name.to_string()));
        Ok(())
    }

    fn list(&self, scope: &str) -> Result<Vec<String>, SecretError> {
        let guard = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        let mut out: Vec<String> = guard
            .keys()
            .filter(|(s, _)| s == scope)
            .map(|(_, n)| n.clone())
            .collect();
        out.sort();
        Ok(out)
    }
}

/// Adapter that exposes any `KeyringBackend` impl as a
/// `SecretBackend` for inclusion in `SecretsFacade::backends`.
pub struct KeyringBackendAdapter {
    inner: Box<dyn KeyringBackend>,
}

impl KeyringBackendAdapter {
    pub fn os_keyring() -> Self {
        Self {
            inner: Box::new(OsKeyring::new()),
        }
    }

    #[cfg(test)]
    pub(crate) fn mock(mock: MockKeyring) -> Self {
        Self {
            inner: Box::new(mock),
        }
    }
}

impl SecretBackend for KeyringBackendAdapter {
    fn id(&self) -> ResolvedFrom {
        ResolvedFrom::Keyring
    }

    fn get(&self, scope: &str, name: &str) -> Result<Zeroizing<String>, SecretError> {
        self.inner.get(scope, name)
    }

    fn set(&self, scope: &str, name: &str, value: Zeroizing<String>) -> Result<(), SecretError> {
        self.inner.set(scope, name, value)
    }

    fn delete(&self, scope: &str, name: &str) -> Result<(), SecretError> {
        self.inner.delete(scope, name)
    }

    fn list(&self, scope: &str) -> Result<Vec<String>, SecretError> {
        self.inner.list(scope)
    }
}
