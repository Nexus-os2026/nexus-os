//! P3-C11-HM-A1: T₀ migration, ledger checks and member states of the dormant
//! historical-memory module (P3-HM-CONTRACT-02 §9, §18.2, §18.2a).
//!
//! Synthetic databases only: in-memory, or files in a test-owned temporary
//! directory. Crash and restart steps run in child processes of this test
//! binary; readiness is a line on a pipe, never a sleep.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use nexus_kernel::hm::{
    evaluate_label, is_t0_member, run_t0_option_a, run_t0_option_a_probed, verify_at_start,
    EvaluatedLabel, LedgerReport, MemberState, OriginLabel, RecordTable, StartStatus, T0Outcome,
    T0Point,
};
use nexus_persistence::hm::{L4_TRIGGERS, TRG_DEL_AGENT_MEMORY};
use nexus_persistence::{NexusDatabase, StateStore};
use sha2::{Digest, Sha256};
use uuid::Uuid;

// ── fixtures and helpers ────────────────────────────────────────────────────

const AGENT_X: &str = "0a0a0a0a-0000-4000-8000-00000000000a";
const AGENT_Y: &str = "0b0b0b0b-0000-4000-8000-00000000000b";
const AGENT_Z: &str = "0c0c0c0c-0000-4000-8000-00000000000c";
/// A child process waits at most this long for a readiness line (a bound,
/// not a synchronisation).
const CHILD_DEADLINE: Duration = Duration::from_secs(120);

fn uuid(s: &str) -> Uuid {
    Uuid::parse_str(s).unwrap()
}

fn agents() -> Vec<Uuid> {
    vec![uuid(AGENT_X), uuid(AGENT_Y), uuid(AGENT_Z)]
}

fn exec(db: &NexusDatabase, sql: &str) -> Result<(), String> {
    db.with_transaction(|tx| {
        tx.execute_batch(sql)?;
        Ok(())
    })
    .map_err(|e| e.to_string())
}

fn must(db: &NexusDatabase, sql: &str) {
    exec(db, sql).unwrap_or_else(|e| panic!("{e} for: {sql}"));
}

fn count(db: &NexusDatabase, sql: &str) -> i64 {
    db.with_transaction(|tx| Ok(tx.query_row(sql, [], |r| r.get::<_, i64>(0))?))
        .unwrap_or_else(|e| panic!("{e} for: {sql}"))
}

/// One text column of every result row.
fn strings(db: &NexusDatabase, sql: &str) -> Vec<String> {
    db.with_transaction(|tx| {
        let mut stmt = tx.prepare(sql)?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    })
    .unwrap_or_else(|e| panic!("{e} for: {sql}"))
}

fn open(path: &Path) -> NexusDatabase {
    NexusDatabase::open(path).expect("open synthetic database")
}

/// Synthetic pre-T₀ rows in all five in-scope tables (two agents, NULLs,
/// reals, a gap in the ids).
fn seed_fixture(db: &NexusDatabase) {
    must(
        db,
        &format!(
            "INSERT INTO agent_memory (id, agent_id, memory_type, key, value_json, relevance_score, access_count, created_at, last_accessed, expires_at) VALUES
               (1, '{AGENT_X}', 'episodic', 'SYNTH-k1', '\"SYNTH-v1\"', 0.75, 2, '2026-01-01T00:00:01Z', '2026-01-02T00:00:00Z', NULL),
               (2, '{AGENT_X}', 'semantic', 'SYNTH-k2', '\"SYNTH-v2\"', 1.0, 0, '2026-01-01T00:00:02Z', '2026-01-01T00:00:02Z', '2027-01-01T00:00:00Z'),
               (5, '{AGENT_Y}', 'procedural', 'SYNTH-k5', '\"SYNTH-v5\"', 0.125, 9, '2026-01-01T00:00:05Z', '2026-01-03T00:00:00Z', NULL);
             INSERT INTO task_history (id, agent_id, goal, status, steps_json, result_json, fuel_consumed, started_at, completed_at, success) VALUES
               ('11111111-0000-4000-8000-000000000001', '{AGENT_X}', 'SYNTH-goal-1', 'completed', '[\"SYNTH-s\"]', '\"SYNTH-r\"', 3.5, '2026-02-01T00:00:00Z', '2026-02-01T01:00:00Z', 1),
               ('11111111-0000-4000-8000-000000000002', '{AGENT_Y}', 'SYNTH-goal-2', 'running', '[]', NULL, 0.0, '2099-12-31T00:00:00Z', NULL, 0);
             INSERT INTO strategy_scores (id, agent_id, strategy_hash, goal_type, uses, successes, total_fuel, total_duration_secs, composite_score, updated_at) VALUES
               (1, '{AGENT_X}', 'SYNTH-hash', 'SYNTH-type', 2, 1, 1.5, 2.5, 0.5, '2026-03-01T00:00:00Z');
             INSERT INTO world_model_entities (id, agent_id, entity_json, created_at) VALUES
               (1, '{AGENT_X}', '{{\"SYNTH\":\"e1\"}}', '2026-04-01T00:00:00Z'),
               (2, '{AGENT_Y}', '{{\"SYNTH\":\"e2\"}}', '2026-04-01T00:00:00Z');
             INSERT INTO world_model_relationships (id, agent_id, relationship_json, created_at) VALUES
               (1, '{AGENT_X}', '{{\"SYNTH\":\"r1\"}}', '2026-04-01T00:00:00Z');"
        ),
    );
}

const IN_SCOPE: [&str; 5] = [
    "agent_memory",
    "task_history",
    "strategy_scores",
    "world_model_entities",
    "world_model_relationships",
];

/// Per-row SHA-256 over every column (storage class and value) of every
/// in-scope row, keyed `table:id`. Independent of the §5.2 digest.
fn row_hashes(db: &NexusDatabase) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for t in IN_SCOPE {
        let cols = strings(
            db,
            &format!("SELECT name FROM pragma_table_info('{t}') ORDER BY cid"),
        );
        let expr = cols
            .iter()
            .map(|c| {
                format!(
                    "typeof({c}) || ':' || CASE typeof({c}) WHEN 'real' THEN printf('%!.20e', {c}) ELSE hex({c}) END"
                )
            })
            .collect::<Vec<_>>()
            .join(" || '|' || ");
        let sql = format!("SELECT CAST(id AS TEXT) || char(9) || {expr} FROM {t} ORDER BY id");
        for line in strings(db, &sql) {
            let (id, body) = line.split_once('\t').unwrap();
            out.insert(
                format!("{t}:{id}"),
                hex::encode(Sha256::digest(body.as_bytes())),
            );
        }
    }
    out
}

/// Independent §5.2 encoder (test-side).
fn t_content(table: &str, fields: &[Option<&str>]) -> String {
    let mut b: Vec<u8> = format!("nexus-hm-v1/{table}").into_bytes();
    for f in fields {
        match f {
            Some(v) => {
                b.extend_from_slice(&(v.len() as u64).to_be_bytes());
                b.extend_from_slice(v.as_bytes());
            }
            None => b.extend_from_slice(&[0xFF; 8]),
        }
    }
    hex::encode(Sha256::digest(&b))
}

/// Independent §9.5 encoder (test-side); sorts its input.
fn t_member_set(rows: &[(String, String, String, String)]) -> String {
    let mut sorted = rows.to_vec();
    sorted.sort_by(|a, b| (a.0.as_bytes(), a.1.as_bytes()).cmp(&(b.0.as_bytes(), b.1.as_bytes())));
    let mut b: Vec<u8> = b"nexus-hm-v1/t0-members".to_vec();
    for (t, id, a, d) in &sorted {
        for f in [t, id, a, d] {
            b.extend_from_slice(&(f.len() as u64).to_be_bytes());
            b.extend_from_slice(f.as_bytes());
        }
    }
    hex::encode(Sha256::digest(&b))
}

