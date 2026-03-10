//! SQLite-backed marketplace registry.
//!
//! Replaces the in-memory `HashMap` registry with persistent storage.
//! Database file defaults to `~/.nexus/marketplace.db` and migrates on first run.

use crate::package::{
    sign_package, verify_attestation, verify_package, MarketplaceError, SignedPackageBundle,
    UnsignedPackageBundle,
};
use crate::registry::PackageSummary;
use ed25519_dalek::SigningKey;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Agent detail record returned by `get_agent`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentDetail {
    pub package_id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub tags: Vec<String>,
    pub capabilities: Vec<String>,
    pub price_cents: i64,
    pub downloads: i64,
    pub rating: f64,
    pub review_count: i64,
    pub created_at: String,
    pub updated_at: String,
}

/// A review row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewRecord {
    pub id: i64,
    pub agent_id: String,
    pub reviewer: String,
    pub stars: u8,
    pub comment: String,
    pub created_at: String,
}

/// A version history row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionRecord {
    pub id: i64,
    pub agent_id: String,
    pub version: String,
    pub wasm_hash: String,
    pub signature_hex: String,
    pub changelog: String,
    pub created_at: String,
}

/// SQLite-backed marketplace registry with full-text search.
pub struct SqliteRegistry {
    conn: Connection,
}

impl std::fmt::Debug for SqliteRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SqliteRegistry")
            .field("path", &self.conn.path())
            .finish()
    }
}

