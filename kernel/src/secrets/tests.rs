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
use super::{ResolvedFrom, SecretError, SecretsFacade};
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
    let facade = Arc::new(SecretsFacade::new(env, kr, Some(sql), mem, &cfg));
    (facade, db)
}

// ── Refinement D: locked precedence semantics ─────────────────────

#[test]
fn keyring_wins_over_stale_env_under_default() {
    // Refinement D #1.
    std::env::set_var("ANTHROPIC_API_KEY", "stale-env-value");
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
    let got = facade.get_secret("llm", "anthropic_api_key").expect("ok");
    assert_eq!(got.value.to_string(), "sk-live-keyring");
    assert_eq!(got.source, ResolvedFrom::Keyring);
    std::env::remove_var("ANTHROPIC_API_KEY");
}

#[test]
fn env_wins_when_provider_in_override_list() {
    // Refinement D #2.
    std::env::set_var("OPENAI_API_KEY", "sk-env-value");
    let mock = MockKeyring::new();
    mock.set("llm", "openai_api_key", Zeroizing::new("sk-keyring".into()))
        .unwrap();
    let cfg = CredentialFacadeConfig {
        env_override_providers: vec!["llm".to_string()],
    };
    let (facade, _db) = build_facade(cfg, KeyringBackendAdapter::mock(mock));
    let got = facade.get_secret("llm", "openai_api_key").expect("ok");
    assert_eq!(got.value.to_string(), "sk-env-value");
    assert_eq!(got.source, ResolvedFrom::Env);
    std::env::remove_var("OPENAI_API_KEY");
}

#[test]
fn keyring_miss_falls_through_to_env_for_llm_providers() {
    // Refinement D #3 — locks the fallback expansion documented
    // in ADR 0004 Consequences (Subtle fallback expansion).
    std::env::set_var("HUGGINGFACE_API_KEY", "env-fallback-value");
    let mock = MockKeyring::new(); // empty
    let (facade, _db) = build_facade(
        CredentialFacadeConfig::default(),
        KeyringBackendAdapter::mock(mock),
    );
    let got = facade.get_secret("llm", "huggingface_api_key").expect("ok");
    assert_eq!(got.value.to_string(), "env-fallback-value");
    assert_eq!(got.source, ResolvedFrom::Env);
    std::env::remove_var("HUGGINGFACE_API_KEY");
}

#[test]
fn keyring_miss_returns_not_found_when_no_env_set() {
    // Refinement D #4.
    std::env::remove_var("OPENROUTER_API_KEY");
    let mock = MockKeyring::new(); // empty
    let (facade, _db) = build_facade(
        CredentialFacadeConfig::default(),
        KeyringBackendAdapter::mock(mock),
    );
    let err = facade
        .get_secret("llm", "openrouter_api_key")
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
    let a = facade.get_secret("social", "x_access_token").expect("ok");
    let b = facade.get_secret("social", "x_access_token").expect("ok");
    assert_eq!(a.value.to_string(), b.value.to_string());
    assert_eq!(a.source, ResolvedFrom::Keyring);
}

// ── SqliteEnvelope round-trip + write-through ─────────────────────

#[test]
fn set_secret_writes_to_sqlite_and_get_round_trips() {
    let cfg = CredentialFacadeConfig::default();
    // Use the OsKeyring stub here so that `set` falls through to
    // sqlite (the stub returns BackendNotConfigured on every op).
    // MockKeyring would itself accept the write and short-circuit.
    let (facade, _db) = build_facade(cfg, KeyringBackendAdapter::os_keyring());
    facade
        .set_secret(
            "social",
            "x_access_token",
            Zeroizing::new("at-12345".into()),
        )
        .expect("ok");
    let got = facade.get_secret("social", "x_access_token").expect("ok");
    assert_eq!(got.value.to_string(), "at-12345");
    assert_eq!(got.source, ResolvedFrom::Sqlite);
}

// ── Migration (Refinement B transactionality) ──────────────────────

#[test]
fn migrate_config_to_vault_happy_path_clears_fields_and_bumps_version() {
    let cfg = CredentialFacadeConfig::default();
    let (facade, db) = build_facade(cfg, KeyringBackendAdapter::os_keyring());
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
    std::env::set_var("NEXUS_CONFIG_PATH", &cfg_path);

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
        let got = facade.get_secret("social", name).expect("ok");
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
        let got = facade.get_secret("llm", name).expect("ok");
        assert!(!got.value.is_empty());
    }
    // Re-run is a no-op.
    let report2 = migrate_config_to_vault(&mut config, &facade).unwrap();
    assert!(matches!(report2, MigrationReport::AlreadyRun));

    std::env::remove_var("NEXUS_CONFIG_PATH");
    let _ = std::fs::remove_dir_all(&tmpdir);
}

#[test]
fn migrate_with_no_credentials_still_bumps_schema_version() {
    let (facade, db) = build_facade(
        CredentialFacadeConfig::default(),
        KeyringBackendAdapter::os_keyring(),
    );
    let mut config = NexusConfig::default();
    let report = migrate_config_to_vault(&mut config, &facade).expect("ok");
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
    // process-global env state. Serialization with other env-
    // mutating secrets tests is tracked by AK-8 in the Phase 1.5
    // backlog.
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
            std::env::remove_var("NEXUS_CONFIG_PATH");
        }
    }
    let _guard = Cleanup(tmpdir.clone());

    // Regular FILE at the intermediate path: any attempt to
    // create_dir_all a child fails with ENOTDIR.
    let blocker = tmpdir.join("blocker");
    std::fs::write(&blocker, b"x").unwrap();
    let bad_path = blocker.join("subdir").join("config.toml");
    std::env::set_var("NEXUS_CONFIG_PATH", &bad_path);

    let (facade, db) = build_facade(
        CredentialFacadeConfig::default(),
        KeyringBackendAdapter::os_keyring(),
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
    let kr = Arc::new(KeyringBackendAdapter::os_keyring());
    let mem = Arc::new(MemoryBackend::new());
    let facade = SecretsFacade::new(env, kr, None, mem, &CredentialFacadeConfig::default());
    let mut config = NexusConfig::default();
    config.social.x_api_key = "ck".into();
    let err =
        migrate_config_to_vault(&mut config, &facade).expect_err("expected SqliteUnavailable");
    assert!(matches!(err, MigrationError::SqliteUnavailable));
}