/// Recomputes every in-scope row's §5.2 digest from its column values with
/// the test-side encoder: `(table, id, agent_id, digest)`.
fn t_expected_members(db: &NexusDatabase) -> Vec<(String, String, String, String)> {
    let digest_cols: [(&str, &[&str]); 5] = [
        (
            "agent_memory",
            &["agent_id", "memory_type", "key", "value_json", "created_at"],
        ),
        (
            "task_history",
            &[
                "id",
                "agent_id",
                "goal",
                "steps_json",
                "result_json",
                "started_at",
            ],
        ),
        (
            "strategy_scores",
            &["agent_id", "strategy_hash", "goal_type"],
        ),
        (
            "world_model_entities",
            &["agent_id", "entity_json", "created_at"],
        ),
        (
            "world_model_relationships",
            &["agent_id", "relationship_json", "created_at"],
        ),
    ];
    let mut out = Vec::new();
    for (t, cols) in digest_cols {
        let sel = cols.join(", ");
        let rows: Vec<(String, String, Vec<Option<String>>)> = db
            .with_transaction(|tx| {
                let mut stmt = tx.prepare(&format!(
                    "SELECT CAST(id AS TEXT), agent_id, {sel} FROM {t}"
                ))?;
                let rows = stmt.query_map([], |r| {
                    let mut vals = Vec::new();
                    for i in 0..cols.len() {
                        vals.push(r.get::<_, Option<String>>(i + 2)?);
                    }
                    Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, vals))
                })?;
                let mut v = Vec::new();
                for row in rows {
                    v.push(row?);
                }
                Ok(v)
            })
            .unwrap();
        for (id, agent, vals) in rows {
            let refs: Vec<Option<&str>> = vals.iter().map(|v| v.as_deref()).collect();
            out.push((t.to_string(), id, agent, t_content(t, &refs)));
        }
    }
    out
}

fn ledger(db: &NexusDatabase) -> Vec<(String, String, String, String)> {
    db.with_transaction(|tx| {
        let mut stmt = tx.prepare(
            "SELECT record_table, record_id, agent_id, t0_content_sha256 FROM hm_t0_member
             ORDER BY record_table, record_id",
        )?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
        let mut v = Vec::new();
        for row in rows {
            v.push(row?);
        }
        Ok(v)
    })
    .unwrap()
}

fn sorted(mut v: Vec<(String, String, String, String)>) -> Vec<(String, String, String, String)> {
    v.sort();
    v
}

fn report(db: &NexusDatabase) -> LedgerReport {
    match verify_at_start(db).expect("verify_at_start") {
        StartStatus::Verified(r) => r,
        StartStatus::NotMigrated => panic!("expected a migrated store"),
    }
}

fn state_of(r: &LedgerReport, table: RecordTable, id: &str) -> Option<MemberState> {
    r.members
        .iter()
        .find(|m| m.table == table && m.record_id == id)
        .map(|m| m.state)
}

fn present_unchanged() -> MemberState {
    MemberState::Present {
        label: EvaluatedLabel::Bound(OriginLabel::LegacyUnknown),
        unchanged_since_t0: true,
    }
}

/// `(subject, kind, agent_id, fields)` of every event.
fn events(db: &NexusDatabase) -> Vec<(String, String, Option<String>, String)> {
    db.with_transaction(|tx| {
        let mut stmt =
            tx.prepare("SELECT subject, kind, agent_id, fields FROM hm_event ORDER BY n")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
        let mut v = Vec::new();
        for row in rows {
            v.push(row?);
        }
        Ok(v)
    })
    .unwrap()
}

/// Every event is content-free: no synthetic marker, agent id or digest of a
/// record appears in any field; observational subjects are `obs:<kind>:<32 hex>`.
fn assert_events_content_free(db: &NexusDatabase) {
    for (subject, kind, agent_id, fields) in events(db) {
        assert!(agent_id.is_none(), "{kind}: no agent id expected");
        for needle in ["SYNTH", AGENT_X, AGENT_Y, AGENT_Z] {
            assert!(
                !fields.contains(needle),
                "{kind} fields leak {needle}: {fields}"
            );
            assert!(!subject.contains(needle), "{kind} subject leaks {needle}");
        }
        let v: serde_json::Value = serde_json::from_str(&fields).unwrap();
        let keys: BTreeSet<&str> = v.as_object().unwrap().keys().map(|k| k.as_str()).collect();
        match kind.as_str() {
            "memory.migration_t0" => {
                assert_eq!(subject, "migration:t0");
                assert_eq!(
                    keys,
                    BTreeSet::from(["rows", "member_count", "member_set_sha256"])
                );
                let tables: BTreeSet<&str> = v["rows"]
                    .as_object()
                    .unwrap()
                    .keys()
                    .map(|k| k.as_str())
                    .collect();
                assert_eq!(tables, BTreeSet::from(IN_SCOPE));
            }
            "memory.member_set_mismatch" | "memory.member_record_inconsistent" => {
                let prefix = format!("obs:{kind}:");
                let id = subject
                    .strip_prefix(&prefix)
                    .expect("observational subject");
                assert_eq!(id.len(), 32, "128-bit id");
                assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
                let allowed: BTreeSet<&str> = if kind == "memory.member_set_mismatch" {
                    BTreeSet::from(["failed_checks"])
                } else {
                    BTreeSet::from(["missing_untombstoned", "reappeared"])
                };
                assert_eq!(keys, allowed, "{kind} fields");
            }
            other => panic!("unexpected event kind {other}"),
        }
    }
}

fn event_count(db: &NexusDatabase, kind: &str) -> i64 {
    count(
        db,
        &format!("SELECT COUNT(*) FROM hm_event WHERE kind = '{kind}'"),
    )
}

fn hm_objects(db: &NexusDatabase) -> i64 {
    count(
        db,
        "SELECT COUNT(*) FROM sqlite_master
         WHERE name LIKE 'hm\\_%' ESCAPE '\\' OR tbl_name LIKE 'hm\\_%' ESCAPE '\\'",
    )
}

// ── N3: A-M1 ────────────────────────────────────────────────────────────────

#[test]
fn hm_n3_a_m1_rows_unchanged_labelled_ledgered_and_second_start_noop() {
    let dir = tempfile::tempdir().unwrap();
    let db = open(&dir.path().join("state.db"));
    seed_fixture(&db);
    let before = row_hashes(&db);
    let expected = sorted(t_expected_members(&db));
    assert_eq!(expected.len(), 9);
    assert_eq!(verify_at_start(&db).unwrap(), StartStatus::NotMigrated);

    let outcome = run_t0_option_a(&db, &agents()).unwrap();
    let expected_digest = t_member_set(&expected);
    assert_eq!(
        outcome,
        T0Outcome::Migrated {
            member_count: 9,
            member_set_sha256: expected_digest.clone()
        }
    );
    // Per-row SHA-256 identical before and after.
    assert_eq!(row_hashes(&db), before);
    // Ledger = exactly the pre-T0 rows, with independently computed digests.
    assert_eq!(ledger(&db), expected);
    assert_eq!(count(&db, "SELECT member_count FROM hm_migration"), 9);
    assert_eq!(
        strings(&db, "SELECT member_set_sha256 FROM hm_migration"),
        vec![expected_digest.clone()]
    );
    assert_eq!(
        strings(
            &db,
            "SELECT contract_version || '|' || state FROM hm_migration"
        ),
        vec!["P3-HM-CONTRACT-02|COMPLETE".to_string()]
    );
    // Every row LEGACY_UNKNOWN, source_kind legacy, generation 0, bound.
    let labels = strings(
        &db,
        "SELECT record_table || '|' || record_id || '|' || agent_id || '|' || content_sha256 || '|' || origin || '|' || source_kind || '|' || policy_generation
         FROM hm_record_label ORDER BY record_table, record_id",
    );
    let want: Vec<String> = expected
        .iter()
        .map(|(t, id, a, d)| format!("{t}|{id}|{a}|{d}|LEGACY_UNKNOWN|legacy|0"))
        .collect();
    assert_eq!(labels, want);
    for (t, id, _, _) in &expected {
        let table = RecordTable::ALL
            .into_iter()
            .find(|x| x.table_name() == t)
            .unwrap();
        assert_eq!(
            evaluate_label(&db, table, id).unwrap(),
            EvaluatedLabel::Bound(OriginLabel::LegacyUnknown)
        );
        assert!(is_t0_member(&db, table, id).unwrap());
    }
    let r = report(&db);
    assert!(r.checks.all_pass(), "{:?}", r.checks);
    assert_eq!(r.members.len(), 9);
    assert!(r.members.iter().all(|m| m.state == present_unchanged()));
    assert!(!r.mismatch_event_written && !r.inconsistency_event_written);

    // Second start: a no-op.
    let labels_before = labels.clone();
    assert_eq!(
        run_t0_option_a(&db, &agents()).unwrap(),
        T0Outcome::AlreadyComplete
    );
    drop(db);
    let db = open(&dir.path().join("state.db"));
    assert_eq!(
        run_t0_option_a(&db, &agents()).unwrap(),
        T0Outcome::AlreadyComplete
    );
    assert_eq!(row_hashes(&db), before);
    assert_eq!(ledger(&db), expected);
    assert_eq!(
        strings(
            &db,
            "SELECT record_table || '|' || record_id || '|' || agent_id || '|' || content_sha256 || '|' || origin || '|' || source_kind || '|' || policy_generation
             FROM hm_record_label ORDER BY record_table, record_id",
        ),
        labels_before
    );
    assert_eq!(count(&db, "SELECT COUNT(*) FROM hm_migration"), 1);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM hm_agent_policy"), 3);
    assert_eq!(event_count(&db, "memory.migration_t0"), 1);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM hm_event"), 1);
    // The event carries counts and the member-set digest only.
    let ev = events(&db);
    let fields: serde_json::Value = serde_json::from_str(&ev[0].3).unwrap();
    assert_eq!(fields["member_count"], 9);
    assert_eq!(fields["member_set_sha256"], expected_digest.as_str());
    assert_eq!(fields["rows"]["agent_memory"], 3);
    assert_eq!(fields["rows"]["task_history"], 2);
    assert_eq!(fields["rows"]["strategy_scores"], 1);
    assert_eq!(fields["rows"]["world_model_entities"], 2);
    assert_eq!(fields["rows"]["world_model_relationships"], 1);
    assert_events_content_free(&db);
}

