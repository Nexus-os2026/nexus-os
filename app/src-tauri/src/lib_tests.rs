use super::{
    agent_memory_clear, agent_memory_forget, agent_memory_get_stats, agent_memory_recall,
    agent_memory_remember, chat_with_documents, check_model_compatibility, complete_build,
    complete_research, create_agent, create_agent_immediately, create_simulation,
    economy_create_wallet, economy_earn, economy_freeze_wallet, economy_get_history,
    economy_get_stats, economy_get_wallet, economy_spend, economy_transfer, evolution_evolve_once,
    evolution_get_active_strategy, evolution_get_history, evolution_get_status,
    evolution_register_strategy, evolution_rollback, factory_create_project,
    factory_get_build_history, factory_list_projects, get_active_llm_provider, get_agent_activity,
    get_browser_history, get_configured_provider, get_input_control_status, get_knowledge_base,
    get_live_system_metrics, get_messaging_status, get_simulation_report, get_simulation_status,
    get_system_specs, ghost_protocol_add_peer, ghost_protocol_remove_peer, ghost_protocol_status,
    ghost_protocol_toggle, index_document, inject_simulation_variable, learning_agent_action,
    list_agents, list_indexed_documents, list_local_models, list_prebuilt_manifest_paths,
    list_simulations, mcp_host_add_server, mcp_host_list_servers, mcp_host_list_tools,
    mcp_host_remove_server, navigate_to, neural_bridge_delete, neural_bridge_ingest,
    neural_bridge_search, neural_bridge_status, neural_bridge_toggle, parse_agent_manifest_json,
    pause_agent, payment_create_invoice, payment_create_plan, payment_get_revenue_stats,
    payment_list_plans, payment_pay_invoice, remove_indexed_document, replay_export_bundle,
    replay_get_bundle, replay_list_bundles, replay_toggle_recording, replay_verify_bundle,
    resume_agent, run_parallel_simulation_reports, search_documents, set_default_agent,
    start_agent, start_build, start_learning, start_research, start_simulation_with_observer,
    stop_agent, stop_computer_action, time_machine_create_checkpoint,
    time_machine_list_checkpoints, time_machine_redo, time_machine_undo, tracing_end_span,
    tracing_end_trace, tracing_get_trace, tracing_list_traces, tracing_start_span,
    tracing_start_trace, voice_get_status, voice_load_whisper_model, voice_transcribe, AppState,
    LearningSource,
};
use nexus_kernel::simulation::SimulationObserver;
use serde_json::json;
use std::{sync::Arc, thread, time::Duration};
use uuid::Uuid;

fn build_manifest(name: &str) -> String {
    json!({
        "name": name,
        "version": "2.0.0",
        "capabilities": ["web.search", "llm.query", "fs.read"],
        "fuel_budget": 10000,
        "schedule": null,
        "llm_model": "claude-sonnet-4-5"
    })
    .to_string()
}

fn build_transcendent_manifest(name: &str) -> String {
    json!({
        "name": name,
        "version": "2.0.0",
        "description": "transcendent review test",
        "capabilities": ["web.search", "llm.query", "fs.read", "self.modify", "cognitive_modify"],
        "fuel_budget": 10000,
        "autonomy_level": 6,
        "schedule": null,
        "llm_model": "claude-sonnet-4-5"
    })
    .to_string()
}

#[test]
fn test_tauri_create_agent_command() {
    let state = AppState::new();
    let created = create_agent(&state, build_manifest("my-social-poster"));
    assert!(created.is_ok());

    if let Ok(agent_id) = created {
        let parsed = uuid::Uuid::parse_str(agent_id.as_str());
        assert!(parsed.is_ok());
    }
}

#[test]
fn test_tauri_create_agent_rejects_manifest_names_outside_kernel_schema() {
    let state = AppState::new();
    let invalid_manifest = json!({
        "name": "NEXUS ORACLE",
        "version": "1.0.0",
        "description": "planner prompt",
        "capabilities": ["web.search", "web.read"],
        "fuel_budget": 1000,
        "llm_model": "qwen3.5:9b"
    })
    .to_string();

    let created = create_agent(&state, invalid_manifest);
    assert!(created.is_err());
    assert!(created
        .err()
        .unwrap_or_default()
        .contains("name must be alphanumeric plus hyphens only"));
}

#[test]
fn test_nexus_operator_manifest_parses() {
    let raw =
        std::fs::read_to_string("../../agents/prebuilt/nexus-operator.json").unwrap_or_else(|e| {
            eprintln!("read_to_string failed: {e}");
            std::process::exit(1)
        });
    let manifest = parse_agent_manifest_json(&raw).unwrap_or_else(|e| {
        eprintln!("manifest parse failed: {e}");
        std::process::exit(1)
    });
    assert_eq!(manifest.name, "nexus-operator");
    assert!(manifest.capabilities.contains(&"computer.use".to_string()));
    assert!(manifest
        .capabilities
        .contains(&"screen.capture".to_string()));
    assert_eq!(manifest.autonomy_level, Some(4));
}

struct TestSimulationObserver;

impl SimulationObserver for TestSimulationObserver {}

#[test]
fn test_create_simulation_command() {
    let state = AppState::new_in_memory();
    let world_id = create_simulation(
        &state,
        "Forecast".to_string(),
        "Policy X is heading toward a major vote.".to_string(),
        12,
        4,
        None,
    )
    .unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    let status = get_simulation_status(&state, world_id.clone()).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    assert_eq!(status.world_id, world_id);
    assert_eq!(status.persona_count, 12);
    assert_eq!(status.max_ticks, 4);
}

#[test]
fn test_simulation_inject_variable_updates_status() {
    let state = AppState::new_in_memory();
    let world_id = create_simulation(
        &state,
        "Injectable".to_string(),
        "A policy scenario with uncertainty.".to_string(),
        10,
        3,
        None,
    )
    .unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    inject_simulation_variable(
        &state,
        world_id.clone(),
        "policy_signal".to_string(),
        "passed".to_string(),
    )
    .unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    let status = get_simulation_status(&state, world_id).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    assert_eq!(
        status.variables.get("policy_signal"),
        Some(&"passed".to_string())
    );
}

#[test]
fn test_start_simulation_produces_report() {
    let state = AppState::new_in_memory();
    let world_id = create_simulation(
        &state,
        "Runtime".to_string(),
        "A governance forecast with multiple stakeholders.".to_string(),
        8,
        2,
        None,
    )
    .unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    let row = state
        .db
        .load_simulation_world(&world_id)
        .unwrap_or_else(|e| {
            eprintln!("operation failed: {e}");
            std::process::exit(1)
        })
        .unwrap_or_else(|| {
            eprintln!("simulation world not found");
            std::process::exit(1)
        });
    let mut persisted = super::load_persisted_simulation_state(&row).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    persisted.tick_interval_ms = 0;
    state
        .db
        .save_simulation_world(
            &row.id,
            &row.name,
            &row.seed_text,
            "ready",
            row.tick_count,
            row.persona_count,
            &serde_json::to_string(&persisted).unwrap_or_else(|_| "{}".to_string()),
            row.report_json.as_deref(),
            row.completed_at.as_deref(),
        )
        .unwrap_or_else(|e| {
            eprintln!("operation failed: {e}");
            std::process::exit(1)
        });
    start_simulation_with_observer(&state, world_id.clone(), Arc::new(TestSimulationObserver))
        .unwrap_or_else(|e| {
            eprintln!("operation failed: {e}");
            std::process::exit(1)
        });
    for _ in 0..50 {
        if let Ok(report) = get_simulation_report(&state, world_id.clone()) {
            assert!(report.confidence > 0.0);
            return;
        }
        thread::sleep(Duration::from_millis(20));
    }
    eprintln!("simulation report was not generated in time");
    std::process::exit(1);
}

#[test]
fn test_list_simulations_and_parallel_reports() {
    let state = AppState::new_in_memory();
    create_simulation(
        &state,
        "Listed".to_string(),
        "Market conditions are shifting.".to_string(),
        10,
        3,
        None,
    )
    .unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    let summaries = list_simulations(&state).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    assert_eq!(summaries.len(), 1);
    let reports =
        run_parallel_simulation_reports(&state, "Macro outlook with rate pressure.".to_string(), 3)
            .unwrap_or_else(|e| {
                eprintln!("operation failed: {e}");
                std::process::exit(1)
            });
    assert_eq!(reports.len(), 3);
}

#[test]
fn test_nexus_prophet_manifest_parses() {
    let raw =
        std::fs::read_to_string("../../agents/prebuilt/nexus-prophet.json").unwrap_or_else(|e| {
            eprintln!("read_to_string failed: {e}");
            std::process::exit(1)
        });
    let manifest = parse_agent_manifest_json(&raw).unwrap_or_else(|e| {
        eprintln!("manifest parse failed: {e}");
        std::process::exit(1)
    });
    assert_eq!(manifest.name, "nexus-prophet");
    assert!(manifest.capabilities.contains(&"web.search".to_string()));
    assert!(manifest.capabilities.contains(&"self.modify".to_string()));
    assert_eq!(manifest.autonomy_level, Some(4));
}

#[test]
fn test_stop_computer_action_updates_status_surface() {
    let state = AppState::new_in_memory();
    {
        let mut engine = state
            .computer_control
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        engine.enable();
    }
    stop_computer_action(&state, "session-1".to_string()).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    let status = get_input_control_status(&state).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    assert!(!status.enabled);
}

#[test]
fn test_tauri_create_l6_agent_requests_review() {
    let state = AppState::new_in_memory();
    let created = create_agent(&state, build_transcendent_manifest("transcendent-pending"));
    assert!(created.is_ok());
    let created = created.unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    assert!(created.starts_with("approval-requested:"));

    let pending = state.db.load_pending_consent().unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].operation_type, "transcendent_creation");
    assert!(pending[0]
        .operation_json
        .contains("\"min_review_seconds\":60"));
}

#[test]
fn test_tauri_list_agents() {
    let state = AppState::new_in_memory();
    let baseline = list_agents(&state).map(|a| a.len()).unwrap_or(0);

    let a = create_agent(&state, build_manifest("a-agent"));
    assert!(a.is_ok());
    let b = create_agent(&state, build_manifest("b-agent"));
    assert!(b.is_ok());
    let c = create_agent(&state, build_manifest("c-agent"));
    assert!(c.is_ok());

    let listed = list_agents(&state);
    assert!(listed.is_ok());

    if let Ok(agents) = listed {
        assert_eq!(agents.len(), baseline + 3);
    }
}

#[test]
fn test_new_l6_manifests_parse() {
    let files = [
        "ascendant.json",
        "architect_prime.json",
        "oracle_supreme.json",
        "warden.json",
        "genesis_prime.json",
        "legion.json",
        "oracle_omega.json",
        "arbiter.json",
        "continuum.json",
        "nexus_prime.json",
    ];
    let paths = list_prebuilt_manifest_paths();

    for file in files {
        let path = paths
            .iter()
            .find(|path| path.file_name().and_then(|name| name.to_str()) == Some(file))
            .unwrap_or_else(|| {
                eprintln!("manifest should exist: {file}");
                std::process::exit(1)
            });
        let raw = std::fs::read_to_string(path).unwrap_or_else(|e| {
            eprintln!("manifest {file} should be readable: {e}");
            std::process::exit(1);
        });
        let manifest = parse_agent_manifest_json(&raw).unwrap_or_else(|e| {
            eprintln!("manifest {file} failed to parse: {e}");
            std::process::exit(1);
        });
        assert_eq!(manifest.autonomy_level, Some(6));
    }
}

