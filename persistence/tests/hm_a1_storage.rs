//! P3-C11-HM-A1: storage-core tests for the dormant historical-memory module
//! (P3-HM-CONTRACT-02 §5.1 schema and triggers, §5.2 digest, persistence-scope
//! guards of A-S1). Synthetic in-memory or test-owned databases only.

use nexus_persistence::hm::{HM_TABLES, HM_TRIGGERS};
use nexus_persistence::NexusDatabase;

// ── helpers ─────────────────────────────────────────────────────────────────

fn db_with_schema() -> NexusDatabase {
    let db = NexusDatabase::in_memory().expect("in-memory database");
    db.hm()
        .immediate(|tx| tx.create_schema())
        .expect("create hm schema");
    db
}

/// Runs a batch on a test transaction; returns the SQLite error text on failure.
fn exec(db: &NexusDatabase, sql: &str) -> Result<(), String> {
    db.with_transaction(|tx| {
        tx.execute_batch(sql)?;
        Ok(())
    })
    .map_err(|e| e.to_string())
}

fn count(db: &NexusDatabase, sql: &str) -> i64 {
    db.with_transaction(|tx| Ok(tx.query_row(sql, [], |r| r.get::<_, i64>(0))?))
        .expect("count query")
}

fn text(db: &NexusDatabase, sql: &str) -> Option<String> {
    db.with_transaction(|tx| Ok(tx.query_row(sql, [], |r| r.get::<_, Option<String>>(0))?))
        .expect("text query")
}

fn assert_rejected(db: &NexusDatabase, sql: &str, needle: &str) {
    match exec(db, sql) {
        Ok(()) => panic!("expected rejection containing {needle:?} for: {sql}"),
        Err(e) => assert!(
            e.contains(needle),
            "rejection {e:?} lacks {needle:?} for: {sql}"
        ),
    }
}

fn assert_accepted(db: &NexusDatabase, sql: &str) {
    if let Err(e) = exec(db, sql) {
        panic!("expected acceptance, got {e} for: {sql}");
    }
}

const MIGRATION_ROW: &str = "INSERT INTO hm_migration
    (k, contract_version, state, t0_wallclock, member_count, member_set_sha256)
    VALUES ('hm', 'P3-HM-CONTRACT-02', 'COMPLETE', '2026-01-01T00:00:00Z', 1, 'x')";

/// One agent_memory row (id 1), labelled and ledgered as a T0 member, with
/// the migration row written (the ledger is then closed).
fn db_with_member() -> NexusDatabase {
    let db = db_with_schema();
    assert_accepted(
        &db,
        "INSERT INTO agent_memory (id, agent_id, memory_type, key, value_json, created_at, last_accessed)
           VALUES (1, 'a', 'episodic', 'SYNTH-k1', '\"SYNTH-v1\"', 't', 't');
         INSERT INTO hm_t0_member VALUES ('agent_memory', '1', 'a', 'd1');
         INSERT INTO hm_record_label VALUES ('agent_memory', '1', 'a', 'LEGACY_UNKNOWN', 'legacy', 'd1', 0);",
    );
    assert_accepted(&db, MIGRATION_ROW);
    db
}

// ── N1: exact §5.1 schema ───────────────────────────────────────────────────

/// Independent transcription of the contract's §5.1 DDL (02-P3-HM-CONTRACT-02.md
/// lines 366-465). Compared after removing `--` comments and collapsing
/// whitespace.
const CONTRACT_5_1_DDL: &str = r#"
CREATE TABLE hm_migration (
  k                 TEXT PRIMARY KEY CHECK (k = 'hm'),
  contract_version  TEXT NOT NULL,            -- 'P3-HM-CONTRACT-02'
  state             TEXT NOT NULL CHECK (state IN ('COMPLETE')),
  t0_wallclock      TEXT NOT NULL,            -- informational only; no decision reads it
  member_count      INTEGER NOT NULL,         -- rows of hm_t0_member, fixed at T0
  member_set_sha256 TEXT NOT NULL             -- over hm_t0_member, §9.5; fixed at T0
);

CREATE TABLE hm_t0_member (                    -- immutable T0 membership ledger (M1)
  record_table       TEXT NOT NULL CHECK (record_table IN
                       ('agent_memory','task_history','strategy_scores',
                        'world_model_entities','world_model_relationships')),
  record_id          TEXT NOT NULL,
  agent_id           TEXT NOT NULL,
  t0_content_sha256  TEXT NOT NULL,           -- §5.2 digest of the row at T0
  PRIMARY KEY (record_table, record_id)
) WITHOUT ROWID;

CREATE TABLE hm_t0_tombstone (                 -- a T0 member that was later deleted (M1)
  record_table  TEXT NOT NULL,
  record_id     TEXT NOT NULL,
  PRIMARY KEY (record_table, record_id),
  FOREIGN KEY (record_table, record_id)
    REFERENCES hm_t0_member (record_table, record_id)
) WITHOUT ROWID;

CREATE TABLE hm_agent_policy (
  agent_id           TEXT PRIMARY KEY,        -- supervisor UUID, canonical lowercase
  local_only         INTEGER NOT NULL CHECK (local_only IN (0,1)),
  policy_generation  INTEGER NOT NULL CHECK (policy_generation >= 1),
  revocation_seq     INTEGER NOT NULL CHECK (revocation_seq >= 0),
  basis              TEXT NOT NULL CHECK (basis IN
                       ('t0_baseline','default_restrictive','owner_confirmed','tightened'))
);

CREATE TABLE hm_policy_change (                -- authority history, not audit (§5.3)
  agent_id           TEXT NOT NULL,
  policy_generation  INTEGER NOT NULL,         -- the generation this change produced
  local_only         INTEGER NOT NULL CHECK (local_only IN (0,1)),
  basis              TEXT NOT NULL,
  cause              TEXT NOT NULL CHECK (cause IN
                       ('t0_baseline','row_created','governed_tighten','config_tighten',
                        'owner_loosen')),
  PRIMARY KEY (agent_id, policy_generation)
);

CREATE TABLE hm_record_label (
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
);

CREATE TABLE hm_derivation (
  child_table   TEXT NOT NULL, child_id  TEXT NOT NULL,
  parent_table  TEXT NOT NULL, parent_id TEXT NOT NULL,
  parent_label_at_derivation TEXT NOT NULL,
  PRIMARY KEY (child_table, child_id, parent_table, parent_id)
);

CREATE TABLE hm_grant (
  grant_id         TEXT PRIMARY KEY,          -- 128-bit random, backend-generated
  agent_id         TEXT NOT NULL,
  scope            TEXT NOT NULL CHECK (scope IN ('record','pre_t0_legacy_set')),
  record_table     TEXT,                      -- record scope only
  record_id        TEXT,                      -- record scope only
  bound_content_sha256 TEXT,                  -- record scope only
  state            TEXT NOT NULL CHECK (state IN ('live','revoked')),
  revoked_at_revocation_seq INTEGER            -- agent's revocation_seq after this revocation; NULL while live
);

CREATE TABLE hm_dispatch_admission (
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
);

CREATE TABLE hm_event (                        -- content-free audit outbox (§15)
  n        INTEGER PRIMARY KEY AUTOINCREMENT,
  subject  TEXT NOT NULL UNIQUE,               -- the authority row it describes, §5.3
  kind     TEXT NOT NULL,
  agent_id TEXT,
  fields   TEXT NOT NULL                       -- JSON of counts, classes, generations only
);
"#;