#[test]
fn hm_n3_a_m1_empty_database_migrates_with_empty_ledger() {
    let db = NexusDatabase::in_memory().unwrap();
    let out = run_t0_option_a(&db, &[]).unwrap();
    assert_eq!(
        out,
        T0Outcome::Migrated {
            member_count: 0,
            member_set_sha256: t_member_set(&[])
        }
    );
    let r = report(&db);
    assert!(r.checks.all_pass());
    assert!(r.members.is_empty());
    assert_eq!(count(&db, "SELECT COUNT(*) FROM hm_agent_policy"), 0);
}

// ── N3: Option A baseline ───────────────────────────────────────────────────

#[test]
fn hm_n3_option_a_every_registered_agent_local_only_with_t0_baseline_history() {
    let db = NexusDatabase::in_memory().unwrap();
    seed_fixture(&db);
    // Z is registered but owns no row; a duplicate is written once. The
    // agents owning rows are not inferred: an unregistered owner gets no row.
    let registered = vec![uuid(AGENT_Z), uuid(AGENT_X), uuid(AGENT_X)];
    run_t0_option_a(&db, &registered).unwrap();
    assert_eq!(
        strings(
            &db,
            "SELECT agent_id || '|' || local_only || '|' || policy_generation || '|' || revocation_seq || '|' || basis
             FROM hm_agent_policy ORDER BY agent_id"
        ),
        vec![
            format!("{AGENT_X}|1|1|0|t0_baseline"),
            format!("{AGENT_Z}|1|1|0|t0_baseline"),
        ]
    );
    assert_eq!(
        strings(
            &db,
            "SELECT agent_id || '|' || policy_generation || '|' || local_only || '|' || basis || '|' || cause
             FROM hm_policy_change ORDER BY agent_id"
        ),
        vec![
            format!("{AGENT_X}|1|1|t0_baseline|t0_baseline"),
            format!("{AGENT_Z}|1|1|t0_baseline|t0_baseline"),
        ]
    );
    assert_eq!(
        count(
            &db,
            &format!("SELECT COUNT(*) FROM hm_agent_policy WHERE agent_id = '{AGENT_Y}'")
        ),
        0
    );
    // No grant, derivation or admission row is written (AD-4).
    for t in [
        "hm_grant",
        "hm_derivation",
        "hm_dispatch_admission",
        "hm_t0_tombstone",
    ] {
        assert_eq!(count(&db, &format!("SELECT COUNT(*) FROM {t}")), 0, "{t}");
    }
}

#[test]
fn hm_n3_option_a_agent_ids_are_canonical_lowercase() {
    let db = NexusDatabase::in_memory().unwrap();
    let upper = Uuid::parse_str("ABCDEFAB-0000-4000-8000-0000000000AB").unwrap();
    run_t0_option_a(&db, &[upper]).unwrap();
    assert_eq!(
        strings(&db, "SELECT agent_id FROM hm_agent_policy"),
        vec!["abcdefab-0000-4000-8000-0000000000ab".to_string()]
    );
}

// ── N3: A-M2 (crash inside T₀) ──────────────────────────────────────────────

const READY_PREFIX: &str = "HM-A1-READY ";

/// Spawns this test binary as a child running one ignored entry test.
fn spawn_child(entry: &str, envs: &[(&str, String)]) -> std::process::Child {
    let mut cmd = Command::new(std::env::current_exe().unwrap());
    cmd.args([
        "--exact",
        entry,
        "--ignored",
        "--nocapture",
        "--test-threads=1",
    ])
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::inherit());
    for (k, v) in envs {
        cmd.env(k, v);
    }
    cmd.spawn().expect("spawn child")
}

/// Reads the child's stdout on a thread and returns the lines it sends.
fn line_channel(child: &mut std::process::Child) -> mpsc::Receiver<String> {
    let stdout = child.stdout.take().unwrap();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    rx
}

/// Waits (bounded) for the first line containing `prefix` and returns it
/// from the prefix on. libtest prints `test <name> ... ` without a newline
/// before the test body runs, so the marker can arrive mid-line.
fn wait_line(rx: &mpsc::Receiver<String>, prefix: &str) -> String {
    loop {
        match rx.recv_timeout(CHILD_DEADLINE) {
            Ok(line) => {
                if let Some(i) = line.find(prefix) {
                    return line[i..].to_string();
                }
            }
            Err(e) => panic!("child sent no {prefix:?} line: {e}"),
        }
    }
}

/// Child entry for A-M2: runs T₀ and, at the named point, announces
/// readiness and blocks on stdin. It never commits: if stdin closes it
/// exits without returning from the probe.
#[test]
#[ignore = "child process entry for A-M2; run by the parent test"]
fn hm_child_t0_kill_point() {
    let Ok(path) = std::env::var("HM_A1_CHILD_DB") else {
        return;
    };
    let point = std::env::var("HM_A1_CHILD_POINT").unwrap();
    let db = open(Path::new(&path));
    let mut probe = |p: T0Point| {
        if p.name() == point {
            println!("{READY_PREFIX}{}", p.name());
            std::io::stdout().flush().unwrap();
            let mut buf = String::new();
            let _ = std::io::stdin().read_line(&mut buf);
            std::process::exit(3);
        }
    };
    let _ = run_t0_option_a_probed(&db, &agents(), &mut probe);
    println!("HM-A1-UNEXPECTED-COMMIT");
    std::process::exit(4);
}

