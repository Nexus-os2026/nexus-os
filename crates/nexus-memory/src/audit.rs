//! Hash-chained audit log for memory operations.
//!
//! Every memory write, read, delete, and search is logged here with a SHA-256
//! hash chain linking each entry to its predecessor.  This is Invariant #1:
//! **every memory operation must be audited**.

use chrono::Utc;
use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::types::{MemoryAuditEntry, MemoryError, MemoryId, MemoryOperation, MemoryType};

/// Hash-chained audit log backed by SQLite.
pub struct MemoryAuditLog {
    db: Mutex<Connection>,
    last_hash: Mutex<Option<String>>,
}

impl MemoryAuditLog {
    /// Opens (or creates) the audit database at `db_path` and loads the last
    /// hash for chain continuity.
    pub fn new(db_path: &str) -> Result<Self, MemoryError> {
        let conn = Connection::open(db_path)
            .map_err(|e| MemoryError::PersistenceError(format!("audit db open: {e}")))?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS memory_audit (
                id TEXT PRIMARY KEY,
                agent_id TEXT NOT NULL,
                accessor_id TEXT NOT NULL,
                operation TEXT NOT NULL,
                memory_type TEXT NOT NULL,
                entry_id TEXT,
                timestamp TEXT NOT NULL,
                details TEXT,
                hash TEXT NOT NULL,
                previous_hash TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_memory_audit_agent
                ON memory_audit(agent_id);
            CREATE INDEX IF NOT EXISTS idx_memory_audit_time
                ON memory_audit(timestamp);",
        )
        .map_err(|e| MemoryError::PersistenceError(format!("audit schema: {e}")))?;

        // Load last hash for chain continuity
        let last_hash: Option<String> = conn
            .query_row(
                "SELECT hash FROM memory_audit ORDER BY rowid DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .ok();

        Ok(Self {
            db: Mutex::new(conn),
            last_hash: Mutex::new(last_hash),
        })
    }

    /// Creates an in-memory audit log (for testing).
    pub fn in_memory() -> Result<Self, MemoryError> {
        Self::new(":memory:")
    }

    /// Computes the SHA-256 hash for a new audit entry.
    fn compute_hash(
        previous_hash: &Option<String>,
        agent_id: &str,
        operation: &MemoryOperation,
        timestamp: &str,
        entry_id: &Option<MemoryId>,
    ) -> String {
        let mut hasher = Sha256::new();
        if let Some(ref prev) = previous_hash {
            hasher.update(prev.as_bytes());
        }
        hasher.update(agent_id.as_bytes());
        hasher.update(operation.to_string().as_bytes());
        hasher.update(timestamp.as_bytes());
        if let Some(eid) = entry_id {
            hasher.update(eid.to_string().as_bytes());
        }
        hex::encode(hasher.finalize())
    }

    /// Appends an audit entry, computing its hash and chaining to the previous.
    pub async fn log(
        &self,
        agent_id: &str,
        accessor_id: &str,
        operation: MemoryOperation,
        memory_type: MemoryType,
        entry_id: Option<MemoryId>,
        details: Option<String>,
    ) -> Result<MemoryAuditEntry, MemoryError> {
        let id = Uuid::new_v4();
        let timestamp = Utc::now();
        let ts_str = timestamp.to_rfc3339();

        let mut last_hash = self.last_hash.lock().await;
        let hash = Self::compute_hash(&last_hash, agent_id, &operation, &ts_str, &entry_id);

        let entry = MemoryAuditEntry {
            id,
            agent_id: agent_id.to_string(),
            accessor_id: accessor_id.to_string(),
            operation,
            memory_type,
            entry_id,
            timestamp,
            details,
            hash: hash.clone(),
            previous_hash: last_hash.clone(),
        };

        let db = self.db.lock().await;
        db.execute(
            "INSERT INTO memory_audit (id, agent_id, accessor_id, operation, memory_type,
             entry_id, timestamp, details, hash, previous_hash)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                entry.id.to_string(),
                entry.agent_id,
                entry.accessor_id,
                entry.operation.to_string(),
                entry.memory_type.to_string(),
                entry.entry_id.map(|e| e.to_string()),
                ts_str,
                entry.details,
                entry.hash,
                entry.previous_hash,
            ],
        )
        .map_err(|e| MemoryError::PersistenceError(format!("audit insert: {e}")))?;

        *last_hash = Some(hash);

