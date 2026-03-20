//! API rate limiting and request validation for Nexus OS.
//!
//! Provides per-category token-bucket rate limiting via [`governor`] and
//! request-level input validation (size limits, depth limits, path traversal
//! prevention).

use governor::clock::{Clock, DefaultClock};
use governor::state::keyed::DashMapStateStore;
use governor::{Quota, RateLimiter};
use serde::{Deserialize, Serialize};
use std::num::NonZeroU32;
use std::sync::Arc;

// ── Error ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RateLimitError {
    #[error("rate limit exceeded for {category}: retry after {retry_after_secs}s")]
    Exceeded {
        category: String,
        retry_after_secs: u64,
    },

    #[error("request validation failed: {0}")]
    ValidationFailed(String),
}

// ── Rate Categories ────────────────────────────────────────────────────

/// Rate-limit categories — each category has its own quota.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RateCategory {
    /// Default for most commands (list, get, status queries).
    Default,
    /// LLM inference requests (expensive).
    LlmRequest,
    /// Agent execution / goal assignment.
    AgentExecute,
    /// Audit log exports and compliance reports.
    AuditExport,
    /// Backup creation (heavy I/O).
    BackupCreate,
    /// Admin operations (config changes, key rotation).
    AdminOperation,
}

impl std::fmt::Display for RateCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Default => write!(f, "default"),
            Self::LlmRequest => write!(f, "llm_request"),
            Self::AgentExecute => write!(f, "agent_execute"),
            Self::AuditExport => write!(f, "audit_export"),
            Self::BackupCreate => write!(f, "backup_create"),
            Self::AdminOperation => write!(f, "admin_operation"),
        }
    }
}

// ── Configuration ──────────────────────────────────────────────────────

/// Rate-limiting configuration (embedded in `NexusConfig`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RateLimitConfig {
    /// Master switch — when false, all rate checks are no-ops.
    #[serde(default = "yes")]
    pub enabled: bool,

    /// Default requests per minute for most commands.
    #[serde(default = "default_rpm")]
    pub default_rpm: u32,

    /// LLM requests per minute (more expensive → lower limit).
    #[serde(default = "default_llm_rpm")]
    pub llm_rpm: u32,

    /// Agent execution requests per minute.
    #[serde(default = "default_agent_rpm")]
    pub agent_execute_rpm: u32,

    /// Audit export requests per minute.
    #[serde(default = "default_audit_rpm")]
    pub audit_export_rpm: u32,

    /// Backup creation requests per minute.
    #[serde(default = "default_backup_rpm")]
    pub backup_create_rpm: u32,

    /// Admin operation requests per minute.
    #[serde(default = "default_admin_rpm")]
    pub admin_rpm: u32,

    /// Maximum burst size (tokens available immediately).
    #[serde(default = "default_burst")]
    pub burst_size: u32,
}

fn yes() -> bool {
    true
}
fn default_rpm() -> u32 {
    120
}
fn default_llm_rpm() -> u32 {
    20
}
fn default_agent_rpm() -> u32 {
    30
}
fn default_audit_rpm() -> u32 {
    5
}
fn default_backup_rpm() -> u32 {
    2
}
fn default_admin_rpm() -> u32 {
    10
}
fn default_burst() -> u32 {
    10
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            default_rpm: default_rpm(),
            llm_rpm: default_llm_rpm(),
            agent_execute_rpm: default_agent_rpm(),
            audit_export_rpm: default_audit_rpm(),
            backup_create_rpm: default_backup_rpm(),
            admin_rpm: default_admin_rpm(),
            burst_size: default_burst(),
        }
    }
}

/// API hardening / request validation configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ApiHardeningConfig {
    /// Maximum request body size in bytes (default 10 MiB).
    #[serde(default = "default_max_body")]
    pub max_request_body_bytes: usize,

    /// Maximum length for any single string field.
    #[serde(default = "default_max_string")]
    pub max_string_length: usize,

    /// Maximum JSON nesting depth.
    #[serde(default = "default_max_depth")]
    pub json_max_depth: usize,
}

fn default_max_body() -> usize {
    10_485_760
}
fn default_max_string() -> usize {
    100_000
}
fn default_max_depth() -> usize {
    32
}

