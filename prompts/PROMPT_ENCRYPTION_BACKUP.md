# PROMPT: Encryption at Rest + Backup & Restore for Nexus OS

## Context
Enterprise deployments require data-at-rest encryption and automated backup/restore capabilities. Nexus OS stores agent data, audit trails, genomes, and configuration in local SQLite databases.

## Part 1: Encryption at Rest

### Objective
Encrypt all local data stores using AES-256-GCM with keys derived via Argon2id.

### Implementation

#### Step 1: Add nexus-crypto module to nexus-kernel

**Dependencies:**
```toml
aes-gcm = "0.10"
argon2 = "0.5"
rand = "0.8"
zeroize = { version = "1", features = ["derive"] }
```

#### Step 2: Key management

```rust
use zeroize::Zeroize;

#[derive(Zeroize)]
#[zeroize(drop)]
pub struct EncryptionKey {
    key: [u8; 32], // AES-256
}

impl EncryptionKey {
    /// Derive from user password / master secret
    pub fn derive(password: &[u8], salt: &[u8; 16]) -> Result<Self, CryptoError> {
        let params = argon2::Params::new(65536, 3, 4, Some(32))?;
        let argon2 = argon2::Argon2::new(
            argon2::Algorithm::Argon2id,
            argon2::Version::V0x13,
            params,
        );
        let mut key = [0u8; 32];
        argon2.hash_password_into(password, salt, &mut key)?;
        Ok(Self { key })
    }

    /// Load from environment variable or OS keychain
    pub fn from_env() -> Result<Self, CryptoError> {
        // Try NEXUS_ENCRYPTION_KEY env var first
        // Fall back to OS keychain (libsecret/Keychain/Credential Manager)
    }
}
```

#### Step 3: SQLCipher or manual encryption

**Option A (Preferred):** Use `sqlcipher` via `rusqlite` with `bundled-sqlcipher` feature:
```toml
rusqlite = { version = "0.32", features = ["bundled-sqlcipher"] }
```
This encrypts the entire SQLite database transparently.

**Option B (Fallback):** If SQLCipher adds too much binary size, encrypt sensitive fields individually using AES-256-GCM before writing to SQLite.

#### Step 4: Encrypt these data stores
- Audit trail database
- Agent genome database
- Agent configuration store
- Session/auth tokens
- LLM API keys stored locally

#### Step 5: Key rotation

```rust
pub async fn rotate_encryption_key(
    old_key: &EncryptionKey,
    new_key: &EncryptionKey,
    data_dir: &Path,
) -> Result<(), CryptoError> {
    // 1. Open each database with old key
    // 2. Create new database with new key
    // 3. Copy all data
    // 4. Verify new database integrity
    // 5. Replace old database files
    // 6. Zeroize old key from memory
}
```

#### Step 6: Configuration

```toml
[security]
encryption_at_rest = true
encryption_key_source = "env"  # "env" | "keychain" | "file"
encryption_key_env = "NEXUS_ENCRYPTION_KEY"
# encryption_key_file = "/run/secrets/nexus-key"  # For Docker/K8s secrets
```

---

## Part 2: Backup & Restore

### Objective
Automated backup and restore of all Nexus OS data with encryption and integrity verification.

### Implementation

#### Step 1: Create nexus-backup module in nexus-kernel

#### Step 2: Backup contents

A backup archive (.tar.gz, optionally encrypted) includes:
- All SQLite databases (audit, genomes, agents, config)
- Agent manifests (TOML files)
- Encryption salt (NOT the key)
- Configuration files
- Genome evolution history
- Backup metadata (version, timestamp, checksum)

**Excluded from backups:**
- Encryption keys (never backed up with data)
- OS keychain entries
- Temporary files
- WASM compilation cache

#### Step 3: Backup implementation

```rust
pub struct BackupConfig {
    pub output_path: PathBuf,
    pub include_audit: bool,
    pub include_genomes: bool,
    pub include_config: bool,
    pub encrypt: bool,
    pub compression: Compression, // Gzip | Zstd
}

pub struct BackupMetadata {
    pub version: String,        // Nexus OS version
    pub created_at: DateTime<Utc>,
    pub checksum: String,       // SHA-256 of archive
    pub contents: Vec<String>,  // List of included items
    pub size_bytes: u64,
}

pub async fn create_backup(config: BackupConfig) -> Result<BackupMetadata, BackupError> {
    // 1. Pause audit writes (brief lock)
    // 2. Copy SQLite databases (snapshot via VACUUM INTO)
    // 3. Package into tar archive
    // 4. Compress (gzip or zstd)
    // 5. Optionally encrypt with AES-256-GCM
    // 6. Calculate checksum
    // 7. Write metadata file
    // 8. Resume audit writes
}

pub async fn restore_backup(
    archive_path: &Path,
    encryption_key: Option<&EncryptionKey>,
) -> Result<RestoreResult, BackupError> {
    // 1. Verify checksum
    // 2. Decrypt if needed
    // 3. Extract archive
    // 4. Verify database integrity (PRAGMA integrity_check)
    // 5. Verify audit chain integrity
    // 6. Stop all agents
    // 7. Replace data files
    // 8. Restart system
}
```

#### Step 4: Tauri commands

```rust
#[tauri::command]
async fn backup_create(state: State<'_, AppState>, config: BackupConfig) -> Result<BackupMetadata, NexusError>

#[tauri::command]
async fn backup_restore(state: State<'_, AppState>, archive_path: String) -> Result<RestoreResult, NexusError>

#[tauri::command]
async fn backup_list(state: State<'_, AppState>) -> Result<Vec<BackupMetadata>, NexusError>

#[tauri::command]
async fn backup_verify(state: State<'_, AppState>, archive_path: String) -> Result<VerifyResult, NexusError>
```

#### Step 5: Scheduled backups

```rust
pub struct BackupSchedule {
    pub cron: String,           // "0 2 * * *" = daily at 2 AM
    pub retention_count: u32,   // Keep last N backups
    pub output_dir: PathBuf,
    pub encrypt: bool,
}
```

Use `tokio::time::interval` for desktop mode or Kubernetes CronJob for server mode.

#### Step 6: Configuration

```toml
[backup]
enabled = true
schedule = "0 2 * * *"
output_dir = "~/.local/share/nexus-os/backups"
retention_count = 30
encrypt = true
compression = "zstd"
include_audit = true
include_genomes = true
```

## Testing
- Unit test: Encryption round-trip (encrypt → decrypt → verify)
- Unit test: Key derivation determinism
- Unit test: Backup create → restore → verify data integrity
- Unit test: Backup with encryption
- Unit test: Audit chain integrity after restore
- Integration test: Key rotation with active database

## Finish
Run `cargo fmt` and `cargo clippy` on modified crates only.
Do NOT use `--all-features`.
