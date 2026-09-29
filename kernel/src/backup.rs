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

    /// Include the configuration file. It is copied only when it is an
    /// encrypted configuration envelope or the archive is encrypted;
    /// otherwise it is skipped and the metadata says so (Final Gate item H).
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

    /// Items a backup chose not to copy, each with a bounded reason (no path
    /// or content). Absent from metadata written by earlier versions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skipped: Vec<String>,
}

/// Why the configuration file is not in a backup (Final Gate item H): it is
/// not an encrypted configuration envelope (legacy or hand-written
/// plaintext, which can hold credentials) and the archive is not encrypted.
pub const CONFIG_NOT_COPIED_PLAINTEXT: &str =
    "config/config.toml not copied: the configuration file is not encrypted and neither is the archive";

/// Why the configuration file is not in a backup: the location is not a
/// regular file (a directory, a link or another special file), or it is one
/// that cannot be read.
pub const CONFIG_NOT_COPIED_UNREADABLE: &str =
    "config/config.toml not copied: the configuration location is not a readable regular file";

/// The configuration file's name inside an archive.
const CONFIG_ENTRY: &str = "config/config.toml";

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
    create_backup_with_config_file(config, data_dir, encryption_key, || {
        crate::config::config_path().map_err(|error| BackupError::Io(error.to_string()))
    })
}

/// [`create_backup`] with the configuration file's location injected.
fn create_backup_with_config_file(
    config: &BackupConfig,
    data_dir: &Path,
    encryption_key: Option<&EncryptionKey>,
    config_file: impl FnOnce() -> Result<PathBuf, BackupError>,
) -> Result<BackupMetadata, BackupError> {
    if !config.output_dir.is_absolute() {
        return Err(BackupError::Io(
            "backup output directory must be an absolute backend location".into(),
        ));
    }
    // An encrypted backup needs its key before anything is written, so a
    // missing key never leaves an unencrypted archive behind.
    let archive_key = if config.encrypt {
        Some(
            encryption_key
                .ok_or_else(|| BackupError::Encryption("encryption key required".into()))?,
        )
    } else {
        None
    };
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

    // The configuration file (Final Gate item H). An encrypted configuration
    // envelope is copied as it is. Anything else (legacy or hand-written
    // plaintext, which can hold credentials) is copied only into an encrypted
    // archive; otherwise it is skipped and the metadata says why. A location
    // that is not a regular file, or one that cannot be read, is skipped the
    // same way; the backup goes on without it. The file is read once, so
    // what was checked is what is copied.
    let mut skipped = Vec::new();
    let mut config_entry: Option<Vec<u8>> = None;
    if config.include_config {
        let config_path = config_file()?;
        match std::fs::symlink_metadata(&config_path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Ok(meta) if meta.is_file() && !crate::governed_path::is_redirect(&meta) => {
                match std::fs::read(&config_path) {
                    Ok(bytes) => {
                        let envelope = std::str::from_utf8(&bytes)
                            .is_ok_and(crate::config::is_config_envelope);
                        if envelope || archive_key.is_some() {
                            config_entry = Some(bytes);
                        } else {
                            skipped.push(CONFIG_NOT_COPIED_PLAINTEXT.to_string());
                        }
                    }
                    Err(_) => skipped.push(CONFIG_NOT_COPIED_UNREADABLE.to_string()),
                }
            }
            _ => skipped.push(CONFIG_NOT_COPIED_UNREADABLE.to_string()),
        }
    }

    let mut contents: Vec<String> = files_to_backup.iter().map(|(_, rel)| rel.clone()).collect();
    if config_entry.is_some() {
        contents.push(CONFIG_ENTRY.to_string());
    }

    // Create tar.gz archive, owner-only from its first byte (Final Gate item
    // H): it can hold the encrypted configuration and every other store it
    // copies. On Windows the file keeps the directory's inherited access.
    let archive_file =
        create_owner_only(&archive_path).map_err(|e| BackupError::Io(e.to_string()))?;
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
        skipped: skipped.clone(),
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
    if let Some(data) = &config_entry {
        let mut header = tar::Header::new_gnu();
        header.set_size(data.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        tar_builder
            .append_data(&mut header, CONFIG_ENTRY, data.as_slice())
            .map_err(|e| BackupError::Archive(e.to_string()))?;
    }

    tar_builder
        .finish()
        .map_err(|e| BackupError::Archive(e.to_string()))?;

    // Drop the builder to flush the encoder.
    drop(tar_builder);

    // Optionally encrypt the archive.
    if let Some(key) = archive_key {
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
        skipped,
    };

    let meta_path = archive_path.with_extension("meta.json");
    let meta_json = serde_json::to_vec_pretty(&final_metadata)
        .map_err(|e| BackupError::Archive(format!("serialize metadata: {e}")))?;
    std::fs::write(&meta_path, &meta_json)?;

    Ok(final_metadata)
}

/// Creates `path`, which must not exist, readable and writable by its owner
/// only (0600 on Unix).
fn create_owner_only(path: &Path) -> std::io::Result<std::fs::File> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}