impl Default for ApiHardeningConfig {
    fn default() -> Self {
        Self {
            max_request_body_bytes: default_max_body(),
            max_string_length: default_max_string(),
            json_max_depth: default_max_depth(),
        }
    }
}

// ── Rate Limiter ───────────────────────────────────────────────────────

type KeyedLimiter = RateLimiter<String, DashMapStateStore<String>, DefaultClock>;

/// Central rate limiter — holds one token-bucket per category, keyed by
/// user/session ID so each caller gets independent quotas.
#[derive(Clone)]
pub struct NexusRateLimiter {
    enabled: bool,
    default: Arc<KeyedLimiter>,
    llm: Arc<KeyedLimiter>,
    agent_execute: Arc<KeyedLimiter>,
    audit_export: Arc<KeyedLimiter>,
    backup_create: Arc<KeyedLimiter>,
    admin: Arc<KeyedLimiter>,
}

impl std::fmt::Debug for NexusRateLimiter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NexusRateLimiter")
            .field("enabled", &self.enabled)
            .finish()
    }
}

fn build_limiter(rpm: u32, burst: u32) -> Arc<KeyedLimiter> {
    let per_minute = NonZeroU32::new(rpm.max(1)).unwrap_or(NonZeroU32::MIN);
    let burst_size = NonZeroU32::new(burst.max(1)).unwrap_or(NonZeroU32::MIN);
    let quota = Quota::per_minute(per_minute).allow_burst(burst_size);
    Arc::new(RateLimiter::dashmap(quota))
}

impl NexusRateLimiter {
    /// Create a rate limiter from configuration.
    pub fn from_config(config: &RateLimitConfig) -> Self {
        Self {
            enabled: config.enabled,
            default: build_limiter(config.default_rpm, config.burst_size),
            llm: build_limiter(config.llm_rpm, config.burst_size),
            agent_execute: build_limiter(config.agent_execute_rpm, config.burst_size),
            audit_export: build_limiter(config.audit_export_rpm, config.burst_size.min(3)),
            backup_create: build_limiter(config.backup_create_rpm, 2),
            admin: build_limiter(config.admin_rpm, config.burst_size.min(5)),
        }
    }

    /// Create a disabled (no-op) rate limiter.
    pub fn disabled() -> Self {
        let config = RateLimitConfig {
            enabled: false,
            ..RateLimitConfig::default()
        };
        Self::from_config(&config)
    }

    /// Check whether a request in the given category is allowed.
    ///
    /// `caller_id` identifies the caller — use `"global"` for single-user
    /// desktop mode, or a session/user ID for multi-tenant server mode.
    pub fn check(&self, category: RateCategory, caller_id: &str) -> Result<(), RateLimitError> {
        if !self.enabled {
            return Ok(());
        }

        let limiter = match category {
            RateCategory::Default => &self.default,
            RateCategory::LlmRequest => &self.llm,
            RateCategory::AgentExecute => &self.agent_execute,
            RateCategory::AuditExport => &self.audit_export,
            RateCategory::BackupCreate => &self.backup_create,
            RateCategory::AdminOperation => &self.admin,
        };

        let key = caller_id.to_string();

        match limiter.check_key(&key) {
            Ok(()) => Ok(()),
            Err(not_until) => {
                let wait = not_until.wait_time_from(DefaultClock::default().now());
                let retry_secs = wait.as_secs().max(1);
                Err(RateLimitError::Exceeded {
                    category: category.to_string(),
                    retry_after_secs: retry_secs,
                })
            }
        }
    }

    /// Return the remaining quota snapshot for a category (informational).
    pub fn remaining(&self, category: RateCategory, caller_id: &str) -> RateLimitInfo {
        if !self.enabled {
            return RateLimitInfo {
                limit: 0,
                remaining: 0,
                reset_after_secs: 0,
            };
        }

        let limiter = match category {
            RateCategory::Default => &self.default,
            RateCategory::LlmRequest => &self.llm,
            RateCategory::AgentExecute => &self.agent_execute,
            RateCategory::AuditExport => &self.audit_export,
            RateCategory::BackupCreate => &self.backup_create,
            RateCategory::AdminOperation => &self.admin,
        };

        let key = caller_id.to_string();
        match limiter.check_key(&key) {
            Ok(()) => RateLimitInfo {
                limit: 0,
                remaining: 1, // At least 1 available since check passed.
                reset_after_secs: 0,
            },
            Err(not_until) => {
                let wait = not_until.wait_time_from(DefaultClock::default().now());
                RateLimitInfo {
                    limit: 0,
                    remaining: 0,
                    reset_after_secs: wait.as_secs(),
                }
            }
        }
    }
}

