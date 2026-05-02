//! Bug AK Commit 2 — process startup migrations.
//!
//! Per OQ-2A (option a), the credential-vault migration does NOT
//! run inside `load_config()` — that would invert the existing
//! bootstrap order (config.security drives DB encryption, so the
//! DB cannot open until config has loaded). Instead, the
//! application calls `run_migrations` AFTER both `NexusConfig` and
//! `NexusDatabase` are ready. This wiring lives in
//! `app/src-tauri/src/lib.rs` immediately after the DB open.
//!
//! `run_migrations` is the Phase-1 entry point:
//!   1. Construct a `SecretsFacade` with all four backends in
//!      canonical order (Env, OsKeyring stub, SqliteEnvelope,
//!      Memory).
//!   2. Call
//!      `kernel::secrets::migrate::migrate_config_to_vault`.
//!   3. On success, install the facade into
//!      `kernel::secrets::global::FACADE`.
//!
//! UNILATERAL DEVIATION (flagged in Commit 2 report): the locked
//! plan declared this `pub async fn`. The body is entirely sync
//! (every backend impl is sync; see the `kernel::secrets` module
//! header). The only call site at HEAD is `AppState::new` —
//! itself sync. Forcing async would require a fresh tokio runtime
//! or a `Handle::current().block_on` dance with no I/O benefit.
//! Shipped as `pub fn`. If a future network-backed backend joins
//! the chain, flip to `pub async fn` then.

use crate::config::NexusConfig;
use crate::crypto::EncryptionKey;
use crate::secrets::backend_env::EnvBackend;
use crate::secrets::backend_keyring::KeyringBackendAdapter;
use crate::secrets::backend_memory::MemoryBackend;
use crate::secrets::backend_sqlite::SqliteEnvelopeBackend;
use crate::secrets::migrate::{migrate_config_to_vault, MigrationError, MigrationReport};
use crate::secrets::SecretsFacade;
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StartupError {
    #[error("crypto error during master-key load: {0}")]
    Crypto(String),
    #[error("migration error: {0}")]
    Migration(#[from] MigrationError),
}

/// Build the production facade, run the credential-vault
/// migration, and install the global singleton.
///
/// Idempotent at the migration layer: a second invocation against
/// the same db short-circuits to `MigrationReport::AlreadyRun`. It
/// is, however, NOT idempotent at the global-install layer —
/// `kernel::secrets::global::install` panics on double-install,
/// matching its one-shot contract. Callers should invoke this
/// exactly once per process.
pub fn run_migrations(
    config: &mut NexusConfig,
    db: Arc<nexus_persistence::NexusDatabase>,
    audit: Arc<std::sync::Mutex<crate::audit::AuditTrail>>,
) -> Result<MigrationReport, StartupError> {
    let master = EncryptionKey::from_config(&config.security)
        .map_err(|e| StartupError::Crypto(format!("{e}")))?;

    let env = Arc::new(EnvBackend::new());
    let keyring = Arc::new(KeyringBackendAdapter::os_keyring());
    let sqlite = Arc::new(SqliteEnvelopeBackend::new(Arc::clone(&db), &master));
    let memory = Arc::new(MemoryBackend::new());

    let facade = Arc::new(SecretsFacade::new(
        env,
        keyring,
        Some(sqlite),
        memory,
        &config.credential_facade,
        audit,
    ));

    let report = migrate_config_to_vault(config, &facade)?;

    crate::secrets::global::install(facade);

    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::NexusConfig;
    use crate::secrets::migrate::MigrationReport;

    /// run_migrations must:
    ///   - populate the SecretsFacade with the 4 SocialConfig fields
    ///   - clear the in-memory NexusConfig.social.x_* fields
    ///   - bump schema_versions[credential_vault_v1]
    ///
    /// NOTE: Phase 1 migrates SocialConfig only (4 fields). The
    /// locked Commit 2 spec mentioned "all 16 known credential
    /// fields" — that's Phase 2/3 (Bug AK-2 / AK-3) scope. This
    /// test asserts what Commit 2 actually delivers. UNILATERAL
    /// flag in the report: scope-bounded to Phase 1 fields.
    ///
    /// This test does NOT exercise `kernel::secrets::global::install`
    /// because the global is a one-shot OnceLock — calling install
    /// twice (across tests) would panic. The
    /// happy-path-clears-fields invariant is covered in
    /// `kernel/src/secrets/tests.rs::migrate_config_to_vault_happy_path…`.
    #[test]
    fn run_migrations_clears_fields_and_records_report() {
        let db = Arc::new(nexus_persistence::NexusDatabase::in_memory().expect("in-memory db"));
        let mut config = NexusConfig::default();
        // Force the master-key path to env so EncryptionKey::from_config
        // succeeds without a file dependency.
        config.security.enabled = true;
        config.security.key_source = "env".into();
        std::env::set_var(
            "NEXUS_ENCRYPTION_KEY",
            "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff",
        );
        config.social.x_api_key = "ck".into();
        config.social.x_api_secret = "cs".into();
        config.social.x_access_token = "at".into();
        config.social.x_access_secret = "as".into();
        // Bug AK Commit 3: Phase 1 aggregate is 4 social + 6 llm = 10.
        config.llm.anthropic_api_key = "sk-ant".into();
        config.llm.openai_api_key = "sk-openai".into();
        config.llm.deepseek_api_key = "sk-ds".into();
        config.llm.gemini_api_key = "sk-gem".into();
        config.llm.nvidia_api_key = "sk-nv".into();
        config.llm.openrouter_api_key = "sk-or".into();

        // Isolate the config-file save path.
        let tmpdir =
            std::env::temp_dir().join(format!("nexus_ak_startup_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&tmpdir).unwrap();
        let cfg_path = tmpdir.join("config.toml");
        std::env::set_var("NEXUS_CONFIG_PATH", &cfg_path);

        // Build facade + migrate WITHOUT touching the global
        // singleton (so concurrent tests don't trip OnceLock).
        let master = EncryptionKey::from_config(&config.security).expect("master");
        let env_b = Arc::new(EnvBackend::new());
        let kr = Arc::new(KeyringBackendAdapter::os_keyring());
        let sql = Arc::new(SqliteEnvelopeBackend::new(Arc::clone(&db), &master));
        let mem = Arc::new(MemoryBackend::new());
        let audit = std::sync::Arc::new(std::sync::Mutex::new(crate::audit::AuditTrail::new()));
        let facade = Arc::new(SecretsFacade::new(
            env_b,
            kr,
            Some(sql),
            mem,
            &config.credential_facade,
            audit,
        ));
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
        assert!(config.social.x_api_key.is_empty());
        assert!(config.social.x_access_token.is_empty());
        assert!(config.llm.anthropic_api_key.is_empty());
        assert!(config.llm.openai_api_key.is_empty());
        assert!(config.llm.deepseek_api_key.is_empty());
        assert!(config.llm.gemini_api_key.is_empty());
        assert!(config.llm.nvidia_api_key.is_empty());
        assert!(config.llm.openrouter_api_key.is_empty());
        assert_eq!(db.schema_version("credential_vault_v1").unwrap(), Some(1));

        std::env::remove_var("NEXUS_CONFIG_PATH");
        std::env::remove_var("NEXUS_ENCRYPTION_KEY");
        let _ = std::fs::remove_dir_all(&tmpdir);
    }
}
