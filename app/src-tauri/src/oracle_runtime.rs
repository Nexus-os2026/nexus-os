//! Production GovernanceOracle runtime wiring.
//!
//! Phase 1.5a: the desktop backend must ship an actually-running
//! `GovernanceOracle` + `DecisionEngine` pair. Prior to this module the app
//! held a `GovernanceRuleset` and a `DecisionAuditLog` on `AppState` but
//! never spawned a `DecisionEngine` task, which meant `GovernanceOracle`
//! was only ever constructed inside unit tests — no subsystem could
//! actually obtain a governance decision at runtime.
//!
//! Architecture
//! ------------
//! The `DecisionEngine::run` loop owns its `mpsc::Receiver<OracleRequest>`.
//! Callers (swarm, future cost_ceiling migrations, etc.) need the paired
//! `Sender` to submit capability requests. This module owns both sides
//! via a thin relay:
//!
//!   caller ──► external_tx ══► relay task ──► engine_tx ══► DecisionEngine
//!              (in AppState)                    (internal)
//!
//! The relay repacks each `OracleRequest`, forwards it to the engine,
//! awaits the engine's decision on an internal oneshot, increments the
//! processed counter, and forwards the decision to the caller's original
//! oneshot. This gives us an externally-visible counter without modifying
//! the `nexus-governance-engine` crate.
//!
//! Shutdown is cascading: drop `OracleRuntime` → external_tx drops →
//! relay's `external_rx.recv()` returns `None` → relay exits → engine_tx
//! drops → engine's `request_rx.recv()` returns `None` → engine task
//! exits. For tests that want to drive shutdown explicitly, use
//! [`OracleRuntime::shutdown`].

use nexus_crypto::{CryptoIdentity, SignatureAlgorithm};
use nexus_governance_engine::{DecisionEngine, GovernanceRuleset};
use nexus_governance_oracle::{GovernanceDecision, GovernanceOracle, OracleRequest};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

/// Channel buffer for in-flight oracle requests. Bounded to expose
/// backpressure — a flooded channel will make `Sender::send` await.
pub const ORACLE_CHANNEL_BUFFER: usize = 256;

/// Response ceiling for the production oracle. Keeps decision timing
/// constant so callers can't infer policy complexity from latency.
pub const ORACLE_RESPONSE_CEILING: Duration = Duration::from_millis(100);

/// Env escape hatch: if set to `1`, the oracle identity is generated fresh
/// at startup and never written to disk. Sealed tokens from that session
/// become unverifiable after restart. Intended for tests and ephemeral dev
/// loops only.
const EPHEMERAL_ENV: &str = "NEXUS_ORACLE_EPHEMERAL";

/// Identity file V1 layout (Bug N):
///   bytes 0..4   : magic `NXOK` (Nexus Oracle Key)
///   byte  4      : version = 1
///   bytes 5..70  : `CryptoIdentity::to_bytes()` payload — 65 bytes
///                  `[algorithm_byte (1) | signing_key (32) | verifying_key (32)]`
/// Total: 70 bytes. `CryptoIdentity::from_bytes` re-derives the verifying
/// key from the signing key, so we only pass `payload[1..33]` into it; the
/// trailing 32 verifying-key bytes are written for offline inspection.
///
/// V0 = legacy format written by Phase 1.5a.1: raw 65-byte payload, no
/// header. Auto-migrated to V1 on next start. See `load_identity` for the
/// detection ladder and `write_identity` for the atomic V1 writer.
const IDENTITY_MAGIC: &[u8; 4] = b"NXOK";
const IDENTITY_VERSION_V1: u8 = 1;
const IDENTITY_HEADER_LEN: usize = 5;
const IDENTITY_PAYLOAD_LEN: usize = 65;
const IDENTITY_V1_LEN: usize = IDENTITY_HEADER_LEN + IDENTITY_PAYLOAD_LEN; // 70
const IDENTITY_V0_LEN: usize = IDENTITY_PAYLOAD_LEN; // 65
const ED25519_ALGO_BYTE: u8 = 0x01;

