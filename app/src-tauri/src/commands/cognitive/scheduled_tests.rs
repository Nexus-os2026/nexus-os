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

/// P0-FINAL-GATE (items G and K, composed): stream 5 made `create_agent` and
/// `start_agent` refuse a manifest schedule the scheduler rejects, and stream
/// 6 bounded agent schedules to at most once per minute. Composed, a
/// sub-minute schedule is refused at both with the scheduler's bounded
/// reason, and nothing is saved, registered, scheduled or audited.
#[test]
fn p0_fg_sub_minute_manifest_schedules_fail_create_and_start() {
    let state = AppState::new_in_memory();
    // Registration may start a scheduler task; nothing here polls it.
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let _entered = runtime.enter();
    let scheduled = |name: &str, schedule: Option<&str>| {
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
    let snapshot = |state: &AppState| {
        (
            state.db.list_agents().unwrap().len(),
            state.supervisor.lock().unwrap().health_check().len(),
            state.meta.lock().unwrap().len(),
            state.agent_scheduler.list().len(),
            state.audit.lock().unwrap().events().len(),
        )
    };
    const ONCE_PER_MINUTE: &str = "invalid cron expression: an agent schedule fires at most once per minute, so its seconds field must be one value from 0 to 59";
    let sub_minute = "*/30 * * * * *";

    let before = snapshot(&state);
    assert_eq!(
        crate::create_agent(&state, scheduled("sub-minute-created", Some(sub_minute))),
        Err(ONCE_PER_MINUTE.to_string())
    );
    assert_eq!(snapshot(&state), before);

    let stored = crate::create_agent(&state, scheduled("sub-minute-stored", None)).unwrap();
    crate::stop_agent(&state, stored.clone()).unwrap();
    state
        .db
        .save_agent(
            &stored,
            &scheduled("sub-minute-stored", Some(sub_minute)),
            "stopped",
            0,
            "native",
        )
        .unwrap();
    let before = snapshot(&state);
    assert_eq!(
        crate::start_agent(&state, stored.clone()),
        Err(ONCE_PER_MINUTE.to_string())
    );
    assert_eq!(snapshot(&state), before);
    assert_eq!(
        state
            .supervisor
            .lock()
            .unwrap()
            .get_agent(Uuid::parse_str(&stored).unwrap())
            .map(|handle| handle.state),
        Some(AgentState::Stopped)
    );
}
