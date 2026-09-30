//! Durable, fail-closed, run-scoped ledger for governed coding runs
//! (Phase One P1A-002).
//!
//! This is deliberately separate from the general `audit_events` table, which
//! is fail-open, may fall back to an in-memory database, and hashes only
//! `"{prev}:{seq}:{detail}"`. The coding-run ledger instead:
//!
//! - opens only a file-backed database at an absolute path; there is no
//!   in-memory constructor and no silent fallback;
//! - uses its own connection, so `synchronous=FULL` (with WAL) applies to this
//!   store only and does not change the durability policy of the main Nexus
//!   database;
//! - assigns a per-run sequence inside an IMMEDIATE transaction, with the
//!   database enforcing uniqueness of `(run_id, seq)`;
//! - hashes every stored field with an unambiguous, length-prefixed encoding
//!   under a domain tag, chained through the previous hash;
//! - refuses UPDATE and DELETE through triggers (a convenience, not the
//!   integrity mechanism: whoever can write the file can drop a trigger, which
//!   is why [`CodingRunLedger::verify_run`] recomputes every hash).
//!
//! Every append returns a `Result`; callers must treat an error as "the event
//! was not recorded" and fail closed.

use std::path::Path;
use std::sync::Mutex;

use rusqlite::{params, Connection, OpenFlags, OptionalExtension, TransactionBehavior};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

/// Hash domain for ledger entries. Changing the encoding requires a new tag.
const ENTRY_DOMAIN: &[u8] = b"nexus.coding_run_ledger.entry.v1";

/// Previous hash of the first entry of every run.
pub const GENESIS_HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";

const MAX_KIND_LEN: usize = 64;
const MAX_PAYLOAD_LEN: usize = 1 << 20;

#[derive(Debug, Error)]
pub enum LedgerError {
    #[error("coding-run ledger unavailable: {0}")]
    Unavailable(String),
    #[error("coding-run ledger database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("coding-run ledger rejected the event: {0}")]
    InvalidEvent(&'static str),
    #[error("coding-run ledger integrity violation: {0}")]
    Integrity(IntegrityViolation),
}

/// What a verification found wrong. Carries no payload content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntegrityViolation {
    /// The stored sequence does not continue from the previous entry.
    SequenceGap { expected: i64, found: i64 },
    /// The stored previous hash is not the previous entry's hash.
    PreviousHashMismatch { seq: i64 },
    /// Recomputing the entry hash from its stored fields gives another value.
    HashMismatch { seq: i64 },
    /// A stored field cannot be decoded (for example a malformed hash).
    MalformedEntry { seq: i64 },
}

impl std::fmt::Display for IntegrityViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SequenceGap { expected, found } => {
                write!(f, "sequence gap: expected {expected}, found {found}")
            }
            Self::PreviousHashMismatch { seq } => write!(f, "previous-hash mismatch at {seq}"),
            Self::HashMismatch { seq } => write!(f, "entry hash mismatch at {seq}"),
            Self::MalformedEntry { seq } => write!(f, "malformed entry at {seq}"),
        }
    }
}

/// An event to append. The ledger assigns the sequence and hashes.
#[derive(Debug, Clone, Copy)]
pub struct NewLedgerEvent<'a> {
    pub run_id: Uuid,
    /// Nanoseconds since the Unix epoch, supplied by the caller and hashed.
    pub timestamp_unix_nanos: i64,
    pub actor_kind: &'a str,
    pub event_kind: &'a str,
    /// Exact payload bytes to store and hash (normally compact JSON).
    pub payload: &'a str,
}

/// A durably committed entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerRecord {
    pub run_id: Uuid,
    pub seq: i64,
    pub timestamp_unix_nanos: i64,
    pub actor_kind: String,
    pub event_kind: String,
    pub payload: String,
    pub previous_hash: String,
    pub current_hash: String,
}

/// A file-backed coding-run ledger with its own connection.
pub struct CodingRunLedger {
    conn: Mutex<Connection>,
}

impl std::fmt::Debug for CodingRunLedger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CodingRunLedger").finish_non_exhaustive()
    }
}