#[test]
fn test_new_l6_manifests_have_comprehensive_descriptions() {
    let files = [
        "ascendant.json",
        "architect_prime.json",
        "oracle_supreme.json",
        "warden.json",
        "genesis_prime.json",
        "legion.json",
        "oracle_omega.json",
        "arbiter.json",
        "continuum.json",
        "nexus_prime.json",
    ];
    let paths = list_prebuilt_manifest_paths();

    for file in files {
        let path = paths
            .iter()
            .find(|path| path.file_name().and_then(|name| name.to_str()) == Some(file))
            .unwrap_or_else(|| {
                eprintln!("manifest should exist: {file}");
                std::process::exit(1)
            });
        let raw = std::fs::read_to_string(path).unwrap_or_else(|e| {
            eprintln!("manifest {file} should be readable: {e}");
            std::process::exit(1);
        });
        parse_agent_manifest_json(&raw).unwrap_or_else(|e| {
            eprintln!("manifest {file} failed to parse: {e}");
            std::process::exit(1);
        });
        let description = super::parse_manifest_description(&raw);
        let word_count = description.split_whitespace().count();
        assert!(
            word_count >= 500,
            "manifest {file} should have at least 500 words, found {word_count}"
        );
    }
}

#[test]
fn test_prebuilt_manifest_count_is_nonzero() {
    let paths = list_prebuilt_manifest_paths();
    assert!(!paths.is_empty());
}

#[test]
fn test_load_prebuilt_agents_registers_every_manifest() {
    let state = AppState::new_in_memory();
    state.load_prebuilt_agents();
    let agents = state.db.list_agents().unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    assert_eq!(agents.len(), list_prebuilt_manifest_paths().len());
}

#[test]
fn test_load_prebuilt_agents_skips_duplicate_names() {
    let state = AppState::new_in_memory();
    state.load_prebuilt_agents();
    state.load_prebuilt_agents();
    let agents = state.db.list_agents().unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    assert_eq!(agents.len(), list_prebuilt_manifest_paths().len());
}

#[test]
fn test_list_agents_includes_stopped_prebuilt_agents_from_persistence() {
    let state = AppState::new_in_memory();
    state.load_prebuilt_agents();

    let agents = list_agents(&state).unwrap_or_else(|e| {
        eprintln!("list_agents should succeed: {e}");
        std::process::exit(1)
    });
    let manifest_count = list_prebuilt_manifest_paths().len();

    assert_eq!(agents.len(), manifest_count);
    assert!(agents.iter().all(|agent| !agent.id.trim().is_empty()));
    assert!(agents.iter().any(|agent| agent.name == "nexus-oracle"));
}

#[test]
fn test_get_preinstalled_agents_keeps_persisted_agent_ids() {
    let state = AppState::new_in_memory();
    state.load_prebuilt_agents();

    let agents = super::get_preinstalled_agents(&state).unwrap_or_else(|e| {
        eprintln!("preinstalled agent query should succeed: {e}");
        std::process::exit(1)
    });
    let manifest_count = list_prebuilt_manifest_paths().len();

    assert_eq!(agents.len(), manifest_count);
    assert!(agents.iter().all(|agent| !agent.agent_id.trim().is_empty()));
    assert!(agents.iter().any(|agent| agent.name == "nexus-oracle"));
}

#[test]
fn test_start_l6_agent_requests_review_instead_of_restarting() {
    let state = AppState::new_in_memory();
    let created = create_agent_immediately(
        &state,
        parse_agent_manifest_json(&build_transcendent_manifest("transcendent-start"))
            .unwrap_or_else(|e| {
                eprintln!("manifest parse failed: {e}");
                std::process::exit(1)
            }),
        build_transcendent_manifest("transcendent-start"),
    )
    .unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });

    stop_agent(&state, created.clone()).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    start_agent(&state, created.clone()).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });

    let pending = state.db.load_pending_consent().unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    assert_eq!(pending.len(), 1);
    assert!(pending[0].operation_json.contains("activate_existing"));

    let listed = list_agents(&state).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    let agent = listed
        .iter()
        .find(|row| row.id == created)
        .unwrap_or_else(|| {
            eprintln!("unexpected None");
            std::process::exit(1)
        });
    assert_eq!(agent.status, "Stopped");
}

#[test]
fn test_tauri_pause_and_resume() {
    let state = AppState::new_in_memory();
    let created = create_agent(&state, build_manifest("voice-agent"));
    assert!(created.is_ok());

    if let Ok(agent_id) = created {
        let paused = pause_agent(&state, agent_id.clone());
        assert!(paused.is_ok());

        let paused_rows = list_agents(&state).unwrap_or_else(|e| {
            eprintln!("list should succeed: {e}");
            std::process::exit(1)
        });
        let target = paused_rows
            .iter()
            .find(|a| a.id == agent_id)
            .unwrap_or_else(|| {
                eprintln!("agent should exist");
                std::process::exit(1)
            });
        assert_eq!(target.status, "Paused");
        assert_eq!(target.last_action, "paused");

        let resumed = resume_agent(&state, agent_id.clone());
        assert!(resumed.is_ok());

        let resumed_rows = list_agents(&state).unwrap_or_else(|e| {
            eprintln!("list should succeed: {e}");
            std::process::exit(1)
        });
        let target = resumed_rows
            .iter()
            .find(|a| a.id == agent_id)
            .unwrap_or_else(|| {
                eprintln!("agent should exist");
                std::process::exit(1)
            });
        assert_eq!(target.status, "Running");
        assert_eq!(target.last_action, "resumed");
    }
}

#[test]
fn test_cleanup_legacy_agent_db_only_once() {
    let temp = std::env::temp_dir().join(format!("nexus-cleanup-test-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&temp).unwrap_or_else(|e| {
        eprintln!("create_dir_all failed: {e}");
        std::process::exit(1)
    });
    let db_path = temp.join("nexus.db");
    let flag_path = temp.join(".cleanup-flag");

    std::fs::write(&db_path, "stale-db").unwrap_or_else(|e| {
        eprintln!("fs::write failed: {e}");
        std::process::exit(1)
    });
    super::cleanup_legacy_agent_db_if_needed(&db_path, &flag_path);
    assert!(!db_path.exists());
    assert!(flag_path.exists());

    std::fs::write(&db_path, "fresh-db").unwrap_or_else(|e| {
        eprintln!("fs::write failed: {e}");
        std::process::exit(1)
    });
    super::cleanup_legacy_agent_db_if_needed(&db_path, &flag_path);
    assert!(db_path.exists());
    let _ = std::fs::remove_dir_all(&temp);
}

// ── Browser Navigate Tests ──

#[test]
fn test_browser_navigate_logs_audit() {
    let state = AppState::new();
    let result = navigate_to(&state, "https://docs.rust-lang.org/".to_string());
    assert!(result.is_ok());
    let nav = result.unwrap_or_else(|e| {
        eprintln!("command failed: {e}");
        std::process::exit(1)
    });
    assert!(nav.allowed);
    assert_eq!(nav.url, "https://docs.rust-lang.org/");

    // History should have one entry
    let hist = get_browser_history(&state).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    assert_eq!(hist.len(), 1);
    assert_eq!(hist[0].url, "https://docs.rust-lang.org/");

    // Activity log should have recorded the visit
    let activity = get_agent_activity(&state).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    assert!(!activity.is_empty());

    // Audit trail should have at least one event
    let audit = state.audit.lock().unwrap_or_else(|p| p.into_inner());
    assert!(!audit.events().is_empty());
}

#[test]
fn test_browser_blocked_domain_returns_error() {
    let state = AppState::new();
    let result = navigate_to(&state, "https://malware.example.com/payload".to_string());
    assert!(result.is_ok());
    let nav = result.unwrap_or_else(|e| {
        eprintln!("command failed: {e}");
        std::process::exit(1)
    });
    assert!(!nav.allowed);
    assert!(nav.deny_reason.is_some());
    assert!(nav
        .deny_reason
        .unwrap_or_else(|| {
            eprintln!("expected deny_reason");
            std::process::exit(1)
        })
        .contains("blocked by egress policy"));
}

#[test]
fn test_browser_invalid_protocol_blocked() {
    let state = AppState::new();
    let result = navigate_to(&state, "ftp://files.example.com/data".to_string());
    assert!(result.is_ok());
    let nav = result.unwrap_or_else(|e| {
        eprintln!("command failed: {e}");
        std::process::exit(1)
    });
    assert!(!nav.allowed);
}

// ── Research Session Tests ──

#[test]
fn test_research_session_creates_multiple_agents() {
    let state = AppState::new();
    let result = start_research(&state, "Rust async patterns".to_string(), 3);
    assert!(result.is_ok());
    let session = result.unwrap_or_else(|e| {
        eprintln!("command failed: {e}");
        std::process::exit(1)
    });
    assert_eq!(session.sub_agents.len(), 3);
    assert_eq!(session.status, "running");
    assert_eq!(session.topic, "Rust async patterns");

    // Each agent should have a unique ID and a query
    let ids: Vec<_> = session.sub_agents.iter().map(|a| &a.agent_id).collect();
    let unique: std::collections::HashSet<_> = ids.iter().collect();
    assert_eq!(unique.len(), 3, "agent IDs should be unique");

    for agent in &session.sub_agents {
        assert!(!agent.query.is_empty());
        assert_eq!(agent.status, "searching");
    }
}

#[test]
fn test_research_complete_merges_findings() {
    let state = AppState::new();
    let session = start_research(&state, "WebAssembly".to_string(), 2).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    let result = complete_research(&state, session.session_id);
    assert!(result.is_ok());
    let completed = result.unwrap_or_else(|e| {
        eprintln!("command failed: {e}");
        std::process::exit(1)
    });
    assert_eq!(completed.status, "complete");
    assert!(completed.total_fuel_used > 0);
}

// ── Build Session Tests ──

#[test]
fn test_build_session_streams_code() {
    let state = AppState::new();
    let session = start_build(&state, "Dashboard widget".to_string()).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    assert_eq!(session.status, "planning");
    assert!(!session.messages.is_empty());

    // Complete the build
    let result = complete_build(&state, session.session_id);
    assert!(result.is_ok());
    let completed = result.unwrap_or_else(|e| {
        eprintln!("command failed: {e}");
        std::process::exit(1)
    });
    assert_eq!(completed.status, "complete");
}

// ── Learning Session Tests ──

#[test]
fn test_learning_session_extracts_takeaways() {
    let state = AppState::new();
    let sources = vec![
        LearningSource {
            url: "https://docs.rust-lang.org/stable/".to_string(),
            label: "Rust Docs".to_string(),
            category: "documentation".to_string(),
        },
        LearningSource {
            url: "https://blog.rust-lang.org/".to_string(),
            label: "Rust Blog".to_string(),
            category: "blog".to_string(),
        },
    ];

    let session = start_learning(&state, sources).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    assert_eq!(session.status, "browsing");
    assert_eq!(session.sources.len(), 2);

    // Browse first source
    let browsed = learning_agent_action(
        &state,
        session.session_id.clone(),
        "browse".to_string(),
        Some("https://docs.rust-lang.org/stable/".to_string()),
        None,
    )
    .unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    assert_eq!(browsed.pages_visited, 1);
    assert!(browsed.fuel_used > 0);

    // Extract from it
    let extracted = learning_agent_action(
        &state,
        session.session_id.clone(),
        "extract".to_string(),
        Some("https://docs.rust-lang.org/stable/".to_string()),
        Some("Rust 1.78 adds diagnostic attributes".to_string()),
    )
    .unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    assert_eq!(extracted.knowledge_base.len(), 1);
    assert!(extracted.knowledge_base[0]
        .key_points
        .iter()
        .any(|p| p.contains("diagnostic")));

    // Compare with existing knowledge
    let compared = learning_agent_action(
        &state,
        session.session_id.clone(),
        "compare".to_string(),
        None,
        None,
    )
    .unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    assert!(compared.knowledge_base[0].is_new);

    // Complete session
    let done = learning_agent_action(
        &state,
        session.session_id.clone(),
        "done".to_string(),
        None,
        None,
    )
    .unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    assert_eq!(done.status, "complete");

    // Global knowledge base should now have entries
    let kb = get_knowledge_base(&state).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    assert!(!kb.is_empty());
}

