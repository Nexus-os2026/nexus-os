//! Historical-memory storage core (P3-HM-CONTRACT-02 §5, §9).
//!
//! Dormant: no production path calls this module. It holds the §5.1 DDL and
//! triggers, the SQL and plain row methods used by `nexus_kernel::hm`. It
//! returns row data and decides nothing; every authority decision (T₀,
//! ledger checks, member states, evaluated labels) belongs to the kernel.

use rusqlite::types::ValueRef;
use rusqlite::{params, OptionalExtension, Transaction, TransactionBehavior};
use sha2::{Digest, Sha256};

use crate::{NexusDatabase, Result};

/// The five in-scope record tables (§2.1), in a fixed order.
pub const IN_SCOPE_TABLES: [&str; 5] = [
    "agent_memory",
    "task_history",
    "strategy_scores",
    "world_model_entities",
    "world_model_relationships",
];

// ── §5.1 tables ─────────────────────────────────────────────────────────────

pub const DDL_HM_MIGRATION: &str = "CREATE TABLE hm_migration (
  k                 TEXT PRIMARY KEY CHECK (k = 'hm'),
  contract_version  TEXT NOT NULL,            -- 'P3-HM-CONTRACT-02'
  state             TEXT NOT NULL CHECK (state IN ('COMPLETE')),
  t0_wallclock      TEXT NOT NULL,            -- informational only; no decision reads it
  member_count      INTEGER NOT NULL,         -- rows of hm_t0_member, fixed at T0
  member_set_sha256 TEXT NOT NULL             -- over hm_t0_member, §9.5; fixed at T0
)";

pub const DDL_HM_T0_MEMBER: &str =
    "CREATE TABLE hm_t0_member (                    -- immutable T0 membership ledger (M1)
  record_table       TEXT NOT NULL CHECK (record_table IN
                       ('agent_memory','task_history','strategy_scores',
                        'world_model_entities','world_model_relationships')),
  record_id          TEXT NOT NULL,
  agent_id           TEXT NOT NULL,
  t0_content_sha256  TEXT NOT NULL,           -- §5.2 digest of the row at T0
  PRIMARY KEY (record_table, record_id)
) WITHOUT ROWID";

pub const DDL_HM_T0_TOMBSTONE: &str =
    "CREATE TABLE hm_t0_tombstone (                 -- a T0 member that was later deleted (M1)
  record_table  TEXT NOT NULL,
  record_id     TEXT NOT NULL,
  PRIMARY KEY (record_table, record_id),
  FOREIGN KEY (record_table, record_id)
    REFERENCES hm_t0_member (record_table, record_id)
) WITHOUT ROWID";

pub const DDL_HM_AGENT_POLICY: &str = "CREATE TABLE hm_agent_policy (
  agent_id           TEXT PRIMARY KEY,        -- supervisor UUID, canonical lowercase
  local_only         INTEGER NOT NULL CHECK (local_only IN (0,1)),
  policy_generation  INTEGER NOT NULL CHECK (policy_generation >= 1),
  revocation_seq     INTEGER NOT NULL CHECK (revocation_seq >= 0),
  basis              TEXT NOT NULL CHECK (basis IN
                       ('t0_baseline','default_restrictive','owner_confirmed','tightened'))
)";

pub const DDL_HM_POLICY_CHANGE: &str =
    "CREATE TABLE hm_policy_change (                -- authority history, not audit (§5.3)
  agent_id           TEXT NOT NULL,
  policy_generation  INTEGER NOT NULL,         -- the generation this change produced
  local_only         INTEGER NOT NULL CHECK (local_only IN (0,1)),
  basis              TEXT NOT NULL,
  cause              TEXT NOT NULL CHECK (cause IN
                       ('t0_baseline','row_created','governed_tighten','config_tighten',
                        'owner_loosen')),
  PRIMARY KEY (agent_id, policy_generation)
)";

pub const DDL_HM_RECORD_LABEL: &str = "CREATE TABLE hm_record_label (
  record_table       TEXT NOT NULL CHECK (record_table IN
                       ('agent_memory','task_history','strategy_scores',
                        'world_model_entities','world_model_relationships')),
  record_id          TEXT NOT NULL,           -- decimal id or goal UUID
  agent_id           TEXT NOT NULL,
  origin             TEXT NOT NULL CHECK (origin IN
                       ('UNRESTRICTED_ORIGIN','LEGACY_UNKNOWN','LOCAL_ONLY_ORIGIN')),
  source_kind        TEXT NOT NULL,           -- closed enum, §6.5
  content_sha256     TEXT NOT NULL,           -- §5.2, current content
  policy_generation  INTEGER NOT NULL,        -- writer's snapshot (0 for T0 labels)
  PRIMARY KEY (record_table, record_id)
)";

pub const DDL_HM_DERIVATION: &str = "CREATE TABLE hm_derivation (
  child_table   TEXT NOT NULL, child_id  TEXT NOT NULL,
  parent_table  TEXT NOT NULL, parent_id TEXT NOT NULL,
  parent_label_at_derivation TEXT NOT NULL,
  PRIMARY KEY (child_table, child_id, parent_table, parent_id)
)";

pub const DDL_HM_GRANT: &str = "CREATE TABLE hm_grant (
  grant_id         TEXT PRIMARY KEY,          -- 128-bit random, backend-generated
  agent_id         TEXT NOT NULL,
  scope            TEXT NOT NULL CHECK (scope IN ('record','pre_t0_legacy_set')),
  record_table     TEXT,                      -- record scope only
  record_id        TEXT,                      -- record scope only
  bound_content_sha256 TEXT,                  -- record scope only
  state            TEXT NOT NULL CHECK (state IN ('live','revoked')),
  revoked_at_revocation_seq INTEGER            -- agent's revocation_seq after this revocation; NULL while live
)";

pub const DDL_HM_DISPATCH_ADMISSION: &str = "CREATE TABLE hm_dispatch_admission (
  admission_id      TEXT PRIMARY KEY,         -- 128-bit random
  agent_id          TEXT,                     -- NULL for an OwnerDirect (HR/HB) admission
  unit_kind         TEXT NOT NULL,
  route_class       TEXT NOT NULL CHECK (route_class IN ('local','non_local')),
  policy_generation INTEGER NOT NULL,
  revocation_seq    INTEGER NOT NULL,
  record_count      INTEGER NOT NULL,
  grant_dep_count   INTEGER NOT NULL,
  raw_label         TEXT NOT NULL,           -- envelope raw_label (§11.2)
  outcome           TEXT NOT NULL CHECK (outcome IN
                      ('admitted','reply_used','reply_discarded','transport_failed'))
)";

pub const DDL_HM_EVENT: &str =
    "CREATE TABLE hm_event (                        -- content-free audit outbox (§15)
  n        INTEGER PRIMARY KEY AUTOINCREMENT,
  subject  TEXT NOT NULL UNIQUE,               -- the authority row it describes, §5.3
  kind     TEXT NOT NULL,
  agent_id TEXT,
  fields   TEXT NOT NULL                       -- JSON of counts, classes, generations only
)";

/// The §5.1 tables in creation order (the tombstone's foreign key names the
/// ledger, so the ledger comes first).
pub const HM_TABLES: [(&str, &str); 10] = [
    ("hm_migration", DDL_HM_MIGRATION),
    ("hm_t0_member", DDL_HM_T0_MEMBER),
    ("hm_t0_tombstone", DDL_HM_T0_TOMBSTONE),
    ("hm_agent_policy", DDL_HM_AGENT_POLICY),
    ("hm_policy_change", DDL_HM_POLICY_CHANGE),
    ("hm_record_label", DDL_HM_RECORD_LABEL),
    ("hm_derivation", DDL_HM_DERIVATION),
    ("hm_grant", DDL_HM_GRANT),
    ("hm_dispatch_admission", DDL_HM_DISPATCH_ADMISSION),
    ("hm_event", DDL_HM_EVENT),
];

// ── §5.1 triggers ───────────────────────────────────────────────────────────
//
// Each text starts with `CREATE TRIGGER` and has no trailing semicolon, so
// `sqlite_master.sql` stores it byte for byte; check L4 (§9.5) compares the
// stored text with these constants.

