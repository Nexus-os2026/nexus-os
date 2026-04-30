//! Bug AK Phase 1 — one-shot config-to-vault migration.
//!
//! Phase 1 commits the SocialConfig fields (consumer key/secret +
//! access token/secret) into the kernel `SecretsFacade` under
//! scope `"social"`. Idempotent via
//! `schema_versions.key = "credential_vault_v1"`.
//!
//! Refinement B (transactionality, ADR 0004 Decision section):
//!   1. Pre-encrypt each value (outside any transaction) via the
//!      facade's `SqliteEnvelopeBackend` domain key.
//!   2. Call `db.migrate_credential_vault_v1(rows, key, version)`
//!      which opens a SQLite transaction, inserts every row,
//!      verify-reads each, bumps `schema_versions`, and commits.
//!      Any failure rolls back the entire batch.
//!   3. ONLY AFTER commit: clear in-memory config fields and
//!      re-save the envelope-encrypted config via
//!      `crate::config::save_config`.
//!   4. If step 3's config-file save fails AFTER the DB commit
//!      succeeded: schema_version is bumped, consumers read from
//!      facade (which has the data), config-file fields are stale
//!      but unused. Log WARN and continue.
//!
//! Field-name mapping (Bug AK Commit 2): the on-disk
//! `SocialConfig` field names use Twitter-API-1.0 nomenclature
//! (`x_api_key`, `x_api_secret`, `x_access_token`,
//! `x_access_secret`). The vault stores them under the
//! Twitter-API-2.0 / OAuth2 nomenclature (`x_consumer_key`,
//! `x_consumer_secret`, `x_access_token`,
//! `x_access_token_secret`) so consumers downstream of Commit 2
//! see a forward-looking key naming.

use crate::config::NexusConfig;
use thiserror::Error;
use zeroize::Zeroizing;

const SCHEMA_KEY: &str = "credential_vault_v1";
const SCHEMA_VERSION: i64 = 1;

#[derive(Debug, Error)]
pub enum MigrationError {
    #[error("storage error: {0}")]
    Storage(String),
    #[error("crypto error: {0}")]
    Crypto(String),
    #[error("verify-read mismatch for {scope}.{name}")]
    VerifyMismatch { scope: String, name: String },
    #[error("sqlite backend not configured on facade")]
    SqliteUnavailable,
}

#[derive(Debug, Clone)]
pub enum MigrationReport {
    AlreadyRun,
    Migrated {
        fields_migrated: Vec<(String, String)>,
        config_resave_failed: bool,
    },
}

/// Phase 1 fields to migrate. Tuple is `(scope, vault_name, value)`.
/// Empty values are skipped. Note the on-disk -> vault name
/// renames documented in the module header.
///
/// Bug AK Commit 3: extended with the six LLM api_key fields
/// from `NexusConfig.llm`. Vault scope `"llm"`, names match the
/// provider short identifiers (anthropic / openai / deepseek /
/// gemini / nvidia / openrouter). Empty fields skip; v1
/// schema_version covers both SocialConfig and LLM clearing in
/// one idempotency gate (locked: v1-extended, no v2 bump).
fn collect_phase1_fields(config: &NexusConfig) -> Vec<(String, String, String)> {
    let s = &config.social;
    let l = &config.llm;
    let candidates: [(&str, &str, &String); 10] = [
        ("social", "x_consumer_key", &s.x_api_key),
        ("social", "x_consumer_secret", &s.x_api_secret),
        ("social", "x_access_token", &s.x_access_token),
        ("social", "x_access_token_secret", &s.x_access_secret),
        ("llm", "anthropic", &l.anthropic_api_key),
        ("llm", "openai", &l.openai_api_key),
        ("llm", "deepseek", &l.deepseek_api_key),
        ("llm", "gemini", &l.gemini_api_key),
        ("llm", "nvidia", &l.nvidia_api_key),
        ("llm", "openrouter", &l.openrouter_api_key),
    ];
    candidates
        .iter()
        .filter(|(_, _, v)| !v.trim().is_empty())
        .map(|(s, n, v)| (s.to_string(), n.to_string(), (*v).clone()))
        .collect()
}

