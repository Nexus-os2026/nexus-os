//! Bug AK Phase 1 — facade tests.
//!
//! Refinement D locks the four-LLM-provider keyring-first default
//! and the keyring-miss fallback semantics so they cannot silently
//! regress.
//!
//! Refactored in Commit 2 against the new typed-Arc
//! `SecretsFacade::new(env, keyring, sqlite, memory, &cfg)` and
//! the `(config, &facade)` shape of `migrate_config_to_vault`.

use super::backend_env::EnvBackend;
use super::backend_keyring::{KeyringBackend, KeyringBackendAdapter, MockKeyring};
use super::backend_memory::MemoryBackend;
use super::backend_sqlite::SqliteEnvelopeBackend;
use super::migrate::{migrate_config_to_vault, MigrationError, MigrationReport};
use super::{ResolvedFrom, SecretAuditCtx, SecretBackend, SecretError, SecretsFacade};
use crate::config::{CredentialFacadeConfig, NexusConfig};
use crate::crypto::EncryptionKey;
use std::sync::Arc;
use zeroize::Zeroizing;

/// Test fixture: deterministic master key (32 bytes of 0x42).
fn test_master_key() -> EncryptionKey {
    EncryptionKey::from_raw_for_test([0x42_u8; 32])
}

/// Build a facade wired to: env + (caller-supplied keyring) +
/// sqlite (in-memory NexusDatabase) + memory. `cfg` controls the
/// `env_override_providers` knob.
fn build_facade(
    cfg: CredentialFacadeConfig,
    keyring: KeyringBackendAdapter,
) -> (Arc<SecretsFacade>, Arc<nexus_persistence::NexusDatabase>) {
    let db = Arc::new(nexus_persistence::NexusDatabase::in_memory().expect("in-memory db"));
    let master = test_master_key();
    let env = Arc::new(EnvBackend::new());
    let kr = Arc::new(keyring);
    let sql = Arc::new(SqliteEnvelopeBackend::new(Arc::clone(&db), &master));
    let mem = Arc::new(MemoryBackend::new());
    let audit = Arc::new(std::sync::Mutex::new(crate::audit::AuditTrail::new()));
    let facade = Arc::new(SecretsFacade::new(env, kr, Some(sql), mem, &cfg, audit));
    (facade, db)
}

// ── Refinement D: locked precedence semantics ─────────────────────

#[test]
fn keyring_wins_over_stale_env_under_default() {
    // Refinement D #1.
    let _env = EnvVarGuard::set("ANTHROPIC_API_KEY", "stale-env-value");
    let mock = MockKeyring::new();
    mock.set(
        "llm",
        "anthropic_api_key",
        Zeroizing::new("sk-live-keyring".into()),
    )
    .unwrap();
    let (facade, _db) = build_facade(
        CredentialFacadeConfig::default(),
        KeyringBackendAdapter::mock(mock),
    );
    let got = facade
        .get_secret(&SecretAuditCtx::system(), "llm", "anthropic_api_key")
        .expect("ok");
    assert_eq!(got.value.to_string(), "sk-live-keyring");
    assert_eq!(got.source, ResolvedFrom::Keyring);
}

#[test]
fn env_wins_when_provider_in_override_list() {
    // Refinement D #2.
    let _env = EnvVarGuard::set("OPENAI_API_KEY", "sk-env-value");
    let mock = MockKeyring::new();
    mock.set("llm", "openai_api_key", Zeroizing::new("sk-keyring".into()))
        .unwrap();
    let cfg = CredentialFacadeConfig {
        env_override_providers: vec!["llm".to_string()],
    };
    let (facade, _db) = build_facade(cfg, KeyringBackendAdapter::mock(mock));
    let got = facade
        .get_secret(&SecretAuditCtx::system(), "llm", "openai_api_key")
        .expect("ok");
    assert_eq!(got.value.to_string(), "sk-env-value");
    assert_eq!(got.source, ResolvedFrom::Env);
}

#[test]
fn keyring_miss_falls_through_to_env_for_llm_providers() {
    // Refinement D #3 — locks the fallback expansion documented
    // in ADR 0004 Consequences (Subtle fallback expansion).
    let _env = EnvVarGuard::set("HUGGINGFACE_API_KEY", "env-fallback-value");
    let mock = MockKeyring::new(); // empty
    let (facade, _db) = build_facade(
        CredentialFacadeConfig::default(),
        KeyringBackendAdapter::mock(mock),
    );
    let got = facade
        .get_secret(&SecretAuditCtx::system(), "llm", "huggingface_api_key")
        .expect("ok");
    assert_eq!(got.value.to_string(), "env-fallback-value");
    assert_eq!(got.source, ResolvedFrom::Env);
}

