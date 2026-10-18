//! P3-C11-HM-M3: production acceptance tests of the desktop
//! (P3-HM-CONTRACT-02 §18.2) on the desktop's own state.
//!
//! Tests build the state with `AppState::new_in_memory()` (the P0 Final
//! Gate forbids any test from building the real application state). Its
//! construction runs the same start-up sequence as the production
//! constructor over its database; the full A-M1 assertions on a synthetic
//! pre-T₀ database file run against that start-up function (`hm_wiring`
//! tests), and a source guard pins that both constructors call it.

use nexus_persistence::NexusDatabase;

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
    assert_eq!(
        tables,
        vec![
            "hm_agent_policy",
            "hm_derivation",
            "hm_dispatch_admission",
            "hm_event",
            "hm_grant",
            "hm_migration",
            "hm_policy_change",
            "hm_record_label",
            "hm_t0_member",
            "hm_t0_tombstone",
        ],
        "the desktop's start-up ran no T0 migration"
    );
    assert_eq!(
        strings(
            &state.db,
            "SELECT state || '|' || member_count FROM hm_migration"
        ),
        vec!["COMPLETE|0"]
    );
}