/// Errors returned by the fallible entry points on `OracleRuntime`. The
/// infallible `OracleRuntime::start` wraps these in a panic with the error
/// message — a corrupt or unreadable oracle identity is a fatal startup
/// precondition and the app must not come up in a degraded trust state.
#[derive(Debug)]
pub enum OracleRuntimeError {
    HomeDirMissing,
    IdentityDirectoryCreate {
        path: PathBuf,
        source: std::io::Error,
    },
    IdentityRead {
        path: PathBuf,
        source: std::io::Error,
    },
    IdentityWrite {
        path: PathBuf,
        source: std::io::Error,
    },
    IdentityChmod {
        path: PathBuf,
        source: std::io::Error,
    },
    IdentityBadPerms {
        path: PathBuf,
        mode: u32,
    },
    IdentityFormat {
        path: PathBuf,
        detail: String,
    },
    /// Bug N: file does not match the V1 magic header AND does not match
    /// the legacy V0 length. Refusing to start prevents the trust root
    /// from being silently regenerated over a corrupt or unrelated file.
    IdentityFileCorrupt {
        path: PathBuf,
        detail: String,
    },
    /// Bug N: file matches the V1 magic but declares a version this build
    /// does not understand. Non-recoverable — the app must not start, and
    /// must not regenerate (would orphan the trust root).
    IdentityFileFutureVersion {
        path: PathBuf,
        version: u8,
    },
}

impl std::fmt::Display for OracleRuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::HomeDirMissing => write!(
                f,
                "cannot locate oracle identity: HOME environment variable is unset"
            ),
            Self::IdentityDirectoryCreate { path, source } => {
                write!(f, "create identity directory {}: {source}", path.display())
            }
            Self::IdentityRead { path, source } => {
                write!(f, "read identity file {}: {source}", path.display())
            }
            Self::IdentityWrite { path, source } => {
                write!(f, "write identity file {}: {source}", path.display())
            }
            Self::IdentityChmod { path, source } => write!(
                f,
                "chmod 0600 identity file {}: {source}",
                path.display()
            ),
            Self::IdentityBadPerms { path, mode } => write!(
                f,
                "identity file {} has permissions 0o{:o}; expected 0o600 (group/other bits must be clear)",
                path.display(),
                mode
            ),
            Self::IdentityFormat { path, detail } => {
                write!(f, "identity file {} is corrupt: {detail}", path.display())
            }
            Self::IdentityFileCorrupt { path, detail } => write!(
                f,
                "identity file {} is corrupt or unrelated (refusing to regenerate): {detail}",
                path.display()
            ),
            Self::IdentityFileFutureVersion { path, version } => write!(
                f,
                "identity file {} declares future version v{version}; this build only understands v{}; \
                 refusing to start (will not regenerate to preserve trust root)",
                path.display(),
                IDENTITY_VERSION_V1
            ),
        }
    }
}