/// Clear the migrated SocialConfig + LLM fields. Called only
/// AFTER the DB transaction has committed.
fn clear_phase1_fields(config: &mut NexusConfig) {
    config.social.x_api_key.clear();
    config.social.x_api_secret.clear();
    config.social.x_access_token.clear();
    config.social.x_access_secret.clear();
    config.llm.anthropic_api_key.clear();
    config.llm.openai_api_key.clear();
    config.llm.deepseek_api_key.clear();
    config.llm.gemini_api_key.clear();
    config.llm.nvidia_api_key.clear();
    config.llm.openrouter_api_key.clear();
}

/// Run the one-shot migration. See module header for the
/// transactionality contract (Refinement B).
///
/// Locked Commit 2 signature: `(config, &facade)`. The facade owns
/// the master-key-derived domain key (via its
/// `SqliteEnvelopeBackend`) and the underlying `NexusDatabase`,
/// so callers don't need to thread a separate db handle.
pub fn migrate_config_to_vault(
    config: &mut NexusConfig,
    facade: &super::SecretsFacade,
) -> Result<MigrationReport, MigrationError> {
    let sqlite = facade
        .sqlite_backend()
        .ok_or(MigrationError::SqliteUnavailable)?;
    let db = sqlite.db();

    if db
        .schema_version(SCHEMA_KEY)
        .map_err(|e| MigrationError::Storage(format!("{e}")))?
        .is_some()
    {
        return Ok(MigrationReport::AlreadyRun);
    }

    let fields = collect_phase1_fields(config);
    if fields.is_empty() {
        // Empty migration: still bump schema_version so subsequent
        // boots short-circuit.
        db.migrate_credential_vault_v1(&[], SCHEMA_KEY, SCHEMA_VERSION)
            .map_err(|e| MigrationError::Storage(format!("{e}")))?;
        return Ok(MigrationReport::Migrated {
            fields_migrated: Vec::new(),
            config_resave_failed: false,
        });
    }

    // Pre-encrypt outside the transaction so a crypto error
    // doesn't surface as a SQLite rollback.
    let mut prepared: Vec<(String, String, Vec<u8>, Vec<u8>)> = Vec::new();
    for (scope, name, value) in &fields {
        let z = Zeroizing::new(value.clone());
        let (nonce, ciphertext) = sqlite
            .encrypt_for_migration(z.as_bytes())
            .map_err(|e| MigrationError::Crypto(format!("{e}")))?;
        prepared.push((scope.clone(), name.clone(), nonce, ciphertext));
    }

    db.migrate_credential_vault_v1(&prepared, SCHEMA_KEY, SCHEMA_VERSION)
        .map_err(|e| match e {
            nexus_persistence::PersistenceError::Serialization(msg)
                if msg.starts_with("verify-read mismatch") =>
            {
                MigrationError::VerifyMismatch {
                    scope: "<rolled-back>".into(),
                    name: msg.replace("verify-read mismatch on ", ""),
                }
            }
            other => MigrationError::Storage(format!("{other}")),
        })?;

    let migrated: Vec<(String, String)> = prepared
        .iter()
        .map(|(s, n, _, _)| (s.clone(), n.clone()))
        .collect();

    clear_phase1_fields(config);

    // Refinement B step 4: re-save the envelope-encrypted config.
    // Failure here is the documented "one acceptable inconsistency"
    // — the facade is authoritative once schema_version is bumped.
    let resave_failed = match crate::config::save_config(config) {
        Ok(()) => false,
        Err(e) => {
            eprintln!(
                "config-file re-save failed after migration commit; \
                 on-disk SocialConfig fields are stale but unused: {e}"
            );
            true
        }
    };

    Ok(MigrationReport::Migrated {
        fields_migrated: migrated,
        config_resave_failed: resave_failed,
    })
}
