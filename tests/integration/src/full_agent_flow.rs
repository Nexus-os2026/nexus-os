use nexus_desktop_backend::{
    approve_consent_request, create_agent, execute_agent_goal, persist_task_completion,
    start_agent, AppState, DbMemoryStore,
};
use nexus_kernel::cognitive::{AgentMemoryManager, CognitivePhase, CognitivePlanner, PlannerLlm};
use nexus_persistence::{ConsentRow, StateStore};
use std::sync::{mpsc, OnceLock};
use std::time::{Duration, Instant};
use tempfile::TempDir;

static HOME_ENV_GUARD: OnceLock<std::sync::Mutex<()>> = OnceLock::new();

struct HomeVarGuard {
    original: Option<std::ffi::OsString>,
}

impl HomeVarGuard {
    fn set(path: &std::path::Path) -> Self {
        let original = std::env::var_os("HOME");
        std::env::set_var("HOME", path);
        Self { original }
    }
}

impl Drop for HomeVarGuard {
    fn drop(&mut self) {
        match self.original.take() {
            Some(value) => std::env::set_var("HOME", value),
            None => std::env::remove_var("HOME"),
        }
    }
}

struct FixedPlannerLlm;

impl PlannerLlm for FixedPlannerLlm {
    fn plan_query(&self, _prompt: &str) -> Result<String, nexus_kernel::errors::AgentError> {
        Ok(
            r#"[{"action":{"type":"FileWrite","path":"test.txt","content":"Hello World"},"description":"Create the requested file"}]"#
                .to_string(),
        )
    }
}