#[test]
fn hm_n3_a_m2_child_killed_inside_t0_leaves_no_trace_and_restart_completes() {
    for point in T0Point::ALL {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.db");
        {
            let db = open(&path);
            seed_fixture(&db);
        }
        let before = row_hashes(&open(&path));

        let mut child = spawn_child(
            "hm_child_t0_kill_point",
            &[
                ("HM_A1_CHILD_DB", path.display().to_string()),
                ("HM_A1_CHILD_POINT", point.name().to_string()),
            ],
        );
        let rx = line_channel(&mut child);
        let line = wait_line(&rx, READY_PREFIX);
        assert_eq!(line, format!("{READY_PREFIX}{}", point.name()));
        child.kill().expect("kill child");
        let status = child.wait().unwrap();
        assert!(
            !status.success(),
            "{}: child must die by the kill",
            point.name()
        );

        // No hm_* table, trigger or index; no label; rows unchanged.
        let db = open(&path);
        assert_eq!(
            hm_objects(&db),
            0,
            "{}: hm_* objects survived",
            point.name()
        );
        assert_eq!(verify_at_start(&db).unwrap(), StartStatus::NotMigrated);
        assert_eq!(
            evaluate_label(&db, RecordTable::AgentMemory, "1").unwrap(),
            EvaluatedLabel::Unbound
        );
        assert_eq!(row_hashes(&db), before, "{}", point.name());

        // Restart completes.
        let out = run_t0_option_a(&db, &agents()).unwrap();
        assert!(matches!(
            out,
            T0Outcome::Migrated {
                member_count: 9,
                ..
            }
        ));
        let r = report(&db);
        assert!(r.checks.all_pass(), "{}: {:?}", point.name(), r.checks);
        assert_eq!(row_hashes(&db), before);
    }
}

// ── N3: A-M3 (exact membership) ─────────────────────────────────────────────

#[test]
fn hm_n3_a_m3_concurrent_writer_blocked_during_t0_and_its_row_not_a_member() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.db");
    let db1 = open(&path);
    seed_fixture(&db1);
    let db2 = open(&path);
    // The second connection fails fast instead of waiting 5 s per attempt.
    db2.with_transaction(|tx| {
        tx.busy_timeout(Duration::from_millis(50))?;
        Ok(())
    })
    .unwrap();

    let (at_tx, at_rx) = mpsc::channel::<T0Point>();
    let (go_tx, go_rx) = mpsc::channel::<()>();
    let t0 = std::thread::spawn(move || {
        let mut probe = |p: T0Point| {
            at_tx.send(p).unwrap();
            go_rx.recv().unwrap();
        };
        run_t0_option_a_probed(&db1, &agents(), &mut probe).map(|o| (o, db1))
    });

    let insert = |key: &str| {
        exec(
            &db2,
            &format!(
                "INSERT INTO agent_memory (agent_id, memory_type, key, value_json, created_at, last_accessed)
                 VALUES ('{AGENT_X}', 'episodic', '{key}', '\"v\"', '2000-01-01T00:00:00Z', 't')"
            ),
        )
    };
    let mut seen = Vec::new();
    for _ in T0Point::ALL {
        let p = at_rx.recv_timeout(CHILD_DEADLINE).expect("T0 point");
        let err = insert("SYNTH-concurrent").expect_err("a write during T0 must be blocked");
        assert!(
            err.contains("locked") || err.contains("busy"),
            "{}: {err}",
            p.name()
        );
        seen.push(p);
        go_tx.send(()).unwrap();
    }
    assert_eq!(seen, T0Point::ALL.to_vec());
    let (out, db1) = t0.join().unwrap().unwrap();
    assert!(matches!(
        out,
        T0Outcome::Migrated {
            member_count: 9,
            ..
        }
    ));

    // After T0 the same write succeeds and is not a member.
    insert("SYNTH-after").expect("write after T0");
    let new_id = count(&db1, "SELECT MAX(id) FROM agent_memory");
    assert_eq!(
        count(
            &db1,
            "SELECT COUNT(*) FROM agent_memory WHERE key = 'SYNTH-concurrent'"
        ),
        0
    );
    assert!(!is_t0_member(&db1, RecordTable::AgentMemory, &new_id.to_string()).unwrap());
    assert_eq!(
        evaluate_label(&db1, RecordTable::AgentMemory, &new_id.to_string()).unwrap(),
        EvaluatedLabel::Unbound
    );
    assert_eq!(count(&db1, "SELECT COUNT(*) FROM hm_t0_member"), 9);
    let r = report(&db1);
    assert!(r.checks.all_pass());
    assert_eq!(r.members.len(), 9);
}

#[test]
fn hm_n3_a_m3_membership_never_from_timestamp_or_id_watermark() {
    let db = NexusDatabase::in_memory().unwrap();
    seed_fixture(&db);
    run_t0_option_a(&db, &agents()).unwrap();
    // Post-T0 rows: ids inside the pre-T0 id range (gap 3, 4 below MAX 5), and
    // a UUID-keyed row with a clock earlier than every pre-T0 row.
    must(
        &db,
        &format!(
            "INSERT INTO agent_memory (id, agent_id, memory_type, key, value_json, created_at, last_accessed) VALUES
               (3, '{AGENT_X}', 'episodic', 'SYNTH-post3', '\"v\"', '1999-01-01T00:00:00Z', 't'),
               (4, '{AGENT_Y}', 'episodic', 'SYNTH-post4', '\"v\"', '1999-01-01T00:00:00Z', 't');
             INSERT INTO task_history (id, agent_id, goal, status, steps_json, started_at) VALUES
               ('00000000-0000-4000-8000-000000000000', '{AGENT_X}', 'SYNTH-post-goal', 'running', '[]', '1970-01-01T00:00:00Z');
             INSERT INTO world_model_entities (id, agent_id, entity_json, created_at) VALUES
               (0, '{AGENT_X}', '{{}}', '1970-01-01T00:00:00Z');"
        ),
    );
    for (t, id) in [
        (RecordTable::AgentMemory, "3"),
        (RecordTable::AgentMemory, "4"),
        (
            RecordTable::TaskHistory,
            "00000000-0000-4000-8000-000000000000",
        ),
        (RecordTable::WorldModelEntities, "0"),
    ] {
        assert!(!is_t0_member(&db, t, id).unwrap(), "{t:?}:{id} is post-T0");
        assert_eq!(evaluate_label(&db, t, id).unwrap(), EvaluatedLabel::Unbound);
    }
    // A pre-T0 row with a later clock than T0 itself is a member.
    for (t, id) in [
        (
            RecordTable::TaskHistory,
            "11111111-0000-4000-8000-000000000002",
        ),
        (RecordTable::AgentMemory, "5"),
        (RecordTable::AgentMemory, "1"),
    ] {
        assert!(is_t0_member(&db, t, id).unwrap(), "{t:?}:{id} is pre-T0");
    }
    // An upserted pre-T0 aggregate leaves the unchanged set; its ledger row stays.
    let ledger_before = ledger(&db);
    db.upsert_strategy_score(AGENT_X, "SYNTH-hash", "SYNTH-type-2", true, 1.0, 1.0)
        .unwrap();
    let r = report(&db);
    assert!(r.checks.all_pass());
    assert_eq!(r.members.len(), 9, "post-T0 rows never join the ledger");
    assert_eq!(
        state_of(&r, RecordTable::StrategyScores, "1"),
        Some(MemberState::Present {
            label: EvaluatedLabel::Unbound,
            unchanged_since_t0: false
        })
    );
    assert_eq!(ledger(&db), ledger_before);
}

// ── N3: ledger checks L1–L4 ─────────────────────────────────────────────────

fn migrated_db() -> NexusDatabase {
    let db = NexusDatabase::in_memory().unwrap();
    seed_fixture(&db);
    run_t0_option_a(&db, &agents()).unwrap();
    db
}

fn assert_only_failed(db: &NexusDatabase, failed: &[&str]) {
    let r = report(db);
    assert_eq!(r.checks.failed(), failed.to_vec(), "{:?}", r.checks);
    assert!(!r.checks.all_pass());
    assert!(r.mismatch_event_written);
    let ev = events(db);
    let mism: Vec<_> = ev
        .iter()
        .filter(|e| e.1 == "memory.member_set_mismatch")
        .collect();
    assert_eq!(mism.len(), 1);
    let v: serde_json::Value = serde_json::from_str(&mism[0].3).unwrap();
    assert_eq!(v["failed_checks"], serde_json::json!(failed));
    assert_events_content_free(db);
}

#[test]
fn hm_n3_ledger_checks_all_pass_on_a_clean_migration() {
    let db = migrated_db();
    let r = report(&db);
    assert!(r.checks.all_pass());
    assert!(r.checks.failed().is_empty());
    assert!(!r.mismatch_event_written);
    assert_eq!(event_count(&db, "memory.member_set_mismatch"), 0);
}