#[test]
fn test_learning_blocked_source_rejected() {
    let state = AppState::new();
    let sources = vec![LearningSource {
        url: "https://phishing.evil.com/".to_string(),
        label: "Bad Source".to_string(),
        category: "blog".to_string(),
    }];

    let result = start_learning(&state, sources);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("blocked"));
}

#[test]
fn test_learning_browse_blocked_url() {
    let state = AppState::new();
    let sources = vec![LearningSource {
        url: "https://docs.rust-lang.org/".to_string(),
        label: "Rust Docs".to_string(),
        category: "documentation".to_string(),
    }];

    let session = start_learning(&state, sources).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });

    // Try browsing a blocked URL during the session
    let result = learning_agent_action(
        &state,
        session.session_id,
        "browse".to_string(),
        Some("https://darkweb.example.com/".to_string()),
        None,
    );
    assert!(result.is_err());
}

#[test]
fn test_get_configured_provider_fallback() {
    // Without Ollama running or API keys set, should fall back to MockProvider.
    let provider = get_configured_provider();
    // In CI / test environments, mock is the expected fallback.
    // If a real provider is configured, that's fine too — just verify it returns something.
    assert!(!provider.name().is_empty());
}

#[test]
fn test_chat_with_documents_returns_answer() {
    // This test requires a working LLM provider with embedding models.
    // Skip gracefully when Ollama is not available or has no models (CI, etc.).
    let ollama = nexus_connectors_llm::providers::OllamaProvider::from_env();
    let has_embedding_model = ollama
        .health_check()
        .ok()
        .filter(|&ok| ok)
        .and_then(|_| ollama.list_models().ok())
        .map(|models| models.iter().any(|m| m.name.contains("nomic-embed")))
        .unwrap_or(false);

    if !has_embedding_model {
        eprintln!("SKIPPED: Ollama not available or nomic-embed-text model not installed at localhost:11434");
        return;
    }

    let state = AppState::new();

    // Write a temp file to index
    let tmp = std::env::temp_dir().join("nexus_rag_test_chat.txt");
    std::fs::write(
        &tmp,
        "Rust is a systems programming language focused on safety.",
    )
    .unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });

    // Index the document
    let ingest_result = index_document(&state, tmp.to_string_lossy().to_string());
    assert!(
        ingest_result.is_ok(),
        "ingest failed: {:?}",
        ingest_result.err()
    );

    // Chat with documents
    let chat_result = chat_with_documents(&state, "What is Rust?".to_string());
    assert!(chat_result.is_ok(), "chat failed: {:?}", chat_result.err());

    let parsed: serde_json::Value = serde_json::from_str(&chat_result.unwrap_or_else(|e| e))
        .unwrap_or_else(|e| {
            eprintln!("JSON parse failed: {e}");
            std::process::exit(1)
        });
    assert!(parsed.get("answer").is_some());
    assert!(parsed.get("sources").is_some());
    assert!(parsed.get("model").is_some());
    assert!(parsed.get("tokens").is_some());

    let _ = std::fs::remove_file(&tmp);
    // Note: don't remove LLM_PROVIDER — tests run in parallel in the same process.
}

#[test]
fn test_provider_status_command() {
    let state = AppState::new();
    let result = get_active_llm_provider(&state);
    assert!(result.is_ok());

    let parsed: serde_json::Value = serde_json::from_str(&result.unwrap_or_else(|e| e))
        .unwrap_or_else(|e| {
            eprintln!("JSON parse failed: {e}");
            std::process::exit(1)
        });
    assert!(parsed.get("provider").is_some());
    assert!(parsed.get("model").is_some());
    assert!(parsed.get("embedding_model").is_some());
    assert!(parsed.get("status").is_some());
    assert!(parsed.get("message").is_some());

    let provider = parsed["provider"].as_str().unwrap_or_else(|| {
        eprintln!("expected string value");
        std::process::exit(1)
    });
    assert!(!provider.is_empty());
}

// ── RAG wiring tests ────────────────────────────────────────────────

#[test]
fn test_index_document_end_to_end() {
    std::env::set_var("LLM_PROVIDER", "mock");
    // Mock provider falls back to Ollama for embeddings; skip if unavailable.
    let ollama = nexus_connectors_llm::providers::OllamaProvider::from_env();
    let has_embed = ollama
        .health_check()
        .ok()
        .filter(|&ok| ok)
        .and_then(|_| ollama.list_models().ok())
        .map(|models| models.iter().any(|m| m.name.contains("nomic-embed")))
        .unwrap_or(false);
    if !has_embed {
        eprintln!("SKIPPED: Ollama embedding model not available");
        return;
    }
    let state = AppState::new();
    let tmp = std::env::temp_dir().join("nexus_test_index_e2e.md");
    std::fs::write(&tmp, "# Heading\n\nSome markdown content about Nexus OS.").unwrap_or_else(
        |e| {
            eprintln!("fs::write failed: {e}");
            std::process::exit(1)
        },
    );

    let result = index_document(&state, tmp.to_string_lossy().to_string());
    assert!(result.is_ok(), "index_document failed: {:?}", result.err());

    let parsed: serde_json::Value = serde_json::from_str(&result.unwrap_or_else(|e| e))
        .unwrap_or_else(|e| {
            eprintln!("JSON parse failed: {e}");
            std::process::exit(1)
        });
    assert!(
        parsed["chunk_count"].as_u64().unwrap_or_else(|| {
            eprintln!("expected u64 value");
            std::process::exit(1)
        }) > 0
    );
    assert_eq!(
        parsed["path"].as_str().unwrap_or_else(|| {
            eprintln!("expected string value");
            std::process::exit(1)
        }),
        tmp.to_string_lossy()
    );

    let _ = std::fs::remove_file(&tmp);
    // Note: don't remove LLM_PROVIDER — tests run in parallel in the same process.
}

#[test]
fn test_search_documents_end_to_end() {
    std::env::set_var("LLM_PROVIDER", "mock");
    let ollama = nexus_connectors_llm::providers::OllamaProvider::from_env();
    let has_embed = ollama
        .health_check()
        .ok()
        .filter(|&ok| ok)
        .and_then(|_| ollama.list_models().ok())
        .map(|models| models.iter().any(|m| m.name.contains("nomic-embed")))
        .unwrap_or(false);
    if !has_embed {
        eprintln!("SKIPPED: Ollama embedding model not available");
        return;
    }
    let state = AppState::new();
    let tmp = std::env::temp_dir().join("nexus_test_search_e2e.txt");
    std::fs::write(
        &tmp,
        "Quantum computing uses qubits for parallel computation.",
    )
    .unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });

    let _ = index_document(&state, tmp.to_string_lossy().to_string()).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    let result = search_documents(&state, "quantum".to_string(), Some(5));
    assert!(result.is_ok(), "search failed: {:?}", result.err());

    let parsed: Vec<serde_json::Value> = serde_json::from_str(&result.unwrap_or_else(|e| e))
        .unwrap_or_else(|e| {
            eprintln!("JSON parse failed: {e}");
            std::process::exit(1)
        });
    // MockProvider embeddings may not produce high cosine similarity for all queries,
    // so we only verify the response parses as an array of valid result objects.
    for r in &parsed {
        assert!(r.get("chunk_id").is_some());
        assert!(r.get("score").is_some());
    }

    let _ = std::fs::remove_file(&tmp);
    // Note: don't remove LLM_PROVIDER — tests run in parallel in the same process.
}

#[test]
fn test_list_indexed_documents_two_docs() {
    std::env::set_var("LLM_PROVIDER", "mock");
    let ollama = nexus_connectors_llm::providers::OllamaProvider::from_env();
    let has_embed = ollama
        .health_check()
        .ok()
        .filter(|&ok| ok)
        .and_then(|_| ollama.list_models().ok())
        .map(|models| models.iter().any(|m| m.name.contains("nomic-embed")))
        .unwrap_or(false);
    if !has_embed {
        eprintln!("SKIPPED: Ollama embedding model not available");
        return;
    }
    let state = AppState::new();
    let tmp1 = std::env::temp_dir().join("nexus_test_list_a.txt");
    let tmp2 = std::env::temp_dir().join("nexus_test_list_b.txt");
    std::fs::write(&tmp1, "Document A content.").unwrap_or_else(|e| {
        eprintln!("fs::write failed: {e}");
        std::process::exit(1)
    });
    std::fs::write(&tmp2, "Document B content.").unwrap_or_else(|e| {
        eprintln!("fs::write failed: {e}");
        std::process::exit(1)
    });

    let _ = index_document(&state, tmp1.to_string_lossy().to_string()).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    let _ = index_document(&state, tmp2.to_string_lossy().to_string()).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });

    let result = list_indexed_documents(&state).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    let parsed: Vec<serde_json::Value> = serde_json::from_str(&result).unwrap_or_else(|e| {
        eprintln!("JSON parse failed: {e}");
        std::process::exit(1)
    });
    assert_eq!(parsed.len(), 2);

    let _ = std::fs::remove_file(&tmp1);
    let _ = std::fs::remove_file(&tmp2);
    // Note: don't remove LLM_PROVIDER — tests run in parallel in the same process.
}

#[test]
fn test_remove_indexed_document() {
    std::env::set_var("LLM_PROVIDER", "mock");
    let ollama = nexus_connectors_llm::providers::OllamaProvider::from_env();
    let has_embed = ollama
        .health_check()
        .ok()
        .filter(|&ok| ok)
        .and_then(|_| ollama.list_models().ok())
        .map(|models| models.iter().any(|m| m.name.contains("nomic-embed")))
        .unwrap_or(false);
    if !has_embed {
        eprintln!("SKIPPED: Ollama embedding model not available");
        return;
    }
    let state = AppState::new();
    let tmp = std::env::temp_dir().join("nexus_test_remove.txt");
    std::fs::write(&tmp, "Content to be removed.").unwrap_or_else(|e| {
        eprintln!("fs::write failed: {e}");
        std::process::exit(1)
    });
    let path_str = tmp.to_string_lossy().to_string();

    let _ = index_document(&state, path_str.clone()).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    let result = remove_indexed_document(&state, path_str).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap_or_else(|e| {
        eprintln!("JSON parse failed: {e}");
        std::process::exit(1)
    });
    assert!(parsed["removed"].as_bool().unwrap_or_else(|| {
        eprintln!("expected bool value");
        std::process::exit(1)
    }));

    let list = list_indexed_documents(&state).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    let docs: Vec<serde_json::Value> = serde_json::from_str(&list).unwrap_or_else(|e| {
        eprintln!("JSON parse failed: {e}");
        std::process::exit(1)
    });
    assert!(docs.is_empty());

    let _ = std::fs::remove_file(&tmp);
    // Note: don't remove LLM_PROVIDER — tests run in parallel in the same process.
}

// ── Model Hub wiring tests ──────────────────────────────────────────