pub const TRG_T0_MEMBER_NO_INSERT_AFTER_T0: &str = "CREATE TRIGGER hm_t0_member_no_insert_after_t0
BEFORE INSERT ON hm_t0_member
WHEN EXISTS (SELECT 1 FROM hm_migration)
BEGIN
  SELECT RAISE(ABORT, 'hm_t0_member: ledger is closed after T0');
END";

pub const TRG_T0_MEMBER_NO_UPDATE: &str = "CREATE TRIGGER hm_t0_member_no_update
BEFORE UPDATE ON hm_t0_member
BEGIN
  SELECT RAISE(ABORT, 'hm_t0_member: ledger is immutable');
END";

pub const TRG_T0_MEMBER_NO_DELETE: &str = "CREATE TRIGGER hm_t0_member_no_delete
BEFORE DELETE ON hm_t0_member
BEGIN
  SELECT RAISE(ABORT, 'hm_t0_member: ledger is immutable');
END";

pub const TRG_T0_TOMBSTONE_REQUIRES_ABSENT_ROW: &str =
    "CREATE TRIGGER hm_t0_tombstone_requires_absent_row
BEFORE INSERT ON hm_t0_tombstone
WHEN (NEW.record_table = 'agent_memory'
      AND EXISTS (SELECT 1 FROM agent_memory WHERE id = CAST(NEW.record_id AS INTEGER)))
  OR (NEW.record_table = 'task_history'
      AND EXISTS (SELECT 1 FROM task_history WHERE id = NEW.record_id))
  OR (NEW.record_table = 'strategy_scores'
      AND EXISTS (SELECT 1 FROM strategy_scores WHERE id = CAST(NEW.record_id AS INTEGER)))
  OR (NEW.record_table = 'world_model_entities'
      AND EXISTS (SELECT 1 FROM world_model_entities WHERE id = CAST(NEW.record_id AS INTEGER)))
  OR (NEW.record_table = 'world_model_relationships'
      AND EXISTS (SELECT 1 FROM world_model_relationships WHERE id = CAST(NEW.record_id AS INTEGER)))
BEGIN
  SELECT RAISE(ABORT, 'hm_t0_tombstone: the named row still exists');
END";

pub const TRG_T0_TOMBSTONE_NO_UPDATE: &str = "CREATE TRIGGER hm_t0_tombstone_no_update
BEFORE UPDATE ON hm_t0_tombstone
BEGIN
  SELECT RAISE(ABORT, 'hm_t0_tombstone: tombstones are immutable');
END";

pub const TRG_T0_TOMBSTONE_NO_DELETE: &str = "CREATE TRIGGER hm_t0_tombstone_no_delete
BEFORE DELETE ON hm_t0_tombstone
BEGIN
  SELECT RAISE(ABORT, 'hm_t0_tombstone: tombstones are immutable');
END";

pub const TRG_DEL_AGENT_MEMORY: &str = "CREATE TRIGGER hm_del_agent_memory
AFTER DELETE ON agent_memory
BEGIN
  DELETE FROM hm_record_label
    WHERE record_table = 'agent_memory' AND record_id = CAST(OLD.id AS TEXT);
  DELETE FROM hm_derivation
    WHERE child_table = 'agent_memory' AND child_id = CAST(OLD.id AS TEXT);
  INSERT OR IGNORE INTO hm_t0_tombstone (record_table, record_id)
    SELECT record_table, record_id FROM hm_t0_member
    WHERE record_table = 'agent_memory' AND record_id = CAST(OLD.id AS TEXT);
END";

pub const TRG_DEL_TASK_HISTORY: &str = "CREATE TRIGGER hm_del_task_history
AFTER DELETE ON task_history
BEGIN
  DELETE FROM hm_record_label
    WHERE record_table = 'task_history' AND record_id = CAST(OLD.id AS TEXT);
  DELETE FROM hm_derivation
    WHERE child_table = 'task_history' AND child_id = CAST(OLD.id AS TEXT);
  INSERT OR IGNORE INTO hm_t0_tombstone (record_table, record_id)
    SELECT record_table, record_id FROM hm_t0_member
    WHERE record_table = 'task_history' AND record_id = CAST(OLD.id AS TEXT);
END";

pub const TRG_DEL_STRATEGY_SCORES: &str = "CREATE TRIGGER hm_del_strategy_scores
AFTER DELETE ON strategy_scores
BEGIN
  DELETE FROM hm_record_label
    WHERE record_table = 'strategy_scores' AND record_id = CAST(OLD.id AS TEXT);
  DELETE FROM hm_derivation
    WHERE child_table = 'strategy_scores' AND child_id = CAST(OLD.id AS TEXT);
  INSERT OR IGNORE INTO hm_t0_tombstone (record_table, record_id)
    SELECT record_table, record_id FROM hm_t0_member
    WHERE record_table = 'strategy_scores' AND record_id = CAST(OLD.id AS TEXT);
END";

pub const TRG_DEL_WORLD_MODEL_ENTITIES: &str = "CREATE TRIGGER hm_del_world_model_entities
AFTER DELETE ON world_model_entities
BEGIN
  DELETE FROM hm_record_label
    WHERE record_table = 'world_model_entities' AND record_id = CAST(OLD.id AS TEXT);
  DELETE FROM hm_derivation
    WHERE child_table = 'world_model_entities' AND child_id = CAST(OLD.id AS TEXT);
  INSERT OR IGNORE INTO hm_t0_tombstone (record_table, record_id)
    SELECT record_table, record_id FROM hm_t0_member
    WHERE record_table = 'world_model_entities' AND record_id = CAST(OLD.id AS TEXT);
END";

pub const TRG_DEL_WORLD_MODEL_RELATIONSHIPS: &str =
    "CREATE TRIGGER hm_del_world_model_relationships
AFTER DELETE ON world_model_relationships
BEGIN
  DELETE FROM hm_record_label
    WHERE record_table = 'world_model_relationships' AND record_id = CAST(OLD.id AS TEXT);
  DELETE FROM hm_derivation
    WHERE child_table = 'world_model_relationships' AND child_id = CAST(OLD.id AS TEXT);
  INSERT OR IGNORE INTO hm_t0_tombstone (record_table, record_id)
    SELECT record_table, record_id FROM hm_t0_member
    WHERE record_table = 'world_model_relationships' AND record_id = CAST(OLD.id AS TEXT);
END";

pub const TRG_RECORD_LABEL_NO_LOWERING: &str = "CREATE TRIGGER hm_record_label_no_lowering
BEFORE UPDATE OF origin ON hm_record_label
WHEN (CASE NEW.origin WHEN 'UNRESTRICTED_ORIGIN' THEN 0 WHEN 'LEGACY_UNKNOWN' THEN 1
                      WHEN 'LOCAL_ONLY_ORIGIN' THEN 2 END)
   < (CASE OLD.origin WHEN 'UNRESTRICTED_ORIGIN' THEN 0 WHEN 'LEGACY_UNKNOWN' THEN 1
                      WHEN 'LOCAL_ONLY_ORIGIN' THEN 2 END)
BEGIN
  SELECT RAISE(ABORT, 'hm_record_label: origin cannot be lowered');
END";

pub const TRG_GRANT_NO_UNREVOKE: &str = "CREATE TRIGGER hm_grant_no_unrevoke
BEFORE UPDATE OF state ON hm_grant
WHEN OLD.state = 'revoked' AND NEW.state = 'live'
BEGIN
  SELECT RAISE(ABORT, 'hm_grant: a revoked grant cannot become live');
END";

pub const TRG_GRANT_REVOKED_SEQ_FIXED: &str = "CREATE TRIGGER hm_grant_revoked_seq_fixed
BEFORE UPDATE OF revoked_at_revocation_seq ON hm_grant
WHEN OLD.revoked_at_revocation_seq IS NOT NULL
 AND NEW.revoked_at_revocation_seq IS NOT OLD.revoked_at_revocation_seq
BEGIN
  SELECT RAISE(ABORT, 'hm_grant: revoked_at_revocation_seq is fixed once set');
END";

pub const TRG_AGENT_POLICY_NO_LOWERING: &str = "CREATE TRIGGER hm_agent_policy_no_lowering
BEFORE UPDATE ON hm_agent_policy
WHEN NEW.policy_generation < OLD.policy_generation
  OR NEW.revocation_seq < OLD.revocation_seq
BEGIN
  SELECT RAISE(ABORT, 'hm_agent_policy: generation and revocation sequence never fall');
