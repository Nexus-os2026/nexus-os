//! SQLite persistence layer for memory entries.
//!
//! All memory entries are stored in a single SQLite database with JSON columns
//! for complex fields.  Soft-delete preserves entries for audit compliance.

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::types::{
    EpistemicClass, MemoryContent, MemoryEntry, MemoryError, MemoryId, MemoryScope, MemoryType,
    SensitivityClass, ValidationState,
};

/// SQLite persistence for memory entries.
pub struct MemoryPersistence {
    db: Connection,
}

impl MemoryPersistence {
    /// Opens (or creates) the persistence database at `db_path`.
    pub fn new(db_path: &str) -> Result<Self, MemoryError> {
        let conn = Connection::open(db_path)
            .map_err(|e| MemoryError::PersistenceError(format!("db open: {e}")))?;

        conn.execute_batch(Self::SCHEMA)
            .map_err(|e| MemoryError::PersistenceError(format!("schema init: {e}")))?;

        Ok(Self { db: conn })
    }

    /// Creates an in-memory persistence store (for testing).
    pub fn in_memory() -> Result<Self, MemoryError> {
        Self::new(":memory:")
    }

    const SCHEMA: &str = "
        CREATE TABLE IF NOT EXISTS memory_entries (
            id TEXT PRIMARY KEY,
            schema_version INTEGER NOT NULL DEFAULT 1,
            agent_id TEXT NOT NULL,
            memory_type TEXT NOT NULL,
            epistemic_class_json TEXT NOT NULL,
            validation_state TEXT NOT NULL DEFAULT 'Unverified',
            content_json TEXT NOT NULL,
            embedding BLOB,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            valid_from TEXT NOT NULL,
            valid_to TEXT,
            trust_score REAL NOT NULL DEFAULT 0.5,
            importance REAL NOT NULL DEFAULT 0.5,
            confidence REAL NOT NULL DEFAULT 0.5,
            supersedes TEXT,
            derived_from_json TEXT NOT NULL DEFAULT '[]',
            source_task_id TEXT,
            source_conversation_id TEXT,
            scope_json TEXT NOT NULL,
            sensitivity TEXT NOT NULL DEFAULT 'Internal',
            access_count INTEGER NOT NULL DEFAULT 0,
            last_accessed TEXT NOT NULL,
            version INTEGER NOT NULL DEFAULT 1,
            ttl_seconds INTEGER,
            tags_json TEXT NOT NULL DEFAULT '[]',
            is_deleted INTEGER NOT NULL DEFAULT 0,
            deleted_reason TEXT
        );

        CREATE INDEX IF NOT EXISTS idx_me_agent
            ON memory_entries(agent_id, memory_type);
        CREATE INDEX IF NOT EXISTS idx_me_created
            ON memory_entries(created_at);
        CREATE INDEX IF NOT EXISTS idx_me_accessed
            ON memory_entries(last_accessed);
        CREATE INDEX IF NOT EXISTS idx_me_deleted
            ON memory_entries(is_deleted);