fn normalize_sql(sql: &str) -> String {
    let no_comments: Vec<&str> = sql
        .lines()
        .map(|l| match l.find("--") {
            Some(i) => &l[..i],
            None => l,
        })
        .collect();
    no_comments
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_end_matches(';')
        .trim()
        .to_string()
}

#[test]
fn hm_n1_schema_matches_contract_5_1_exactly() {
    let db = db_with_schema();
    // Comments are removed before splitting: one of them contains a ';'.
    let contract: Vec<String> = normalize_sql(CONTRACT_5_1_DDL)
        .split(';')
        .map(normalize_sql)
        .filter(|s| !s.is_empty())
        .collect();
    assert_eq!(contract.len(), 10, "the contract defines ten tables");
    assert_eq!(HM_TABLES.len(), 10);
    for stmt in &contract {
        let name = stmt
            .strip_prefix("CREATE TABLE ")
            .and_then(|r| r.split_whitespace().next())
            .expect("table name");
        let stored = text(
            &db,
            &format!("SELECT sql FROM sqlite_master WHERE type='table' AND name='{name}'"),
        )
        .unwrap_or_else(|| panic!("table {name} missing"));
        assert_eq!(
            normalize_sql(&stored),
            *stmt,
            "table {name} differs from §5.1"
        );
    }
    // No other hm_* table exists.
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name LIKE 'hm\\_%' ESCAPE '\\'"
        ),
        10
    );
    // WITHOUT ROWID on the ledger and the tombstone table.
    for t in ["hm_t0_member", "hm_t0_tombstone"] {
        let sql = text(
            &db,
            &format!("SELECT sql FROM sqlite_master WHERE name='{t}'"),
        )
        .unwrap();
        assert!(
            sql.trim_end().ends_with("WITHOUT ROWID"),
            "{t} must be WITHOUT ROWID"
        );
    }
    // Every §5.1 trigger exists, and no other hm_* trigger.
    for (name, _) in HM_TRIGGERS {
        assert_eq!(
            count(
                &db,
                &format!(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='trigger' AND name='{name}'"
                )
            ),
            1,
            "trigger {name} missing"
        );
    }
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM sqlite_master WHERE type='trigger' AND name LIKE 'hm\\_%' ESCAPE '\\'"
        ),
        HM_TRIGGERS.len() as i64
    );
    // Stored trigger text is byte-identical to the constants (L4 relies on it).
    for (name, sql) in HM_TRIGGERS {
        let stored = text(
            &db,
            &format!("SELECT sql FROM sqlite_master WHERE type='trigger' AND name='{name}'"),
        )
        .unwrap();
        assert_eq!(stored, sql, "stored text of {name}");
    }
}

#[test]
fn hm_n1_schema_check_constraints_reject_out_of_domain_values() {
    let db = db_with_schema();
    for (sql, needle) in [
        ("INSERT INTO hm_migration VALUES ('x','c','COMPLETE','t',0,'d')", "CHECK"),
        ("INSERT INTO hm_migration VALUES ('hm','c','PARTIAL','t',0,'d')", "CHECK"),
        ("INSERT INTO hm_t0_member VALUES ('agents','1','a','d')", "CHECK"),
        ("INSERT INTO hm_record_label VALUES ('agent_memory','1','a','PUBLIC','legacy','d',0)", "CHECK"),
        ("INSERT INTO hm_policy_change VALUES ('a',1,2,'t0_baseline','t0_baseline')", "CHECK"),
        ("INSERT INTO hm_policy_change VALUES ('a',1,1,'t0_baseline','webview')", "CHECK"),
        ("INSERT INTO hm_grant VALUES ('g','a','everything',NULL,NULL,NULL,'live',NULL)", "CHECK"),
        ("INSERT INTO hm_grant VALUES ('g','a','record',NULL,NULL,NULL,'pending',NULL)", "CHECK"),
        ("INSERT INTO hm_dispatch_admission VALUES ('d','a','Goal','cloud',1,0,0,0,'L','admitted')", "CHECK"),
        ("INSERT INTO hm_dispatch_admission VALUES ('d','a','Goal','local',1,0,0,0,'L','sent')", "CHECK"),
    ] {
        assert_rejected(&db, sql, needle);
    }
    assert_accepted(
        &db,
        "INSERT INTO hm_policy_change VALUES ('a',1,1,'t0_baseline','t0_baseline');
         INSERT INTO hm_agent_policy VALUES ('a',1,1,0,'t0_baseline');",
    );
    for (sql, needle) in [
        (
            "UPDATE hm_agent_policy SET basis='webview' WHERE agent_id='a'",
            "CHECK",
        ),
        (
            "INSERT INTO hm_policy_change VALUES ('b',0,1,'x','row_created');
          INSERT INTO hm_agent_policy VALUES ('b',1,0,0,'tightened')",
            "CHECK",
        ),
        (
            "UPDATE hm_agent_policy SET revocation_seq=-1 WHERE agent_id='a'",
            "",
        ),
    ] {
        assert_rejected(&db, sql, needle);
    }
    assert_accepted(
        &db,
        "INSERT INTO hm_event (subject, kind, agent_id, fields) VALUES ('s','k',NULL,'{}')",
    );
    assert_rejected(
        &db,
        "INSERT INTO hm_event (subject, kind, agent_id, fields) VALUES ('s','k2',NULL,'{}')",
        "UNIQUE",
    );
}

// ── N1: ledger triggers ─────────────────────────────────────────────────────

#[test]
fn hm_n1_ledger_insert_accepted_before_migration_row() {
    let db = db_with_schema();
    assert_accepted(
        &db,
        "INSERT INTO hm_t0_member VALUES ('agent_memory','7','a','d')",
    );
    assert_eq!(count(&db, "SELECT COUNT(*) FROM hm_t0_member"), 1);
}

#[test]
fn hm_n1_ledger_insert_rejected_after_migration_row() {
    let db = db_with_member();
    assert_rejected(
        &db,
        "INSERT INTO hm_t0_member VALUES ('agent_memory','2','a','d2')",
        "ledger is closed after T0",
    );
    assert_eq!(count(&db, "SELECT COUNT(*) FROM hm_t0_member"), 1);
}

#[test]
fn hm_n1_ledger_update_rejected() {
    let db = db_with_member();
    for set in [
        "t0_content_sha256='forged'",
        "agent_id='b'",
        "record_id='9'",
        "record_table='task_history'",
    ] {
        assert_rejected(
            &db,
            &format!("UPDATE hm_t0_member SET {set} WHERE record_id='1'"),
            "ledger is immutable",
        );
    }
    assert_eq!(
        text(&db, "SELECT t0_content_sha256 FROM hm_t0_member").as_deref(),
        Some("d1")
    );
}

#[test]
fn hm_n1_ledger_update_trigger_accepts_label_content_update() {
    // §6.7: a content update rewrites the label digest, never the ledger.
    let db = db_with_member();
    assert_accepted(
        &db,
        "UPDATE hm_record_label SET content_sha256='d1-new', origin='LOCAL_ONLY_ORIGIN'
           WHERE record_table='agent_memory' AND record_id='1'",
    );
    assert_eq!(
        text(&db, "SELECT t0_content_sha256 FROM hm_t0_member").as_deref(),
        Some("d1")
    );
}

#[test]
fn hm_n1_ledger_delete_rejected() {
    let db = db_with_member();
    assert_rejected(&db, "DELETE FROM hm_t0_member", "ledger is immutable");
    assert_eq!(count(&db, "SELECT COUNT(*) FROM hm_t0_member"), 1);
}

