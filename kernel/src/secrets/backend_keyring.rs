//! OSKeyring backend.
//!
//! Bug AK Commit 4: real `keyring` v3-backed implementation
//! replaces the Commit 1 stub. Linux uses kernel keyrings via
//! the `linux-native` feature (no D-Bus / libsecret runtime
//! dependency); macOS uses Security framework via `apple-native`;
//! Windows uses Credential Manager via `windows-native`.
//!
//! Service-string scheme: `nexus.<scope>.<name>` (e.g.
//! `nexus.llm.openai`, `nexus.social.x_consumer_key`). Single
//! function `service_string()` owns the namespace; per-provider
//! constants in nexus-swarm were deleted in Commit 3.
//!
//! Username component: `KEYRING_USER = "nexus"` for every entry.
//! keyring v3 requires a non-empty user; we hardcode rather than
//! using OS user identity so behavior is identical across
//! single-user and multi-user installs (Bug AK-13 may revisit
//! per-OS-user isolation).
//!
//! Soft-error policy: any `keyring::Error` other than `NoEntry`
//! returns `SecretError::BackendNotConfigured`, which the facade
//! treats as "skip this backend, try next" (see mod.rs::
//! get_secret/set_secret/for_each_backend dispatch). This means
//! a CI runner without a kernel keyring service falls through to
//! env / sqlite / memory automatically — never panics, never
//! blocks the pipeline.
//!
//! `list` always returns an empty Vec because keyring v3 has no
//! portable list/enumerate API. Callers receive an empty
//! contribution to the SecretsFacade::list_secrets union.
//!
//! Module-private `KeyringBackend` trait + `MockKeyring` test
//! impl are preserved unchanged from Commit 1 — Phase 1 tests
//! still need a swappable shape.

use super::{ResolvedFrom, SecretBackend, SecretError};
#[cfg(test)]
use std::collections::HashMap;
#[cfg(test)]
use std::sync::Mutex;
use zeroize::Zeroizing;

pub(crate) const KEYRING_USER: &str = "nexus";

/// Compose the OS-keyring service string from a vault `(scope, name)`
/// pair. Single namespace owner; called from every keyring entry
/// construction. nexus-swarm provider files deleted their per-
/// provider KEYRING_SERVICE constants in Commit 3 in anticipation
/// of this central function.
pub fn service_string(scope: &str, name: &str) -> String {
    format!("nexus.{scope}.{name}")
}

pub(crate) trait KeyringBackend: Send + Sync {
    fn get(&self, scope: &str, name: &str) -> Result<Zeroizing<String>, SecretError>;
    fn set(&self, scope: &str, name: &str, value: Zeroizing<String>) -> Result<(), SecretError>;
    fn delete(&self, scope: &str, name: &str) -> Result<(), SecretError>;
    fn list(&self, scope: &str) -> Result<Vec<String>, SecretError>;
}

/// Production keyring backend. Bug AK Commit 4: real
/// `keyring` v3-backed implementation. See module header for
/// the OS-by-OS feature matrix and the soft-error policy.
pub(crate) struct OsKeyring;

impl OsKeyring {
    pub(crate) fn new() -> Self {
        Self
    }

    fn entry(&self, scope: &str, name: &str) -> Result<keyring::Entry, SecretError> {
        let svc = service_string(scope, name);
        keyring::Entry::new(&svc, KEYRING_USER).map_err(|e| {
            SecretError::BackendNotConfigured(format!(
                "keyring entry construction failed for {svc}: {e}"
            ))
        })
    }
}

impl KeyringBackend for OsKeyring {
    fn get(&self, scope: &str, name: &str) -> Result<Zeroizing<String>, SecretError> {
        let entry = self.entry(scope, name)?;
        match entry.get_password() {
            Ok(s) => Ok(Zeroizing::new(s)),
            Err(keyring::Error::NoEntry) => Err(SecretError::NotFound),
            Err(e) => Err(SecretError::BackendNotConfigured(format!(
                "keyring get_password({scope}.{name}): {e}"
            ))),
        }
    }

    fn set(&self, scope: &str, name: &str, value: Zeroizing<String>) -> Result<(), SecretError> {
        let entry = self.entry(scope, name)?;
        entry.set_password(value.as_str()).map_err(|e| {
            SecretError::BackendNotConfigured(format!("keyring set_password({scope}.{name}): {e}"))
        })
    }

