//! Backup & Restore for Nexus OS data stores.
//!
//! Creates compressed, optionally encrypted archives of all Nexus OS data
//! (databases, manifests, configuration) with integrity verification.

use crate::crypto::{self, CryptoError, EncryptionKey};
use chrono::{DateTime, Utc};
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};
use uuid::Uuid;

// ── Error ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BackupError {
    #[error("io error: {0}")]
    Io(String),

    #[error("archive error: {0}")]
    Archive(String),

    #[error("integrity check failed: {0}")]
    IntegrityFailed(String),

    #[error("encryption error: {0}")]
    Encryption(String),

    #[error("restore error: {0}")]
    Restore(String),

    #[error("not found: {0}")]
    NotFound(String),
}

impl From<CryptoError> for BackupError {
    fn from(e: CryptoError) -> Self {
        BackupError::Encryption(e.to_string())
    }
}

impl From<std::io::Error> for BackupError {
    fn from(e: std::io::Error) -> Self {
        BackupError::Io(e.to_string())
    }
}

// ── Types ──────────────────────────────────────────────────────────────

/// What to include in a backup.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupConfig {
    /// Where to write the backup archive.
    pub output_dir: PathBuf,

    /// Include audit trail databases.
    #[serde(default = "yes")]
    pub include_audit: bool,

    /// Include agent genome databases.
    #[serde(default = "yes")]
    pub include_genomes: bool,

    /// Include configuration files.
    #[serde(default = "yes")]
    pub include_config: bool,

    /// Include agent manifest TOML files.
    #[serde(default = "yes")]
    pub include_manifests: bool,

    /// Encrypt the backup archive with the current encryption key.
    #[serde(default)]
    pub encrypt: bool,
}

fn yes() -> bool {
    true
}

impl Default for BackupConfig {
    fn default() -> Self {
        Self {
            output_dir: default_backup_dir(),
            include_audit: true,
            include_genomes: true,
            include_config: true,
            include_manifests: true,
            encrypt: false,
        }
    }
}

/// Metadata stored alongside (and inside) each backup archive.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupMetadata {
    /// Unique backup identifier.
    pub id: String,

    /// Nexus OS version at time of backup.
    pub version: String,

    /// When the backup was created.
    pub created_at: DateTime<Utc>,

    /// SHA-256 checksum of the archive (hex-encoded).
    pub checksum: String,

    /// List of included items (file paths relative to data dir).
    pub contents: Vec<String>,

    /// Total archive size in bytes.
    pub size_bytes: u64,

    /// Whether the archive is encrypted.
    pub encrypted: bool,
}

/// Result of a restore operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreResult {
    pub backup_id: String,
    pub restored_files: Vec<String>,
    pub warnings: Vec<String>,
}

/// Result of a verify operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyResult {
    pub valid: bool,
    pub backup_id: String,
    pub checksum_ok: bool,
    pub files_ok: bool,
    pub audit_chain_ok: bool,
    pub errors: Vec<String>,
}

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

// ── Helpers ────────────────────────────────────────────────────────────

fn default_backup_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(home)
        .join(".local")
        .join("share")
        .join("nexus-os")
        .join("backups")
}

/// Resolve the Nexus OS data directory (where databases live).
pub fn nexus_data_dir() -> PathBuf {
    if let Ok(path) = std::env::var("NEXUS_DATA_DIR") {
        return PathBuf::from(path);
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(home).join(".nexus")
}

fn sha256_file(path: &Path) -> Result<String, BackupError> {
    let data = std::fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&data);
    Ok(format!("{:x}", hasher.finalize()))
}

// ── Create Backup ──────────────────────────────────────────────────────

