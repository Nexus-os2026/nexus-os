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

/// P0-FINAL-GATE (item K, composed): `start_autonomous_loop` accepts only
/// intervals the bounded scheduler can express (60 to 3599 seconds, whole
/// minutes) and refuses any other with a bounded reason, registering nothing.
#[test]
fn p0_fg_autonomous_loop_intervals_outside_the_schedule_bound_are_refused() {
    let state = AppState::new_in_memory();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let _entered = runtime.enter();
    let agent = Uuid::new_v4().to_string();
    for interval in [0, 1, 30, 59, 3600, 86_400, u64::MAX] {
        assert_eq!(
            super::start_autonomous_loop(&state, agent.clone(), Some(interval), None),
            Err(super::AUTONOMOUS_LOOP_INTERVAL.to_string()),
            "{interval}"
        );
        assert!(state.agent_scheduler.list().is_empty(), "{interval}");
    }
    for interval in [60, 90, 3599] {
        super::start_autonomous_loop(&state, agent.clone(), Some(interval), None).unwrap();
        assert_eq!(state.agent_scheduler.list().len(), 1, "{interval}");
        state.agent_scheduler.unregister_agent(&agent);
    }
}

/// Re-audit of candidate 5 (V4): a scheduled tick already under way when the
/// owner stops the agent does not bring it back. The owner's stop removes the
/// schedule, then stops the agent under the supervisor's lock; a tick that saw
/// its schedule as it began and finds it gone gives up before any restart.
#[test]
fn p3_a_tick_overtaken_by_an_owners_stop_does_not_restart_the_agent() {
    let state = AppState::new_in_memory();
    // Registration schedules nothing here; the scheduler's tasks are not polled.
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let _entered = runtime.enter();
    let manifest =
        crate::commands::chat_llm::parse_agent_manifest_json(&manifest("overtaken", 2)).unwrap();
    let id = state
        .supervisor
        .lock()
        .unwrap()
        .start_agent(manifest)
        .unwrap()
        .to_string();
    super::start_autonomous_loop(&state, id.clone(), Some(60), None).unwrap();
    // The tick begins while the owner's stop holds the supervisor.
    let mut supervisor = state.supervisor.lock().unwrap();
    let tick = {
        let executor = ScheduledGoalExecutor {
            state: state.clone(),
        };
        let id = id.clone();
        std::thread::spawn(move || executor.execute(&id, "p3-scheduled-goal"))
    };
    std::thread::sleep(std::time::Duration::from_millis(500));
    state.agent_scheduler.unregister_agent(&id);
    supervisor
        .stop_agent(Uuid::parse_str(&id).unwrap())
        .unwrap();
    drop(supervisor);
    assert_eq!(
        tick.join().unwrap(),
        Err("scheduled run skipped: the agent's schedule was removed".to_string())
    );
    assert_eq!(
        snapshot(&state, &id).map(|(s, _)| s),
        Some(AgentState::Stopped)
    );
}

/// Verification of candidate 6 (G10-A N2, path A): a tick that wakes just
/// after the owner's stop removed its schedule (so it never saw it) does not
/// restart the agent: Phase Three's record of the owner's stop says so.
#[cfg(target_os = "linux")]
#[test]
fn p3_a_tick_that_never_saw_its_schedule_does_not_revive_an_owner_stopped_agent() {
    let root = std::env::temp_dir().join(format!("nexus-p3-tick-{}", Uuid::new_v4()));
    let mut state = AppState::new_in_memory();
    state.real_world = Ok(crate::governed_real_world::RealWorld::for_tests(
        &root,
        Arc::new(Mutex::new(nexus_kernel::audit::AuditTrail::new())),
        Arc::new(nexus_persistence::NexusDatabase::in_memory().unwrap()),
    ));
    // Registration schedules nothing here; the scheduler's tasks are not polled.
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let _entered = runtime.enter();
    let manifest =
        crate::commands::chat_llm::parse_agent_manifest_json(&manifest("owner-stopped", 2))
            .unwrap();
    let id = state
        .supervisor
        .lock()
        .unwrap()
        .start_agent(manifest)
        .unwrap()
        .to_string();
    super::start_autonomous_loop(&state, id.clone(), Some(60), None).unwrap();
    // The owner stops the agent: its schedule is removed, Phase Three told.
    let stopped = crate::commands::agents::stop_agents(&state, std::slice::from_ref(&id));
    assert!(stopped.iter().all(Result::is_ok), "{stopped:?}");
    // A tick that had already fired runs now, without its schedule.
    let executor = ScheduledGoalExecutor {
        state: state.clone(),
    };
    assert_eq!(
        executor.execute(&id, "p3-scheduled-goal"),
        Err("scheduled run skipped: the agent's schedule was removed".to_string())
    );
    assert_eq!(
        snapshot(&state, &id).map(|(s, _)| s),
        Some(AgentState::Stopped)
    );
    let _ = std::fs::remove_dir_all(&root);
}