    fn delete(&self, scope: &str, name: &str) -> Result<(), SecretError> {
        let entry = self.entry(scope, name)?;
        match entry.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(SecretError::BackendNotConfigured(format!(
                "keyring delete_credential({scope}.{name}): {e}"
            ))),
        }
    }

    fn list(&self, _scope: &str) -> Result<Vec<String>, SecretError> {
        // keyring v3 has no portable list API. Return an empty
        // Vec so SecretsFacade::list_secrets gets a benign
        // contribution to the union from this backend.
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

    /// Bug AK Commit 4 test fixture: a keyring backend that
    /// returns `BackendNotConfigured` on every operation,
    /// mimicking the Commit 1 stub for tests that specifically
    /// need the chain to fall through to sqlite/memory. The
    /// real `OsKeyring` may or may not accept writes depending
    /// on whether the host has an active kernel keyring service;
    /// tests that assert on a specific resolved-from source need
    /// determinism.
    #[cfg(test)]
    pub(crate) fn rejecting() -> Self {
        struct Rejecting;
        impl KeyringBackend for Rejecting {
            fn get(&self, _: &str, _: &str) -> Result<Zeroizing<String>, SecretError> {
                Err(SecretError::BackendNotConfigured(
                    "test fixture: rejecting keyring".into(),
                ))
            }
            fn set(&self, _: &str, _: &str, _: Zeroizing<String>) -> Result<(), SecretError> {
                Err(SecretError::BackendNotConfigured(
                    "test fixture: rejecting keyring".into(),
                ))
            }
            fn delete(&self, _: &str, _: &str) -> Result<(), SecretError> {
                Err(SecretError::BackendNotConfigured(
                    "test fixture: rejecting keyring".into(),
                ))
            }
            fn list(&self, _: &str) -> Result<Vec<String>, SecretError> {
                Ok(Vec::new())
            }
        }
        Self {
            inner: Box::new(Rejecting),
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

#[cfg(test)]
mod ak_commit4_tests {
    use super::{service_string, KeyringBackend, KeyringBackendAdapter, OsKeyring, KEYRING_USER};
    use crate::secrets::SecretError;
    use zeroize::Zeroizing;

    #[test]
    fn service_string_format_llm_scope() {
        assert_eq!(service_string("llm", "openai"), "nexus.llm.openai");
        assert_eq!(service_string("llm", "anthropic"), "nexus.llm.anthropic");
    }

    #[test]
    fn service_string_format_social_scope() {
        assert_eq!(
            service_string("social", "x_consumer_key"),
            "nexus.social.x_consumer_key"
        );
        assert_eq!(
            service_string("social", "x_access_token_secret"),
            "nexus.social.x_access_token_secret"
        );
    }

    #[test]
    fn keyring_user_constant_is_nexus() {
        assert_eq!(KEYRING_USER, "nexus");
    }

    #[test]
    fn os_keyring_construction_does_not_panic() {
        let _ = OsKeyring::new();
        let _ = KeyringBackendAdapter::os_keyring();
    }

    /// Bug AK Commit 4 live round-trip. Marked `#[ignore]` because
    /// kernel keyring availability is environment-specific:
    /// - bare-metal Linux with CONFIG_KEYS: works.
    /// - many CI runners: keyring service unreachable -> the
    ///   backend returns BackendNotConfigured, the soft-error
    ///   path that lets the facade fall through. We cannot
    ///   distinguish "no keyring" from "keyring exists but is
    ///   read-only" in a deterministic CI assertion, so we skip
    ///   the live test by default.
    ///
    /// Run on a desktop session manually:
    ///     cargo test -p nexus-kernel --lib
    ///         secrets::backend_keyring::ak_commit4_tests::live_round_trip
    ///         -- --include-ignored --nocapture
    #[test]
    #[ignore]
    fn live_round_trip() {
        use std::sync::atomic::{AtomicU64, Ordering};
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let unique_name = format!(
            "ak_commit4_test_{}_{}",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::Relaxed),
        );
        let backend = OsKeyring::new();
        let value = Zeroizing::new(format!("test-value-{unique_name}"));

        // Best-effort write. If keyring backend is unavailable
        // (BackendNotConfigured), abort the test cleanly.
        match backend.set("test", &unique_name, value.clone()) {
            Ok(()) => {}
            Err(SecretError::BackendNotConfigured(msg)) => {
                eprintln!("live_round_trip: keyring not available: {msg}");
                return;
            }
            Err(e) => panic!("unexpected error on set: {e:?}"),
        }

        let got = backend.get("test", &unique_name).expect("get after set");
        assert_eq!(got.as_str(), value.as_str());

        backend
            .delete("test", &unique_name)
            .expect("delete after get");

        // Post-delete must surface as NotFound.
        match backend.get("test", &unique_name) {
            Err(SecretError::NotFound) => {}
            Err(SecretError::BackendNotConfigured(_)) => {
                // some keyring impls return BackendUnavailable
                // after delete on certain hosts; tolerate.
            }
            other => panic!("expected NotFound after delete, got {other:?}"),
        }
    }
}