impl CodingRunLedger {
    /// Open (creating if needed) the ledger database at an absolute file path.
    /// URI, in-memory and relative names are refused, and the opened database
    /// must report a file, WAL journaling and `synchronous=FULL`; otherwise the
    /// ledger is unavailable and nothing falls back.
    pub fn open(path: &Path) -> Result<Self, LedgerError> {
        let text = path
            .to_str()
            .ok_or(LedgerError::Unavailable("ledger path is not UTF-8".into()))?;
        if !path.is_absolute()
            || text.is_empty()
            || text.contains(":memory:")
            || text.starts_with("file:")
        {
            return Err(LedgerError::Unavailable(
                "ledger requires an absolute file path".into(),
            ));
        }
        let flags = OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_NO_MUTEX;
        let conn = Connection::open_with_flags(path, flags)
            .map_err(|e| LedgerError::Unavailable(e.to_string()))?;
        let journal: String = conn
            .query_row("PRAGMA journal_mode=WAL", [], |row| row.get(0))
            .map_err(|e| LedgerError::Unavailable(e.to_string()))?;
        if !journal.eq_ignore_ascii_case("wal") {
            return Err(LedgerError::Unavailable(
                "WAL journaling unavailable".into(),
            ));
        }
        conn.execute_batch("PRAGMA synchronous=FULL;")
            .map_err(|e| LedgerError::Unavailable(e.to_string()))?;
        let synchronous: i64 = conn
            .query_row("PRAGMA synchronous", [], |row| row.get(0))
            .map_err(|e| LedgerError::Unavailable(e.to_string()))?;
        if synchronous != 2 {
            return Err(LedgerError::Unavailable(
                "synchronous=FULL unavailable".into(),
            ));
        }
        let file: String = conn
            .query_row(
                "SELECT file FROM pragma_database_list WHERE name = 'main'",
                [],
                |row| row.get(0),
            )
            .map_err(|e| LedgerError::Unavailable(e.to_string()))?;
        if file.is_empty() {
            return Err(LedgerError::Unavailable("ledger is not file-backed".into()));
        }
        conn.execute_batch(SCHEMA)
            .map_err(|e| LedgerError::Unavailable(e.to_string()))?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Durably append one event. The sequence is the next integer for the run
    /// (starting at 0) and the previous hash is the run's last entry hash, both
    /// read and written in one IMMEDIATE transaction. Returns the committed
    /// record; any error means nothing was recorded.
    pub fn append(&self, event: NewLedgerEvent<'_>) -> Result<LedgerRecord, LedgerError> {
        validate_kind(event.actor_kind)?;
        validate_kind(event.event_kind)?;
        if event.payload.len() > MAX_PAYLOAD_LEN {
            return Err(LedgerError::InvalidEvent("payload too large"));
        }
        if event.timestamp_unix_nanos < 0 {
            return Err(LedgerError::InvalidEvent("timestamp before the epoch"));
        }
        if event.run_id.is_nil() {
            return Err(LedgerError::InvalidEvent("nil run id"));
        }
        let mut conn = self
            .conn
            .lock()
            .map_err(|_| LedgerError::Unavailable("ledger lock poisoned".into()))?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let run_key = event.run_id.to_string();
        let last: Option<(i64, String)> = tx
            .query_row(
                "SELECT seq, current_hash FROM coding_run_ledger \
                 WHERE run_id = ?1 ORDER BY seq DESC LIMIT 1",
                params![run_key],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let (seq, previous_hash) = match last {
            Some((seq, hash)) => (
                seq.checked_add(1)
                    .ok_or(LedgerError::InvalidEvent("sequence exhausted"))?,
                hash,
            ),
            None => (0, GENESIS_HASH.to_string()),
        };
        let current_hash = entry_hash(
            event.run_id,
            seq,
            event.timestamp_unix_nanos,
            event.actor_kind,
            event.event_kind,
            event.payload,
            &previous_hash,
        )
        .ok_or(LedgerError::Integrity(IntegrityViolation::MalformedEntry {
            seq: seq.saturating_sub(1),
        }))?;
        tx.execute(
            "INSERT INTO coding_run_ledger \
             (run_id, seq, timestamp_unix_nanos, actor_kind, event_kind, payload, \
              previous_hash, current_hash) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                run_key,
                seq,
                event.timestamp_unix_nanos,
                event.actor_kind,
                event.event_kind,
                event.payload,
                previous_hash,
                current_hash,
            ],
        )?;
        tx.commit()?;
        Ok(LedgerRecord {
            run_id: event.run_id,
            seq,
            timestamp_unix_nanos: event.timestamp_unix_nanos,
            actor_kind: event.actor_kind.to_string(),
            event_kind: event.event_kind.to_string(),
            payload: event.payload.to_string(),
            previous_hash,
            current_hash,
        })
    }

    /// Read every entry of a run and verify it: the sequence must run 0, 1, 2,
    /// … without gaps, each previous hash must equal the prior entry's hash
    /// (the genesis value for the first), and each stored hash must equal the
    /// hash recomputed from all stored fields. Returns the verified entries.
    pub fn verify_run(&self, run_id: Uuid) -> Result<Vec<LedgerRecord>, LedgerError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| LedgerError::Unavailable("ledger lock poisoned".into()))?;
        let mut stmt = conn.prepare(
            "SELECT seq, timestamp_unix_nanos, actor_kind, event_kind, payload, \
                    previous_hash, current_hash \
             FROM coding_run_ledger WHERE run_id = ?1 ORDER BY seq ASC",
        )?;
        let rows = stmt.query_map(params![run_id.to_string()], |row| {
            Ok(LedgerRecord {
                run_id,
                seq: row.get(0)?,
                timestamp_unix_nanos: row.get(1)?,
                actor_kind: row.get(2)?,
                event_kind: row.get(3)?,
                payload: row.get(4)?,
                previous_hash: row.get(5)?,
                current_hash: row.get(6)?,
            })
        })?;
        let mut records = Vec::new();
        let mut expected_previous = GENESIS_HASH.to_string();
        for (index, row) in rows.enumerate() {
            let record = row?;
            let expected_seq = index as i64;
            if record.seq != expected_seq {
                return Err(LedgerError::Integrity(IntegrityViolation::SequenceGap {
                    expected: expected_seq,
                    found: record.seq,
                }));
            }
            if record.previous_hash != expected_previous {
                return Err(LedgerError::Integrity(
                    IntegrityViolation::PreviousHashMismatch { seq: record.seq },
                ));
            }
            let recomputed = entry_hash(
                run_id,
                record.seq,
                record.timestamp_unix_nanos,
                &record.actor_kind,
                &record.event_kind,
                &record.payload,
                &record.previous_hash,
            )
            .ok_or(LedgerError::Integrity(IntegrityViolation::MalformedEntry {
                seq: record.seq,
            }))?;
            if recomputed != record.current_hash {
                return Err(LedgerError::Integrity(IntegrityViolation::HashMismatch {
                    seq: record.seq,
                }));
            }
            expected_previous = record.current_hash.clone();
            records.push(record);
        }
        Ok(records)
    }
}

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS coding_run_ledger (
    run_id               TEXT    NOT NULL,
    seq                  INTEGER NOT NULL CHECK (seq >= 0),
    timestamp_unix_nanos INTEGER NOT NULL CHECK (timestamp_unix_nanos >= 0),
    actor_kind           TEXT    NOT NULL,
    event_kind           TEXT    NOT NULL,
    payload              TEXT    NOT NULL,
    previous_hash        TEXT    NOT NULL,
    current_hash         TEXT    NOT NULL,
    PRIMARY KEY (run_id, seq)
) WITHOUT ROWID;
CREATE TRIGGER IF NOT EXISTS coding_run_ledger_no_update
BEFORE UPDATE ON coding_run_ledger
BEGIN SELECT RAISE(ABORT, 'coding_run_ledger is append-only'); END;
CREATE TRIGGER IF NOT EXISTS coding_run_ledger_no_delete
BEFORE DELETE ON coding_run_ledger
BEGIN SELECT RAISE(ABORT, 'coding_run_ledger is append-only'); END;
";

