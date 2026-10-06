//! Candidate 9 (C8-3): an agent the supervisor holds no record of has no
//! authority to run. Its loop runs no cycle and it takes no HiveMind
//! sub-task, as a stopped agent; the scheduler's tick and Phase Three's
//! bridge already refuse it.

use super::*;

fn register(state: &AppState, name: &str) -> String {
    let manifest = crate::commands::chat_llm::parse_agent_manifest_json(
        &json!({
            "name": name,
            "version": "1.0.0",
            "capabilities": ["llm.query"],
            "fuel_budget": 1000,
            "autonomy_level": 2,
        })
        .to_string(),
    )
    .unwrap();
    state
        .supervisor
        .lock()
        .unwrap()
        .start_agent(manifest)
        .unwrap()
        .to_string()
}

/// Wait (bounded) until the task of `goal` is recorded as ended.
fn ended_task(state: &AppState, agent: &str, goal: &str) -> nexus_persistence::TaskRow {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let ended = state
            .db
            .load_tasks_by_agent(agent, 10)
            .unwrap()
            .into_iter()
            .find(|task| task.id == goal && task.completed_at.is_some());
        if let Some(task) = ended {
            return task;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "the loop of an agent with no authority to run did not end"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

#[test]
fn an_agent_the_supervisor_does_not_hold_has_no_authority_to_run() {
    let state = AppState::new_in_memory();
    // Not an agent id, and an agent never registered: stopped.
    for missing in ["not-an-agent".to_string(), Uuid::new_v4().to_string()] {
        assert!(
            agent_stopped(&state, &missing),
            "a missing agent has authority to run: {missing}"
        );
    }
    // A registered agent runs while it runs (or is paused: pausing keeps its
    // loop, which Phase Three's bridge refuses on its own).
    let id = register(&state, "authority-to-run");
    let uuid = Uuid::parse_str(&id).unwrap();
    assert!(!agent_stopped(&state, &id));
    assert!(!agent_stopped(&state, &id.to_uppercase()));
    state.supervisor.lock().unwrap().pause_agent(uuid).unwrap();
    assert!(!agent_stopped(&state, &id));
    state.supervisor.lock().unwrap().resume_agent(uuid).unwrap();
    // Once its record is gone (cleared), it is stopped however its id is
    // written.
    state.supervisor.lock().unwrap().clear_all_agents();
    for spelling in [id.clone(), id.to_uppercase()] {
        assert!(
            agent_stopped(&state, &spelling),
            "a missing agent has authority to run: {spelling}"
        );
    }
}

/// The failure path of C8-3: an agent's record is cleared while its goal is
/// assigned; the loop started for it runs no cycle, and it takes no
/// sub-task.
#[test]
fn a_cleared_agents_loop_runs_no_cycle_and_it_takes_no_subtask() {
    let state = AppState::new_in_memory();
    let id = register(&state, "cleared-loop");
    let goal =
        execute_agent_goal(&state, id.clone(), "summarize the notes".into(), 5, None).unwrap();
    state.supervisor.lock().unwrap().clear_all_agents();
    spawn_cognitive_loop_with_bridge(
        BackendEventBridge::default(),
        state.clone(),
        id.clone(),
        goal.clone(),
    );
    let task = ended_task(&state, &id, &goal);
    assert!(!task.success);
    assert!(
        task.result_json
            .as_deref()
            .is_some_and(|json| json.contains("the agent is stopped")),
        "the loop of an agent with no authority to run ran a cycle: {:?}",
        task.result_json
    );
    let refused = execute_hivemind_subtask(&state, &id, "summarize the notes");
    assert!(
        refused.as_ref().is_err_and(|e| e.contains("is stopped")),
        "{refused:?}"
    );
}
