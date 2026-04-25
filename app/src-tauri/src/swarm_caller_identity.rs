//! Persistent swarm caller identity (Bug O).
//!
//! Phase 1.5b's `swarm_plan` Tauri command generated a fresh
//! `CryptoIdentity` per request — every plan looked like it came from a
//! different user even within one session. This module persists a
//! per-machine caller identity at `~/.nexus/swarm_caller_identity.key`
//! so audit trails can link plans across requests.
//!
//! # Architectural separation from oracle identity
//!
//! Bug N's oracle identity (`oracle_identity.key`, magic `NXOK`) is the
//! system trust root — singular, signed governance decisions come from
//! it. Swarm caller identity is a user/session identity. Future
//! multi-user support will give each user their own caller identity
//! while the oracle stays singular. Future federation could derive
//! caller identity from external SSO. The two files are deliberately
//! independent.
//!
//! # File format (V1)
//!
//! Mirrors Bug N's V1 layout exactly, with a different magic so the two
//! files can never be confused:
//!
//!   bytes 0..4   : magic `NXSC` (Nexus Swarm Caller)
//!   byte  4      : version = 1
//!   bytes 5..70  : `CryptoIdentity::to_bytes()` payload — 65 bytes
//!                  `[algorithm_byte (1) | signing_key (32) | verifying_key (32)]`
//!
//! Total: 70 bytes. No V0 — this file path is new in Bug O. A 65-byte
//! file here is corrupt (or an oracle file accidentally at the wrong
//! path), not legacy.
//!
//! # Why duplicate the V1 reader/writer instead of sharing with Bug N
//!
//! At sample size two the spec leans duplicate. The two readers also
//! have a real semantic divergence — oracle accepts V0 and migrates;
//! swarm caller treats 65-byte as corrupt. The atomic writer IS
//! byte-identical between the two; consolidating it is tracked as Bug Z
//! once a third consumer arrives or a writer fix needs to land in two
//! places.

use crate::oracle_runtime::{default_identity_path_for, OracleRuntimeError};
use nexus_crypto::{CryptoIdentity, SignatureAlgorithm};
use std::path::{Path, PathBuf};

const SWARM_CALLER_MAGIC: &[u8; 4] = b"NXSC";
const IDENTITY_VERSION_V1: u8 = 1;
const IDENTITY_HEADER_LEN: usize = 5;
const IDENTITY_PAYLOAD_LEN: usize = 65;
const IDENTITY_V1_LEN: usize = IDENTITY_HEADER_LEN + IDENTITY_PAYLOAD_LEN; // 70
const ED25519_ALGO_BYTE: u8 = 0x01;

/// Env escape hatch: when `1`, the swarm caller identity is generated
/// fresh at startup and never written to disk. Mirrors
/// `NEXUS_ORACLE_EPHEMERAL`. Sealed plans from this session lose
/// caller-correlation after restart. Intended for tests + ephemeral dev
/// loops.
const EPHEMERAL_ENV: &str = "NEXUS_SWARM_CALLER_EPHEMERAL";

/// Where the caller identity lives for a given run. `Persistent(path)`
/// is production: load if present, generate-and-save if absent.
/// `Ephemeral` skips disk entirely.
#[derive(Debug, Clone)]
pub enum SwarmCallerMode {
    Persistent(PathBuf),
    Ephemeral,
}

impl SwarmCallerMode {
    /// Honor `NEXUS_SWARM_CALLER_EPHEMERAL=1`, otherwise resolve to
    /// `$HOME/.nexus/swarm_caller_identity.key`.
    pub fn from_env() -> Result<Self, OracleRuntimeError> {
        if std::env::var(EPHEMERAL_ENV).is_ok_and(|v| v == "1") {
            eprintln!(
                "[startup] {EPHEMERAL_ENV}=1 — swarm caller identity is ephemeral for this run; audit trails for plans from this session won't link to prior or future runs"
            );
            return Ok(SwarmCallerMode::Ephemeral);
        }
        Ok(SwarmCallerMode::Persistent(default_swarm_caller_path()?))
    }
}

/// Default persistent location: `$HOME/.nexus/swarm_caller_identity.key`.
/// Reuses the same `~/.nexus/` parent directory the oracle identity
/// lives in (also chmod 0700) — see `oracle_runtime::default_identity_path_for`.
pub fn default_swarm_caller_path() -> Result<PathBuf, OracleRuntimeError> {
    default_identity_path_for("swarm_caller_identity.key")
}

/// Resolve mode → identity. `Ephemeral` always generates fresh.
/// `Persistent(path)`: load if file exists; generate + write atomically
/// if absent; error on corrupt or future-version files.
pub fn try_load_or_generate(mode: &SwarmCallerMode) -> Result<CryptoIdentity, OracleRuntimeError> {
    match mode {
        SwarmCallerMode::Ephemeral => Ok(CryptoIdentity::generate(SignatureAlgorithm::Ed25519)
            .expect("Ed25519 key generation cannot fail on supported platforms")),
        SwarmCallerMode::Persistent(path) => {
            if path.exists() {
                read_swarm_caller_identity(path)
            } else {
                let fresh = CryptoIdentity::generate(SignatureAlgorithm::Ed25519)
                    .expect("Ed25519 key generation cannot fail on supported platforms");
                write_swarm_caller_identity(path, &fresh)?;
                Ok(fresh)
            }
        }
    }
}