END";

pub const TRG_POLICY_CHANGE_NO_UPDATE: &str = "CREATE TRIGGER hm_policy_change_no_update
BEFORE UPDATE ON hm_policy_change
BEGIN
  SELECT RAISE(ABORT, 'hm_policy_change: insert-only');
END";

pub const TRG_POLICY_CHANGE_NO_DELETE: &str = "CREATE TRIGGER hm_policy_change_no_delete
BEFORE DELETE ON hm_policy_change
BEGIN
  SELECT RAISE(ABORT, 'hm_policy_change: insert-only');
END";

pub const TRG_AGENT_POLICY_INSERT_REQUIRES_HISTORY: &str =
    "CREATE TRIGGER hm_agent_policy_insert_requires_history
BEFORE INSERT ON hm_agent_policy
WHEN NOT EXISTS (SELECT 1 FROM hm_policy_change
                 WHERE agent_id = NEW.agent_id
                   AND policy_generation = NEW.policy_generation
                   AND local_only = NEW.local_only)
BEGIN
  SELECT RAISE(ABORT, 'hm_agent_policy: write its hm_policy_change row first');
END";

pub const TRG_AGENT_POLICY_UPDATE_REQUIRES_HISTORY: &str =
    "CREATE TRIGGER hm_agent_policy_update_requires_history
BEFORE UPDATE ON hm_agent_policy
WHEN (NEW.local_only IS NOT OLD.local_only
      OR NEW.policy_generation IS NOT OLD.policy_generation)
 AND NOT EXISTS (SELECT 1 FROM hm_policy_change
                 WHERE agent_id = NEW.agent_id
                   AND policy_generation = NEW.policy_generation
                   AND local_only = NEW.local_only)
BEGIN
  SELECT RAISE(ABORT, 'hm_agent_policy: write its hm_policy_change row first');
END";

/// Every §5.1 trigger, in creation order.
pub const HM_TRIGGERS: [(&str, &str); 19] = [
    (
        "hm_t0_member_no_insert_after_t0",
        TRG_T0_MEMBER_NO_INSERT_AFTER_T0,
    ),
    ("hm_t0_member_no_update", TRG_T0_MEMBER_NO_UPDATE),
    ("hm_t0_member_no_delete", TRG_T0_MEMBER_NO_DELETE),
    (
        "hm_t0_tombstone_requires_absent_row",
        TRG_T0_TOMBSTONE_REQUIRES_ABSENT_ROW,
    ),
    ("hm_t0_tombstone_no_update", TRG_T0_TOMBSTONE_NO_UPDATE),
    ("hm_t0_tombstone_no_delete", TRG_T0_TOMBSTONE_NO_DELETE),
    ("hm_del_agent_memory", TRG_DEL_AGENT_MEMORY),
    ("hm_del_task_history", TRG_DEL_TASK_HISTORY),
    ("hm_del_strategy_scores", TRG_DEL_STRATEGY_SCORES),
    ("hm_del_world_model_entities", TRG_DEL_WORLD_MODEL_ENTITIES),
    (
        "hm_del_world_model_relationships",
        TRG_DEL_WORLD_MODEL_RELATIONSHIPS,
    ),
    ("hm_record_label_no_lowering", TRG_RECORD_LABEL_NO_LOWERING),
    ("hm_grant_no_unrevoke", TRG_GRANT_NO_UNREVOKE),
    ("hm_grant_revoked_seq_fixed", TRG_GRANT_REVOKED_SEQ_FIXED),
    ("hm_agent_policy_no_lowering", TRG_AGENT_POLICY_NO_LOWERING),
    ("hm_policy_change_no_update", TRG_POLICY_CHANGE_NO_UPDATE),
    ("hm_policy_change_no_delete", TRG_POLICY_CHANGE_NO_DELETE),
    (
        "hm_agent_policy_insert_requires_history",
        TRG_AGENT_POLICY_INSERT_REQUIRES_HISTORY,
    ),
    (
        "hm_agent_policy_update_requires_history",
        TRG_AGENT_POLICY_UPDATE_REQUIRES_HISTORY,
    ),
];

/// The triggers check L4 (§9.5) compares against `sqlite_master`: the
/// ledger, tombstone and delete triggers.
pub const L4_TRIGGERS: [(&str, &str); 11] = [
    (
        "hm_t0_member_no_insert_after_t0",
        TRG_T0_MEMBER_NO_INSERT_AFTER_T0,
    ),
    ("hm_t0_member_no_update", TRG_T0_MEMBER_NO_UPDATE),
    ("hm_t0_member_no_delete", TRG_T0_MEMBER_NO_DELETE),
    (
        "hm_t0_tombstone_requires_absent_row",
        TRG_T0_TOMBSTONE_REQUIRES_ABSENT_ROW,
    ),
    ("hm_t0_tombstone_no_update", TRG_T0_TOMBSTONE_NO_UPDATE),
    ("hm_t0_tombstone_no_delete", TRG_T0_TOMBSTONE_NO_DELETE),
    ("hm_del_agent_memory", TRG_DEL_AGENT_MEMORY),
    ("hm_del_task_history", TRG_DEL_TASK_HISTORY),
    ("hm_del_strategy_scores", TRG_DEL_STRATEGY_SCORES),
    ("hm_del_world_model_entities", TRG_DEL_WORLD_MODEL_ENTITIES),
    (
        "hm_del_world_model_relationships",
        TRG_DEL_WORLD_MODEL_RELATIONSHIPS,
    ),
];

// ── §5.2 content digest ─────────────────────────────────────────────────────

/// Length written for a NULL value: 0xFFFFFFFFFFFFFFFF (§5.2).
pub const NULL_LENGTH_MARKER: u64 = u64::MAX;

/// Row digests start with this tag followed by the table name (§5.2).
pub const ROW_DIGEST_TAG_PREFIX: &str = "nexus-hm-v1/";

/// The member-set digest starts with this tag (§9.5).
pub const MEMBER_SET_DIGEST_TAG: &str = "nexus-hm-v1/t0-members";

/// The columns the §5.2 digest covers for each in-scope table, in order.
/// Every other column is mutable bookkeeping and is excluded.
pub fn digest_columns(table: &str) -> Option<&'static [&'static str]> {
    match table {
        "agent_memory" => Some(&["agent_id", "memory_type", "key", "value_json", "created_at"]),
        "task_history" => Some(&[
            "id",
            "agent_id",
            "goal",
            "steps_json",
            "result_json",
            "started_at",
        ]),
        "strategy_scores" => Some(&["agent_id", "strategy_hash", "goal_type"]),
        "world_model_entities" => Some(&["agent_id", "entity_json", "created_at"]),
        "world_model_relationships" => Some(&["agent_id", "relationship_json", "created_at"]),
        _ => None,
    }
}

/// Appends one value: 8-byte big-endian length, then the bytes; NULL is the
/// length marker alone.
fn put_field(h: &mut Sha256, value: Option<&[u8]>) {
    match value {
        Some(bytes) => {
            h.update((bytes.len() as u64).to_be_bytes());
            h.update(bytes);
        }
        None => h.update(NULL_LENGTH_MARKER.to_be_bytes()),
    }
}

/// §5.2 digest of one row: SHA-256 over `"nexus-hm-v1/" + table`, then each
/// covered column value length-prefixed. Lowercase hex.
pub fn content_digest(table: &str, values: &[Option<&[u8]>]) -> String {
    let mut h = Sha256::new();
    h.update(ROW_DIGEST_TAG_PREFIX.as_bytes());
    h.update(table.as_bytes());
    for value in values {
        put_field(&mut h, *value);
    }
    hex::encode(h.finalize())
}

/// One `hm_t0_member` row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberRow {
    pub record_table: String,
    pub record_id: String,
    pub agent_id: String,
    pub t0_content_sha256: String,
}