#[test]
fn hm_n3_l1_count_mismatch_detected() {
    let db = migrated_db();
    must(
        &db,
        "UPDATE hm_migration SET member_count = member_count + 1",
    );
    assert_only_failed(&db, &["L1"]);
}

#[test]
fn hm_n3_l2_digest_mismatch_detected() {
    let db = migrated_db();
    must(&db, "UPDATE hm_migration SET member_set_sha256 = '00'");
    assert_only_failed(&db, &["L2"]);
}

#[test]
fn hm_n3_l3_orphan_tombstone_detected() {
    let db = migrated_db();
    db.execute_raw("PRAGMA foreign_keys=OFF").unwrap();
    must(
        &db,
        "INSERT INTO hm_t0_tombstone VALUES ('agent_memory', '999')",
    );
    db.execute_raw("PRAGMA foreign_keys=ON").unwrap();
    assert_only_failed(&db, &["L3"]);
}

#[test]
fn hm_n3_l4_missing_or_altered_trigger_detected() {
    for (name, _) in L4_TRIGGERS {
        let db = migrated_db();
        must(&db, &format!("DROP TRIGGER {name}"));
        assert_only_failed(&db, &["L4"]);
    }
    // Present but with a different definition.
    let db = migrated_db();
    must(
        &db,
        "DROP TRIGGER hm_del_agent_memory;
         CREATE TRIGGER hm_del_agent_memory AFTER DELETE ON agent_memory BEGIN SELECT 1; END;",
    );
    assert_only_failed(&db, &["L4"]);
}

// ── N3: the four per-member states ──────────────────────────────────────────

#[test]
fn hm_n3_member_states_present_deleted_missing_untombstoned_reappeared() {
    let db = migrated_db();
    // deleted: through the delete trigger.
    must(&db, "DELETE FROM agent_memory WHERE id = 1");
    // missing-untombstoned: removed out of band, trigger restored byte-exact.
    must(
        &db,
        "DROP TRIGGER hm_del_agent_memory; DELETE FROM agent_memory WHERE id = 2;",
    );
    must(&db, TRG_DEL_AGENT_MEMORY);
    // reappeared: deleted (tombstoned), then a row with the same identity.
    must(
        &db,
        &format!(
            "DELETE FROM world_model_entities WHERE id = 1;
             INSERT INTO world_model_entities (id, agent_id, entity_json, created_at)
               VALUES (1, '{AGENT_X}', '{{\"SYNTH\":\"again\"}}', '2026-05-01T00:00:00Z');"
        ),
    );
    // present but changed: a content update without a label update.
    must(&db, "UPDATE world_model_relationships SET relationship_json = '{\"SYNTH\":\"new\"}' WHERE id = 1");

    let r = report(&db);
    assert!(r.checks.all_pass(), "{:?}", r.checks);
    assert!(!r.mismatch_event_written);
    assert_eq!(
        state_of(&r, RecordTable::AgentMemory, "1"),
        Some(MemberState::Deleted)
    );
    assert_eq!(
        state_of(&r, RecordTable::AgentMemory, "2"),
        Some(MemberState::MissingUntombstoned)
    );
    assert_eq!(
        state_of(&r, RecordTable::WorldModelEntities, "1"),
        Some(MemberState::Reappeared {
            label: EvaluatedLabel::Unbound
        })
    );
    assert_eq!(
        state_of(&r, RecordTable::WorldModelRelationships, "1"),
        Some(MemberState::Present {
            label: EvaluatedLabel::Unbound,
            unchanged_since_t0: false
        })
    );
    assert_eq!(
        state_of(&r, RecordTable::AgentMemory, "5"),
        Some(present_unchanged())
    );
    assert_eq!(r.members.len(), 9);
    // Inconsistency is audited with counts only.
    assert!(r.inconsistency_event_written);
    let ev: Vec<_> = events(&db)
        .into_iter()
        .filter(|e| e.1 == "memory.member_record_inconsistent")
        .collect();
    assert_eq!(ev.len(), 1);
    let v: serde_json::Value = serde_json::from_str(&ev[0].3).unwrap();
    assert_eq!(v["missing_untombstoned"], 1);
    assert_eq!(v["reappeared"], 1);
    assert_events_content_free(&db);
    // The ledger itself never changed.
    assert_eq!(count(&db, "SELECT COUNT(*) FROM hm_t0_member"), 9);
}

#[test]
fn hm_n3_reappeared_member_is_evaluated_by_its_own_binding_label() {
    let db = migrated_db();
    must(
        &db,
        &format!(
            "DELETE FROM agent_memory WHERE id = 5;
             INSERT INTO agent_memory (id, agent_id, memory_type, key, value_json, created_at, last_accessed)
               VALUES (5, '{AGENT_Y}', 'episodic', 'SYNTH-new5', '\"v\"', '2026-06-01T00:00:00Z', 't');"
        ),
    );
    // A post-T0 labelled write (simulated) binds the new row.
    let digest = t_content(
        "agent_memory",
        &[
            Some(AGENT_Y),
            Some("episodic"),
            Some("SYNTH-new5"),
            Some("\"v\""),
            Some("2026-06-01T00:00:00Z"),
        ],
    );
    must(
        &db,
        &format!(
            "INSERT INTO hm_record_label VALUES ('agent_memory', '5', '{AGENT_Y}', 'LOCAL_ONLY_ORIGIN', 'loop_episodic', '{digest}', 1)"
        ),
    );
    let r = report(&db);
    assert_eq!(
        state_of(&r, RecordTable::AgentMemory, "5"),
        Some(MemberState::Reappeared {
            label: EvaluatedLabel::Bound(OriginLabel::LocalOnlyOrigin)
        })
    );
    assert!(r.inconsistency_event_written);
}

// ── N3: evaluated-label rule ────────────────────────────────────────────────

#[test]
fn hm_n3_evaluated_label_missing_malformed_or_mismatched_is_unbound() {
    let db = NexusDatabase::in_memory().unwrap();
    seed_fixture(&db);
    // Before T0 every record is UNBOUND.
    assert_eq!(
        evaluate_label(&db, RecordTable::AgentMemory, "1").unwrap(),
        EvaluatedLabel::Unbound
    );
    run_t0_option_a(&db, &agents()).unwrap();
    let bound = EvaluatedLabel::Bound(OriginLabel::LegacyUnknown);
    assert_eq!(
        evaluate_label(&db, RecordTable::AgentMemory, "1").unwrap(),
        bound
    );

    // missing label row
    must(
        &db,
        "DELETE FROM hm_record_label WHERE record_table = 'agent_memory' AND record_id = '1'",
    );
    // malformed origin (CHECK bypassed in the fixture)
    db.execute_raw("PRAGMA ignore_check_constraints=ON")
        .unwrap();
    must(&db, "UPDATE hm_record_label SET origin = 'PUBLIC' WHERE record_table = 'agent_memory' AND record_id = '2'");
    db.execute_raw("PRAGMA ignore_check_constraints=OFF")
        .unwrap();
    // label agent differs from the record's
    must(&db, &format!("UPDATE hm_record_label SET agent_id = '{AGENT_Z}' WHERE record_table = 'agent_memory' AND record_id = '5'"));
    // label digest differs from the record's
    must(&db, "UPDATE hm_record_label SET content_sha256 = 'ff' WHERE record_table = 'strategy_scores' AND record_id = '1'");
    // record content changed after labelling
    must(&db, "UPDATE task_history SET goal = 'SYNTH-changed' WHERE id = '11111111-0000-4000-8000-000000000001'");

    for (t, id) in [
        (RecordTable::AgentMemory, "1"),
        (RecordTable::AgentMemory, "2"),
        (RecordTable::AgentMemory, "5"),
        (RecordTable::StrategyScores, "1"),
        (
            RecordTable::TaskHistory,
            "11111111-0000-4000-8000-000000000001",
        ),
        (RecordTable::AgentMemory, "777"),
        (RecordTable::AgentMemory, "01"),
    ] {
        let l = evaluate_label(&db, t, id).unwrap();
        assert_eq!(l, EvaluatedLabel::Unbound, "{t:?}:{id}");
        assert_eq!(l.effective(), OriginLabel::LocalOnlyOrigin);
    }
    // Untouched rows still bind.
    assert_eq!(
        evaluate_label(&db, RecordTable::WorldModelEntities, "2").unwrap(),
        bound
    );
    // A label with no record is not a binding either.
    must(&db, "DELETE FROM world_model_entities WHERE id = 2");
    assert_eq!(
        evaluate_label(&db, RecordTable::WorldModelEntities, "2").unwrap(),
        EvaluatedLabel::Unbound
    );
}

