//! Phase 1.4 Deliverable 2 — CostCeiling persistence and accounting.

use std::sync::{Arc, Mutex};
use std::thread;

use nexus_ui_repair::governance::{CostCeiling, CostCeilingError, DEFAULT_CEILING_USD};
use tempfile::tempdir;

fn fresh_path() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("spend.json");
    (dir, path)
}

#[test]
fn fresh_load_returns_zero_spend() {
    let (_dir, path) = fresh_path();
    let c = CostCeiling::load_from_disk(path, DEFAULT_CEILING_USD).expect("load");
    assert_eq!(c.spent_usd(), 0.0);
    assert_eq!(c.ceiling_usd(), DEFAULT_CEILING_USD);
}

#[test]
fn record_spend_updates_and_persists() {
    let (_dir, path) = fresh_path();
    {
        let mut c = CostCeiling::load_from_disk(path.clone(), DEFAULT_CEILING_USD).expect("load");
        c.record_spend(1.25).expect("spend");
        assert_eq!(c.spent_usd(), 1.25);
    }
    let c2 = CostCeiling::load_from_disk(path, DEFAULT_CEILING_USD).expect("reload");
    assert!((c2.spent_usd() - 1.25).abs() < 1e-9);
}

#[test]
fn can_afford_returns_false_at_boundary() {
    let (_dir, path) = fresh_path();
    let mut c = CostCeiling::load_from_disk(path, 10.0).expect("load");
    c.record_spend(9.5).expect("spend");
    assert!(c.can_afford(0.5));
    assert!(!c.can_afford(0.51));
}

#[test]
fn record_spend_returns_ceiling_exceeded_when_over() {
    let (_dir, path) = fresh_path();
    let mut c = CostCeiling::load_from_disk(path, 5.0).expect("load");
    c.record_spend(4.0).expect("spend");
    let err = c.record_spend(2.0).unwrap_err();
    match err {
        CostCeilingError::CeilingExceeded { ceiling, attempted } => {
            assert!((ceiling - 5.0).abs() < 1e-9);
            assert!((attempted - 6.0).abs() < 1e-9);
        }
        other => panic!("expected CeilingExceeded, got {:?}", other),
    }
    // State is not mutated on rejection.
    assert!((c.spent_usd() - 4.0).abs() < 1e-9);
}

#[test]
fn load_after_save_round_trips() {
    let (_dir, path) = fresh_path();
    let mut c = CostCeiling::new_with_path(path.clone(), 10.0);
    c.record_spend(2.75).expect("spend");
    let c2 = CostCeiling::load_from_disk(path, 10.0).expect("reload");
    assert!((c2.spent_usd() - 2.75).abs() < 1e-9);
}

#[test]
fn concurrent_record_spend_does_not_corrupt_file() {
    let (_dir, path) = fresh_path();
    let c = CostCeiling::load_from_disk(path.clone(), 100.0).expect("load");
    let shared = Arc::new(Mutex::new(c));

    let mut handles = Vec::new();
    for _ in 0..10 {
        let s = Arc::clone(&shared);
        handles.push(thread::spawn(move || {
            for _ in 0..10 {
                let mut g = s.lock().expect("mutex");
                g.record_spend(0.10).expect("spend");
            }
        }));
    }
    for h in handles {
        h.join().expect("join");
    }
    let final_spent = shared.lock().expect("mutex").spent_usd();
    assert!((final_spent - 10.0).abs() < 1e-6, "got {}", final_spent);

    let reloaded = CostCeiling::load_from_disk(path, 100.0).expect("reload");
    assert!((reloaded.spent_usd() - 10.0).abs() < 1e-6);
}
