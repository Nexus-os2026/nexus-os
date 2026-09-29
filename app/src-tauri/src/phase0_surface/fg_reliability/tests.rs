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
    assert_eq!(nexus_kernel::immune::arena::MAX_ARENA_ROUNDS, 50);
    assert_eq!(nexus_kernel::temporal::types::MAX_TEMPORAL_FORKS, 10);
    assert_eq!(
        nexus_kernel::temporal::types::MAX_FORK_BUDGET_TOKENS,
        200_000
    );
}

#[test]
fn p0_fg_k_adversarial_rounds_are_refused_outside_their_bound() {
    // Through the IPC command's own function (consent.rs), which checks the
    // kernel's bound first (`check_rounds`) and then runs `try_run_session`:
    // the refusal is the kernel's text and the command's error.
    for rounds in [0, 51, 10_000] {
        let error = crate::run_adversarial_session("attacker".into(), "defender".into(), rounds)
            .expect_err("an out-of-range round count must be refused");
        assert_eq!(
            error,
            format!("arena rounds must be between 1 and 50, got {rounds}")
        );
    }
    for rounds in [1, 50] {
        let session =
            crate::run_adversarial_session("attacker".into(), "defender".into(), rounds).unwrap();
        assert_eq!(session["rounds"], rounds);
        assert_eq!(
            session["results"].as_array().map(Vec::len),
            Some(rounds as usize)
        );
    }
}

#[test]
fn p0_fg_k_temporal_fork_limits_are_refused_and_never_stored() {
    let state = AppState::new_in_memory();
    let stored = |state: &AppState| {
        let engine = state.temporal_engine.lock().unwrap();
        (
            engine.config().max_parallel_forks,
            engine.config().fork_budget_tokens,
        )
    };
    let before = stored(&state);
    for (forks, tokens) in [
        (0, 50_000),
        (11, 50_000),
        (u32::MAX, 50_000),
        (5, 0),
        (5, 200_001),
        (5, u64::MAX),
    ] {
        assert!(
            crate::set_temporal_config(&state, forks, "BestFinalScore".into(), tokens).is_err(),
            "{forks} forks, {tokens} tokens"
        );
        assert_eq!(stored(&state), before, "a refused config was stored");
    }
    // A fork-count override is refused before the configuration is read, a
    // provider is built or the model is called, and is never stored.
    for forks in [0, 11, u32::MAX] {
        let error = crate::temporal_fork(&state, "request".into(), "agent".into(), Some(forks))
            .expect_err("an out-of-range fork count must be refused");
        assert_eq!(
            error,
            format!("invalid fork count: {forks} (allowed: 1 to 10)")
        );
        assert_eq!(stored(&state), before);
    }
    crate::set_temporal_config(&state, 10, "BestFinalScore".into(), 200_000).unwrap();
    assert_eq!(stored(&state), (10, 200_000));
    crate::set_temporal_config(&state, 1, "LowestRisk".into(), 1).unwrap();
    assert_eq!(stored(&state), (1, 1));
}

#[test]
fn p0_fg_k_an_older_loops_exit_keeps_a_newer_loops_cancellation_entry() {
    use crate::commands::cognitive::CognitiveCancelGuard;
    let state = AppState::new_in_memory();
    let id = agent(&state, "fg-k-cancel");
    let entry = |state: &AppState| {
        state
            .cognitive_cancellations
            .lock()
            .unwrap()
            .get(&id)
            .cloned()
    };
    let executor = crate::commands::cognitive::ScheduledGoalExecutor {
        state: state.clone(),
    };

    let (older_flag, older) = CognitiveCancelGuard::register(&state, &id);
    let (newer_flag, newer) = CognitiveCancelGuard::register(&state, &id);
    assert!(Arc::ptr_eq(&entry(&state).unwrap(), &newer_flag));

    // The older loop ends first: the newer loop's entry must stay, so Stop
    // and the scheduler still see the loop that is running.
    drop(older);
    let current = entry(&state).expect("the newer loop's entry was erased");
    assert!(Arc::ptr_eq(&current, &newer_flag));
    assert!(!Arc::ptr_eq(&current, &older_flag));
    assert!(executor.execute(&id, "scheduled goal").is_err());

    // The newer loop ends: its own entry goes, and ticks run again.
    drop(newer);
    assert!(entry(&state).is_none());
    executor.execute(&id, "scheduled goal").unwrap();

    // Reverse order: the newer loop ends first, then the older one; the
    // older guard finds no entry of its own and removes nothing.
    let other = agent(&state, "fg-k-cancel-reverse");
    let (_, older) = CognitiveCancelGuard::register(&state, &other);
    let (_, newer) = CognitiveCancelGuard::register(&state, &other);
    drop(newer);
    let (third_flag, third) = CognitiveCancelGuard::register(&state, &other);
    drop(older);
    let current = state
        .cognitive_cancellations
        .lock()
        .unwrap()
        .get(&other)
        .cloned()
        .expect("a stale guard erased a later loop's entry");
    assert!(Arc::ptr_eq(&current, &third_flag));
    drop(third);
    assert!(state
        .cognitive_cancellations
        .lock()
        .unwrap()
        .get(&other)
        .is_none());
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
    let source = include_str!("../../lib.rs").replace("\r\n", "\n");
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

/// Test isolation: the cognitive loop of an in-memory `AppState` records its
/// L6 cooldowns and algorithm selections in the in-memory database. The
/// production-executor test, run in a child process whose identity home is a
/// scratch directory, leaves that home without a nexus.db. (The loop used to
/// open the identity home's nexus.db: on a developer machine, the real one.)
#[test]
fn p0_fg_k_an_in_memory_state_loop_writes_no_identity_home_database() {
    let home = std::env::temp_dir().join(format!("p0-fg-state-db-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(home.join(".nexus")).unwrap();
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "phase0_surface::tests::p0_002c5c_a2a_and_agent_actions_are_decided_by_the_production_executor",
            "--nocapture",
        ])
        .env("HOME", &home)
        .env_remove("NEXUS_DB_PATH")
        .output()
        .unwrap();
    let written = home.join(".nexus").join("nexus.db").exists();
    std::fs::remove_dir_all(&home).unwrap();
    assert!(
        output.status.success(),
        "child failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("1 passed"),
        "the child ran no test"
    );
    assert!(!written, "the identity home's nexus.db was written");
}