impl std::error::Error for OracleRuntimeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::IdentityDirectoryCreate { source, .. }
            | Self::IdentityRead { source, .. }
            | Self::IdentityWrite { source, .. }
            | Self::IdentityChmod { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// Where the oracle's signing identity lives for a given run.
///
/// `Persistent(path)` is production: load if present, generate-and-save
/// if absent. `Ephemeral` skips disk entirely — tests and
/// `NEXUS_ORACLE_EPHEMERAL=1` use this.
#[derive(Debug, Clone)]
pub enum IdentityMode {
    Persistent(PathBuf),
    Ephemeral,
}

impl IdentityMode {
    /// Production resolution: honor `NEXUS_ORACLE_EPHEMERAL=1`, otherwise
    /// use `$HOME/.nexus/oracle_identity.key`.
    pub fn from_env() -> Result<Self, OracleRuntimeError> {
        if std::env::var(EPHEMERAL_ENV).is_ok_and(|v| v == "1") {
            eprintln!(
                "[startup] {EPHEMERAL_ENV}=1 — oracle identity is ephemeral for this run; sealed tokens from this session will be unverifiable after restart"
            );
            return Ok(Self::Ephemeral);
        }
        Ok(Self::Persistent(default_identity_path()?))
    }
}

/// `$HOME/.nexus/oracle_identity.key`. Matches the convention used for
/// `metering.db`, `swarm_routing.toml`, and other per-user state files.
pub fn default_identity_path() -> Result<PathBuf, OracleRuntimeError> {
    let home = std::env::var("HOME").map_err(|_| OracleRuntimeError::HomeDirMissing)?;
    Ok(PathBuf::from(home)
        .join(".nexus")
        .join("oracle_identity.key"))
}

/// Detection result for the V1 file-format ladder. Internal to the load
/// helper — outer code only sees the final `CryptoIdentity` or a typed
/// error.
#[derive(Debug)]
enum IdentityFileVersion {
    V1,
    /// Legacy 1.5a.1 format. Caller migrates to V1 in place after parse.
    V0Migrated,
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

/// V0/V1/future/corrupt detection ladder. Returns the parsed identity
/// alongside which version was loaded so the caller can rewrite a V0 file
/// in V1 format on the same startup.
fn read_identity(path: &Path) -> Result<(CryptoIdentity, IdentityFileVersion), OracleRuntimeError> {
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

    // V1: starts with NXOK magic.
    if bytes.len() >= IDENTITY_HEADER_LEN && &bytes[..4] == IDENTITY_MAGIC {
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
        let identity = parse_payload(path, &bytes[IDENTITY_HEADER_LEN..])?;
        return Ok((identity, IdentityFileVersion::V1));
    }

    // V0: legacy raw 65-byte payload.
    if bytes.len() == IDENTITY_V0_LEN && bytes[0] == ED25519_ALGO_BYTE {
        let identity = parse_payload(path, &bytes)?;
        return Ok((identity, IdentityFileVersion::V0Migrated));
    }

    // Anything else: corrupt or unrelated. Refuse to start.
    Err(OracleRuntimeError::IdentityFileCorrupt {
        path: path.to_path_buf(),
        detail: format!(
            "no NXOK magic at start, and length ({} bytes) does not match V0 ({IDENTITY_V0_LEN}) or V1 ({IDENTITY_V1_LEN})",
            bytes.len()
        ),
    })
}

/// Public load entry point: returns the parsed identity. If the file was
/// V0, it's transparently rewritten in V1 format before returning so the
/// next start finds a clean V1 file.
fn load_identity(path: &Path) -> Result<CryptoIdentity, OracleRuntimeError> {
    let (identity, version) = read_identity(path)?;
    if matches!(version, IdentityFileVersion::V0Migrated) {
        eprintln!(
            "[oracle] migrated identity file {} from V0 → V1 format",
            path.display()
        );
        write_identity(path, &identity)?;
    }
    Ok(identity)
}

/// Write the identity file in V1 format atomically: write to a sibling
/// tempfile, fsync it, then rename over the destination. Prevents a
/// half-written file from being observed if the process crashes mid-write.
fn write_identity(path: &Path, identity: &CryptoIdentity) -> Result<(), OracleRuntimeError> {
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
            // Best-effort: tighten parent directory to 0700 if we created it
            // or if it was writable. Failure is non-fatal — dev machines
            // sometimes have the home directory already more open and we
            // should not refuse to start there.
            let _ = std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700));
        }
    }

    // Build the V1 byte vector: magic + version + payload.
    let payload = identity.to_bytes();
    let mut out = Vec::with_capacity(IDENTITY_V1_LEN);
    out.extend_from_slice(IDENTITY_MAGIC);
    out.push(IDENTITY_VERSION_V1);
    out.extend_from_slice(&payload);
    debug_assert_eq!(out.len(), IDENTITY_V1_LEN);

    // Sibling tempfile in the same directory so the rename is atomic on
    // the same filesystem. Suffix encodes pid + a per-call uuid so
    // parallel writers (e.g. two cargo test threads racing on the same
    // home dir) don't collide — without the uuid, thread A would rename
    // the shared tempfile away mid-write and thread B's chmod would fail
    // ENOENT.
    let parent = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("oracle_identity.key");
    let tmp_path = parent.join(format!(
        ".{file_name}.tmp.{}.{}",
        std::process::id(),
        uuid::Uuid::new_v4()
    ));

    // Write + fsync the tempfile.
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
        // Set 0600 on the tempfile BEFORE rename so the destination is
        // never observable with looser permissions.
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

    // Atomic rename over destination.
    if let Err(source) = std::fs::rename(&tmp_path, path) {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(OracleRuntimeError::IdentityWrite {
            path: path.to_path_buf(),
            source,
        });
    }

    Ok(())
}

fn resolve_identity(mode: &IdentityMode) -> Result<CryptoIdentity, OracleRuntimeError> {
    match mode {
        IdentityMode::Ephemeral => Ok(CryptoIdentity::generate(SignatureAlgorithm::Ed25519)
            .expect("Ed25519 key generation cannot fail on supported platforms")),
        IdentityMode::Persistent(path) => {
            if path.exists() {
                load_identity(path)
            } else {
                let identity = CryptoIdentity::generate(SignatureAlgorithm::Ed25519)
                    .expect("Ed25519 key generation cannot fail on supported platforms");
                write_identity(path, &identity)?;
                eprintln!(
                    "[startup] Oracle signing identity generated and written to {}",
                    path.display()
                );
                Ok(identity)
            }
        }
    }
}