fn parse_payload(path: &Path, payload: &[u8]) -> Result<CryptoIdentity, OracleRuntimeError> {
    if payload.len() != IDENTITY_PAYLOAD_LEN {
        return Err(OracleRuntimeError::IdentityFileCorrupt {
            path: path.to_path_buf(),
            detail: format!(
                "payload length {} ≠ expected {IDENTITY_PAYLOAD_LEN}",
                payload.len()
            ),
        });
    }
    let algo = match payload[0] {
        ED25519_ALGO_BYTE => SignatureAlgorithm::Ed25519,
        other => {
            return Err(OracleRuntimeError::IdentityFormat {
                path: path.to_path_buf(),
                detail: format!("unknown algorithm byte 0x{other:02x}"),
            });
        }
    };
    CryptoIdentity::from_bytes(algo, &payload[1..33]).map_err(|e| {
        OracleRuntimeError::IdentityFormat {
            path: path.to_path_buf(),
            detail: format!("{e}"),
        }
    })
}

/// Read the V1 file. Unlike Bug N's oracle reader this rejects 65-byte
/// payloads as corrupt — there's no legacy V0 for this file path.
fn read_swarm_caller_identity(path: &Path) -> Result<CryptoIdentity, OracleRuntimeError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let meta = std::fs::metadata(path).map_err(|source| OracleRuntimeError::IdentityRead {
            path: path.to_path_buf(),
            source,
        })?;
        let mode = meta.permissions().mode() & 0o777;
        if mode & 0o077 != 0 {
            return Err(OracleRuntimeError::IdentityBadPerms {
                path: path.to_path_buf(),
                mode,
            });
        }
    }
    let bytes = std::fs::read(path).map_err(|source| OracleRuntimeError::IdentityRead {
        path: path.to_path_buf(),
        source,
    })?;

    if bytes.len() < IDENTITY_HEADER_LEN || &bytes[..4] != SWARM_CALLER_MAGIC {
        return Err(OracleRuntimeError::IdentityFileCorrupt {
            path: path.to_path_buf(),
            detail: format!(
                "expected NXSC magic at start; length {} bytes, this file does not match V1 ({IDENTITY_V1_LEN})",
                bytes.len()
            ),
        });
    }
    let version = bytes[4];
    if version != IDENTITY_VERSION_V1 {
        return Err(OracleRuntimeError::IdentityFileFutureVersion {
            path: path.to_path_buf(),
            version,
        });
    }
    if bytes.len() != IDENTITY_V1_LEN {
        return Err(OracleRuntimeError::IdentityFileCorrupt {
            path: path.to_path_buf(),
            detail: format!(
                "V1 file length {} ≠ expected {IDENTITY_V1_LEN} (5-byte header + 65-byte payload)",
                bytes.len()
            ),
        });
    }
    parse_payload(path, &bytes[IDENTITY_HEADER_LEN..])
}

/// Atomic write: tempfile + fsync + chmod + rename. Tempfile name
/// carries pid + uuid so parallel writers don't collide on the rename
/// (same race that hit Bug N's first ci-local run).
fn write_swarm_caller_identity(
    path: &Path,
    identity: &CryptoIdentity,
) -> Result<(), OracleRuntimeError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| {
            OracleRuntimeError::IdentityDirectoryCreate {
                path: parent.to_path_buf(),
                source,
            }
        })?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700));
        }
    }

    let payload = identity.to_bytes();
    let mut out = Vec::with_capacity(IDENTITY_V1_LEN);
    out.extend_from_slice(SWARM_CALLER_MAGIC);
    out.push(IDENTITY_VERSION_V1);
    out.extend_from_slice(&payload);
    debug_assert_eq!(out.len(), IDENTITY_V1_LEN);

    let parent = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("swarm_caller_identity.key");
    let tmp_path = parent.join(format!(
        ".{file_name}.tmp.{}.{}",
        std::process::id(),
        uuid::Uuid::new_v4()
    ));

    {
        use std::io::Write;
        let mut f = std::fs::File::create(&tmp_path).map_err(|source| {
            OracleRuntimeError::IdentityWrite {
                path: tmp_path.clone(),
                source,
            }
        })?;
        f.write_all(&out)
            .map_err(|source| OracleRuntimeError::IdentityWrite {
                path: tmp_path.clone(),
                source,
            })?;
        f.sync_all()
            .map_err(|source| OracleRuntimeError::IdentityWrite {
                path: tmp_path.clone(),
                source,
            })?;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Err(source) =
            std::fs::set_permissions(&tmp_path, std::fs::Permissions::from_mode(0o600))
        {
            let _ = std::fs::remove_file(&tmp_path);
            return Err(OracleRuntimeError::IdentityChmod {
                path: tmp_path,
                source,
            });
        }
    }

    if let Err(source) = std::fs::rename(&tmp_path, path) {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(OracleRuntimeError::IdentityWrite {
            path: path.to_path_buf(),
            source,
        });
    }

    Ok(())
}
