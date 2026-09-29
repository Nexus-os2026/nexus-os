use super::{
    agent_memory_clear, agent_memory_forget, agent_memory_get_stats, agent_memory_recall,
    agent_memory_remember, chat_with_documents, check_model_compatibility, complete_build,
    complete_research, create_agent, create_agent_immediately, create_simulation,
    economy_create_wallet, economy_earn, economy_freeze_wallet, economy_get_history,
    economy_get_stats, economy_get_wallet, economy_spend, economy_transfer, evolution_evolve_once,
    evolution_get_active_strategy, evolution_get_history, evolution_get_status,
    evolution_register_strategy, evolution_rollback, factory_get_build_history,
    factory_list_projects, get_active_llm_provider, get_agent_activity, get_browser_history,
    get_configured_provider, get_input_control_status, get_knowledge_base, get_live_system_metrics,
    get_messaging_status, get_simulation_report, get_simulation_status, get_system_specs,
    ghost_protocol_add_peer, ghost_protocol_remove_peer, ghost_protocol_status,
    ghost_protocol_toggle, inject_simulation_variable, learning_agent_action, list_agents,
    list_indexed_documents, list_local_models, list_prebuilt_manifest_paths, list_simulations,
    mcp_host_add_server, mcp_host_list_servers, mcp_host_list_tools, mcp_host_remove_server,
    navigate_to, neural_bridge_delete, neural_bridge_ingest, neural_bridge_search,
    neural_bridge_status, neural_bridge_toggle, parse_agent_manifest_json, pause_agent,
    payment_create_invoice, payment_create_plan, payment_get_revenue_stats, payment_list_plans,
    payment_pay_invoice, remove_indexed_document, replay_export_bundle, replay_get_bundle,
    replay_list_bundles, replay_toggle_recording, replay_verify_bundle, resume_agent,
    run_parallel_simulation_reports, search_documents, set_default_agent, start_agent, start_build,
    start_learning, start_research, start_simulation_with_observer, stop_agent,
    stop_computer_action, time_machine_create_checkpoint, time_machine_list_checkpoints,
    time_machine_redo, time_machine_undo, tracing_end_span, tracing_end_trace, tracing_get_trace,
    tracing_list_traces, tracing_start_span, tracing_start_trace, voice_get_status,
    voice_transcribe, AppState, LearningSource,
};
use nexus_kernel::simulation::SimulationObserver;
use serde_json::json;
use std::{sync::Arc, thread, time::Duration};
use uuid::Uuid;

/// P0-002C5A: `index_document` took a raw file path as authority and is
/// closed. The RAG wiring tests ingest their fixture text through the
/// pipeline directly, with the same embedding-dimension probe it used.
fn ingest_test_document(state: &AppState, label: &str, content: &str) -> Result<String, String> {
    use nexus_connectors_llm::providers::LlmProvider;
    let provider = get_configured_provider();
    let mut rag = state.rag.lock().unwrap_or_else(|p| p.into_inner());
    if rag.documents.is_empty() {
        if let Ok(probe) = provider.embed(&["dimension probe"], &rag.config.embedding_model) {
            if let Some(first) = probe.embeddings.first() {
                if first.len() != rag.config.embedding_dimension {
                    rag.config.embedding_dimension = first.len();
                    rag.vector_store =
                        nexus_connectors_llm::vector_store::VectorStore::new(first.len());
                }
            }
        }
    }
    let mut redaction = nexus_kernel::redaction::RedactionEngine::default();
    let doc = rag
        .ingest_document(
            content,
            label,
            nexus_connectors_llm::chunking::SupportedFormat::PlainText,
            &provider,
            &mut redaction,
        )
        .map_err(|e| format!("ingest failed: {e}"))?;
    serde_json::to_string(&doc).map_err(|e| format!("serialize error: {e}"))
}

#[test]
fn p0_002c1_appstate_owns_empty_shared_authority_registry() {
    use nexus_kernel::manifest::FsPermissionLevel;
    use nexus_kernel::workspace_authority::{
        WorkspaceAuthorityError, WorkspaceAuthoritySource, WorkspaceBinding, WorkspaceGrantId,
    };

    let state = AppState::new_in_memory();
    let cloned = state.clone();
    assert!(Arc::ptr_eq(
        &state.workspace_authority,
        &cloned.workspace_authority
    ));
    let owner = WorkspaceBinding {
        agent_id: Uuid::new_v4(),
        run_id: Uuid::new_v4(),
    };
    let unknown: WorkspaceGrantId = serde_json::from_value(json!(Uuid::new_v4())).unwrap();
    assert_eq!(
        state
            .workspace_authority
            .resolve(unknown, owner)
            .unwrap_err(),
        WorkspaceAuthorityError::UnknownGrant
    );
    let id = state
        .workspace_authority
        .issue_trusted_root(
            &std::env::temp_dir(),
            owner,
            WorkspaceAuthoritySource::BackendAllocated,
            FsPermissionLevel::ReadOnly,
            None,
        )
        .unwrap();
    assert!(cloned.workspace_authority.resolve(id, owner).is_ok());
    cloned.workspace_authority.revoke(id, owner).unwrap();
    assert_eq!(
        state.workspace_authority.resolve(id, owner).unwrap_err(),
        WorkspaceAuthorityError::RevokedGrant
    );
    state.shutdown_oracle_runtime();
}

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
    let state = AppState::new_in_memory();
    let created = create_agent(&state, build_manifest("my-social-poster"));
    assert!(created.is_ok());

    if let Ok(agent_id) = created {
        let parsed = uuid::Uuid::parse_str(agent_id.as_str());
        assert!(parsed.is_ok());
    }
}