#[test]
fn test_list_local_models_returns_array() {
    let state = AppState::new();
    let result = list_local_models(&state);
    assert!(result.is_ok());
    // Must parse as a JSON array (may be empty)
    let _: Vec<serde_json::Value> = serde_json::from_str(&result.unwrap_or_else(|e| e))
        .unwrap_or_else(|e| {
            eprintln!("JSON parse failed: {e}");
            std::process::exit(1)
        });
}

#[test]
fn test_get_system_specs_has_fields() {
    let result = get_system_specs();
    assert!(result.is_ok());
    let parsed: serde_json::Value = serde_json::from_str(&result.unwrap_or_else(|e| e))
        .unwrap_or_else(|e| {
            eprintln!("JSON parse failed: {e}");
            std::process::exit(1)
        });
    assert!(parsed.get("total_ram_mb").is_some());
    assert!(parsed.get("cpu_name").is_some());
    assert!(parsed.get("cpu_cores").is_some());
    assert!(
        parsed["total_ram_mb"].as_u64().unwrap_or_else(|| {
            eprintln!("expected u64 value");
            std::process::exit(1)
        }) > 0
    );
}

#[test]
fn test_get_live_system_metrics_has_fields() {
    let state = AppState::new();
    let result = get_live_system_metrics(&state);
    assert!(result.is_ok());
    let parsed: serde_json::Value = serde_json::from_str(&result.unwrap_or_else(|e| e))
        .unwrap_or_else(|e| {
            eprintln!("JSON parse failed: {e}");
            std::process::exit(1)
        });
    assert!(parsed.get("cpu_avg").is_some());
    assert!(parsed.get("cpu_cores").is_some());
    assert!(parsed.get("total_ram").is_some());
    assert!(parsed.get("used_ram").is_some());
    assert!(parsed.get("uptime_secs").is_some());
    assert!(parsed.get("process_count").is_some());
    assert!(parsed.get("agents").is_some());
    assert!(
        parsed["total_ram"].as_u64().unwrap_or_else(|| {
            eprintln!("expected u64 value");
            std::process::exit(1)
        }) > 0
    );
}

#[test]
fn test_check_model_compatibility() {
    let state = AppState::new();
    // 500 MB file
    let result = check_model_compatibility(&state, 500_000_000);
    assert!(result.is_ok());
    let parsed: serde_json::Value = serde_json::from_str(&result.unwrap_or_else(|e| e))
        .unwrap_or_else(|e| {
            eprintln!("JSON parse failed: {e}");
            std::process::exit(1)
        });
    assert!(parsed.get("can_run").is_some());
}

// ── Time Machine wiring tests ───────────────────────────────────────

#[test]
fn test_time_machine_create_and_list_checkpoints() {
    let state = AppState::new();
    let baseline: Vec<serde_json::Value> =
        serde_json::from_str(&time_machine_list_checkpoints(&state).unwrap_or_else(|e| {
            eprintln!("JSON parse failed: {e}");
            std::process::exit(1)
        }))
        .unwrap_or_else(|e| {
            eprintln!("deserialization failed: {e}");
            std::process::exit(1)
        });
    let baseline_count = baseline.len();

    let created = time_machine_create_checkpoint(&state, "test-checkpoint".to_string());
    assert!(created.is_ok());
    let cp_id = created.unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    assert!(!cp_id.is_empty());

    let list_result = time_machine_list_checkpoints(&state).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    let parsed: Vec<serde_json::Value> = serde_json::from_str(&list_result).unwrap_or_else(|e| {
        eprintln!("JSON parse failed: {e}");
        std::process::exit(1)
    });
    assert!(
        parsed.len() == baseline_count || parsed.len() == baseline_count + 1,
        "checkpoint list should either grow by one or evict at capacity"
    );
    // Our checkpoint should be the last one
    let last = parsed.last().unwrap_or_else(|| {
        eprintln!("unexpected None");
        std::process::exit(1)
    });
    assert_eq!(
        last["label"].as_str().unwrap_or_else(|| {
            eprintln!("expected string value");
            std::process::exit(1)
        }),
        "test-checkpoint"
    );
    assert_eq!(
        last["id"].as_str().unwrap_or_else(|| {
            eprintln!("expected string value");
            std::process::exit(1)
        }),
        cp_id
    );
}

#[test]
fn test_time_machine_undo_empty() {
    // Use a fresh supervisor directly to avoid checkpoints created
    // during agent restoration from the persistence DB.
    let mut sup = nexus_kernel::supervisor::Supervisor::new();
    let result = sup.time_machine_mut().undo();
    assert!(result.is_err());
}

#[test]
fn test_time_machine_create_undo_redo_cycle() {
    let state = AppState::new();

    let _ = time_machine_create_checkpoint(&state, "cycle-test".to_string()).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });

    // Undo
    let undo_result = time_machine_undo(&state);
    assert!(undo_result.is_ok());
    let undo_parsed: serde_json::Value = serde_json::from_str(&undo_result.unwrap_or_else(|e| {
        eprintln!("JSON parse failed: {e}");
        std::process::exit(1)
    }))
    .unwrap_or_else(|e| {
        eprintln!("deserialization failed: {e}");
        std::process::exit(1)
    });
    assert_eq!(
        undo_parsed["label"].as_str().unwrap_or_else(|| {
            eprintln!("expected string value");
            std::process::exit(1)
        }),
        "cycle-test"
    );

    // Redo
    let redo_result = time_machine_redo(&state);
    assert!(redo_result.is_ok());
    let redo_parsed: serde_json::Value = serde_json::from_str(&redo_result.unwrap_or_else(|e| {
        eprintln!("JSON parse failed: {e}");
        std::process::exit(1)
    }))
    .unwrap_or_else(|e| {
        eprintln!("deserialization failed: {e}");
        std::process::exit(1)
    });
    assert_eq!(
        redo_parsed["label"].as_str().unwrap_or_else(|| {
            eprintln!("expected string value");
            std::process::exit(1)
        }),
        "cycle-test"
    );
}

// ── Voice wiring tests ──────────────────────────────────────────────

#[test]
fn test_voice_get_status_json() {
    let state = AppState::new();
    let result = voice_get_status(&state);
    assert!(result.is_ok());
    let parsed: serde_json::Value = serde_json::from_str(&result.unwrap_or_else(|e| e))
        .unwrap_or_else(|e| {
            eprintln!("JSON parse failed: {e}");
            std::process::exit(1)
        });
    assert!(parsed.get("is_listening").is_some());
    assert!(parsed.get("wake_word").is_some());
    assert!(parsed.get("python_server_running").is_some());
    assert!(parsed.get("whisper_loaded").is_some());
    assert!(parsed.get("transcription_engine").is_some());
    // Default state: whisper not loaded, engine is stub
    assert_eq!(parsed["whisper_loaded"].as_bool(), Some(false));
    assert_eq!(parsed["transcription_engine"].as_str(), Some("stub"));
}

#[test]
fn test_voice_transcribe_fallback_stub() {
    std::env::set_var("LLM_PROVIDER", "mock");
    let state = AppState::new();
    // With no whisper model loaded and no python server, should return clear error
    let result = voice_transcribe(&state, "AAAA".to_string());
    assert!(result.is_ok());
    let parsed: serde_json::Value = serde_json::from_str(&result.unwrap_or_else(|e| e))
        .unwrap_or_else(|e| {
            eprintln!("JSON parse failed: {e}");
            std::process::exit(1)
        });
    assert_eq!(
        parsed["text"].as_str(),
        Some("Voice transcription requires Whisper model - load via Model Hub")
    );
    assert_eq!(parsed["engine"].as_str(), Some("none"));
    assert!(parsed.get("duration_ms").is_some());
    assert_eq!(parsed["error"].as_bool(), Some(true));
}

#[test]
fn test_voice_load_whisper_model_missing() {
    let state = AppState::new();
    let result = voice_load_whisper_model(&state, "/nonexistent/whisper/model".to_string());
    assert!(result.is_err());
    // Whisper should still not be loaded
    let status = voice_get_status(&state).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    let parsed: serde_json::Value = serde_json::from_str(&status).unwrap_or_else(|e| {
        eprintln!("JSON parse failed: {e}");
        std::process::exit(1)
    });
    assert_eq!(parsed["whisper_loaded"].as_bool(), Some(false));
}

#[test]
fn test_voice_transcribe_returns_engine_field() {
    std::env::set_var("LLM_PROVIDER", "mock");
    let state = AppState::new();
    // Send some base64 data (doesn't matter what — stub ignores content)
    let result = voice_transcribe(&state, "SGVsbG8gV29ybGQ=".to_string());
    assert!(result.is_ok());
    let parsed: serde_json::Value = serde_json::from_str(&result.unwrap_or_else(|e| e))
        .unwrap_or_else(|e| {
            eprintln!("JSON parse failed: {e}");
            std::process::exit(1)
        });
    // Must always have text, engine, and duration_ms
    assert!(parsed["text"].is_string());
    assert!(parsed["engine"].is_string());
    assert!(parsed["duration_ms"].is_number());
}

// ── Economy wiring tests ────────────────────────────────────────────

#[test]
fn test_economy_full_cycle() {
    let state = AppState::new();
    let agent_id = uuid::Uuid::new_v4().to_string();

    // Create wallet
    let wallet_result = economy_create_wallet(&state, agent_id.clone());
    assert!(wallet_result.is_ok());

    // Earn credits
    let earn_result = economy_earn(&state, agent_id.clone(), 100.0, "test earnings".to_string());
    assert!(earn_result.is_ok());

    // Check balance (default_balance=100 + earned=100 = 200)
    let wallet = economy_get_wallet(&state, agent_id.clone()).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    let wallet_parsed: serde_json::Value = serde_json::from_str(&wallet).unwrap_or_else(|e| {
        eprintln!("JSON parse failed: {e}");
        std::process::exit(1)
    });
    let balance = wallet_parsed["balance"].as_f64().unwrap_or_else(|| {
        eprintln!("expected f64 value");
        std::process::exit(1)
    });
    assert!((balance - 200.0).abs() < 0.01);

    // Spend credits (within default spending_limit of 10.0)
    let spend_result = economy_spend(
        &state,
        agent_id.clone(),
        5.0,
        "ApiCall".to_string(),
        "test spend".to_string(),
    );
    assert!(spend_result.is_ok());

    // Verify balance after spend (200 - 5 = 195)
    let wallet2 = economy_get_wallet(&state, agent_id.clone()).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    let w2: serde_json::Value = serde_json::from_str(&wallet2).unwrap_or_else(|e| {
        eprintln!("JSON parse failed: {e}");
        std::process::exit(1)
    });
    let balance2 = w2["balance"].as_f64().unwrap_or_else(|| {
        eprintln!("expected f64 value");
        std::process::exit(1)
    });
    assert!((balance2 - 195.0).abs() < 0.01);

    // History should have 2 transactions
    let history = economy_get_history(&state, agent_id.clone()).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    let h: Vec<serde_json::Value> = serde_json::from_str(&history).unwrap_or_else(|e| {
        eprintln!("JSON parse failed: {e}");
        std::process::exit(1)
    });
    assert_eq!(h.len(), 2);

    // Stats
    let stats = economy_get_stats(&state).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    let s: serde_json::Value = serde_json::from_str(&stats).unwrap_or_else(|e| {
        eprintln!("JSON parse failed: {e}");
        std::process::exit(1)
    });
    assert!(s.get("total_wallets").is_some());
}