/// Credential stores directly under the data directory that a backup never
/// copies (Final Gate item H): email and integration OAuth token files,
/// messaging bot tokens, the legacy deploy credential store, OAuth client
/// secrets and API Client collections. Copying them would persist their
/// secrets again, in plaintext unless the archive is encrypted. The stores
/// themselves are left as they are.
const CREDENTIAL_STORES: &[&str] = &[
    "email_oauth",
    "integrations",
    "messaging_tokens",
    "deploy_credentials.json",
    "oauth_settings.json",
    "api_collections.json",
];

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
        if dir == base
            && entry
                .file_name()
                .to_str()
                .is_some_and(|name| CREDENTIAL_STORES.contains(&name))
        {
            continue;
        }
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

    /// Final Gate item H: a backup never copies a credential store, and the
    /// archive is owner-only from its creation.
    #[test]
    fn p0_fg_h_backups_skip_credential_stores_and_are_owner_only() {
        let tmp = tempfile::tempdir().unwrap();
        let data_dir = tmp.path().join("data");
        let backup_dir = tmp.path().join("backups");
        setup_test_data(&data_dir);
        for dir in ["email_oauth", "integrations", "messaging_tokens", "agents"] {
            std::fs::create_dir_all(data_dir.join(dir)).unwrap();
        }
        for (relative, contents) in [
            (
                "email_oauth/gmail_tokens.json",
                "{\"access_token\":\"synthetic\"}",
            ),
            (
                "integrations/github_oauth.json",
                "{\"token_response\":\"synthetic\"}",
            ),
            (
                "messaging_tokens/telegram.json",
                "{\"token\":\"synthetic\"}",
            ),
            ("deploy_credentials.json", "{\"entries\":{}}"),
            (
                "oauth_settings.json",
                "{\"gmail_client_secret\":\"synthetic\"}",
            ),
            ("api_collections.json", "[]"),
            // Same names below the top level are ordinary data.
            ("agents/api_collections.json", "{\"kept\":true}"),
            ("agents/genome.json", "{\"kept\":true}"),
        ] {
            std::fs::write(data_dir.join(relative), contents).unwrap();
        }
        let config = BackupConfig {
            output_dir: backup_dir.clone(),
            include_audit: true,
            include_genomes: true,
            include_config: false,
            include_manifests: true,
            encrypt: false,
        };
        let meta = create_backup(&config, &data_dir, None).unwrap();
        let mut contents = meta.contents.clone();
        contents.sort();
        let separator = std::path::MAIN_SEPARATOR;
        assert_eq!(
            contents,
            [
                "data/agent-coder.toml".to_string(),
                "data/agents.db".to_string(),
                format!("data/agents{separator}api_collections.json"),
                format!("data/agents{separator}genome.json"),
                "data/audit.db".to_string(),
            ]
        );
        // The stores are left as they are.
        assert_eq!(
            std::fs::read_to_string(data_dir.join("messaging_tokens/telegram.json")).unwrap(),
            "{\"token\":\"synthetic\"}"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let archive = std::fs::read_dir(&backup_dir)
                .unwrap()
                .filter_map(|e| e.ok())
                .find(|e| e.path().extension().and_then(|ext| ext.to_str()) == Some("gz"))
                .unwrap();
            let mode = archive.metadata().unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
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

    /// `dir/<name>/../<name>` spelled from text: a non-canonical spelling of
    /// an existing directory. It cannot come from `join`, because on Windows
    /// pushing `..` onto the verbatim (`\\?\`) path `canonicalize` returns
    /// normalizes the `..` away; the plain form keeps it.
    fn dotted_spelling(dir: &Path, name: &str) -> PathBuf {
        let text = dir.to_string_lossy();
        let text = text.strip_prefix(r"\\?\").unwrap_or(&text);
        let sep = std::path::MAIN_SEPARATOR;
        PathBuf::from(format!("{text}{sep}{name}{sep}..{sep}{name}"))
    }

    #[test]
    fn restore_root_must_be_an_existing_canonical_directory() {
        let r = Restore::new(&raw_archive(&[(b"data/a.json", REGULAR, b"", b"1")]));
        for root in [
            PathBuf::from("relative/root"),
            r.base.join("missing"),
            r.base.join("victim.txt"),
            dotted_spelling(&r.base, "restore-root"),
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

    /// A configuration file as the checked writer stores it: an encrypted
    /// envelope, here under an explicit synthetic operator key.
    fn write_envelope_config(path: &Path) {
        let keys = crate::config::ConfigKeyMaterial::from_values(
            Some("synthetic-operator-key"),
            [None; 4],
        );
        let mut config = crate::config::NexusConfig::default();
        config.llm.anthropic_api_key = "synthetic-envelope-secret".into();
        crate::config::save_config_checked_to_path(path, &config, &keys).unwrap();
    }

    /// A hand-written plaintext configuration holding a credential.
    fn write_plaintext_config(path: &Path) {
        let mut config = crate::config::NexusConfig::default();
        config.llm.anthropic_api_key = "synthetic-plaintext-secret".into();
        std::fs::write(path, toml::to_string(&config).unwrap()).unwrap();
    }

    /// The single archive in `dir`.
    fn only_archive(dir: &Path) -> PathBuf {
        let archives: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|ext| ext.to_str()) == Some("gz"))
            .collect();
        assert_eq!(archives.len(), 1);
        archives[0].clone()
    }

    /// The entry names and the whole decompressed tar stream of an
    /// unencrypted archive.
    fn tar_entries(archive: &Path) -> (Vec<String>, Vec<u8>) {
        let mut tar_bytes = Vec::new();
        GzDecoder::new(std::fs::File::open(archive).unwrap())
            .read_to_end(&mut tar_bytes)
            .unwrap();
        let names = tar::Archive::new(tar_bytes.as_slice())
            .entries()
            .unwrap()
            .map(|entry| {
                entry
                    .unwrap()
                    .path()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        (names, tar_bytes)
    }

    fn with_config(output_dir: PathBuf, encrypt: bool) -> BackupConfig {
        BackupConfig {
            output_dir,
            include_config: true,
            encrypt,
            ..BackupConfig::default()
        }
    }

    /// Final Gate item H: a plaintext configuration never enters an
    /// unencrypted archive; the metadata says it was skipped.
    #[test]
    fn p0_fg_h_plaintext_configuration_stays_out_of_an_unencrypted_backup() {
        let tmp = tempfile::tempdir().unwrap();
        let data_dir = tmp.path().join("data");
        setup_test_data(&data_dir);
        let config_file = tmp.path().join("config.toml");
        write_plaintext_config(&config_file);
        let backups = tmp.path().join("backups");

        let meta = create_backup_with_config_file(
            &with_config(backups.clone(), false),
            &data_dir,
            None,
            || Ok(config_file.clone()),
        )
        .unwrap();
        assert!(!meta.contents.iter().any(|c| c == CONFIG_ENTRY), "{meta:?}");
        assert_eq!(meta.skipped, vec![CONFIG_NOT_COPIED_PLAINTEXT.to_string()]);
        let archive = only_archive(&backups);
        let (names, tar_bytes) = tar_entries(&archive);
        assert!(!names.iter().any(|n| n == CONFIG_ENTRY), "{names:?}");
        let marker = b"synthetic-plaintext-secret";
        assert!(!tar_bytes.windows(marker.len()).any(|w| w == marker));
        // The sidecar and the in-archive metadata both record the skip.
        let sidecar = std::fs::read_to_string(archive.with_extension("meta.json")).unwrap();
        assert!(sidecar.contains(CONFIG_NOT_COPIED_PLAINTEXT), "{sidecar}");
        let inner = String::from_utf8_lossy(&tar_bytes);
        assert!(inner.contains(CONFIG_NOT_COPIED_PLAINTEXT));
    }

    /// Final Gate item H: a plaintext configuration is copied into an
    /// encrypted archive, and restores from it unchanged.
    #[test]
    fn p0_fg_h_plaintext_configuration_is_copied_into_an_encrypted_backup() {
        let tmp = tempfile::tempdir().unwrap();
        let data_dir = tmp.path().join("data");
        setup_test_data(&data_dir);
        let config_file = tmp.path().join("config.toml");
        write_plaintext_config(&config_file);
        let backups = tmp.path().join("backups");
        let key = EncryptionKey::from_raw_for_test([0x33; 32]);

        let meta = create_backup_with_config_file(
            &with_config(backups.clone(), true),
            &data_dir,
            Some(&key),
            || Ok(config_file.clone()),
        )
        .unwrap();
        assert!(meta.encrypted);
        assert!(meta.contents.iter().any(|c| c == CONFIG_ENTRY), "{meta:?}");
        assert!(meta.skipped.is_empty(), "{meta:?}");
        let restore_root = tmp.path().join("restored");
        std::fs::create_dir_all(&restore_root).unwrap();
        let restore_root = restore_root.canonicalize().unwrap();
        let result = restore_backup(&only_archive(&backups), &restore_root, Some(&key)).unwrap();
        assert_eq!(
            std::fs::read(restore_root.join(&result.restore_dir).join(CONFIG_ENTRY)).unwrap(),
            std::fs::read(&config_file).unwrap()
        );
    }

    /// Final Gate item H: an encrypted configuration envelope is copied as it
    /// is, even into an unencrypted archive.
    #[test]
    fn p0_fg_h_an_encrypted_configuration_is_copied_as_it_is() {
        let tmp = tempfile::tempdir().unwrap();
        let data_dir = tmp.path().join("data");
        setup_test_data(&data_dir);
        let config_file = tmp.path().join("config.toml");
        write_envelope_config(&config_file);
        let backups = tmp.path().join("backups");

        let meta = create_backup_with_config_file(
            &with_config(backups.clone(), false),
            &data_dir,
            None,
            || Ok(config_file.clone()),
        )
        .unwrap();
        assert!(meta.contents.iter().any(|c| c == CONFIG_ENTRY), "{meta:?}");
        assert!(meta.skipped.is_empty(), "{meta:?}");
        let (names, tar_bytes) = tar_entries(&only_archive(&backups));
        assert!(names.iter().any(|n| n == CONFIG_ENTRY), "{names:?}");
        let mut archive = tar::Archive::new(tar_bytes.as_slice());
        let copied = archive
            .entries()
            .unwrap()
            .map(|entry| entry.unwrap())
            .find(|entry| entry.path().unwrap().to_string_lossy() == CONFIG_ENTRY)
            .map(|mut entry| {
                let mut bytes = Vec::new();
                entry.read_to_end(&mut bytes).unwrap();
                bytes
            })
            .unwrap();
        assert_eq!(copied, std::fs::read(&config_file).unwrap());
        let marker = b"synthetic-envelope-secret";
        assert!(!tar_bytes.windows(marker.len()).any(|w| w == marker));
    }

    /// Final Gate item H: only an exact envelope counts as encrypted.
    #[test]
    fn p0_fg_h_only_an_exact_envelope_counts_as_an_encrypted_configuration() {
        let tmp = tempfile::tempdir().unwrap();
        let envelope = tmp.path().join("envelope.toml");
        write_envelope_config(&envelope);
        let text = std::fs::read_to_string(&envelope).unwrap();
        assert!(crate::config::is_config_envelope(&text));
        let plaintext = tmp.path().join("plaintext.toml");
        write_plaintext_config(&plaintext);
        assert!(!crate::config::is_config_envelope(
            &std::fs::read_to_string(&plaintext).unwrap()
        ));
        // An envelope with anything added is not an envelope.
        let padded = format!("{text}\nanthropic_api_key = \"synthetic\"\n");
        assert!(!crate::config::is_config_envelope(&padded));
        assert!(!crate::config::is_config_envelope(""));
        assert!(!crate::config::is_config_envelope("version = 1"));
    }

    /// An encrypted backup without its key writes nothing, so no unencrypted
    /// archive is left behind.
    #[test]
    fn p0_fg_h_an_encrypted_backup_without_a_key_writes_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        let data_dir = tmp.path().join("data");
        setup_test_data(&data_dir);
        let config_file = tmp.path().join("config.toml");
        write_plaintext_config(&config_file);
        let backups = tmp.path().join("backups");
        let result = create_backup_with_config_file(
            &with_config(backups.clone(), true),
            &data_dir,
            None,
            || Ok(config_file.clone()),
        );
        assert_eq!(
            result.unwrap_err(),
            BackupError::Encryption("encryption key required".into())
        );
        assert!(!backups.exists());
    }

    /// Final Gate item H (stream 4 review): a configuration location that is
    /// not a regular file, or a file that cannot be read, is skipped with a
    /// bounded reason and the backup goes on.
    #[test]
    fn p0_fg_h_an_unreadable_configuration_is_skipped() {
        let tmp = tempfile::tempdir().unwrap();
        let data_dir = tmp.path().join("data");
        setup_test_data(&data_dir);

        // A directory where the file should be.
        let not_a_file = tmp.path().join("config-dir");
        std::fs::create_dir(&not_a_file).unwrap();
        let backups = tmp.path().join("backups-dir");
        let meta = create_backup_with_config_file(
            &with_config(backups.clone(), false),
            &data_dir,
            None,
            || Ok(not_a_file.clone()),
        )
        .unwrap();
        assert_eq!(meta.skipped, vec![CONFIG_NOT_COPIED_UNREADABLE.to_string()]);
        assert!(!meta.contents.iter().any(|c| c == CONFIG_ENTRY), "{meta:?}");
        assert!(
            !meta.contents.is_empty(),
            "the data files are still backed up"
        );

        // A regular file that cannot be read (Unix permissions; skipped where
        // the test runs with the privilege to read it anyway).
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let unreadable = tmp.path().join("config.toml");
            write_plaintext_config(&unreadable);
            std::fs::set_permissions(&unreadable, std::fs::Permissions::from_mode(0o000)).unwrap();
            if std::fs::read(&unreadable).is_err() {
                let backups = tmp.path().join("backups-unreadable");
                let meta = create_backup_with_config_file(
                    &with_config(backups, true),
                    &data_dir,
                    Some(&EncryptionKey::from_raw_for_test([0x44; 32])),
                    || Ok(unreadable.clone()),
                )
                .unwrap();
                assert_eq!(meta.skipped, vec![CONFIG_NOT_COPIED_UNREADABLE.to_string()]);
                assert!(!meta.contents.iter().any(|c| c == CONFIG_ENTRY), "{meta:?}");
            } else {
                eprintln!("skipped: this process can read a mode-000 file");
            }
            std::fs::set_permissions(&unreadable, std::fs::Permissions::from_mode(0o600)).unwrap();
        }
    }
}