#[test]
fn test_tauri_create_agent_rejects_manifest_names_outside_kernel_schema() {
    let state = AppState::new_in_memory();
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

/// P0-FINAL-GATE (item G): an L6 (transcendent) agent needs a human approval
/// the backend cannot verify. Creating one used to enqueue a review that any
/// caller could approve over IPC at once (the "60-second review" was only a
/// frontend countdown); it is now refused before any agent record, meta
/// entry, supervisor entry or consent request is written. Creation below L6,
/// which never had an approval step, is unchanged.
#[test]
fn p0_fg_transcendent_creation_is_refused_and_changes_nothing() {
    use crate::phase0_surface::{closed, Closure};
    let state = AppState::new_in_memory();
    let stored = state.db.list_agents().unwrap().len();
    let registered = state.supervisor.lock().unwrap().health_check().len();
    let named = state.meta.lock().unwrap().len();

    assert_eq!(
        create_agent(&state, build_transcendent_manifest("transcendent-pending")),
        Err(closed("create_agent", Closure::ApprovalRequired))
    );
    assert!(state.db.load_pending_consent().unwrap().is_empty());
    assert_eq!(state.db.list_agents().unwrap().len(), stored);
    assert_eq!(
        state.supervisor.lock().unwrap().health_check().len(),
        registered
    );
    assert_eq!(state.meta.lock().unwrap().len(), named);

    let created = create_agent(&state, build_manifest("below-transcendent")).unwrap();
    assert!(Uuid::parse_str(&created).is_ok());
    assert!(state.db.load_pending_consent().unwrap().is_empty());
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

/// The prebuilt manifests at autonomy level 6, by agent name.
const TRANSCENDENT_PREBUILT: [&str; 12] = [
    "nexus-arbiter",
    "nexus-architect-prime",
    "nexus-ascendant",
    "nexus-continuum",
    "nexus-genesis-prime",
    "nexus-legion",
    "nexus-mirror",
    "nexus-oracle-omega",
    "nexus-oracle-supreme",
    "nexus-prime",
    "nexus-warden",
    "nexus-weaver",
];

/// (name, autonomy level, manifest JSON) of every prebuilt manifest.
fn prebuilt_manifests() -> Vec<(String, Option<u8>, String)> {
    list_prebuilt_manifest_paths()
        .iter()
        .map(|path| {
            let json = std::fs::read_to_string(path).unwrap();
            let manifest = parse_agent_manifest_json(&json).unwrap();
            (manifest.name, manifest.autonomy_level, json)
        })
        .collect()
}

/// How many prebuilt manifests are below L6, after checking that the L6 ones
/// are exactly `TRANSCENDENT_PREBUILT`.
fn prebuilt_count_below_l6() -> usize {
    let manifests = prebuilt_manifests();
    let mut transcendent: Vec<&str> = manifests
        .iter()
        .filter(|(_, level, _)| *level == Some(6))
        .map(|(name, _, _)| name.as_str())
        .collect();
    transcendent.sort_unstable();
    assert_eq!(transcendent, TRANSCENDENT_PREBUILT);
    manifests.len() - TRANSCENDENT_PREBUILT.len()
}

/// (name, autonomy level) of every agent registered with the supervisor,
/// sorted.
fn registered_agents(state: &AppState) -> Vec<(String, u8)> {
    let supervisor = state.supervisor.lock().unwrap();
    let mut agents: Vec<(String, u8)> = supervisor
        .health_check()
        .into_iter()
        .filter_map(|status| supervisor.get_agent(status.id))
        .map(|handle| (handle.manifest.name.clone(), handle.autonomy_level))
        .collect();
    agents.sort();
    agents
}

/// Every prebuilt manifest below L6 is stored and registered. None of the
/// twelve L6 manifests is (P0-FINAL-GATE item G). This test used to be
/// `test_load_prebuilt_agents_registers_every_manifest`. It asserted that
/// every manifest was stored, the L6 ones included, and so pinned the
/// loader's registration of L6 agents.
#[test]
fn test_load_prebuilt_agents_loads_every_manifest_below_l6() {
    let state = AppState::new_in_memory();
    state.load_prebuilt_agents();
    let below = prebuilt_count_below_l6();
    let rows = state.db.list_agents().unwrap();
    assert_eq!(rows.len(), below);
    assert!(rows.iter().all(|row| row.autonomy_level < 6));
    let registered = registered_agents(&state);
    assert_eq!(registered.len(), below);
    assert!(registered.iter().all(|(_, level)| *level < 6));
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
    // The L6 manifests are never loaded (P0-FINAL-GATE item G).
    assert_eq!(agents.len(), prebuilt_count_below_l6());
}

#[test]
fn test_list_agents_includes_stopped_prebuilt_agents_from_persistence() {
    let state = AppState::new_in_memory();
    state.load_prebuilt_agents();

    let agents = list_agents(&state).unwrap_or_else(|e| {
        eprintln!("list_agents should succeed: {e}");
        std::process::exit(1)
    });
    // The L6 manifests are never loaded (P0-FINAL-GATE item G).
    let manifest_count = prebuilt_count_below_l6();

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
    // The L6 manifests are never loaded (P0-FINAL-GATE item G).
    let manifest_count = prebuilt_count_below_l6();

    assert_eq!(agents.len(), manifest_count);
    assert!(agents.iter().all(|agent| !agent.agent_id.trim().is_empty()));
    assert!(agents.iter().any(|agent| agent.name == "nexus-oracle"));
}

/// P0-FINAL-GATE (item G): the real startup order, restore then the prebuilt
/// load, registers no L6 (transcendent) agent on any run:
/// - a first run over an empty store;
/// - a restart over that store, after an earlier build also stored the
///   twelve L6 prebuilt records there (some running, some stopped);
/// - a further restart.
///
/// The L6 records stay exactly as stored, and every run registers the same
/// agents.
#[test]
fn p0_fg_startup_registers_no_transcendent_agent_on_any_run() {
    let below = prebuilt_count_below_l6();
    let copy_store = |rows: Vec<nexus_persistence::AgentRow>| {
        let state = AppState::new_in_memory();
        for row in rows {
            state
                .db
                .save_agent(
                    &row.id,
                    &row.manifest_json,
                    &row.state,
                    row.autonomy_level,
                    &row.execution_mode,
                )
                .unwrap();
        }
        state
    };

    let first = AppState::new_in_memory();
    first.load_agents_deferred();
    let registered = registered_agents(&first);
    assert_eq!(registered.len(), below);
    assert!(registered.iter().all(|(_, level)| *level < 6));
    let stored = first.db.list_agents().unwrap();
    assert_eq!(stored.len(), below);
    assert!(stored.iter().all(|row| row.autonomy_level < 6));

    let second = copy_store(stored);
    let mut transcendent_ids = Vec::new();
    for (index, (_, _, json)) in prebuilt_manifests()
        .into_iter()
        .filter(|(_, level, _)| *level == Some(6))
        .enumerate()
    {
        let id = Uuid::new_v4().to_string();
        let stored_state = if index % 2 == 0 { "running" } else { "stopped" };
        second
            .db
            .save_agent(&id, &json, stored_state, 6, "native")
            .unwrap();
        transcendent_ids.push(id);
    }
    let transcendent_rows = |state: &AppState| {
        let mut rows: Vec<_> = state
            .db
            .list_agents()
            .unwrap()
            .into_iter()
            .filter(|row| transcendent_ids.contains(&row.id))
            .map(|row| {
                (
                    row.id,
                    row.manifest_json,
                    row.state,
                    row.was_running,
                    row.autonomy_level,
                    row.execution_mode,
                    row.updated_at,
                )
            })
            .collect();
        rows.sort();
        rows
    };
    let before = transcendent_rows(&second);
    assert_eq!(before.len(), TRANSCENDENT_PREBUILT.len());
    assert!(before.iter().any(|row| row.3), "a running L6 record");
    second.load_agents_deferred();
    assert_eq!(registered_agents(&second), registered);
    assert_eq!(transcendent_rows(&second), before);

    let third = copy_store(second.db.list_agents().unwrap());
    let before = transcendent_rows(&third);
    third.load_agents_deferred();
    assert_eq!(registered_agents(&third), registered);
    assert_eq!(transcendent_rows(&third), before);
}

/// P0-FINAL-GATE (item G): starting an L6 (transcendent) agent used to
/// enqueue an activation review that any caller could approve over IPC. It
/// is now refused before the agent's state changes, and no review is
/// enqueued. The refusal holds whether the stored record or the registered
/// agent says L6. (No current route registers an L6 agent: creation refuses
/// it, restore skips it and the prebuilt load skips L6 manifests. See
/// `p0_fg_startup_registers_no_transcendent_agent_on_any_run`. The direct
/// registration below stands in for an L6 agent that some other route
/// might register.)
#[test]
fn p0_fg_transcendent_activation_is_refused_and_changes_nothing() {
    use crate::phase0_surface::{closed, Closure};
    let state = AppState::new_in_memory();
    let created = create_agent_immediately(
        &state,
        parse_agent_manifest_json(&build_transcendent_manifest("transcendent-start")).unwrap(),
        build_transcendent_manifest("transcendent-start"),
    )
    .unwrap();
    let id = Uuid::parse_str(&created).unwrap();
    let registered_state = |state: &AppState| {
        state
            .supervisor
            .lock()
            .unwrap()
            .health_check()
            .into_iter()
            .find(|status| status.id == id)
            .map(|status| status.state)
    };
    stop_agent(&state, created.clone()).unwrap();
    let stopped = registered_state(&state);
    assert_eq!(stopped, Some(nexus_kernel::lifecycle::AgentState::Stopped));

    let refused = Err(closed("start_agent", Closure::ApprovalRequired));
    assert_eq!(start_agent(&state, created.clone()), refused);
    assert!(state.db.load_pending_consent().unwrap().is_empty());
    assert_eq!(registered_state(&state), stopped);
    let listed = list_agents(&state).unwrap();
    let agent = listed.iter().find(|row| row.id == created).unwrap();
    assert_eq!(agent.status, "Stopped");

    // Without the stored record, the registered L6 agent is refused as well.
    state.db.delete_agent(&created).unwrap();
    assert_eq!(start_agent(&state, created.clone()), refused);
    assert_eq!(registered_state(&state), stopped);
    assert!(state.db.load_pending_consent().unwrap().is_empty());

    // Starting an agent below L6 is unchanged.
    let below = create_agent(&state, build_manifest("below-transcendent")).unwrap();
    stop_agent(&state, below.clone()).unwrap();
    assert_eq!(start_agent(&state, below), Ok(()));
}

/// P0-FINAL-GATE (item G, review W3): resuming a paused L6 (transcendent)
/// agent is refused before its state changes, as starting one is. No state,
/// stored state or audit event changes. (No current route registers an L6
/// agent; the direct registration stands in for one.) Resuming a paused
/// agent below L6 is unchanged.
#[test]
fn p0_fg_transcendent_resume_is_refused_and_changes_nothing() {
    use crate::phase0_surface::{closed, Closure};
    use nexus_kernel::lifecycle::AgentState;
    let state = AppState::new_in_memory();
    let transcendent = state
        .supervisor
        .lock()
        .unwrap()
        .start_agent(
            parse_agent_manifest_json(&build_transcendent_manifest("transcendent-resume")).unwrap(),
        )
        .unwrap();
    state
        .supervisor
        .lock()
        .unwrap()
        .pause_agent(transcendent)
        .unwrap();
    let agent_state = |state: &AppState, id: Uuid| {
        state
            .supervisor
            .lock()
            .unwrap()
            .get_agent(id)
            .map(|handle| handle.state)
    };
    assert_eq!(agent_state(&state, transcendent), Some(AgentState::Paused));
    let events = state.audit.lock().unwrap().events().len();

    assert_eq!(
        resume_agent(&state, transcendent.to_string()),
        Err(closed("resume_agent", Closure::ApprovalRequired))
    );
    assert_eq!(agent_state(&state, transcendent), Some(AgentState::Paused));
    assert_eq!(state.audit.lock().unwrap().events().len(), events);

    // Resuming a paused agent below L6 is unchanged.
    let below = create_agent(&state, build_manifest("below-resume")).unwrap();
    pause_agent(&state, below.clone()).unwrap();
    assert_eq!(resume_agent(&state, below.clone()), Ok(()));
    assert_eq!(
        agent_state(&state, Uuid::parse_str(&below).unwrap()),
        Some(AgentState::Running)
    );
}

/// P0-FINAL-GATE (schedule refusal, a coordinator request following stream
/// 3's review of stream 6): a manifest schedule the scheduler refuses is
/// refused by `create_agent` before anything is registered or written, and
/// by `start_agent` before the agent is restarted. The scheduler's reason is
/// passed through: it begins "invalid cron expression". Both commands used
/// to report success while `register_manifest_schedule` dropped the schedule
/// with only a log line. An accepted schedule is still registered.
///
/// The expression here is malformed, so the scheduler refuses it on every
/// branch. A sub-minute schedule is refused only once stream 6's
/// once-per-minute bound is composed; that case is tested there.
#[test]
fn p0_fg_refused_manifest_schedules_fail_create_and_start() {
    use nexus_kernel::lifecycle::AgentState;
    let state = AppState::new_in_memory();
    // The scheduler starts a task per registration; nothing here polls it.
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let _entered = runtime.enter();
    let manifest = |name: &str, schedule: Option<&str>| {
        json!({
            "name": name,
            "version": "1.0.0",
            "capabilities": ["llm.query"],
            "fuel_budget": 1000,
            "schedule": schedule,
            "default_goal": "p0fg scheduled goal",
        })
        .to_string()
    };
    let malformed = "p0fg is not a schedule";
    let snapshot = |state: &AppState| {
        (
            state.db.list_agents().unwrap().len(),
            state.supervisor.lock().unwrap().health_check().len(),
            state.meta.lock().unwrap().len(),
            state.agent_scheduler.list().len(),
            state.audit.lock().unwrap().events().len(),
        )
    };

    let before = snapshot(&state);
    let refused = create_agent(&state, manifest("scheduled-refused", Some(malformed))).unwrap_err();
    assert!(refused.starts_with("invalid cron expression"), "{refused}");
    assert_eq!(snapshot(&state), before);

    // A stored agent whose schedule the scheduler refuses is not started.
    let stored = create_agent(&state, manifest("scheduled-stored", None)).unwrap();
    stop_agent(&state, stored.clone()).unwrap();
    state
        .db
        .save_agent(
            &stored,
            &manifest("scheduled-stored", Some(malformed)),
            "stopped",
            0,
            "native",
        )
        .unwrap();
    let before = snapshot(&state);
    let refused = start_agent(&state, stored.clone()).unwrap_err();
    assert!(refused.starts_with("invalid cron expression"), "{refused}");
    assert_eq!(snapshot(&state), before);
    let stored_id = Uuid::parse_str(&stored).unwrap();
    assert_eq!(
        state
            .supervisor
            .lock()
            .unwrap()
            .get_agent(stored_id)
            .map(|handle| handle.state),
        Some(AgentState::Stopped)
    );

    // An accepted schedule is still registered.
    let accepted =
        create_agent(&state, manifest("scheduled-accepted", Some("0 0 9 * * *"))).unwrap();
    assert_eq!(
        state
            .agent_scheduler
            .list()
            .iter()
            .filter(|scheduled| scheduled.agent_id == accepted)
            .count(),
        1
    );
    state.agent_scheduler.unregister_agent(&accepted);
}

/// P0-FINAL-GATE (item G): the goal, autonomous-loop and tool routes refuse
/// an L6 (transcendent) agent before anything changes, with the bounded
/// `ApprovalRequired` reason:
/// - `assign_agent_goal`, and `execute_agent_goal` through it;
/// - `start_autonomous_loop`;
/// - the level resolution of `tools_execute`, at any claimed level.
///
/// No goal, loop, task, schedule or audit event results, and no input is
/// echoed. Both a registered L6 agent and a stored L6 record are refused.
/// (No current route registers an L6 agent; the direct registration stands
/// in for one.) An L5 agent is not refused.
#[test]
fn p0_fg_goal_loop_and_tool_routes_refuse_a_transcendent_agent() {
    use crate::commands::cognitive::{assign_agent_goal, execute_agent_goal};
    use crate::commands::crate_bridges::tool_call_autonomy;
    use crate::phase0_surface::{closed, Closure};
    let state = AppState::new_in_memory();
    // The scheduler starts a task per registration; nothing here polls it.
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let _entered = runtime.enter();

    let registered = state
        .supervisor
        .lock()
        .unwrap()
        .start_agent(
            parse_agent_manifest_json(&build_transcendent_manifest("transcendent-goal")).unwrap(),
        )
        .unwrap()
        .to_string();
    let stored = Uuid::new_v4().to_string();
    state
        .db
        .save_agent(
            &stored,
            &build_transcendent_manifest("transcendent-stored"),
            "running",
            6,
            "native",
        )
        .unwrap();
    let sovereign = create_agent(
        &state,
        json!({
            "name": "sovereign-goal",
            "version": "1.0.0",
            "capabilities": ["llm.query"],
            "fuel_budget": 1000,
            "autonomy_level": 5,
        })
        .to_string(),
    )
    .unwrap();

    let audit_events = |state: &AppState| state.audit.lock().unwrap().events().len();
    let before = audit_events(&state);
    let refused = |surface| Err(closed(surface, Closure::ApprovalRequired));
    let sentinel = "p0fg-goal-sentinel";
    for agent in [&registered, &stored] {
        assert_eq!(
            assign_agent_goal(&state, agent.clone(), sentinel.into(), 5, None),
            refused("assign_agent_goal")
        );
        assert_eq!(
            execute_agent_goal(&state, agent.clone(), sentinel.into(), 5, None),
            refused("assign_agent_goal")
        );
        assert_eq!(
            super::start_autonomous_loop(&state, agent.clone(), Some(5), Some(sentinel.into())),
            Err(closed("start_autonomous_loop", Closure::ApprovalRequired))
        );
        assert!(!state.cognitive_runtime.has_active_loop(agent));
        assert!(state.db.load_tasks_by_agent(agent, 10).unwrap().is_empty());
    }
    for claimed in [0, 1, 5, 6, u8::MAX] {
        assert_eq!(
            tool_call_autonomy(&state, &registered, claimed),
            Err(closed("tools_execute", Closure::ApprovalRequired))
        );
    }
    // A stored L6 record is not registered, so no tool call can name it.
    assert_eq!(
        tool_call_autonomy(&state, &stored, 5),
        Err("tools_execute: agent_id must name a registered agent".to_string())
    );
    assert!(state.agent_scheduler.list().is_empty());
    assert_eq!(audit_events(&state), before);

    // An L5 agent is not refused.
    assign_agent_goal(&state, sovereign.clone(), "control goal".into(), 5, None).unwrap();
    assert!(state.cognitive_runtime.has_active_loop(&sovereign));
    super::start_autonomous_loop(&state, sovereign.clone(), Some(120), None).unwrap();
    assert_eq!(state.agent_scheduler.list().len(), 1);
    state.agent_scheduler.unregister_agent(&sovereign);
    assert_eq!(tool_call_autonomy(&state, &sovereign, 6), Ok(5));
}

/// P0-FINAL-GATE (item G, review W4): the L6 check finds a stored L6 record
/// under any spelling of its id: canonical, upper case, braced, as a URN or
/// without hyphens. The stored branch used to compare the text given with
/// the stored id, so another spelling of a stored-only L6 record's id passed
/// the check. A goal for it then reached the rate limit, and an autonomous
/// loop for it was registered with the scheduler.
#[test]
fn p0_fg_transcendent_check_matches_every_spelling_of_a_stored_id() {
    use crate::commands::agents::is_transcendent_agent;
    use crate::commands::cognitive::assign_agent_goal;
    use crate::phase0_surface::{closed, Closure};
    let state = AppState::new_in_memory();
    // The scheduler starts a task per registration; nothing here polls it.
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let _entered = runtime.enter();
    let id = Uuid::new_v4();
    state
        .db
        .save_agent(
            &id.to_string(),
            &build_transcendent_manifest("transcendent-spelling"),
            "running",
            6,
            "native",
        )
        .unwrap();
    for spelling in [
        id.to_string(),
        id.to_string().to_uppercase(),
        format!("{{{id}}}"),
        format!("urn:uuid:{id}"),
        id.simple().to_string(),
    ] {
        assert!(is_transcendent_agent(&state, &spelling), "{spelling}");
        assert_eq!(
            assign_agent_goal(&state, spelling.clone(), "p0fg".into(), 5, None),
            Err(closed("assign_agent_goal", Closure::ApprovalRequired)),
            "{spelling}"
        );
        assert_eq!(
            super::start_autonomous_loop(&state, spelling.clone(), Some(120), None),
            Err(closed("start_autonomous_loop", Closure::ApprovalRequired)),
            "{spelling}"
        );
    }
    assert!(state.agent_scheduler.list().is_empty());
    assert!(!is_transcendent_agent(&state, &Uuid::new_v4().to_string()));
    assert!(!is_transcendent_agent(&state, "not-an-agent-id"));
}

/// P0-FINAL-GATE (item G, review W5): a stored record above L6 counts as L6.
/// Manifest validation admits no level above 6, but a stored record is read
/// without it. The checks used to compare with 6 exactly, so a stored
/// autonomy 7 passed:
/// - `start_agent` went on to the supervisor;
/// - a goal reached the rate limit;
/// - an autonomous loop was registered.
///
/// Every check now uses the bound `TRANSCENDENT_AUTONOMY` (6 and above).
/// Levels up to 5 are unaffected.
#[test]
fn p0_fg_stored_levels_above_l6_count_as_transcendent() {
    use crate::commands::agents::{is_transcendent_agent, is_transcendent_level};
    use crate::commands::cognitive::assign_agent_goal;
    use crate::phase0_surface::{closed, Closure};
    for level in 0..=5 {
        assert!(!is_transcendent_level(level), "{level}");
    }
    for level in [6, 7, u8::MAX] {
        assert!(is_transcendent_level(level), "{level}");
    }

    let state = AppState::new_in_memory();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let _entered = runtime.enter();
    let beyond = Uuid::new_v4().to_string();
    let manifest = json!({
        "name": "beyond-transcendent",
        "version": "1.0.0",
        "capabilities": ["llm.query"],
        "fuel_budget": 1000,
        "autonomy_level": 7,
    })
    .to_string();
    state
        .db
        .save_agent(&beyond, &manifest, "stopped", 7, "native")
        .unwrap();
    assert!(is_transcendent_agent(&state, &beyond));
    assert_eq!(
        start_agent(&state, beyond.clone()),
        Err(closed("start_agent", Closure::ApprovalRequired))
    );
    assert_eq!(
        assign_agent_goal(&state, beyond.clone(), "p0fg".into(), 5, None),
        Err(closed("assign_agent_goal", Closure::ApprovalRequired))
    );
    assert_eq!(
        super::start_autonomous_loop(&state, beyond.clone(), Some(120), None),
        Err(closed("start_autonomous_loop", Closure::ApprovalRequired))
    );
    assert!(state.agent_scheduler.list().is_empty());
}

/// P0-FINAL-GATE (item G): in Phase Zero an enabled Warden review denies
/// with the bounded `WARDEN_REVIEW_UNAVAILABLE` reason, and nothing can stand
/// in for the Warden. That holds in each of these cases:
/// - no agent named "nexus-warden" exists;
/// - the prebuilt Warden's record is stored by an earlier build and left
///   unregistered by restore;
/// - a caller-created agent named "nexus-warden" (L2, as `create_agent`
///   accepts) is stopped;
/// - that agent is running. Its model's YES used to allow the action and was
///   audited as a Warden review.
///
/// The decision takes only the review flag. No Warden is looked up and no
/// model is resolved or queried: the engine holds no model code, and the
/// `fg_approval` guard pins its whole body. No audit event or consent
/// request is written. A disabled review (the default) allows, as before.
#[test]
fn p0_fg_enabled_warden_review_denies_and_no_stand_in_can_allow() {
    use crate::commands::cognitive::{WardenReviewEngine, WARDEN_REVIEW_UNAVAILABLE};
    use nexus_kernel::actuators::ActionReviewDecision;
    let state = AppState::new_in_memory();
    let engine = WardenReviewEngine {
        state: state.clone(),
    };
    let check = |case: &str| {
        let before = (
            state.audit.lock().unwrap().events().len(),
            state.db.load_pending_consent().unwrap().len(),
            state.db.get_audit_count().unwrap(),
        );
        assert_eq!(
            engine.review_with(true),
            ActionReviewDecision::Deny {
                reason: WARDEN_REVIEW_UNAVAILABLE.to_string(),
            },
            "{case}"
        );
        assert_eq!(
            engine.review_with(false),
            ActionReviewDecision::Allow {
                reason: "Warden governance review disabled".to_string(),
            },
            "{case}"
        );
        let after = (
            state.audit.lock().unwrap().events().len(),
            state.db.load_pending_consent().unwrap().len(),
            state.db.get_audit_count().unwrap(),
        );
        assert_eq!(after, before, "{case}");
    };

    check("no Warden");

    let warden = list_prebuilt_manifest_paths()
        .into_iter()
        .find(|path| path.file_name().and_then(|name| name.to_str()) == Some("warden.json"))
        .unwrap();
    let warden_json = std::fs::read_to_string(warden).unwrap();
    assert_eq!(
        parse_agent_manifest_json(&warden_json)
            .unwrap()
            .autonomy_level,
        Some(6)
    );
    state
        .db
        .save_agent(
            &Uuid::new_v4().to_string(),
            &warden_json,
            "running",
            6,
            "native",
        )
        .unwrap();
    crate::commands::agents::restore_persisted_agents(&state);
    check("stored prebuilt Warden");

    let stand_in = create_agent(
        &state,
        json!({
            "name": "nexus-warden",
            "version": "1.0.0",
            "capabilities": ["llm.query"],
            "fuel_budget": 1000,
            "autonomy_level": 2,
            "llm_model": "p0fg-warden-model",
        })
        .to_string(),
    )
    .unwrap();
    stop_agent(&state, stand_in.clone()).unwrap();
    check("stopped stand-in");
    start_agent(&state, stand_in.clone()).unwrap();
    let running = Uuid::parse_str(&stand_in).unwrap();
    assert!(state
        .supervisor
        .lock()
        .unwrap()
        .health_check()
        .iter()
        .any(|status| status.id == running
            && status.state == nexus_kernel::lifecycle::AgentState::Running));
    check("running stand-in");
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
    let state = AppState::new_in_memory();
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
    let state = AppState::new_in_memory();
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
    let state = AppState::new_in_memory();
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
    let state = AppState::new_in_memory();
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
    let state = AppState::new_in_memory();
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
    let state = AppState::new_in_memory();
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
    let state = AppState::new_in_memory();
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
    let state = AppState::new_in_memory();
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
    let state = AppState::new_in_memory();
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

    let state = AppState::new_in_memory();

    // Index the document
    let ingest_result = ingest_test_document(
        &state,
        "nexus_rag_test_chat.txt",
        "Rust is a systems programming language focused on safety.",
    );
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
    // Note: don't remove LLM_PROVIDER — tests run in parallel in the same process.
}

#[test]
fn test_provider_status_command() {
    let state = AppState::new_in_memory();
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
    let state = AppState::new_in_memory();
    let _ = ingest_test_document(
        &state,
        "nexus_test_search_e2e.txt",
        "Quantum computing uses qubits for parallel computation.",
    )
    .unwrap_or_else(|e| {
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
    let state = AppState::new_in_memory();
    let _ = ingest_test_document(&state, "nexus_test_list_a.txt", "Document A content.")
        .unwrap_or_else(|e| {
            eprintln!("operation failed: {e}");
            std::process::exit(1)
        });
    let _ = ingest_test_document(&state, "nexus_test_list_b.txt", "Document B content.")
        .unwrap_or_else(|e| {
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
    let state = AppState::new_in_memory();
    let path_str = "nexus_test_remove.txt".to_string();

    let _ = ingest_test_document(&state, &path_str, "Content to be removed.").unwrap_or_else(|e| {
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
    // Note: don't remove LLM_PROVIDER — tests run in parallel in the same process.
}

// ── Model Hub wiring tests ──────────────────────────────────────────

#[test]
fn test_list_local_models_returns_array() {
    let state = AppState::new_in_memory();
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
    let state = AppState::new_in_memory();
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
    let state = AppState::new_in_memory();
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
    let state = AppState::new_in_memory();
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
    let state = AppState::new_in_memory();

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
    let state = AppState::new_in_memory();
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
    let state = AppState::new_in_memory();
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
fn test_voice_transcribe_returns_engine_field() {
    std::env::set_var("LLM_PROVIDER", "mock");
    let state = AppState::new_in_memory();
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
    let state = AppState::new_in_memory();
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
    let state = AppState::new_in_memory();
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
    let state = AppState::new_in_memory();
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
    let state = AppState::new_in_memory();
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
    let state = AppState::new_in_memory();

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
    let state = AppState::new_in_memory();
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
    let state = AppState::new_in_memory();
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
    let state = AppState::new_in_memory();
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
    let state = AppState::new_in_memory();

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
    let state = AppState::new_in_memory();
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
    let state = AppState::new_in_memory();
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
    let state = AppState::new_in_memory();

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
    let state = AppState::new_in_memory();
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

// ── Payments wiring tests ───────────────────────────────────────────

#[test]
fn test_payment_plan_and_invoice() {
    let state = AppState::new_in_memory();

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
    let state = AppState::new_in_memory();

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
    let result = approve_consent_request(&state, "c-approve-1".into());
    assert!(result.is_ok());
    // Verify it's no longer pending
    let pending = list_pending_consents(&state).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    assert!(pending.iter().all(|p| p.consent_id != "c-approve-1"));
}

/// P0-FINAL-GATE (item G): approving a transcendent request (one enqueued
/// before this closure) used to create or restart an L6 agent on any caller's
/// word. It is now refused before the request is resolved: nothing is
/// created, started, resolved or audited as approved. A batch approval
/// cannot resolve it either, nor can a review-each resolution (review W8).
/// The request can still be denied.
#[test]
fn p0_fg_transcendent_approval_is_refused_and_changes_nothing() {
    use crate::phase0_surface::{closed, Closure};
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
        .unwrap();
    let request = |mode: &str, goal: Option<&str>| {
        json!({
            "summary": "Create L6 Transcendent agent 'approved-transcendent'",
            "side_effects": ["Maximum-autonomy L6 activation"],
            "fuel_cost": 0.0,
            "min_review_seconds": 60,
            "mode": mode,
            "manifest_json": manifest_json,
            "goal_id": goal,
        })
    };
    for (id, mode, goal) in [
        ("c-transcendent-create", "create_new", None),
        (
            "c-transcendent-activate",
            "activate_existing",
            Some("goal-transcendent"),
        ),
    ] {
        enqueue_test_consent_json(
            &state,
            id,
            &pending_agent_id,
            "transcendent_creation",
            "Tier3",
            request(mode, goal),
        );
    }
    let stored = state.db.list_agents().unwrap().len();
    let registered = state.supervisor.lock().unwrap().health_check().len();

    for id in ["c-transcendent-create", "c-transcendent-activate"] {
        assert_eq!(
            approve_consent_request(&state, id.into()).map(|_| ()),
            Err(closed("approve_consent_request", Closure::ApprovalRequired))
        );
    }
    assert_eq!(
        batch_approve_consents(&state, "goal-transcendent".into()).map(|_| ()),
        Err(closed("batch_approve_consents", Closure::ApprovalRequired))
    );
    // Review W8: nor can it be resolved into review-each mode.
    for id in ["c-transcendent-create", "c-transcendent-activate"] {
        assert_eq!(
            review_consent_batch(&state, id.into()).map(|_| ()),
            Err(closed("review_consent_batch", Closure::ApprovalRequired))
        );
    }
    assert!(!state
        .cognitive_runtime
        .review_each_mode(&pending_agent_id)
        .unwrap_or(false));

    let pending = state.db.load_pending_consent().unwrap();
    assert_eq!(pending.len(), 2);
    assert!(pending
        .iter()
        .all(|row| row.status == "pending" && row.resolved_by.is_none()));
    assert_eq!(state.db.list_agents().unwrap().len(), stored);
    assert_eq!(
        state.supervisor.lock().unwrap().health_check().len(),
        registered
    );
    let events = state.db.load_audit_events(None, 200, 0).unwrap();
    for approval in [
        "consent_approved",
        "consent_batch_approved",
        "consent_batch_review_each",
        "transcendent_creation_approved",
    ] {
        assert!(
            !events
                .iter()
                .any(|event| event.detail_json.contains(approval)),
            "{approval}"
        );
    }

    // Denial still resolves the request and removes the unapproved placeholder.
    deny_consent_request(&state, "c-transcendent-create".into(), None).unwrap();
    assert!(state
        .db
        .list_agents()
        .unwrap()
        .iter()
        .all(|row| row.id != pending_agent_id));
    assert_eq!(state.db.load_pending_consent().unwrap().len(), 1);
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

        approve_consent_request(&state, "c-approve-wake".into()).unwrap_or_else(|e| {
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
fn test_deny_consent_request() {
    let state = AppState::new_in_memory();
    enqueue_test_consent(&state, "c-deny-1", "a2", "process.exec", "Tier2");
    let result = deny_consent_request(&state, "c-deny-1".into(), Some("too risky".into()));
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

        deny_consent_request(&state, "c-deny-wake".into(), Some("too risky".into()))
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
    approve_consent_request(&state, "c-lp-1".into()).unwrap_or_else(|e| {
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
    approve_consent_request(&state, "c-hist-1".into()).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    deny_consent_request(&state, "c-hist-2".into(), None).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    approve_consent_request(&state, "c-hist-3".into()).unwrap_or_else(|e| {
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
    let result = approve_consent_request(&state, "nonexistent-id".into());
    assert!(result.is_err());
}

#[test]
fn test_deny_nonexistent_consent_fails() {
    let state = AppState::new_in_memory();
    let result = deny_consent_request(&state, "nonexistent-id".into(), None);
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

    let (resolved, meta) =
        batch_approve_consents(&state, "goal-batch".into()).unwrap_or_else(|e| {
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

    let meta = review_consent_batch(&state, "c-review-batch-1".into()).unwrap_or_else(|e| {
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

    let (resolved, meta) = batch_deny_consents(&state, "goal-deny".into(), Some("deny all".into()))
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
    approve_consent_request(&state, "c-audit-a".into()).unwrap_or_else(|e| {
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
    deny_consent_request(&state, "c-audit-d".into(), Some("unauthorized".into())).unwrap_or_else(
        |e| {
            eprintln!("operation failed: {e}");
            std::process::exit(1)
        },
    );
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
    approve_consent_request(&state, "c-double".into()).unwrap_or_else(|e| {
        eprintln!("operation failed: {e}");
        std::process::exit(1)
    });
    // Second approve should fail (no longer pending)
    let result = approve_consent_request(&state, "c-double".into());
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
    let result = deny_consent_request(&state, "c-no-reason".into(), None);
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

    approve_consent_request(&state, "c-resolve-1".into()).unwrap_or_else(|e| {
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

    deny_consent_request(&state, "c-resolve-2".into(), None).unwrap_or_else(|e| {
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

/// P0-FINAL-GATE (item G): a decision delivered by the desktop interface is
/// recorded with the fixed resolver label, in the consent row and the audit
/// event, never with a name the caller chose. None of the five resolution
/// functions takes a name: each records this label itself (pinned by
/// `fg_approval`), so no command wrapper can pass another.
#[test]
fn p0_fg_desktop_consent_resolutions_record_the_interface_label() {
    let resolver = super::DESKTOP_UI_RESOLVER;
    assert_eq!(resolver, "desktop-ui (unverified)");
    let state = AppState::new_in_memory();
    enqueue_test_consent(&state, "c-label-approve", "a1", "fs.write", "Tier1");
    enqueue_test_consent(&state, "c-label-deny", "a1", "fs.write", "Tier1");
    for (id, goal) in [
        ("c-label-batch-approve", "goal-label-approve"),
        ("c-label-review", "goal-label-review"),
        ("c-label-batch-deny", "goal-label-deny"),
    ] {
        enqueue_test_consent_json(
            &state,
            id,
            "a1",
            "cognitive.hitl_batch",
            "Tier1",
            json!({"summary": "batch", "goal_id": goal, "review_each_available": true}),
        );
    }
    approve_consent_request(&state, "c-label-approve".into()).unwrap();
    deny_consent_request(&state, "c-label-deny".into(), None).unwrap();
    batch_approve_consents(&state, "goal-label-approve".into()).unwrap();
    review_consent_batch(&state, "c-label-review".into()).unwrap();
    batch_deny_consents(&state, "goal-label-deny".into(), None).unwrap();

    let history = get_consent_history(&state, 10).unwrap();
    for (id, status) in [
        ("c-label-approve", "approved"),
        ("c-label-deny", "denied"),
        ("c-label-batch-approve", "approved"),
        ("c-label-review", "review_each"),
        ("c-label-batch-deny", "denied"),
    ] {
        let row = history.iter().find(|row| row.consent_id == id).unwrap();
        assert_eq!(row.status, status);
        assert_eq!(row.resolved_by.as_deref(), Some(resolver));
    }
    let events = state.db.load_audit_events(None, 100, 0).unwrap();
    for (action, field) in [
        ("consent_approved", "approved_by"),
        ("consent_denied", "denied_by"),
        ("consent_batch_approved", "approved_by"),
        ("consent_batch_review_each", "reviewed_by"),
        ("consent_batch_denied", "denied_by"),
    ] {
        let event = events
            .iter()
            .find(|event| event.detail_json.contains(action))
            .unwrap_or_else(|| panic!("{action} audited"));
        let detail: serde_json::Value = serde_json::from_str(&event.detail_json).unwrap();
        assert_eq!(detail[field], json!(resolver), "{}", event.detail_json);
    }
}

/// P0-FINAL-GATE (item G): an approval delivered over desktop IPC never
/// reaches the kernel consent runtime. That queue records approvals by
/// approver identity, and the desktop has none to give. Even when a kernel
/// policy would accept the interface label as an approver, and a desktop
/// consent row carries the kernel request's own id, approving the row leaves
/// the kernel request unapproved: the governed operation still requires
/// approval afterwards.
#[test]
fn p0_fg_desktop_approvals_do_not_reach_the_kernel_consent_queue() {
    use nexus_kernel::consent::{GovernedOperation, HitlTier};
    use nexus_kernel::errors::AgentError;
    let resolver = super::DESKTOP_UI_RESOLVER;
    let state = AppState::new_in_memory();
    let agent = create_agent(&state, build_manifest("kernel-consent-agent")).unwrap();
    let id = Uuid::parse_str(&agent).unwrap();
    let request = |state: &AppState| {
        state.supervisor.lock().unwrap().require_consent(
            id,
            GovernedOperation::TerminalCommand,
            b"p0-fg-probe",
        )
    };
    {
        let mut supervisor = state.supervisor.lock().unwrap();
        let handle = supervisor.get_agent_mut(id).unwrap();
        handle.consent_runtime.policy_engine_mut().set_policy(
            GovernedOperation::TerminalCommand,
            HitlTier::Tier2,
            vec![resolver.to_string()],
        );
    }
    let request_id = match request(&state) {
        Err(AgentError::ApprovalRequired { request_id }) => request_id,
        other => panic!("the kernel must require approval first: {other:?}"),
    };

    enqueue_test_consent(
        &state,
        &request_id,
        &agent,
        "cognitive.hitl_approval",
        "Tier2",
    );
    approve_consent_request(&state, request_id.clone()).unwrap();
    let history = get_consent_history(&state, 10).unwrap();
    let row = history
        .iter()
        .find(|row| row.consent_id == request_id)
        .unwrap();
    assert_eq!(row.status, "approved");

    assert_eq!(
        request(&state),
        Err(AgentError::ApprovalRequired {
            request_id: request_id.clone()
        })
    );
}

/// P0-FINAL-GATE (item G): accepting a self-improvement proposal in the
/// interface is an IPC call, not the Tier3 HITL approval that invariant #9
/// requires, and the pipeline applies nothing. The proposal is recorded as
/// accepted and nothing more: its status stays `Proposed`, with no
/// checkpoint and no canary, and the audit event says it was neither
/// HITL-approved nor applied. It used to be recorded as validated with a
/// fabricated `hitl:approved` signature, audited as applied, and put under
/// canary "monitoring". A proposal that breaks another invariant is still
/// refused, and nothing is recorded for it.
#[test]
fn p0_fg_self_improvement_acceptance_claims_no_hitl_approval() {
    use crate::commands::self_improvement::self_improve_approve_proposal;
    use nexus_self_improve::types::{
        ImprovementProposal, ImprovementStatus, ProposedChange, RollbackPlan, RollbackStep,
    };
    let proposal = |fuel_cost: u64| {
        let change = ProposedChange::ConfigChange {
            key: "agent.response_timeout_ms".into(),
            old_value: json!(5000),
            new_value: json!(6000),
            justification: "p0-fg latency".into(),
        };
        ImprovementProposal {
            id: Uuid::new_v4(),
            opportunity_id: Uuid::new_v4(),
            domain: change.domain(),
            description: "p0-fg proposal".into(),
            change,
            rollback_plan: RollbackPlan {
                checkpoint_id: Uuid::new_v4(),
                steps: vec![RollbackStep {
                    description: "revert".into(),
                    action: json!({"revert": true}),
                }],
                estimated_rollback_time_ms: 100,
                automatic: true,
            },
            expected_tests: vec![],
            proof: None,
            generated_by: "test".into(),
            fuel_cost,
        }
    };
    let state = AppState::new_in_memory();
    let accepted = proposal(100);
    let over_budget = proposal(u64::MAX);
    state
        .self_improve_state
        .lock()
        .unwrap()
        .proposals
        .extend([accepted.clone(), over_budget.clone()]);

    let recorded = self_improve_approve_proposal(&state, accepted.id.to_string()).unwrap();
    assert_eq!(recorded["status"], json!("Proposed"));
    assert_eq!(recorded["checkpoint_id"], json!(Uuid::nil()));
    assert_eq!(recorded["canary_deadline"], json!(0));
    {
        let si = state.self_improve_state.lock().unwrap();
        assert!(si.proposals.iter().all(|p| p.id != accepted.id));
        let entry = si
            .history
            .iter()
            .find(|entry| entry.proposal_id == accepted.id)
            .unwrap();
        assert_eq!(entry.status, ImprovementStatus::Proposed);
    }
    let events = state.audit.lock().unwrap().events().to_vec();
    let event = events
        .iter()
        .find(|event| event.payload["proposal_id"] == json!(accepted.id.to_string()))
        .expect("the acceptance is audited");
    assert_eq!(event.payload["type"], json!("self_improvement_recorded"));
    assert_eq!(event.payload["hitl_approved"], json!(false));
    assert_eq!(event.payload["applied"], json!(false));
    assert_eq!(
        event.payload["resolved_by"],
        json!(super::DESKTOP_UI_RESOLVER)
    );
    for claim in [
        "self_improvement_applied",
        "hitl:approved",
        "hitl_signature",
    ] {
        assert!(
            !events
                .iter()
                .any(|event| event.payload.to_string().contains(claim)),
            "{claim}"
        );
    }

    // Only the broken invariant is reported: #9 is not claimed, so it is not
    // part of the check.
    let refused = self_improve_approve_proposal(&state, over_budget.id.to_string()).unwrap_err();
    assert_eq!(
        refused,
        format!(
            "Invariant violations: Invariant violation #5 Fuel limits enforced: proposal costs {} fuel but only 5000 remaining",
            u64::MAX
        )
    );
    let si = state.self_improve_state.lock().unwrap();
    assert!(si.proposals.iter().any(|p| p.id == over_budget.id));
    assert!(si
        .history
        .iter()
        .all(|entry| entry.proposal_id != over_budget.id));
}

/// P0-FINAL-GATE (item G): the self-improvement report counts only applied
/// changes. An accepted proposal is recorded as `Proposed` and nothing is
/// applied. So after two acceptances the report shows no improvement
/// applied, listed or active, and no fuel consumed. `cycles_run` counts the
/// cycles actually run. (The report used to count every accepted proposal
/// as applied, give `cycles_run` as the number of history entries and
/// report the fuel budget as consumed.) A history entry whose status says it
/// was applied is still counted, so the report is filtered, not emptied.
#[test]
fn p0_fg_self_improvement_report_counts_no_recorded_acceptance_as_applied() {
    use crate::commands::self_improvement::{
        self_improve_approve_proposal, self_improve_get_report, self_improve_run_cycle,
    };
    use nexus_self_improve::types::{
        AppliedImprovement, ImprovementProposal, ImprovementStatus, ProposedChange, RollbackPlan,
        RollbackStep,
    };
    let proposal = || {
        let change = ProposedChange::ConfigChange {
            key: "agent.response_timeout_ms".into(),
            old_value: json!(5000),
            new_value: json!(6000),
            justification: "p0-fg report".into(),
        };
        ImprovementProposal {
            id: Uuid::new_v4(),
            opportunity_id: Uuid::new_v4(),
            domain: change.domain(),
            description: "p0-fg report proposal".into(),
            change,
            rollback_plan: RollbackPlan {
                checkpoint_id: Uuid::new_v4(),
                steps: vec![RollbackStep {
                    description: "revert".into(),
                    action: json!({"revert": true}),
                }],
                estimated_rollback_time_ms: 100,
                automatic: true,
            },
            expected_tests: vec![],
            proof: None,
            generated_by: "test".into(),
            fuel_cost: 100,
        }
    };
    let state = AppState::new_in_memory();
    let accepted = [proposal(), proposal()];
    state
        .self_improve_state
        .lock()
        .unwrap()
        .proposals
        .extend(accepted.clone());
    for proposal in &accepted {
        self_improve_approve_proposal(&state, proposal.id.to_string()).unwrap();
    }
    assert_eq!(state.self_improve_state.lock().unwrap().history.len(), 2);

    let report = self_improve_get_report(&state, 30).unwrap();
    assert_eq!(report["improvements_applied"], json!(0));
    assert_eq!(report["improvements_committed"], json!(0));
    assert_eq!(report["top_improvements"], json!([]));
    assert_eq!(report["domains_active"], json!([]));
    assert_eq!(report["cycles_run"], json!(0));
    assert_eq!(report["fuel_consumed"], json!(0));

    self_improve_run_cycle(&state).unwrap();
    let report = self_improve_get_report(&state, 30).unwrap();
    assert_eq!(report["cycles_run"], json!(1));
    assert_eq!(report["improvements_applied"], json!(0));

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    state
        .self_improve_state
        .lock()
        .unwrap()
        .history
        .push(AppliedImprovement {
            id: Uuid::new_v4(),
            proposal_id: Uuid::new_v4(),
            checkpoint_id: Uuid::new_v4(),
            applied_at: now,
            status: ImprovementStatus::Committed,
            canary_deadline: 0,
        });
    let report = self_improve_get_report(&state, 30).unwrap();
    assert_eq!(report["improvements_applied"], json!(1));
    assert_eq!(report["improvements_committed"], json!(1));
}

/// P0-FINAL-GATE (item K, cross-stream request from stream 6): a parallel
/// simulation request for a variant count outside 1..=10 is refused before
/// the simulation model is built, the seed is parsed or any variant is
/// started. No model is built or called, and nothing is audited. In-range
/// requests run as before and do call the model.
#[test]
fn p0_fg_parallel_simulation_variants_are_bounded_before_any_model_call() {
    use crate::commands::consent::run_parallel_simulation_reports_with;
    use nexus_kernel::cognitive::PlannerLlm;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct CountingLlm(Arc<AtomicUsize>);
    impl PlannerLlm for CountingLlm {
        fn plan_query(&self, prompt: &str) -> Result<String, nexus_kernel::errors::AgentError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            crate::commands::cognitive::TestSimulationPlannerLlm.plan_query(prompt)
        }
    }

    let state = AppState::new_in_memory();
    let built = Arc::new(AtomicUsize::new(0));
    let queries = Arc::new(AtomicUsize::new(0));
    let model = || {
        let (built, queries) = (built.clone(), queries.clone());
        move || -> Arc<dyn PlannerLlm> {
            built.fetch_add(1, Ordering::SeqCst);
            Arc::new(CountingLlm(queries))
        }
    };
    let seed = "Macro outlook with rate pressure.";

    for variants in [0, 11] {
        assert_eq!(
            run_parallel_simulation_reports_with(&state, seed.into(), variants, model())
                .map(|reports| reports.len()),
            Err("variant_count must be between 1 and 10".to_string()),
            "{variants}"
        );
    }
    assert_eq!(built.load(Ordering::SeqCst), 0, "no model is built");
    assert_eq!(queries.load(Ordering::SeqCst), 0, "no model is called");
    let events = state.db.load_audit_events(None, 100, 0).unwrap();
    assert!(!events
        .iter()
        .any(|event| event.detail_json.contains("run_parallel_simulations")));

    for variants in [1, 10] {
        let reports =
            run_parallel_simulation_reports_with(&state, seed.into(), variants, model()).unwrap();
        assert_eq!(reports.len(), variants as usize);
    }
    assert_eq!(built.load(Ordering::SeqCst), 2);
    assert!(queries.load(Ordering::SeqCst) > 0);
}

/// P0-FINAL-GATE (item K, cross-stream request from stream 6): an adversarial
/// session outside 1..=50 rounds is refused before the arena is built or any
/// round is allocated or run. The arena calls no model. In-range sessions run
/// as before, with one result per round.
#[test]
fn p0_fg_adversarial_session_rounds_are_bounded_before_any_work() {
    use crate::commands::consent::run_adversarial_session;
    for rounds in [0, 51] {
        assert_eq!(
            run_adversarial_session("attacker".into(), "defender".into(), rounds),
            Err(format!(
                "arena rounds must be between 1 and 50, got {rounds}"
            )),
            "{rounds}"
        );
    }
    for rounds in [1, 50] {
        let session =
            run_adversarial_session("attacker".into(), "defender".into(), rounds).unwrap();
        assert_eq!(session["rounds"], json!(rounds));
        assert_eq!(
            session["results"].as_array().map(Vec::len),
            Some(rounds as usize)
        );
    }
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

// ── P0-002C4D0: legacy Builder static build is closed ─────────────────────

// Returns one function (signature through its matching closing brace) with
// comments removed. CRLF is normalized first and braces are matched by depth,
// skipping comments and string literals, so the result is identical on LF and
// CRLF checkouts.
fn p0_002c4d0_function(source: &str, signature: &str) -> String {
    let source = source.replace("\r\n", "\n");
    let start = source.find(signature).expect("function signature");
    let chars: Vec<char> = source[start..].chars().collect();
    let (mut out, mut depth, mut i) = (String::new(), 0usize, 0usize);
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        if c == '/' && next == Some('/') {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if c == '/' && next == Some('*') {
            i += 2;
            while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            i += 2;
            continue;
        }
        out.push(c);
        i += 1;
        match c {
            '"' => {
                while i < chars.len() && chars[i] != '"' {
                    if chars[i] == '\\' {
                        out.push(chars[i]);
                        i += 1;
                    }
                    out.push(chars[i]);
                    i += 1;
                }
                out.push('"');
                i += 1;
            }
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return out;
                }
            }
            _ => {}
        }
    }
    panic!("unbalanced function: {signature}");
}

#[test]
fn p0_002c4d0_function_extraction_is_line_ending_agnostic() {
    let lf = "fn before() {}\nfn target(x: u8) -> u8 {\n    // a { comment\n    \
              let s = \"}\\\"{\";\n    /* } */ if x > 0 { 1 } else { 0 }\n}\nfn after() {}\n";
    let crlf = lf.replace('\n', "\r\n");
    assert!(crlf.contains("\r\n}\r\n"));
    let extracted = p0_002c4d0_function(lf, "fn target(");
    assert_eq!(p0_002c4d0_function(&crlf, "fn target("), extracted);
    assert!(extracted.starts_with("fn target(x: u8) -> u8 {"));
    assert!(extracted.ends_with("{ 1 } else { 0 }\n}"));
    assert!(!extracted.contains("comment") && !extracted.contains("fn after"));
    assert!(extracted.contains("let s = \"}\\\"{\";"));
}

// Security invariant guard: the production `builder_build_static` command is a
// direct bounded denial. It cannot derive a path from HOME or `project_id`,
// touch the filesystem, or launch npm/npx/Node/Vite or any other process.
#[test]
fn p0_002c4d0_builder_build_static_always_denies_without_path_or_process() {
    let lib = include_str!("lib.rs");
    let command = p0_002c4d0_function(lib, "fn builder_build_static(");
    let body = &command[command.find('{').unwrap()..];
    let compact = |text: &str| text.split_whitespace().collect::<String>();
    assert_eq!(
        compact(body),
        compact(r#"{ Err("Builder static build: build unavailable".into()) }"#),
        "{command}"
    );
    assert!(!body.contains("project_id"), "project_id is not authority");
    for forbidden in [
        "std::env",
        "env::",
        "HOME",
        "USERPROFILE",
        "\".\"",
        ".nexus",
        "builds",
        "Path",
        "exists",
        "package.json",
        "node_modules",
        "dist",
        "fs::",
        "join(",
        "Command",
        "process",
        "spawn",
        "npm",
        "npx",
        "node",
        "vite",
        "sh",
        "cmd",
        "exec",
        "Grant",
        "grant",
        "pid",
        "port",
    ] {
        assert!(!command.contains(forbidden), "{forbidden} in:\n{command}");
    }
    // Still registered, so callers get the explicit denial; defined only once.
    let code = p0_002c4d0_code_only(lib);
    assert_eq!(code.matches("fn builder_build_static(").count(), 1);
    assert_eq!(code.matches("builder_build_static,").count(), 1);
}

fn p0_002c4d0_code_only(source: &str) -> String {
    source
        .replace("\r\n", "\n")
        .lines()
        .map(|line| line.split("//").next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n")
}

/// P0-002C5C: a stored agent record is not authority. A record naming a
/// capability outside the registry (such as `a2a.delegate`) or an undefined
/// autonomy level is not restored; a record a validated manifest could have
/// produced is. (Since the Final Gate the highest restorable level is L5: an
/// L6 record is never registered, see
/// `p0_fg_stored_transcendent_records_are_not_registered_and_stay_stored`.)
#[test]
fn p0_002c5c_persisted_agents_holding_unregistered_authority_are_not_restored() {
    let state = AppState::new_in_memory();
    let manifest = |name: &str, capabilities: &[&str], autonomy: u8| {
        json!({
            "name": name,
            "version": "1.0.0",
            "capabilities": capabilities,
            "fuel_budget": 1000,
            "autonomy_level": autonomy,
        })
        .to_string()
    };
    let delegating = Uuid::new_v4();
    let unknown = Uuid::new_v4();
    let beyond = Uuid::new_v4();
    let clean = Uuid::new_v4();
    for (id, json) in [
        (
            delegating,
            manifest("delegating-agent", &["llm.query", "a2a.delegate"], 5),
        ),
        (
            unknown,
            manifest("unknown-agent", &["llm.query", "root.all"], 1),
        ),
        (beyond, manifest("beyond-agent", &["llm.query"], 7)),
        (clean, manifest("clean-agent", &["llm.query", "fs.read"], 5)),
    ] {
        state
            .db
            .save_agent(&id.to_string(), &json, "running", 1, "native")
            .unwrap();
    }

    crate::commands::agents::restore_persisted_agents(&state);

    let supervisor = state.supervisor.lock().unwrap_or_else(|p| p.into_inner());
    for refused in [delegating, unknown, beyond] {
        assert!(supervisor.get_agent(refused).is_none(), "{refused}");
    }
    let restored = supervisor.get_agent(clean).expect("clean agent restored");
    assert_eq!(
        restored.manifest.capabilities,
        vec!["llm.query".to_string(), "fs.read".to_string()]
    );
}

/// P0-FINAL-GATE (item G): an L6 (transcendent) agent needs a human approval
/// the backend cannot verify, so restore registers no L6 record: not one
/// approved over IPC before the closure (running or stopped), and not a
/// creation request that was never approved (its `pending_approval`
/// placeholder used to come back as a registered L6 agent after a restart).
/// The stored records are left exactly as they were. An L5 record is
/// restored as before.
#[test]
fn p0_fg_stored_transcendent_records_are_not_registered_and_stay_stored() {
    let state = AppState::new_in_memory();
    let manifest = |name: &str, autonomy: u8| {
        json!({
            "name": name,
            "version": "1.0.0",
            "capabilities": ["llm.query"],
            "fuel_budget": 1000,
            "autonomy_level": autonomy,
        })
        .to_string()
    };
    let running = Uuid::new_v4();
    let stopped = Uuid::new_v4();
    let placeholder = Uuid::new_v4();
    let below = Uuid::new_v4();
    for (id, json, stored_state, level) in [
        (running, manifest("approved-transcendent", 6), "running", 6),
        (stopped, manifest("stopped-transcendent", 6), "stopped", 6),
        (
            placeholder,
            manifest("requested-transcendent", 6),
            "pending_approval",
            6,
        ),
        (below, manifest("sovereign-agent", 5), "running", 5),
    ] {
        state
            .db
            .save_agent(&id.to_string(), &json, stored_state, level, "native")
            .unwrap();
    }
    let snapshot = |state: &AppState, id: Uuid| {
        state
            .db
            .list_agents()
            .unwrap()
            .into_iter()
            .find(|row| row.id == id.to_string())
            .map(|row| {
                (
                    row.manifest_json,
                    row.state,
                    row.was_running,
                    row.autonomy_level,
                    row.execution_mode,
                    row.updated_at,
                )
            })
            .expect("stored record")
    };
    let before: Vec<_> = [running, stopped, placeholder]
        .into_iter()
        .map(|id| snapshot(&state, id))
        .collect();

    crate::commands::agents::restore_persisted_agents(&state);

    let supervisor = state.supervisor.lock().unwrap_or_else(|p| p.into_inner());
    for refused in [running, stopped, placeholder] {
        assert!(supervisor.get_agent(refused).is_none(), "{refused}");
    }
    assert!(supervisor.get_agent(below).is_some());
    assert!(supervisor.health_check().iter().all(|status| supervisor
        .get_agent(status.id)
        .unwrap()
        .autonomy_level
        < 6));
    drop(supervisor);
    let after: Vec<_> = [running, stopped, placeholder]
        .into_iter()
        .map(|id| snapshot(&state, id))
        .collect();
    assert_eq!(after, before);
}

#[test]
fn p0_002c5b_persisted_agents_naming_a_consent_policy_path_are_not_restored() {
    let state = AppState::new_in_memory();
    let dir = std::env::temp_dir().join(format!("nexus-c5b-consent-{}", Uuid::new_v4()));
    std::fs::create_dir(&dir).unwrap();
    let policy = dir.join("consent.toml");
    std::fs::write(&policy, "").unwrap();
    let manifest = |name: &str, consent: Option<&str>| {
        json!({
            "name": name,
            "version": "1.0.0",
            "capabilities": ["llm.query"],
            "fuel_budget": 1000,
            "autonomy_level": 1,
            "consent_policy_path": consent,
        })
        .to_string()
    };
    let tainted = Uuid::new_v4();
    let clean = Uuid::new_v4();
    let policy_text = policy.to_string_lossy().into_owned();
    for (id, json) in [
        (
            tainted,
            manifest("tainted-agent", Some(policy_text.as_str())),
        ),
        (clean, manifest("clean-agent", None)),
    ] {
        state
            .db
            .save_agent(&id.to_string(), &json, "running", 1, "native")
            .unwrap();
    }

    crate::commands::agents::restore_persisted_agents(&state);

    let supervisor = state.supervisor.lock().unwrap_or_else(|p| p.into_inner());
    assert!(supervisor.get_agent(tainted).is_none());
    assert!(supervisor.get_agent(clean).is_some());
    drop(supervisor);
    // The named policy was neither read into a queue nor extended.
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
    std::fs::remove_dir_all(&dir).unwrap();
}

/// P0-002C5C (Architect decision): screen observation over desktop IPC is
/// denied. The four capture routes (including the real nx handler) and the
/// enabling branch of `computer_control_toggle` return their bounded denial
/// whatever the engine or emergency-stop state is. They never enable the
/// engine, clear or bypass the emergency stop, record an action, or audit a
/// capture, analysis or enable. Status, history, disabling and stop stay
/// available, and the earlier OS-input closures stay closed. Nothing here
/// captures the screen, starts a capture tool or contacts a model: every
/// route under test fails before any of that.
#[cfg(all(
    feature = "tauri-runtime",
    any(target_os = "windows", target_os = "macos", target_os = "linux")
))]
#[test]
fn p0_002c5c_screen_observation_requests_are_denied_and_change_nothing() {
    use crate::phase0_surface::{closed, Closure};
    use crate::runtime::{
        analyze_screen, capture_screen, computer_control_capture_screen,
        computer_control_execute_action, start_computer_action,
    };
    use nexus_kernel::computer_control::{
        activate_emergency_kill_switch, emergency_kill_switch_active, reset_emergency_kill_switch,
    };

    // The emergency stop is process-wide; this is the only test that sets it.
    static EMERGENCY_STOP: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _serial = EMERGENCY_STOP.lock().unwrap_or_else(|p| p.into_inner());

    let observation = |command| Err(closed(command, Closure::ScreenObservation));
    let denied = |state: &AppState| {
        assert_eq!(capture_screen(), observation("capture_screen"));
        assert_eq!(analyze_screen(), observation("analyze_screen"));
        assert_eq!(
            computer_control_capture_screen(),
            observation("computer_control_capture_screen")
        );
        match tauri::async_runtime::block_on(
            crate::nx_bridge::commands::nx_computer_use_screenshot(),
        ) {
            Ok(shot) => panic!("the nx handler returned a screenshot: {shot:?}"),
            Err(error) => assert_eq!(
                error,
                closed("nx_computer_use_screenshot", Closure::ScreenObservation)
            ),
        }
        assert_eq!(
            super::computer_control_toggle(state, true),
            observation("computer_control_toggle")
        );
        assert_eq!(
            computer_control_execute_action(),
            Err(closed("computer_control_execute_action", Closure::OsInput))
        );
        assert_eq!(
            start_computer_action(),
            Err(closed("start_computer_action", Closure::OsInput))
        );
    };
    let engine = |state: &AppState| {
        let engine = state
            .computer_control
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        (engine.is_enabled(), engine.total_actions())
    };

    // A disabled engine stays disabled, and nothing is recorded.
    let state = AppState::new_in_memory();
    assert_eq!(engine(&state), (false, 0));
    denied(&state);
    assert_eq!(engine(&state), (false, 0));
    assert_eq!(
        super::computer_control_get_history(&state),
        Ok("[]".to_string())
    );
    let status: serde_json::Value = serde_json::from_str(
        &super::computer_control_status(&state).expect("status stays readable"),
    )
    .expect("status json");
    assert_eq!(status["enabled"], json!(false));
    assert!(
        !get_input_control_status(&state)
            .expect("input status stays readable")
            .enabled
    );

    // Denial does not depend on the engine state: an engine enabled by other
    // means is not a capture grant, and the requests leave it unchanged.
    state
        .computer_control
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .enable();
    denied(&state);
    assert_eq!(engine(&state), (true, 0));
    // Disabling stays available.
    assert_eq!(
        super::computer_control_toggle(&state, false),
        Ok(json!({ "enabled": false }).to_string())
    );
    assert_eq!(engine(&state), (false, 0));

    // After the emergency stop the requests stay denied. They neither clear
    // nor bypass it. Stop and disable stay available during it.
    activate_emergency_kill_switch();
    denied(&state);
    assert!(emergency_kill_switch_active());
    assert_eq!(engine(&state), (false, 0));
    assert!(
        get_input_control_status(&state)
            .expect("input status stays readable")
            .kill_switch_active
    );
    stop_computer_action(&state, "p0-002c5c-session".to_string()).expect("stop stays available");
    assert_eq!(
        super::computer_control_toggle(&state, false),
        Ok(json!({ "enabled": false }).to_string())
    );
    assert!(emergency_kill_switch_active());
    assert_eq!(engine(&state), (false, 0));
    // Test cleanup only: no desktop route can clear the stop.
    reset_emergency_kill_switch();

    // No capture, analysis or enable was audited; only the disables were.
    let audit = state.audit.lock().unwrap_or_else(|p| p.into_inner());
    let control: Vec<&str> = audit
        .events()
        .iter()
        .filter(|event| event.payload["source"] == "computer-control")
        .filter_map(|event| event.payload["action"].as_str())
        .collect();
    assert_eq!(
        control,
        ["disable", "stop_computer_action", "disable"],
        "{control:?}"
    );
}