        Ok(entry)
    }

    /// Retrieves audit history for an agent, most recent first.
    pub async fn query(
        &self,
        agent_id: &str,
        limit: usize,
    ) -> Result<Vec<MemoryAuditEntry>, MemoryError> {
        let db = self.db.lock().await;
        let mut stmt = db
            .prepare(
                "SELECT id, agent_id, accessor_id, operation, memory_type,
                        entry_id, timestamp, details, hash, previous_hash
                 FROM memory_audit
                 WHERE agent_id = ?1
                 ORDER BY rowid DESC
                 LIMIT ?2",
            )
            .map_err(|e| MemoryError::PersistenceError(format!("audit query prepare: {e}")))?;

        let rows = stmt
            .query_map(params![agent_id, limit as i64], |row| {
                Ok(AuditRow {
                    id: row.get(0)?,
                    agent_id: row.get(1)?,
                    accessor_id: row.get(2)?,
                    operation: row.get(3)?,
                    memory_type: row.get(4)?,
                    entry_id: row.get(5)?,
                    timestamp: row.get(6)?,
                    details: row.get(7)?,
                    hash: row.get(8)?,
                    previous_hash: row.get(9)?,
                })
            })
            .map_err(|e| MemoryError::PersistenceError(format!("audit query: {e}")))?;

        let mut entries = Vec::new();
        for row in rows {
            let r =
                row.map_err(|e| MemoryError::PersistenceError(format!("audit row read: {e}")))?;
            entries.push(parse_audit_row(r)?);
        }
        Ok(entries)
    }

    /// Verifies the integrity of the entire hash chain.
    /// Returns `true` if every entry's hash matches its recomputed value and
    /// chains correctly to the previous entry.
    pub async fn verify_chain(&self) -> Result<bool, MemoryError> {
        let db = self.db.lock().await;
        let mut stmt = db
            .prepare(
                "SELECT id, agent_id, accessor_id, operation, memory_type,
                        entry_id, timestamp, details, hash, previous_hash
                 FROM memory_audit
                 ORDER BY rowid ASC",
            )
            .map_err(|e| MemoryError::PersistenceError(format!("verify prepare: {e}")))?;

        let rows = stmt
            .query_map([], |row| {
                Ok(AuditRow {
                    id: row.get(0)?,
                    agent_id: row.get(1)?,
                    accessor_id: row.get(2)?,
                    operation: row.get(3)?,
                    memory_type: row.get(4)?,
                    entry_id: row.get(5)?,
                    timestamp: row.get(6)?,
                    details: row.get(7)?,
                    hash: row.get(8)?,
                    previous_hash: row.get(9)?,
                })
            })
            .map_err(|e| MemoryError::PersistenceError(format!("verify query: {e}")))?;

        let mut expected_prev: Option<String> = None;

        for row in rows {
            let r = row.map_err(|e| MemoryError::PersistenceError(format!("verify row: {e}")))?;

            // Check chain linkage
            if r.previous_hash != expected_prev {
                return Ok(false);
            }

            // Parse the operation for hash recomputation
            let operation = parse_operation(&r.operation)?;
            let entry_id = r
                .entry_id
                .as_deref()
                .map(|s| s.parse::<Uuid>().unwrap_or_else(|_| Uuid::nil()));

            let recomputed = Self::compute_hash(
                &r.previous_hash,
                &r.agent_id,
                &operation,
                &r.timestamp,
                &entry_id,
            );

            if recomputed != r.hash {
                return Ok(false);
            }

            expected_prev = Some(r.hash);
        }

        Ok(true)
    }
}

// ── Internal helpers ────────────────────────────────────────────────────────

/// Raw row from SQLite (all strings).
struct AuditRow {
    id: String,
    agent_id: String,
    accessor_id: String,
    operation: String,
    memory_type: String,
    entry_id: Option<String>,
    timestamp: String,
    details: Option<String>,
    hash: String,
    previous_hash: Option<String>,
}

fn parse_operation(s: &str) -> Result<MemoryOperation, MemoryError> {
    match s {
        "Read" => Ok(MemoryOperation::Read),
        "Write" => Ok(MemoryOperation::Write),
        "Update" => Ok(MemoryOperation::Update),
        "SoftDelete" => Ok(MemoryOperation::SoftDelete),
        "Search" => Ok(MemoryOperation::Search),
        "Share" => Ok(MemoryOperation::Share),
        "Rollback" => Ok(MemoryOperation::Rollback),
        "GarbageCollect" => Ok(MemoryOperation::GarbageCollect),
        other => Err(MemoryError::ValidationError(format!(
            "unknown operation: {other}"
        ))),
    }
}

fn parse_memory_type(s: &str) -> Result<MemoryType, MemoryError> {
    match s {
        "Working" => Ok(MemoryType::Working),
        "Episodic" => Ok(MemoryType::Episodic),
        "Semantic" => Ok(MemoryType::Semantic),
        "Procedural" => Ok(MemoryType::Procedural),
        other => Err(MemoryError::ValidationError(format!(
            "unknown memory type: {other}"
        ))),
    }
}