impl SqliteRegistry {
    /// Open (or create) a registry at the given path.  Runs migrations automatically.
    pub fn open(db_path: &Path) -> Result<Self, SqliteRegistryError> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                SqliteRegistryError::Io(format!(
                    "Cannot create directory {}: {e}",
                    parent.display()
                ))
            })?;
        }
        let conn =
            Connection::open(db_path).map_err(|e| SqliteRegistryError::Database(e.to_string()))?;
        let registry = Self { conn };
        registry.migrate()?;
        Ok(registry)
    }

    /// Open an in-memory registry (useful for tests).
    pub fn open_in_memory() -> Result<Self, SqliteRegistryError> {
        let conn = Connection::open_in_memory()
            .map_err(|e| SqliteRegistryError::Database(e.to_string()))?;
        let registry = Self { conn };
        registry.migrate()?;
        Ok(registry)
    }

    /// Default database path: `~/.nexus/marketplace.db`.
    pub fn default_db_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        PathBuf::from(home).join(".nexus").join("marketplace.db")
    }

    /// Run schema migrations.  Idempotent — safe to call on every open.
    fn migrate(&self) -> Result<(), SqliteRegistryError> {
        self.conn
            .execute_batch(
                "
            CREATE TABLE IF NOT EXISTS agents (
                id          TEXT PRIMARY KEY,
                name        TEXT NOT NULL,
                version     TEXT NOT NULL,
                description TEXT NOT NULL DEFAULT '',
                author      TEXT NOT NULL DEFAULT '',
                tags        TEXT NOT NULL DEFAULT '[]',
                capabilities TEXT NOT NULL DEFAULT '[]',
                price_cents INTEGER NOT NULL DEFAULT 0,
                downloads   INTEGER NOT NULL DEFAULT 0,
                rating      REAL NOT NULL DEFAULT 0.0,
                created_at  TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at  TEXT NOT NULL DEFAULT (datetime('now')),
                bundle_json TEXT NOT NULL DEFAULT '{}'
            );

            CREATE TABLE IF NOT EXISTS reviews (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                agent_id    TEXT NOT NULL REFERENCES agents(id),
                reviewer    TEXT NOT NULL,
                stars       INTEGER NOT NULL CHECK(stars >= 1 AND stars <= 5),
                comment     TEXT NOT NULL DEFAULT '',
                created_at  TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS versions (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                agent_id    TEXT NOT NULL REFERENCES agents(id),
                version     TEXT NOT NULL,
                wasm_hash   TEXT NOT NULL DEFAULT '',
                signature   TEXT NOT NULL DEFAULT '',
                changelog   TEXT NOT NULL DEFAULT '',
                created_at  TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE INDEX IF NOT EXISTS idx_reviews_agent ON reviews(agent_id);
            CREATE INDEX IF NOT EXISTS idx_versions_agent ON versions(agent_id);
            ",
            )
            .map_err(|e| SqliteRegistryError::Database(e.to_string()))?;
        Ok(())
    }

    /// Publish a signed agent bundle to the registry.
    pub fn publish(
        &self,
        package: UnsignedPackageBundle,
        author_key: &SigningKey,
    ) -> Result<String, SqliteRegistryError> {
        let signed = sign_package(package, author_key)?;
        verify_attestation(&signed)?;
        self.insert_bundle(&signed)?;
        Ok(signed.package_id)
    }

    /// Insert a pre-signed bundle directly.
    pub fn upsert_signed(&self, bundle: &SignedPackageBundle) -> Result<(), SqliteRegistryError> {
        self.insert_bundle(bundle)
    }

    fn insert_bundle(&self, bundle: &SignedPackageBundle) -> Result<(), SqliteRegistryError> {
        let tags_json = serde_json::to_string(&bundle.metadata.tags)
            .map_err(|e| SqliteRegistryError::Serialization(e.to_string()))?;
        let caps_json = serde_json::to_string(&bundle.metadata.capabilities)
            .map_err(|e| SqliteRegistryError::Serialization(e.to_string()))?;
        let bundle_json = serde_json::to_string(bundle)
            .map_err(|e| SqliteRegistryError::Serialization(e.to_string()))?;
        let sig_hex = hex::encode(&bundle.signature);

        self.conn
            .execute(
                "INSERT OR REPLACE INTO agents (id, name, version, description, author, tags, capabilities, bundle_json, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, datetime('now'))",
                params![
                    bundle.package_id,
                    bundle.metadata.name,
                    bundle.metadata.version,
                    bundle.metadata.description,
                    bundle.metadata.author_id,
                    tags_json,
                    caps_json,
                    bundle_json,
                ],
            )
            .map_err(|e| SqliteRegistryError::Database(e.to_string()))?;

        // Record version history
        self.conn
            .execute(
                "INSERT INTO versions (agent_id, version, wasm_hash, signature, changelog)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    bundle.package_id,
                    bundle.metadata.version,
                    bundle.attestation.materials_sha256,
                    sig_hex,
                    "",
                ],
            )
            .map_err(|e| SqliteRegistryError::Database(e.to_string()))?;

        Ok(())
    }

    /// Full-text search across name, description, tags, and author.
    pub fn search(&self, query: &str) -> Result<Vec<PackageSummary>, SqliteRegistryError> {
        let pattern = format!("%{}%", query.trim().to_lowercase());
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, name, description, author, tags FROM agents
                 WHERE LOWER(name) LIKE ?1
                    OR LOWER(description) LIKE ?1
                    OR LOWER(author) LIKE ?1
                    OR LOWER(tags) LIKE ?1
                 ORDER BY name",
            )
            .map_err(|e| SqliteRegistryError::Database(e.to_string()))?;

        let rows = stmt
            .query_map(params![pattern], |row| {
                let tags_json: String = row.get(4)?;
                let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
                Ok(PackageSummary {
                    package_id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    author_id: row.get(3)?,
                    tags,
                })
            })
            .map_err(|e| SqliteRegistryError::Database(e.to_string()))?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row.map_err(|e| SqliteRegistryError::Database(e.to_string()))?);
        }
        Ok(results)
    }

    /// Install an agent by ID.  Increments the download counter and returns the bundle.
    pub fn install(&self, package_id: &str) -> Result<SignedPackageBundle, SqliteRegistryError> {
        let bundle = self.get_bundle(package_id)?;
        verify_package(&bundle)?;

        self.conn
            .execute(
                "UPDATE agents SET downloads = downloads + 1 WHERE id = ?1",
                params![package_id],
            )
            .map_err(|e| SqliteRegistryError::Database(e.to_string()))?;

        Ok(bundle)
    }

    /// Update an existing agent with a new version bundle.
    pub fn update(
        &self,
        package_id: &str,
        new_bundle: UnsignedPackageBundle,
        author_key: &SigningKey,
        changelog: &str,
    ) -> Result<String, SqliteRegistryError> {
        // Verify the agent already exists
        let existing = self.get_agent(package_id)?;

        let signed = sign_package(new_bundle, author_key)?;
        verify_attestation(&signed)?;

        // Verify version is different
        if signed.metadata.version == existing.version {
            return Err(SqliteRegistryError::VersionConflict(format!(
                "Version {} already exists. Bump the version number.",
                existing.version
            )));
        }

        // Update the agent row with new bundle
        let tags_json = serde_json::to_string(&signed.metadata.tags)
            .map_err(|e| SqliteRegistryError::Serialization(e.to_string()))?;
        let caps_json = serde_json::to_string(&signed.metadata.capabilities)
            .map_err(|e| SqliteRegistryError::Serialization(e.to_string()))?;
        let bundle_json = serde_json::to_string(&signed)
            .map_err(|e| SqliteRegistryError::Serialization(e.to_string()))?;
        let sig_hex = hex::encode(&signed.signature);

        self.conn
            .execute(
                "UPDATE agents SET version = ?1, description = ?2, tags = ?3,
                        capabilities = ?4, bundle_json = ?5, updated_at = datetime('now')
                 WHERE id = ?6",
                params![
                    signed.metadata.version,
                    signed.metadata.description,
                    tags_json,
                    caps_json,
                    bundle_json,
                    package_id,
                ],
            )
            .map_err(|e| SqliteRegistryError::Database(e.to_string()))?;

        // Record version history
        self.conn
            .execute(
                "INSERT INTO versions (agent_id, version, wasm_hash, signature, changelog)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    package_id,
                    signed.metadata.version,
                    signed.attestation.materials_sha256,
                    sig_hex,
                    changelog,
                ],
            )
            .map_err(|e| SqliteRegistryError::Database(e.to_string()))?;

        Ok(package_id.to_string())
    }

    /// Add a review/rating for an agent.
    pub fn rate(
        &self,
        agent_id: &str,
        reviewer: &str,
        stars: u8,
        comment: &str,
    ) -> Result<f64, SqliteRegistryError> {
        let stars = stars.clamp(1, 5);

        // Verify agent exists
        let _ = self.get_agent(agent_id)?;

        self.conn
            .execute(
                "INSERT INTO reviews (agent_id, reviewer, stars, comment) VALUES (?1, ?2, ?3, ?4)",
                params![agent_id, reviewer, stars, comment],
            )
            .map_err(|e| SqliteRegistryError::Database(e.to_string()))?;

        // Recalculate average rating
        let avg: f64 = self
            .conn
            .query_row(
                "SELECT AVG(CAST(stars AS REAL)) FROM reviews WHERE agent_id = ?1",
                params![agent_id],
                |row| row.get(0),
            )
            .map_err(|e| SqliteRegistryError::Database(e.to_string()))?;

        self.conn
            .execute(
                "UPDATE agents SET rating = ?1 WHERE id = ?2",
                params![avg, agent_id],
            )
            .map_err(|e| SqliteRegistryError::Database(e.to_string()))?;

        Ok(avg)
    }

    /// Get full agent detail by ID.
    pub fn get_agent(&self, agent_id: &str) -> Result<AgentDetail, SqliteRegistryError> {
        let review_count: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM reviews WHERE agent_id = ?1",
                params![agent_id],
                |row| row.get(0),
            )
            .unwrap_or(0);

        self.conn
            .query_row(
                "SELECT id, name, version, description, author, tags, capabilities,
                        price_cents, downloads, rating, created_at, updated_at
                 FROM agents WHERE id = ?1",
                params![agent_id],
                |row| {
                    let tags_json: String = row.get(5)?;
                    let caps_json: String = row.get(6)?;
                    Ok(AgentDetail {
                        package_id: row.get(0)?,
                        name: row.get(1)?,
                        version: row.get(2)?,
                        description: row.get(3)?,
                        author: row.get(4)?,
                        tags: serde_json::from_str(&tags_json).unwrap_or_default(),
                        capabilities: serde_json::from_str(&caps_json).unwrap_or_default(),
                        price_cents: row.get(7)?,
                        downloads: row.get(8)?,
                        rating: row.get(9)?,
                        review_count,
                        created_at: row.get(10)?,
                        updated_at: row.get(11)?,
                    })
                },
            )
            .map_err(|_| SqliteRegistryError::NotFound(agent_id.to_string()))
    }

    /// Get the raw signed bundle JSON from the database.
    fn get_bundle(&self, package_id: &str) -> Result<SignedPackageBundle, SqliteRegistryError> {
        let bundle_json: String = self
            .conn
            .query_row(
                "SELECT bundle_json FROM agents WHERE id = ?1",
                params![package_id],
                |row| row.get(0),
            )
            .map_err(|_| SqliteRegistryError::NotFound(package_id.to_string()))?;

        serde_json::from_str(&bundle_json)
            .map_err(|e| SqliteRegistryError::Serialization(e.to_string()))
    }

    /// Remove an agent from the registry.
    pub fn remove(&self, package_id: &str) -> Result<bool, SqliteRegistryError> {
        // Delete reviews and versions first (foreign key deps)
        self.conn
            .execute(
                "DELETE FROM reviews WHERE agent_id = ?1",
                params![package_id],
            )
            .map_err(|e| SqliteRegistryError::Database(e.to_string()))?;
        self.conn
            .execute(
                "DELETE FROM versions WHERE agent_id = ?1",
                params![package_id],
            )
            .map_err(|e| SqliteRegistryError::Database(e.to_string()))?;

        let deleted = self
            .conn
            .execute("DELETE FROM agents WHERE id = ?1", params![package_id])
            .map_err(|e| SqliteRegistryError::Database(e.to_string()))?;

        Ok(deleted > 0)
    }

    /// Get version history for an agent.
    pub fn version_history(
        &self,
        agent_id: &str,
    ) -> Result<Vec<VersionRecord>, SqliteRegistryError> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, agent_id, version, wasm_hash, signature, changelog, created_at
                 FROM versions WHERE agent_id = ?1 ORDER BY id",
            )
            .map_err(|e| SqliteRegistryError::Database(e.to_string()))?;

        let rows = stmt
            .query_map(params![agent_id], |row| {
                Ok(VersionRecord {
                    id: row.get(0)?,
                    agent_id: row.get(1)?,
                    version: row.get(2)?,
                    wasm_hash: row.get(3)?,
                    signature_hex: row.get(4)?,
                    changelog: row.get(5)?,
                    created_at: row.get(6)?,
                })
            })
            .map_err(|e| SqliteRegistryError::Database(e.to_string()))?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row.map_err(|e| SqliteRegistryError::Database(e.to_string()))?);
        }
        Ok(results)
    }

    /// Get reviews for an agent.
    pub fn get_reviews(&self, agent_id: &str) -> Result<Vec<ReviewRecord>, SqliteRegistryError> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, agent_id, reviewer, stars, comment, created_at
                 FROM reviews WHERE agent_id = ?1 ORDER BY id",
            )
            .map_err(|e| SqliteRegistryError::Database(e.to_string()))?;

        let rows = stmt
            .query_map(params![agent_id], |row| {
                Ok(ReviewRecord {
                    id: row.get(0)?,
                    agent_id: row.get(1)?,
                    reviewer: row.get(2)?,
                    stars: row.get::<_, u8>(3)?,
                    comment: row.get(4)?,
                    created_at: row.get(5)?,
                })
            })
            .map_err(|e| SqliteRegistryError::Database(e.to_string()))?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row.map_err(|e| SqliteRegistryError::Database(e.to_string()))?);
        }
        Ok(results)
    }

    /// Get download count for an agent.
    pub fn download_count(&self, agent_id: &str) -> Result<i64, SqliteRegistryError> {
        self.conn
            .query_row(
                "SELECT downloads FROM agents WHERE id = ?1",
                params![agent_id],
                |row| row.get(0),
            )
            .map_err(|_| SqliteRegistryError::NotFound(agent_id.to_string()))
    }
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SqliteRegistryError {
    Database(String),
    Serialization(String),
    NotFound(String),
    VersionConflict(String),
    Io(String),
    Marketplace(MarketplaceError),
}

