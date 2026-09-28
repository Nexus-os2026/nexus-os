//! P0-FINAL-GATE-CLOSURE guards for item K and the resource bounds.
//!
//! Each dossier "Resource bounds" surface is reachable from any script in the
//! webview. These guards drive the desktop entry points with out-of-range
//! requests and check that they are refused before any work, and they pin
//! the approved limits so a bound cannot be loosened silently.

use crate::AppState;
use nexus_kernel::cognitive::ScheduledGoalExecutor as _;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

fn agent(state: &AppState, name: &str) -> String {
    let manifest = nexus_kernel::manifest::parse_manifest(&format!(
        r#"
name = "{name}"
version = "1.0.0"
capabilities = ["llm.query"]
fuel_budget = 10000
autonomy_level = 1
"#
    ))
    .unwrap();
    state
        .supervisor
        .lock()
        .unwrap()
        .start_agent(manifest)
        .unwrap()
        .to_string()
}

fn audited_actions(state: &AppState) -> Vec<(String, String)> {
    state
        .audit
        .lock()
        .unwrap()
        .events()
        .iter()
        .map(|event| {
            (
                event.payload["action"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
                event.payload["reason"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
            )
        })
        .collect()
}

#[test]
fn p0_fg_k_approved_limits_are_pinned() {
    assert_eq!(crate::MAX_STRESS_PERSONAS, 1_000);
    assert_eq!(
        nexus_kernel::simulation::runtime::MAX_PARALLEL_SIMULATION_VARIANTS,
        10
    );
    assert_eq!(nexus_kernel::temporal::dilation::MAX_DILATED_ITERATIONS, 50);
    assert_eq!(crate::MAX_FRONTEND_ERROR_FIELD_BYTES, 8 * 1024);
    assert_eq!(crate::MAX_FRONTEND_ERROR_LOG_BYTES, 4 * 1024 * 1024);
    assert_eq!(web_builder_agent::budget::MAX_BUILD_HISTORY, 1_000);
}

#[test]
fn p0_fg_k_stress_persona_count_is_refused_outside_its_bound() {
    let state = AppState::new_in_memory();
    for count in [0, crate::MAX_STRESS_PERSONAS + 1, u32::MAX] {
        let error = crate::stress_generate_personas(&state, count)
            .expect_err("an out-of-range persona count must be refused");
        assert!(error.contains("between 1 and 1000"), "{error}");
    }
    for count in [1, crate::MAX_STRESS_PERSONAS] {
        let json = crate::stress_generate_personas(&state, count).unwrap();
        let personas: Vec<serde_json::Value> = serde_json::from_str(&json).unwrap();
        assert_eq!(personas.len(), count as usize);
    }
}

#[test]
fn p0_fg_k_parallel_simulation_count_is_refused_outside_its_bound() {
    let state = AppState::new_in_memory();
    for count in [0, 11, u32::MAX] {
        let error = crate::run_parallel_simulation_reports(
            &state,
            "A contested climate bill enters parliament".into(),
            count,
        )
        .expect_err("an out-of-range variant count must be refused");
        assert!(error.contains("between 1 and 10"), "{count}: {error}");
    }
}

#[test]
fn p0_fg_k_dilated_session_iterations_are_refused_outside_their_bound() {
    let state = AppState::new_in_memory();
    for count in [0, 51, u32::MAX] {
        let error = crate::run_dilated_session(&state, "task".into(), Vec::new(), count)
            .expect_err("an out-of-range iteration count must be refused");
        assert_eq!(
            error,
            format!("invalid iteration count: {count} (allowed: 1 to 50)")
        );
    }
}

#[test]
fn p0_fg_k_agent_schedules_fire_at_most_once_per_minute() {
    let state = AppState::new_in_memory();
    let id = agent(&state, "fg-k-cron");
    for expression in ["* * * * * *", "*/5 * * * * *", "0,30 * * * * *"] {
        assert!(nexus_kernel::cognitive::AgentScheduler::validate_cron(expression).is_err());
        // No Tokio runtime exists in this test: registration must refuse
        // before it would spawn the schedule loop.
        assert!(state
            .agent_scheduler
            .register_agent(&id, expression, "goal")
            .is_err());
        // The path create_agent, start_agent and the startup restore use.
        crate::register_manifest_schedule(&state, &id, Some(expression), Some("goal"), None);
    }
    assert!(state.agent_scheduler.list().is_empty());
    assert!(nexus_kernel::cognitive::AgentScheduler::validate_cron("0 * * * * *").is_ok());
    assert!(nexus_kernel::cognitive::AgentScheduler::validate_cron("*/5 * * * *").is_ok());
}

#[test]
fn p0_fg_k_a_scheduled_tick_never_overlaps_the_agents_running_loop() {
    let state = AppState::new_in_memory();
    let id = agent(&state, "fg-k-scheduled");
    let executor = crate::commands::cognitive::ScheduledGoalExecutor {
        state: state.clone(),
    };

    // A running desktop loop holds its cancellation entry, under any
    // spelling of the agent's id.
    for key in [id.clone(), id.to_ascii_uppercase()] {
        state
            .cognitive_cancellations
            .lock()
            .unwrap()
            .insert(key.clone(), Arc::new(AtomicBool::new(false)));
        let error = executor
            .execute(&id, "scheduled goal")
            .expect_err("a tick must be skipped while the loop runs");
        assert!(error.contains("still running"), "{error}");
        assert!(
            !state.cognitive_runtime.has_active_loop(&id),
            "a skipped tick assigned a goal"
        );
        state.cognitive_cancellations.lock().unwrap().remove(&key);
    }
    let actions = audited_actions(&state);
    assert_eq!(
        actions
            .iter()
            .filter(|(action, reason)| action == "scheduled_execution_skipped"
                && reason == "agent_loop_active")
            .count(),
        2
    );
    assert!(!actions
        .iter()
        .any(|(action, _)| action == "scheduled_execution_triggered"));

    // With no loop running the tick proceeds as before.
    executor.execute(&id, "scheduled goal").unwrap();
    assert!(state.cognitive_runtime.has_active_loop(&id));
    assert!(audited_actions(&state)
        .iter()
        .any(|(action, _)| action == "scheduled_execution_triggered"));
}

#[test]
fn p0_fg_k_the_frontend_error_command_uses_the_bounded_log() {
    let source = include_str!("../lib.rs").replace("\r\n", "\n");
    let start = source
        .find("fn log_frontend_error(")
        .expect("log_frontend_error is registered");
    let body = &source[start..start + source[start..].find("\n    }\n").unwrap()];
    assert!(
        body.contains("super::record_frontend_error(&message, &stack, &component_stack);"),
        "{body}"
    );
    for unbounded in ["OpenOptions", "writeln!", "eprintln!", "std::fs::"] {
        assert!(!body.contains(unbounded), "{unbounded} in {body}");
    }
}

#[test]
fn p0_fg_k_build_records_outside_the_bounds_are_refused() {
    use web_builder_agent::budget::BuildRecord;
    let valid = BuildRecord {
        project_name: "site".into(),
        model_name: "model".into(),
        provider: "anthropic".into(),
        input_tokens: 1,
        output_tokens: 1,
        cost_usd: 0.01,
        elapsed_seconds: 1.0,
        lines_generated: 1,
        checkpoint_id: String::new(),
        timestamp: "2026-09-28T00:00:00Z".into(),
    };
    valid.validate().unwrap();
    for invalid in [
        BuildRecord {
            cost_usd: 1.7e308,
            ..valid.clone()
        },
        BuildRecord {
            cost_usd: f64::NAN,
            ..valid.clone()
        },
        BuildRecord {
            project_name: "x".repeat(257),
            ..valid.clone()
        },
        BuildRecord {
            provider: "not a provider".into(),
            ..valid.clone()
        },
    ] {
        assert!(invalid.validate().is_err());
    }
}