fn parse_audit_row(r: AuditRow) -> Result<MemoryAuditEntry, MemoryError> {
    let id =
        r.id.parse::<Uuid>()
            .map_err(|e| MemoryError::ValidationError(format!("bad audit id: {e}")))?;
    let operation = parse_operation(&r.operation)?;
    let memory_type = parse_memory_type(&r.memory_type)?;
    let entry_id = r
        .entry_id
        .as_deref()
        .map(|s| {
            s.parse::<Uuid>()
                .map_err(|e| MemoryError::ValidationError(format!("bad entry_id: {e}")))
        })
        .transpose()?;
    let timestamp = chrono::DateTime::parse_from_rfc3339(&r.timestamp)
        .map_err(|e| MemoryError::ValidationError(format!("bad timestamp: {e}")))?
        .with_timezone(&Utc);

    Ok(MemoryAuditEntry {
        id,
        agent_id: r.agent_id,
        accessor_id: r.accessor_id,
        operation,
        memory_type,
        entry_id,
        timestamp,
        details: r.details,
        hash: r.hash,
        previous_hash: r.previous_hash,
    })
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn append_and_query() {
        let log = MemoryAuditLog::in_memory().unwrap();

        log.log(
            "agent-1",
            "system",
            MemoryOperation::Write,
            MemoryType::Working,
            Some(Uuid::new_v4()),
            Some("test write".into()),
        )
        .await
        .unwrap();

        log.log(
            "agent-1",
            "system",
            MemoryOperation::Read,
            MemoryType::Working,
            None,
            None,
        )
        .await
        .unwrap();

        let entries = log.query("agent-1", 10).await.unwrap();
        assert_eq!(entries.len(), 2);
        // Most recent first
        assert_eq!(entries[0].operation, MemoryOperation::Read);
        assert_eq!(entries[1].operation, MemoryOperation::Write);
    }

    #[tokio::test]
    async fn hash_chain_valid() {
        let log = MemoryAuditLog::in_memory().unwrap();

        for i in 0..5 {
            log.log(
                "agent-1",
                "system",
                MemoryOperation::Write,
                MemoryType::Episodic,
                Some(Uuid::new_v4()),
                Some(format!("entry {i}")),
            )
            .await
            .unwrap();
        }

        assert!(log.verify_chain().await.unwrap());
    }

    #[tokio::test]
    async fn hash_chain_detects_tampering() {
        let log = MemoryAuditLog::in_memory().unwrap();

        log.log(
            "agent-1",
            "system",
            MemoryOperation::Write,
            MemoryType::Working,
            None,
            None,
        )
        .await
        .unwrap();

        log.log(
            "agent-1",
            "system",
            MemoryOperation::Read,
            MemoryType::Working,
            None,
            None,
        )
        .await
        .unwrap();

        // Tamper with first entry's hash
        {
            let db = log.db.lock().await;
            db.execute(
                "UPDATE memory_audit SET hash = 'tampered' WHERE rowid = 1",
                [],
            )
            .unwrap();
        }

        assert!(!log.verify_chain().await.unwrap());
    }

    #[tokio::test]
    async fn query_by_agent_filters_correctly() {
        let log = MemoryAuditLog::in_memory().unwrap();

        log.log(
            "agent-1",
            "sys",
            MemoryOperation::Write,
            MemoryType::Working,
            None,
            None,
        )
        .await
        .unwrap();
        log.log(
            "agent-2",
            "sys",
            MemoryOperation::Write,
            MemoryType::Working,
            None,
            None,
        )
        .await
        .unwrap();

        let a1 = log.query("agent-1", 10).await.unwrap();
        let a2 = log.query("agent-2", 10).await.unwrap();
        assert_eq!(a1.len(), 1);
        assert_eq!(a2.len(), 1);
        assert_eq!(a1[0].agent_id, "agent-1");
        assert_eq!(a2[0].agent_id, "agent-2");
    }

    #[tokio::test]
    async fn empty_chain_verifies() {
        let log = MemoryAuditLog::in_memory().unwrap();
        assert!(log.verify_chain().await.unwrap());
    }

    #[tokio::test]
    async fn chain_links_previous_hash() {
        let log = MemoryAuditLog::in_memory().unwrap();

        let e1 = log
            .log(
                "a",
                "s",
                MemoryOperation::Write,
                MemoryType::Working,
                None,
                None,
            )
            .await
            .unwrap();
        let e2 = log
            .log(
                "a",
                "s",
                MemoryOperation::Read,
                MemoryType::Working,
                None,
                None,
            )
            .await
            .unwrap();

        assert!(e1.previous_hash.is_none());
        assert_eq!(e2.previous_hash, Some(e1.hash));
    }
}