/// Snapshot returned by the `oracle_runtime_status` Tauri command.
#[derive(Debug, Clone, Serialize)]
pub struct OracleRuntimeStatus {
    pub is_running: bool,
    pub pending_requests: usize,
    pub total_processed: u64,
    pub uptime_seconds: u64,
}

/// Long-lived governance oracle runtime. Stored on `AppState` as
/// `Arc<OracleRuntime>`; the sender is cloned out via
/// [`OracleRuntime::sender`] for downstream subsystems.
///
/// If `start` is called outside a tokio runtime (e.g. from a synchronous
/// unit test that only wants `AppState::new_in_memory()` for its DB
/// fixture), the runtime falls back to a *stub* mode: the tasks are not
/// spawned, `is_running()` returns `false`, and the external channel
/// receiver is parked on the struct so `sender().send()` up to the buffer
/// capacity still succeeds. Actual decisions never arrive in stub mode —
/// callers must not await them. Production always has a tokio runtime.
pub struct OracleRuntime {
    /// Full GovernanceOracle bound to the same mpsc channel the DecisionEngine
    /// reads from. Owns the signing keypair for this process; subsystems that
    /// need sealed tokens (e.g. SwarmOracleBridge) clone this Arc.
    oracle: Arc<GovernanceOracle>,
    external_tx: mpsc::Sender<OracleRequest>,
    processed: Arc<AtomicU64>,
    started_at: Instant,
    relay_handle: std::sync::Mutex<Option<JoinHandle<()>>>,
    engine_handle: std::sync::Mutex<Option<JoinHandle<()>>>,
    /// Receiver parked here in stub mode so the channel stays alive.
    /// `None` in normal mode (the receiver was moved into the relay task).
    _stub_rx: std::sync::Mutex<Option<mpsc::Receiver<OracleRequest>>>,
}

impl OracleRuntime {
    /// Production entry point. Resolves the identity mode from the
    /// environment, constructs the oracle, and spawns the relay + engine
    /// tasks. Panics on any identity load/save error — a corrupt oracle
    /// identity is a fatal startup precondition and the app must not come
    /// up in a degraded trust state. Use `try_start_with_mode` for
    /// programmatic error handling (tests, embedders).
    pub fn start(ruleset: GovernanceRuleset) -> Arc<Self> {
        let mode = IdentityMode::from_env()
            .expect("OracleRuntime: identity mode resolution must succeed at startup");
        Self::try_start_with_mode(ruleset, mode).unwrap_or_else(|e| {
            panic!("OracleRuntime startup failed: {e}");
        })
    }

    /// Fallible startup with an explicit identity mode. Used by integration
    /// tests that supply a temporary identity path or ephemeral mode.
    pub fn try_start_with_mode(
        ruleset: GovernanceRuleset,
        mode: IdentityMode,
    ) -> Result<Arc<Self>, OracleRuntimeError> {
        let identity = resolve_identity(&mode)?;
        Ok(Self::spawn_with_identity(ruleset, identity))
    }

