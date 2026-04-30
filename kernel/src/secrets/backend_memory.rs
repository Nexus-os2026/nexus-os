//! Memory backend. Test-only.
//!
//! Implements every `SecretBackend` operation against an
//! `Arc<Mutex<HashMap<(scope, name), value>>>`. Always last in the
//! chain order. Production code should never see this backend
//! resolve a real credential — the `SecretsFacade` deliberately
//! suppresses its startup-diagnostic log line for memory hits to
//! avoid masking misconfiguration in production.

use super::{ResolvedFrom, SecretBackend, SecretError};
use std::collections::HashMap;
use std::sync::Mutex;
use zeroize::Zeroizing;

#[derive(Default)]
pub struct MemoryBackend {
    inner: Mutex<HashMap<(String, String), String>>,
}

impl MemoryBackend {
    pub fn new() -> Self {
        Self::default()
    }
}

impl SecretBackend for MemoryBackend {
    fn id(&self) -> ResolvedFrom {
        ResolvedFrom::Memory
    }

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