#[test]
fn test_economy_transfer_between_wallets() {
    let state = AppState::new();
    let from_id = uuid::Uuid::new_v4().to_string();
    let to_id = uuid::Uuid::new_v4().to_string();

    economy_create_wallet(&state, from_id.clone()).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    economy_create_wallet(&state, to_id.clone()).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    economy_earn(&state, from_id.clone(), 200.0, "seed".to_string()).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });

    let transfer = economy_transfer(
        &state,
        from_id.clone(),
        to_id.clone(),
        50.0,
        "pay".to_string(),
    );
    assert!(transfer.is_ok());

    // from: default(100) + earn(200) - transfer(50) = 250
    let from_w = economy_get_wallet(&state, from_id).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    let from_v: serde_json::Value = serde_json::from_str(&from_w).unwrap_or_else(|e| {
        eprintln!("JSON parse failed: {e}");
        std::process::exit(1)
    });
    assert!(
        (from_v["balance"].as_f64().unwrap_or_else(|| {
            eprintln!("expected f64 value");
            std::process::exit(1)
        }) - 250.0)
            .abs()
            < 0.01
    );

    // to: default(100) + received(50) = 150
    let to_w = economy_get_wallet(&state, to_id).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    let to_v: serde_json::Value = serde_json::from_str(&to_w).unwrap_or_else(|e| {
        eprintln!("JSON parse failed: {e}");
        std::process::exit(1)
    });
    assert!(
        (to_v["balance"].as_f64().unwrap_or_else(|| {
            eprintln!("expected f64 value");
            std::process::exit(1)
        }) - 150.0)
            .abs()
            < 0.01
    );
}

#[test]
fn test_economy_freeze_wallet() {
    let state = AppState::new();
    let agent_id = uuid::Uuid::new_v4().to_string();
    economy_create_wallet(&state, agent_id.clone()).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    economy_earn(&state, agent_id.clone(), 100.0, "seed".to_string()).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });

    let freeze = economy_freeze_wallet(&state, agent_id.clone());
    assert!(freeze.is_ok());

    // Spending on frozen wallet should fail
    let spend = economy_spend(
        &state,
        agent_id,
        10.0,
        "ApiCall".to_string(),
        "test".to_string(),
    );
    assert!(spend.is_err());
}

// ── Ghost Protocol wiring tests ─────────────────────────────────────

#[test]
fn test_ghost_protocol_status_has_device_id() {
    let state = AppState::new();
    let result = ghost_protocol_status(&state).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap_or_else(|e| {
        eprintln!("JSON parse failed: {e}");
        std::process::exit(1)
    });
    assert!(parsed.get("device_id").is_some());
    assert!(parsed.get("enabled").is_some());
    assert!(parsed.get("peer_count").is_some());
    assert!(parsed.get("stats").is_some());
}

#[test]
fn test_ghost_protocol_toggle() {
    let state = AppState::new();

    let toggle = ghost_protocol_toggle(&state, true).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    let parsed: serde_json::Value = serde_json::from_str(&toggle).unwrap_or_else(|e| {
        eprintln!("JSON parse failed: {e}");
        std::process::exit(1)
    });
    assert!(parsed["enabled"].as_bool().unwrap_or_else(|| {
        eprintln!("expected bool value");
        std::process::exit(1)
    }));

    let status = ghost_protocol_status(&state).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    let s: serde_json::Value = serde_json::from_str(&status).unwrap_or_else(|e| {
        eprintln!("JSON parse failed: {e}");
        std::process::exit(1)
    });
    assert!(s["enabled"].as_bool().unwrap_or_else(|| {
        eprintln!("expected bool value");
        std::process::exit(1)
    }));
}

#[test]
fn test_ghost_protocol_add_remove_peer() {
    let state = AppState::new();
    ghost_protocol_toggle(&state, true).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });

    let add_result = ghost_protocol_add_peer(
        &state,
        "127.0.0.1:9090".to_string(),
        "test-peer".to_string(),
    );
    assert!(add_result.is_ok());
    let added: serde_json::Value = serde_json::from_str(&add_result.unwrap_or_else(|e| {
        eprintln!("JSON parse failed: {e}");
        std::process::exit(1)
    }))
    .unwrap_or_else(|e| {
        eprintln!("deserialization failed: {e}");
        std::process::exit(1)
    });
    let peer_device_id = added["device_id"]
        .as_str()
        .unwrap_or_else(|| {
            eprintln!("expected string value");
            std::process::exit(1)
        })
        .to_string();

    // Verify peer count
    let status = ghost_protocol_status(&state).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    let s: serde_json::Value = serde_json::from_str(&status).unwrap_or_else(|e| {
        eprintln!("JSON parse failed: {e}");
        std::process::exit(1)
    });
    assert_eq!(
        s["peer_count"].as_u64().unwrap_or_else(|| {
            eprintln!("expected u64 value");
            std::process::exit(1)
        }),
        1
    );

    // Remove peer
    let remove = ghost_protocol_remove_peer(&state, peer_device_id);
    assert!(remove.is_ok());
}

// ── Evolution wiring tests ──────────────────────────────────────────

#[test]
fn test_evolution_status() {
    let state = AppState::new();
    let result = evolution_get_status(&state).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap_or_else(|e| {
        eprintln!("JSON parse failed: {e}");
        std::process::exit(1)
    });
    assert!(parsed.get("enabled").is_some());
    assert!(parsed.get("total_strategies").is_some());
    assert!(parsed.get("active_agents").is_some());
}

#[test]
fn test_evolution_register_and_evolve() {
    let state = AppState::new();
    let agent_id = uuid::Uuid::new_v4().to_string();
    let params = json!({"learning_rate": 0.01, "batch_size": 32}).to_string();

    let reg = evolution_register_strategy(
        &state,
        agent_id.clone(),
        "test-strategy".to_string(),
        params,
    );
    assert!(reg.is_ok());
    let strategy: serde_json::Value = serde_json::from_str(&reg.unwrap_or_else(|e| {
        eprintln!("JSON parse failed: {e}");
        std::process::exit(1)
    }))
    .unwrap_or_else(|e| {
        eprintln!("deserialization failed: {e}");
        std::process::exit(1)
    });
    assert_eq!(
        strategy["name"].as_str().unwrap_or_else(|| {
            eprintln!("expected string value");
            std::process::exit(1)
        }),
        "test-strategy"
    );

    // Evolve
    let evolve = evolution_evolve_once(&state, agent_id.clone());
    assert!(evolve.is_ok());
    let evolved: serde_json::Value = serde_json::from_str(&evolve.unwrap_or_else(|e| {
        eprintln!("JSON parse failed: {e}");
        std::process::exit(1)
    }))
    .unwrap_or_else(|e| {
        eprintln!("deserialization failed: {e}");
        std::process::exit(1)
    });
    assert!(evolved.get("generation").is_some());

    // History
    let history = evolution_get_history(&state, agent_id.clone()).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    let h: serde_json::Value = serde_json::from_str(&history).unwrap_or_else(|e| {
        eprintln!("JSON parse failed: {e}");
        std::process::exit(1)
    });
    assert!(h.get("total_generations").is_some());

    // Active strategy
    let active = evolution_get_active_strategy(&state, agent_id.clone());
    assert!(active.is_ok());

    // Rollback — may fail if evolve_once didn't accept the child (no parent to rollback to).
    // We just verify it doesn't panic.
    let _ = evolution_rollback(&state, agent_id);
}

// ── MCP Host wiring tests ───────────────────────────────────────────

#[test]
fn test_mcp_host_add_list_remove_server() {
    let state = AppState::new();

    // Initially empty
    let list = mcp_host_list_servers(&state).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    let servers: Vec<serde_json::Value> = serde_json::from_str(&list).unwrap_or_else(|e| {
        eprintln!("JSON parse failed: {e}");
        std::process::exit(1)
    });
    assert!(servers.is_empty());

    // Add server
    let add = mcp_host_add_server(
        &state,
        "test-server".to_string(),
        "http://localhost:8080".to_string(),
        "http".to_string(),
        None,
    );
    assert!(add.is_ok());
    let added: serde_json::Value = serde_json::from_str(&add.unwrap_or_else(|e| {
        eprintln!("JSON parse failed: {e}");
        std::process::exit(1)
    }))
    .unwrap_or_else(|e| {
        eprintln!("deserialization failed: {e}");
        std::process::exit(1)
    });
    let server_id = added["id"]
        .as_str()
        .unwrap_or_else(|| {
            eprintln!("expected string value");
            std::process::exit(1)
        })
        .to_string();

    // List should have 1
    let list2 = mcp_host_list_servers(&state).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    let servers2: Vec<serde_json::Value> = serde_json::from_str(&list2).unwrap_or_else(|e| {
        eprintln!("JSON parse failed: {e}");
        std::process::exit(1)
    });
    assert_eq!(servers2.len(), 1);
    assert_eq!(
        servers2[0]["name"].as_str().unwrap_or_else(|| {
            eprintln!("expected string value");
            std::process::exit(1)
        }),
        "test-server"
    );

    // Tools should be empty (not connected)
    let tools = mcp_host_list_tools(&state).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    let tools_parsed: Vec<serde_json::Value> = serde_json::from_str(&tools).unwrap_or_else(|e| {
        eprintln!("JSON parse failed: {e}");
        std::process::exit(1)
    });
    assert!(tools_parsed.is_empty());

    // Remove
    let remove = mcp_host_remove_server(&state, server_id);
    assert!(remove.is_ok());

    // List should be empty again
    let list3 = mcp_host_list_servers(&state).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    let servers3: Vec<serde_json::Value> = serde_json::from_str(&list3).unwrap_or_else(|e| {
        eprintln!("JSON parse failed: {e}");
        std::process::exit(1)
    });
    assert!(servers3.is_empty());
}

// ── Neural Bridge wiring tests ──────────────────────────────────────

#[test]
fn test_neural_bridge_status() {
    let state = AppState::new();
    let result = neural_bridge_status(&state).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap_or_else(|e| {
        eprintln!("JSON parse failed: {e}");
        std::process::exit(1)
    });
    assert!(parsed.get("stats").is_some());
    assert!(parsed.get("config").is_some());
}

#[test]
fn test_neural_bridge_ingest_and_search() {
    let state = AppState::new();
    neural_bridge_toggle(&state, true).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });

    // Ingest content
    let ingest = neural_bridge_ingest(
        &state,
        "Clipboard".to_string(),
        "Nexus OS uses capability-based security for agent governance.".to_string(),
        json!({}),
    );
    assert!(ingest.is_ok());
    let entry: serde_json::Value = serde_json::from_str(&ingest.unwrap_or_else(|e| {
        eprintln!("JSON parse failed: {e}");
        std::process::exit(1)
    }))
    .unwrap_or_else(|e| {
        eprintln!("deserialization failed: {e}");
        std::process::exit(1)
    });
    let entry_id = entry["id"]
        .as_str()
        .unwrap_or_else(|| {
            eprintln!("expected string value");
            std::process::exit(1)
        })
        .to_string();
    assert!(!entry_id.is_empty());

    // Search
    let search = neural_bridge_search(
        &state,
        "capability security".to_string(),
        None,
        None,
        Some(5),
    );
    assert!(search.is_ok());
    let results: Vec<serde_json::Value> = serde_json::from_str(&search.unwrap_or_else(|e| {
        eprintln!("JSON parse failed: {e}");
        std::process::exit(1)
    }))
    .unwrap_or_else(|e| {
        eprintln!("deserialization failed: {e}");
        std::process::exit(1)
    });
    assert!(!results.is_empty());

    // Delete
    let del = neural_bridge_delete(&state, entry_id);
    assert!(del.is_ok());
    let d: serde_json::Value = serde_json::from_str(&del.unwrap_or_else(|e| {
        eprintln!("JSON parse failed: {e}");
        std::process::exit(1)
    }))
    .unwrap_or_else(|e| {
        eprintln!("deserialization failed: {e}");
        std::process::exit(1)
    });
    assert!(d["deleted"].as_bool().unwrap_or_else(|| {
        eprintln!("expected bool value");
        std::process::exit(1)
    }));
}