#[test]
fn hm_n1_ledger_delete_trigger_accepts_member_row_deletion() {
    // Deleting the in-scope row is accepted; the ledger row survives (§6.8).
    let db = db_with_member();
    assert_accepted(&db, "DELETE FROM agent_memory WHERE id=1");
    assert_eq!(count(&db, "SELECT COUNT(*) FROM hm_t0_member"), 1);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM hm_t0_tombstone"), 1);
}

// ── N1: tombstone triggers ──────────────────────────────────────────────────

#[test]
fn hm_n1_tombstone_insert_rejected_while_row_exists() {
    let db = db_with_member();
    assert_rejected(
        &db,
        "INSERT INTO hm_t0_tombstone VALUES ('agent_memory','1')",
        "the named row still exists",
    );
    // Same for a UUID-keyed table.
    assert_accepted(
        &db,
        "INSERT INTO task_history (id, agent_id, goal, status, steps_json, started_at)
           VALUES ('11111111-1111-4111-8111-111111111111','a','SYNTH-goal','done','[]','t')",
    );
    assert_rejected(
        &db,
        "INSERT INTO hm_t0_tombstone VALUES ('task_history','11111111-1111-4111-8111-111111111111')",
        "the named row still exists",
    );
    assert_eq!(count(&db, "SELECT COUNT(*) FROM hm_t0_tombstone"), 0);
}

#[test]
fn hm_n1_tombstone_insert_accepted_once_row_is_absent() {
    let db = db_with_member();
    // Remove the row out of band (no delete trigger), then tombstone it directly.
    assert_accepted(
        &db,
        "DROP TRIGGER hm_del_agent_memory; DELETE FROM agent_memory WHERE id=1;",
    );
    assert_accepted(
        &db,
        "INSERT INTO hm_t0_tombstone VALUES ('agent_memory','1')",
    );
    assert_eq!(count(&db, "SELECT COUNT(*) FROM hm_t0_tombstone"), 1);
}

#[test]
fn hm_n1_tombstone_update_rejected() {
    let db = db_with_member();
    assert_accepted(&db, "DELETE FROM agent_memory WHERE id=1");
    assert_rejected(
        &db,
        "UPDATE hm_t0_tombstone SET record_id='2'",
        "tombstones are immutable",
    );
    assert_eq!(
        text(&db, "SELECT record_id FROM hm_t0_tombstone").as_deref(),
        Some("1")
    );
}

#[test]
fn hm_n1_tombstone_update_trigger_accepts_idempotent_reinsert() {
    // The delete triggers use INSERT OR IGNORE: a repeated tombstone is a no-op.
    let db = db_with_member();
    assert_accepted(&db, "DELETE FROM agent_memory WHERE id=1");
    assert_accepted(
        &db,
        "INSERT OR IGNORE INTO hm_t0_tombstone VALUES ('agent_memory','1')",
    );
    assert_eq!(count(&db, "SELECT COUNT(*) FROM hm_t0_tombstone"), 1);
}

#[test]
fn hm_n1_tombstone_delete_rejected() {
    let db = db_with_member();
    assert_accepted(&db, "DELETE FROM agent_memory WHERE id=1");
    assert_rejected(
        &db,
        "DELETE FROM hm_t0_tombstone",
        "tombstones are immutable",
    );
    assert_eq!(count(&db, "SELECT COUNT(*) FROM hm_t0_tombstone"), 1);
}

#[test]
fn hm_n1_tombstone_delete_trigger_accepts_trigger_written_tombstone() {
    let db = db_with_member();
    assert_accepted(&db, "DELETE FROM agent_memory WHERE id=1");
    assert_eq!(
        text(
            &db,
            "SELECT record_table || ':' || record_id FROM hm_t0_tombstone"
        )
        .as_deref(),
        Some("agent_memory:1")
    );
}

// ── N1: the five delete triggers ────────────────────────────────────────────

struct InScope {
    table: &'static str,
    insert_member: &'static str,
    insert_other: &'static str,
    member_id: &'static str,
    other_id: &'static str,
    delete_member: &'static str,
    delete_other: &'static str,
}

const IN_SCOPE: [InScope; 5] = [
    InScope {
        table: "agent_memory",
        insert_member: "INSERT INTO agent_memory (id, agent_id, memory_type, key, value_json, created_at, last_accessed) VALUES (10,'a','episodic','SYNTH-k','\"v\"','t','t')",
        insert_other: "INSERT INTO agent_memory (id, agent_id, memory_type, key, value_json, created_at, last_accessed) VALUES (11,'a','episodic','SYNTH-k2','\"v\"','t','t')",
        member_id: "10",
        other_id: "11",
        delete_member: "DELETE FROM agent_memory WHERE id=10",
        delete_other: "DELETE FROM agent_memory WHERE id=11",
    },
    InScope {
        table: "task_history",
        insert_member: "INSERT INTO task_history (id, agent_id, goal, status, steps_json, started_at) VALUES ('aaaaaaaa-0000-4000-8000-000000000010','a','SYNTH-g','done','[]','t')",
        insert_other: "INSERT INTO task_history (id, agent_id, goal, status, steps_json, started_at) VALUES ('aaaaaaaa-0000-4000-8000-000000000011','a','SYNTH-g2','done','[]','t')",
        member_id: "aaaaaaaa-0000-4000-8000-000000000010",
        other_id: "aaaaaaaa-0000-4000-8000-000000000011",
        delete_member: "DELETE FROM task_history WHERE id='aaaaaaaa-0000-4000-8000-000000000010'",
        delete_other: "DELETE FROM task_history WHERE id='aaaaaaaa-0000-4000-8000-000000000011'",
    },
    InScope {
        table: "strategy_scores",
        insert_member: "INSERT INTO strategy_scores (id, agent_id, strategy_hash, goal_type, updated_at) VALUES (10,'a','h10','g','t')",
        insert_other: "INSERT INTO strategy_scores (id, agent_id, strategy_hash, goal_type, updated_at) VALUES (11,'a','h11','g','t')",
        member_id: "10",
        other_id: "11",
        delete_member: "DELETE FROM strategy_scores WHERE id=10",
        delete_other: "DELETE FROM strategy_scores WHERE id=11",
    },
    InScope {
        table: "world_model_entities",
        insert_member: "INSERT INTO world_model_entities (id, agent_id, entity_json, created_at) VALUES (10,'a','{}','t')",
        insert_other: "INSERT INTO world_model_entities (id, agent_id, entity_json, created_at) VALUES (11,'a','{}','t')",
        member_id: "10",
        other_id: "11",
        delete_member: "DELETE FROM world_model_entities WHERE id=10",
        delete_other: "DELETE FROM world_model_entities WHERE id=11",
    },
    InScope {
        table: "world_model_relationships",
        insert_member: "INSERT INTO world_model_relationships (id, agent_id, relationship_json, created_at) VALUES (10,'a','{}','t')",
        insert_other: "INSERT INTO world_model_relationships (id, agent_id, relationship_json, created_at) VALUES (11,'a','{}','t')",
        member_id: "10",
        other_id: "11",
        delete_member: "DELETE FROM world_model_relationships WHERE id=10",
        delete_other: "DELETE FROM world_model_relationships WHERE id=11",
    },
];

