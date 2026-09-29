//! P0-FINAL-GATE (item G): a scheduled tick for a transcendent (L6) agent is
//! refused before anything is audited, restarted or assigned.

use super::*;
use nexus_kernel::cognitive::ScheduledGoalExecutor as _;
use std::sync::atomic::AtomicBool;

fn manifest(name: &str, autonomy_level: u8) -> String {
    json!({
        "name": name,
        "version": "1.0.0",
        "capabilities": ["llm.query"],
        "fuel_budget": 1000,
        "autonomy_level": autonomy_level,
    })
    .to_string()
}

fn register_stopped(state: &AppState, name: &str, autonomy_level: u8) -> String {
    let manifest =
        crate::commands::chat_llm::parse_agent_manifest_json(&manifest(name, autonomy_level))
            .unwrap();
    let mut supervisor = state.supervisor.lock().unwrap();
    let id = supervisor.start_agent(manifest).unwrap();
    supervisor.stop_agent(id).unwrap();
    id.to_string()
}

fn snapshot(state: &AppState, id: &str) -> Option<(AgentState, u64)> {
    let supervisor = state.supervisor.lock().unwrap();
    supervisor
        .get_agent(Uuid::parse_str(id).unwrap())
        .map(|handle| (handle.state, handle.remaining_fuel))
}

#[test]
fn p0_fg_scheduled_ticks_refuse_a_transcendent_agent_before_any_state_change() {
    let state = AppState::new_in_memory();
    // Registration schedules nothing here; the scheduler's tasks are not polled.
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let _entered = runtime.enter();
    let executor = ScheduledGoalExecutor {
        state: state.clone(),
    };
    let refused = Err(crate::phase0_surface::closed(
        "assign_agent_goal",
        crate::phase0_surface::Closure::ApprovalRequired,
    ));
    let audit_events = |state: &AppState| state.audit.lock().unwrap().events().len();

    // A registered, stopped L6 agent: no restart, no fuel spent, no audit.
    let registered = register_stopped(&state, "transcendent-scheduled", 6);
    let before = snapshot(&state, &registered);
    assert_eq!(before.map(|(s, _)| s), Some(AgentState::Stopped));
    let events = audit_events(&state);
    assert_eq!(
        executor.execute(&registered, "p0fg-scheduled-goal"),
        refused
    );
    assert_eq!(snapshot(&state, &registered), before);
    assert_eq!(audit_events(&state), events);
    assert!(!state.cognitive_runtime.has_active_loop(&registered));

    // A running loop entry would have produced a skip audit; the refusal
    // comes first.
    state
        .cognitive_cancellations
        .lock()
        .unwrap()
        .insert(registered.clone(), Arc::new(AtomicBool::new(false)));
    assert_eq!(
        executor.execute(&registered, "p0fg-scheduled-goal"),
        refused
    );
    assert_eq!(audit_events(&state), events);
    state.cognitive_cancellations.lock().unwrap().clear();

    // A stored-only L6 record is refused the same way.
    let stored = Uuid::new_v4().to_string();
    state
        .db
        .save_agent(
            &stored,
            &manifest("transcendent-scheduled-stored", 6),
            "running",
            6,
            "native",
        )
        .unwrap();
    assert_eq!(executor.execute(&stored, "p0fg-scheduled-goal"), refused);
    assert_eq!(audit_events(&state), events);

    // Control: an L5 agent passes the check and is restarted as before.
    let sovereign = register_stopped(&state, "sovereign-scheduled", 5);
    assert_ne!(executor.execute(&sovereign, "p0fg-scheduled-goal"), refused);
    assert_ne!(
        snapshot(&state, &sovereign).map(|(s, _)| s),
        Some(AgentState::Stopped)
    );
}
