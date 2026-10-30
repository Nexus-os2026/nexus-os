//! Candidate 9 (C8-5): HiveMind sessions are bounded. At most
//! `hive::MAX_SESSIONS` run at once, admitted under the agent-execution rate
//! limit before any thread starts; each has its own identity and
//! cancellation; ending, however it ends, gives its place back; the owner's
//! cancellation, the emergency stop and quitting signal them; a cancelled
//! session assigns no further sub-task.

use super::*;
use nexus_kernel::cognitive::HivemindLlm;
use nexus_kernel::hm::{DerivedItem, DispatchContext, PromptSource};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Barrier;

fn manifest(name: &str) -> AgentManifest {
    crate::commands::chat_llm::parse_agent_manifest_json(
        &json!({
            "name": name,
            "version": "1.0.0",
            "capabilities": ["llm.query"],
            "fuel_budget": 1000,
            "autonomy_level": 2,
        })
        .to_string(),
    )
    .unwrap()
}

fn register(state: &AppState, name: &str) -> String {
    state
        .supervisor
        .lock()
        .unwrap()
        .start_agent(manifest(name))
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
    fn decompose(
        &self,
        ctx: &mut DispatchContext,
        _: &mut PromptSource<'_>,
    ) -> Result<DerivedItem, nexus_kernel::errors::AgentError> {
        if let Some((sessions, id)) = &self.cancel {
            sessions.cancel(id);
        }
        Ok(ctx.produce(
            json!([
                {"id": "a", "description": "first part", "required_capabilities": ["llm.query"]},
                {"id": "b", "description": "second part", "required_capabilities": ["llm.query"]},
                {"id": "c", "description": "third part", "required_capabilities": ["llm.query"]},
            ])
            .to_string(),
        ))
    }
    fn merge(
        &self,
        ctx: &mut DispatchContext,
        _: &mut PromptSource<'_>,
    ) -> Result<DerivedItem, nexus_kernel::errors::AgentError> {
        Ok(ctx.produce("merged"))
    }
}

