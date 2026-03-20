//! Integration configuration — loaded from environment variables and TOML.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Top-level integration configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IntegrationConfig {
    #[serde(default)]
    pub slack: Option<ProviderConfig>,
    #[serde(default)]
    pub teams: Option<ProviderConfig>,
    #[serde(default)]
    pub discord: Option<ProviderConfig>,
    #[serde(default)]
    pub telegram: Option<ProviderConfig>,
    #[serde(default)]
    pub jira: Option<ProviderConfig>,
    #[serde(default)]
    pub servicenow: Option<ProviderConfig>,
    #[serde(default)]
    pub github: Option<ProviderConfig>,
    #[serde(default)]
    pub gitlab: Option<ProviderConfig>,
    #[serde(default)]
    pub webhooks: HashMap<String, WebhookConfig>,
}

/// Per-provider configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub enabled: bool,
    /// Map of config keys to values. Sensitive values use `_env` suffix
    /// to indicate the value should be read from an environment variable.
    #[serde(default)]
    pub settings: HashMap<String, String>,
    /// Which event kinds this provider should receive. `["*"]` means all events.
    #[serde(default)]
    pub events: Vec<String>,
    /// Rate limit: max requests per minute for this provider.
    #[serde(default = "default_rpm")]
    pub rate_limit_rpm: u32,
}

fn default_rpm() -> u32 {
    30
}

/// Custom webhook configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookConfig {
    pub enabled: bool,
    pub url: String,
    #[serde(default = "default_method")]
    pub method: String,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub auth: Option<WebhookAuth>,
    /// Which event kinds to forward.
    #[serde(default)]
    pub events: Vec<String>,
    #[serde(default = "default_retry")]
    pub retry_count: u32,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
}

fn default_method() -> String {
    "POST".to_string()
}
fn default_retry() -> u32 {
    3
}
fn default_timeout() -> u64 {
    10_000
}

/// Webhook authentication options.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WebhookAuth {
    #[serde(rename = "bearer")]
    Bearer {
        /// Env var name containing the token.
        token_env: String,
    },
    #[serde(rename = "basic")]
    Basic {
        username: String,
        password_env: String,
    },
    #[serde(rename = "api_key")]
    ApiKey { header: String, key_env: String },
    #[serde(rename = "hmac")]
    HmacSignature { secret_env: String },
}

impl ProviderConfig {
    /// Resolve a setting, reading from env var if the key ends with `_env`.
    pub fn resolve_setting(&self, key: &str) -> Option<String> {
        if let Some(value) = self.settings.get(key) {
            return Some(value.clone());
        }
        let env_key = format!("{key}_env");
        if let Some(env_name) = self.settings.get(&env_key) {
            return std::env::var(env_name).ok();
        }
        None
    }

    /// Check if this provider should handle the given event kind.
    pub fn matches_event(&self, event_kind: &str) -> bool {
        if !self.enabled {
            return false;
        }
        if self.events.is_empty() {
            return true; // no filter = accept all
        }
        self.events.iter().any(|e| e == "*" || e == event_kind)
    }
}

impl WebhookConfig {
    /// Check if this webhook should handle the given event kind.
    pub fn matches_event(&self, event_kind: &str) -> bool {
        if !self.enabled {
            return false;
        }
        if self.events.is_empty() {
            return true;
        }
        self.events.iter().any(|e| e == "*" || e == event_kind)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_config_matches_wildcard() {
        let cfg = ProviderConfig {
            enabled: true,
            settings: HashMap::new(),
            events: vec!["*".to_string()],
            rate_limit_rpm: 30,
        };
        assert!(cfg.matches_event("agent_error"));
        assert!(cfg.matches_event("security_event"));
    }

    #[test]
    fn provider_config_matches_specific() {
        let cfg = ProviderConfig {
            enabled: true,
            settings: HashMap::new(),
            events: vec!["agent_error".to_string(), "hitl_required".to_string()],
            rate_limit_rpm: 30,
        };
        assert!(cfg.matches_event("agent_error"));
        assert!(!cfg.matches_event("agent_started"));
    }

    #[test]
    fn disabled_provider_matches_nothing() {
        let cfg = ProviderConfig {
            enabled: false,
            settings: HashMap::new(),
            events: vec!["*".to_string()],
            rate_limit_rpm: 30,
        };
        assert!(!cfg.matches_event("agent_error"));
    }

    #[test]
    fn empty_events_means_all() {
        let cfg = ProviderConfig {
            enabled: true,
            settings: HashMap::new(),
            events: vec![],
            rate_limit_rpm: 30,
        };
        assert!(cfg.matches_event("anything"));
    }

    #[test]
    fn webhook_config_event_matching() {
        let cfg = WebhookConfig {
            enabled: true,
            url: "https://example.com/hook".into(),
            method: "POST".into(),
            headers: HashMap::new(),
            auth: None,
            events: vec!["agent_error".into()],
            retry_count: 3,
            timeout_ms: 10_000,
        };
        assert!(cfg.matches_event("agent_error"));
        assert!(!cfg.matches_event("backup_completed"));
    }

    #[test]
    fn resolve_setting_direct() {
        let mut settings = HashMap::new();
        settings.insert("default_channel".into(), "#nexus".into());
        let cfg = ProviderConfig {
            enabled: true,
            settings,
            events: vec![],
            rate_limit_rpm: 30,
        };
        assert_eq!(
            cfg.resolve_setting("default_channel"),
            Some("#nexus".into())
        );
        assert_eq!(cfg.resolve_setting("missing"), None);
    }
}
