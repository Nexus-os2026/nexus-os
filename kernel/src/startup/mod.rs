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
//!   1. Load the master key from the validated key source
//!      (`EncryptionKey::from_config`) and verify that it opens every
//!      secret already stored in the vault (Final Gate item E).
//!   2. Construct a `SecretsFacade` with all four backends in
//!      canonical order (Env, OsKeyring, SqliteEnvelope, Memory).
//!   3. Call
//!      `kernel::secrets::migrate::migrate_config_to_vault`.
//!   4. On success, install the facade into
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
use crate::secrets::migrate::{
    migrate_config_to_vault, plan_legacy_upgrade, record_upgrade_audit, MigrationError,
    MigrationReport,
};
use crate::secrets::{SecretBackend, SecretError, SecretsFacade};
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StartupError {
    #[error("crypto error during master-key load: {0}")]
    Crypto(String),
    #[error("migration error: {0}")]
    Migration(#[from] MigrationError),
    /// Final Gate item E: the master key does not open a secret already in
    /// the vault, so the vault is not used with it.
    #[error("the vault key does not open the stored secrets; the vault is not used")]
    VaultKeyMismatch,
    /// Final Gate item E: a stored secret's row is damaged whatever the key:
    /// its nonce has the wrong length, or it opens to a value that is not
    /// text. Reported apart from a key that does not open the vault.
    #[error("a stored vault secret is damaged and cannot be read; the vault is not used")]
    VaultSecretDamaged,
    #[error("the stored vault secrets could not be listed or read; the vault is not used")]
    VaultUnreadable,
}

/// The vault scopes Nexus OS reads or writes through the facade: provider
/// keys (`llm`), the migrated social credentials, messaging, HTTP connector
/// and OIDC secrets.
const VAULT_SCOPES: &[&str] = &[
    "llm",
    "social",
    "messaging.whatsapp",
    "messaging.matrix",
    "http",
    "auth.oidc",
];

/// Final Gate item E: the master key must open every secret already stored
/// in the vault scopes before anything reads the vault or writes to it. A
/// changed or wrong key source is refused here rather than leaving old rows
/// unreadable and writing new rows under another key. An empty vault has
/// nothing to verify, so its first key is accepted.
fn verify_vault_key(sqlite: &SqliteEnvelopeBackend) -> Result<(), StartupError> {
    check_vault_rows(sqlite, false)
}

/// The Final Gate item E check. With `legacy_checked_by_plan`, legacy
/// (unbound) rows are skipped: the M0-FIX F4 upgrade plan has already
/// proven, read-only, that every one of them opens under this key.
fn check_vault_rows(
    sqlite: &SqliteEnvelopeBackend,
    legacy_checked_by_plan: bool,
) -> Result<(), StartupError> {
    for scope in VAULT_SCOPES {
        let names = sqlite
            .db()
            .list_secrets(scope)
            .map_err(|_| StartupError::VaultUnreadable)?;
        for name in names {
            // The decrypted value is dropped (and zeroized) at once.
            match sqlite.get(scope, &name) {
                Ok(_) => {}
                Err(SecretError::LegacyUnbound) if legacy_checked_by_plan => {}
                // The authenticated decryption failed: this key does not
                // open the row.
                Err(SecretError::DecryptionFailed) => {
                    return Err(StartupError::VaultKeyMismatch);
                }
                // The row itself is malformed (wrong nonce length, or a
                // value that is not text).
                Err(SecretError::Crypto(_)) => return Err(StartupError::VaultSecretDamaged),
                Err(_) => return Err(StartupError::VaultUnreadable),
            }
        }
    }
    Ok(())
}

/// M0-FIX F4: the one-time upgrade of legacy (unbound) vault rows to the
/// identity-bound format, keeping Final Gate item E's "refuse a wrong key
/// before writing anything":
///   1. plan the upgrade (read-only): every legacy row must open under this
///      key, else the key is refused (`VaultKeyMismatch`) or the row is
///      reported as damaged (`VaultSecretDamaged`);
///   2. check every bound row (read-only);
///   3. only then write: all re-bound rows and the completion marker in one
///      transaction, and record one audit event with the row count.
///
/// Legacy rows in a vault that is already upgraded fail startup closed
/// (`StartupError::Migration(LegacyAfterUpgrade)`): the whole vault is
/// refused, not only that secret.
fn upgrade_vault_rows(
    sqlite: &SqliteEnvelopeBackend,
    audit: &std::sync::Mutex<crate::audit::AuditTrail>,
) -> Result<(), StartupError> {
    let upgrade_error = |e| match e {
        MigrationError::LegacyUndecryptable { .. } => StartupError::VaultKeyMismatch,
        MigrationError::LegacyDamaged { .. } => StartupError::VaultSecretDamaged,
        other => StartupError::Migration(other),
    };
    let plan = plan_legacy_upgrade(sqlite).map_err(upgrade_error)?;
    check_vault_rows(sqlite, true)?;
    let report = plan.commit(sqlite).map_err(upgrade_error)?;
    record_upgrade_audit(audit, &report);
    Ok(())
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

    let sqlite = Arc::new(SqliteEnvelopeBackend::new(Arc::clone(&db), &master));
    upgrade_vault_rows(&sqlite, &audit)?;
    // Every row is bound now; check them all again.
    verify_vault_key(&sqlite)?;

    let env = Arc::new(EnvBackend::new());
    let keyring = Arc::new(KeyringBackendAdapter::os_keyring());
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
        // This test sets NEXUS_CONFIG_PATH and NEXUS_ENCRYPTION_KEY for the
        // whole process: it takes the locks the secrets and crypto tests take
        // for the same variables (always in this order), so no other test
        // sees or clears them while it runs.
        let _config_path = crate::secrets::tests::NEXUS_CONFIG_PATH_GUARD
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let _encryption_key = crate::crypto::tests::ENV_KEY_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let db = Arc::new(nexus_persistence::NexusDatabase::in_memory().expect("in-memory db"));
        let mut config = NexusConfig::default();
        // Force the master-key path to env so EncryptionKey::from_config
        // succeeds without a file dependency.
        config.security.enabled = true;
        config.security.key_source = "env".into();
        let _key_env = crate::secrets::tests::EnvVarGuard::set(
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
        let _path_env = crate::secrets::tests::EnvVarGuard::set("NEXUS_CONFIG_PATH", &cfg_path);

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

        let _ = std::fs::remove_dir_all(&tmpdir);
    }

    /// Final Gate item E: a master key opens the vault only if it opens every
    /// stored secret; an empty vault accepts its first key.
    #[test]
    fn p0_fg_e_the_vault_key_must_open_the_stored_secrets() {
        use crate::secrets::Zeroizing;
        let db = Arc::new(nexus_persistence::NexusDatabase::in_memory().expect("in-memory db"));
        let key_a = || EncryptionKey::from_raw_for_test([0x11; 32]);
        let key_b = || EncryptionKey::from_raw_for_test([0x22; 32]);
        assert!(verify_vault_key(&SqliteEnvelopeBackend::new(Arc::clone(&db), &key_b())).is_ok());
        let writer = SqliteEnvelopeBackend::new(Arc::clone(&db), &key_a());
        writer
            .set("llm", "anthropic", Zeroizing::new("synthetic-llm".into()))
            .unwrap();
        writer
            .set(
                "auth.oidc",
                "client_secret",
                Zeroizing::new("synthetic-oidc".into()),
            )
            .unwrap();
        assert!(verify_vault_key(&SqliteEnvelopeBackend::new(Arc::clone(&db), &key_a())).is_ok());
        assert!(matches!(
            verify_vault_key(&SqliteEnvelopeBackend::new(Arc::clone(&db), &key_b())),
            Err(StartupError::VaultKeyMismatch)
        ));
        // One row under another key is enough to refuse.
        let mixed = Arc::new(nexus_persistence::NexusDatabase::in_memory().expect("in-memory db"));
        SqliteEnvelopeBackend::new(Arc::clone(&mixed), &key_a())
            .set("llm", "openai", Zeroizing::new("synthetic-a".into()))
            .unwrap();
        SqliteEnvelopeBackend::new(Arc::clone(&mixed), &key_b())
            .set(
                "social",
                "x_consumer_key",
                Zeroizing::new("synthetic-b".into()),
            )
            .unwrap();
        for key in [key_a(), key_b()] {
            assert!(matches!(
                verify_vault_key(&SqliteEnvelopeBackend::new(Arc::clone(&mixed), &key)),
                Err(StartupError::VaultKeyMismatch)
            ));
        }
    }

    /// Final Gate item E (stream 6 review): a damaged row is reported apart
    /// from a key that does not open the vault: a nonce of the wrong length,
    /// or a row that the key opens to a value that is not text.
    #[test]
    fn p0_fg_e_a_damaged_vault_row_is_reported_as_damaged() {
        use crate::secrets::Zeroizing;
        let key = || EncryptionKey::from_raw_for_test([0x11; 32]);
        let good = || Zeroizing::new("synthetic-llm".to_string());

        let short_nonce =
            Arc::new(nexus_persistence::NexusDatabase::in_memory().expect("in-memory db"));
        let backend = SqliteEnvelopeBackend::new(Arc::clone(&short_nonce), &key());
        backend.set("llm", "anthropic", good()).unwrap();
        short_nonce
            .record_secret("llm", "openai", &[0_u8; 5], b"synthetic-ciphertext")
            .unwrap();
        assert!(matches!(
            verify_vault_key(&backend),
            Err(StartupError::VaultSecretDamaged)
        ));

        let not_text =
            Arc::new(nexus_persistence::NexusDatabase::in_memory().expect("in-memory db"));
        let backend = SqliteEnvelopeBackend::new(Arc::clone(&not_text), &key());
        let (nonce, ciphertext) = backend
            .encrypt_for_migration("social", "x_consumer_key", &[0xff, 0xfe, 0x00])
            .unwrap();
        not_text
            .record_secret("social", "x_consumer_key", &nonce, &ciphertext)
            .unwrap();
        assert!(matches!(
            verify_vault_key(&backend),
            Err(StartupError::VaultSecretDamaged)
        ));

        let messages = [
            StartupError::VaultKeyMismatch.to_string(),
            StartupError::VaultSecretDamaged.to_string(),
        ];
        assert_ne!(messages[0], messages[1]);
        assert!(messages[1].contains("damaged"), "{}", messages[1]);
    }

    /// M0-FIX F4: startup upgrades legacy rows only when every one opens
    /// under the configured key; otherwise it refuses the key (or reports a
    /// damaged row) and changes nothing.
    #[test]
    fn f4_startup_upgrades_legacy_vault_rows_only_under_their_key() {
        use crate::secrets::tests::f4_write_legacy_row;
        let key_a = || EncryptionKey::from_raw_for_test([0x11; 32]);
        let key_b = || EncryptionKey::from_raw_for_test([0x22; 32]);
        let db = Arc::new(nexus_persistence::NexusDatabase::in_memory().expect("in-memory db"));
        f4_write_legacy_row(&db, &key_a(), "llm", "anthropic", b"synthetic-llm");
        f4_write_legacy_row(
            &db,
            &key_a(),
            "auth.oidc",
            "client_secret",
            b"synthetic-oidc",
        );
        let before = db.list_secret_rows().unwrap();
        let audit = std::sync::Mutex::new(crate::audit::AuditTrail::new());
        let wrong = SqliteEnvelopeBackend::new(Arc::clone(&db), &key_b());
        assert!(matches!(
            upgrade_vault_rows(&wrong, &audit),
            Err(StartupError::VaultKeyMismatch)
        ));
        assert_eq!(db.list_secret_rows().unwrap(), before);
        assert_eq!(db.schema_version("secrets_aad_v2").unwrap(), None);
        assert!(audit.lock().unwrap().events().is_empty());

        let right = SqliteEnvelopeBackend::new(Arc::clone(&db), &key_a());
        upgrade_vault_rows(&right, &audit).unwrap();
        verify_vault_key(&right).unwrap();
        // One audit event with the count only: no names, no values.
        {
            let trail = audit.lock().unwrap();
            assert_eq!(trail.events().len(), 1);
            let payload = &trail.events()[0].payload;
            assert_eq!(payload["event"], "secrets_format_upgraded");
            assert_eq!(payload["rows_upgraded"], 2);
            let text = payload.to_string();
            for leaked in ["synthetic", "anthropic", "client_secret", "llm", "oidc"] {
                assert!(!text.contains(leaked), "{leaked} in {text}");
            }
        }
        // A second start records nothing and writes nothing.
        let upgraded = db.list_secret_rows().unwrap();
        upgrade_vault_rows(&right, &audit).unwrap();
        assert_eq!(audit.lock().unwrap().events().len(), 1);
        assert_eq!(db.list_secret_rows().unwrap(), upgraded);

        // Replay through startup: marker deleted, a legacy row for
        // llm.anthropic planted into auth.oidc.client_secret. Startup fails
        // closed and the planted row is neither re-bound nor opened.
        let old = Arc::new(nexus_persistence::NexusDatabase::in_memory().expect("in-memory db"));
        f4_write_legacy_row(&old, &key_a(), "llm", "anthropic", b"synthetic-llm");
        let (nonce, ciphertext) = old.lookup_secret("llm", "anthropic").unwrap().unwrap();
        db.with_transaction(|tx| {
            tx.execute_batch("DELETE FROM schema_versions WHERE key = 'secrets_aad_v2'")?;
            Ok(())
        })
        .unwrap();
        db.record_secret("auth.oidc", "client_secret", &nonce, &ciphertext)
            .unwrap();
        let planted = db.list_secret_rows().unwrap();
        assert!(matches!(
            upgrade_vault_rows(&right, &audit),
            Err(StartupError::Migration(
                MigrationError::LegacyAfterUpgrade { count: 1 }
            ))
        ));
        assert_eq!(db.list_secret_rows().unwrap(), planted);
        assert!(matches!(
            right.get("auth.oidc", "client_secret"),
            Err(SecretError::LegacyUnbound)
        ));
        assert!(matches!(
            verify_vault_key(&right),
            Err(StartupError::VaultUnreadable)
        ));
        assert_eq!(audit.lock().unwrap().events().len(), 1);
        assert_eq!(
            right.get("llm", "anthropic").unwrap().as_str(),
            "synthetic-llm"
        );
        assert!(matches!(
            verify_vault_key(&wrong),
            Err(StartupError::VaultKeyMismatch)
        ));

        let damaged =
            Arc::new(nexus_persistence::NexusDatabase::in_memory().expect("in-memory db"));
        f4_write_legacy_row(&damaged, &key_a(), "llm", "openai", &[0xff, 0xfe]);
        assert!(matches!(
            upgrade_vault_rows(
                &SqliteEnvelopeBackend::new(Arc::clone(&damaged), &key_a()),
                &audit
            ),
            Err(StartupError::VaultSecretDamaged)
        ));

        // Nothing is written before the key check: a vault whose only row
        // is malformed is refused as damaged without recording the marker.
        let malformed =
            Arc::new(nexus_persistence::NexusDatabase::in_memory().expect("in-memory db"));
        malformed
            .record_secret("llm", "openai", &[0_u8; 5], b"synthetic-ciphertext")
            .unwrap();
        assert!(matches!(
            upgrade_vault_rows(
                &SqliteEnvelopeBackend::new(Arc::clone(&malformed), &key_a()),
                &audit
            ),
            Err(StartupError::VaultSecretDamaged)
        ));
        assert_eq!(malformed.schema_version("secrets_aad_v2").unwrap(), None);
    }

    /// Final Gate item E: startup refuses a key file whose key does not open
    /// the vault before it migrates, clears or writes anything, and before a
    /// facade could be installed.
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn p0_fg_e_startup_refuses_a_vault_key_that_does_not_open_the_vault() {
        use crate::secrets::Zeroizing;
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let db = Arc::new(nexus_persistence::NexusDatabase::in_memory().expect("in-memory db"));
        SqliteEnvelopeBackend::new(
            Arc::clone(&db),
            &EncryptionKey::from_raw_for_test([0x11; 32]),
        )
        .set("llm", "anthropic", Zeroizing::new("synthetic-llm".into()))
        .unwrap();
        let rows_before = db.list_secret_rows().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let key_path = dir.path().join("vault.key");
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&key_path)
            .unwrap()
            .write_all(&[0x22; 32])
            .unwrap();
        let mut config = NexusConfig::default();
        config.security.enabled = true;
        config.security.key_source = "file".into();
        config.security.key_file = Some(key_path.to_str().unwrap().into());
        config.social.x_api_key = "synthetic-social".into();
        let audit = Arc::new(std::sync::Mutex::new(crate::audit::AuditTrail::new()));
        match run_migrations(&mut config, Arc::clone(&db), audit) {
            Err(StartupError::VaultKeyMismatch) => {}
            other => panic!("{other:?}"),
        }
        assert_eq!(db.schema_version("credential_vault_v1").unwrap(), None);
        // M0-FIX F4 review: nor the secrets-upgrade marker, nor any row.
        assert_eq!(db.schema_version("secrets_aad_v2").unwrap(), None);
        assert_eq!(db.list_secret_rows().unwrap(), rows_before);
        assert_eq!(config.social.x_api_key, "synthetic-social");
        assert!(db.list_secrets("social").unwrap().is_empty());
        assert_eq!(
            db.list_secrets("llm").unwrap(),
            vec!["anthropic".to_string()]
        );
    }
}
