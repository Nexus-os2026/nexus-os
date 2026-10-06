//! Candidate 9 (C8-5): HiveMind sessions are bounded. At most
//! `hive::MAX_SESSIONS` run at once, admitted under the agent-execution rate
//! limit before any thread starts; each has its own identity and
//! cancellation; ending, however it ends, gives its place back; the owner's
//! cancellation, the emergency stop and quitting signal them; a cancelled
//! session assigns no further sub-task.

use super::*;
use nexus_kernel::cognitive::{HivemindCoordinator, HivemindLlm};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Barrier;

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

#[test]
fn at_most_the_cap_runs_at_once_and_an_ended_session_gives_its_place_back() {
    let state = AppState::new_in_memory();
    let sessions = state.hive_sessions.clone();
    let mut admitted: Vec<_> = (0..hive::MAX_SESSIONS)
        .map(|_| sessions.admit().unwrap())
        .collect();
    assert_eq!(sessions.live(), hive::MAX_SESSIONS);
    let refused = sessions.admit();
    assert!(
        refused.is_err(),
        "a session started beyond the cap on sessions at once"
    );
    // Each has its own identity.
    assert_ne!(admitted[0].id(), admitted[1].id());
    // A session that ends gives its place back.
    admitted.pop();
    assert_eq!(sessions.live(), hive::MAX_SESSIONS - 1);
    admitted.push(sessions.admit().unwrap());
    // So does one whose thread panics.
    let session = admitted.pop().unwrap();
    let panicked = std::thread::spawn(move || {
        let _session = session;
        panic!("a session's thread panics (test)");
    })
    .join();
    assert!(panicked.is_err());
    assert_eq!(
        sessions.live(),
        hive::MAX_SESSIONS - 1,
        "a panicked session kept its place"
    );
    drop(admitted);
    assert_eq!(sessions.live(), 0);
}

/// Places are checked and taken at once: starts racing for them never run
/// more sessions than the cap.
#[test]
fn racing_starts_never_exceed_the_cap() {
    let state = AppState::new_in_memory();
    let sessions = state.hive_sessions.clone();
    let barrier = Arc::new(Barrier::new(32));
    let admitted = Arc::new(AtomicUsize::new(0));
    let threads: Vec<_> = (0..32)
        .map(|_| {
            let (sessions, barrier, admitted) =
                (sessions.clone(), barrier.clone(), admitted.clone());
            std::thread::spawn(move || {
                barrier.wait();
                let session = sessions.admit();
                if session.is_ok() {
                    admitted.fetch_add(1, Ordering::SeqCst);
                }
                // Every start has been decided before any place is given back.
                barrier.wait();
                drop(session);
            })
        })
        .collect();
    for thread in threads {
        thread.join().unwrap();
    }
    assert_eq!(
        admitted.load(Ordering::SeqCst),
        hive::MAX_SESSIONS,
        "racing starts ran more sessions than the cap"
    );
    assert_eq!(sessions.live(), 0);
}

/// A start is refused by the agent-execution rate limit, and then holds no
/// place.
#[test]
fn a_start_is_rate_limited_and_a_refused_one_holds_no_place() {
    let mut state = AppState::new_in_memory();
    // The desktop's limiter, as configured by default (the in-memory state
    // runs with none).
    state.rate_limiter = nexus_kernel::rate_limit::NexusRateLimiter::from_config(
        &nexus_kernel::rate_limit::RateLimitConfig::default(),
    );
    let mut spent = 0;
    while state
        .check_rate(nexus_kernel::rate_limit::RateCategory::AgentExecute)
        .is_ok()
    {
        spent += 1;
        assert!(spent < 1000, "the agent-execution rate limit never refused");
    }
    let refused = admit_hivemind(&state);
    assert!(
        refused.is_err(),
        "a HiveMind start passed the exhausted agent-execution rate limit"
    );
    assert_eq!(
        state.hive_sessions.live(),
        0,
        "a refused start kept a place"
    );
}

/// The owner cancels a session by the identity its admission recorded; the
/// emergency stop signals every session; quitting signals every one and
/// refuses new ones.
#[test]
fn the_owner_the_emergency_stop_and_quitting_signal_sessions() {
    let state = AppState::new_in_memory();
    let first = admit_hivemind(&state).unwrap();
    let second = admit_hivemind(&state).unwrap();
    let recorded = state.audit.lock().unwrap().events().iter().any(|event| {
        event.payload["action"] == "hivemind_session_admitted"
            && event.payload["session"] == first.id()
    });
    assert!(recorded, "a session's identity was not recorded");
    let cancelled = cancel_hivemind(&state, first.id().to_string());
    assert!(
        cancelled.is_ok() && first.cancelled(),
        "the owner's cancellation did not reach it: {cancelled:?}"
    );
    assert!(!second.cancelled());
    // An identity that names no session under way: the stored sessions'
    // cancellation, which knows none.
    assert!(cancel_hivemind(&state, Uuid::new_v4().to_string()).is_err());
    assert_eq!(state.hive_sessions.cancel_all(), 2);
    assert!(second.cancelled(), "the emergency stop did not reach it");
    drop((first, second));
    let third = admit_hivemind(&state).unwrap();
    assert_eq!(state.hive_sessions.close(), 1);
    assert!(third.cancelled(), "quitting did not reach it");
    assert!(
        state.hive_sessions.admit().is_err(),
        "a session started while the desktop quits"
    );
}