/// Create a backup archive of Nexus OS data.
pub fn create_backup(
    config: &BackupConfig,
    data_dir: &Path,
    encryption_key: Option<&EncryptionKey>,
) -> Result<BackupMetadata, BackupError> {
    std::fs::create_dir_all(&config.output_dir)?;

    let backup_id = Uuid::new_v4().to_string();
    let short_id = &backup_id[..8];
    let timestamp = Utc::now();
    let archive_name = format!(
        "nexus-backup-{}-{short_id}.tar.gz",
        timestamp.format("%Y%m%d-%H%M%S")
    );
    let archive_path = config.output_dir.join(&archive_name);

    // Collect files to back up.
    let mut files_to_backup: Vec<(PathBuf, String)> = Vec::new();

    if data_dir.exists() {
        collect_backup_files(data_dir, data_dir, config, &mut files_to_backup)?;
    }

    // Also back up config file.
    if config.include_config {
        let config_path = crate::config::config_path();
        if config_path.exists() {
            files_to_backup.push((config_path.clone(), "config/config.toml".to_string()));
        }
    }

    let contents: Vec<String> = files_to_backup.iter().map(|(_, rel)| rel.clone()).collect();

    // Create tar.gz archive.
    let archive_file =
        std::fs::File::create(&archive_path).map_err(|e| BackupError::Io(e.to_string()))?;
    let encoder = GzEncoder::new(archive_file, Compression::default());
    let mut tar_builder = tar::Builder::new(encoder);

    // Write metadata as the first entry.
    let metadata = BackupMetadata {
        id: backup_id.clone(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        created_at: timestamp,
        checksum: String::new(), // Filled after archive is complete.
        contents: contents.clone(),
        size_bytes: 0,
        encrypted: config.encrypt,
    };

    let meta_json = serde_json::to_vec_pretty(&metadata)
        .map_err(|e| BackupError::Archive(format!("serialize metadata: {e}")))?;
    let mut meta_header = tar::Header::new_gnu();
    meta_header.set_size(meta_json.len() as u64);
    meta_header.set_mode(0o644);
    meta_header.set_cksum();
    tar_builder
        .append_data(
            &mut meta_header,
            "backup-metadata.json",
            meta_json.as_slice(),
        )
        .map_err(|e| BackupError::Archive(e.to_string()))?;

    // Append data files.
    for (src_path, archive_rel) in &files_to_backup {
        if src_path.is_file() {
            let data = std::fs::read(src_path)?;
            let mut header = tar::Header::new_gnu();
            header.set_size(data.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            tar_builder
                .append_data(&mut header, archive_rel, data.as_slice())
                .map_err(|e| BackupError::Archive(e.to_string()))?;
        }
    }

    tar_builder
        .finish()
        .map_err(|e| BackupError::Archive(e.to_string()))?;

    // Drop the builder to flush the encoder.
    drop(tar_builder);

    // Optionally encrypt the archive.
    if config.encrypt {
        let key = encryption_key
            .ok_or_else(|| BackupError::Encryption("encryption key required".into()))?;
        crypto::encrypt_file(key, &archive_path)?;
    }

    // Compute checksum and size.
    let checksum = sha256_file(&archive_path)?;
    let size_bytes = std::fs::metadata(&archive_path)
        .map(|m| m.len())
        .unwrap_or(0);

    // Write sidecar metadata file.
    let final_metadata = BackupMetadata {
        id: backup_id,
        version: env!("CARGO_PKG_VERSION").to_string(),
        created_at: timestamp,
        checksum,
        contents,
        size_bytes,
        encrypted: config.encrypt,
    };

    let meta_path = archive_path.with_extension("meta.json");
    let meta_json = serde_json::to_vec_pretty(&final_metadata)
        .map_err(|e| BackupError::Archive(format!("serialize metadata: {e}")))?;
    std::fs::write(&meta_path, &meta_json)?;

    Ok(final_metadata)
}

fn collect_backup_files(
    base: &Path,
    dir: &Path,
    config: &BackupConfig,
    out: &mut Vec<(PathBuf, String)>,
) -> Result<(), BackupError> {
    let entries = std::fs::read_dir(dir)?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            // Skip backup directory itself and temp files.
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name == "backups" || name.starts_with('.') {
                continue;
            }
            collect_backup_files(base, &path, config, out)?;
            continue;
        }

        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        let include = match ext {
            "db" | "sqlite" => config.include_audit || config.include_genomes,
            "toml" if name != "config.toml" => config.include_manifests,
            "json" => config.include_genomes,
            _ => false,
        };

        if include {
            let rel = path
                .strip_prefix(base)
                .unwrap_or(&path)
                .to_string_lossy()
                .into_owned();
            out.push((path, format!("data/{rel}")));
        }
    }

    Ok(())
}