/// A session the owner cancels (here while it plans) assigns no sub-task to
/// any agent: no goal, no loop, no task.
#[test]
fn a_cancelled_session_assigns_no_further_subtask() {
    let state = AppState::new_in_memory();
    let agents = [
        register(&state, "hive-first"),
        register(&state, "hive-second"),
    ];
    let session = admit_hivemind(&state).unwrap();
    let cancel = Some((state.hive_sessions.clone(), session.id().to_string()));
    let outcome = start_hivemind_with(&state, &session, "the goal".into(), agents.to_vec(), |_| {
        Box::new(Planner { cancel })
    })
    .unwrap();
    assert_eq!(outcome["status"], "Failed", "{outcome}");
    for agent in &agents {
        assert!(
            state.db.load_tasks_by_agent(agent, 10).unwrap().is_empty(),
            "a cancelled session assigned a sub-task to {agent}"
        );
        assert!(!state.cognitive_runtime.has_active_loop(agent));
    }
    // And a sub-task asked of it directly is refused before anything.
    let (admission, session_ctx) = hivemind_test_session(&state, &agents);
    let refused = execute_hivemind_subtask(
        &state,
        &session,
        &admission,
        &session_ctx,
        1,
        &agents[0],
        "one more",
    );
    assert!(refused.context.is_none());
    assert_eq!(
        refused.outcome.err(),
        Some(hive::SESSION_CANCELLED.to_string())
    );
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

/// T173b: a session timeout ends its own goal, never a newer goal or the
/// agent's governed-control run. No loop driver, timer sleep or LLM is needed.
#[cfg(target_os = "linux")]
#[test]
fn a_subtask_timeout_ends_only_its_goal_and_preserves_governed_control() {
    use crate::governed_real_world::{AgentBridge, RealWorld};
    use nexus_governed_control::authority::approval::{
        ActionConfirmation, ControlConfirmer, GrantConfirmation, ResumeConfirmation,
    };
    use nexus_governed_control::governed::{GrantRequest, Intent};
    use nexus_governed_control::tool::ToolIntent;
    struct Yes;
    impl ControlConfirmer for Yes {
        fn confirm_action(&self, _: &ActionConfirmation) -> bool {
            true
        }
        fn confirm_grant(&self, _: &GrantConfirmation) -> bool {
            true
        }
        fn confirm_resume(&self, _: &ResumeConfirmation) -> bool {
            true
        }
        fn confirm_acknowledgement(
            &self,
            _: &nexus_governed_control::authority::approval::AcknowledgementConfirmation,
        ) -> bool {
            false
        }
    }
    struct Root(std::path::PathBuf);
    impl Drop for Root {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let root = Root(std::env::temp_dir().join(format!("nexus-p3-timeout-{}", Uuid::new_v4())));
    let mut state = AppState::new_in_memory();
    let world = RealWorld::for_tests(&root.0, state.audit.clone(), state.db.clone());
    state.real_world = Ok(world.clone());
    let agent = register(&state, "hive-timeout");
    let session = admit_hivemind(&state).unwrap();
    world
        .request_grant(
            &GrantRequest::Tool {
                tool: "text.sha256".into(),
            },
            600,
            &Yes,
        )
        .unwrap();
    let bridge = AgentBridge::with_warden(world.clone(), || false);
    let intent = Intent::Tool(ToolIntent {
        tool: "text.sha256".into(),
        input: json!({"text":"still active"}),
    });
    assert!(bridge.act(&agent, &intent, false).is_ok());
    let timeout = |goal: &str| {
        assert_eq!(
            await_subtask(
                &state,
                &session,
                &agent,
                goal,
                "part",
                std::time::Duration::ZERO
            ),
            Err("sub-task timed out after 0s".to_string())
        );
    };
    // When the timed-out goal is still current, it is removed.
    let old = execute_agent_goal(&state, agent.clone(), "part".into(), 5, None).unwrap();
    timeout(&old);
    assert!(
        !state.cognitive_runtime.has_active_loop(&agent),
        "the timed-out goal kept its loop"
    );
    assert!(
        !world.was_stopped(&agent),
        "a timeout became an agent-wide stop"
    );
    assert!(
        bridge.act(&agent, &intent, false).is_ok(),
        "governed control stopped with a subtask"
    );
    // A replacement is already installed before the old wait times out.
    let old = execute_agent_goal(&state, agent.clone(), "older part".into(), 5, None).unwrap();
    let newer = execute_agent_goal(&state, agent.clone(), "newer goal".into(), 5, None).unwrap();
    timeout(&old);
    assert_eq!(
        state
            .cognitive_runtime
            .get_agent_status_fast(&agent)
            .and_then(|status| status.active_goal)
            .map(|goal| goal.id),
        Some(newer.clone()),
        "a subtask timeout ended a newer goal"
    );
    assert!(
        !world.was_stopped(&agent),
        "a timeout became an agent-wide stop"
    );
    assert!(
        bridge.act(&agent, &intent, false).is_ok(),
        "the existing governed run must remain usable"
    );
    end_goal_loop(&state, &agent, &newer);
}

// ── P4-S1-H1: a session's lead, and each participant, take new work only
// while Running or Paused ──

/// The lifecycle states in which an agent takes no new work.
const HELD: [AgentState; 5] = [
    AgentState::Created,
    AgentState::Starting,
    AgentState::Stopping,
    AgentState::Stopped,
    AgentState::Destroyed,
];

/// An agent in `lifecycle`: started (then paused or stopped), or, for a
/// start or removal that has not finished, registered in that state.
fn register_in(state: &AppState, name: &str, lifecycle: AgentState) -> String {
    let mut supervisor = state.supervisor.lock().unwrap();
    let id = match lifecycle {
        AgentState::Running | AgentState::Paused | AgentState::Stopped => {
            let id = supervisor.start_agent(manifest(name)).unwrap();
            match lifecycle {
                AgentState::Paused => supervisor.pause_agent(id).unwrap(),
                AgentState::Stopped => supervisor.stop_agent(id).unwrap(),
                _ => {}
            }
            id
        }
        held => supervisor
            .register_agent(Uuid::new_v4(), manifest(name), held)
            .unwrap(),
    };
    assert_eq!(
        supervisor.get_agent(id).map(|handle| handle.state),
        Some(lifecycle)
    );
    id.to_string()
}

/// The session model calls a planner was asked for.
#[derive(Default)]
struct Calls {
    decompose: AtomicUsize,
    merge: AtomicUsize,
}

/// A planner that counts the model calls of its session and decomposes the
/// goal into `plan`. While it decomposes it can stop an agent, as the
/// owner's stop landing while that call is in flight.
struct CountingPlanner {
    calls: Arc<Calls>,
    plan: serde_json::Value,
    stop_while_planning: Option<(AppState, String)>,
}

impl HivemindLlm for CountingPlanner {
    fn decompose(
        &self,
        ctx: &mut DispatchContext,
        _: &mut PromptSource<'_>,
    ) -> Result<DerivedItem, nexus_kernel::errors::AgentError> {
        self.calls.decompose.fetch_add(1, Ordering::SeqCst);
        if let Some((state, agent)) = &self.stop_while_planning {
            state
                .supervisor
                .lock()
                .unwrap()
                .stop_agent(Uuid::parse_str(agent).unwrap())
                .unwrap();
        }
        Ok(ctx.produce(self.plan.to_string()))
    }
    fn merge(
        &self,
        ctx: &mut DispatchContext,
        _: &mut PromptSource<'_>,
    ) -> Result<DerivedItem, nexus_kernel::errors::AgentError> {
        self.calls.merge.fetch_add(1, Ordering::SeqCst);
        Ok(ctx.produce("merged"))
    }
}

fn three_parts() -> serde_json::Value {
    json!([
        {"id": "a", "description": "first part", "required_capabilities": ["llm.query"]},
        {"id": "b", "description": "second part", "required_capabilities": ["llm.query"]},
        {"id": "c", "description": "third part", "required_capabilities": ["llm.query"]},
    ])
}

/// A decomposition whose one sub-task needs a capability no participant
/// has: nothing is assigned or run, and the coordinator goes on to merge
/// (a decomposition of no sub-task at all is refused as such).
fn nobody_can_take() -> serde_json::Value {
    json!([
        {"id": "x", "description": "a part", "required_capabilities": ["p4.h1.no.such.capability"]},
    ])
}

/// Nothing was started for `agents`: no task, loop or published goal.
fn assert_nothing_started(state: &AppState, agents: &[&String], context: &str) {
    for agent in agents {
        assert!(
            state.db.load_tasks_by_agent(agent, 10).unwrap().is_empty(),
            "{context}: a task for {agent}"
        );
        assert!(
            !state.cognitive_runtime.has_active_loop(agent),
            "{context}: a loop for {agent}"
        );
        assert!(
            state
                .cognitive_runtime
                .get_agent_status_fast(agent)
                .is_none(),
            "{context}: a goal published for {agent}"
        );
    }
}

/// A session whose lead takes no new work is refused before anything is
/// resolved or made for it: its planner is never made, no model is asked
/// (no decomposition, no merge), nothing is started for any participant,
/// and nothing is stored or audited after its admission. An id that names
/// no registered agent is refused as before.
#[test]
fn p4_s1_h1_a_session_whose_lead_takes_no_new_work_is_refused_before_any_model_call() {
    for lifecycle in HELD {
        let state = AppState::new_in_memory();
        let lead = register_in(&state, "h1-held-lead", lifecycle);
        let member = register(&state, "h1-member");
        let session = admit_hivemind(&state).unwrap();
        let audited = state.audit.lock().unwrap().events().len();
        let (calls, made) = (Arc::new(Calls::default()), AtomicUsize::new(0));
        let refused = start_hivemind_with(
            &state,
            &session,
            "the goal".into(),
            vec![lead.clone(), member.clone()],
            |_| {
                made.fetch_add(1, Ordering::SeqCst);
                Box::new(CountingPlanner {
                    calls: calls.clone(),
                    plan: three_parts(),
                    stop_while_planning: None,
                })
            },
        );
        assert_eq!(
            refused,
            Err(format!(
                "hivemind session refused: its lead agent '{lead}' is {lifecycle}: \
                 it takes no new goal or task"
            )),
            "{lifecycle}"
        );
        assert_eq!(made.load(Ordering::SeqCst), 0, "{lifecycle}: a planner");
        assert_eq!(calls.decompose.load(Ordering::SeqCst), 0, "{lifecycle}");
        assert_eq!(calls.merge.load(Ordering::SeqCst), 0, "{lifecycle}");
        assert_nothing_started(&state, &[&lead, &member], &lifecycle.to_string());
        assert!(state.db.list_hivemind_sessions().unwrap().is_empty());
        assert_eq!(
            state.audit.lock().unwrap().events().len(),
            audited,
            "{lifecycle}: audited"
        );
    }
    let state = AppState::new_in_memory();
    let member = register(&state, "h1-member");
    let unregistered = Uuid::new_v4().to_string();
    let session = admit_hivemind(&state).unwrap();
    let calls = Arc::new(Calls::default());
    let refused = start_hivemind_with(
        &state,
        &session,
        "the goal".into(),
        vec![unregistered.clone(), member.clone()],
        |_| {
            Box::new(CountingPlanner {
                calls: calls.clone(),
                plan: three_parts(),
                stop_while_planning: None,
            })
        },
    );
    assert_eq!(
        refused,
        Err(format!(
            "hivemind session refused: '{unregistered}' is not a registered agent"
        ))
    );
    assert_eq!(calls.decompose.load(Ordering::SeqCst), 0);
    assert_nothing_started(&state, &[&unregistered, &member], "unregistered");
}

/// The control: a Running or Paused lead (the frozen paused-tick contract
/// keeps a paused agent's work) still has its session planned and merged.
#[test]
fn p4_s1_h1_a_running_or_paused_lead_still_plans_and_merges() {
    for lifecycle in [AgentState::Running, AgentState::Paused] {
        let state = AppState::new_in_memory();
        let lead = register_in(&state, "h1-lead", lifecycle);
        let member = register(&state, "h1-member");
        let session = admit_hivemind(&state).unwrap();
        let calls = Arc::new(Calls::default());
        let outcome = start_hivemind_with(
            &state,
            &session,
            "the goal".into(),
            vec![lead.clone(), member.clone()],
            |_| {
                Box::new(CountingPlanner {
                    calls: calls.clone(),
                    plan: nobody_can_take(),
                    stop_while_planning: None,
                })
            },
        )
        .unwrap();
        assert_eq!(outcome["status"], "Completed", "{lifecycle}: {outcome}");
        assert_eq!(calls.decompose.load(Ordering::SeqCst), 1, "{lifecycle}");
        assert_eq!(calls.merge.load(Ordering::SeqCst), 1, "{lifecycle}");
        assert_nothing_started(&state, &[&lead, &member], &lifecycle.to_string());
    }
}

/// The owner's stop of the lead lands after the session was admitted and
/// its context made, before it plans: no model is asked anything (the
/// decomposition is refused at its requalification), nothing is started
/// for any participant, and the session is not stored.
#[test]
fn p4_s1_h1_a_lead_stopped_before_its_session_plans_gets_no_decomposition() {
    let state = AppState::new_in_memory();
    let lead = register(&state, "h1-lead");
    let member = register(&state, "h1-member");
    let session = admit_hivemind(&state).unwrap();
    let calls = Arc::new(Calls::default());
    let ended = start_hivemind_with(
        &state,
        &session,
        "the goal".into(),
        vec![lead.clone(), member.clone()],
        |_| {
            state
                .supervisor
                .lock()
                .unwrap()
                .stop_agent(Uuid::parse_str(&lead).unwrap())
                .unwrap();
            Box::new(CountingPlanner {
                calls: calls.clone(),
                plan: three_parts(),
                stop_while_planning: None,
            })
        },
    );
    assert!(
        ended.as_ref().is_err_and(|e| e.contains(&format!(
            "hivemind session ended: its lead agent '{lead}' is Stopped: \
             it takes no new goal or task"
        ))),
        "{ended:?}"
    );
    assert_eq!(calls.decompose.load(Ordering::SeqCst), 0, "planned");
    assert_eq!(calls.merge.load(Ordering::SeqCst), 0, "merged");
    assert_nothing_started(&state, &[&lead, &member], "stopped before planning");
    assert!(state.db.list_hivemind_sessions().unwrap().is_empty());
}

/// The owner's stop of the lead lands while the session plans: the
/// decomposition already in flight completes (no lock is held across a
/// model call), and nothing after it runs. Every sub-task is refused,
/// its reassignment too, so no participant is started anything, and the
/// session fails with no merge.
#[test]
fn p4_s1_h1_a_lead_stopped_while_its_session_plans_gets_no_subtask_run_and_no_merge() {
    let state = AppState::new_in_memory();
    let lead = register(&state, "h1-lead");
    let member = register(&state, "h1-member");
    let session = admit_hivemind(&state).unwrap();
    let calls = Arc::new(Calls::default());
    let outcome = start_hivemind_with(
        &state,
        &session,
        "the goal".into(),
        vec![lead.clone(), member.clone()],
        |_| {
            Box::new(CountingPlanner {
                calls: calls.clone(),
                plan: three_parts(),
                stop_while_planning: Some((state.clone(), lead.clone())),
            })
        },
    )
    .unwrap();
    assert_eq!(outcome["status"], "Failed", "{outcome}");
    assert_eq!(calls.decompose.load(Ordering::SeqCst), 1);
    assert_eq!(
        calls.merge.load(Ordering::SeqCst),
        0,
        "a stopped lead's session merged"
    );
    assert_nothing_started(&state, &[&lead, &member], "stopped while planning");
    // Only the lead was stopped: the member could still take work of its own.
    assert_eq!(
        state
            .supervisor
            .lock()
            .unwrap()
            .get_agent(Uuid::parse_str(&member).unwrap())
            .map(|handle| handle.state),
        Some(AgentState::Running)
    );
}

/// The owner's stop of the lead lands before the session merges (here a
/// session whose one sub-task no participant can take, stopped as it
/// plans, so the coordinator goes straight on to merge): the merge is not
/// asked for, the session ends with the reason, and it is not stored.
#[test]
fn p4_s1_h1_a_lead_stopped_before_its_session_merges_gets_no_merge() {
    let state = AppState::new_in_memory();
    let lead = register(&state, "h1-lead");
    let member = register(&state, "h1-member");
    let session = admit_hivemind(&state).unwrap();
    let calls = Arc::new(Calls::default());
    let ended = start_hivemind_with(
        &state,
        &session,
        "the goal".into(),
        vec![lead.clone(), member.clone()],
        |_| {
            Box::new(CountingPlanner {
                calls: calls.clone(),
                plan: nobody_can_take(),
                stop_while_planning: Some((state.clone(), lead.clone())),
            })
        },
    );
    assert!(
        ended.as_ref().is_err_and(|e| e.contains(&format!(
            "hivemind session ended: its lead agent '{lead}' is Stopped: \
             it takes no new goal or task"
        ))),
        "{ended:?}"
    );
    assert_eq!(calls.decompose.load(Ordering::SeqCst), 1);
    assert_eq!(
        calls.merge.load(Ordering::SeqCst),
        0,
        "merged after the stop"
    );
    assert_nothing_started(&state, &[&lead, &member], "stopped before merging");
    assert!(state.db.list_hivemind_sessions().unwrap().is_empty());
}

/// The desktop's predicate is the kernel's: for every lifecycle state, a
/// goal is admitted on the desktop's own route exactly when the desktop
/// says the agent takes new work, and a refusal carries the desktop's
/// reason.
#[test]
fn p4_s1_h1_the_desktop_admits_exactly_what_the_kernel_admits() {
    for lifecycle in [AgentState::Running, AgentState::Paused]
        .into_iter()
        .chain(HELD)
    {
        let state = AppState::new_in_memory();
        let agent = register_in(&state, "h1-parity", lifecycle);
        assert_eq!(lifecycle_of(&state, &agent), Some(lifecycle));
        let kernel = execute_agent_goal(&state, agent.clone(), "a goal".into(), 5, None);
        match refuses_new_work(&agent, lifecycle) {
            None => assert!(kernel.is_ok(), "{lifecycle}: {kernel:?}"),
            Some(reason) => assert!(
                kernel.as_ref().is_err_and(|e| e.contains(&reason)),
                "{lifecycle}: {kernel:?} / {reason}"
            ),
        }
    }
}

/// A session requalifies only while its lead takes new work: Running and
/// Paused still qualify; every other state, or a lead no longer
/// registered, ends the session's work (its next sub-task, its next model
/// call, its sub-tasks' loops' next model calls).
#[test]
fn p4_s1_h1_a_session_requalifies_only_while_its_lead_is_running_or_paused() {
    for lifecycle in [AgentState::Running, AgentState::Paused]
        .into_iter()
        .chain(HELD)
    {
        let state = AppState::new_in_memory();
        let lead = register_in(&state, "h1-lead", lifecycle);
        let member = register(&state, "h1-member");
        let (admission, _) = hivemind_test_session(&state, &[lead.clone(), member.clone()]);
        let expected = if HELD.contains(&lifecycle) {
            Err(format!(
                "hivemind session ended: its lead agent '{lead}' is {lifecycle}: \
                 it takes no new goal or task"
            ))
        } else {
            Ok(())
        };
        assert_eq!(admission.requalify(), expected, "{lifecycle}");
    }
    let state = AppState::new_in_memory();
    let lead = register(&state, "h1-lead");
    let member = register(&state, "h1-member");
    let (admission, _) = hivemind_test_session(&state, &[lead.clone(), member.clone()]);
    assert_eq!(admission.requalify(), Ok(()));
    state.supervisor.lock().unwrap().clear_all_agents();
    assert_eq!(
        admission.requalify(),
        Err(format!(
            "hivemind session ended: its lead '{lead}' is not a registered agent"
        ))
    );
}

/// A participant other than the lead that takes no new work is refused
/// its sub-task, truthfully, before anything is started for it (no goal,
/// loop, task, or model call: its loop never starts); the session still
/// qualifies, its lead taking new work.
#[test]
fn p4_s1_h1_a_member_that_takes_no_new_work_is_refused_its_subtask_truthfully() {
    for lifecycle in HELD {
        let state = AppState::new_in_memory();
        let lead = register(&state, "h1-lead");
        let member = register_in(&state, "h1-held-member", lifecycle);
        let session = admit_hivemind(&state).unwrap();
        let (admission, session_ctx) =
            hivemind_test_session(&state, &[lead.clone(), member.clone()]);
        let refused = execute_hivemind_subtask(
            &state,
            &session,
            &admission,
            &session_ctx,
            1,
            &member,
            "a part",
        );
        assert!(refused.context.is_none(), "{lifecycle}");
        let truthful = match lifecycle {
            AgentState::Created | AgentState::Starting => format!(
                "sub-task refused: agent '{member}' is {lifecycle}: it takes no new goal or task"
            ),
            _ => format!("sub-task refused: agent '{member}' is stopped"),
        };
        assert_eq!(refused.outcome.err(), Some(truthful), "{lifecycle}");
        assert_nothing_started(&state, &[&member], &lifecycle.to_string());
        assert_eq!(admission.requalify(), Ok(()), "{lifecycle}");
    }
}