/// Snapshot of rate-limit status for a caller+category.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitInfo {
    pub limit: u32,
    pub remaining: u32,
    pub reset_after_secs: u64,
}

// ── Request Validation ─────────────────────────────────────────────────

/// Validate a string input against hardening limits.
pub fn validate_string(value: &str, config: &ApiHardeningConfig) -> Result<(), RateLimitError> {
    if value.len() > config.max_string_length {
        return Err(RateLimitError::ValidationFailed(format!(
            "string length {} exceeds maximum {}",
            value.len(),
            config.max_string_length
        )));
    }
    Ok(())
}

/// Validate a JSON value against depth and size limits.
pub fn validate_json(
    value: &serde_json::Value,
    config: &ApiHardeningConfig,
) -> Result<(), RateLimitError> {
    let serialized = serde_json::to_vec(value)
        .map_err(|e| RateLimitError::ValidationFailed(format!("json serialization: {e}")))?;

    if serialized.len() > config.max_request_body_bytes {
        return Err(RateLimitError::ValidationFailed(format!(
            "request body size {} exceeds maximum {}",
            serialized.len(),
            config.max_request_body_bytes
        )));
    }

    let depth = json_depth(value);
    if depth > config.json_max_depth {
        return Err(RateLimitError::ValidationFailed(format!(
            "json nesting depth {depth} exceeds maximum {}",
            config.json_max_depth
        )));
    }

    Ok(())
}

/// Validate that a file path does not contain traversal sequences.
pub fn validate_path(path: &str) -> Result<(), RateLimitError> {
    // Reject path traversal attempts.
    if path.contains("..") {
        return Err(RateLimitError::ValidationFailed(
            "path traversal (..) not allowed".into(),
        ));
    }

    // Reject null bytes.
    if path.contains('\0') {
        return Err(RateLimitError::ValidationFailed(
            "null bytes in path not allowed".into(),
        ));
    }

    Ok(())
}

