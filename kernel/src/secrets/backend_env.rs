//! EnvVar backend. Read-only.
//!
//! Lookup convention: `(scope, name)` maps to `<NAME_UPPER>` for the
//! "llm" scope (matching existing project convention —
//! `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, etc.) and to
//! `NEXUS_<SCOPE>_<NAME>` for everything else.
//!
//! `set` always returns `BackendReadOnly` — the env is operator-owned;
//! mutating it from inside the process would not survive the process
//! anyway. `list` returns the empty set; the env namespace is too
//! noisy to enumerate meaningfully.

use super::{ResolvedFrom, SecretBackend, SecretError};
use zeroize::Zeroizing;

pub struct EnvBackend;

impl EnvBackend {
    pub fn new() -> Self {
        Self
    }

    /// Map `(scope, name)` to the env-var name we look up.
    /// LLM keys keep their conventional shape (`ANTHROPIC_API_KEY`)
    /// to preserve compatibility with existing operator workflows.
    /// Everything else gets the namespaced `NEXUS_<SCOPE>_<NAME>`.
    fn env_var_name(scope: &str, name: &str) -> String {
        if scope == "llm" {
            // anthropic / openai / openrouter / huggingface api_key
            // already-uppercased convention.
            let provider = name.trim_end_matches("_api_key").to_uppercase();
            format!("{provider}_API_KEY")
        } else {
            format!("NEXUS_{}_{}", scope.to_uppercase(), name.to_uppercase())
        }
    }
}

impl Default for EnvBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl SecretBackend for EnvBackend {
    fn id(&self) -> ResolvedFrom {
        ResolvedFrom::Env
    }

    fn get(&self, scope: &str, name: &str) -> Result<Zeroizing<String>, SecretError> {
        let var = Self::env_var_name(scope, name);
        match std::env::var(&var) {
            Ok(value) if !value.is_empty() => Ok(Zeroizing::new(value)),
            _ => Err(SecretError::NotFound),
        }
    }

    fn set(&self, _scope: &str, _name: &str, _value: Zeroizing<String>) -> Result<(), SecretError> {
        Err(SecretError::BackendReadOnly)
    }

    fn delete(&self, _scope: &str, _name: &str) -> Result<(), SecretError> {
        Err(SecretError::BackendReadOnly)
    }

    fn list(&self, _scope: &str) -> Result<Vec<String>, SecretError> {
        Ok(Vec::new())
    }
}