async fn run_headless_goal_loop(
    state: AppState,
    agent_id: String,
    goal_id: String,
) -> Result<(), String> {
    let planner = CognitivePlanner::new(Box::new(FixedPlannerLlm));
    let mem_store = DbMemoryStore {
        db: state.db.clone(),
    };
    let memory_mgr = AgentMemoryManager::new(Box::new(mem_store));

    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let workspace_base = std::path::PathBuf::from(&home)
        .join(".nexus")
        .join("agents");
    let executor = nexus_kernel::cognitive::RegistryExecutor::new(
        workspace_base,
        state.audit.clone(),
        state.supervisor.clone(),
        None,
    );

    for _ in 0..50_u32 {
        let cycle_result = match {
            let mut audit_guard = state.audit.lock().unwrap_or_else(|p| p.into_inner());
            state.cognitive_runtime.run_cycle_with_evolution(
                &agent_id,
                &planner,
                &memory_mgr,
                &executor,
                &mut audit_guard,
                Some(&state.evolution_tracker),
            )
        } {
            Ok(result) => result,
            Err(error) => {
                let result_summary = format!("cognitive cycle error for {goal_id}: {error}");
                persist_task_completion(
                    &state,
                    &agent_id,
                    &goal_id,
                    "failed",
                    &result_summary,
                    false,
                    0.0,
                );
                return Err(result_summary);
            }
        };
        if cycle_result.phase == CognitivePhase::Blocked {
            let action_desc = cycle_result
                .blocked_reason
                .clone()
                .unwrap_or_else(|| "perform a governed action".to_string());
            let step_info = state
                .cognitive_runtime
                .get_agent_status(&agent_id)
                .map(|status| {
                    serde_json::json!({
                        "summary": action_desc,
                        "goal": status.active_goal.as_ref().map(|goal| goal.description.clone()),
                        "phase": format!("{}", status.phase),
                        "fuel_cost": 5.0,
                        "side_effects": [action_desc.clone()],
                    })
                })
                .unwrap_or_else(|| serde_json::json!({ "summary": action_desc }));
            let consent_id = uuid::Uuid::new_v4().to_string();
            let notify = state.register_blocked_consent_wait(&agent_id, &consent_id);
            let consent_row = ConsentRow {
                id: consent_id.clone(),
                agent_id: agent_id.clone(),
                operation_type: "cognitive.hitl_approval".to_string(),
                operation_json: serde_json::to_string(&step_info).unwrap_or_default(),
                hitl_tier: "Tier1".to_string(),
                status: "pending".to_string(),
                created_at: chrono::Utc::now().to_rfc3339(),
                resolved_at: None,
                resolved_by: None,
            };
            state
                .db
                .enqueue_consent(&consent_row)
                .map_err(|e| format!("enqueue consent failed: {e}"))?;

            notify.notified().await;
            state.clear_blocked_consent_wait(&agent_id, &consent_id);

            if !state.cognitive_runtime.has_active_loop(&agent_id) {
                return Err(format!("cognitive loop for {goal_id} stopped unexpectedly"));
            }
            continue;
        }

        if !cycle_result.should_continue {
            if cycle_result.phase == CognitivePhase::Learn {
                let result_summary = state
                    .cognitive_runtime
                    .get_agent_status(&agent_id)
                    .and_then(|status| status.active_goal)
                    .map(|goal| {
                        format!(
                            "Completed: {} ({} steps, {:.1} fuel used)",
                            goal.description,
                            cycle_result.steps_executed,
                            cycle_result.fuel_consumed
                        )
                    })
                    .unwrap_or_else(|| "Goal completed successfully.".to_string());
                persist_task_completion(
                    &state,
                    &agent_id,
                    &goal_id,
                    "completed",
                    &result_summary,
                    true,
                    cycle_result.fuel_consumed,
                );
                return Ok(());
            }
            let result_summary = cycle_result
                .blocked_reason
                .clone()
                .unwrap_or_else(|| format!("goal stopped in {}", cycle_result.phase));
            persist_task_completion(
                &state,
                &agent_id,
                &goal_id,
                "failed",
                &result_summary,
                false,
                cycle_result.fuel_consumed,
            );
            return Err(result_summary);
        }

        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    let result_summary = format!("goal {goal_id} did not finish before the cycle limit");
    persist_task_completion(
        &state,
        &agent_id,
        &goal_id,
        "failed",
        &result_summary,
        false,
        0.0,
    );
    Err(result_summary)
}

fn wait_for_pending_consent(state: &AppState, agent_id: &str) -> ConsentRow {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let pending = state.db.load_pending_consent().unwrap();
        if let Some(row) = pending.into_iter().find(|row| row.agent_id == agent_id) {
            return row;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for a pending consent request for {agent_id}"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn agent_workspace_file(home: &TempDir, agent_id: &str, relative_path: &str) -> std::path::PathBuf {
    home.path()
        .join(".nexus")
        .join("agents")
        .join(agent_id)
        .join("workspace")
        .join(relative_path)
}

#[test]
fn test_full_agent_flow() {
    let _home_guard = HOME_ENV_GUARD
        .get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .expect("HOME env guard lock");
    let home = tempfile::tempdir().expect("temp home");
    let _home_var = HomeVarGuard::set(home.path());

    let state = AppState::new_in_memory();
    let manifest_json = serde_json::json!({
        "name": "TestBot",
        "version": "1.0.0",
        "capabilities": ["fs.read", "fs.write"],
        "autonomy_level": 2,
        "fuel_budget": 1000,
        "schedule": null,
        "default_goal": null,
        "llm_model": null
    })
    .to_string();

    let agent_id = create_agent(&state, manifest_json).expect("create_agent should succeed");
    start_agent(&state, agent_id.clone()).expect("start_agent should succeed");
    let goal_id = execute_agent_goal(
        &state,
        agent_id.clone(),
        "Create a file called test.txt with content Hello World".to_string(),
        5,
    )
    .expect("execute_agent_goal should assign the goal");
    let (tx, rx) = mpsc::channel();
    let loop_state = state.clone();
    let loop_agent_id = agent_id.clone();
    let loop_goal_id = goal_id.clone();
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("tokio runtime");
        let result = runtime.block_on(run_headless_goal_loop(
            loop_state,
            loop_agent_id,
            loop_goal_id,
        ));
        let _ = tx.send(result);
    });

    let consent_row = wait_for_pending_consent(&state, &agent_id);
    let blocked_status = state
        .cognitive_runtime
        .get_agent_status(&agent_id)
        .expect("cognitive status should exist");
    assert_eq!(blocked_status.phase, CognitivePhase::Blocked);

    approve_consent_request(
        &state,
        consent_row.id.clone(),
        "integration-test".to_string(),
    )
    .expect("approve_consent_request should succeed");

    let loop_result = match rx.recv_timeout(Duration::from_secs(20)) {
        Ok(result) => result,
        Err(error) => {
            let status = state.cognitive_runtime.get_agent_status(&agent_id);
            let pending = state.db.load_pending_consent().unwrap_or_default();
            let tasks = state
                .db
                .load_tasks_by_agent(&agent_id, 10)
                .unwrap_or_default();
            panic!(
                "cognitive loop should resume and finish within 10 seconds: {error:?}; status={status:?}; pending_consents={pending:?}; tasks={tasks:?}"
            );
        }
    };
    loop_result.expect("goal should complete successfully after approval");

    let tasks = state.db.load_tasks_by_agent(&agent_id, 10).unwrap();
    let completed_task = tasks
        .iter()
        .find(|task| task.id == goal_id)
        .expect("task history should contain the executed goal");
    assert_eq!(completed_task.status, "completed");
    assert!(completed_task.success);

    let workspace_file = agent_workspace_file(&home, &agent_id, "test.txt");
    assert!(
        workspace_file.exists(),
        "expected {:?} to exist",
        workspace_file
    );
    let file_content = std::fs::read_to_string(&workspace_file).expect("read workspace file");
    assert_eq!(file_content, "Hello World");

    let audit_events = state.db.load_audit_events(None, 200, 0).unwrap();
    assert!(
        audit_events
            .iter()
            .any(|event| event.detail_json.contains("\"event\":\"create_agent\"")),
        "expected create_agent audit event"
    );
    assert!(
        audit_events.iter().any(|event| event
            .detail_json
            .contains("\"action\":\"assign_agent_goal\"")),
        "expected assign_agent_goal audit event"
    );
    assert!(
        audit_events.iter().any(|event| event
            .detail_json
            .contains("\"action\":\"consent_approved\"")),
        "expected consent_approved audit event"
    );
}