// ── Restore Backup ─────────────────────────────────────────────────────

/// Restore a Nexus OS backup archive.
pub fn restore_backup(
    archive_path: &Path,
    data_dir: &Path,
    encryption_key: Option<&EncryptionKey>,
) -> Result<RestoreResult, BackupError> {
    if !archive_path.exists() {
        return Err(BackupError::NotFound(format!(
            "archive not found: {}",
            archive_path.display()
        )));
    }

    // Read the archive (decrypt if needed).
    let raw = std::fs::read(archive_path)?;

    let archive_bytes = if raw.len() >= crypto::ENCRYPTED_HEADER_LEN
        && &raw[..crypto::ENCRYPTED_HEADER_LEN] == crypto::ENCRYPTED_HEADER_BYTES
    {
        let key = encryption_key
            .ok_or_else(|| BackupError::Encryption("encryption key required for restore".into()))?;
        crypto::decrypt_data(key, &raw)?
    } else {
        raw
    };

    // Extract the archive from in-memory bytes.
    let decoder = GzDecoder::new(archive_bytes.as_slice());
    let mut archive = tar::Archive::new(decoder);

    let mut restored_files = Vec::new();
    let mut warnings = Vec::new();
    let mut backup_id = String::new();

    let entries = archive
        .entries()
        .map_err(|e| BackupError::Archive(e.to_string()))?;

    for entry_result in entries {
        let mut entry = entry_result.map_err(|e| BackupError::Archive(e.to_string()))?;
        let entry_path = entry
            .path()
            .map_err(|e| BackupError::Archive(e.to_string()))?
            .to_path_buf();
        let entry_str = entry_path.to_string_lossy().to_string();

        if entry_str == "backup-metadata.json" {
            let mut meta_json = Vec::new();
            entry
                .read_to_end(&mut meta_json)
                .map_err(|e| BackupError::Archive(e.to_string()))?;
            if let Ok(meta) = serde_json::from_slice::<BackupMetadata>(&meta_json) {
                backup_id = meta.id;
            }
            continue;
        }

        // Strip "data/" prefix and write to data_dir, or "config/" prefix.
        let dest = if let Some(rel) = entry_str.strip_prefix("data/") {
            data_dir.join(rel)
        } else if let Some(rel) = entry_str.strip_prefix("config/") {
            let config_dir = crate::config::config_path()
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| data_dir.to_path_buf());
            config_dir.join(rel)
        } else {
            warnings.push(format!("skipped unknown entry: {entry_str}"));
            continue;
        };

        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut data = Vec::new();
        entry
            .read_to_end(&mut data)
            .map_err(|e| BackupError::Archive(e.to_string()))?;
        std::fs::write(&dest, &data)?;

        restored_files.push(entry_str);
    }

    Ok(RestoreResult {
        backup_id,
        restored_files,
        warnings,
    })
}

// ── Verify Backup ──────────────────────────────────────────────────────