// ── Tracing wiring tests ────────────────────────────────────────────

#[test]
fn test_tracing_full_lifecycle() {
    let state = AppState::new();

    // Start trace
    let trace_result = tracing_start_trace(&state, "test-operation".to_string(), None);
    assert!(trace_result.is_ok());
    let t: serde_json::Value = serde_json::from_str(&trace_result.unwrap_or_else(|e| {
        eprintln!("JSON parse failed: {e}");
        std::process::exit(1)
    }))
    .unwrap_or_else(|e| {
        eprintln!("deserialization failed: {e}");
        std::process::exit(1)
    });
    let trace_id = t["trace_id"]
        .as_str()
        .unwrap_or_else(|| {
            eprintln!("expected string value");
            std::process::exit(1)
        })
        .to_string();
    let root_span_id = t["span_id"]
        .as_str()
        .unwrap_or_else(|| {
            eprintln!("expected string value");
            std::process::exit(1)
        })
        .to_string();

    // Start child span
    let span_result = tracing_start_span(
        &state,
        trace_id.clone(),
        root_span_id.clone(),
        "child-op".to_string(),
        None,
    );
    assert!(span_result.is_ok());
    let s: serde_json::Value = serde_json::from_str(&span_result.unwrap_or_else(|e| {
        eprintln!("JSON parse failed: {e}");
        std::process::exit(1)
    }))
    .unwrap_or_else(|e| {
        eprintln!("deserialization failed: {e}");
        std::process::exit(1)
    });
    let child_span_id = s["span_id"]
        .as_str()
        .unwrap_or_else(|| {
            eprintln!("expected string value");
            std::process::exit(1)
        })
        .to_string();

    // End child span
    let end_child = tracing_end_span(&state, child_span_id, "Ok".to_string(), None);
    assert!(end_child.is_ok());

    // End root span
    let end_root = tracing_end_span(&state, root_span_id, "Ok".to_string(), None);
    assert!(end_root.is_ok());

    // End trace
    let end_trace = tracing_end_trace(&state, trace_id.clone());
    assert!(end_trace.is_ok());
    let completed: serde_json::Value = serde_json::from_str(&end_trace.unwrap_or_else(|e| {
        eprintln!("JSON parse failed: {e}");
        std::process::exit(1)
    }))
    .unwrap_or_else(|e| {
        eprintln!("deserialization failed: {e}");
        std::process::exit(1)
    });
    assert!(completed.get("spans").is_some());

    // List traces
    let list = tracing_list_traces(&state, Some(10)).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    let traces: Vec<serde_json::Value> = serde_json::from_str(&list).unwrap_or_else(|e| {
        eprintln!("JSON parse failed: {e}");
        std::process::exit(1)
    });
    assert!(!traces.is_empty());

    // Get specific trace
    let get = tracing_get_trace(&state, trace_id);
    assert!(get.is_ok());
}

// ── Agent Memory wiring tests ───────────────────────────────────────

#[test]
fn test_agent_memory_remember_and_recall() {
    let state = AppState::new();
    let agent_id = uuid::Uuid::new_v4().to_string();

    // Remember
    let mem_result = agent_memory_remember(
        &state,
        agent_id.clone(),
        "The sky is blue.".to_string(),
        "Fact".to_string(),
        0.9,
        vec!["science".to_string()],
    );
    assert!(mem_result.is_ok());
    let entry: serde_json::Value = serde_json::from_str(&mem_result.unwrap_or_else(|e| {
        eprintln!("JSON parse failed: {e}");
        std::process::exit(1)
    }))
    .unwrap_or_else(|e| {
        eprintln!("deserialization failed: {e}");
        std::process::exit(1)
    });
    assert!(entry.get("id").is_some());

    // Recall
    let recall = agent_memory_recall(&state, agent_id.clone(), "sky".to_string(), Some(5));
    assert!(recall.is_ok());
    let results: Vec<serde_json::Value> = serde_json::from_str(&recall.unwrap_or_else(|e| {
        eprintln!("JSON parse failed: {e}");
        std::process::exit(1)
    }))
    .unwrap_or_else(|e| {
        eprintln!("deserialization failed: {e}");
        std::process::exit(1)
    });
    assert!(!results.is_empty());

    // Stats
    let stats = agent_memory_get_stats(&state, agent_id.clone()).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    let s: serde_json::Value = serde_json::from_str(&stats).unwrap_or_else(|e| {
        eprintln!("JSON parse failed: {e}");
        std::process::exit(1)
    });
    assert!(s.get("total").is_some());

    // Forget
    let memory_id = entry["id"]
        .as_str()
        .unwrap_or_else(|| {
            eprintln!("expected string value");
            std::process::exit(1)
        })
        .to_string();
    let forget = agent_memory_forget(&state, agent_id.clone(), memory_id);
    assert!(forget.is_ok());

    // Clear
    let clear = agent_memory_clear(&state, agent_id);
    assert!(clear.is_ok());
}

// ── Factory wiring tests ────────────────────────────────────────────

#[test]
fn test_factory_create_project_and_list() {
    let state = AppState::new();
    let tmp_dir = std::env::temp_dir().join("nexus_test_factory");
    let _ = std::fs::create_dir_all(&tmp_dir);

    let create = factory_create_project(
        &state,
        "test-project".to_string(),
        "rust".to_string(),
        tmp_dir.to_string_lossy().to_string(),
    );
    assert!(create.is_ok());
    let project: serde_json::Value = serde_json::from_str(&create.unwrap_or_else(|e| {
        eprintln!("JSON parse failed: {e}");
        std::process::exit(1)
    }))
    .unwrap_or_else(|e| {
        eprintln!("deserialization failed: {e}");
        std::process::exit(1)
    });
    assert_eq!(
        project["name"].as_str().unwrap_or_else(|| {
            eprintln!("expected string value");
            std::process::exit(1)
        }),
        "test-project"
    );
    assert!(project.get("id").is_some());

    // List
    let list = factory_list_projects(&state).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    let projects: Vec<serde_json::Value> = serde_json::from_str(&list).unwrap_or_else(|e| {
        eprintln!("JSON parse failed: {e}");
        std::process::exit(1)
    });
    assert_eq!(projects.len(), 1);

    // Build history (empty initially)
    let project_id = project["id"]
        .as_str()
        .unwrap_or_else(|| {
            eprintln!("expected string value");
            std::process::exit(1)
        })
        .to_string();
    let history = factory_get_build_history(&state, project_id).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    let h: Vec<serde_json::Value> = serde_json::from_str(&history).unwrap_or_else(|e| {
        eprintln!("JSON parse failed: {e}");
        std::process::exit(1)
    });
    assert!(h.is_empty());

    let _ = std::fs::remove_dir_all(&tmp_dir);
}

// ── Payments wiring tests ───────────────────────────────────────────

#[test]
fn test_payment_plan_and_invoice() {
    let state = AppState::new();

    // Create plan
    let plan = payment_create_plan(
        &state,
        "Pro Plan".to_string(),
        999,
        "Monthly".to_string(),
        vec![
            "unlimited-agents".to_string(),
            "priority-support".to_string(),
        ],
    );
    assert!(plan.is_ok());
    let plan_parsed: serde_json::Value = serde_json::from_str(&plan.unwrap_or_else(|e| {
        eprintln!("JSON parse failed: {e}");
        std::process::exit(1)
    }))
    .unwrap_or_else(|e| {
        eprintln!("deserialization failed: {e}");
        std::process::exit(1)
    });
    let plan_id = plan_parsed["id"]
        .as_str()
        .unwrap_or_else(|| {
            eprintln!("expected string value");
            std::process::exit(1)
        })
        .to_string();
    assert_eq!(
        plan_parsed["name"].as_str().unwrap_or_else(|| {
            eprintln!("expected string value");
            std::process::exit(1)
        }),
        "Pro Plan"
    );
    assert_eq!(
        plan_parsed["price_cents"].as_u64().unwrap_or_else(|| {
            eprintln!("expected u64 value");
            std::process::exit(1)
        }),
        999
    );

    // List plans
    let plans = payment_list_plans(&state).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    let plans_parsed: Vec<serde_json::Value> = serde_json::from_str(&plans).unwrap_or_else(|e| {
        eprintln!("JSON parse failed: {e}");
        std::process::exit(1)
    });
    assert_eq!(plans_parsed.len(), 1);

    // Create invoice
    let invoice = payment_create_invoice(&state, plan_id, "buyer-123".to_string());
    assert!(invoice.is_ok());
    let inv: serde_json::Value = serde_json::from_str(&invoice.unwrap_or_else(|e| {
        eprintln!("JSON parse failed: {e}");
        std::process::exit(1)
    }))
    .unwrap_or_else(|e| {
        eprintln!("deserialization failed: {e}");
        std::process::exit(1)
    });
    let invoice_id = inv["id"]
        .as_str()
        .unwrap_or_else(|| {
            eprintln!("expected string value");
            std::process::exit(1)
        })
        .to_string();
    assert_eq!(
        inv["status"].as_str().unwrap_or_else(|| {
            eprintln!("expected string value");
            std::process::exit(1)
        }),
        "Pending"
    );

    // Pay invoice
    let pay = payment_pay_invoice(&state, invoice_id);
    assert!(pay.is_ok());
    let paid: serde_json::Value = serde_json::from_str(&pay.unwrap_or_else(|e| {
        eprintln!("JSON parse failed: {e}");
        std::process::exit(1)
    }))
    .unwrap_or_else(|e| {
        eprintln!("deserialization failed: {e}");
        std::process::exit(1)
    });
    assert_eq!(
        paid["status"].as_str().unwrap_or_else(|| {
            eprintln!("expected string value");
            std::process::exit(1)
        }),
        "Paid"
    );

    // Revenue stats
    let stats = payment_get_revenue_stats(&state).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    let s: serde_json::Value = serde_json::from_str(&stats).unwrap_or_else(|e| {
        eprintln!("JSON parse failed: {e}");
        std::process::exit(1)
    });
    assert!(s.get("total_revenue_cents").is_some());
}