/// The planner the coordinator asks to decompose a goal: three sub-tasks
/// any agent can take; it can also cancel the session as it plans.
struct Planner {
    cancel: Option<(Arc<hive::HiveSessions>, String)>,
}

impl HivemindLlm for Planner {
    fn decompose(&self, _: &str) -> Result<String, nexus_kernel::errors::AgentError> {
        if let Some((sessions, id)) = &self.cancel {
            sessions.cancel(id);
        }
        Ok(json!([
            {"id": "a", "description": "first part", "required_capabilities": ["llm.query"]},
            {"id": "b", "description": "second part", "required_capabilities": ["llm.query"]},
            {"id": "c", "description": "third part", "required_capabilities": ["llm.query"]},
        ])
        .to_string())
    }
    fn merge(&self, _: &str) -> Result<String, nexus_kernel::errors::AgentError> {
        Ok("merged".to_string())
    }
}

/// A session the owner cancels (here while it plans) assigns no sub-task to
/// any agent: no goal, no loop, no task.
#[test]
fn a_cancelled_session_assigns_no_further_subtask() {
    let mut state = AppState::new_in_memory();
    let agents = [
        register(&state, "hive-first"),
        register(&state, "hive-second"),
    ];
    let session = admit_hivemind(&state).unwrap();
    state.hivemind = Arc::new(HivemindCoordinator::new(
        Box::new(Planner {
            cancel: Some((state.hive_sessions.clone(), session.id().to_string())),
        }),
        Arc::new(nexus_kernel::cognitive::hivemind::NoOpHivemindEmitter),
        Arc::new(Mutex::new(AuditTrail::new())),
    ));
    let outcome = start_hivemind(&state, &session, "the goal".into(), agents.to_vec()).unwrap();
    assert_eq!(outcome["status"], "Failed", "{outcome}");
    for agent in &agents {
        assert!(
            state.db.load_tasks_by_agent(agent, 10).unwrap().is_empty(),
            "a cancelled session assigned a sub-task to {agent}"
        );
        assert!(!state.cognitive_runtime.has_active_loop(agent));
    }
    // And a sub-task asked of it directly is refused before anything.
    let refused = execute_hivemind_subtask(&state, &session, &agents[0], "one more");
    assert_eq!(refused, Err(hive::SESSION_CANCELLED.to_string()));
    assert!(state
        .db
        .load_tasks_by_agent(&agents[0], 10)
        .unwrap()
        .is_empty());
}

/// A sub-task the session waits on ends with the session's cancellation,
/// and only its own goal ends: one the agent was given since is left.
#[test]
fn a_cancelled_sessions_waiting_subtask_ends_only_its_own_goal() {
    let state = AppState::new_in_memory();
    let agent = register(&state, "hive-waiting");
    let session = admit_hivemind(&state).unwrap();
    // The sub-task's goal (its loop not driven here), then cancelled.
    let goal = execute_agent_goal(&state, agent.clone(), "part".into(), 5, None).unwrap();
    assert!(state.hive_sessions.cancel(session.id()));
    let ended = await_subtask(
        &state,
        &session,
        &agent,
        &goal,
        "part",
        std::time::Duration::from_secs(30),
    );
    assert_eq!(ended, Err(hive::SESSION_CANCELLED.to_string()));
    assert!(
        !state.cognitive_runtime.has_active_loop(&agent),
        "a cancelled session's sub-task kept its loop"
    );
    // A goal given to the agent since is not the session's to end.
    let newer = execute_agent_goal(&state, agent.clone(), "the owner's".into(), 5, None).unwrap();
    let ended = await_subtask(
        &state,
        &session,
        &agent,
        &goal,
        "part",
        std::time::Duration::from_secs(30),
    );
    assert_eq!(ended, Err(hive::SESSION_CANCELLED.to_string()));
    assert_eq!(
        state
            .cognitive_runtime
            .get_agent_status_fast(&agent)
            .and_then(|status| status.active_goal)
            .map(|goal| goal.id),
        Some(newer),
        "a cancelled session ended a goal that was not its own"
    );
}