        CREATE TABLE IF NOT EXISTS memory_versions (
            id TEXT PRIMARY KEY,
            entry_id TEXT NOT NULL,
            version INTEGER NOT NULL,
            content_json TEXT NOT NULL,
            created_at TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_mv_entry
            ON memory_versions(entry_id);

        CREATE TABLE IF NOT EXISTS memory_access_grants (
            id TEXT PRIMARY KEY,
            owner_agent_id TEXT NOT NULL,
            grantee_agent_id TEXT NOT NULL,
            access_json TEXT NOT NULL,
            granted_at TEXT NOT NULL,
            expires_at TEXT
        );

        CREATE INDEX IF NOT EXISTS idx_mag_owner
            ON memory_access_grants(owner_agent_id);
        CREATE INDEX IF NOT EXISTS idx_mag_grantee
            ON memory_access_grants(grantee_agent_id);
    ";

    /// Saves a memory entry (insert or replace).
    pub fn save_entry(&self, entry: &MemoryEntry) -> Result<(), MemoryError> {
        let epistemic_json = serde_json::to_string(&entry.epistemic_class)
            .map_err(|e| MemoryError::PersistenceError(format!("serialize epistemic: {e}")))?;
        let content_json = serde_json::to_string(&entry.content)
            .map_err(|e| MemoryError::PersistenceError(format!("serialize content: {e}")))?;
        let derived_json = serde_json::to_string(&entry.derived_from)
            .map_err(|e| MemoryError::PersistenceError(format!("serialize derived: {e}")))?;
        let scope_json = serde_json::to_string(&entry.scope)
            .map_err(|e| MemoryError::PersistenceError(format!("serialize scope: {e}")))?;
        let tags_json = serde_json::to_string(&entry.tags)
            .map_err(|e| MemoryError::PersistenceError(format!("serialize tags: {e}")))?;

        let embedding_blob: Option<Vec<u8>> = entry
            .embedding
            .as_ref()
            .map(|v| v.iter().flat_map(|f| f.to_le_bytes()).collect());

        self.db
            .execute(
                "INSERT OR REPLACE INTO memory_entries
                 (id, schema_version, agent_id, memory_type, epistemic_class_json,
                  validation_state, content_json, embedding, created_at, updated_at,
                  valid_from, valid_to, trust_score, importance, confidence,
                  supersedes, derived_from_json, source_task_id, source_conversation_id,
                  scope_json, sensitivity, access_count, last_accessed, version,
                  ttl_seconds, tags_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                         ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19,
                         ?20, ?21, ?22, ?23, ?24, ?25, ?26)",
                params![
                    entry.id.to_string(),
                    entry.schema_version,
                    entry.agent_id,
                    entry.memory_type.to_string(),
                    epistemic_json,
                    entry.validation_state.to_string(),
                    content_json,
                    embedding_blob,
                    entry.created_at.to_rfc3339(),
                    entry.updated_at.to_rfc3339(),
                    entry.valid_from.to_rfc3339(),
                    entry.valid_to.map(|t| t.to_rfc3339()),
                    entry.trust_score,
                    entry.importance,
                    entry.confidence,
                    entry.supersedes.map(|s| s.to_string()),
                    derived_json,
                    entry.source_task_id,
                    entry.source_conversation_id,
                    scope_json,
                    entry.sensitivity.to_string(),
                    entry.access_count as i64,
                    entry.last_accessed.to_rfc3339(),
                    entry.version,
                    entry.ttl,
                    tags_json,
                ],
            )
            .map_err(|e| MemoryError::PersistenceError(format!("save entry: {e}")))?;

        Ok(())
    }

    /// Loads all non-deleted entries for an agent, optionally filtered by type.
    pub fn load_entries(
        &self,
        agent_id: &str,
        memory_type: Option<MemoryType>,
    ) -> Result<Vec<MemoryEntry>, MemoryError> {
        let (sql, type_filter);
        if let Some(mt) = memory_type {
            type_filter = mt.to_string();
            sql = "SELECT * FROM memory_entries WHERE agent_id = ?1 AND memory_type = ?2 AND is_deleted = 0";
        } else {
            type_filter = String::new();
            sql = "SELECT * FROM memory_entries WHERE agent_id = ?1 AND is_deleted = 0";
        }

        let mut stmt = self
            .db
            .prepare(sql)
            .map_err(|e| MemoryError::PersistenceError(format!("load prepare: {e}")))?;

        let rows = if memory_type.is_some() {
            stmt.query_map(params![agent_id, type_filter], row_to_entry)
        } else {
            stmt.query_map(params![agent_id], row_to_entry)
        }
        .map_err(|e| MemoryError::PersistenceError(format!("load query: {e}")))?;

        let mut entries = Vec::new();
        for row in rows {
            let entry = row.map_err(|e| MemoryError::PersistenceError(format!("load row: {e}")))?;
            entries.push(entry?);
        }
        Ok(entries)
    }

    /// Loads a single entry by ID (including soft-deleted).
    pub fn load_entry(&self, id: MemoryId) -> Result<Option<MemoryEntry>, MemoryError> {
        let mut stmt = self
            .db
            .prepare("SELECT * FROM memory_entries WHERE id = ?1")
            .map_err(|e| MemoryError::PersistenceError(format!("load_entry prepare: {e}")))?;

        let mut rows = stmt
            .query_map(params![id.to_string()], row_to_entry)
            .map_err(|e| MemoryError::PersistenceError(format!("load_entry query: {e}")))?;

        match rows.next() {
            Some(row) => {
                let entry =
                    row.map_err(|e| MemoryError::PersistenceError(format!("load_entry row: {e}")))?;
                Ok(Some(entry?))
            }
            None => Ok(None),
        }
    }

    /// Soft-deletes an entry, recording the reason.
    pub fn soft_delete(&self, id: MemoryId, reason: &str) -> Result<(), MemoryError> {
        let updated = self
            .db
            .execute(
                "UPDATE memory_entries SET is_deleted = 1, deleted_reason = ?2 WHERE id = ?1",
                params![id.to_string(), reason],
            )
            .map_err(|e| MemoryError::PersistenceError(format!("soft_delete: {e}")))?;

        if updated == 0 {
            return Err(MemoryError::EntryNotFound(id));
        }
        Ok(())
    }

    /// Saves a version snapshot of an entry's content.
    pub fn save_version(
        &self,
        entry_id: MemoryId,
        version: u32,
        content: &MemoryContent,
    ) -> Result<(), MemoryError> {
        let content_json = serde_json::to_string(content)
            .map_err(|e| MemoryError::PersistenceError(format!("serialize version: {e}")))?;

        self.db
            .execute(
                "INSERT INTO memory_versions (id, entry_id, version, content_json, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    Uuid::new_v4().to_string(),
                    entry_id.to_string(),
                    version,
                    content_json,
                    Utc::now().to_rfc3339(),
                ],
            )
            .map_err(|e| MemoryError::PersistenceError(format!("save version: {e}")))?;

        Ok(())
    }