/// Recursively measure the nesting depth of a JSON value.
fn json_depth(value: &serde_json::Value) -> usize {
    match value {
        serde_json::Value::Array(arr) => 1 + arr.iter().map(json_depth).max().unwrap_or(0),
        serde_json::Value::Object(obj) => 1 + obj.values().map(json_depth).max().unwrap_or(0),
        _ => 0,
    }
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn test_config() -> RateLimitConfig {
        RateLimitConfig {
            enabled: true,
            default_rpm: 10,
            llm_rpm: 5,
            agent_execute_rpm: 5,
            audit_export_rpm: 2,
            backup_create_rpm: 1,
            admin_rpm: 5,
            burst_size: 3,
        }
    }

    #[test]
    fn allows_within_burst() {
        let limiter = NexusRateLimiter::from_config(&test_config());
        // Burst size is 3, so 3 immediate requests should succeed.
        for _ in 0..3 {
            assert!(limiter.check(RateCategory::Default, "user-1").is_ok());
        }
    }

    #[test]
    fn blocks_when_exceeded() {
        let limiter = NexusRateLimiter::from_config(&test_config());
        // Exhaust burst (3 for default).
        for _ in 0..3 {
            let _ = limiter.check(RateCategory::Default, "user-1");
        }
        // Next request should be denied.
        let result = limiter.check(RateCategory::Default, "user-1");
        assert!(result.is_err());
        if let Err(RateLimitError::Exceeded {
            category,
            retry_after_secs,
        }) = result
        {
            assert_eq!(category, "default");
            assert!(retry_after_secs > 0);
        }
    }

    #[test]
    fn per_user_isolation() {
        let limiter = NexusRateLimiter::from_config(&test_config());

        // Exhaust user-1's burst.
        for _ in 0..3 {
            let _ = limiter.check(RateCategory::Default, "user-1");
        }
        assert!(limiter.check(RateCategory::Default, "user-1").is_err());

        // user-2 should still have quota.
        assert!(limiter.check(RateCategory::Default, "user-2").is_ok());
    }

    #[test]
    fn disabled_limiter_allows_everything() {
        let limiter = NexusRateLimiter::disabled();
        for _ in 0..1000 {
            assert!(limiter.check(RateCategory::Default, "user-1").is_ok());
        }
    }

    #[test]
    fn different_categories_independent() {
        let limiter = NexusRateLimiter::from_config(&test_config());

        // Exhaust default burst.
        for _ in 0..3 {
            let _ = limiter.check(RateCategory::Default, "user-1");
        }
        assert!(limiter.check(RateCategory::Default, "user-1").is_err());

        // LLM category should still work.
        assert!(limiter.check(RateCategory::LlmRequest, "user-1").is_ok());
    }

    #[test]
    fn backup_has_tight_burst() {
        let limiter = NexusRateLimiter::from_config(&test_config());
        // Backup burst is min(burst_size, 2) = 2.
        assert!(limiter.check(RateCategory::BackupCreate, "u").is_ok());
        assert!(limiter.check(RateCategory::BackupCreate, "u").is_ok());
        assert!(limiter.check(RateCategory::BackupCreate, "u").is_err());
    }

    // ── Request Validation Tests ───────────────────────────────────────

    #[test]
    fn validate_string_within_limit() {
        let config = ApiHardeningConfig::default();
        assert!(validate_string("hello", &config).is_ok());
    }

    #[test]
    fn validate_string_exceeds_limit() {
        let config = ApiHardeningConfig {
            max_string_length: 10,
            ..ApiHardeningConfig::default()
        };
        let long = "a".repeat(11);
        assert!(validate_string(&long, &config).is_err());
    }

    #[test]
    fn validate_json_within_limits() {
        let config = ApiHardeningConfig::default();
        let val = json!({"a": {"b": {"c": 1}}});
        assert!(validate_json(&val, &config).is_ok());
    }

    #[test]
    fn validate_json_exceeds_depth() {
        let config = ApiHardeningConfig {
            json_max_depth: 3,
            ..ApiHardeningConfig::default()
        };
        // Build 5-deep JSON: {"a":{"a":{"a":{"a":{"a":1}}}}}
        let mut val = json!(1);
        for _ in 0..5 {
            val = json!({"a": val});
        }
        assert!(validate_json(&val, &config).is_err());
    }

    #[test]
    fn validate_json_exceeds_body_size() {
        let config = ApiHardeningConfig {
            max_request_body_bytes: 10,
            ..ApiHardeningConfig::default()
        };
        let val = json!({"data": "this is definitely larger than 10 bytes"});
        assert!(validate_json(&val, &config).is_err());
    }

    #[test]
    fn validate_path_rejects_traversal() {
        assert!(validate_path("../etc/passwd").is_err());
        assert!(validate_path("/home/user/../root").is_err());
        assert!(validate_path("safe/path/file.txt").is_ok());
    }

    #[test]
    fn validate_path_rejects_null_bytes() {
        assert!(validate_path("path\0evil").is_err());
    }

    #[test]
    fn validate_path_allows_normal_paths() {
        assert!(validate_path("/home/nexus/.nexus/data/agents.db").is_ok());
        assert!(validate_path("relative/path/file.toml").is_ok());
    }

    #[test]
    fn json_depth_measurement() {
        assert_eq!(json_depth(&json!(1)), 0);
        assert_eq!(json_depth(&json!({"a": 1})), 1);
        assert_eq!(json_depth(&json!({"a": {"b": 1}})), 2);
        assert_eq!(json_depth(&json!([1, [2, [3]]])), 3);
        assert_eq!(json_depth(&json!({"a": [{"b": 1}]})), 3);
    }

    #[test]
    fn rate_limit_error_display() {
        let err = RateLimitError::Exceeded {
            category: "llm_request".into(),
            retry_after_secs: 30,
        };
        let msg = err.to_string();
        assert!(msg.contains("llm_request"));
        assert!(msg.contains("30s"));
    }

    #[test]
    fn remaining_reports_available() {
        let limiter = NexusRateLimiter::from_config(&test_config());
        let info = limiter.remaining(RateCategory::Default, "user-1");
        assert!(info.remaining > 0);
    }
}