/// §9.5 member-set digest: SHA-256 over `"nexus-hm-v1/t0-members"`, then
/// every row sorted by `(record_table, record_id)` (byte order), each row as
/// its four fields length-prefixed as in §5.2. Lowercase hex.
pub fn member_set_digest(rows: &[MemberRow]) -> String {
    let mut sorted: Vec<&MemberRow> = rows.iter().collect();
    sorted.sort_by(|a, b| {
        (a.record_table.as_bytes(), a.record_id.as_bytes())
            .cmp(&(b.record_table.as_bytes(), b.record_id.as_bytes()))
    });
    let mut h = Sha256::new();
    h.update(MEMBER_SET_DIGEST_TAG.as_bytes());
    for r in sorted {
        put_field(&mut h, Some(r.record_table.as_bytes()));
        put_field(&mut h, Some(r.record_id.as_bytes()));
        put_field(&mut h, Some(r.agent_id.as_bytes()));
        put_field(&mut h, Some(r.t0_content_sha256.as_bytes()));
    }
    hex::encode(h.finalize())
}

/// One in-scope row as the digest sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScannedRow {
    /// Decimal id, or the `task_history` goal UUID as stored.
    pub record_id: String,
    pub agent_id: String,
    pub content_sha256: String,
}

fn invalid_type(idx: usize, name: &str, v: ValueRef<'_>) -> rusqlite::Error {
    rusqlite::Error::InvalidColumnType(idx, name.to_string(), v.data_type())
}

/// Reads `id, agent_id, <digest columns>` into a `ScannedRow`.
/// The id is an INTEGER (rendered in decimal) or TEXT; `agent_id` is TEXT.
/// Digest columns are TEXT-affinity: a TEXT or BLOB value contributes its
/// bytes, NULL the marker; any other storage class is an error.
fn scanned_row(
    table: &str,
    cols: &[&str],
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<ScannedRow> {
    let record_id = match row.get_ref(0)? {
        ValueRef::Integer(i) => i.to_string(),
        ValueRef::Text(t) => String::from_utf8_lossy(t).into_owned(),
        v => return Err(invalid_type(0, "id", v)),
    };
    let agent_id = match row.get_ref(1)? {
        ValueRef::Text(t) => String::from_utf8_lossy(t).into_owned(),
        v => return Err(invalid_type(1, "agent_id", v)),
    };
    let mut values: Vec<Option<&[u8]>> = Vec::with_capacity(cols.len());
    for (i, name) in cols.iter().enumerate() {
        values.push(match row.get_ref(i + 2)? {
            ValueRef::Null => None,
            ValueRef::Text(b) | ValueRef::Blob(b) => Some(b),
            v => return Err(invalid_type(i + 2, name, v)),
        });
    }
    Ok(ScannedRow {
        record_id,
        agent_id,
        content_sha256: content_digest(table, &values),
    })
}

/// One `hm_record_label` row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelRow {
    pub record_table: String,
    pub record_id: String,
    pub agent_id: String,
    pub origin: String,
    pub source_kind: String,
    pub content_sha256: String,
    pub policy_generation: i64,
}

/// One `hm_agent_policy` row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyRow {
    pub agent_id: String,
    pub local_only: i64,
    pub policy_generation: i64,
    pub revocation_seq: i64,
    pub basis: String,
}

/// One `hm_agent_policy` row as stored. A field is `None` when its value
/// does not have the declared storage class; the kernel decides what a
/// malformed row means.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyRowRaw {
    pub agent_id: String,
    pub local_only: Option<i64>,
    pub policy_generation: Option<i64>,
    pub revocation_seq: Option<i64>,
    pub basis: Option<String>,
}

fn int_or_none(v: ValueRef<'_>) -> Option<i64> {
    match v {
        ValueRef::Integer(i) => Some(i),
        _ => None,
    }
}

fn text_or_none(v: ValueRef<'_>) -> Option<String> {
    match v {
        ValueRef::Text(t) => Some(String::from_utf8_lossy(t).into_owned()),
        _ => None,
    }
}

/// A new `agent_memory` row: `access_count` starts at 0, `last_accessed`
/// equals `created_at`, `expires_at` is NULL.
#[derive(Debug, Clone, PartialEq)]
pub struct NewMemoryRow {
    pub agent_id: String,
    pub memory_type: String,
    pub key: String,
    pub value_json: String,
    pub relevance_score: f64,
    pub created_at: String,
}

/// The content columns of one `agent_memory` row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryContent {
    pub agent_id: String,
    pub memory_type: String,
    pub key: String,
    pub value_json: String,
}

/// A new `task_history` row (fuel and timing columns take their defaults).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewTaskRow {
    pub id: String,
    pub agent_id: String,
    pub goal: String,
    pub status: String,
    pub steps_json: String,
    pub result_json: Option<String>,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub success: bool,
}

/// Bookkeeping values of one `task_history` row; `None` leaves a column
/// unchanged.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct TaskBookkeeping<'a> {
    pub status: Option<&'a str>,
    pub fuel_consumed: Option<f64>,
    pub fuel_budget: Option<f64>,
    pub completed_at: Option<&'a str>,
    pub success: Option<bool>,
}

/// One contribution to a `strategy_scores` aggregate.
#[derive(Debug, Clone, PartialEq)]
pub struct StrategyContribution {
    pub agent_id: String,
    pub strategy_hash: String,
    pub goal_type: String,
    pub success: bool,
    pub fuel: f64,
    pub duration_secs: f64,
    pub updated_at: String,
}

/// The label columns a recall walk reads with each row. A column is `None`
/// when it is not TEXT.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelBinding {
    pub agent_id: Option<String>,
    pub origin: Option<String>,
    pub source_kind: Option<String>,
    pub content_sha256: Option<String>,
}

/// One row met by a recall walk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecallRow {
    /// Identity, owner and §5.2 digest of the row as stored now.
    pub row: ScannedRow,
    /// The digest-covered column values, in `digest_columns` order.
    pub values: Vec<Option<String>>,
    /// Its label row, if one exists.
    pub label: Option<LabelBinding>,
}

/// The narrowing conditions of a recall walk (mission P3-C11-HM-M3 R8). Each
/// is applied in the query, before the ORDER BY, so a row they exclude can
/// never take the place of a row they select. Column names are checked
/// against the table's §5.2 digest columns; values are bound parameters.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RecallFilter<'a> {
    /// `column = value` on a digest-covered column other than `agent_id`.
    pub column_eq: Option<(&'a str, &'a str)>,
    /// The row's label row has this `source_kind` (§6.6: a role is read
    /// from the label, never from a substring of the content).
    pub source_kind: Option<&'a str>,
    /// `instr(lower(column), needle) > 0` on a digest-covered column other
    /// than `agent_id`; the needle is given lowercased.
    pub contains: Option<(&'a str, &'a str)>,
    /// Newest id first instead of the table's recall order.
    pub newest_first: bool,
}

/// One `hm_dispatch_admission` row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmissionRow {
    pub admission_id: String,
    pub agent_id: Option<String>,
    pub unit_kind: String,
    pub route_class: String,
    pub policy_generation: i64,
    pub revocation_seq: i64,
    pub record_count: i64,
    pub grant_dep_count: i64,
    pub raw_label: String,
    pub outcome: String,
}

/// One `hm_t0_tombstone` row with the agent its ledger row names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TombstoneRow {
    pub record_table: String,
    pub record_id: String,
    pub agent_id: Option<String>,
}

/// The order of a recall over each in-scope table: most relevant first,
/// newest id breaking ties.
fn recall_order(table: &str) -> &'static str {
    match table {
        "agent_memory" => "m.relevance_score DESC, m.id DESC",
        "task_history" => "m.started_at DESC, m.id DESC",
        "strategy_scores" => "m.composite_score DESC, m.id DESC",
        _ => "m.created_at DESC, m.id DESC",
    }
}

/// One `hm_policy_change` row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyChangeRow {
    pub agent_id: String,
    pub policy_generation: i64,
    pub local_only: i64,
    pub basis: String,
    pub cause: String,
}

/// One `hm_migration` row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationRow {
    pub contract_version: String,
    pub state: String,
    pub t0_wallclock: String,
    pub member_count: i64,
    pub member_set_sha256: String,
}

fn member_from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<MemberRow> {
    Ok(MemberRow {
        record_table: r.get(0)?,
        record_id: r.get(1)?,
        agent_id: r.get(2)?,
        t0_content_sha256: r.get(3)?,
    })
}

fn label_from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<LabelRow> {
    Ok(LabelRow {
        record_table: r.get(0)?,
        record_id: r.get(1)?,
        agent_id: r.get(2)?,
        origin: r.get(3)?,
        source_kind: r.get(4)?,
        content_sha256: r.get(5)?,
        policy_generation: r.get(6)?,
    })
}