    /// Loads all version snapshots for an entry, ordered by version.
    pub fn load_versions(
        &self,
        entry_id: MemoryId,
    ) -> Result<Vec<(u32, MemoryContent)>, MemoryError> {
        let mut stmt = self
            .db
            .prepare(
                "SELECT version, content_json FROM memory_versions
                 WHERE entry_id = ?1 ORDER BY version ASC",
            )
            .map_err(|e| MemoryError::PersistenceError(format!("load versions prepare: {e}")))?;

        let rows = stmt
            .query_map(params![entry_id.to_string()], |row| {
                let version: u32 = row.get(0)?;
                let content_json: String = row.get(1)?;
                Ok((version, content_json))
            })
            .map_err(|e| MemoryError::PersistenceError(format!("load versions query: {e}")))?;

        let mut versions = Vec::new();
        for row in rows {
            let (version, json) =
                row.map_err(|e| MemoryError::PersistenceError(format!("version row: {e}")))?;
            let content: MemoryContent = serde_json::from_str(&json)
                .map_err(|e| MemoryError::PersistenceError(format!("parse version: {e}")))?;
            versions.push((version, content));
        }
        Ok(versions)
    }

    /// Counts non-deleted entries for an agent and memory type.
    pub fn count_entries(
        &self,
        agent_id: &str,
        memory_type: MemoryType,
    ) -> Result<usize, MemoryError> {
        let count: i64 = self
            .db
            .query_row(
                "SELECT COUNT(*) FROM memory_entries
                 WHERE agent_id = ?1 AND memory_type = ?2 AND is_deleted = 0",
                params![agent_id, memory_type.to_string()],
                |row| row.get(0),
            )
            .map_err(|e| MemoryError::PersistenceError(format!("count: {e}")))?;

        Ok(count as usize)
    }

    /// Updates access tracking fields for an entry.
    pub fn update_access(&self, id: MemoryId) -> Result<(), MemoryError> {
        self.db
            .execute(
                "UPDATE memory_entries SET access_count = access_count + 1,
                 last_accessed = ?2 WHERE id = ?1",
                params![id.to_string(), Utc::now().to_rfc3339()],
            )
            .map_err(|e| MemoryError::PersistenceError(format!("update_access: {e}")))?;
        Ok(())
    }

    /// Updates the validation state of an entry (used by contradiction resolution).
    pub fn update_validation_state(
        &self,
        id: MemoryId,
        state: ValidationState,
    ) -> Result<(), MemoryError> {
        let rows = self
            .db
            .execute(
                "UPDATE memory_entries SET validation_state = ?2, updated_at = ?3 WHERE id = ?1",
                params![id.to_string(), state.to_string(), Utc::now().to_rfc3339()],
            )
            .map_err(|e| MemoryError::PersistenceError(format!("update_validation_state: {e}")))?;

        if rows == 0 {
            return Err(MemoryError::EntryNotFound(id));
        }
        Ok(())
    }

    /// Updates the `valid_to` field of an entry (used by temporal succession).
    pub fn update_valid_to(
        &self,
        id: MemoryId,
        valid_to: chrono::DateTime<Utc>,
    ) -> Result<(), MemoryError> {
        let rows = self
            .db
            .execute(
                "UPDATE memory_entries SET valid_to = ?2, updated_at = ?3 WHERE id = ?1",
                params![
                    id.to_string(),
                    valid_to.to_rfc3339(),
                    Utc::now().to_rfc3339()
                ],
            )
            .map_err(|e| MemoryError::PersistenceError(format!("update_valid_to: {e}")))?;

        if rows == 0 {
            return Err(MemoryError::EntryNotFound(id));
        }
        Ok(())
    }