fn validate_kind(kind: &str) -> Result<(), LedgerError> {
    let valid = !kind.is_empty()
        && kind.len() <= MAX_KIND_LEN
        && kind
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'.');
    if valid {
        Ok(())
    } else {
        Err(LedgerError::InvalidEvent("actor or event kind"))
    }
}

/// SHA-256 over the domain tag and every field, each length-prefixed (u64
/// big-endian) or fixed-width, so no two distinct field tuples share an
/// encoding. Returns `None` if the previous hash is not 32 hex-encoded bytes.
fn entry_hash(
    run_id: Uuid,
    seq: i64,
    timestamp_unix_nanos: i64,
    actor_kind: &str,
    event_kind: &str,
    payload: &str,
    previous_hash: &str,
) -> Option<String> {
    let previous = hex::decode(previous_hash).ok()?;
    if previous.len() != 32 {
        return None;
    }
    let mut hasher = Sha256::new();
    put_bytes(&mut hasher, ENTRY_DOMAIN);
    hasher.update(run_id.as_bytes());
    hasher.update(seq.to_be_bytes());
    hasher.update(timestamp_unix_nanos.to_be_bytes());
    put_bytes(&mut hasher, actor_kind.as_bytes());
    put_bytes(&mut hasher, event_kind.as_bytes());
    put_bytes(&mut hasher, payload.as_bytes());
    hasher.update(&previous);
    Some(hex::encode(hasher.finalize()))
}

