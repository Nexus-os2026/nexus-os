//! Phase One desktop guards. `p1_tm_*`: the Time Machine "what if" residual
//! (charter §12) stays closed: it takes no caller input, only denies, and
//! can no longer change an agent's fuel, force an agent's state or toggle
//! Warden review.

use super::*;
use crate::phase0_surface::{closed, Closure};

const WHAT_IF: &str = "time_machine_what_if";

fn production_sources() -> Vec<(std::path::PathBuf, String)> {
    fn walk(dir: &std::path::Path, out: &mut Vec<(std::path::PathBuf, String)>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            if path.is_dir() {
                if name != "tests" {
                    walk(&path, out);
                }
            } else if name.ends_with(".rs") && !name.ends_with("tests.rs") {
                let text = std::fs::read_to_string(&path).unwrap();
                out.push((path, text));
            }
        }
    }
    let mut out = Vec::new();
    walk(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut out,
    );
    assert!(out.len() > 20, "desktop sources not found");
    out
}

#[cfg(all(
    feature = "tauri-runtime",
    any(target_os = "windows", target_os = "macos", target_os = "linux")
))]
#[test]
fn p1_tm_01_what_if_takes_no_input_and_only_denies() {
    let reason = Closure::SimulationReplay.reason();
    assert!(reason.len() <= 160 && !reason.contains('/'), "{reason}");
    assert_eq!(
        crate::runtime::time_machine_what_if(),
        Err(closed(WHAT_IF, Closure::SimulationReplay))
    );
}

#[test]
fn p1_tm_02_what_if_changes_no_fuel_state_or_warden_review() {
    let state = AppState::new_in_memory();
    let manifest = serde_json::json!({
        "name": "what-if-probe",
        "version": "2.0.0",
        "capabilities": ["llm.query"],
        "fuel_budget": 10000,
        "schedule": null,
        "llm_model": "local"
    })
    .to_string();
    let id = state
        .supervisor
        .lock()
        .unwrap()
        .start_agent(parse_agent_manifest_json(&manifest).unwrap())
        .unwrap();
    let snapshot = |state: &AppState| {
        let supervisor = state.supervisor.lock().unwrap();
        let handle = supervisor.get_agent(id).unwrap();
        (handle.remaining_fuel, handle.state)
    };
    let before = snapshot(&state);
    // Every former what-if request is now only a string the handler cannot
    // even receive: the registered handler takes no arguments.
    #[cfg(all(
        feature = "tauri-runtime",
        any(target_os = "windows", target_os = "macos", target_os = "linux")
    ))]
    for _request in [
        (format!("agent://{id}/fuel_remaining"), "999999999"),
        (format!("agent://{id}/status"), "Running"),
        ("governance.enable_warden_review".to_string(), "false"),
    ] {
        assert!(crate::runtime::time_machine_what_if().is_err());
    }
    assert_eq!(snapshot(&state), before, "fuel and state unchanged");
}

#[test]
fn p1_tm_03_no_production_code_performs_what_if_mutation() {
    for (path, text) in production_sources() {
        for needle in [
            "time-machine-what-if",
            "time_machine.what_if",
            "variable_key",
            "super::time_machine_what_if",
        ] {
            assert!(
                !text.contains(needle),
                "{} still contains {needle}",
                path.display()
            );
        }
    }
    let model_hub = include_str!("commands/model_hub.rs");
    assert!(!model_hub.contains("fn time_machine_what_if"));
    // Warden review is written only through the reviewed configuration
    // paths, never from a what-if request.
    assert!(!model_hub.contains("enable_warden_review"));
}