    /// Updates tags for an entry (used by contradiction resolution).
    pub fn update_tags(&self, id: MemoryId, tags: &[String]) -> Result<(), MemoryError> {
        let tags_json = serde_json::to_string(tags)
            .map_err(|e| MemoryError::PersistenceError(format!("serialize tags: {e}")))?;

        let rows = self
            .db
            .execute(
                "UPDATE memory_entries SET tags_json = ?2, updated_at = ?3 WHERE id = ?1",
                params![id.to_string(), tags_json, Utc::now().to_rfc3339()],
            )
            .map_err(|e| MemoryError::PersistenceError(format!("update_tags: {e}")))?;

        if rows == 0 {
            return Err(MemoryError::EntryNotFound(id));
        }
        Ok(())
    }
}

// ── Row parsing ─────────────────────────────────────────────────────────────

fn row_to_entry(row: &rusqlite::Row<'_>) -> rusqlite::Result<Result<MemoryEntry, MemoryError>> {
    let id_str: String = row.get("id")?;
    let schema_version: u32 = row.get("schema_version")?;
    let agent_id: String = row.get("agent_id")?;
    let memory_type_str: String = row.get("memory_type")?;
    let epistemic_json: String = row.get("epistemic_class_json")?;
    let validation_str: String = row.get("validation_state")?;
    let content_json: String = row.get("content_json")?;
    let embedding_blob: Option<Vec<u8>> = row.get("embedding")?;
    let created_str: String = row.get("created_at")?;
    let updated_str: String = row.get("updated_at")?;
    let valid_from_str: String = row.get("valid_from")?;
    let valid_to_str: Option<String> = row.get("valid_to")?;
    let trust_score: f64 = row.get("trust_score")?;
    let importance: f64 = row.get("importance")?;
    let confidence: f64 = row.get("confidence")?;
    let supersedes_str: Option<String> = row.get("supersedes")?;
    let derived_json: String = row.get("derived_from_json")?;
    let source_task_id: Option<String> = row.get("source_task_id")?;
    let source_conversation_id: Option<String> = row.get("source_conversation_id")?;
    let scope_json: String = row.get("scope_json")?;
    let sensitivity_str: String = row.get("sensitivity")?;
    let access_count: i64 = row.get("access_count")?;
    let last_accessed_str: String = row.get("last_accessed")?;
    let version: u32 = row.get("version")?;
    let ttl: Option<i64> = row.get("ttl_seconds")?;
    let tags_json: String = row.get("tags_json")?;

    Ok(parse_entry_fields(
        id_str,
        schema_version,
        agent_id,
        memory_type_str,
        epistemic_json,
        validation_str,
        content_json,
        embedding_blob,
        created_str,
        updated_str,
        valid_from_str,
        valid_to_str,
        trust_score,
        importance,
        confidence,
        supersedes_str,
        derived_json,
        source_task_id,
        source_conversation_id,
        scope_json,
        sensitivity_str,
        access_count,
        last_accessed_str,
        version,
        ttl,
        tags_json,
    ))
}