#[test]
fn hm_n3_origin_order_join_and_effective_label() {
    use OriginLabel::*;
    assert!(UnrestrictedOrigin < LegacyUnknown && LegacyUnknown < LocalOnlyOrigin);
    assert_eq!(UnrestrictedOrigin.join(LegacyUnknown), LegacyUnknown);
    assert_eq!(LocalOnlyOrigin.join(UnrestrictedOrigin), LocalOnlyOrigin);
    assert_eq!(LegacyUnknown.join(LegacyUnknown), LegacyUnknown);
    assert_eq!(EvaluatedLabel::Unbound.effective(), LocalOnlyOrigin);
    assert_eq!(
        EvaluatedLabel::Bound(UnrestrictedOrigin).effective(),
        UnrestrictedOrigin
    );
    assert_eq!(
        [UnrestrictedOrigin, LegacyUnknown, LocalOnlyOrigin].map(OriginLabel::class_name),
        ["UNRESTRICTED_ORIGIN", "LEGACY_UNKNOWN", "LOCAL_ONLY_ORIGIN"]
    );
}

// ── N4: A-M4 (§18.2a), AD-4 form ────────────────────────────────────────────
//
// AD-4 is unsigned: steps 3, 4 and 9b (grants) are removed, and "bulk grants
// inert" is read as "no bulk scope is evaluated"; the L1–L4 result and the
// events are asserted instead. The scenario runs once with foreign_keys ON
// and once OFF; the two summaries must be identical.

const AM4_PREFIX: &str = "HM-A1-AM4 ";
const T1: &str = "22222222-0000-4000-8000-000000000001";

/// Opens the synthetic database with the requested foreign-key mode and
/// checks that the mode took effect.
fn open_mode(path: &Path, fk_on: bool) -> NexusDatabase {
    let db = open(path);
    if !fk_on {
        db.execute_raw("PRAGMA foreign_keys=OFF").unwrap();
    }
    assert_eq!(count(&db, "PRAGMA foreign_keys"), i64::from(fk_on));
    db
}

/// A-M4 fixture: X owns m1 m2 m3 (agent_memory 1-3), t1 (task_history),
/// k1 k2 (world_model_entities 1-2), k3 (world_model_relationships 1);
/// Y owns y1 (agent_memory 4); Z owns z1 (agent_memory 5).
fn seed_am4(db: &NexusDatabase) {
    must(
        db,
        &format!(
            "INSERT INTO agent_memory (id, agent_id, memory_type, key, value_json, relevance_score, access_count, created_at, last_accessed, expires_at) VALUES
               (1, '{AGENT_X}', 'episodic', 'SYNTH-m1', '\"SYNTH-m1\"', 1.0, 0, '2026-01-01T00:00:01Z', '2026-01-01T00:00:01Z', NULL),
               (2, '{AGENT_X}', 'episodic', 'SYNTH-m2', '\"SYNTH-m2\"', 1.0, 0, '2026-01-01T00:00:02Z', '2026-01-01T00:00:02Z', NULL),
               (3, '{AGENT_X}', 'semantic', 'SYNTH-m3', '\"SYNTH-m3\"', 0.5, 3, '2026-01-01T00:00:03Z', '2026-01-02T00:00:00Z', NULL),
               (4, '{AGENT_Y}', 'episodic', 'SYNTH-y1', '\"SYNTH-y1\"', 1.0, 0, '2026-01-01T00:00:04Z', '2026-01-01T00:00:04Z', NULL),
               (5, '{AGENT_Z}', 'episodic', 'SYNTH-z1', '\"SYNTH-z1\"', 1.0, 0, '2026-01-01T00:00:05Z', '2026-01-01T00:00:05Z', NULL);
             INSERT INTO task_history (id, agent_id, goal, status, steps_json, result_json, fuel_consumed, started_at, completed_at, success) VALUES
               ('{T1}', '{AGENT_X}', 'SYNTH-t1', 'running', '[\"SYNTH-step\"]', NULL, 0.0, '2026-01-01T00:00:06Z', NULL, 0);
             INSERT INTO world_model_entities (id, agent_id, entity_json, created_at) VALUES
               (1, '{AGENT_X}', '{{\"SYNTH\":\"k1\"}}', '2026-01-01T00:00:07Z'),
               (2, '{AGENT_X}', '{{\"SYNTH\":\"k2\"}}', '2026-01-01T00:00:08Z');
             INSERT INTO world_model_relationships (id, agent_id, relationship_json, created_at) VALUES
               (1, '{AGENT_X}', '{{\"SYNTH\":\"k3\"}}', '2026-01-01T00:00:09Z');"
        ),
    );
}

/// Fixture names → identities.
const AM4_ROWS: [(&str, &str, &str); 9] = [
    ("m1", "agent_memory", "1"),
    ("m2", "agent_memory", "2"),
    ("m3", "agent_memory", "3"),
    ("y1", "agent_memory", "4"),
    ("z1", "agent_memory", "5"),
    ("t1", "task_history", T1),
    ("k1", "world_model_entities", "1"),
    ("k2", "world_model_entities", "2"),
    ("k3", "world_model_relationships", "1"),
];

fn am4_key(name: &str) -> String {
    let (_, t, id) = AM4_ROWS.iter().find(|r| r.0 == name).unwrap();
    format!("{t}:{id}")
}

fn table_of(name: &str) -> RecordTable {
    RecordTable::ALL
        .into_iter()
        .find(|t| t.table_name() == name)
        .unwrap()
}

fn state_name(s: &MemberState) -> String {
    format!("{s:?}")
}

/// Member states keyed `table:id`, as text (for summaries).
fn states(r: &LedgerReport) -> BTreeMap<String, String> {
    r.members
        .iter()
        .map(|m| {
            (
                format!("{}:{}", m.table.table_name(), m.record_id),
                state_name(&m.state),
            )
        })
        .collect()
}

fn event_kinds(db: &NexusDatabase) -> Vec<String> {
    events(db).into_iter().map(|e| e.1).collect()
}

fn emit(v: serde_json::Value) {
    println!("{AM4_PREFIX}{v}");
    std::io::stdout().flush().unwrap();
}

/// Runs one A-M4 child phase and returns its result line.
fn run_am4_child(entry: &str, path: &Path, fk_on: bool) -> serde_json::Value {
    let mut child = spawn_child(
        entry,
        &[
            ("HM_A1_AM4_DB", path.display().to_string()),
            ("HM_A1_AM4_FK", if fk_on { "on" } else { "off" }.to_string()),
        ],
    );
    let rx = line_channel(&mut child);
    let line = wait_line(&rx, AM4_PREFIX);
    let status = child.wait().unwrap();
    assert!(status.success(), "{entry} (fk_on={fk_on}) failed: {status}");
    serde_json::from_str(line.strip_prefix(AM4_PREFIX).unwrap()).unwrap()
}

fn child_env() -> Option<(PathBuf, bool)> {
    let path = std::env::var("HM_A1_AM4_DB").ok()?;
    let fk_on = std::env::var("HM_A1_AM4_FK").unwrap() == "on";
    Some((PathBuf::from(path), fk_on))
}