#[test]
fn test_tauri_replay_evidence_flow() {
    let state = AppState::new();

    // Toggle recording on
    let toggle = replay_toggle_recording(&state, true).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    let t: serde_json::Value = serde_json::from_str(&toggle).unwrap_or_else(|e| {
        eprintln!("JSON parse failed: {e}");
        std::process::exit(1)
    });
    assert_eq!(t["recording"], true);

    // Initially no bundles
    let list = replay_list_bundles(&state, None, Some(50)).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    let bundles: Vec<serde_json::Value> = serde_json::from_str(&list).unwrap_or_else(|e| {
        eprintln!("JSON parse failed: {e}");
        std::process::exit(1)
    });
    assert!(bundles.is_empty());

    // Record a bundle manually via the recorder
    {
        let mut recorder = state
            .replay_recorder
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let bid = recorder.capture_pre_state(
            "test-agent",
            "tool_call",
            vec!["fs.read".into()],
            1000,
            vec![],
            Some("mock".into()),
            json!({"cmd": "ls"}),
        );
        recorder.record_governance_check(&bid, "capability", true, "ok");
        recorder.record_governance_check(&bid, "fuel", true, "ok");
        recorder
            .capture_post_state(
                &bid,
                vec!["fs.read".into()],
                998,
                vec![],
                json!({"out": "ok"}),
            )
            .unwrap_or_else(|e| {
                eprintln!("operation failed: {e}");
                std::process::exit(1)
            });
    }

    // List bundles — should have 1
    let list2 = replay_list_bundles(&state, None, Some(50)).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    let bundles2: Vec<serde_json::Value> = serde_json::from_str(&list2).unwrap_or_else(|e| {
        eprintln!("JSON parse failed: {e}");
        std::process::exit(1)
    });
    assert_eq!(bundles2.len(), 1);
    let bundle_id = bundles2[0]["id"]
        .as_str()
        .unwrap_or_else(|| {
            eprintln!("expected string value");
            std::process::exit(1)
        })
        .to_string();

    // Get full bundle
    let full = replay_get_bundle(&state, bundle_id.clone()).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    let b: serde_json::Value = serde_json::from_str(&full).unwrap_or_else(|e| {
        eprintln!("JSON parse failed: {e}");
        std::process::exit(1)
    });
    assert_eq!(b["agent_id"], "test-agent");
    assert_eq!(b["action_type"], "tool_call");

    // Verify bundle
    let verdict = replay_verify_bundle(&state, bundle_id.clone()).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    assert!(verdict.contains("Verified"));

    // Export bundle
    let exported = replay_export_bundle(&state, bundle_id).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    assert!(exported.contains("test-agent"));
    assert!(exported.contains("bundle_hash"));

    // Filter by agent
    let filtered = replay_list_bundles(&state, Some("nonexistent".into()), Some(50))
        .unwrap_or_else(|e| {
            eprintln!("operation failed: {e}");
            std::process::exit(1)
        });
    let empty: Vec<serde_json::Value> = serde_json::from_str(&filtered).unwrap_or_else(|e| {
        eprintln!("JSON parse failed: {e}");
        std::process::exit(1)
    });
    assert!(empty.is_empty());

    // Toggle off
    let off = replay_toggle_recording(&state, false).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    let o: serde_json::Value = serde_json::from_str(&off).unwrap_or_else(|e| {
        eprintln!("JSON parse failed: {e}");
        std::process::exit(1)
    });
    assert_eq!(o["recording"], false);
}

// ── Consent / HITL Approval Tests ──

use super::{
    approve_consent_request, batch_approve_consents, batch_deny_consents, deny_consent_request,
    get_consent_history, list_pending_consents, review_consent_batch,
};
use nexus_persistence::StateStore;

fn enqueue_test_consent(state: &AppState, id: &str, agent_id: &str, op_type: &str, tier: &str) {
    let now = chrono::Utc::now().to_rfc3339();
    let op_json = serde_json::json!({
        "summary": format!("{op_type}: test-resource"),
        "side_effects": ["writes to disk", "sends network request"],
        "fuel_cost": 100.0
    })
    .to_string();
    state
        .db
        .enqueue_consent(&nexus_persistence::ConsentRow {
            id: id.to_string(),
            agent_id: agent_id.to_string(),
            operation_type: op_type.to_string(),
            operation_json: op_json,
            hitl_tier: tier.to_string(),
            status: "pending".to_string(),
            created_at: now,
            resolved_at: None,
            resolved_by: None,
        })
        .unwrap_or_else(|e| {
            eprintln!("operation failed: {e}");
            std::process::exit(1)
        });
}

fn enqueue_test_consent_json(
    state: &AppState,
    id: &str,
    agent_id: &str,
    op_type: &str,
    tier: &str,
    op_json: serde_json::Value,
) {
    let now = chrono::Utc::now().to_rfc3339();
    state
        .db
        .enqueue_consent(&nexus_persistence::ConsentRow {
            id: id.to_string(),
            agent_id: agent_id.to_string(),
            operation_type: op_type.to_string(),
            operation_json: op_json.to_string(),
            hitl_tier: tier.to_string(),
            status: "pending".to_string(),
            created_at: now,
            resolved_at: None,
            resolved_by: None,
        })
        .unwrap_or_else(|e| {
            eprintln!("operation failed: {e}");
            std::process::exit(1)
        });
}

#[test]
fn test_approve_consent_request() {
    let state = AppState::new_in_memory();
    enqueue_test_consent(&state, "c-approve-1", "a1", "fs.write", "Tier1");
    let result = approve_consent_request(&state, "c-approve-1".into(), "admin".into());
    assert!(result.is_ok());
    // Verify it's no longer pending
    let pending = list_pending_consents(&state).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    assert!(pending.iter().all(|p| p.consent_id != "c-approve-1"));
}

#[test]
fn test_approve_transcendent_creation_creates_agent() {
    let state = AppState::new_in_memory();
    let pending_agent_id = Uuid::new_v4().to_string();
    let manifest_json = build_transcendent_manifest("approved-transcendent");
    state
        .db
        .save_agent(
            &pending_agent_id,
            &manifest_json,
            "pending_approval",
            6,
            "native",
        )
        .unwrap_or_else(|e| {
            eprintln!("operation failed: {e}");
            std::process::exit(1)
        });
    state
        .db
        .enqueue_consent(&nexus_persistence::ConsentRow {
            id: "c-transcendent-create".to_string(),
            agent_id: pending_agent_id.clone(),
            operation_type: "transcendent_creation".to_string(),
            operation_json: json!({
                "summary": "Create L6 Transcendent agent 'approved-transcendent'",
                "side_effects": ["Maximum-autonomy L6 activation"],
                "fuel_cost": 0.0,
                "min_review_seconds": 60,
                "mode": "create_new",
                "manifest_json": manifest_json,
            })
            .to_string(),
            hitl_tier: "Tier3".to_string(),
            status: "pending".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            resolved_at: None,
            resolved_by: None,
        })
        .unwrap_or_else(|e| {
            eprintln!("operation failed: {e}");
            std::process::exit(1)
        });

    approve_consent_request(&state, "c-transcendent-create".into(), "admin".into()).unwrap_or_else(
        |e| {
            eprintln!("operation failed: {e}");
            std::process::exit(1)
        },
    );

    let agents = state.db.list_agents().unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    assert!(agents.iter().all(|row| row.id != pending_agent_id));
    assert!(agents
        .iter()
        .any(|row| row.manifest_json.contains("approved-transcendent")));
}

#[test]
fn test_approve_consent_request_wakes_blocked_wait() {
    let state = AppState::new_in_memory();
    enqueue_test_consent(&state, "c-approve-wake", "a1", "fs.write", "Tier1");
    let notify = state.register_blocked_consent_wait("a1", "c-approve-wake");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap_or_else(|e| {
            eprintln!("operation failed: {e}");
            std::process::exit(1)
        });

    runtime.block_on(async {
        let waiter = tokio::spawn({
            let notify = notify.clone();
            async move {
                notify.notified().await;
            }
        });

        approve_consent_request(&state, "c-approve-wake".into(), "admin".into()).unwrap_or_else(
            |e| {
                eprintln!("operation failed: {e}");
                std::process::exit(1)
            },
        );

        tokio::time::timeout(std::time::Duration::from_millis(100), waiter)
            .await
            .unwrap_or_else(|e| {
                eprintln!("operation failed: {e}");
                std::process::exit(1)
            })
            .unwrap_or_else(|e| {
                eprintln!("operation failed: {e}");
                std::process::exit(1)
            });
    });
}

#[test]
fn test_deny_consent_request() {
    let state = AppState::new_in_memory();
    enqueue_test_consent(&state, "c-deny-1", "a2", "process.exec", "Tier2");
    let result = deny_consent_request(
        &state,
        "c-deny-1".into(),
        "admin".into(),
        Some("too risky".into()),
    );
    assert!(result.is_ok());
    let pending = list_pending_consents(&state).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    assert!(pending.iter().all(|p| p.consent_id != "c-deny-1"));
}

#[test]
fn test_deny_transcendent_creation_cleans_up_pending_agent() {
    let state = AppState::new_in_memory();
    let pending_agent_id = Uuid::new_v4().to_string();
    let manifest_json = build_transcendent_manifest("denied-transcendent");
    state
        .db
        .save_agent(
            &pending_agent_id,
            &manifest_json,
            "pending_approval",
            6,
            "native",
        )
        .unwrap_or_else(|e| {
            eprintln!("operation failed: {e}");
            std::process::exit(1)
        });
    state
        .db
        .enqueue_consent(&nexus_persistence::ConsentRow {
            id: "c-transcendent-deny".to_string(),
            agent_id: pending_agent_id.clone(),
            operation_type: "transcendent_creation".to_string(),
            operation_json: json!({
                "summary": "Create L6 Transcendent agent 'denied-transcendent'",
                "side_effects": ["Maximum-autonomy L6 activation"],
                "fuel_cost": 0.0,
                "min_review_seconds": 60,
                "mode": "create_new",
                "manifest_json": manifest_json,
            })
            .to_string(),
            hitl_tier: "Tier3".to_string(),
            status: "pending".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            resolved_at: None,
            resolved_by: None,
        })
        .unwrap_or_else(|e| {
            eprintln!("operation failed: {e}");
            std::process::exit(1)
        });

    deny_consent_request(
        &state,
        "c-transcendent-deny".into(),
        "admin".into(),
        Some("not today".into()),
    )
    .unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });

    let agents = state.db.list_agents().unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    assert!(agents.iter().all(|row| row.id != pending_agent_id));
}

#[test]
fn test_deny_consent_request_wakes_blocked_wait() {
    let state = AppState::new_in_memory();
    enqueue_test_consent(&state, "c-deny-wake", "a2", "process.exec", "Tier2");
    let notify = state.register_blocked_consent_wait("a2", "c-deny-wake");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap_or_else(|e| {
            eprintln!("operation failed: {e}");
            std::process::exit(1)
        });

    runtime.block_on(async {
        let waiter = tokio::spawn({
            let notify = notify.clone();
            async move {
                notify.notified().await;
            }
        });

        deny_consent_request(
            &state,
            "c-deny-wake".into(),
            "admin".into(),
            Some("too risky".into()),
        )
        .unwrap_or_else(|e| {
            eprintln!("operation failed: {e}");
            std::process::exit(1)
        });

        tokio::time::timeout(std::time::Duration::from_millis(100), waiter)
            .await
            .unwrap_or_else(|e| {
                eprintln!("operation failed: {e}");
                std::process::exit(1)
            })
            .unwrap_or_else(|e| {
                eprintln!("operation failed: {e}");
                std::process::exit(1)
            });
    });
}

#[test]
fn test_list_pending_consents_returns_only_pending() {
    let state = AppState::new_in_memory();
    enqueue_test_consent(&state, "c-lp-1", "a1", "fs.read", "Tier0");
    enqueue_test_consent(&state, "c-lp-2", "a1", "fs.write", "Tier1");
    enqueue_test_consent(&state, "c-lp-3", "a2", "web.search", "Tier0");

    // Resolve one
    approve_consent_request(&state, "c-lp-1".into(), "admin".into()).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });

    let pending = list_pending_consents(&state).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    assert_eq!(pending.len(), 2);
    assert!(pending.iter().any(|p| p.consent_id == "c-lp-2"));
    assert!(pending.iter().any(|p| p.consent_id == "c-lp-3"));
}