/// Member row and non-member row, both labelled; each is the child of one
/// derivation edge and the parent of another. The migration row closes the
/// ledger afterwards.
fn db_for_delete_trigger(s: &InScope) -> NexusDatabase {
    let db = db_with_schema();
    let t = s.table;
    let (m, o) = (s.member_id, s.other_id);
    assert_accepted(&db, s.insert_member);
    assert_accepted(&db, s.insert_other);
    assert_accepted(
        &db,
        &format!(
            "INSERT INTO hm_t0_member VALUES ('{t}','{m}','a','dm');
             INSERT INTO hm_record_label VALUES ('{t}','{m}','a','LEGACY_UNKNOWN','legacy','dm',0);
             INSERT INTO hm_record_label VALUES ('{t}','{o}','a','LOCAL_ONLY_ORIGIN','loop_episodic','do',1);
             INSERT INTO hm_derivation VALUES ('{t}','{m}','agent_memory','900','LEGACY_UNKNOWN');
             INSERT INTO hm_derivation VALUES ('{t}','{o}','agent_memory','901','LOCAL_ONLY_ORIGIN');
             INSERT INTO hm_derivation VALUES ('agent_memory','902','{t}','{m}','LEGACY_UNKNOWN');
             INSERT INTO hm_derivation VALUES ('agent_memory','903','{t}','{o}','LOCAL_ONLY_ORIGIN');"
        ),
    );
    assert_accepted(&db, MIGRATION_ROW);
    db
}

fn label_count(db: &NexusDatabase, t: &str, id: &str) -> i64 {
    count(
        db,
        &format!(
            "SELECT COUNT(*) FROM hm_record_label WHERE record_table='{t}' AND record_id='{id}'"
        ),
    )
}

fn child_edge_count(db: &NexusDatabase, t: &str, id: &str) -> i64 {
    count(
        db,
        &format!("SELECT COUNT(*) FROM hm_derivation WHERE child_table='{t}' AND child_id='{id}'"),
    )
}

fn parent_edge_count(db: &NexusDatabase, t: &str, id: &str) -> i64 {
    count(
        db,
        &format!(
            "SELECT COUNT(*) FROM hm_derivation WHERE parent_table='{t}' AND parent_id='{id}'"
        ),
    )
}

fn tombstone_count(db: &NexusDatabase, t: &str, id: &str) -> i64 {
    count(
        db,
        &format!(
            "SELECT COUNT(*) FROM hm_t0_tombstone WHERE record_table='{t}' AND record_id='{id}'"
        ),
    )
}

#[test]
fn hm_n1_delete_triggers_remove_label_and_child_edges_and_tombstone_members() {
    for s in &IN_SCOPE {
        let db = db_for_delete_trigger(s);
        let (t, m) = (s.table, s.member_id);
        assert_accepted(&db, s.delete_member);
        assert_eq!(label_count(&db, t, m), 0, "{t}: member label removed");
        assert_eq!(child_edge_count(&db, t, m), 0, "{t}: child edges removed");
        assert_eq!(tombstone_count(&db, t, m), 1, "{t}: member tombstoned");
        assert_eq!(
            count(
                &db,
                &format!("SELECT COUNT(*) FROM hm_t0_member WHERE record_table='{t}'")
            ),
            1,
            "{t}: ledger untouched"
        );
    }
}

#[test]
fn hm_n1_delete_triggers_reject_tombstone_for_non_member_and_touch_nothing_else() {
    for s in &IN_SCOPE {
        let db = db_for_delete_trigger(s);
        let (t, m, o) = (s.table, s.member_id, s.other_id);
        assert_accepted(&db, s.delete_other);
        assert_eq!(label_count(&db, t, o), 0, "{t}: non-member label removed");
        assert_eq!(
            child_edge_count(&db, t, o),
            0,
            "{t}: non-member child edges removed"
        );
        assert_eq!(
            tombstone_count(&db, t, o),
            0,
            "{t}: no tombstone for a non-member"
        );
        assert_eq!(count(&db, "SELECT COUNT(*) FROM hm_t0_tombstone"), 0);
        // Parent edges are kept (§6.8); the surviving member is untouched.
        assert_eq!(parent_edge_count(&db, t, o), 1, "{t}: parent edge kept");
        assert_eq!(label_count(&db, t, m), 1, "{t}: other label untouched");
        assert_eq!(
            child_edge_count(&db, t, m),
            1,
            "{t}: other child edge untouched"
        );
        // Deleting the member afterwards keeps its parent edge too.
        assert_accepted(&db, s.delete_member);
        assert_eq!(
            parent_edge_count(&db, t, m),
            1,
            "{t}: member parent edge kept"
        );
    }
}

// ── N1: label origin never lowered ──────────────────────────────────────────

const ORIGINS: [&str; 3] = ["UNRESTRICTED_ORIGIN", "LEGACY_UNKNOWN", "LOCAL_ONLY_ORIGIN"];

fn db_with_label(origin: &str) -> NexusDatabase {
    let db = db_with_schema();
    assert_accepted(
        &db,
        &format!(
            "INSERT INTO hm_record_label VALUES ('agent_memory','5','a','{origin}','loop_episodic','d',1)"
        ),
    );
    db
}

#[test]
fn hm_n1_label_origin_lowering_rejected() {
    for (hi, from) in ORIGINS.iter().enumerate() {
        for to in &ORIGINS[..hi] {
            let db = db_with_label(from);
            assert_rejected(
                &db,
                &format!("UPDATE hm_record_label SET origin='{to}'"),
                "origin cannot be lowered",
            );
            assert_eq!(
                text(&db, "SELECT origin FROM hm_record_label").as_deref(),
                Some(*from)
            );
        }
    }
}

#[test]
fn hm_n1_label_origin_raise_or_keep_accepted() {
    for (lo, from) in ORIGINS.iter().enumerate() {
        for to in &ORIGINS[lo..] {
            let db = db_with_label(from);
            assert_accepted(
                &db,
                &format!("UPDATE hm_record_label SET origin='{to}', content_sha256='d2'"),
            );
            assert_eq!(
                text(&db, "SELECT origin FROM hm_record_label").as_deref(),
                Some(*to)
            );
        }
    }
}

// ── N1: grant triggers (no writer exists; AD-4) ─────────────────────────────

fn db_with_grant(state: &str, seq: &str) -> NexusDatabase {
    let db = db_with_schema();
    assert_accepted(
        &db,
        &format!(
            "INSERT INTO hm_grant VALUES ('g1','a','record','agent_memory','1','d','{state}',{seq})"
        ),
    );
    db
}

#[test]
fn hm_n1_grant_revoked_to_live_rejected() {
    let db = db_with_grant("revoked", "3");
    assert_rejected(
        &db,
        "UPDATE hm_grant SET state='live'",
        "a revoked grant cannot become live",
    );
    assert_rejected(
        &db,
        "UPDATE hm_grant SET state='live', revoked_at_revocation_seq=NULL",
        "",
    );
    assert_eq!(
        text(&db, "SELECT state FROM hm_grant").as_deref(),
        Some("revoked")
    );
}

#[test]
fn hm_n1_grant_live_to_revoked_accepted() {
    let db = db_with_grant("live", "NULL");
    assert_accepted(
        &db,
        "UPDATE hm_grant SET state='revoked', revoked_at_revocation_seq=1",
    );
    assert_eq!(
        text(
            &db,
            "SELECT state || ':' || revoked_at_revocation_seq FROM hm_grant"
        )
        .as_deref(),
        Some("revoked:1")
    );
}

#[test]
fn hm_n1_grant_revoked_seq_change_rejected_once_set() {
    let db = db_with_grant("revoked", "3");
    for v in ["4", "2", "NULL"] {
        assert_rejected(
            &db,
            &format!("UPDATE hm_grant SET revoked_at_revocation_seq={v}"),
            "revoked_at_revocation_seq is fixed once set",
        );
    }
    assert_eq!(
        count(&db, "SELECT revoked_at_revocation_seq FROM hm_grant"),
        3
    );
}

