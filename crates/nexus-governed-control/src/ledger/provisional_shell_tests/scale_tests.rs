//! A measurement, not a property: what opening costs as the ledger grows.
//! Opening verifies the whole chain and every index (draft section 6.4), so
//! its cost grows with the ledger. Run on demand:
//! `cargo test -p nexus-governed-control --lib -- --ignored scale_tests --nocapture`
//! with `NEXUS_LEDGER_SCALE_ROWS` set (default 10,000).

use super::*;
use std::time::Instant;

#[test]
#[ignore = "a measurement, run on demand"]
fn opening_cost_grows_with_the_ledger() {
    let rows: usize = std::env::var("NEXUS_LEDGER_SCALE_ROWS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(10_000);
    let scratch = Scratch::new();
    let path = scratch.path("l.db");
    let mut store = opened(&path);
    let started = Instant::now();
    let mut written = 0;
    while written < rows {
        let batch: Vec<Built<FactRecord>> = (written..rows.min(written + BATCH_MAX))
            .map(|n| store.fact(format!("r{n}").as_bytes(), None, &[0x5a; 64]))
            .collect();
        let mut ticket = store.health().arm().unwrap();
        assert_eq!(
            store.append_facts(&mut ticket, &batch).unwrap(),
            FactOutcome::Committed
        );
        ticket.confirm();
        written += batch.len();
    }
    let appended = started.elapsed();
    drop(store);
    let size: u64 = ["l.db", "l.db-wal"]
        .iter()
        .filter_map(|name| std::fs::metadata(scratch.path(name)).ok())
        .map(|meta| meta.len())
        .sum();
    let started = Instant::now();
    let mut store = Store::open(&path, LIMITS).expect("the file reopens");
    let opened_in = started.elapsed();
    assert_eq!(store.open_session(), Opening::Opened);
    let started = Instant::now();
    assert!(matches!(store.snapshot(), SnapshotOutcome::Nothing));
    let surveyed = started.elapsed();
    eprintln!(
        "scale: rows={rows} batches_of={BATCH_MAX} append_s={:.3} file_bytes={size} open_verify_s={:.3} survey_s={:.3}",
        appended.as_secs_f64(),
        opened_in.as_secs_f64(),
        surveyed.as_secs_f64()
    );
}