#[allow(clippy::too_many_arguments)]
fn parse_entry_fields(
    id_str: String,
    schema_version: u32,
    agent_id: String,
    memory_type_str: String,
    epistemic_json: String,
    validation_str: String,
    content_json: String,
    embedding_blob: Option<Vec<u8>>,
    created_str: String,
    updated_str: String,
    valid_from_str: String,
    valid_to_str: Option<String>,
    trust_score: f64,
    importance: f64,
    confidence: f64,
    supersedes_str: Option<String>,
    derived_json: String,
    source_task_id: Option<String>,
    source_conversation_id: Option<String>,
    scope_json: String,
    sensitivity_str: String,
    access_count: i64,
    last_accessed_str: String,
    version: u32,
    ttl: Option<i64>,
    tags_json: String,
) -> Result<MemoryEntry, MemoryError> {
    let id: Uuid = id_str
        .parse()
        .map_err(|e| MemoryError::PersistenceError(format!("parse id: {e}")))?;

    let memory_type = match memory_type_str.as_str() {
        "Working" => MemoryType::Working,
        "Episodic" => MemoryType::Episodic,
        "Semantic" => MemoryType::Semantic,
        "Procedural" => MemoryType::Procedural,
        other => {
            return Err(MemoryError::PersistenceError(format!(
                "unknown memory type: {other}"
            )))
        }
    };

    let epistemic_class: EpistemicClass = serde_json::from_str(&epistemic_json)
        .map_err(|e| MemoryError::PersistenceError(format!("parse epistemic: {e}")))?;

    let validation_state = match validation_str.as_str() {
        "Unverified" => ValidationState::Unverified,
        "Corroborated" => ValidationState::Corroborated,
        "Contested" => ValidationState::Contested,
        "Deprecated" => ValidationState::Deprecated,
        "Revoked" => ValidationState::Revoked,
        other => {
            return Err(MemoryError::PersistenceError(format!(
                "unknown validation: {other}"
            )))
        }
    };

    let content: MemoryContent = serde_json::from_str(&content_json)
        .map_err(|e| MemoryError::PersistenceError(format!("parse content: {e}")))?;

    let embedding: Option<Vec<f32>> = embedding_blob.map(|blob| {
        blob.chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect()
    });

    let parse_dt = |s: &str| -> Result<DateTime<Utc>, MemoryError> {
        chrono::DateTime::parse_from_rfc3339(s)
            .map(|d| d.with_timezone(&Utc))
            .map_err(|e| MemoryError::PersistenceError(format!("parse datetime: {e}")))
    };

    let created_at = parse_dt(&created_str)?;
    let updated_at = parse_dt(&updated_str)?;
    let valid_from = parse_dt(&valid_from_str)?;
    let valid_to = valid_to_str.as_deref().map(parse_dt).transpose()?;
    let last_accessed = parse_dt(&last_accessed_str)?;

    let supersedes = supersedes_str
        .map(|s| {
            s.parse::<Uuid>()
                .map_err(|e| MemoryError::PersistenceError(format!("parse supersedes: {e}")))
        })
        .transpose()?;

    let derived_from: Vec<Uuid> = serde_json::from_str(&derived_json)
        .map_err(|e| MemoryError::PersistenceError(format!("parse derived: {e}")))?;

    let scope: MemoryScope = serde_json::from_str(&scope_json)
        .map_err(|e| MemoryError::PersistenceError(format!("parse scope: {e}")))?;

    let sensitivity = match sensitivity_str.as_str() {
        "Public" => SensitivityClass::Public,
        "Internal" => SensitivityClass::Internal,
        "Sensitive" => SensitivityClass::Sensitive,
        "Restricted" => SensitivityClass::Restricted,
        other => {
            return Err(MemoryError::PersistenceError(format!(
                "unknown sensitivity: {other}"
            )))
        }
    };

    let tags: Vec<String> = serde_json::from_str(&tags_json)
        .map_err(|e| MemoryError::PersistenceError(format!("parse tags: {e}")))?;

    Ok(MemoryEntry {
        id,
        schema_version,
        agent_id,
        memory_type,
        epistemic_class,
        validation_state,
        content,
        embedding,
        created_at,
        updated_at,
        valid_from,
        valid_to,
        trust_score: trust_score as f32,
        importance: importance as f32,
        confidence: confidence as f32,
        supersedes,
        derived_from,
        source_task_id,
        source_conversation_id,
        scope,
        sensitivity,
        access_count: access_count as u64,
        last_accessed,
        version,
        ttl,
        tags,
    })
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::*;

    fn make_entry(agent_id: &str, mt: MemoryType, content: MemoryContent) -> MemoryEntry {
        let now = Utc::now();
        MemoryEntry {
            id: Uuid::new_v4(),
            schema_version: 1,
            agent_id: agent_id.into(),
            memory_type: mt,
            epistemic_class: EpistemicClass::Observation,
            validation_state: ValidationState::Unverified,
            content,
            embedding: None,
            created_at: now,
            updated_at: now,
            valid_from: now,
            valid_to: None,
            trust_score: 0.9,
            importance: 0.7,
            confidence: 0.8,
            supersedes: None,
            derived_from: vec![],
            source_task_id: None,
            source_conversation_id: None,
            scope: MemoryScope::Agent,
            sensitivity: SensitivityClass::Internal,
            access_count: 0,
            last_accessed: now,
            version: 1,
            ttl: None,
            tags: vec!["test".into()],
        }
    }

    #[test]
    fn save_and_load() {
        let p = MemoryPersistence::in_memory().unwrap();
        let entry = make_entry(
            "agent-1",
            MemoryType::Working,
            MemoryContent::Context {
                key: "goal".into(),
                value: serde_json::json!("test"),
            },
        );
        let id = entry.id;

        p.save_entry(&entry).unwrap();

        let loaded = p.load_entry(id).unwrap().unwrap();
        assert_eq!(loaded.id, id);
        assert_eq!(loaded.agent_id, "agent-1");
        assert_eq!(loaded.memory_type, MemoryType::Working);
    }

    #[test]
    fn load_entries_filters_by_type() {
        let p = MemoryPersistence::in_memory().unwrap();

        let working = make_entry(
            "a",
            MemoryType::Working,
            MemoryContent::Context {
                key: "k".into(),
                value: serde_json::Value::Null,
            },
        );
        let episodic = make_entry(
            "a",
            MemoryType::Episodic,
            MemoryContent::Episode {
                event_type: EpisodeType::Conversation,
                summary: "test".into(),
                details: serde_json::Value::Null,
                outcome: None,
                duration_ms: None,
            },
        );

        p.save_entry(&working).unwrap();
        p.save_entry(&episodic).unwrap();

        let all = p.load_entries("a", None).unwrap();
        assert_eq!(all.len(), 2);

        let just_working = p.load_entries("a", Some(MemoryType::Working)).unwrap();
        assert_eq!(just_working.len(), 1);
        assert_eq!(just_working[0].memory_type, MemoryType::Working);
    }

    #[test]
    fn soft_delete_and_verify() {
        let p = MemoryPersistence::in_memory().unwrap();
        let entry = make_entry(
            "a",
            MemoryType::Working,
            MemoryContent::Context {
                key: "k".into(),
                value: serde_json::Value::Null,
            },
        );
        let id = entry.id;

        p.save_entry(&entry).unwrap();
        p.soft_delete(id, "test reason").unwrap();

        // load_entries excludes soft-deleted
        let entries = p.load_entries("a", None).unwrap();
        assert!(entries.is_empty());

        // load_entry still finds it (for audit)
        let found = p.load_entry(id).unwrap();
        assert!(found.is_some());
    }

    #[test]
    fn save_and_load_versions() {
        let p = MemoryPersistence::in_memory().unwrap();
        let entry_id = Uuid::new_v4();

        let c1 = MemoryContent::Context {
            key: "k".into(),
            value: serde_json::json!("v1"),
        };
        let c2 = MemoryContent::Context {
            key: "k".into(),
            value: serde_json::json!("v2"),
        };

        p.save_version(entry_id, 1, &c1).unwrap();
        p.save_version(entry_id, 2, &c2).unwrap();

        let versions = p.load_versions(entry_id).unwrap();
        assert_eq!(versions.len(), 2);
        assert_eq!(versions[0].0, 1);
        assert_eq!(versions[1].0, 2);
    }

    #[test]
    fn count_entries_by_type() {
        let p = MemoryPersistence::in_memory().unwrap();

        for _ in 0..3 {
            let e = make_entry(
                "a",
                MemoryType::Episodic,
                MemoryContent::Episode {
                    event_type: EpisodeType::ActionExecuted,
                    summary: "x".into(),
                    details: serde_json::Value::Null,
                    outcome: None,
                    duration_ms: None,
                },
            );
            p.save_entry(&e).unwrap();
        }

        assert_eq!(p.count_entries("a", MemoryType::Episodic).unwrap(), 3);
        assert_eq!(p.count_entries("a", MemoryType::Working).unwrap(), 0);
    }

    #[test]
    fn save_entry_with_embedding() {
        let p = MemoryPersistence::in_memory().unwrap();
        let mut entry = make_entry(
            "a",
            MemoryType::Working,
            MemoryContent::Context {
                key: "k".into(),
                value: serde_json::Value::Null,
            },
        );
        entry.embedding = Some(vec![1.0, 2.0, 3.0, 4.0]);

        p.save_entry(&entry).unwrap();
        let loaded = p.load_entry(entry.id).unwrap().unwrap();
        assert_eq!(loaded.embedding, Some(vec![1.0, 2.0, 3.0, 4.0]));
    }

    #[test]
    fn soft_delete_nonexistent_fails() {
        let p = MemoryPersistence::in_memory().unwrap();
        let result = p.soft_delete(Uuid::new_v4(), "nope");
        assert!(result.is_err());
    }
}
