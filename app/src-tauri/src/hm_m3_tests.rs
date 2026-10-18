//! P3-C11-HM-M3: production acceptance tests of the desktop
//! (P3-HM-CONTRACT-02 §18.2) on the desktop's own state.
//!
//! Tests build the state with `AppState::new_in_memory()` (the P0 Final
//! Gate forbids any test from building the real application state). Its
//! construction runs the same start-up sequence as the production
//! constructor over its database; the full A-M1 assertions on a synthetic
//! pre-T₀ database file run against that start-up function (`hm_wiring`
//! tests), and a source guard pins that both constructors call it.

use nexus_persistence::hm::{
    LabelRow, MemberRow, MigrationRow, PolicyChangeRow, PolicyRow, PolicyRowRaw,
};
use nexus_persistence::NexusDatabase;

// ── The desktop's historical-memory test fixtures ───────────────────────────
//
// The one desktop file that reaches the store (`NexusDatabase::hm`) or names
// its tables: a test module (`#[cfg(test)] mod hm_m3_tests;`), never product
// code. The persistence guards pin it as such.

/// TEST FIXTURE, not product code (mission AMENDMENT 1 A1.9): an Owner
/// loosening (M4 builds the native confirmation) committed through the
/// store's row methods, with its history row first (§5.1 trigger).
pub(crate) fn fixture_owner_loosen(db: &NexusDatabase, agent_id: &str) {
    db.hm()
        .immediate(|tx| {
            let raw = tx.policy_row_raw(agent_id)?.expect("policy row");
            let generation = raw.policy_generation.expect("generation") + 1;
            tx.insert_policy_change(&PolicyChangeRow {
                agent_id: agent_id.into(),
                policy_generation: generation,
                local_only: 0,
                basis: "owner_confirmed".into(),
                cause: "owner_loosen".into(),
            })?;
            tx.update_policy(&PolicyRow {
                agent_id: agent_id.into(),
                local_only: 0,
                policy_generation: generation,
                revocation_seq: raw.revocation_seq.expect("revocation sequence"),
                basis: "owner_confirmed".into(),
            })?;
            Ok(())
        })
        .unwrap();
}

/// The stored label rows.
pub(crate) fn labels(db: &NexusDatabase) -> Vec<LabelRow> {
    db.hm().read(|tx| tx.label_rows()).unwrap()
}

/// The T₀ ledger.
pub(crate) fn ledger(db: &NexusDatabase) -> Vec<MemberRow> {
    db.hm().read(|tx| tx.ledger_rows()).unwrap()
}

/// The digest of a ledger (§9.1 step 5).
pub(crate) fn ledger_digest(rows: &[MemberRow]) -> String {
    nexus_persistence::hm::member_set_digest(rows)
}

/// The migration rows.
pub(crate) fn migrations(db: &NexusDatabase) -> Vec<MigrationRow> {
    db.hm().read(|tx| tx.migration_rows()).unwrap()
}

/// The policy rows, as stored, ordered by agent.
pub(crate) fn policies(db: &NexusDatabase) -> Vec<PolicyRowRaw> {
    db.hm().read(|tx| tx.policy_rows_raw()).unwrap()
}

/// `record_table || record_id || origin || content_sha256` of every label
/// row, sorted.
pub(crate) fn label_keys(db: &NexusDatabase) -> Vec<String> {
    let mut keys: Vec<String> = labels(db)
        .into_iter()
        .map(|l| {
            format!(
                "{}{}{}{}",
                l.record_table, l.record_id, l.origin, l.content_sha256
            )
        })
        .collect();
    keys.sort();
    keys
}

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

/// A-M1 (desktop): building the desktop's state runs the T₀ migration on
/// its database before any work is accepted (an empty in-memory database:
/// a ledger of 0 rows, COMPLETE).
#[test]
fn hm_m3_n2_a_m1_building_the_desktop_state_runs_the_t0_migration() {
    let state = crate::AppState::new_in_memory();
    let tables = strings(
        &state.db,
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name LIKE 'hm\\_%' ESCAPE '\\'
         ORDER BY name",
    );
    // The store's tables, named by the store (never here).
    let mut expected: Vec<&str> = nexus_persistence::hm::HM_TABLES
        .iter()
        .map(|(name, _)| *name)
        .collect();
    expected.sort();
    assert_eq!(expected.len(), 10);
    assert_eq!(
        tables, expected,
        "the desktop's start-up ran no T0 migration"
    );
    assert_eq!(
        migrations(&state.db)
            .iter()
            .map(|m| format!("{}|{}", m.state, m.member_count))
            .collect::<Vec<_>>(),
        vec!["COMPLETE|0"]
    );
}