/// Child entry, A-M4 steps 1, 2, 5 and 6.
#[test]
#[ignore = "child process entry for A-M4; run by the parent test"]
fn hm_child_am4_phase1() {
    let Some((path, fk_on)) = child_env() else {
        return;
    };
    let db = open_mode(&path, fk_on);
    seed_am4(&db);

    // Step 1: per-row SHA-256 of all fixture rows, then T0.
    let hashes = row_hashes(&db);
    assert_eq!(hashes.len(), 9);
    let expected = sorted(t_expected_members(&db));
    run_t0_option_a(&db, &agents()).unwrap();

    // Step 2: exactly the nine members, their digests, count and set digest.
    let got = ledger(&db);
    assert_eq!(got, expected);
    let keys: BTreeSet<String> = got.iter().map(|m| format!("{}:{}", m.0, m.1)).collect();
    let want: BTreeSet<String> = AM4_ROWS
        .iter()
        .map(|r| format!("{}:{}", r.1, r.2))
        .collect();
    assert_eq!(keys, want);
    assert_eq!(count(&db, "SELECT member_count FROM hm_migration"), 9);
    let digest = strings(&db, "SELECT member_set_sha256 FROM hm_migration").remove(0);
    assert_eq!(digest, t_member_set(&expected), "independently computed");

    // Steps 3-4 (grants) removed under AD-4.

    // Step 5a: a direct single-row DELETE of m2.
    must(&db, "DELETE FROM agent_memory WHERE id = 2");
    // Step 5b: the production delete of Z's memories (z1).
    db.delete_memories_by_agent(AGENT_Z).unwrap();
    // Step 6: the production knowledge-graph replace for X: k1 k2 k3 out, k4 k5 in.
    db.replace_world_model_state(
        AGENT_X,
        &["{\"SYNTH\":\"k4\"}".to_string()],
        &["{\"SYNTH\":\"k5\"}".to_string()],
    )
    .unwrap();
    let k4 = count(&db, "SELECT MAX(id) FROM world_model_entities");
    let k5 = count(&db, "SELECT MAX(id) FROM world_model_relationships");

    emit(serde_json::json!({
        "hashes": hashes,
        "member_set_sha256": digest,
        "ledger": got.iter().map(|m| [m.0.clone(), m.1.clone(), m.2.clone(), m.3.clone()]).collect::<Vec<_>>(),
        "k4": k4,
        "k5": k5,
    }));
}

/// Child entry, A-M4 steps 7 (restart), 8 and 9.
#[test]
#[ignore = "child process entry for A-M4; run by the parent test"]
fn hm_child_am4_phase2() {
    let Some((path, fk_on)) = child_env() else {
        return;
    };
    let db = open_mode(&path, fk_on);
    let k4 = count(&db, "SELECT MAX(id) FROM world_model_entities").to_string();
    let k5 = count(&db, "SELECT MAX(id) FROM world_model_relationships").to_string();

    // Step 8: deleted members are gone, unlabelled and tombstoned.
    for name in ["m2", "z1", "k1", "k2", "k3"] {
        let (_, t, id) = *AM4_ROWS.iter().find(|r| r.0 == name).unwrap();
        let present = db
            .hm()
            .immediate(|tx| tx.current_row(t, id))
            .unwrap()
            .is_some();
        assert!(!present, "{name} must be absent");
        assert_eq!(
            count(&db, &format!("SELECT COUNT(*) FROM hm_record_label WHERE record_table = '{t}' AND record_id = '{id}'")),
            0,
            "{name} must have no label row"
        );
        assert_eq!(
            count(&db, &format!("SELECT COUNT(*) FROM hm_t0_tombstone WHERE record_table = '{t}' AND record_id = '{id}'")),
            1,
            "{name} must have a tombstone"
        );
        assert_eq!(
            evaluate_label(&db, table_of(t), id).unwrap(),
            EvaluatedLabel::Unbound
        );
    }
    // No production read returns them.
    let x_keys: Vec<String> = db
        .load_memories(AGENT_X, None, 100)
        .unwrap()
        .into_iter()
        .map(|m| m.key)
        .collect();
    assert_eq!(
        x_keys.iter().cloned().collect::<BTreeSet<_>>(),
        BTreeSet::from(["SYNTH-m1".to_string(), "SYNTH-m3".to_string()])
    );
    assert!(db.load_memories(AGENT_Z, None, 100).unwrap().is_empty());
    let ents: Vec<String> = db
        .load_world_model_entities(AGENT_X)
        .unwrap()
        .into_iter()
        .map(|e| e.entity_json)
        .collect();
    assert_eq!(ents, vec!["{\"SYNTH\":\"k4\"}".to_string()]);
    let rels: Vec<String> = db
        .load_world_model_relationships(AGENT_X)
        .unwrap()
        .into_iter()
        .map(|r| r.relationship_json)
        .collect();
    assert_eq!(rels, vec!["{\"SYNTH\":\"k5\"}".to_string()]);
    // k4 and k5 are not members and have no label (UNBOUND).
    for (t, id) in [
        (RecordTable::WorldModelEntities, k4.as_str()),
        (RecordTable::WorldModelRelationships, k5.as_str()),
    ] {
        assert!(
            !is_t0_member(&db, t, id).unwrap(),
            "{t:?}:{id} is not a member"
        );
        assert_eq!(evaluate_label(&db, t, id).unwrap(), EvaluatedLabel::Unbound);
    }

    // Step 9: L1-L4 pass on the ledger; count and digest unchanged.
    let r = report(&db);
    assert!(r.checks.all_pass(), "step 9: {:?}", r.checks);
    assert!(!r.mismatch_event_written);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM hm_t0_member"), 9);
    for name in ["m1", "m3", "t1", "y1"] {
        let (_, t, id) = *AM4_ROWS.iter().find(|r| r.0 == name).unwrap();
        assert_eq!(
            state_of(&r, table_of(t), id),
            Some(present_unchanged()),
            "{name} stays a present, unchanged member"
        );
    }
    for name in ["m2", "z1", "k1", "k2", "k3"] {
        let (_, t, id) = *AM4_ROWS.iter().find(|r| r.0 == name).unwrap();
        assert_eq!(
            state_of(&r, table_of(t), id),
            Some(MemberState::Deleted),
            "{name}"
        );
    }
    assert!(
        !r.inconsistency_event_written,
        "deletion is not an inconsistency"
    );
    assert_events_content_free(&db);

    emit(serde_json::json!({
        "hashes": row_hashes(&db),
        "member_count": count(&db, "SELECT member_count FROM hm_migration"),
        "member_set_sha256": strings(&db, "SELECT member_set_sha256 FROM hm_migration").remove(0),
        "failed_checks": r.checks.failed(),
        "states": states(&r),
        "events": event_kinds(&db),
        "tombstones": strings(&db, "SELECT record_table || ':' || record_id FROM hm_t0_tombstone ORDER BY 1"),
    }));
}

/// Copies the closed synthetic database (and any WAL/SHM side files).
fn copy_db(src: &Path, dir: &Path, name: &str) -> PathBuf {
    let dst = dir.join(name);
    std::fs::copy(src, &dst).unwrap();
    for ext in ["-wal", "-shm"] {
        let side = PathBuf::from(format!("{}{ext}", src.display()));
        if side.exists() {
            std::fs::copy(&side, format!("{}{ext}", dst.display())).unwrap();
        }
    }
    dst
}

/// Recreates `hm_t0_member` (and its triggers, byte-exact) with the rows
/// selected by `select_rows` from the old table, bypassing the ledger
/// triggers. Foreign keys are off while the table is rebuilt.
fn rebuild_ledger(path: &Path, select_rows: &str) {
    let db = open(path);
    db.execute_raw("PRAGMA foreign_keys=OFF").unwrap();
    let table_sql = strings(
        &db,
        "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'hm_t0_member'",
    )
    .remove(0);
    let trigger_sql = strings(
        &db,
        "SELECT sql FROM sqlite_master WHERE type = 'trigger' AND tbl_name = 'hm_t0_member' ORDER BY name",
    );
    assert_eq!(trigger_sql.len(), 3);
    must(
        &db,
        &format!(
            "CREATE TABLE fixture_ledger_copy AS SELECT * FROM hm_t0_member;
             DROP TABLE hm_t0_member;
             {table_sql};
             INSERT INTO hm_t0_member (record_table, record_id, agent_id, t0_content_sha256)
               {select_rows};
             DROP TABLE fixture_ledger_copy;"
        ),
    );
    for t in trigger_sql {
        must(&db, &t);
    }
}

/// Opens a corrupted copy in the requested mode and verifies it.
fn verify_copy(path: &Path, fk_on: bool) -> (NexusDatabase, LedgerReport) {
    let db = open_mode(path, fk_on);
    let r = report(&db);
    (db, r)
}

