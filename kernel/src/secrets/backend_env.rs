//! EnvVar backend. Read-only.
//!
//! Lookup convention: `(scope, name)` maps to `<NAME_UPPER>` for the
//! "llm" scope (matching existing project convention —
//! `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, etc.) and to
//! `NEXUS_<SCOPE>_<NAME>` for everything else.
//!
//! Non-LLM scope/name components are flattened to shell-friendly,
//! uppercase identifiers. For example `auth.oidc/client_secret` maps to
//! `NEXUS_AUTH_OIDC_CLIENT_SECRET`.
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

    /// Convert a secret scope/name component to a shell-friendly environment
    /// variable component. ASCII alphanumerics and underscores are preserved;
    /// separators such as dots and hyphens are flattened to underscores.
    fn env_component(value: &str) -> String {
        value
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() {
                    ch.to_ascii_uppercase()
                } else {
                    '_'
                }
            })
            .collect()
    }

    /// Map `(scope, name)` to the env-var name we look up.
    /// LLM keys keep their conventional shape (`ANTHROPIC_API_KEY`)
    /// to preserve compatibility with existing operator workflows.
    /// Everything else gets the namespaced `NEXUS_<SCOPE>_<NAME>`.
    fn env_var_name(scope: &str, name: &str) -> String {
        if scope == "llm" {
            // Bug AK Commit 3 UNILATERAL: nvidia's canonical env
            // var is NVIDIA_NIM_API_KEY (matches the upstream NIM
            // gateway naming and the existing build_provider_config
            // call site), not NVIDIA_API_KEY. Special-case it so
            // facade.get_secret("llm", "nvidia") finds the env
            // entry that operators already set.
            if name == "nvidia" {
                return "NVIDIA_NIM_API_KEY".to_string();
            }
            // anthropic / openai / openrouter / huggingface api_key
            // already-uppercased convention.
            let provider = name.trim_end_matches("_api_key").to_uppercase();
            format!("{provider}_API_KEY")
        } else {
            format!(
                "NEXUS_{}_{}",
                Self::env_component(scope),
                Self::env_component(name)
            )
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

#[cfg(test)]
mod tests {
    use super::EnvBackend;

    #[test]
    fn flattens_dotted_scope_to_shell_friendly_name() {
        assert_eq!(
            EnvBackend::env_var_name("auth.oidc", "client_secret"),
            "NEXUS_AUTH_OIDC_CLIENT_SECRET"
        );
    }

    #[test]
    fn flattens_non_alphanumeric_separators() {
        assert_eq!(
            EnvBackend::env_var_name("integration.github", "oauth.client-secret"),
            "NEXUS_INTEGRATION_GITHUB_OAUTH_CLIENT_SECRET"
        );
    }

    #[test]
    fn preserves_llm_provider_conventions() {
        assert_eq!(
            EnvBackend::env_var_name("llm", "anthropic"),
            "ANTHROPIC_API_KEY"
        );
        assert_eq!(
            EnvBackend::env_var_name("llm", "nvidia"),
            "NVIDIA_NIM_API_KEY"
        );
    }
}