#[test]
fn hm_n1_grant_revoked_seq_set_from_null_accepted() {
    let db = db_with_grant("live", "NULL");
    assert_accepted(&db, "UPDATE hm_grant SET revoked_at_revocation_seq=7");
    // Re-writing the same value is not a change.
    assert_accepted(
        &db,
        "UPDATE hm_grant SET revoked_at_revocation_seq=7, state='revoked'",
    );
    assert_eq!(
        count(&db, "SELECT revoked_at_revocation_seq FROM hm_grant"),
        7
    );
}

// ── N1: policy triggers ─────────────────────────────────────────────────────

fn db_with_policy() -> NexusDatabase {
    let db = db_with_schema();
    assert_accepted(
        &db,
        "INSERT INTO hm_policy_change VALUES ('a',3,1,'tightened','governed_tighten');
         INSERT INTO hm_agent_policy VALUES ('a',1,3,2,'tightened');",
    );
    db
}

#[test]
fn hm_n1_policy_generation_or_sequence_lowering_rejected() {
    let db = db_with_policy();
    assert_accepted(
        &db,
        "INSERT INTO hm_policy_change VALUES ('a',2,1,'tightened','governed_tighten')",
    );
    assert_rejected(
        &db,
        "UPDATE hm_agent_policy SET policy_generation=2",
        "generation and revocation sequence never fall",
    );
    assert_rejected(
        &db,
        "UPDATE hm_agent_policy SET revocation_seq=1",
        "generation and revocation sequence never fall",
    );
    assert_eq!(
        text(
            &db,
            "SELECT policy_generation || ':' || revocation_seq FROM hm_agent_policy"
        )
        .as_deref(),
        Some("3:2")
    );
}

#[test]
fn hm_n1_policy_generation_or_sequence_raise_accepted() {
    let db = db_with_policy();
    assert_accepted(&db, "UPDATE hm_agent_policy SET revocation_seq=3");
    assert_accepted(
        &db,
        "INSERT INTO hm_policy_change VALUES ('a',4,1,'tightened','config_tighten');
         UPDATE hm_agent_policy SET policy_generation=4;",
    );
    assert_eq!(
        text(
            &db,
            "SELECT policy_generation || ':' || revocation_seq FROM hm_agent_policy"
        )
        .as_deref(),
        Some("4:3")
    );
}

#[test]
fn hm_n1_policy_change_update_and_delete_rejected() {
    let db = db_with_policy();
    assert_rejected(
        &db,
        "UPDATE hm_policy_change SET local_only=0",
        "insert-only",
    );
    assert_rejected(
        &db,
        "UPDATE hm_policy_change SET cause='owner_loosen'",
        "insert-only",
    );
    assert_rejected(&db, "DELETE FROM hm_policy_change", "insert-only");
    assert_eq!(count(&db, "SELECT COUNT(*) FROM hm_policy_change"), 1);
}

#[test]
fn hm_n1_policy_change_insert_accepted() {
    let db = db_with_policy();
    assert_accepted(
        &db,
        "INSERT INTO hm_policy_change VALUES ('a',4,0,'owner_confirmed','owner_loosen')",
    );
    assert_eq!(count(&db, "SELECT COUNT(*) FROM hm_policy_change"), 2);
}

#[test]
fn hm_n1_policy_insert_without_history_rejected() {
    let db = db_with_schema();
    let needle = "write its hm_policy_change row first";
    assert_rejected(
        &db,
        "INSERT INTO hm_agent_policy VALUES ('a',1,1,0,'t0_baseline')",
        needle,
    );
    // A history row with a different generation or local_only does not qualify.
    assert_accepted(
        &db,
        "INSERT INTO hm_policy_change VALUES ('a',2,1,'t0_baseline','t0_baseline');
         INSERT INTO hm_policy_change VALUES ('a',1,0,'t0_baseline','t0_baseline');",
    );
    assert_rejected(
        &db,
        "INSERT INTO hm_agent_policy VALUES ('a',1,1,0,'t0_baseline')",
        needle,
    );
    // History written after the policy row is too late: the insert already failed.
    assert_eq!(count(&db, "SELECT COUNT(*) FROM hm_agent_policy"), 0);
}

#[test]
fn hm_n1_policy_insert_with_history_first_accepted() {
    let db = db_with_schema();
    assert_accepted(
        &db,
        "INSERT INTO hm_policy_change VALUES ('a',1,1,'t0_baseline','t0_baseline');
         INSERT INTO hm_agent_policy VALUES ('a',1,1,0,'t0_baseline');",
    );
    assert_eq!(count(&db, "SELECT COUNT(*) FROM hm_agent_policy"), 1);
}

#[test]
fn hm_n1_policy_update_without_history_rejected() {
    let db = db_with_policy();
    let needle = "write its hm_policy_change row first";
    // Loosening without its history row.
    assert_rejected(
        &db,
        "UPDATE hm_agent_policy SET local_only=0, policy_generation=4, basis='owner_confirmed'",
        needle,
    );
    // Generation bump without its history row.
    assert_rejected(
        &db,
        "UPDATE hm_agent_policy SET policy_generation=4",
        needle,
    );
    // A local_only flip that keeps the generation: no row (a,3,0) exists.
    assert_rejected(&db, "UPDATE hm_agent_policy SET local_only=0", needle);
    assert_eq!(
        text(
            &db,
            "SELECT local_only || ':' || policy_generation FROM hm_agent_policy"
        )
        .as_deref(),
        Some("1:3")
    );
}

#[test]
fn hm_n1_policy_update_with_history_first_accepted() {
    let db = db_with_policy();
    assert_accepted(
        &db,
        "INSERT INTO hm_policy_change VALUES ('a',4,0,'owner_confirmed','owner_loosen');
         UPDATE hm_agent_policy SET local_only=0, policy_generation=4, basis='owner_confirmed';",
    );
    // An update that changes neither local_only nor the generation needs no history row.
    assert_accepted(&db, "UPDATE hm_agent_policy SET revocation_seq=5");
    assert_eq!(
        text(
            &db,
            "SELECT local_only || ':' || policy_generation || ':' || revocation_seq FROM hm_agent_policy"
        )
        .as_deref(),
        Some("0:4:5")
    );
}

// ── N2: §5.2 content digest ─────────────────────────────────────────────────

use nexus_persistence::hm::{content_digest, member_set_digest, MemberRow};
use sha2::{Digest, Sha256};

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn scan(db: &NexusDatabase, table: &str) -> Vec<nexus_persistence::hm::ScannedRow> {
    db.hm()
        .immediate(|tx| tx.scan_table(table))
        .expect("scan table")
}

fn only_digest(db: &NexusDatabase, table: &str) -> String {
    let rows = scan(db, table);
    assert_eq!(rows.len(), 1, "{table}: one fixture row");
    rows[0].content_sha256.clone()
}

/// agent_memory: tag, then agent_id, memory_type, key, value_json, created_at.
#[test]
fn hm_n2_agent_memory_digest_matches_hand_built_bytes() {
    let db = NexusDatabase::in_memory().unwrap();
    assert_accepted(
        &db,
        "INSERT INTO agent_memory (id, agent_id, memory_type, key, value_json, relevance_score, access_count, created_at, last_accessed, expires_at)
           VALUES (3, 'ag', 'episodic', 'k', '\"v\"', 0.5, 4, '2026-01-01', 'LA', 'EX')",
    );
    let mut b: Vec<u8> = Vec::new();
    b.extend_from_slice(b"nexus-hm-v1/agent_memory");
    b.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 2]);
    b.extend_from_slice(b"ag");
    b.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 8]);
    b.extend_from_slice(b"episodic");
    b.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 1]);
    b.extend_from_slice(b"k");
    b.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 3]);
    b.extend_from_slice(b"\"v\"");
    b.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 10]);
    b.extend_from_slice(b"2026-01-01");
    assert_eq!(only_digest(&db, "agent_memory"), sha256_hex(&b));
    assert_eq!(scan(&db, "agent_memory")[0].record_id, "3");
    assert_eq!(scan(&db, "agent_memory")[0].agent_id, "ag");
}