fn put_bytes(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// A temporary, file-backed ledger directory removed on drop.
    struct TempLedger {
        dir: PathBuf,
    }

    impl TempLedger {
        fn new() -> Self {
            let dir = std::env::temp_dir().join(format!("nexus-crl-test-{}", Uuid::new_v4()));
            std::fs::create_dir(&dir).unwrap();
            Self { dir }
        }
        fn path(&self) -> PathBuf {
            self.dir.join("ledger.db")
        }
        fn raw(&self) -> Connection {
            Connection::open(self.path()).unwrap()
        }
    }

    impl Drop for TempLedger {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    fn event(run: Uuid, kind: &str, payload: &str) -> NewLedgerEvent<'static> {
        NewLedgerEvent {
            run_id: run,
            timestamp_unix_nanos: 1_700_000_000_000_000_000,
            actor_kind: "backend",
            event_kind: Box::leak(kind.to_string().into_boxed_str()),
            payload: Box::leak(payload.to_string().into_boxed_str()),
        }
    }

    fn seeded() -> (TempLedger, CodingRunLedger, Uuid) {
        let temp = TempLedger::new();
        let ledger = CodingRunLedger::open(&temp.path()).unwrap();
        let run = Uuid::new_v4();
        for i in 0..3 {
            ledger
                .append(event(run, "run.test", &format!("{{\"i\":{i}}}")))
                .unwrap();
        }
        (temp, ledger, run)
    }

    fn drop_guards(conn: &Connection) {
        conn.execute_batch(
            "DROP TRIGGER coding_run_ledger_no_update; DROP TRIGGER coding_run_ledger_no_delete;",
        )
        .unwrap();
    }

    #[test]
    fn p1a_ledger_opens_only_absolute_file_paths() {
        for bad in [":memory:", "relative.db", "file:x?mode=memory", ""] {
            assert!(matches!(
                CodingRunLedger::open(Path::new(bad)),
                Err(LedgerError::Unavailable(_))
            ));
        }
        let temp = TempLedger::new();
        CodingRunLedger::open(&temp.path()).unwrap();
        assert!(temp.path().is_file());
    }

    #[test]
    fn p1a_ledger_is_durable_across_reopen_and_chains_from_genesis() {
        let (temp, ledger, run) = seeded();
        drop(ledger);
        let reopened = CodingRunLedger::open(&temp.path()).unwrap();
        let records = reopened.verify_run(run).unwrap();
        assert_eq!(records.len(), 3);
        assert_eq!(records[0].seq, 0);
        assert_eq!(records[0].previous_hash, GENESIS_HASH);
        assert_eq!(records[1].previous_hash, records[0].current_hash);
        let next = reopened.append(event(run, "run.test", "{}")).unwrap();
        assert_eq!(next.seq, 3);
        assert_eq!(next.previous_hash, records[2].current_hash);
    }

    #[test]
    fn p1a_ledger_sequences_are_per_run() {
        let temp = TempLedger::new();
        let ledger = CodingRunLedger::open(&temp.path()).unwrap();
        let (a, b) = (Uuid::new_v4(), Uuid::new_v4());
        assert_eq!(ledger.append(event(a, "run.test", "{}")).unwrap().seq, 0);
        assert_eq!(ledger.append(event(b, "run.test", "{}")).unwrap().seq, 0);
        assert_eq!(ledger.append(event(a, "run.test", "{}")).unwrap().seq, 1);
    }

    #[test]
    fn p1a_ledger_refuses_duplicate_run_seq_in_the_database() {
        let (temp, _ledger, run) = seeded();
        let conn = temp.raw();
        let err = conn
            .execute(
                "INSERT INTO coding_run_ledger VALUES (?1, 1, 0, 'backend', 'x', '{}', ?2, ?2)",
                params![run.to_string(), GENESIS_HASH],
            )
            .unwrap_err();
        assert!(err.to_string().contains("UNIQUE"), "{err}");
    }

    #[test]
    fn p1a_ledger_update_and_delete_are_refused_by_triggers() {
        let (temp, _ledger, run) = seeded();
        let conn = temp.raw();
        assert!(conn
            .execute(
                "UPDATE coding_run_ledger SET payload = 'x' WHERE run_id = ?1",
                params![run.to_string()],
            )
            .is_err());
        assert!(conn
            .execute(
                "DELETE FROM coding_run_ledger WHERE run_id = ?1",
                params![run.to_string()],
            )
            .is_err());
    }

    #[test]
    fn p1a_ledger_detects_tampering_of_every_hashed_field() {
        let columns = [
            ("timestamp_unix_nanos", "timestamp_unix_nanos + 1"),
            ("actor_kind", "'owner'"),
            ("event_kind", "'run.other'"),
            ("payload", "'{\"i\":9}'"),
        ];
        for (column, value) in columns {
            let (temp, ledger, run) = seeded();
            let conn = temp.raw();
            drop_guards(&conn);
            conn.execute(
                &format!(
                    "UPDATE coding_run_ledger SET {column} = {value} WHERE run_id = ?1 AND seq = 1"
                ),
                params![run.to_string()],
            )
            .unwrap();
            let err = ledger.verify_run(run).unwrap_err();
            assert!(
                matches!(
                    err,
                    LedgerError::Integrity(IntegrityViolation::HashMismatch { seq: 1 })
                ),
                "{column}: {err}"
            );
        }
        // The stored hash itself.
        let (temp, ledger, run) = seeded();
        let conn = temp.raw();
        drop_guards(&conn);
        conn.execute(
            "UPDATE coding_run_ledger SET current_hash = ?2 WHERE run_id = ?1 AND seq = 2",
            params![run.to_string(), "ab".repeat(32)],
        )
        .unwrap();
        assert!(matches!(
            ledger.verify_run(run).unwrap_err(),
            LedgerError::Integrity(IntegrityViolation::HashMismatch { seq: 2 })
        ));
        // Moving an entry to another run changes its hash input (run id).
        let (temp, ledger, run) = seeded();
        let conn = temp.raw();
        drop_guards(&conn);
        let other = Uuid::new_v4();
        conn.execute(
            "UPDATE coding_run_ledger SET run_id = ?2 WHERE run_id = ?1",
            params![run.to_string(), other.to_string()],
        )
        .unwrap();
        assert!(matches!(
            ledger.verify_run(other).unwrap_err(),
            LedgerError::Integrity(IntegrityViolation::HashMismatch { seq: 0 })
        ));
    }

    #[test]
    fn p1a_ledger_detects_previous_hash_tampering() {
        let (temp, ledger, run) = seeded();
        let conn = temp.raw();
        drop_guards(&conn);
        conn.execute(
            "UPDATE coding_run_ledger SET previous_hash = ?2 WHERE run_id = ?1 AND seq = 1",
            params![run.to_string(), "cd".repeat(32)],
        )
        .unwrap();
        assert!(matches!(
            ledger.verify_run(run).unwrap_err(),
            LedgerError::Integrity(IntegrityViolation::PreviousHashMismatch { seq: 1 })
        ));
    }

    #[test]
    fn p1a_ledger_detects_sequence_gaps() {
        let (temp, ledger, run) = seeded();
        let conn = temp.raw();
        drop_guards(&conn);
        conn.execute(
            "DELETE FROM coding_run_ledger WHERE run_id = ?1 AND seq = 1",
            params![run.to_string()],
        )
        .unwrap();
        assert!(matches!(
            ledger.verify_run(run).unwrap_err(),
            LedgerError::Integrity(IntegrityViolation::SequenceGap {
                expected: 1,
                found: 2
            })
        ));
    }

    #[test]
    fn p1a_ledger_refuses_invalid_events_without_recording() {
        let temp = TempLedger::new();
        let ledger = CodingRunLedger::open(&temp.path()).unwrap();
        let run = Uuid::new_v4();
        let mut bad = event(run, "Run Test", "{}");
        assert!(matches!(
            ledger.append(bad),
            Err(LedgerError::InvalidEvent(_))
        ));
        bad = event(Uuid::nil(), "run.test", "{}");
        assert!(ledger.append(bad).is_err());
        assert!(ledger.verify_run(run).unwrap().is_empty());
    }

    #[test]
    fn p1a_ledger_hash_encoding_is_unambiguous() {
        let run = Uuid::new_v4();
        let a = entry_hash(run, 0, 0, "ab", "c", "{}", GENESIS_HASH).unwrap();
        let b = entry_hash(run, 0, 0, "a", "bc", "{}", GENESIS_HASH).unwrap();
        assert_ne!(a, b);
    }
}