fn assert_detected(name: &str, db: &NexusDatabase, r: &LedgerReport, failed: &[&str]) {
    assert_eq!(r.checks.failed(), failed.to_vec(), "{name}: {:?}", r.checks);
    assert!(r.mismatch_event_written, "{name}: mismatch event");
    assert_eq!(event_count(db, "memory.member_set_mismatch"), 1, "{name}");
    assert_events_content_free(db);
}

fn run_am4(fk_on: bool) -> serde_json::Value {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.db");

    let p1 = run_am4_child("hm_child_am4_phase1", &path, fk_on);
    let p2 = run_am4_child("hm_child_am4_phase2", &path, fk_on);

    // Step 8: surviving fixture rows unchanged.
    for name in ["m1", "m3", "t1", "y1"] {
        let k = am4_key(name);
        assert_eq!(p2["hashes"][&k], p1["hashes"][&k], "{name} per-row SHA-256");
        assert!(p2["hashes"][&k].is_string());
    }
    for name in ["m2", "z1", "k1", "k2", "k3"] {
        assert!(p2["hashes"][&am4_key(name)].is_null(), "{name} absent");
    }
    // Step 9: count and digest unchanged.
    assert_eq!(p2["member_count"], 9);
    assert_eq!(p2["member_set_sha256"], p1["member_set_sha256"]);
    assert_eq!(p2["failed_checks"], serde_json::json!([]));

    let mut out = serde_json::Map::new();
    out.insert("phase1_ledger".into(), p1["ledger"].clone());
    out.insert("phase1_digest".into(), p1["member_set_sha256"].clone());
    out.insert("k4_k5".into(), serde_json::json!([p1["k4"], p1["k5"]]));
    out.insert("phase2_states".into(), p2["states"].clone());
    out.insert("phase2_events".into(), p2["events"].clone());
    out.insert("phase2_tombstones".into(), p2["tombstones"].clone());
    out.insert("phase2_digest".into(), p2["member_set_sha256"].clone());

    let m1 = ("agent_memory", "1");
    let m3 = ("agent_memory", "3");
    let t1 = ("task_history", T1);

    // N-L1: one ledger digest altered (table rebuilt, triggers restored).
    let c = copy_db(&path, dir.path(), "nl1.db");
    rebuild_ledger(
        &c,
        "SELECT record_table, record_id, agent_id,
                CASE WHEN record_table = 'agent_memory' AND record_id = '1'
                     THEN 'forged' ELSE t0_content_sha256 END
         FROM fixture_ledger_copy",
    );
    let (db, r) = verify_copy(&c, fk_on);
    assert_detected("N-L1", &db, &r, &["L2"]);
    out.insert("N-L1".into(), serde_json::json!({"failed": r.checks.failed(), "states": states(&r), "events": event_kinds(&db)}));
    drop(db);

    // N-L2: one ledger entry (m1) omitted.
    let c = copy_db(&path, dir.path(), "nl2.db");
    rebuild_ledger(
        &c,
        "SELECT record_table, record_id, agent_id, t0_content_sha256 FROM fixture_ledger_copy
         WHERE NOT (record_table = 'agent_memory' AND record_id = '1')",
    );
    let (db, r) = verify_copy(&c, fk_on);
    assert_detected("N-L2", &db, &r, &["L1", "L2"]);
    assert!(
        !is_t0_member(&db, RecordTable::AgentMemory, m1.1).unwrap(),
        "N-L2: m1 is no member"
    );
    assert_eq!(state_of(&r, RecordTable::AgentMemory, m1.1), None);
    out.insert("N-L2".into(), serde_json::json!({"failed": r.checks.failed(), "states": states(&r), "events": event_kinds(&db)}));
    drop(db);

    // N-L3: a tombstone for a non-member identity (foreign keys off to insert it).
    let c = copy_db(&path, dir.path(), "nl3.db");
    {
        let db = open(&c);
        db.execute_raw("PRAGMA foreign_keys=OFF").unwrap();
        must(
            &db,
            "INSERT INTO hm_t0_tombstone VALUES ('agent_memory', '999')",
        );
    }
    let (db, r) = verify_copy(&c, fk_on);
    assert_detected("N-L3", &db, &r, &["L3"]);
    out.insert("N-L3".into(), serde_json::json!({"failed": r.checks.failed(), "states": states(&r), "events": event_kinds(&db)}));
    drop(db);

    // N-L4: the ledger immutability trigger dropped.
    let c = copy_db(&path, dir.path(), "nl4.db");
    must(&open(&c), "DROP TRIGGER hm_t0_member_no_update");
    let (db, r) = verify_copy(&c, fk_on);
    assert_detected("N-L4", &db, &r, &["L4"]);
    out.insert("N-L4".into(), serde_json::json!({"failed": r.checks.failed(), "states": states(&r), "events": event_kinds(&db)}));
    drop(db);

    // N-L5: m3 deleted with its delete trigger dropped.
    let c = copy_db(&path, dir.path(), "nl5.db");
    must(
        &open(&c),
        "DROP TRIGGER hm_del_agent_memory; DELETE FROM agent_memory WHERE id = 3;",
    );
    let (db, r) = verify_copy(&c, fk_on);
    assert_detected("N-L5", &db, &r, &["L4"]);
    assert_eq!(
        state_of(&r, RecordTable::AgentMemory, m3.1),
        Some(MemberState::MissingUntombstoned)
    );
    out.insert("N-L5".into(), serde_json::json!({"failed": r.checks.failed(), "states": states(&r), "events": event_kinds(&db)}));
    drop(db);

    // N-L5 variant: only the row removed out of band, triggers intact.
    let c = copy_db(&path, dir.path(), "nl5v.db");
    {
        let db = open(&c);
        must(
            &db,
            "DROP TRIGGER hm_del_agent_memory; DELETE FROM agent_memory WHERE id = 3;",
        );
        must(&db, TRG_DEL_AGENT_MEMORY);
    }
    let (db, r) = verify_copy(&c, fk_on);
    assert!(r.checks.all_pass(), "N-L5 variant: {:?}", r.checks);
    assert!(!r.mismatch_event_written);
    assert_eq!(
        state_of(&r, RecordTable::AgentMemory, m3.1),
        Some(MemberState::MissingUntombstoned)
    );
    assert!(r.inconsistency_event_written, "N-L5 variant: audited");
    assert_eq!(event_count(&db, "memory.member_record_inconsistent"), 1);
    assert_eq!(
        evaluate_label(&db, RecordTable::AgentMemory, m3.1).unwrap(),
        EvaluatedLabel::Unbound
    );
    for (t, id) in [m1, t1] {
        assert_eq!(
            state_of(&r, table_of(t), id),
            Some(present_unchanged()),
            "{t}:{id} still covered"
        );
    }
    assert_events_content_free(&db);
    out.insert("N-L5v".into(), serde_json::json!({"failed": r.checks.failed(), "states": states(&r), "events": event_kinds(&db)}));
    drop(db);

    // Step 10 (on the original): complete t1 post-T0; it leaves the unchanged
    // set and its ledger row stays.
    let db = open_mode(&path, fk_on);
    let ledger_before = ledger(&db);
    db.update_task_status(T1, "completed", Some("\"SYNTH-final\""), 1.0, true)
        .unwrap();
    let r = report(&db);
    assert!(r.checks.all_pass(), "step 10: {:?}", r.checks);
    assert_eq!(
        state_of(&r, RecordTable::TaskHistory, T1),
        Some(MemberState::Present {
            label: EvaluatedLabel::Unbound,
            unchanged_since_t0: false
        })
    );
    assert_eq!(ledger(&db), ledger_before);
    for (t, id) in [m1, m3] {
        assert_eq!(state_of(&r, table_of(t), id), Some(present_unchanged()));
    }
    out.insert(
        "step10".into(),
        serde_json::json!({"failed": r.checks.failed(), "states": states(&r)}),
    );

    serde_json::Value::Object(out)
}

#[test]
fn hm_n4_a_m4_membership_survives_deletion_fk_on_and_off_identical() {
    let on = run_am4(true);
    let off = run_am4(false);
    assert_eq!(
        on, off,
        "foreign_keys ON and OFF must give identical results"
    );
}