/// task_history: tag, then id, agent_id, goal, steps_json, result_json (NULL), started_at.
#[test]
fn hm_n2_task_history_digest_matches_hand_built_bytes_with_null_marker() {
    let db = NexusDatabase::in_memory().unwrap();
    assert_accepted(
        &db,
        "INSERT INTO task_history (id, agent_id, goal, status, steps_json, result_json, fuel_consumed, started_at, completed_at, success)
           VALUES ('u-1', 'ag', 'gl', 'running', '[]', NULL, 1.5, 'S', NULL, 0)",
    );
    let mut b: Vec<u8> = Vec::new();
    b.extend_from_slice(b"nexus-hm-v1/task_history");
    b.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 3]);
    b.extend_from_slice(b"u-1");
    b.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 2]);
    b.extend_from_slice(b"ag");
    b.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 2]);
    b.extend_from_slice(b"gl");
    b.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 2]);
    b.extend_from_slice(b"[]");
    b.extend_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]); // NULL result_json
    b.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 1]);
    b.extend_from_slice(b"S");
    assert_eq!(only_digest(&db, "task_history"), sha256_hex(&b));
    assert_eq!(scan(&db, "task_history")[0].record_id, "u-1");

    // NULL and the empty string are different values.
    let null_digest = only_digest(&db, "task_history");
    assert_accepted(&db, "UPDATE task_history SET result_json = ''");
    let mut e: Vec<u8> = b[..b.len() - 17].to_vec();
    e.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 0]); // '' has length 0
    e.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 1]);
    e.extend_from_slice(b"S");
    assert_eq!(only_digest(&db, "task_history"), sha256_hex(&e));
    assert_ne!(only_digest(&db, "task_history"), null_digest);
}

/// strategy_scores: tag, then agent_id, strategy_hash, goal_type.
#[test]
fn hm_n2_strategy_scores_digest_matches_hand_built_bytes() {
    let db = NexusDatabase::in_memory().unwrap();
    assert_accepted(
        &db,
        "INSERT INTO strategy_scores (id, agent_id, strategy_hash, goal_type, uses, successes, total_fuel, total_duration_secs, composite_score, updated_at)
           VALUES (9, 'ag', 'hh', 'gt', 1, 1, 2.0, 3.0, 0.7, 'U')",
    );
    let mut b: Vec<u8> = Vec::new();
    b.extend_from_slice(b"nexus-hm-v1/strategy_scores");
    b.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 2]);
    b.extend_from_slice(b"ag");
    b.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 2]);
    b.extend_from_slice(b"hh");
    b.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 2]);
    b.extend_from_slice(b"gt");
    assert_eq!(only_digest(&db, "strategy_scores"), sha256_hex(&b));
}

/// world_model_entities: tag, then agent_id, entity_json, created_at.
#[test]
fn hm_n2_world_model_entities_digest_matches_hand_built_bytes() {
    let db = NexusDatabase::in_memory().unwrap();
    assert_accepted(
        &db,
        "INSERT INTO world_model_entities (id, agent_id, entity_json, created_at) VALUES (4, 'ag', '{}', 'C')",
    );
    let mut b: Vec<u8> = Vec::new();
    b.extend_from_slice(b"nexus-hm-v1/world_model_entities");
    b.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 2]);
    b.extend_from_slice(b"ag");
    b.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 2]);
    b.extend_from_slice(b"{}");
    b.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 1]);
    b.extend_from_slice(b"C");
    assert_eq!(only_digest(&db, "world_model_entities"), sha256_hex(&b));
}

/// world_model_relationships: tag, then agent_id, relationship_json, created_at.
#[test]
fn hm_n2_world_model_relationships_digest_matches_hand_built_bytes() {
    let db = NexusDatabase::in_memory().unwrap();
    assert_accepted(
        &db,
        "INSERT INTO world_model_relationships (id, agent_id, relationship_json, created_at) VALUES (4, 'ag', '[1]', 'C')",
    );
    let mut b: Vec<u8> = Vec::new();
    b.extend_from_slice(b"nexus-hm-v1/world_model_relationships");
    b.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 2]);
    b.extend_from_slice(b"ag");
    b.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 3]);
    b.extend_from_slice(b"[1]");
    b.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 1]);
    b.extend_from_slice(b"C");
    assert_eq!(
        only_digest(&db, "world_model_relationships"),
        sha256_hex(&b)
    );
}

#[test]
fn hm_n2_domain_tag_and_length_prefix_separate_values() {
    // Same bytes, different table tag.
    let v: [Option<&[u8]>; 3] = [Some(b"ag"), Some(b"{}"), Some(b"C")];
    assert_ne!(
        content_digest("world_model_entities", &v),
        content_digest("world_model_relationships", &v)
    );
    // Moving a byte across a field boundary changes the digest.
    assert_ne!(
        content_digest("strategy_scores", &[Some(b"ab"), Some(b"c"), Some(b"")]),
        content_digest("strategy_scores", &[Some(b"a"), Some(b"bc"), Some(b"")])
    );
    // NULL is the 0xFF..FF length alone; it differs from the empty string.
    let mut b: Vec<u8> = b"nexus-hm-v1/strategy_scores".to_vec();
    b.extend_from_slice(&[0xFF; 8]);
    assert_eq!(content_digest("strategy_scores", &[None]), sha256_hex(&b));
    assert_ne!(
        content_digest("strategy_scores", &[None]),
        content_digest("strategy_scores", &[Some(b"")])
    );
}

