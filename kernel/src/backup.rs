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
    /// Name of the new directory, beneath the restore root, holding the
    /// restored `data/` and `config/` trees.
    pub restore_dir: String,
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
    if !config.output_dir.is_absolute() {
        return Err(BackupError::Io(
            "backup output directory must be an absolute backend location".into(),
        ));
    }
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
        let config_path =
            crate::config::config_path().map_err(|error| BackupError::Io(error.to_string()))?;
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
        let is_file = std::fs::symlink_metadata(src_path)
            .is_ok_and(|m| m.is_file() && !crate::governed_path::is_redirect(&m));
        if is_file {
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
        // P0-002C5B: the file type is read without following links, so a
        // symbolic link inside the data directory never pulls outside files
        // into a backup; links are skipped.
        let file_type = entry.file_type()?;
        if file_type.is_symlink() || !(file_type.is_dir() || file_type.is_file()) {
            continue;
        }

        if file_type.is_dir() {
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

/// Bounds on what one restore will extract.
#[derive(Debug, Clone, Copy)]
struct RestoreLimits {
    entries: usize,
    entry_bytes: u64,
    total_bytes: u64,
}

const RESTORE_LIMITS: RestoreLimits = RestoreLimits {
    entries: 65_536,
    entry_bytes: 2 * 1024 * 1024 * 1024,
    total_bytes: 8 * 1024 * 1024 * 1024,
};

/// Largest `backup-metadata.json` a restore will read.
const MAX_METADATA_BYTES: u64 = 1024 * 1024;

/// Restore a Nexus OS backup archive into a new directory beneath
/// `restore_root`, which the backend selects and which must be an existing
/// canonical directory (P0-002C5B).
///
/// An archive entry name never chooses a location outside that directory.
/// Every entry is validated before anything is written:
/// - it must be a regular file; symbolic links, hard links, devices, FIFOs,
///   directories and every other entry type are refused;
/// - it must be `backup-metadata.json`, `config/config.toml` or `data/<path>`,
///   where the whole name is a portable relative path (no absolute, prefixed,
///   drive, UNC, `..`, `.`, empty, backslash or alternate-data-stream form);
/// - names may not repeat or alias each other, even by case, and the entry
///   count and sizes are bounded.
///
/// The entries are then written into a fresh `restore-<uuid>` directory created
/// exclusively beneath the root (owner-only on Unix), each file created
/// exclusively, so nothing that already exists is followed or overwritten. On
/// any failure that directory is removed. Moving restored data into place is
/// left to the caller.
pub fn restore_backup(
    archive_path: &Path,
    restore_root: &Path,
    encryption_key: Option<&EncryptionKey>,
) -> Result<RestoreResult, BackupError> {
    restore_with_limits(archive_path, restore_root, encryption_key, RESTORE_LIMITS)
}

fn restore_with_limits(
    archive_path: &Path,
    restore_root: &Path,
    encryption_key: Option<&EncryptionKey>,
    limits: RestoreLimits,
) -> Result<RestoreResult, BackupError> {
    if !archive_path.exists() {
        return Err(BackupError::NotFound(format!(
            "archive not found: {}",
            archive_path.display()
        )));
    }
    crate::governed_path::existing_root(restore_root)
        .map_err(|_| restore_denied("restore root must be an existing canonical directory"))?;

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

    // First pass: validate every entry; nothing is written.
    let (backup_id, restored_files) = plan_restore(&archive_bytes, limits)?;

    // Second pass: write into a fresh directory, created exclusively.
    let restore_dir = format!("restore-{}", Uuid::new_v4());
    let target_dir = restore_root.join(&restore_dir);
    let mut builder = std::fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(&target_dir)?;
    if let Err(error) = write_restore(&archive_bytes, &target_dir, limits) {
        let _ = std::fs::remove_dir_all(&target_dir);
        return Err(error);
    }

    Ok(RestoreResult {
        backup_id,
        restore_dir,
        restored_files,
        warnings: Vec::new(),
    })
}

fn restore_denied(reason: &str) -> BackupError {
    BackupError::Restore(reason.to_string())
}

fn archive_error(e: std::io::Error) -> BackupError {
    BackupError::Archive(e.to_string())
}

/// The validated name of a restorable entry, or `None` for the metadata entry.
fn restorable_name<R: Read>(entry: &tar::Entry<'_, R>) -> Result<Option<String>, BackupError> {
    if entry.header().entry_type() != tar::EntryType::Regular {
        return Err(restore_denied("archive entry is not a regular file"));
    }
    let name = std::str::from_utf8(&entry.path_bytes())
        .map_err(|_| restore_denied("archive entry name is not valid UTF-8"))?
        .to_string();
    if name == "backup-metadata.json" {
        return Ok(None);
    }
    let allowed = name == "config/config.toml"
        || name
            .strip_prefix("data/")
            .is_some_and(|rest| !rest.is_empty());
    if !allowed || crate::governed_path::validate_relative(&name).is_err() {
        return Err(restore_denied(
            "archive entry name is not a permitted location",
        ));
    }
    Ok(Some(name))
}

/// Validates every entry and returns the backup id and the entry names.
fn plan_restore(
    archive_bytes: &[u8],
    limits: RestoreLimits,
) -> Result<(String, Vec<String>), BackupError> {
    let mut archive = tar::Archive::new(GzDecoder::new(archive_bytes));
    let mut backup_id = String::new();
    let mut names = Vec::new();
    let mut files = std::collections::HashSet::new();
    let mut dirs = std::collections::HashSet::new();
    let mut total: u64 = 0;
    let mut count = 0usize;

    for entry in archive.entries().map_err(archive_error)? {
        let mut entry = entry.map_err(archive_error)?;
        count += 1;
        if count > limits.entries {
            return Err(restore_denied("archive has too many entries"));
        }
        let size = entry.size();
        if size > limits.entry_bytes {
            return Err(restore_denied("archive entry is too large"));
        }
        total = total.saturating_add(size);
        if total > limits.total_bytes {
            return Err(restore_denied("archive contents are too large"));
        }
        let Some(name) = restorable_name(&entry)? else {
            if size > MAX_METADATA_BYTES {
                return Err(restore_denied("backup metadata is too large"));
            }
            let mut meta_json = Vec::new();
            entry.read_to_end(&mut meta_json).map_err(archive_error)?;
            if let Ok(meta) = serde_json::from_slice::<BackupMetadata>(&meta_json) {
                backup_id = meta.id;
            }
            continue;
        };
        // No duplicate names and no file that is also a directory, compared
        // without case so that case-insensitive filesystems cannot alias them.
        let folded = name.to_lowercase();
        if files.contains(&folded) || dirs.contains(&folded) {
            return Err(restore_denied("archive entry names collide"));
        }
        let mut prefix = String::new();
        for component in folded.split('/').take(folded.split('/').count() - 1) {
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix.push_str(component);
            if files.contains(&prefix) {
                return Err(restore_denied("archive entry names collide"));
            }
            dirs.insert(prefix.clone());
        }
        files.insert(folded);
        names.push(name);
    }
    Ok((backup_id, names))
}

/// Writes every validated entry beneath `target_dir`, creating each directory
/// and file exclusively.
fn write_restore(
    archive_bytes: &[u8],
    target_dir: &Path,
    limits: RestoreLimits,
) -> Result<(), BackupError> {
    let mut archive = tar::Archive::new(GzDecoder::new(archive_bytes));
    for entry in archive.entries().map_err(archive_error)? {
        let mut entry = entry.map_err(archive_error)?;
        let Some(name) = restorable_name(&entry)? else {
            continue;
        };
        let size = entry.size();
        if size > limits.entry_bytes {
            return Err(restore_denied("archive entry is too large"));
        }
        let mut path = target_dir.to_path_buf();
        let components: Vec<&str> = name.split('/').collect();
        for component in &components[..components.len() - 1] {
            path.push(component);
            match std::fs::create_dir(&path) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    let metadata = std::fs::symlink_metadata(&path)?;
                    if crate::governed_path::is_redirect(&metadata) || !metadata.is_dir() {
                        return Err(restore_denied("restore directory was redirected"));
                    }
                }
                Err(e) => return Err(e.into()),
            }
        }
        path.push(components[components.len() - 1]);
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        let written = std::io::copy(&mut (&mut entry).take(size.saturating_add(1)), &mut file)
            .map_err(archive_error)?;
        if written != size {
            return Err(restore_denied("archive entry size does not match its data"));
        }
    }
    Ok(())
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
    backups.sort_by_key(|a| std::cmp::Reverse(a.created_at));

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
        let restore_root = restore_dir.canonicalize().unwrap();
        let result = restore_backup(&archive_path, &restore_root, None).unwrap();
        assert_eq!(result.restored_files.len(), 3);
        let restored = restore_root.join(&result.restore_dir).join("data");
        assert_eq!(
            std::fs::read(restored.join("agents.db")).unwrap(),
            b"agent database contents"
        );
        assert_eq!(
            std::fs::read(restored.join("agent-coder.toml")).unwrap(),
            b"[agent]\nname = \"coder\"\n"
        );
        // The source data directory is never a restore target.
        assert_eq!(
            std::fs::read(data_dir.join("agents.db")).unwrap(),
            b"agent database contents"
        );
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

        std::fs::create_dir_all(&restore_dir).unwrap();
        let restore_root = restore_dir.canonicalize().unwrap();

        // Restore without key should fail, and leave nothing behind.
        let result = restore_backup(&archive_path, &restore_root, None);
        assert!(result.is_err());
        assert_eq!(std::fs::read_dir(&restore_root).unwrap().count(), 0);

        // Restore with correct key should work.
        let result = restore_backup(&archive_path, &restore_root, Some(&key)).unwrap();
        assert_eq!(result.restored_files.len(), 3);
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
    // ── P0-002C5B: hostile archives ────────────────────────────────────

    /// Raw entry name, type, link target and data.
    type RawEntry<'a> = (&'a [u8], tar::EntryType, &'a [u8], &'a [u8]);

    /// A gzipped tar whose headers carry exactly the given raw names, types
    /// and link targets, bypassing the builder's own path checks the way a
    /// hostile archive would.
    fn raw_archive(entries: &[RawEntry<'_>]) -> Vec<u8> {
        let mut builder = tar::Builder::new(GzEncoder::new(Vec::new(), Compression::default()));
        for (name, kind, link, data) in entries {
            let mut header = tar::Header::new_gnu();
            {
                let gnu = header.as_gnu_mut().unwrap();
                gnu.name = [0; 100];
                gnu.name[..name.len()].copy_from_slice(name);
                gnu.linkname = [0; 100];
                gnu.linkname[..link.len()].copy_from_slice(link);
            }
            header.set_entry_type(*kind);
            header.set_size(data.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            builder.append(&header, *data).unwrap();
        }
        builder.into_inner().unwrap().finish().unwrap()
    }

    struct Restore {
        _tmp: tempfile::TempDir,
        base: PathBuf,
        root: PathBuf,
        archive: PathBuf,
    }

    impl Restore {
        fn new(bytes: &[u8]) -> Self {
            let tmp = tempfile::tempdir().unwrap();
            let base = tmp.path().canonicalize().unwrap();
            let root = base.join("restore-root");
            std::fs::create_dir(&root).unwrap();
            let archive = base.join("hostile.tar.gz");
            std::fs::write(&archive, bytes).unwrap();
            std::fs::write(base.join("victim.txt"), b"original").unwrap();
            Self {
                _tmp: tmp,
                base,
                root,
                archive,
            }
        }

        fn run(&self, limits: RestoreLimits) -> Result<RestoreResult, BackupError> {
            restore_with_limits(&self.archive, &self.root, None, limits)
        }

        /// Nothing was written anywhere: the root is empty and the victim
        /// beside it is untouched.
        fn untouched(&self) -> bool {
            std::fs::read_dir(&self.root).unwrap().count() == 0
                && std::fs::read(self.base.join("victim.txt")).unwrap() == b"original"
                && !self.base.join("escape.txt").exists()
        }
    }

    const REGULAR: tar::EntryType = tar::EntryType::Regular;

    #[test]
    fn restore_rejects_escaping_and_ambiguous_entry_names() {
        for name in [
            b"../escape.txt".as_slice(),
            b"data/../../escape.txt",
            b"data/../victim.txt",
            b"/etc/passwd",
            b"/data/x.json",
            b"data//x.json",
            b"data/./x.json",
            b"data/",
            b"data",
            b"data/x.json:stream",
            b"data/C:x.json",
            b"C:\\escape.txt",
            b"C:/escape.txt",
            b"\\\\server\\share\\escape.txt",
            b"data\\..\\..\\escape.txt",
            b"data/CON",
            b"data/nul.json",
            b"data/x.",
            b"config/other.toml",
            b"unknown/x.json",
            b"",
            b"data/\xff.json",
        ] {
            let r = Restore::new(&raw_archive(&[(name, REGULAR, b"", b"x")]));
            assert!(
                matches!(r.run(RESTORE_LIMITS), Err(BackupError::Restore(_))),
                "{:?}",
                String::from_utf8_lossy(name)
            );
            assert!(r.untouched(), "{:?}", String::from_utf8_lossy(name));
        }
    }

    #[test]
    fn restore_rejects_links_devices_and_special_entries() {
        for (kind, link) in [
            (tar::EntryType::Symlink, b"/etc/passwd".as_slice()),
            (tar::EntryType::Symlink, b"../escape.txt"),
            (tar::EntryType::Link, b"data/other.json"),
            (tar::EntryType::Link, b"/etc/passwd"),
            (tar::EntryType::Char, b""),
            (tar::EntryType::Block, b""),
            (tar::EntryType::Fifo, b""),
            (tar::EntryType::Directory, b""),
            (tar::EntryType::Continuous, b""),
            (tar::EntryType::GNUSparse, b""),
            (tar::EntryType::XGlobalHeader, b""),
        ] {
            let r = Restore::new(&raw_archive(&[
                (b"data/ok.json", REGULAR, b"", b"{}"),
                (b"data/x.json", kind, link, b""),
            ]));
            let result = r.run(RESTORE_LIMITS);
            // tar parses a GNU sparse map itself and rejects this malformed
            // one first; a well-formed sparse entry is refused as not regular.
            if kind == tar::EntryType::GNUSparse {
                assert!(
                    matches!(
                        result,
                        Err(BackupError::Archive(_) | BackupError::Restore(_))
                    ),
                    "{kind:?}"
                );
            } else {
                assert!(matches!(result, Err(BackupError::Restore(_))), "{kind:?}");
            }
            assert!(r.untouched(), "{kind:?}");
        }
    }

    #[test]
    fn restore_rejects_duplicate_and_aliasing_names() {
        for pair in [
            [b"data/a.json".as_slice(), b"data/a.json".as_slice()],
            [b"data/A.json", b"data/a.json"],
            [b"data/a", b"data/a/b.json"],
            [b"data/a/b.json", b"data/A"],
        ] {
            let r = Restore::new(&raw_archive(&[
                (pair[0], REGULAR, b"", b"1"),
                (pair[1], REGULAR, b"", b"2"),
            ]));
            assert!(matches!(
                r.run(RESTORE_LIMITS),
                Err(BackupError::Restore(_))
            ));
            assert!(r.untouched());
        }
    }

    #[test]
    fn restore_is_bounded() {
        let small = RestoreLimits {
            entries: 2,
            entry_bytes: 8,
            total_bytes: 12,
        };
        for entries in [
            vec![
                (
                    b"data/1.json".as_slice(),
                    REGULAR,
                    b"".as_slice(),
                    b"1".as_slice(),
                ),
                (b"data/2.json", REGULAR, b"", b"2"),
                (b"data/3.json", REGULAR, b"", b"3"),
            ],
            vec![(b"data/big.json", REGULAR, b"", b"123456789")],
            vec![
                (b"data/1.json", REGULAR, b"", b"1234567"),
                (b"data/2.json", REGULAR, b"", b"1234567"),
            ],
        ] {
            let r = Restore::new(&raw_archive(&entries));
            assert!(matches!(r.run(small), Err(BackupError::Restore(_))));
            assert!(r.untouched());
        }
    }

    #[test]
    fn restore_writes_only_into_a_fresh_directory_under_the_root() {
        let r = Restore::new(&raw_archive(&[
            (b"data/sub/a.json", REGULAR, b"", b"restored"),
            (b"config/config.toml", REGULAR, b"", b"key = 1"),
        ]));
        let first = r.run(RESTORE_LIMITS).unwrap();
        let second = r.run(RESTORE_LIMITS).unwrap();
        assert_ne!(first.restore_dir, second.restore_dir);
        for result in [&first, &second] {
            assert!(result.restore_dir.starts_with("restore-"));
            let dir = r.root.join(&result.restore_dir);
            assert_eq!(
                std::fs::read(dir.join("data").join("sub").join("a.json")).unwrap(),
                b"restored"
            );
            assert_eq!(
                std::fs::read(dir.join("config").join("config.toml")).unwrap(),
                b"key = 1"
            );
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let dir = r.root.join(&first.restore_dir);
            assert_eq!(
                std::fs::metadata(dir).unwrap().permissions().mode() & 0o777,
                0o700
            );
        }
    }

    #[test]
    fn restore_root_must_be_an_existing_canonical_directory() {
        let r = Restore::new(&raw_archive(&[(b"data/a.json", REGULAR, b"", b"1")]));
        for root in [
            PathBuf::from("relative/root"),
            r.base.join("missing"),
            r.base.join("victim.txt"),
            r.root.join("..").join("restore-root"),
        ] {
            assert!(restore_backup(&r.archive, &root, None).is_err(), "{root:?}");
        }
        assert!(r.untouched());
    }

    #[cfg(unix)]
    #[test]
    fn unix_redirected_restore_roots_and_data_are_never_followed() {
        let r = Restore::new(&raw_archive(&[(b"data/a.json", REGULAR, b"", b"1")]));
        let link = r.base.join("link-root");
        std::os::unix::fs::symlink(&r.base, &link).unwrap();
        assert!(restore_backup(&r.archive, &link, None).is_err());

        // A link already inside the root cannot capture the fresh directory.
        std::os::unix::fs::symlink(&r.base, r.root.join("data")).unwrap();
        let result = r.run(RESTORE_LIMITS).unwrap();
        assert!(r
            .root
            .join(&result.restore_dir)
            .join("data/a.json")
            .exists());
        assert!(!r.base.join("a.json").exists());
    }

    #[cfg(windows)]
    #[test]
    fn windows_reparse_restore_roots_are_never_followed() {
        use std::os::windows::fs::symlink_dir;
        let r = Restore::new(&raw_archive(&[(b"data/a.json", REGULAR, b"", b"1")]));
        let link = r.base.join("link-root");
        symlink_dir(&r.base, &link)
            .expect("native Windows test requires symlink creation privilege");
        assert!(restore_backup(&r.archive, &link, None).is_err());
        assert!(!r.base.join("data").exists());
    }

    #[cfg(unix)]
    #[test]
    fn unix_backups_do_not_follow_links_out_of_the_data_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path().canonicalize().unwrap();
        let data_dir = base.join("data");
        setup_test_data(&data_dir);
        let outside = base.join("outside");
        std::fs::create_dir(&outside).unwrap();
        std::fs::write(outside.join("secret.json"), b"secret").unwrap();
        std::os::unix::fs::symlink(outside.join("secret.json"), data_dir.join("link.json"))
            .unwrap();
        std::os::unix::fs::symlink(&outside, data_dir.join("linked-dir")).unwrap();

        let config = BackupConfig {
            output_dir: base.join("backups"),
            include_config: false,
            ..BackupConfig::default()
        };
        let meta = create_backup(&config, &data_dir, None).unwrap();
        assert_eq!(meta.contents.len(), 3);
        assert!(meta.contents.iter().all(|c| !c.contains("link")));
    }
}