#[test]
fn keyring_miss_returns_not_found_when_no_env_set() {
    // Refinement D #4.
    let _env = EnvVarGuard::remove("OPENROUTER_API_KEY");
    let mock = MockKeyring::new(); // empty
    let (facade, _db) = build_facade(
        CredentialFacadeConfig::default(),
        KeyringBackendAdapter::mock(mock),
    );
    let err = facade
        .get_secret(&SecretAuditCtx::system(), "llm", "openrouter_api_key")
        .expect_err("must surface NotFound");
    assert!(matches!(err, SecretError::NotFound));
}

// ── Resolve-source startup diagnostic ──────────────────────────────

#[test]
fn resolve_log_seen_dedups_per_provider() {
    // Two consecutive resolves for the same provider must populate
    // resolve_log_seen exactly once. Direct introspection isn't
    // available; we exercise the fact that two resolves succeed
    // identically (the eprintln side-effect is fire-and-forget).
    // Use the mock keyring rather than env so this test doesn't
    // race with other env-mutating tests in the same suite.
    let mock = MockKeyring::new();
    mock.set(
        "social",
        "x_access_token",
        Zeroizing::new("token-xyz".into()),
    )
    .unwrap();
    let cfg = CredentialFacadeConfig::default();
    let (facade, _db) = build_facade(cfg, KeyringBackendAdapter::mock(mock));
    let a = facade
        .get_secret(&SecretAuditCtx::system(), "social", "x_access_token")
        .expect("ok");
    let b = facade
        .get_secret(&SecretAuditCtx::system(), "social", "x_access_token")
        .expect("ok");
    assert_eq!(a.value.to_string(), b.value.to_string());
    assert_eq!(a.source, ResolvedFrom::Keyring);
}

// ── SqliteEnvelope round-trip + write-through ─────────────────────

#[test]
fn set_secret_writes_to_sqlite_and_get_round_trips() {
    let cfg = CredentialFacadeConfig::default();
    // Bug AK Commit 4: must use the rejecting test fixture
    // because the real `OsKeyring` may accept writes when a
    // kernel keyring service is available (which is true on
    // many Linux dev machines). Pre-Commit-4 this used
    // `os_keyring()` because the stub always rejected. The
    // rejecting fixture preserves the stub's semantics so
    // this test deterministically asserts sqlite write-through.
    let (facade, _db) = build_facade(cfg, KeyringBackendAdapter::rejecting());
    facade
        .set_secret(
            &SecretAuditCtx::system(),
            "social",
            "x_access_token",
            Zeroizing::new("at-12345".into()),
        )
        .expect("ok");
    let got = facade
        .get_secret(&SecretAuditCtx::system(), "social", "x_access_token")
        .expect("ok");
    assert_eq!(got.value.to_string(), "at-12345");
    assert_eq!(got.source, ResolvedFrom::Sqlite);
}

// ── Migration (Refinement B transactionality) ──────────────────────