#[test]
fn test_get_consent_history_returns_all() {
    let state = AppState::new_in_memory();
    enqueue_test_consent(&state, "c-hist-1", "a1", "fs.read", "Tier0");
    enqueue_test_consent(&state, "c-hist-2", "a1", "fs.write", "Tier1");
    enqueue_test_consent(&state, "c-hist-3", "a2", "web.search", "Tier0");
    enqueue_test_consent(&state, "c-hist-4", "a2", "process.exec", "Tier2");
    enqueue_test_consent(&state, "c-hist-5", "a3", "llm.query", "Tier0");

    // Resolve 3 of them
    approve_consent_request(&state, "c-hist-1".into(), "admin".into()).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    deny_consent_request(&state, "c-hist-2".into(), "admin".into(), None).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    approve_consent_request(&state, "c-hist-3".into(), "user".into()).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });

    let history = get_consent_history(&state, 20).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    assert_eq!(history.len(), 5);
}

#[test]
fn test_auto_timeout_risk_level_mapping() {
    let state = AppState::new_in_memory();
    enqueue_test_consent(&state, "c-risk-1", "a1", "fs.read", "Tier0");
    enqueue_test_consent(&state, "c-risk-2", "a1", "fs.write", "Tier1");
    enqueue_test_consent(&state, "c-risk-3", "a1", "process.exec", "Tier2");
    enqueue_test_consent(&state, "c-risk-4", "a1", "self_mutation", "Tier3");

    let pending = list_pending_consents(&state).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    let risk_levels: Vec<&str> = pending.iter().map(|p| p.risk_level.as_str()).collect();
    assert!(risk_levels.contains(&"Low"));
    assert!(risk_levels.contains(&"Medium"));
    assert!(risk_levels.contains(&"High"));
    assert!(risk_levels.contains(&"Critical"));
}

#[test]
fn test_approve_nonexistent_consent_fails() {
    let state = AppState::new_in_memory();
    let result = approve_consent_request(&state, "nonexistent-id".into(), "admin".into());
    assert!(result.is_err());
}

#[test]
fn test_deny_nonexistent_consent_fails() {
    let state = AppState::new_in_memory();
    let result = deny_consent_request(&state, "nonexistent-id".into(), "admin".into(), None);
    assert!(result.is_err());
}

#[test]
fn test_consent_notification_fields() {
    let state = AppState::new_in_memory();
    enqueue_test_consent(&state, "c-fields-1", "a1", "fs.write", "Tier2");
    let pending = list_pending_consents(&state).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    assert_eq!(pending.len(), 1);
    let notif = &pending[0];
    assert_eq!(notif.consent_id, "c-fields-1");
    assert_eq!(notif.agent_id, "a1");
    assert_eq!(notif.operation_type, "fs.write");
    assert_eq!(notif.risk_level, "High");
    assert_eq!(notif.fuel_cost_estimate, 100.0);
    assert_eq!(notif.side_effects_preview.len(), 2);
    assert!(!notif.auto_deny_at.is_empty());
    assert!(!notif.requested_at.is_empty());
}

#[test]
fn test_l6_consent_notification_has_review_delay() {
    let state = AppState::new_in_memory();
    state
        .db
        .enqueue_consent(&nexus_persistence::ConsentRow {
            id: "c-l6-1".to_string(),
            agent_id: "a1".to_string(),
            operation_type: "transcendent_creation".to_string(),
            operation_json: json!({
                "summary": "Create L6 agent",
                "min_review_seconds": 60
            })
            .to_string(),
            hitl_tier: "Tier3".to_string(),
            status: "pending".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            resolved_at: None,
            resolved_by: None,
        })
        .unwrap_or_else(|e| {
            eprintln!("operation failed: {e}");
            std::process::exit(1)
        });

    let pending = list_pending_consents(&state).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    assert_eq!(pending[0].min_review_seconds, Some(60));
}

#[test]
fn test_batch_consent_notification_fields() {
    let state = AppState::new_in_memory();
    enqueue_test_consent_json(
        &state,
        "c-batch-1",
        "a1",
        "cognitive.hitl_batch",
        "Tier1",
        json!({
            "summary": "Execute 3 governed actions",
            "goal_id": "goal-1",
            "batch_action_count": 3,
            "batch_actions": [
                "ShellCommand: ls -la",
                "FileWrite: analysis.md",
                "ShellCommand: grep TODO src/*.rs"
            ],
            "review_each_available": true,
            "side_effects": ["ShellCommand: ls -la"],
            "fuel_cost": 15.0
        }),
    );

    let pending = list_pending_consents(&state).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    assert_eq!(pending.len(), 1);
    let notif = &pending[0];
    assert_eq!(notif.goal_id.as_deref(), Some("goal-1"));
    assert_eq!(notif.batch_action_count, Some(3));
    assert_eq!(notif.batch_actions.len(), 3);
    assert!(notif.review_each_available);
}

#[test]
fn test_batch_approve_consents_resolves_goal_rows() {
    let state = AppState::new_in_memory();
    enqueue_test_consent_json(
        &state,
        "c-batch-approve-1",
        "a1",
        "cognitive.hitl_batch",
        "Tier1",
        json!({"summary": "batch", "goal_id": "goal-batch"}),
    );
    enqueue_test_consent_json(
        &state,
        "c-batch-approve-2",
        "a1",
        "cognitive.hitl_approval",
        "Tier1",
        json!({"summary": "single", "goal_id": "goal-batch"}),
    );
    enqueue_test_consent_json(
        &state,
        "c-batch-approve-3",
        "a1",
        "cognitive.hitl_approval",
        "Tier1",
        json!({"summary": "other", "goal_id": "goal-other"}),
    );

    let (resolved, meta) = batch_approve_consents(&state, "goal-batch".into(), "user".into())
        .unwrap_or_else(|e| {
            eprintln!("operation failed: {e}");
            std::process::exit(1)
        });
    assert_eq!(resolved.len(), 2);
    assert_eq!(meta.agent_id, "a1");
    assert_eq!(meta.source_surface, "unknown");

    let pending = list_pending_consents(&state).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].goal_id.as_deref(), Some("goal-other"));
}

#[test]
fn test_review_consent_batch_resolves_pending_request() {
    let state = AppState::new_in_memory();
    enqueue_test_consent_json(
        &state,
        "c-review-batch-1",
        "a1",
        "cognitive.hitl_batch",
        "Tier1",
        json!({"summary": "batch", "goal_id": "goal-review", "review_each_available": true}),
    );

    let meta = review_consent_batch(&state, "c-review-batch-1".into(), "user".into())
        .unwrap_or_else(|e| {
            eprintln!("operation failed: {e}");
            std::process::exit(1)
        });
    assert_eq!(meta.agent_id, "a1");

    let pending = list_pending_consents(&state).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    assert!(pending.is_empty());

    let history = get_consent_history(&state, 10).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    assert!(history
        .iter()
        .any(|item| { item.consent_id == "c-review-batch-1" && item.status == "review_each" }));
}

#[test]
fn test_batch_deny_consents_resolves_goal_rows() {
    let state = AppState::new_in_memory();
    enqueue_test_consent_json(
        &state,
        "c-batch-deny-1",
        "a1",
        "cognitive.hitl_batch",
        "Tier1",
        json!({"summary": "batch", "goal_id": "goal-deny"}),
    );
    enqueue_test_consent_json(
        &state,
        "c-batch-deny-2",
        "a1",
        "cognitive.hitl_approval",
        "Tier1",
        json!({"summary": "single", "goal_id": "goal-deny"}),
    );

    let (resolved, meta) = batch_deny_consents(
        &state,
        "goal-deny".into(),
        "user".into(),
        Some("deny all".into()),
    )
    .unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    assert_eq!(resolved.len(), 2);
    assert_eq!(meta.agent_id, "a1");
    assert_eq!(meta.source_surface, "unknown");
    assert!(list_pending_consents(&state)
        .unwrap_or_else(|e| {
            eprintln!("operation failed: {e}");
            std::process::exit(1)
        })
        .is_empty());
}

#[test]
fn test_consent_audit_events_on_approve() {
    let state = AppState::new_in_memory();
    enqueue_test_consent(&state, "c-audit-a", "a1", "fs.write", "Tier1");
    approve_consent_request(&state, "c-audit-a".into(), "admin".into()).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    // Verify audit event was logged
    let events = state
        .db
        .load_audit_events(None, 100, 0)
        .unwrap_or_else(|e| {
            eprintln!("operation failed: {e}");
            std::process::exit(1)
        });
    let consent_events: Vec<_> = events
        .iter()
        .filter(|e| e.detail_json.contains("consent_approved"))
        .collect();
    assert!(!consent_events.is_empty());
}

#[test]
fn test_consent_audit_events_on_deny() {
    let state = AppState::new_in_memory();
    enqueue_test_consent(&state, "c-audit-d", "a1", "process.exec", "Tier2");
    deny_consent_request(
        &state,
        "c-audit-d".into(),
        "admin".into(),
        Some("unauthorized".into()),
    )
    .unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    let events = state
        .db
        .load_audit_events(None, 100, 0)
        .unwrap_or_else(|e| {
            eprintln!("operation failed: {e}");
            std::process::exit(1)
        });
    let consent_events: Vec<_> = events
        .iter()
        .filter(|e| e.detail_json.contains("consent_denied"))
        .collect();
    assert!(!consent_events.is_empty());
}

#[test]
fn test_approve_already_resolved_fails() {
    let state = AppState::new_in_memory();
    enqueue_test_consent(&state, "c-double", "a1", "fs.write", "Tier1");
    approve_consent_request(&state, "c-double".into(), "admin".into()).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    // Second approve should fail (no longer pending)
    let result = approve_consent_request(&state, "c-double".into(), "admin".into());
    assert!(result.is_err());
}

#[test]
fn test_consent_history_limit() {
    let state = AppState::new_in_memory();
    for i in 0..10 {
        enqueue_test_consent(&state, &format!("c-limit-{i}"), "a1", "fs.read", "Tier0");
    }
    let history = get_consent_history(&state, 5).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    assert_eq!(history.len(), 5);
}

#[test]
fn test_deny_with_no_reason() {
    let state = AppState::new_in_memory();
    enqueue_test_consent(&state, "c-no-reason", "a1", "fs.write", "Tier1");
    let result = deny_consent_request(&state, "c-no-reason".into(), "admin".into(), None);
    assert!(result.is_ok());
}

#[test]
fn test_empty_pending_list() {
    let state = AppState::new_in_memory();
    let pending = list_pending_consents(&state).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    assert!(pending.is_empty());
}

#[test]
fn test_consent_resolved_removes_from_pending() {
    let state = AppState::new_in_memory();
    enqueue_test_consent(&state, "c-resolve-1", "a1", "fs.read", "Tier0");
    enqueue_test_consent(&state, "c-resolve-2", "a1", "fs.write", "Tier1");
    assert_eq!(
        list_pending_consents(&state)
            .unwrap_or_else(|e| {
                eprintln!("operation failed: {e}");
                std::process::exit(1)
            })
            .len(),
        2
    );

    approve_consent_request(&state, "c-resolve-1".into(), "user".into()).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    assert_eq!(
        list_pending_consents(&state)
            .unwrap_or_else(|e| {
                eprintln!("operation failed: {e}");
                std::process::exit(1)
            })
            .len(),
        1
    );

    deny_consent_request(&state, "c-resolve-2".into(), "user".into(), None).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    assert_eq!(
        list_pending_consents(&state)
            .unwrap_or_else(|e| {
                eprintln!("operation failed: {e}");
                std::process::exit(1)
            })
            .len(),
        0
    );
}

// ── Messaging Gateway Tests ──

#[test]
fn test_messaging_status_empty_by_default() {
    let state = AppState::new_in_memory();
    let status = get_messaging_status(&state).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    assert!(status.is_empty());
}

#[test]
fn test_set_default_messaging_agent() {
    let state = AppState::new_in_memory();
    let result = set_default_agent(&state, "user-1".into(), "agent-abc".into());
    assert!(result.is_ok());
}