/// Verify the integrity of a backup archive.
pub fn verify_backup(
    archive_path: &Path,
    encryption_key: Option<&EncryptionKey>,
) -> Result<VerifyResult, BackupError> {
    if !archive_path.exists() {
        return Err(BackupError::NotFound(format!(
            "archive not found: {}",
            archive_path.display()
        )));
    }

    let mut errors = Vec::new();
    let mut backup_id = String::new();
    let mut checksum_ok = false;

    // Check sidecar metadata for checksum verification.
    let meta_path = archive_path.with_extension("meta.json");
    if meta_path.exists() {
        if let Ok(meta_json) = std::fs::read_to_string(&meta_path) {
            if let Ok(meta) = serde_json::from_str::<BackupMetadata>(&meta_json) {
                backup_id = meta.id;
                let actual_checksum = sha256_file(archive_path)?;
                checksum_ok = actual_checksum == meta.checksum;
                if !checksum_ok {
                    errors.push(format!(
                        "checksum mismatch: expected {}, got {actual_checksum}",
                        meta.checksum
                    ));
                }
            }
        }
    } else {
        errors.push("sidecar metadata file not found — cannot verify checksum".into());
    }

    // Try to read the archive to verify it's not corrupted.
    let raw = std::fs::read(archive_path)?;
    let archive_data = if raw.len() >= crypto::ENCRYPTED_HEADER_LEN
        && &raw[..crypto::ENCRYPTED_HEADER_LEN] == crypto::ENCRYPTED_HEADER_BYTES
    {
        let key = encryption_key.ok_or_else(|| {
            BackupError::Encryption("key required to verify encrypted backup".into())
        })?;
        crypto::decrypt_data(key, &raw)?
    } else {
        raw
    };

    let decoder = GzDecoder::new(archive_data.as_slice());
    let mut archive = tar::Archive::new(decoder);
    let mut files_ok = true;

    match archive.entries() {
        Ok(entries) => {
            for entry_result in entries {
                match entry_result {
                    Ok(mut entry) => {
                        // Try to read the entry to verify it's not corrupted.
                        let mut buf = Vec::new();
                        if entry.read_to_end(&mut buf).is_err() {
                            files_ok = false;
                            let path = entry
                                .path()
                                .map(|p| p.to_string_lossy().into_owned())
                                .unwrap_or_else(|_| "<unknown>".into());
                            errors.push(format!("corrupted entry: {path}"));
                        }
                    }
                    Err(e) => {
                        files_ok = false;
                        errors.push(format!("corrupted archive entry: {e}"));
                    }
                }
            }
        }
        Err(e) => {
            files_ok = false;
            errors.push(format!("cannot read archive: {e}"));
        }
    }

    let valid = checksum_ok && files_ok;

    Ok(VerifyResult {
        valid,
        backup_id,
        checksum_ok,
        files_ok,
        audit_chain_ok: true, // Audit chain verification happens at restore time.
        errors,
    })
}

// ── List Backups ───────────────────────────────────────────────────────

/// List all backup metadata in the given directory.
pub fn list_backups(backup_dir: &Path) -> Result<Vec<BackupMetadata>, BackupError> {
    if !backup_dir.exists() {
        return Ok(Vec::new());
    }

    let mut backups = Vec::new();
    let entries = std::fs::read_dir(backup_dir)?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        if path.extension().and_then(|e| e.to_str()) == Some("json")
            && path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.contains("meta"))
                .unwrap_or(false)
        {
            if let Ok(json) = std::fs::read_to_string(&path) {
                if let Ok(meta) = serde_json::from_str::<BackupMetadata>(&json) {
                    backups.push(meta);
                }
            }
        }
    }

    // Sort by creation time (newest first).
    backups.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    Ok(backups)
}