impl std::fmt::Display for SqliteRegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SqliteRegistryError::Database(msg) => write!(f, "database error: {msg}"),
            SqliteRegistryError::Serialization(msg) => write!(f, "serialization error: {msg}"),
            SqliteRegistryError::NotFound(id) => write!(f, "not found: {id}"),
            SqliteRegistryError::VersionConflict(msg) => write!(f, "version conflict: {msg}"),
            SqliteRegistryError::Io(msg) => write!(f, "io error: {msg}"),
            SqliteRegistryError::Marketplace(e) => write!(f, "marketplace: {e}"),
        }
    }
}

impl std::error::Error for SqliteRegistryError {}

impl From<MarketplaceError> for SqliteRegistryError {
    fn from(e: MarketplaceError) -> Self {
        SqliteRegistryError::Marketplace(e)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::package::{create_unsigned_bundle, PackageMetadata};

    fn test_key() -> SigningKey {
        SigningKey::from_bytes(&[7u8; 32])
    }

    fn make_metadata(name: &str, version: &str) -> PackageMetadata {
        PackageMetadata {
            name: name.to_string(),
            version: version.to_string(),
            description: format!("A {name} agent"),
            capabilities: vec!["llm.query".to_string()],
            tags: vec!["test".to_string(), "ai".to_string()],
            author_id: "dev-alice".to_string(),
        }
    }

    fn make_manifest(name: &str, version: &str) -> String {
        format!(
            "name = \"{name}\"\nversion = \"{version}\"\ncapabilities = [\"llm.query\"]\nfuel_budget = 10000\n"
        )
    }

    fn make_unsigned(name: &str, version: &str) -> UnsignedPackageBundle {
        create_unsigned_bundle(
            &make_manifest(name, version),
            "fn run() {}",
            make_metadata(name, version),
            &format!("local://{name}"),
            "nexus-test",
        )
        .unwrap()
    }

    fn publish_agent(reg: &SqliteRegistry, name: &str, version: &str) -> String {
        let unsigned = make_unsigned(name, version);
        reg.publish(unsigned, &test_key()).unwrap()
    }

    // -----------------------------------------------------------------------
    // Core tests
    // -----------------------------------------------------------------------

    #[test]
    fn open_in_memory_succeeds() {
        let reg = SqliteRegistry::open_in_memory();
        assert!(reg.is_ok());
    }

    #[test]
    fn publish_then_search_finds_agent() {
        let reg = SqliteRegistry::open_in_memory().unwrap();
        let pkg_id = publish_agent(&reg, "data-cruncher", "1.0.0");

        let results = reg.search("cruncher").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].package_id, pkg_id);
        assert_eq!(results[0].name, "data-cruncher");
    }

    #[test]
    fn search_empty_query_returns_all() {
        let reg = SqliteRegistry::open_in_memory().unwrap();
        publish_agent(&reg, "agent-aaa", "1.0.0");
        publish_agent(&reg, "agent-bbb", "1.0.0");

        let results = reg.search("").unwrap();
        // Empty LIKE '%' matches everything
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn search_by_tag() {
        let reg = SqliteRegistry::open_in_memory().unwrap();
        publish_agent(&reg, "tag-agent", "1.0.0");

        let results = reg.search("ai").unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn search_by_author() {
        let reg = SqliteRegistry::open_in_memory().unwrap();
        publish_agent(&reg, "author-agent", "1.0.0");

        let results = reg.search("alice").unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn search_no_match_returns_empty() {
        let reg = SqliteRegistry::open_in_memory().unwrap();
        publish_agent(&reg, "my-agent", "1.0.0");

        let results = reg.search("zzz-nonexistent").unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn install_increments_downloads() {
        let reg = SqliteRegistry::open_in_memory().unwrap();
        let pkg_id = publish_agent(&reg, "dl-agent", "1.0.0");

        assert_eq!(reg.download_count(&pkg_id).unwrap(), 0);

        reg.install(&pkg_id).unwrap();
        assert_eq!(reg.download_count(&pkg_id).unwrap(), 1);

        reg.install(&pkg_id).unwrap();
        assert_eq!(reg.download_count(&pkg_id).unwrap(), 2);
    }

    #[test]
    fn install_verifies_signature() {
        let reg = SqliteRegistry::open_in_memory().unwrap();
        let pkg_id = publish_agent(&reg, "sig-agent", "1.0.0");

        let bundle = reg.install(&pkg_id).unwrap();
        assert!(verify_package(&bundle).is_ok());
    }

    #[test]
    fn rating_updates_average() {
        let reg = SqliteRegistry::open_in_memory().unwrap();
        let pkg_id = publish_agent(&reg, "rated-agent", "1.0.0");

        let avg = reg.rate(&pkg_id, "user-1", 5, "great").unwrap();
        assert!((avg - 5.0).abs() < f64::EPSILON);

        let avg = reg.rate(&pkg_id, "user-2", 3, "okay").unwrap();
        assert!((avg - 4.0).abs() < f64::EPSILON);

        let detail = reg.get_agent(&pkg_id).unwrap();
        assert_eq!(detail.review_count, 2);
        assert!((detail.rating - 4.0).abs() < f64::EPSILON);
    }

    #[test]
    fn rating_clamps_to_1_5() {
        let reg = SqliteRegistry::open_in_memory().unwrap();
        let pkg_id = publish_agent(&reg, "clamp-agent", "1.0.0");

        reg.rate(&pkg_id, "user-1", 0, "bad").unwrap(); // clamped to 1
        reg.rate(&pkg_id, "user-2", 10, "amazing").unwrap(); // clamped to 5

        let avg = reg.get_agent(&pkg_id).unwrap().rating;
        assert!((avg - 3.0).abs() < f64::EPSILON); // (1+5)/2 = 3
    }

    #[test]
    fn version_update_preserves_history() {
        let reg = SqliteRegistry::open_in_memory().unwrap();
        let pkg_id = publish_agent(&reg, "versioned-agent", "1.0.0");

        // Update to 2.0.0
        let unsigned_v2 = make_unsigned("versioned-agent", "2.0.0");
        reg.update(&pkg_id, unsigned_v2, &test_key(), "Added new feature")
            .unwrap();

        // Current version should be 2.0.0
        let detail = reg.get_agent(&pkg_id).unwrap();
        assert_eq!(detail.version, "2.0.0");

        // Version history should have both entries
        let history = reg.version_history(&pkg_id).unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].version, "1.0.0");
        assert_eq!(history[1].version, "2.0.0");
        assert_eq!(history[1].changelog, "Added new feature");
    }

    #[test]
    fn version_update_rejects_same_version() {
        let reg = SqliteRegistry::open_in_memory().unwrap();
        let pkg_id = publish_agent(&reg, "dup-version", "1.0.0");

        let unsigned_dup = make_unsigned("dup-version", "1.0.0");
        let result = reg.update(&pkg_id, unsigned_dup, &test_key(), "");
        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(SqliteRegistryError::VersionConflict(_))
        ));
    }

    #[test]
    fn get_agent_returns_full_detail() {
        let reg = SqliteRegistry::open_in_memory().unwrap();
        let pkg_id = publish_agent(&reg, "detail-agent", "1.0.0");

        let detail = reg.get_agent(&pkg_id).unwrap();
        assert_eq!(detail.name, "detail-agent");
        assert_eq!(detail.version, "1.0.0");
        assert_eq!(detail.author, "dev-alice");
        assert!(detail.capabilities.contains(&"llm.query".to_string()));
        assert!(detail.tags.contains(&"test".to_string()));
        assert_eq!(detail.downloads, 0);
        assert_eq!(detail.price_cents, 0);
    }

    #[test]
    fn get_agent_not_found() {
        let reg = SqliteRegistry::open_in_memory().unwrap();
        let result = reg.get_agent("pkg-nonexistent");
        assert!(matches!(result, Err(SqliteRegistryError::NotFound(_))));
    }

    #[test]
    fn remove_deletes_agent_and_deps() {
        let reg = SqliteRegistry::open_in_memory().unwrap();
        let pkg_id = publish_agent(&reg, "removable", "1.0.0");
        reg.rate(&pkg_id, "user-1", 5, "great").unwrap();

        assert!(reg.remove(&pkg_id).unwrap());
        assert!(reg.get_agent(&pkg_id).is_err());
        assert!(reg.get_reviews(&pkg_id).unwrap().is_empty());
        assert!(reg.version_history(&pkg_id).unwrap().is_empty());
    }

    #[test]
    fn remove_nonexistent_returns_false() {
        let reg = SqliteRegistry::open_in_memory().unwrap();
        assert!(!reg.remove("pkg-ghost").unwrap());
    }

    #[test]
    fn full_text_search_works_across_fields() {
        let reg = SqliteRegistry::open_in_memory().unwrap();
        publish_agent(&reg, "code-analyzer", "1.0.0");
        publish_agent(&reg, "web-scraper", "1.0.0");

        // Search by name
        assert_eq!(reg.search("analyzer").unwrap().len(), 1);
        // Search by description (contains "A code-analyzer agent")
        assert_eq!(reg.search("code-analyzer agent").unwrap().len(), 1);
        // Search partial
        assert_eq!(reg.search("scrap").unwrap().len(), 1);
    }

    #[test]
    fn reviews_are_persisted() {
        let reg = SqliteRegistry::open_in_memory().unwrap();
        let pkg_id = publish_agent(&reg, "review-agent", "1.0.0");

        reg.rate(&pkg_id, "alice", 5, "excellent").unwrap();
        reg.rate(&pkg_id, "bob", 4, "good work").unwrap();

        let reviews = reg.get_reviews(&pkg_id).unwrap();
        assert_eq!(reviews.len(), 2);
        assert_eq!(reviews[0].reviewer, "alice");
        assert_eq!(reviews[0].stars, 5);
        assert_eq!(reviews[1].reviewer, "bob");
        assert_eq!(reviews[1].comment, "good work");
    }

    #[test]
    fn open_file_and_reopen_persists() {
        let tmp = std::env::temp_dir().join("nexus-sqlite-persist-test.db");
        let _ = std::fs::remove_file(&tmp);

        // Write
        {
            let reg = SqliteRegistry::open(&tmp).unwrap();
            publish_agent(&reg, "persist-agent", "1.0.0");
        }

        // Re-read
        {
            let reg = SqliteRegistry::open(&tmp).unwrap();
            let results = reg.search("persist").unwrap();
            assert_eq!(results.len(), 1);
            assert_eq!(results[0].name, "persist-agent");
        }

        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn install_not_found() {
        let reg = SqliteRegistry::open_in_memory().unwrap();
        let result = reg.install("pkg-ghost");
        assert!(matches!(result, Err(SqliteRegistryError::NotFound(_))));
    }
}