    fn spawn_with_identity(ruleset: GovernanceRuleset, identity: CryptoIdentity) -> Arc<Self> {
        let (external_tx, external_rx) = mpsc::channel::<OracleRequest>(ORACLE_CHANNEL_BUFFER);
        let processed = Arc::new(AtomicU64::new(0));
        let started_at = Instant::now();

        // Construct the GovernanceOracle bound to the external channel with
        // the caller-supplied identity. `GovernanceOracle::with_identity` is
        // synchronous and does not require a tokio runtime, so this is safe
        // in stub mode too.
        let oracle = Arc::new(GovernanceOracle::with_identity(
            external_tx.clone(),
            ORACLE_RESPONSE_CEILING,
            identity,
        ));

        if tokio::runtime::Handle::try_current().is_err() {
            eprintln!(
                "[startup] GovernanceOracle runtime in stub mode — no tokio runtime in context; submit_request() calls will time out after the response ceiling"
            );
            return Arc::new(Self {
                oracle,
                external_tx,
                processed,
                started_at,
                relay_handle: std::sync::Mutex::new(None),
                engine_handle: std::sync::Mutex::new(None),
                _stub_rx: std::sync::Mutex::new(Some(external_rx)),
            });
        }

        let (engine_tx, engine_rx) = mpsc::channel::<OracleRequest>(ORACLE_CHANNEL_BUFFER);
        let processed_relay = Arc::clone(&processed);
        let mut external_rx = external_rx;

        let relay_handle = tokio::spawn(async move {
            while let Some(req) = external_rx.recv().await {
                let OracleRequest {
                    request,
                    response_tx: original_response,
                } = req;
                let (inner_tx, inner_rx) = tokio::sync::oneshot::channel();
                let repacked = OracleRequest {
                    request,
                    response_tx: inner_tx,
                };
                if engine_tx.send(repacked).await.is_err() {
                    let _ = original_response.send(GovernanceDecision::Denied);
                    break;
                }
                match inner_rx.await {
                    Ok(decision) => {
                        processed_relay.fetch_add(1, Ordering::Relaxed);
                        let _ = original_response.send(decision);
                    }
                    Err(_) => {
                        let _ = original_response.send(GovernanceDecision::Denied);
                    }
                }
            }
        });

        let engine_handle = tokio::spawn(async move {
            let mut engine = DecisionEngine::new(engine_rx, ruleset);
            engine.run().await;
        });

        let vk_prefix =
            oracle
                .verifying_key_bytes()
                .iter()
                .take(4)
                .fold(String::new(), |mut s, b| {
                    use std::fmt::Write;
                    let _ = write!(s, "{b:02x}");
                    s
                });
        eprintln!(
            "[startup] GovernanceOracle runtime started on mpsc channel (buffer={ORACLE_CHANNEL_BUFFER}, verifying_key_prefix={vk_prefix}…)"
        );

        Arc::new(Self {
            oracle,
            external_tx,
            processed,
            started_at,
            relay_handle: std::sync::Mutex::new(Some(relay_handle)),
            engine_handle: std::sync::Mutex::new(Some(engine_handle)),
            _stub_rx: std::sync::Mutex::new(None),
        })
    }

    /// Obtain a cloned `Sender<OracleRequest>` for submitting capability
    /// requests. Downstream subsystems that only need raw `GovernanceDecision`
    /// (no sealed-token surface) can use this; subsystems that need
    /// `SealedToken` + `verify_token` must use [`Self::oracle`] instead.
    pub fn sender(&self) -> mpsc::Sender<OracleRequest> {
        self.external_tx.clone()
    }

    /// Full `GovernanceOracle` handle for subsystems that need sealed tokens
    /// (SwarmOracleBridge, future cost_ceiling migrations). The returned
    /// `Arc<GovernanceOracle>` is backed by the same `CryptoIdentity` for
    /// the lifetime of the process — sealed tokens issued by one clone are
    /// verifiable by every other clone.
    pub fn oracle(&self) -> Arc<GovernanceOracle> {
        Arc::clone(&self.oracle)
    }

    /// Current runtime snapshot for the `oracle_runtime_status` command.
    ///
    /// `pending_requests` is measured from the external channel only
    /// (caller-visible queue). The engine's internal channel drains fast
    /// enough that including it adds noise without signal.
    pub fn status(&self) -> OracleRuntimeStatus {
        let max = self.external_tx.max_capacity();
        let free = self.external_tx.capacity();
        let pending = max.saturating_sub(free);
        OracleRuntimeStatus {
            is_running: self.is_running(),
            pending_requests: pending,
            total_processed: self.processed.load(Ordering::Relaxed),
            uptime_seconds: self.started_at.elapsed().as_secs(),
        }
    }

    /// Running if the relay and engine tasks are still alive.
    pub fn is_running(&self) -> bool {
        let relay_alive = match self.relay_handle.lock() {
            Ok(guard) => guard.as_ref().map(|h| !h.is_finished()).unwrap_or(false),
            Err(_) => false,
        };
        let engine_alive = match self.engine_handle.lock() {
            Ok(guard) => guard.as_ref().map(|h| !h.is_finished()).unwrap_or(false),
            Err(_) => false,
        };
        relay_alive && engine_alive
    }

    pub fn total_processed(&self) -> u64 {
        self.processed.load(Ordering::Relaxed)
    }

    /// Abort the relay and engine tasks. Used on app shutdown and by tests
    /// that want deterministic teardown. After this call, the runtime
    /// cannot accept new requests.
    pub fn shutdown(&self) {
        if let Ok(mut guard) = self.relay_handle.lock() {
            if let Some(h) = guard.take() {
                h.abort();
            }
        }
        if let Ok(mut guard) = self.engine_handle.lock() {
            if let Some(h) = guard.take() {
                h.abort();
            }
        }
        eprintln!(
            "[shutdown] GovernanceOracle runtime stopped (total_processed={})",
            self.processed.load(Ordering::Relaxed)
        );
    }
}