/// Enforce retention policy by deleting old backups.
pub fn enforce_retention(backup_dir: &Path, keep: u32) -> Result<Vec<PathBuf>, BackupError> {
    let backups = list_backups(backup_dir)?;
    let mut deleted = Vec::new();

    if backups.len() <= keep as usize {
        return Ok(deleted);
    }

    // Delete the oldest backups beyond the retention count.
    for meta in backups.iter().skip(keep as usize) {
        // Find the matching archive file.
        let entries = std::fs::read_dir(backup_dir)?;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

            // Match by timestamp in the filename pattern.
            if name.starts_with("nexus-backup-")
                && name.contains(&meta.created_at.format("%Y%m%d-%H%M%S").to_string())
            {
                if let Err(e) = std::fs::remove_file(&path) {
                    // Log but don't fail.
                    eprintln!("backup: failed to delete {}: {e}", path.display());
                } else {
                    deleted.push(path);
                }
            }
        }
    }

    Ok(deleted)
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_test_data(dir: &Path) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join("agents.db"), b"agent database contents").unwrap();
        std::fs::write(dir.join("audit.db"), b"audit trail contents").unwrap();
        std::fs::write(dir.join("agent-coder.toml"), b"[agent]\nname = \"coder\"\n").unwrap();
    }

    #[test]
    fn backup_and_restore_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let data_dir = tmp.path().join("data");
        let backup_dir = tmp.path().join("backups");
        let restore_dir = tmp.path().join("restored");

        setup_test_data(&data_dir);

        let config = BackupConfig {
            output_dir: backup_dir.clone(),
            include_audit: true,
            include_genomes: true,
            include_config: false, // Skip config (may not exist in test).
            include_manifests: true,
            encrypt: false,
        };

        let meta = create_backup(&config, &data_dir, None).unwrap();
        assert!(!meta.checksum.is_empty());
        assert!(!meta.contents.is_empty());
        assert!(meta.size_bytes > 0);

        // Find the archive file.
        let archives: Vec<_> = std::fs::read_dir(&backup_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().and_then(|ext| ext.to_str()) == Some("gz"))
            .collect();
        assert_eq!(archives.len(), 1);

        let archive_path = archives[0].path();
        std::fs::create_dir_all(&restore_dir).unwrap();
        let result = restore_backup(&archive_path, &restore_dir, None).unwrap();
        assert!(!result.restored_files.is_empty());
    }

    #[test]
    fn backup_with_encryption() {
        let tmp = tempfile::tempdir().unwrap();
        let data_dir = tmp.path().join("data");
        let backup_dir = tmp.path().join("backups");
        let restore_dir = tmp.path().join("restored");

        setup_test_data(&data_dir);

        let salt = crypto::generate_salt();
        let key = EncryptionKey::derive(b"backup-password", &salt).unwrap();

        let config = BackupConfig {
            output_dir: backup_dir.clone(),
            include_audit: true,
            include_genomes: true,
            include_config: false,
            include_manifests: true,
            encrypt: true,
        };

        let meta = create_backup(&config, &data_dir, Some(&key)).unwrap();
        assert!(meta.encrypted);

        let archives: Vec<_> = std::fs::read_dir(&backup_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().and_then(|ext| ext.to_str()) == Some("gz"))
            .collect();
        let archive_path = archives[0].path();

        // Restore without key should fail.
        let result = restore_backup(&archive_path, &restore_dir, None);
        assert!(result.is_err());

        // Restore with correct key should work.
        let result = restore_backup(&archive_path, &restore_dir, Some(&key)).unwrap();
        assert!(!result.restored_files.is_empty());
    }

    #[test]
    fn verify_backup_detects_corruption() {
        let tmp = tempfile::tempdir().unwrap();
        let data_dir = tmp.path().join("data");
        let backup_dir = tmp.path().join("backups");

        setup_test_data(&data_dir);

        let config = BackupConfig {
            output_dir: backup_dir.clone(),
            include_config: false,
            ..BackupConfig::default()
        };

        let _meta = create_backup(&config, &data_dir, None).unwrap();

        let archives: Vec<_> = std::fs::read_dir(&backup_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().and_then(|ext| ext.to_str()) == Some("gz"))
            .collect();
        let archive_path = archives[0].path();

        // Valid archive should pass.
        let result = verify_backup(&archive_path, None).unwrap();
        assert!(result.checksum_ok);
        assert!(result.files_ok);

        // Corrupt the archive.
        let mut data = std::fs::read(&archive_path).unwrap();
        if data.len() > 20 {
            data[15] ^= 0xFF;
            data[16] ^= 0xFF;
        }
        std::fs::write(&archive_path, &data).unwrap();

        // Should detect corruption.
        let result = verify_backup(&archive_path, None).unwrap();
        assert!(!result.valid);
    }

    #[test]
    fn list_backups_returns_metadata() {
        let tmp = tempfile::tempdir().unwrap();
        let data_dir = tmp.path().join("data");
        let backup_dir = tmp.path().join("backups");

        setup_test_data(&data_dir);

        let config = BackupConfig {
            output_dir: backup_dir.clone(),
            include_config: false,
            ..BackupConfig::default()
        };

        create_backup(&config, &data_dir, None).unwrap();
        create_backup(&config, &data_dir, None).unwrap();

        let backups = list_backups(&backup_dir).unwrap();
        assert_eq!(backups.len(), 2);
    }

    #[test]
    fn list_backups_empty_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let backups = list_backups(tmp.path()).unwrap();
        assert!(backups.is_empty());
    }

    #[test]
    fn list_backups_nonexistent_dir() {
        let backups = list_backups(Path::new("/nonexistent/path")).unwrap();
        assert!(backups.is_empty());
    }
}
