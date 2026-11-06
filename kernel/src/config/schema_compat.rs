//! Compatibility schemas retained independently of the legacy runtimes.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Scheduled backup configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BackupScheduleConfig {
    #[serde(default)]
    pub enabled: bool,

    /// Cron expression for backup schedule (e.g. "0 2 * * *" = daily 2 AM).
    #[serde(default = "default_schedule")]
    pub schedule: String,

    /// Directory to store backups.
    #[serde(default = "default_backup_dir_string")]
    pub output_dir: String,

    /// Number of backups to retain before rotating old ones.
    #[serde(default = "default_retention")]
    pub retention_count: u32,

    /// Encrypt backups.
    #[serde(default)]
    pub encrypt: bool,

    /// Compression: "gzip" (default).
    #[serde(default = "default_compression")]
    pub compression: String,

    #[serde(default = "yes_eq")]
    pub include_audit: bool,

    #[serde(default = "yes_eq")]
    pub include_genomes: bool,
}

fn yes_eq() -> bool {
    true
}
fn default_schedule() -> String {
    "0 2 * * *".to_string()
}
fn default_backup_dir_string() -> String {
    default_backup_dir().to_string_lossy().into_owned()
}
fn default_retention() -> u32 {
    30
}
fn default_compression() -> String {
    "gzip".to_string()
}

impl Default for BackupScheduleConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            schedule: default_schedule(),
            output_dir: default_backup_dir_string(),
            retention_count: 30,
            encrypt: false,
            compression: default_compression(),
            include_audit: true,
            include_genomes: true,
        }
    }
}

/// The default backup directory beneath the validated identity home. With no
/// valid identity home it is empty, which `create_backup` refuses rather than
/// resolving against the working directory or a shared temporary directory
/// (P0-002C5B).
fn default_backup_dir() -> PathBuf {
    crate::identity_home::identity_home()
        .map(|home| {
            home.join(".local")
                .join("share")
                .join("nexus-os")
                .join("backups")
        })
        .unwrap_or_default()
}

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