/// Updating an excluded (bookkeeping) column leaves the digest unchanged;
/// updating a covered column changes it.
#[test]
fn hm_n2_excluded_columns_do_not_change_the_digest() {
    let db = NexusDatabase::in_memory().unwrap();
    assert_accepted(
        &db,
        "INSERT INTO agent_memory (id, agent_id, memory_type, key, value_json, relevance_score, access_count, created_at, last_accessed, expires_at)
           VALUES (1, 'ag', 'episodic', 'k', 'v', 0.5, 0, 'C', 'L', NULL);
         INSERT INTO task_history (id, agent_id, goal, status, steps_json, result_json, fuel_consumed, fuel_budget, estimated_time_secs, actual_time_secs, quality_score, started_at, completed_at, success)
           VALUES ('u', 'ag', 'g', 'running', '[]', 'r', 0.0, NULL, NULL, NULL, NULL, 'S', NULL, 0);
         INSERT INTO strategy_scores (id, agent_id, strategy_hash, goal_type, uses, successes, total_fuel, total_duration_secs, composite_score, updated_at)
           VALUES (1, 'ag', 'h', 'gt', 1, 0, 0.0, 0.0, 0.0, 'U');
         INSERT INTO world_model_entities (id, agent_id, entity_json, created_at) VALUES (1, 'ag', '{}', 'C');
         INSERT INTO world_model_relationships (id, agent_id, relationship_json, created_at) VALUES (1, 'ag', '{}', 'C');",
    );
    let excluded: [(&str, &[&str]); 3] = [
        (
            "agent_memory",
            &[
                "relevance_score = 0.1",
                "access_count = 99",
                "last_accessed = 'L2'",
                "expires_at = 'E2'",
            ],
        ),
        (
            "task_history",
            &[
                "status = 'done'",
                "fuel_consumed = 5.0",
                "fuel_budget = 6.0",
                "estimated_time_secs = 7.0",
                "actual_time_secs = 8.0",
                "quality_score = 0.9",
                "completed_at = 'D'",
                "success = 1",
            ],
        ),
        (
            "strategy_scores",
            &[
                "uses = 2",
                "successes = 1",
                "total_fuel = 3.0",
                "total_duration_secs = 4.0",
                "composite_score = 0.5",
                "updated_at = 'U2'",
            ],
        ),
    ];
    for (table, sets) in excluded {
        let before = only_digest(&db, table);
        for set in sets {
            assert_accepted(&db, &format!("UPDATE {table} SET {set}"));
            assert_eq!(
                only_digest(&db, table),
                before,
                "{table}: {set} must not change the digest"
            );
        }
    }
    let covered: [(&str, &[&str]); 5] = [
        (
            "agent_memory",
            &[
                "agent_id = 'b'",
                "memory_type = 'semantic'",
                "key = 'k2'",
                "value_json = 'v2'",
                "created_at = 'C2'",
            ],
        ),
        (
            "task_history",
            &[
                "agent_id = 'b'",
                "goal = 'g2'",
                "steps_json = '[1]'",
                "result_json = 'r2'",
                "started_at = 'S2'",
            ],
        ),
        (
            "strategy_scores",
            &[
                "agent_id = 'b'",
                "strategy_hash = 'h2'",
                "goal_type = 'gt2'",
            ],
        ),
        (
            "world_model_entities",
            &["agent_id = 'b'", "entity_json = '{1}'", "created_at = 'C2'"],
        ),
        (
            "world_model_relationships",
            &[
                "agent_id = 'b'",
                "relationship_json = '{1}'",
                "created_at = 'C2'",
            ],
        ),
    ];
    for (table, sets) in covered {
        for set in sets {
            let before = only_digest(&db, table);
            assert_accepted(&db, &format!("UPDATE {table} SET {set}"));
            assert_ne!(
                only_digest(&db, table),
                before,
                "{table}: {set} must change the digest"
            );
        }
    }
    // task_history.id is covered too: the same content under another id differs.
    let before = only_digest(&db, "task_history");
    assert_accepted(&db, "UPDATE task_history SET id = 'u2'");
    assert_ne!(only_digest(&db, "task_history"), before);
}

#[test]
fn hm_n2_member_set_digest_matches_hand_built_bytes_in_sorted_order() {
    let row = |t: &str, id: &str, a: &str, d: &str| MemberRow {
        record_table: t.to_string(),
        record_id: id.to_string(),
        agent_id: a.to_string(),
        t0_content_sha256: d.to_string(),
    };
    // Given out of order; "10" sorts before "9" (byte order of TEXT ids).
    let rows = vec![
        row("task_history", "u", "a", "d3"),
        row("agent_memory", "9", "a", "d2"),
        row("agent_memory", "10", "b", "d1"),
    ];
    let mut b: Vec<u8> = b"nexus-hm-v1/t0-members".to_vec();
    for (t, id, a, d) in [
        ("agent_memory", "10", "b", "d1"),
        ("agent_memory", "9", "a", "d2"),
        ("task_history", "u", "a", "d3"),
    ] {
        for f in [t, id, a, d] {
            b.extend_from_slice(&(f.len() as u64).to_be_bytes());
            b.extend_from_slice(f.as_bytes());
        }
    }
    assert_eq!(member_set_digest(&rows), sha256_hex(&b));
    assert_eq!(
        member_set_digest(&[]),
        sha256_hex(b"nexus-hm-v1/t0-members"),
        "empty ledger digest is the tag alone"
    );
}

#[test]
fn hm_n2_current_row_matches_scan_and_requires_canonical_ids() {
    let db = NexusDatabase::in_memory().unwrap();
    assert_accepted(
        &db,
        "INSERT INTO agent_memory (id, agent_id, memory_type, key, value_json, created_at, last_accessed)
           VALUES (12, 'ag', 'episodic', 'k', 'v', 'C', 'L')",
    );
    let scanned = scan(&db, "agent_memory");
    let cur = db
        .hm()
        .immediate(|tx| {
            Ok((
                tx.current_row("agent_memory", "12")?,
                tx.current_row("agent_memory", "012")?,
                tx.current_row("agent_memory", "12abc")?,
                tx.current_row("agent_memory", "13")?,
            ))
        })
        .unwrap();
    assert_eq!(cur.0.as_ref(), scanned.first());
    assert_eq!(cur.1, None);
    assert_eq!(cur.2, None);
    assert_eq!(cur.3, None);
    assert!(db.hm().immediate(|tx| tx.scan_table("agents")).is_err());
}

// ── N5: guards (persistence-scope parts of A-S1) ────────────────────────────

use std::path::{Path, PathBuf};

const IN_SCOPE_TABLES: [&str; 5] = [
    "agent_memory",
    "task_history",
    "strategy_scores",
    "world_model_entities",
    "world_model_relationships",
];

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

/// Every source file under the workspace with one of `exts`, skipping build
/// output, dependencies and VCS metadata.
fn source_files(exts: &[&str]) -> Vec<PathBuf> {
    fn walk(dir: &Path, exts: &[&str], out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            let Ok(ft) = entry.file_type() else { continue };
            if ft.is_dir() {
                if matches!(name.as_str(), "target" | "node_modules" | ".git" | "dist")
                    || name.starts_with("target")
                {
                    continue;
                }
                walk(&path, exts, out);
            } else if ft.is_file()
                && path
                    .extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| exts.contains(&e))
            {
                out.push(path);
            }
        }
    }
    let mut out = Vec::new();
    walk(&workspace_root(), exts, &mut out);
    out.sort();
    out
}