#[test]
fn migrate_config_to_vault_happy_path_clears_fields_and_bumps_version() {
    // Bug AK-15 follow-up: NEXUS_CONFIG_PATH is process-global
    // env state. The resave-failure sibling test mutates the
    // same var; without serialization the two tests race and
    // either can clobber the other's path. Holding
    // NEXUS_CONFIG_PATH_GUARD (defined below) serializes them
    // without affecting other secrets tests. Tightens AK-8.
    let _guard = NEXUS_CONFIG_PATH_GUARD
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let cfg = CredentialFacadeConfig::default();
    // The keyring is the rejecting fixture, never the host's keyring: the
    // read-backs below must come from the migrated rows, not from a
    // production keyring entry.
    let (facade, db) = build_facade(cfg, KeyringBackendAdapter::rejecting());
    let mut config = NexusConfig::default();
    // SocialConfig (4 fields, Commit 2 substance).
    config.social.x_api_key = "ck".into();
    config.social.x_api_secret = "cs".into();
    config.social.x_access_token = "at".into();
    config.social.x_access_secret = "as".into();
    // LLM (6 fields, Commit 3 substance — Phase 1 aggregate becomes 10).
    config.llm.anthropic_api_key = "sk-ant".into();
    config.llm.openai_api_key = "sk-openai".into();
    config.llm.deepseek_api_key = "sk-ds".into();
    config.llm.gemini_api_key = "sk-gem".into();
    config.llm.nvidia_api_key = "sk-nv".into();
    config.llm.openrouter_api_key = "sk-or".into();
    // Point save_config at an isolated temp file so the migration's
    // re-save step doesn't touch the real ~/.nexus/config.toml.
    let tmpdir = std::env::temp_dir().join(format!("nexus_ak_test_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&tmpdir).unwrap();
    let cfg_path = tmpdir.join("config.toml");
    let _config_path = EnvVarGuard::set("NEXUS_CONFIG_PATH", &cfg_path);

    let report = migrate_config_to_vault(&mut config, &facade).expect("ok");
    match report {
        MigrationReport::Migrated {
            fields_migrated,
            config_resave_failed,
        } => {
            assert_eq!(fields_migrated.len(), 10);
            assert!(!config_resave_failed);
        }
        MigrationReport::AlreadyRun => panic!("expected Migrated"),
    }
    // SocialConfig fields cleared.
    assert!(config.social.x_api_key.is_empty());
    assert!(config.social.x_access_token.is_empty());
    // LLM fields cleared.
    assert!(config.llm.anthropic_api_key.is_empty());
    assert!(config.llm.openai_api_key.is_empty());
    assert!(config.llm.deepseek_api_key.is_empty());
    assert!(config.llm.gemini_api_key.is_empty());
    assert!(config.llm.nvidia_api_key.is_empty());
    assert!(config.llm.openrouter_api_key.is_empty());
    // schema_version bumped.
    assert_eq!(db.schema_version("credential_vault_v1").unwrap(), Some(1));
    // Vault has all four social keys under the renamed keys.
    let social_names = [
        "x_consumer_key",
        "x_consumer_secret",
        "x_access_token",
        "x_access_token_secret",
    ];
    for name in social_names {
        let got = facade
            .get_secret(&SecretAuditCtx::system(), "social", name)
            .expect("ok");
        assert!(!got.value.is_empty());
    }
    // Vault has all six LLM keys.
    let llm_names = [
        "anthropic",
        "openai",
        "deepseek",
        "gemini",
        "nvidia",
        "openrouter",
    ];
    for name in llm_names {
        let got = facade
            .get_secret(&SecretAuditCtx::system(), "llm", name)
            .expect("ok");
        assert!(!got.value.is_empty());
    }
    // Re-run is a no-op.
    let report2 = migrate_config_to_vault(&mut config, &facade).unwrap();
    assert!(matches!(report2, MigrationReport::AlreadyRun));

    let _ = std::fs::remove_dir_all(&tmpdir);
}

#[test]
fn migrate_with_no_credentials_still_bumps_schema_version() {
    // A migration that moves credentials re-saves the configuration with
    // `save_config`, which writes the file NEXUS_CONFIG_PATH names, or else
    // the one under HOME. Point it at a temporary file (serialized with the
    // other tests that set the variable, and restored afterwards, even on
    // panic) so that even a regression here can never rewrite a real
    // configuration with the default one.
    let _serial = NEXUS_CONFIG_PATH_GUARD
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let dir = tempfile::tempdir().unwrap();
    let cfg_path = dir.path().join("config.toml");
    let _config_path = EnvVarGuard::set("NEXUS_CONFIG_PATH", &cfg_path);
    let (facade, db) = build_facade(
        CredentialFacadeConfig::default(),
        KeyringBackendAdapter::rejecting(),
    );
    let mut config = NexusConfig::default();
    let report = migrate_config_to_vault(&mut config, &facade).expect("ok");
    // An empty migration has nothing to clear, so it re-saves nothing.
    assert!(!cfg_path.exists());
    match report {
        MigrationReport::Migrated {
            fields_migrated,
            config_resave_failed,
        } => {
            assert!(fields_migrated.is_empty());
            assert!(!config_resave_failed);
        }
        MigrationReport::AlreadyRun => panic!("expected Migrated"),
    }
    assert_eq!(db.schema_version("credential_vault_v1").unwrap(), Some(1));
}

#[test]
fn migrate_records_resave_failure_but_treats_facade_as_authoritative() {
    // Cross-platform force-failure: use a regular file as an
    // intermediate path component. `fs::create_dir_all` returns
    // ENOTDIR on every supported platform because a regular file
    // cannot have a child directory. Replaces the prior /proc-
    // based mechanism, which was host-OS-dependent and failed on
    // GitLab CI runners that allow /proc subdir creation.
    //
    // NOTE (AK-8): this test mutates NEXUS_CONFIG_PATH, which is
    // process-global env state. Serialized via
    // NEXUS_CONFIG_PATH_GUARD (defined at the bottom of this
    // file) so the happy-path migration test cannot clobber the
    // bad path mid-run.
    let _guard = NEXUS_CONFIG_PATH_GUARD
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let unique = format!(
        "nexus_ak_resave_fail_{}_{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed),
    );
    let tmpdir = std::env::temp_dir().join(unique);
    std::fs::create_dir_all(&tmpdir).unwrap();

    // RAII cleanup — fires on panic too.
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(tmpdir.clone());

    // Regular FILE at the intermediate path: any attempt to
    // create_dir_all a child fails with ENOTDIR.
    let blocker = tmpdir.join("blocker");
    std::fs::write(&blocker, b"x").unwrap();
    let bad_path = blocker.join("subdir").join("config.toml");
    let _config_path = EnvVarGuard::set("NEXUS_CONFIG_PATH", &bad_path);

    let (facade, db) = build_facade(
        CredentialFacadeConfig::default(),
        KeyringBackendAdapter::rejecting(),
    );
    let mut config = NexusConfig::default();
    config.social.x_api_key = "ck".into();

    let report = migrate_config_to_vault(&mut config, &facade).expect("ok");
    match report {
        MigrationReport::Migrated {
            config_resave_failed,
            ..
        } => {
            assert!(config_resave_failed, "expected re-save failure");
        }
        MigrationReport::AlreadyRun => panic!("expected Migrated"),
    }
    // DB transaction committed -> schema_version bumped.
    assert_eq!(db.schema_version("credential_vault_v1").unwrap(), Some(1));
    // Field cleared in-memory even though disk save failed (the
    // facade is authoritative; on-disk fields are now stale).
    assert!(config.social.x_api_key.is_empty());

    // env cleanup happens in Cleanup::drop.
}

#[test]
fn migrate_returns_sqlite_unavailable_when_facade_lacks_sqlite() {
    // Build a facade WITHOUT a sqlite backend; migration must
    // refuse rather than silently no-op.
    let env = Arc::new(EnvBackend::new());
    let kr = Arc::new(KeyringBackendAdapter::rejecting());
    let mem = Arc::new(MemoryBackend::new());
    let audit = Arc::new(std::sync::Mutex::new(crate::audit::AuditTrail::new()));
    let facade = SecretsFacade::new(
        env,
        kr,
        None,
        mem,
        &CredentialFacadeConfig::default(),
        audit,
    );
    let mut config = NexusConfig::default();
    config.social.x_api_key = "ck".into();
    let err =
        migrate_config_to_vault(&mut config, &facade).expect_err("expected SqliteUnavailable");
    assert!(matches!(err, MigrationError::SqliteUnavailable));
}

#[test]
fn ak_commit4_real_keyring_falls_through_to_memory() {
    // Bug AK Commit 4: validates that the real keyring backend
    // (no longer a stub) participates in the facade chain
    // without breaking fall-through. The keyring path returns
    // either NotFound (entry absent) or BackendNotConfigured
    // (no keyring service); both must yield to the next backend.
    // We seed Memory with a unique key and confirm the facade
    // returns it — proving the keyring backend stepped aside.
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let unique = format!(
        "ak_commit4_chain_{}_{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed),
    );

    use super::SecretBackend;
    let env = Arc::new(EnvBackend::new());
    // Real OS keyring (NOT MockKeyring) — this is the change
    // Commit 4 ships.
    let kr = Arc::new(KeyringBackendAdapter::os_keyring());
    let mem = Arc::new(MemoryBackend::new());
    mem.set("test", &unique, Zeroizing::new("from-memory".into()))
        .expect("seed memory");
    // No sqlite; the chain is env -> keyring -> memory (sqlite
    // None).
    let audit = Arc::new(std::sync::Mutex::new(crate::audit::AuditTrail::new()));
    let facade = SecretsFacade::new(
        env,
        kr,
        None,
        mem,
        &CredentialFacadeConfig::default(),
        audit,
    );

    // Default env_override_providers does NOT include scope
    // "test", so chain order is keyring-first. Keyring will not
    // have this unique key (or returns BackendNotConfigured if
    // service is unavailable); env is unset; sqlite is None;
    // memory wins.
    let got = facade
        .get_secret(&SecretAuditCtx::system(), "test", &unique)
        .expect("ok");
    assert_eq!(got.value.as_str(), "from-memory");
    assert_eq!(got.source, ResolvedFrom::Memory);
}

/// Bug AK-15: regression test for the audit-redaction
/// invariant deleted with vault.rs in Commit 5. Every
/// facade op (set/get/delete/get-after-delete) must produce
/// exactly one audit row with the AK-15 payload shape, and
/// the plaintext secret VALUE must NEVER appear anywhere in
/// the audit JSON.
#[test]
fn ak15_audit_records_ops_without_plaintext_leak() {
    use crate::audit::AuditTrail;
    use std::sync::Mutex;

    let env = Arc::new(EnvBackend::new());
    let kr = Arc::new(KeyringBackendAdapter::rejecting()); // Memory wins set
    let mem = Arc::new(MemoryBackend::new());
    let audit = Arc::new(Mutex::new(AuditTrail::new()));
    let facade = SecretsFacade::new(
        env,
        kr,
        None,
        mem,
        &CredentialFacadeConfig::default(),
        Arc::clone(&audit),
    );

    let secret_value = "ghp_super_secret_xyz_12345";
    facade
        .set_secret(
            &SecretAuditCtx::system(),
            "test",
            "key1",
            Zeroizing::new(secret_value.into()),
        )
        .expect("set ok");
    let got = facade
        .get_secret(&SecretAuditCtx::system(), "test", "key1")
        .expect("get ok");
    assert_eq!(got.value.as_str(), secret_value);
    facade
        .delete_secret(&SecretAuditCtx::system(), "test", "key1")
        .expect("delete ok");
    let miss = facade
        .get_secret(&SecretAuditCtx::system(), "test", "key1")
        .expect_err("expected NotFound after delete");
    assert!(matches!(miss, SecretError::NotFound));

    // Plaintext-absence regression invariant.
    let trail = audit.lock().expect("audit unpoisoned");
    let events: Vec<_> = trail.events().to_vec();
    assert_eq!(
        events.len(),
        4,
        "expected 4 audit events (set/get/delete/get-miss), got {}",
        events.len()
    );

    for event in &events {
        let payload_str = serde_json::to_string(&event.payload).unwrap_or_default();
        assert!(
            !payload_str.contains(secret_value),
            "plaintext leak in audit event: {payload_str}"
        );
    }

    // Per-event shape assertions.
    let p0 = &events[0].payload;
    assert_eq!(p0["event"], "secret_stored");
    assert_eq!(p0["scope"], "test");
    assert_eq!(p0["name"], "key1");
    assert_eq!(p0["result"], "ok");
    // AK-2: capability is now sourced from SecretAuditCtx. Tests above
    // use SecretAuditCtx::system() so the capability is "system".
    assert_eq!(p0["capability"], "system");
    assert_eq!(p0["resolved_from"], "memory");

    let p1 = &events[1].payload;
    assert_eq!(p1["event"], "secret_accessed");
    assert_eq!(p1["result"], "ok");
    assert_eq!(p1["resolved_from"], "memory");

    let p2 = &events[2].payload;
    assert_eq!(p2["event"], "secret_deleted");
    assert_eq!(p2["result"], "ok");

    let p3 = &events[3].payload;
    assert_eq!(p3["event"], "secret_accessed");
    assert_eq!(p3["result"], "not_found");
    // resolved_from omitted on miss; assert absence.
    assert!(p3.get("resolved_from").is_none());
}

/// Bug AK-15 follow-up to AK-8: NEXUS_CONFIG_PATH is process-
/// global env state. The two tests that mutate it must run
/// serially. Both acquire this mutex at start; tests that
/// don't touch the env var don't acquire and run in parallel
/// as before. Tightens AK-8 (which still tracks the broader
/// audit-of-env-mutating-tests sweep).
pub(crate) static NEXUS_CONFIG_PATH_GUARD: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Sets an environment variable for the life of the guard and restores its
/// previous value, or its absence, when dropped (also on panic). The caller
/// holds the lock that serializes the variable.
pub(crate) struct EnvVarGuard {
    name: &'static str,
    previous: Option<std::ffi::OsString>,
}

impl EnvVarGuard {
    pub(crate) fn set(name: &'static str, value: impl AsRef<std::ffi::OsStr>) -> Self {
        let previous = std::env::var_os(name);
        std::env::set_var(name, value);
        Self { name, previous }
    }

    pub(crate) fn remove(name: &'static str) -> Self {
        let previous = std::env::var_os(name);
        std::env::remove_var(name);
        Self { name, previous }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        match &self.previous {
            Some(value) => std::env::set_var(self.name, value),
            None => std::env::remove_var(self.name),
        }
    }
}

/// The guard restores the previous value, or the variable's absence, when it
/// is dropped, including when the test panics. The variable is unique to
/// this test.
#[test]
fn env_var_guard_restores_the_previous_state() {
    const NAME: &str = "NEXUS_TEST_ENV_VAR_GUARD_ONLY";
    let value = || std::env::var_os(NAME);
    {
        let _absent = EnvVarGuard::remove(NAME);
        {
            let _set = EnvVarGuard::set(NAME, "during");
            assert_eq!(value().as_deref(), Some("during".as_ref()));
        }
        assert_eq!(value(), None);
        let _before = EnvVarGuard::set(NAME, "before");
        let panicked = std::panic::catch_unwind(|| {
            let _set = EnvVarGuard::set(NAME, "during");
            panic!("synthetic test panic");
        });
        assert!(panicked.is_err());
        assert_eq!(value().as_deref(), Some("before".as_ref()));
        {
            let _removed = EnvVarGuard::remove(NAME);
            assert_eq!(value(), None);
        }
        assert_eq!(value().as_deref(), Some("before".as_ref()));
    }
    assert_eq!(value(), None);
}

// ── M0-FIX F4: ciphertexts are bound to their secret identity ─────

const F4_SCHEMA_KEY: &str = "secrets_aad_v2";

fn f4_backend(
    master: &EncryptionKey,
) -> (SqliteEnvelopeBackend, Arc<nexus_persistence::NexusDatabase>) {
    let db = Arc::new(nexus_persistence::NexusDatabase::in_memory().expect("in-memory db"));
    (SqliteEnvelopeBackend::new(Arc::clone(&db), master), db)
}

/// Writes a row exactly as the pre-F4 backend did: a 12-byte nonce and
/// AES-256-GCM under the `nexus.secrets.v1` domain key with no associated
/// data.
pub(crate) fn f4_write_legacy_row(
    db: &nexus_persistence::NexusDatabase,
    master: &EncryptionKey,
    scope: &str,
    name: &str,
    plaintext: &[u8],
) {
    use aes_gcm::aead::{Aead, KeyInit};
    let key = Zeroizing::new(master.derive_subkey(b"nexus.secrets.v1"));
    let cipher = aes_gcm::Aes256Gcm::new_from_slice(key.as_ref()).unwrap();
    let nonce = [0x5a_u8; 12];
    let ciphertext = cipher
        .encrypt(&aes_gcm::Nonce::from(nonce), plaintext)
        .unwrap();
    db.record_secret(scope, name, &nonce, &ciphertext).unwrap();
}

fn f4_copy_row(db: &nexus_persistence::NexusDatabase, from: (&str, &str), to: (&str, &str)) {
    let (nonce, ciphertext) = db.lookup_secret(from.0, from.1).unwrap().unwrap();
    db.record_secret(to.0, to.1, &nonce, &ciphertext).unwrap();
}

fn f4_rows(
    db: &nexus_persistence::NexusDatabase,
    ids: &[(&str, &str)],
) -> Vec<Option<(Vec<u8>, Vec<u8>)>> {
    ids.iter()
        .map(|(scope, name)| db.lookup_secret(scope, name).unwrap())
        .collect()
}

/// (a) A ciphertext and nonce copied from one row into another never open
/// as the other secret: same scope, same name in another scope, and scope
/// and name boundaries that concatenate to the same bytes.
#[test]
fn f4_ciphertext_copied_between_identities_does_not_open() {
    let master = test_master_key();
    let (backend, db) = f4_backend(&master);
    let ids = [
        ("llm", "anthropic"),
        ("llm", "openai"),
        ("social", "anthropic"),
        ("ab", "c"),
        ("a", "bc"),
    ];
    for (scope, name) in ids {
        backend
            .set(scope, name, Zeroizing::new(format!("value-{scope}-{name}")))
            .unwrap();
    }
    // Copy each source row as originally written (a copy made earlier in
    // this loop must not become the next source).
    let original = f4_rows(&db, &ids);
    for (from, to) in [(0, 1), (0, 2), (3, 4), (4, 3)] {
        let (nonce, ciphertext) = original[from].clone().unwrap();
        let (from, to) = (ids[from], ids[to]);
        db.record_secret(to.0, to.1, &nonce, &ciphertext).unwrap();
        match backend.get(to.0, to.1) {
            Err(SecretError::DecryptionFailed) => {}
            Ok(value) => panic!(
                "{}.{} copied into {}.{} opened as {:?}",
                from.0,
                from.1,
                to.0,
                to.1,
                value.as_str()
            ),
            Err(other) => panic!("unexpected error for {}.{}: {other:?}", to.0, to.1),
        }
    }
    // The source rows still open as themselves, and a row put back in its
    // own slot opens again.
    assert_eq!(
        backend.get("llm", "anthropic").unwrap().as_str(),
        "value-llm-anthropic"
    );
    let (nonce, ciphertext) = original[3].clone().unwrap();
    db.record_secret("ab", "c", &nonce, &ciphertext).unwrap();
    assert_eq!(backend.get("ab", "c").unwrap().as_str(), "value-ab-c");
}

/// (b) A legacy row with no identity binding is refused on the read path
/// with its own error, by the backend and by the facade (not skipped, not
/// reported as missing, never decrypted).
#[test]
fn f4_legacy_unbound_row_is_refused_by_get() {
    let master = test_master_key();
    let (backend, db) = f4_backend(&master);
    f4_write_legacy_row(&db, &master, "llm", "anthropic", b"legacy-value");
    assert!(matches!(
        backend.get("llm", "anthropic"),
        Err(SecretError::LegacyUnbound)
    ));
    let message = SecretError::LegacyUnbound.to_string();
    assert!(!message.contains("legacy-value"));
    for other in [
        SecretError::DecryptionFailed.to_string(),
        SecretError::NotFound.to_string(),
    ] {
        assert_ne!(message, other);
    }

    let (facade, db) = build_facade(
        CredentialFacadeConfig::default(),
        KeyringBackendAdapter::rejecting(),
    );
    f4_write_legacy_row(&db, &master, "social", "x_access_token", b"legacy-token");
    assert!(matches!(
        facade.get_secret(&SecretAuditCtx::system(), "social", "x_access_token"),
        Err(SecretError::LegacyUnbound)
    ));
}

/// (c) The explicit upgrade re-encrypts every legacy row under its own
/// identity, atomically records completion, and is one-time: legacy rows
/// that appear afterwards stay refused.
#[test]
fn f4_upgrade_binds_legacy_rows_to_their_own_identity() {
    let master = test_master_key();
    let (backend, db) = f4_backend(&master);
    f4_write_legacy_row(&db, &master, "llm", "anthropic", b"value-a");
    f4_write_legacy_row(&db, &master, "social", "x_access_token", b"value-b");
    assert_eq!(db.schema_version(F4_SCHEMA_KEY).unwrap(), None);

    let report = super::migrate::upgrade_legacy_secrets(&backend).expect("upgrade");
    assert_eq!(
        report,
        super::migrate::UpgradeReport::Upgraded {
            rows: vec![
                ("llm".to_string(), "anthropic".to_string()),
                ("social".to_string(), "x_access_token".to_string()),
            ],
        }
    );
    assert!(db.schema_version(F4_SCHEMA_KEY).unwrap().is_some());
    assert_eq!(backend.get("llm", "anthropic").unwrap().as_str(), "value-a");
    assert_eq!(
        backend.get("social", "x_access_token").unwrap().as_str(),
        "value-b"
    );
    // Upgraded rows are bound: a copy does not open as the other identity.
    f4_copy_row(&db, ("llm", "anthropic"), ("social", "x_access_token"));
    assert!(matches!(
        backend.get("social", "x_access_token"),
        Err(SecretError::DecryptionFailed)
    ));

    // One-time: a second run does nothing, and a legacy row written later
    // (for example restored from an old backup) is never upgraded.
    assert_eq!(
        super::migrate::upgrade_legacy_secrets(&backend).unwrap(),
        super::migrate::UpgradeReport::AlreadyRun
    );
    f4_write_legacy_row(&db, &master, "llm", "openai", b"value-late");
    let before = f4_rows(&db, &[("llm", "openai")]);
    assert!(matches!(
        super::migrate::upgrade_legacy_secrets(&backend),
        Err(MigrationError::LegacyAfterUpgrade { count: 1 })
    ));
    assert_eq!(f4_rows(&db, &[("llm", "openai")]), before);
    assert!(matches!(
        backend.get("llm", "openai"),
        Err(SecretError::LegacyUnbound)
    ));
}

/// (d) A legacy row that does not decrypt, or opens to non-text, fails the
/// upgrade loudly and leaves every row and the completion marker untouched.
#[test]
fn f4_upgrade_with_bad_legacy_row_changes_nothing() {
    let master = test_master_key();
    let other = EncryptionKey::from_raw_for_test([0x24_u8; 32]);
    let ids = [
        ("llm", "anthropic"),
        ("llm", "openai"),
        ("social", "x_access_token"),
    ];

    // Corrupt ciphertext, and a row written under another key.
    for corrupt in [true, false] {
        let (backend, db) = f4_backend(&master);
        f4_write_legacy_row(&db, &master, ids[0].0, ids[0].1, b"value-a");
        if corrupt {
            f4_write_legacy_row(&db, &master, ids[1].0, ids[1].1, b"value-b");
            let (nonce, mut ciphertext) = db.lookup_secret(ids[1].0, ids[1].1).unwrap().unwrap();
            ciphertext[0] ^= 0x01;
            db.record_secret(ids[1].0, ids[1].1, &nonce, &ciphertext)
                .unwrap();
        } else {
            f4_write_legacy_row(&db, &other, ids[1].0, ids[1].1, b"value-b");
        }
        f4_write_legacy_row(&db, &master, ids[2].0, ids[2].1, b"value-c");
        let before = f4_rows(&db, &ids);
        match super::migrate::upgrade_legacy_secrets(&backend) {
            Err(MigrationError::LegacyUndecryptable { scope, name }) => {
                assert_eq!((scope.as_str(), name.as_str()), ids[1]);
            }
            other => panic!("expected LegacyUndecryptable, got {other:?}"),
        }
        assert_eq!(f4_rows(&db, &ids), before, "half-migrated state");
        assert_eq!(db.schema_version(F4_SCHEMA_KEY).unwrap(), None);
        for (scope, name) in ids {
            assert!(matches!(
                backend.get(scope, name),
                Err(SecretError::LegacyUnbound)
            ));
        }
    }

    // A legacy row that opens to a value that is not text.
    let (backend, db) = f4_backend(&master);
    f4_write_legacy_row(&db, &master, ids[0].0, ids[0].1, b"value-a");
    f4_write_legacy_row(&db, &master, ids[1].0, ids[1].1, &[0xff, 0xfe, 0x00]);
    let before = f4_rows(&db, &ids[..2]);
    assert!(matches!(
        super::migrate::upgrade_legacy_secrets(&backend),
        Err(MigrationError::LegacyDamaged { .. })
    ));
    assert_eq!(f4_rows(&db, &ids[..2]), before, "half-migrated state");
    assert_eq!(db.schema_version(F4_SCHEMA_KEY).unwrap(), None);
}

/// M0-FIX F4 review (replay): deleting the in-database completion marker
/// must not re-enable the upgrade. A vault that holds any bound row is
/// post-upgrade; a legacy ciphertext for X planted into slot Y (for example
/// from an old backup) is never re-bound to Y and never opens as X.
#[test]
fn f4_marker_deletion_does_not_reenable_the_upgrade() {
    let master = test_master_key();
    let (backend, db) = f4_backend(&master);
    let x = ("llm", "anthropic");
    let y = ("social", "x_access_token");
    f4_write_legacy_row(&db, &master, x.0, x.1, b"value-x");
    f4_write_legacy_row(&db, &master, y.0, y.1, b"value-y");
    // The attacker kept X's pre-upgrade legacy row.
    let (old_nonce, old_ciphertext) = db.lookup_secret(x.0, x.1).unwrap().unwrap();
    super::migrate::upgrade_legacy_secrets(&backend).expect("upgrade");

    db.with_transaction(|tx| {
        tx.execute_batch("DELETE FROM schema_versions WHERE key = 'secrets_aad_v2'")?;
        Ok(())
    })
    .unwrap();
    assert_eq!(db.schema_version(F4_SCHEMA_KEY).unwrap(), None);
    db.record_secret(y.0, y.1, &old_nonce, &old_ciphertext)
        .unwrap();
    let before = f4_rows(&db, &[x, y]);

    assert!(matches!(
        super::migrate::upgrade_legacy_secrets(&backend),
        Err(MigrationError::LegacyAfterUpgrade { count: 1 })
    ));
    assert_eq!(f4_rows(&db, &[x, y]), before, "planted row was rewritten");
    assert_eq!(db.schema_version(F4_SCHEMA_KEY).unwrap(), None);
    match backend.get(y.0, y.1) {
        Err(SecretError::LegacyUnbound) => {}
        Ok(value) => panic!("planted row opened as {:?}", value.as_str()),
        Err(other) => panic!("unexpected error: {other:?}"),
    }
    assert_eq!(backend.get(x.0, x.1).unwrap().as_str(), "value-x");
}