fn not_in_scope(table: &str) -> crate::PersistenceError {
    crate::PersistenceError::NotFound(format!("not an in-scope table: {table}"))
}

// ── Store handle ────────────────────────────────────────────────────────────

/// Row-level access to the historical-memory tables. Obtained only through
/// `NexusDatabase::hm`. Its single legitimate caller is `nexus_kernel::hm`
/// (§11.0); it carries no label or route-class decision.
#[doc(hidden)]
pub struct HmStore<'a> {
    db: &'a NexusDatabase,
}

impl<'a> HmStore<'a> {
    pub(crate) fn new(db: &'a NexusDatabase) -> Self {
        Self { db }
    }

    /// Runs `f` inside one `BEGIN IMMEDIATE` transaction on this database's
    /// connection. Commits only if `f` returns `Ok`; otherwise the
    /// transaction is rolled back when it is dropped.
    pub fn immediate<R>(&self, f: impl FnOnce(&HmTx<'_>) -> Result<R>) -> Result<R> {
        let mut conn = self.db.conn.lock().unwrap_or_else(|p| p.into_inner());
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let out = f(&HmTx { tx: &tx })?;
        tx.commit()?;
        Ok(out)
    }

    /// Runs `f` inside one DEFERRED transaction: a read transaction whose
    /// snapshot starts at its first read. It takes no write lock until a
    /// write; the kernel uses it only to read.
    pub fn read<R>(&self, f: impl FnOnce(&HmTx<'_>) -> Result<R>) -> Result<R> {
        let mut conn = self.db.conn.lock().unwrap_or_else(|p| p.into_inner());
        let tx = conn.transaction_with_behavior(TransactionBehavior::Deferred)?;
        let out = f(&HmTx { tx: &tx })?;
        tx.commit()?;
        Ok(out)
    }
}

/// Plain row methods inside one `HmStore::immediate` transaction.
#[doc(hidden)]
pub struct HmTx<'t> {
    tx: &'t Transaction<'t>,
}

impl HmTx<'_> {
    /// Whether a table of this name exists.
    pub fn table_exists(&self, name: &str) -> Result<bool> {
        let n: i64 = self.tx.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
            params![name],
            |r| r.get(0),
        )?;
        Ok(n > 0)
    }

    /// Creates the §5.1 tables, then the §5.1 triggers.
    pub fn create_schema(&self) -> Result<()> {
        for (_, ddl) in HM_TABLES {
            self.tx.execute_batch(ddl)?;
        }
        for (_, ddl) in HM_TRIGGERS {
            self.tx.execute_batch(ddl)?;
        }
        Ok(())
    }

    /// Every row of one in-scope table, ordered by id, with its §5.2 digest.
    pub fn scan_table(&self, table: &str) -> Result<Vec<ScannedRow>> {
        let cols = digest_columns(table).ok_or_else(|| not_in_scope(table))?;
        let sql = format!(
            "SELECT id, agent_id, {} FROM {table} ORDER BY id",
            cols.join(", ")
        );
        let mut stmt = self.tx.prepare(&sql)?;
        let rows = stmt.query_map([], |r| scanned_row(table, cols, r))?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// The current row with this identity, if present. An integer-keyed
    /// table matches only a canonical decimal `record_id`.
    pub fn current_row(&self, table: &str, record_id: &str) -> Result<Option<ScannedRow>> {
        let cols = digest_columns(table).ok_or_else(|| not_in_scope(table))?;
        let sql = format!(
            "SELECT id, agent_id, {} FROM {table} WHERE id = ?1",
            cols.join(", ")
        );
        let mut stmt = self.tx.prepare(&sql)?;
        let row = if table == "task_history" {
            stmt.query_row(params![record_id], |r| scanned_row(table, cols, r))
                .optional()?
        } else {
            match record_id.parse::<i64>() {
                Ok(n) if n.to_string() == record_id => stmt
                    .query_row(params![n], |r| scanned_row(table, cols, r))
                    .optional()?,
                _ => None,
            }
        };
        Ok(row)
    }

    pub fn insert_member(&self, m: &MemberRow) -> Result<()> {
        self.tx.execute(
            "INSERT INTO hm_t0_member (record_table, record_id, agent_id, t0_content_sha256)
             VALUES (?1, ?2, ?3, ?4)",
            params![m.record_table, m.record_id, m.agent_id, m.t0_content_sha256],
        )?;
        Ok(())
    }

    /// Every ledger row, ordered by `(record_table, record_id)`.
    pub fn ledger_rows(&self) -> Result<Vec<MemberRow>> {
        let mut stmt = self.tx.prepare(
            "SELECT record_table, record_id, agent_id, t0_content_sha256
             FROM hm_t0_member ORDER BY record_table, record_id",
        )?;
        let rows = stmt.query_map([], member_from_row)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn ledger_count(&self) -> Result<i64> {
        Ok(self
            .tx
            .query_row("SELECT COUNT(*) FROM hm_t0_member", [], |r| r.get(0))?)
    }

    pub fn ledger_member(&self, table: &str, record_id: &str) -> Result<Option<MemberRow>> {
        Ok(self
            .tx
            .query_row(
                "SELECT record_table, record_id, agent_id, t0_content_sha256
                 FROM hm_t0_member WHERE record_table = ?1 AND record_id = ?2",
                params![table, record_id],
                member_from_row,
            )
            .optional()?)
    }

    pub fn is_tombstoned(&self, table: &str, record_id: &str) -> Result<bool> {
        let n: i64 = self.tx.query_row(
            "SELECT COUNT(*) FROM hm_t0_tombstone WHERE record_table = ?1 AND record_id = ?2",
            params![table, record_id],
            |r| r.get(0),
        )?;
        Ok(n > 0)
    }

    /// Tombstones that name no ledger row (counted by query; independent of
    /// the foreign-key pragma).
    pub fn orphan_tombstone_count(&self) -> Result<i64> {
        Ok(self.tx.query_row(
            "SELECT COUNT(*) FROM hm_t0_tombstone t
             WHERE NOT EXISTS (SELECT 1 FROM hm_t0_member m
                               WHERE m.record_table = t.record_table
                                 AND m.record_id = t.record_id)",
            [],
            |r| r.get(0),
        )?)
    }

    pub fn insert_label(&self, l: &LabelRow) -> Result<()> {
        self.tx.execute(
            "INSERT INTO hm_record_label
               (record_table, record_id, agent_id, origin, source_kind, content_sha256, policy_generation)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                l.record_table,
                l.record_id,
                l.agent_id,
                l.origin,
                l.source_kind,
                l.content_sha256,
                l.policy_generation
            ],
        )?;
        Ok(())
    }

    pub fn label_row(&self, table: &str, record_id: &str) -> Result<Option<LabelRow>> {
        Ok(self
            .tx
            .query_row(
                "SELECT record_table, record_id, agent_id, origin, source_kind, content_sha256,
                        policy_generation
                 FROM hm_record_label WHERE record_table = ?1 AND record_id = ?2",
                params![table, record_id],
                label_from_row,
            )
            .optional()?)
    }

    /// The number of label rows of `agent_id`'s records of `table` with
    /// `source_kind` (content-free).
    pub fn count_labels_of_kind(
        &self,
        table: &str,
        agent_id: &str,
        source_kind: &str,
    ) -> Result<i64> {
        Ok(self.tx.query_row(
            "SELECT COUNT(*) FROM hm_record_label
             WHERE record_table = ?1 AND agent_id = ?2 AND source_kind = ?3",
            params![table, agent_id, source_kind],
            |r| r.get(0),
        )?)
    }

    /// Every label row, ordered by `(record_table, record_id)`.
    pub fn label_rows(&self) -> Result<Vec<LabelRow>> {
        let mut stmt = self.tx.prepare(
            "SELECT record_table, record_id, agent_id, origin, source_kind, content_sha256,
                    policy_generation
             FROM hm_record_label ORDER BY record_table, record_id",
        )?;
        let rows = stmt.query_map([], label_from_row)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn insert_policy_change(&self, c: &PolicyChangeRow) -> Result<()> {
        self.tx.execute(
            "INSERT INTO hm_policy_change (agent_id, policy_generation, local_only, basis, cause)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                c.agent_id,
                c.policy_generation,
                c.local_only,
                c.basis,
                c.cause
            ],
        )?;
        Ok(())
    }

    pub fn insert_policy(&self, p: &PolicyRow) -> Result<()> {
        self.tx.execute(
            "INSERT INTO hm_agent_policy (agent_id, local_only, policy_generation, revocation_seq, basis)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                p.agent_id,
                p.local_only,
                p.policy_generation,
                p.revocation_seq,
                p.basis
            ],
        )?;
        Ok(())
    }

    /// The `hm_agent_policy` row of one agent, as stored.
    pub fn policy_row_raw(&self, agent_id: &str) -> Result<Option<PolicyRowRaw>> {
        Ok(self
            .tx
            .query_row(
                "SELECT agent_id, local_only, policy_generation, revocation_seq, basis
                 FROM hm_agent_policy WHERE agent_id = ?1",
                params![agent_id],
                |r| {
                    Ok(PolicyRowRaw {
                        agent_id: r.get(0)?,
                        local_only: int_or_none(r.get_ref(1)?),
                        policy_generation: int_or_none(r.get_ref(2)?),
                        revocation_seq: int_or_none(r.get_ref(3)?),
                        basis: text_or_none(r.get_ref(4)?),
                    })
                },
            )
            .optional()?)
    }

    /// Every `hm_agent_policy` row, as stored, ordered by `agent_id`.
    pub fn policy_rows_raw(&self) -> Result<Vec<PolicyRowRaw>> {
        let mut stmt = self.tx.prepare(
            "SELECT agent_id, local_only, policy_generation, revocation_seq, basis
             FROM hm_agent_policy ORDER BY agent_id",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(PolicyRowRaw {
                agent_id: r.get(0)?,
                local_only: int_or_none(r.get_ref(1)?),
                policy_generation: int_or_none(r.get_ref(2)?),
                revocation_seq: int_or_none(r.get_ref(3)?),
                basis: text_or_none(r.get_ref(4)?),
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// Updates one `hm_agent_policy` row. Returns the number of rows changed.
    pub fn update_policy(&self, p: &PolicyRow) -> Result<usize> {
        Ok(self.tx.execute(
            "UPDATE hm_agent_policy
             SET local_only = ?2, policy_generation = ?3, revocation_seq = ?4, basis = ?5
             WHERE agent_id = ?1",
            params![
                p.agent_id,
                p.local_only,
                p.policy_generation,
                p.revocation_seq,
                p.basis
            ],
        )?)
    }

    /// Inserts one `agent_memory` row and returns its id.
    pub fn insert_agent_memory(&self, m: &NewMemoryRow) -> Result<i64> {
        self.tx.execute(
            "INSERT INTO agent_memory
               (agent_id, memory_type, key, value_json, relevance_score, access_count,
                created_at, last_accessed, expires_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6, ?6, NULL)",
            params![
                m.agent_id,
                m.memory_type,
                m.key,
                m.value_json,
                m.relevance_score,
                m.created_at
            ],
        )?;
        Ok(self.tx.last_insert_rowid())
    }

    /// The content columns of one `agent_memory` row, if present.
    pub fn memory_content(&self, id: i64) -> Result<Option<MemoryContent>> {
        Ok(self
            .tx
            .query_row(
                "SELECT agent_id, memory_type, key, value_json FROM agent_memory WHERE id = ?1",
                params![id],
                |r| {
                    Ok(MemoryContent {
                        agent_id: r.get(0)?,
                        memory_type: r.get(1)?,
                        key: r.get(2)?,
                        value_json: r.get(3)?,
                    })
                },
            )
            .optional()?)
    }

    pub fn insert_task(&self, t: &NewTaskRow) -> Result<()> {
        self.tx.execute(
            "INSERT INTO task_history
               (id, agent_id, goal, status, steps_json, result_json, started_at, completed_at,
                success)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                t.id,
                t.agent_id,
                t.goal,
                t.status,
                t.steps_json,
                t.result_json,
                t.started_at,
                t.completed_at,
                t.success
            ],
        )?;
        Ok(())
    }

    /// Updates the progress columns of one `task_history` row (`goal` and
    /// `started_at` are unchanged). Returns the number of rows changed.
    pub fn update_task_progress(
        &self,
        id: &str,
        status: &str,
        steps_json: &str,
        result_json: Option<&str>,
        completed_at: Option<&str>,
        success: bool,
    ) -> Result<usize> {
        Ok(self.tx.execute(
            "UPDATE task_history
             SET status = ?2, steps_json = ?3, result_json = ?4, completed_at = ?5, success = ?6
             WHERE id = ?1",
            params![id, status, steps_json, result_json, completed_at, success],
        )?)
    }

    /// The id of the `strategy_scores` row of `(agent_id, strategy_hash)`.
    pub fn strategy_id(&self, agent_id: &str, strategy_hash: &str) -> Result<Option<i64>> {
        Ok(self
            .tx
            .query_row(
                "SELECT id FROM strategy_scores WHERE agent_id = ?1 AND strategy_hash = ?2",
                params![agent_id, strategy_hash],
                |r| r.get(0),
            )
            .optional()?)
    }

    /// Inserts the first contribution of an aggregate and returns its id.
    pub fn insert_strategy(&self, c: &StrategyContribution) -> Result<i64> {
        self.tx.execute(
            "INSERT INTO strategy_scores
               (agent_id, strategy_hash, goal_type, uses, successes, total_fuel,
                total_duration_secs, composite_score, updated_at)
             VALUES (?1, ?2, ?3, 1, ?4, ?5, ?6, 0.0, ?7)",
            params![
                c.agent_id,
                c.strategy_hash,
                c.goal_type,
                i64::from(c.success),
                c.fuel,
                c.duration_secs,
                c.updated_at
            ],
        )?;
        Ok(self.tx.last_insert_rowid())
    }

    /// Adds one contribution to an aggregate; `goal_type` is overwritten.
    pub fn add_strategy_contribution(&self, id: i64, c: &StrategyContribution) -> Result<usize> {
        Ok(self.tx.execute(
            "UPDATE strategy_scores
             SET goal_type = ?2, uses = uses + 1, successes = successes + ?3,
                 total_fuel = total_fuel + ?4, total_duration_secs = total_duration_secs + ?5,
                 updated_at = ?6
             WHERE id = ?1",
            params![
                id,
                c.goal_type,
                i64::from(c.success),
                c.fuel,
                c.duration_secs,
                c.updated_at
            ],
        )?)
    }

    /// Sets `origin` and `content_sha256` of one label row. The §5.1
    /// trigger rejects a lowering. Returns the number of rows changed.
    pub fn update_label(
        &self,
        table: &str,
        record_id: &str,
        origin: &str,
        content_sha256: &str,
    ) -> Result<usize> {
        Ok(self.tx.execute(
            "UPDATE hm_record_label SET origin = ?3, content_sha256 = ?4
             WHERE record_table = ?1 AND record_id = ?2",
            params![table, record_id, origin, content_sha256],
        )?)
    }

    /// Inserts one derivation edge; an existing identical edge is kept.
    pub fn insert_derivation(
        &self,
        child_table: &str,
        child_id: &str,
        parent_table: &str,
        parent_id: &str,
        parent_label: &str,
    ) -> Result<()> {
        self.tx.execute(
            "INSERT OR IGNORE INTO hm_derivation
               (child_table, child_id, parent_table, parent_id, parent_label_at_derivation)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![child_table, child_id, parent_table, parent_id, parent_label],
        )?;
        Ok(())
    }

    /// Walks one agent's rows of an in-scope table in recall order, with
    /// each row's label row (LEFT JOIN). With `origin`, only rows whose
    /// label row has that origin are selected, in the query, before the
    /// ORDER BY. `visit` returns false to stop the walk.
    pub fn recall_walk(
        &self,
        table: &str,
        agent_id: &str,
        origin: Option<&str>,
        visit: &mut dyn FnMut(RecallRow) -> bool,
    ) -> Result<()> {
        let cols = digest_columns(table).ok_or_else(|| not_in_scope(table))?;
        let selected = cols
            .iter()
            .map(|c| format!("m.{c}"))
            .collect::<Vec<_>>()
            .join(", ");
        let filter = if origin.is_some() {
            " AND l.origin = ?2"
        } else {
            ""
        };
        let sql = format!(
            "SELECT m.id, m.agent_id, {selected},
                    l.agent_id, l.origin, l.source_kind, l.content_sha256, l.record_id
             FROM {table} m
             LEFT JOIN hm_record_label l
               ON l.record_table = '{table}' AND l.record_id = CAST(m.id AS TEXT)
             WHERE m.agent_id = ?1{filter}
             ORDER BY {}",
            recall_order(table)
        );
        let mut stmt = self.tx.prepare(&sql)?;
        let mut rows = match origin {
            Some(o) => stmt.query(params![agent_id, o])?,
            None => stmt.query(params![agent_id])?,
        };
        let n = cols.len() + 2;
        while let Some(r) = rows.next()? {
            let row = scanned_row(table, cols, r)?;
            let mut values = Vec::with_capacity(cols.len());
            for i in 0..cols.len() {
                values.push(match r.get_ref(i + 2)? {
                    ValueRef::Text(b) | ValueRef::Blob(b) => {
                        Some(String::from_utf8_lossy(b).into_owned())
                    }
                    _ => None,
                });
            }
            let label = match r.get_ref(n + 4)? {
                ValueRef::Null => None,
                _ => Some(LabelBinding {
                    agent_id: text_or_none(r.get_ref(n)?),
                    origin: text_or_none(r.get_ref(n + 1)?),
                    source_kind: text_or_none(r.get_ref(n + 2)?),
                    content_sha256: text_or_none(r.get_ref(n + 3)?),
                }),
            };
            if !visit(RecallRow { row, values, label }) {
                break;
            }
        }
        Ok(())
    }

    /// For one agent's rows of an in-scope table: the number whose label
    /// row is missing or has an origin other than `origin`, and the number
    /// with no label row.
    pub fn recall_withheld_counts(
        &self,
        table: &str,
        agent_id: &str,
        origin: &str,
    ) -> Result<(i64, i64)> {
        digest_columns(table).ok_or_else(|| not_in_scope(table))?;
        let sql = format!(
            "SELECT COUNT(*), COALESCE(SUM(l.record_id IS NULL), 0)
             FROM {table} m
             LEFT JOIN hm_record_label l
               ON l.record_table = '{table}' AND l.record_id = CAST(m.id AS TEXT)
             WHERE m.agent_id = ?1 AND (l.record_id IS NULL OR l.origin IS NOT ?2)"
        );
        Ok(self.tx.query_row(&sql, params![agent_id, origin], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })?)
    }

    /// The SQL conditions and bound values of `filter` for `table`; the
    /// first placeholder is `?first_param`.
    fn filter_sql(
        table: &str,
        filter: &RecallFilter<'_>,
        first_param: usize,
    ) -> Result<(String, Vec<String>)> {
        let cols = digest_columns(table).ok_or_else(|| not_in_scope(table))?;
        let column = |name: &str| -> Result<String> {
            if name != "agent_id" && cols.contains(&name) {
                Ok(name.to_string())
            } else {
                Err(crate::PersistenceError::NotFound(format!(
                    "not a recall column of {table}: {name}"
                )))
            }
        };
        let mut sql = String::new();
        let mut values = Vec::new();
        let mut next = first_param;
        if let Some((name, value)) = filter.column_eq {
            sql.push_str(&format!(" AND m.{} = ?{next}", column(name)?));
            values.push(value.to_string());
            next += 1;
        }
        if let Some(kind) = filter.source_kind {
            sql.push_str(&format!(" AND l.source_kind = ?{next}"));
            values.push(kind.to_string());
            next += 1;
        }
        if let Some((name, needle)) = filter.contains {
            sql.push_str(&format!(
                " AND instr(lower(m.{}), ?{next}) > 0",
                column(name)?
            ));
            values.push(needle.to_string());
        }
        Ok((sql, values))
    }

    /// `recall_walk` with the narrowing conditions of `filter`, all in the
    /// query before the ORDER BY. With `origin`, only rows whose label row
    /// has that origin are selected. `visit` returns false to stop.
    pub fn recall_walk_filtered(
        &self,
        table: &str,
        agent_id: &str,
        origin: Option<&str>,
        filter: &RecallFilter<'_>,
        visit: &mut dyn FnMut(RecallRow) -> bool,
    ) -> Result<()> {
        let cols = digest_columns(table).ok_or_else(|| not_in_scope(table))?;
        let selected = cols
            .iter()
            .map(|c| format!("m.{c}"))
            .collect::<Vec<_>>()
            .join(", ");
        let origin_sql = if origin.is_some() {
            " AND l.origin = ?2"
        } else {
            ""
        };
        let first = if origin.is_some() { 3 } else { 2 };
        let (filter_sql, filter_values) = Self::filter_sql(table, filter, first)?;
        let order = if filter.newest_first {
            "m.id DESC"
        } else {
            recall_order(table)
        };
        let sql = format!(
            "SELECT m.id, m.agent_id, {selected},
                    l.agent_id, l.origin, l.source_kind, l.content_sha256, l.record_id
             FROM {table} m
             LEFT JOIN hm_record_label l
               ON l.record_table = '{table}' AND l.record_id = CAST(m.id AS TEXT)
             WHERE m.agent_id = ?1{origin_sql}{filter_sql}
             ORDER BY {order}"
        );
        let mut params_list: Vec<&dyn rusqlite::ToSql> = vec![&agent_id];
        if let Some(o) = &origin {
            params_list.push(o);
        }
        for v in &filter_values {
            params_list.push(v);
        }
        let mut stmt = self.tx.prepare(&sql)?;
        let mut rows = stmt.query(params_list.as_slice())?;
        let n = cols.len() + 2;
        while let Some(r) = rows.next()? {
            let row = scanned_row(table, cols, r)?;
            let mut values = Vec::with_capacity(cols.len());
            for i in 0..cols.len() {
                values.push(match r.get_ref(i + 2)? {
                    ValueRef::Text(b) | ValueRef::Blob(b) => {
                        Some(String::from_utf8_lossy(b).into_owned())
                    }
                    _ => None,
                });
            }
            let label = match r.get_ref(n + 4)? {
                ValueRef::Null => None,
                _ => Some(LabelBinding {
                    agent_id: text_or_none(r.get_ref(n)?),
                    origin: text_or_none(r.get_ref(n + 1)?),
                    source_kind: text_or_none(r.get_ref(n + 2)?),
                    content_sha256: text_or_none(r.get_ref(n + 3)?),
                }),
            };
            if !visit(RecallRow { row, values, label }) {
                break;
            }
        }
        Ok(())
    }

    /// `recall_withheld_counts` over the rows `filter` selects: the number
    /// whose label row is missing or has an origin other than `origin`,
    /// and the number with no label row.
    pub fn recall_withheld_counts_filtered(
        &self,
        table: &str,
        agent_id: &str,
        origin: &str,
        filter: &RecallFilter<'_>,
    ) -> Result<(i64, i64)> {
        let (filter_sql, filter_values) = Self::filter_sql(table, filter, 3)?;
        let sql = format!(
            "SELECT COUNT(*), COALESCE(SUM(l.record_id IS NULL), 0)
             FROM {table} m
             LEFT JOIN hm_record_label l
               ON l.record_table = '{table}' AND l.record_id = CAST(m.id AS TEXT)
             WHERE m.agent_id = ?1 AND (l.record_id IS NULL OR l.origin IS NOT ?2){filter_sql}"
        );
        let mut params_list: Vec<&dyn rusqlite::ToSql> = vec![&agent_id, &origin];
        for v in &filter_values {
            params_list.push(v);
        }
        Ok(self
            .tx
            .query_row(&sql, params_list.as_slice(), |r| Ok((r.get(0)?, r.get(1)?)))?)
    }

    /// Deletes one agent's knowledge-graph rows (entities, then
    /// relationships). The §5.1 delete triggers fire for each row in this
    /// transaction. Returns the numbers deleted.
    pub fn delete_world_model(&self, agent_id: &str) -> Result<(usize, usize)> {
        let entities = self.tx.execute(
            "DELETE FROM world_model_entities WHERE agent_id = ?1",
            params![agent_id],
        )?;
        let relationships = self.tx.execute(
            "DELETE FROM world_model_relationships WHERE agent_id = ?1",
            params![agent_id],
        )?;
        Ok((entities, relationships))
    }

    /// Inserts one `world_model_entities` row and returns its id.
    pub fn insert_world_model_entity(
        &self,
        agent_id: &str,
        entity_json: &str,
        created_at: &str,
    ) -> Result<i64> {
        self.tx.execute(
            "INSERT INTO world_model_entities (agent_id, entity_json, created_at)
             VALUES (?1, ?2, ?3)",
            params![agent_id, entity_json, created_at],
        )?;
        Ok(self.tx.last_insert_rowid())
    }

    /// Inserts one `world_model_relationships` row and returns its id.
    pub fn insert_world_model_relationship(
        &self,
        agent_id: &str,
        relationship_json: &str,
        created_at: &str,
    ) -> Result<i64> {
        self.tx.execute(
            "INSERT INTO world_model_relationships (agent_id, relationship_json, created_at)
             VALUES (?1, ?2, ?3)",
            params![agent_id, relationship_json, created_at],
        )?;
        Ok(self.tx.last_insert_rowid())
    }

    /// Sets the completion columns of one `task_history` row: `status`,
    /// `result_json` (content), `completed_at` and `success`. `goal`,
    /// `steps_json` and `started_at` are unchanged. Returns the number of
    /// rows changed.
    pub fn update_task_completion(
        &self,
        id: &str,
        status: &str,
        result_json: Option<&str>,
        completed_at: &str,
        success: bool,
    ) -> Result<usize> {
        Ok(self.tx.execute(
            "UPDATE task_history
             SET status = ?2, result_json = ?3, completed_at = ?4, success = ?5
             WHERE id = ?1",
            params![id, status, result_json, completed_at, success],
        )?)
    }

    /// Sets bookkeeping columns of one `task_history` row (none of them is
    /// covered by the §5.2 digest): each given value replaces its column.
    /// Returns the number of rows changed.
    pub fn update_task_bookkeeping(&self, id: &str, b: &TaskBookkeeping<'_>) -> Result<usize> {
        Ok(self.tx.execute(
            "UPDATE task_history
             SET status = COALESCE(?2, status),
                 fuel_consumed = COALESCE(?3, fuel_consumed),
                 fuel_budget = COALESCE(?4, fuel_budget),
                 completed_at = COALESCE(?5, completed_at),
                 success = COALESCE(?6, success)
             WHERE id = ?1",
            params![
                id,
                b.status,
                b.fuel_consumed,
                b.fuel_budget,
                b.completed_at,
                b.success
            ],
        )?)
    }

    pub fn insert_admission(&self, a: &AdmissionRow) -> Result<()> {
        self.tx.execute(
            "INSERT INTO hm_dispatch_admission
               (admission_id, agent_id, unit_kind, route_class, policy_generation, revocation_seq,
                record_count, grant_dep_count, raw_label, outcome)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                a.admission_id,
                a.agent_id,
                a.unit_kind,
                a.route_class,
                a.policy_generation,
                a.revocation_seq,
                a.record_count,
                a.grant_dep_count,
                a.raw_label,
                a.outcome
            ],
        )?;
        Ok(())
    }

    /// Every admission row, in insertion order (content-free).
    pub fn admission_rows(&self) -> Result<Vec<AdmissionRow>> {
        let mut stmt = self.tx.prepare(
            "SELECT admission_id, agent_id, unit_kind, route_class, policy_generation,
                    revocation_seq, record_count, grant_dep_count, raw_label, outcome
             FROM hm_dispatch_admission ORDER BY rowid",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(AdmissionRow {
                admission_id: r.get(0)?,
                agent_id: r.get(1)?,
                unit_kind: r.get(2)?,
                route_class: r.get(3)?,
                policy_generation: r.get(4)?,
                revocation_seq: r.get(5)?,
                record_count: r.get(6)?,
                grant_dep_count: r.get(7)?,
                raw_label: r.get(8)?,
                outcome: r.get(9)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// Sets the outcome of an admission still recorded as `admitted`.
    /// Returns the number of rows changed.
    pub fn update_admission_outcome(&self, admission_id: &str, outcome: &str) -> Result<usize> {
        Ok(self.tx.execute(
            "UPDATE hm_dispatch_admission SET outcome = ?2
             WHERE admission_id = ?1 AND outcome = 'admitted'",
            params![admission_id, outcome],
        )?)
    }

    /// `hm_policy_change` rows with a cause other than `except_cause` that
    /// have no event with subject `policy:<agent_id>:<policy_generation>`,
    /// at most `limit`, ordered by `(agent_id, policy_generation)`.
    pub fn policy_changes_without_event(
        &self,
        except_cause: &str,
        limit: i64,
    ) -> Result<Vec<PolicyChangeRow>> {
        let mut stmt = self.tx.prepare(
            "SELECT c.agent_id, c.policy_generation, c.local_only, c.basis, c.cause
             FROM hm_policy_change c
             WHERE c.cause <> ?1
               AND NOT EXISTS (SELECT 1 FROM hm_event e
                               WHERE e.subject = 'policy:' || c.agent_id || ':' || c.policy_generation)
             ORDER BY c.agent_id, c.policy_generation
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![except_cause, limit], |r| {
            Ok(PolicyChangeRow {
                agent_id: r.get(0)?,
                policy_generation: r.get(1)?,
                local_only: r.get(2)?,
                basis: r.get(3)?,
                cause: r.get(4)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// `hm_t0_tombstone` rows that have no event with subject
    /// `tombstone:<record_table>:<record_id>`, at most `limit`, ordered by
    /// `(record_table, record_id)`.
    pub fn tombstones_without_event(&self, limit: i64) -> Result<Vec<TombstoneRow>> {
        let mut stmt = self.tx.prepare(
            "SELECT t.record_table, t.record_id, m.agent_id
             FROM hm_t0_tombstone t
             LEFT JOIN hm_t0_member m
               ON m.record_table = t.record_table AND m.record_id = t.record_id
             WHERE NOT EXISTS (SELECT 1 FROM hm_event e
                               WHERE e.subject = 'tombstone:' || t.record_table || ':' || t.record_id)
             ORDER BY t.record_table, t.record_id
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], |r| {
            Ok(TombstoneRow {
                record_table: r.get(0)?,
                record_id: r.get(1)?,
                agent_id: r.get(2)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn insert_migration(&self, m: &MigrationRow) -> Result<()> {
        self.tx.execute(
            "INSERT INTO hm_migration
               (k, contract_version, state, t0_wallclock, member_count, member_set_sha256)
             VALUES ('hm', ?1, ?2, ?3, ?4, ?5)",
            params![
                m.contract_version,
                m.state,
                m.t0_wallclock,
                m.member_count,
                m.member_set_sha256
            ],
        )?;
        Ok(())
    }

    /// The `hm_migration` rows (at most one under its primary key).
    pub fn migration_rows(&self) -> Result<Vec<MigrationRow>> {
        let mut stmt = self.tx.prepare(
            "SELECT contract_version, state, t0_wallclock, member_count, member_set_sha256
             FROM hm_migration",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(MigrationRow {
                contract_version: r.get(0)?,
                state: r.get(1)?,
                t0_wallclock: r.get(2)?,
                member_count: r.get(3)?,
                member_set_sha256: r.get(4)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// Inserts one `hm_event` row. A duplicate subject is rejected by
    /// `UNIQUE(subject)`.
    pub fn insert_event(
        &self,
        subject: &str,
        kind: &str,
        agent_id: Option<&str>,
        fields_json: &str,
    ) -> Result<()> {
        self.tx.execute(
            "INSERT INTO hm_event (subject, kind, agent_id, fields) VALUES (?1, ?2, ?3, ?4)",
            params![subject, kind, agent_id, fields_json],
        )?;
        Ok(())
    }

    /// The `(kind, agent_id)` of every `hm_event` row, in order
    /// (content-free).
    pub fn event_kinds(&self) -> Result<Vec<(String, Option<String>)>> {
        let mut stmt = self
            .tx
            .prepare("SELECT kind, agent_id FROM hm_event ORDER BY n")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// The stored `sqlite_master.sql` of a trigger, if one of that name exists.
    pub fn trigger_sql(&self, name: &str) -> Result<Option<String>> {
        Ok(self
            .tx
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type = 'trigger' AND name = ?1",
                params![name],
                |r| r.get(0),
            )
            .optional()?)
    }
}