fn relative(path: &Path) -> String {
    path.strip_prefix(workspace_root())
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// Lowercase with every whitespace run collapsed to one space.
fn squash(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// The token after each `replace into` (covers `insert or replace into`).
fn replace_targets(squashed: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = squashed;
    while let Some(i) = rest.find("replace into ") {
        let after = &rest[i + "replace into ".len()..];
        let token: String = after
            .chars()
            .take_while(|c| !c.is_whitespace() && *c != '(')
            .collect();
        out.push(
            token
                .trim_matches(|c| c == '"' || c == '`' || c == '[' || c == ']')
                .to_string(),
        );
        rest = after;
    }
    out
}

#[test]
fn hm_n5_no_replace_write_to_an_in_scope_table() {
    let files = source_files(&["rs", "sql"]);
    assert!(files.len() > 100, "workspace scan found too few files");
    let mut hits = Vec::new();
    for f in &files {
        // This file plants REPLACE strings for the scanner's negative control.
        if relative(f) == "persistence/tests/hm_a1_storage.rs" {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(f) else {
            continue;
        };
        let squashed = squash(&text);
        for target in replace_targets(&squashed) {
            let bare = target.strip_prefix("main.").unwrap_or(&target);
            if IN_SCOPE_TABLES.contains(&bare) || bare.starts_with('{') || bare.is_empty() {
                hits.push(format!("{}: REPLACE INTO {target}", relative(f)));
            }
        }
        // A REPLACE conflict clause in a column or table definition.
        if squashed.contains("on conflict replace") {
            for t in IN_SCOPE_TABLES {
                if squashed.contains(&format!("create table if not exists {t} "))
                    || squashed.contains(&format!("create table {t} "))
                {
                    hits.push(format!("{}: ON CONFLICT REPLACE near {t}", relative(f)));
                }
            }
        }
    }
    assert!(hits.is_empty(), "REPLACE on an in-scope table: {hits:#?}");
}

#[test]
fn hm_n5_replace_guard_detects_a_planted_replace() {
    // Negative control for the scanner itself.
    let planted =
        squash("conn.execute(\"INSERT  OR\n REPLACE INTO agent_memory (id) VALUES (1)\")");
    assert_eq!(replace_targets(&planted), vec!["agent_memory".to_string()]);
    let planted = squash("format!(\"REPLACE INTO {table} VALUES (?1)\")");
    assert_eq!(replace_targets(&planted), vec!["{table}".to_string()]);
}

#[test]
fn hm_n5_every_nexus_database_constructor_enables_foreign_keys() {
    // Runtime: both constructors.
    let db = NexusDatabase::in_memory().unwrap();
    assert_eq!(count(&db, "PRAGMA foreign_keys"), 1, "in_memory");
    let dir = std::env::temp_dir().join(format!(
        "hm-a1-n5-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4()
    ));
    let path = dir.join("state.db");
    {
        let db = NexusDatabase::open(&path).unwrap();
        assert_eq!(count(&db, "PRAGMA foreign_keys"), 1, "open (new file)");
    }
    {
        let db = NexusDatabase::open(&path).unwrap();
        assert_eq!(count(&db, "PRAGMA foreign_keys"), 1, "open (existing file)");
    }
    std::fs::remove_dir_all(&dir).unwrap();

    // Source: every place lib.rs builds a NexusDatabase connection is in a
    // function that sets foreign_keys=ON before constructing it.
    let src = std::fs::read_to_string(workspace_root().join("persistence/src/lib.rs")).unwrap();
    let mut constructions = 0;
    let mut start = 0;
    while let Some(i) = src[start..].find("conn: Mutex::new(conn)") {
        let at = start + i;
        let fn_start = src[..at].rfind("\n    pub fn ").expect("enclosing fn");
        let body = &src[fn_start..at];
        let name: String = body["\n    pub fn ".len()..]
            .chars()
            .take_while(|c| *c != '(')
            .collect();
        assert!(
            body.contains("PRAGMA foreign_keys=ON"),
            "NexusDatabase::{name} does not enable foreign keys"
        );
        constructions += 1;
        start = at + 1;
    }
    assert_eq!(
        constructions, 2,
        "open and in_memory are the only constructors"
    );
    let opens = src.matches("Connection::open").count();
    assert_eq!(
        opens, 2,
        "every Connection::open in lib.rs is a checked constructor"
    );
}

/// The ten §5.1 table names.
const HM_TABLE_NAMES: [&str; 10] = [
    "hm_migration",
    "hm_t0_member",
    "hm_t0_tombstone",
    "hm_agent_policy",
    "hm_policy_change",
    "hm_record_label",
    "hm_derivation",
    "hm_grant",
    "hm_dispatch_admission",
    "hm_event",
];

fn hm_module_path(rel: &str) -> bool {
    rel == "persistence/src/hm.rs"
        || rel.starts_with("persistence/src/hm/")
        || rel == "kernel/src/hm.rs"
        || rel.starts_with("kernel/src/hm/")
        || (rel.starts_with("persistence/tests/hm_") && rel.ends_with(".rs"))
        || (rel.starts_with("kernel/tests/hm_") && rel.ends_with(".rs"))
}

/// Whole-word occurrences of `word` (identifier characters as boundaries).
fn has_word(text: &str, word: &str) -> bool {
    let ident = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let mut start = 0;
    while let Some(i) = text[start..].find(word) {
        let at = start + i;
        let before = text[..at].chars().next_back();
        let after = text[at + word.len()..].chars().next();
        if !before.is_some_and(ident) && !after.is_some_and(ident) {
            return true;
        }
        start = at + word.len();
    }
    false
}

#[test]
fn hm_n5_hm_tables_are_named_only_in_the_hm_modules() {
    let files = source_files(&["rs", "sql", "ts", "tsx", "js", "py", "json", "toml"]);
    let mut hits = Vec::new();
    for f in &files {
        let rel = relative(f);
        if hm_module_path(&rel) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(f) else {
            continue;
        };
        for t in HM_TABLE_NAMES {
            if has_word(&text, t) {
                hits.push(format!("{rel}: {t}"));
            }
        }
    }
    assert!(
        hits.is_empty(),
        "hm_* table named outside the hm modules: {hits:#?}"
    );
    // The only lib.rs reference is the accessor, which names no table.
    let lib = std::fs::read_to_string(workspace_root().join("persistence/src/lib.rs")).unwrap();
    assert!(lib.contains("pub fn hm(&self) -> hm::HmStore<'_>"));
}

#[test]
fn hm_n5_table_name_scanner_detects_a_planted_name() {
    assert!(has_word("INSERT INTO hm_grant VALUES (1)", "hm_grant"));
    assert!(has_word("\"hm_event\"", "hm_event"));
    assert!(!has_word("SELECT * FROM xhm_grant", "hm_grant"));
    assert!(!has_word("hm_grants_total", "hm_grant"));
    assert!(!hm_module_path("persistence/src/lib.rs"));
    assert!(hm_module_path("kernel/tests/hm_a1_ledger.rs"));
}

#[test]
fn hm_n5_clear_all_agents_does_not_reset_sqlite_sequence() {
    use nexus_persistence::StateStore;
    let db = db_with_schema();
    for i in 0..3 {
        db.save_memory("a", "episodic", &format!("SYNTH-k{i}"), "\"v\"")
            .unwrap();
    }
    let max_before = count(&db, "SELECT MAX(id) FROM agent_memory");
    let seq_before = count(
        &db,
        "SELECT seq FROM sqlite_sequence WHERE name = 'agent_memory'",
    );
    assert_eq!(seq_before, max_before);
    // Mark one row as a T0 member so the delete trigger tombstones it.
    assert_accepted(
        &db,
        &format!("INSERT INTO hm_t0_member VALUES ('agent_memory', '{max_before}', 'a', 'd')"),
    );
    db.clear_all_agents().unwrap();
    assert_eq!(count(&db, "SELECT COUNT(*) FROM agent_memory"), 0);
    assert_eq!(
        count(
            &db,
            "SELECT seq FROM sqlite_sequence WHERE name = 'agent_memory'"
        ),
        seq_before,
        "sqlite_sequence must survive clear_all_agents"
    );
    assert_eq!(count(&db, "SELECT COUNT(*) FROM hm_t0_tombstone"), 1);
    db.save_memory("a", "episodic", "SYNTH-after", "\"v\"")
        .unwrap();
    assert!(
        count(&db, "SELECT MAX(id) FROM agent_memory") > max_before,
        "ids are never reused after clear_all_agents"
    );

    // Source: clear_all_agents and the rest of the persistence crate never
    // write sqlite_sequence.
    let src = std::fs::read_to_string(workspace_root().join("persistence/src/lib.rs")).unwrap();
    let start = src
        .find("fn clear_all_agents(&self) -> Result<usize> {\n        let conn")
        .expect("clear_all_agents");
    let end = start + src[start..].find("\n    }\n").unwrap();
    assert!(!src[start..end].contains("sqlite_sequence"));
    for f in source_files(&["rs"]) {
        let rel = relative(&f);
        if !rel.starts_with("persistence/src/") {
            continue;
        }
        let text = std::fs::read_to_string(&f).unwrap();
        assert!(
            !text.contains("sqlite_sequence"),
            "{rel} names sqlite_sequence"
        );
    }
}
