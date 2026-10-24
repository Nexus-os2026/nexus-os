//! Phase Three in the desktop, isolated: a temporary runtime root, an
//! in-memory audit trail and database, scripted owner answers. Nothing here
//! opens the owner's state directory or database.

use super::{AgentBridge, RealWorld};
use nexus_governed_control::authority::approval::{
    ActionConfirmation, ControlConfirmer, GrantConfirmation, ResumeConfirmation,
};
use nexus_governed_control::authority::run::RunOrigin;
use nexus_governed_control::governed::{GrantRequest, Intent};
use nexus_governed_control::ingress::CommandEnvelope;
use nexus_governed_control::tool::ToolIntent;
use nexus_kernel::audit::AuditTrail;
use nexus_persistence::NexusDatabase;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

/// The owner's scripted answer, counting the asks.
struct Owner(bool, AtomicU32);

impl ControlConfirmer for Owner {
    fn confirm_action(&self, _: &ActionConfirmation) -> bool {
        self.1.fetch_add(1, Ordering::SeqCst);
        self.0
    }
    fn confirm_grant(&self, _: &GrantConfirmation) -> bool {
        self.1.fetch_add(1, Ordering::SeqCst);
        self.0
    }
    fn confirm_resume(&self, _: &ResumeConfirmation) -> bool {
        self.1.fetch_add(1, Ordering::SeqCst);
        self.0
    }
    fn confirm_acknowledgement(
        &self,
        _: &nexus_governed_control::authority::approval::AcknowledgementConfirmation,
    ) -> bool {
        false
    }
}

struct Isolated {
    world: Arc<RealWorld>,
    audit: Arc<Mutex<AuditTrail>>,
    root: std::path::PathBuf,
}

impl Drop for Isolated {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn isolated() -> Isolated {
    let root = std::env::temp_dir().join(format!("nexus-p3-desktop-{}", uuid::Uuid::new_v4()));
    let audit = Arc::new(Mutex::new(AuditTrail::new()));
    let db = Arc::new(NexusDatabase::in_memory().unwrap());
    let world = RealWorld::for_tests(&root, audit.clone(), db);
    Isolated { world, audit, root }
}

/// A commitment's state as the interface lists it.
fn commitment_state(world: &RealWorld, id: &str) -> String {
    world.status()["commitments"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == id)
        .map(|c| c["state"].as_str().unwrap().to_string())
        .unwrap_or_default()
}

fn evidence_events(audit: &Arc<Mutex<AuditTrail>>) -> usize {
    audit
        .lock()
        .unwrap()
        .events()
        .iter()
        .filter(|e| e.payload["event_kind"] == "p3.action.evidence")
        .count()
}

#[test]
fn an_owner_command_is_understood_committed_approved_and_audited() {
    let t = isolated();
    let owner = Owner(true, AtomicU32::new(0));
    t.world
        .request_grant(
            &GrantRequest::Tool {
                tool: "text.sha256".into(),
            },
            60,
            &owner,
        )
        .unwrap();
    let submitted = t
        .world
        .submit(CommandEnvelope {
            text: Some("hash abc".into()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(submitted["understood"], true);
    let commitment = submitted["commitment"]["id"].as_str().unwrap().to_string();
    assert_eq!(submitted["commitment"]["class"], "R0");
    assert_eq!(submitted["commitment"]["state"], "prepared");
    let output = t.world.approve(&commitment, &owner).unwrap();
    assert!(output["text"]
        .as_str()
        .unwrap()
        .starts_with("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"));
    // The run of an owner command ends with it; nothing runs twice.
    assert!(t.world.approve(&commitment, &owner).is_err());
    // Every phase went into the hash-chained audit trail.
    assert!(
        evidence_events(&t.audit) >= 5,
        "grant, run, prepared, authorized, started, finished"
    );
}

#[test]
fn a_command_outside_the_grammar_commits_nothing() {
    let t = isolated();
    let before = evidence_events(&t.audit);
    let answer = t
        .world
        .submit(CommandEnvelope {
            text: Some("please run bash -c 'rm -rf /'".into()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(answer["understood"], false);
    assert_eq!(evidence_events(&t.audit), before);
    // Without a grant, an understood command is refused before committing.
    assert!(t
        .world
        .submit(CommandEnvelope {
            text: Some("hash abc".into()),
            ..Default::default()
        })
        .is_err());
}

#[test]
fn a_denied_or_declined_commitment_never_runs() {
    let t = isolated();
    let owner = Owner(true, AtomicU32::new(0));
    t.world
        .request_grant(
            &GrantRequest::Tool {
                tool: "text.sha256".into(),
            },
            60,
            &owner,
        )
        .unwrap();
    let submitted = t
        .world
        .submit(CommandEnvelope {
            text: Some("hash abc".into()),
            ..Default::default()
        })
        .unwrap();
    let commitment = submitted["commitment"]["id"].as_str().unwrap().to_string();
    t.world.deny(&commitment).unwrap();
    assert!(t.world.approve(&commitment, &owner).is_err());
    // A declined grant creates nothing.
    assert!(t
        .world
        .request_grant(
            &GrantRequest::Perception,
            60,
            &Owner(false, AtomicU32::new(0))
        )
        .is_err());
}

#[test]
fn an_agent_runs_granted_r0_and_never_raises_the_owners_dialog() {
    let t = isolated();
    let owner = Owner(true, AtomicU32::new(0));
    t.world
        .request_grant(
            &GrantRequest::Tool {
                tool: "text.sha256".into(),
            },
            60,
            &owner,
        )
        .unwrap();
    let asked = owner.1.load(Ordering::SeqCst);
    let bridge = AgentBridge::with_warden(t.world.clone(), || false);
    let agent = uuid::Uuid::new_v4().to_string();
    let intent = Intent::Tool(ToolIntent {
        tool: "text.sha256".into(),
        input: json!({ "text": "abc" }),
    });
    let out = bridge.act(&agent, &intent, true).unwrap();
    assert!(out.contains("ba7816bf"), "{out}");
    assert_eq!(owner.1.load(Ordering::SeqCst), asked);
    // Another agent cannot use this loop's run.
    assert!(bridge
        .act(&uuid::Uuid::new_v4().to_string(), &intent, true)
        .is_err());
    // Emergency stop: the agent's next action is refused.
    t.world.emergency_stop();
    assert!(bridge.act(&agent, &intent, true).is_err());
}

/// The owner's Warden setting reviews governed actions as it reviews the
/// registry's: enabled, it refuses every action it reviews, before anything
/// is prepared.
#[test]
fn an_enabled_warden_review_refuses_a_reviewed_agent_action() {
    let t = isolated();
    let owner = Owner(true, AtomicU32::new(0));
    t.world
        .request_grant(
            &GrantRequest::Tool {
                tool: "text.sha256".into(),
            },
            60,
            &owner,
        )
        .unwrap();
    let bridge = AgentBridge::with_warden(t.world.clone(), || true);
    let agent = uuid::Uuid::new_v4().to_string();
    let intent = Intent::Tool(ToolIntent {
        tool: "text.sha256".into(),
        input: json!({ "text": "abc" }),
    });
    let refused = bridge.act(&agent, &intent, true).unwrap_err();
    assert!(refused.starts_with("Warden blocked action"), "{refused}");
    assert!(
        t.world.status()["commitments"]
            .as_array()
            .is_none_or(|c| c.is_empty()),
        "nothing was prepared"
    );
    // An action the review does not cover (a page read) is not refused by it.
    assert!(bridge.act(&agent, &intent, false).is_ok());
}

/// The owner's stop of an agent cancels its Phase Three run: what it left
/// waiting for approval can no longer be approved.
#[test]
fn stopping_an_agent_cancels_what_it_left_waiting() {
    let t = isolated();
    let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", server.local_addr().unwrap());
    let owner = Owner(true, AtomicU32::new(0));
    t.world
        .request_grant(
            &GrantRequest::Egress {
                origin: origin.clone(),
                methods: vec!["POST".into()],
                allow_private: true,
            },
            60,
            &owner,
        )
        .unwrap();
    let bridge = AgentBridge::with_warden(t.world.clone(), || false);
    let agent = uuid::Uuid::new_v4().to_string();
    let post = Intent::Request(nexus_governed_control::egress::EgressIntent {
        method: "POST".into(),
        url: format!("{origin}/x"),
        headers: vec![],
        body: Some("{}".into()),
    });
    let waiting: Value = serde_json::from_str(&bridge.act(&agent, &post, true).unwrap()).unwrap();
    let commitment = waiting["awaiting_owner_approval"]
        .as_str()
        .unwrap()
        .to_string();
    t.world.cancel_agent(&agent);
    assert!(t.world.approve(&commitment, &owner).is_err());
    let state = t.world.status()["commitments"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == commitment)
        .map(|c| c["state"].clone())
        .unwrap();
    assert_eq!(state, "revoked");
    server.set_nonblocking(true).unwrap();
    assert!(server.accept().is_err(), "nothing was sent");
}

#[test]
fn the_production_executor_routes_governed_actions_only_through_phase_three() {
    use nexus_kernel::cognitive::loop_runtime::ActionExecutor;
    use nexus_kernel::cognitive::PlannedAction;
    use std::io::{Read, Write};

    // A loopback page server that records the user agent it is asked with.
    let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", server.local_addr().unwrap());
    let seen = Arc::new(Mutex::new(Vec::<String>::new()));
    {
        let seen = seen.clone();
        std::thread::spawn(move || {
            for stream in server.incoming().flatten().take(1) {
                let mut stream = stream;
                let mut request = [0u8; 4096];
                let n = stream.read(&mut request).unwrap_or(0);
                seen.lock()
                    .unwrap()
                    .push(String::from_utf8_lossy(&request[..n]).into_owned());
                let _ = stream.write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 8\r\nConnection: close\r\n\r\ngoverned",
                );
            }
        });
    }
    let t = isolated();
    let state = crate::AppState::new_in_memory();
    let memory = Arc::new(nexus_kernel::cognitive::AgentMemoryManager::new(Box::new(
        crate::DbMemoryStore {
            db: state.db.clone(),
        },
    )));
    let executor = crate::phase0_agent_executor(
        &state,
        memory,
        crate::hm_dispatch::GatewayPlannerLlm::new(&state),
    )
    .with_real_world(t.world.clone());
    let mut audit = state.audit.clone();
    let agent = uuid::Uuid::new_v4().to_string();
    let fetch = PlannedAction::WebFetch {
        url: format!("{origin}/page"),
    };
    // A model-chosen URL is not egress authority: no grant, no request.
    let refused = executor
        .execute(&agent, &fetch, &mut audit, true)
        .unwrap_err();
    assert!(refused.contains("no_covering_grant"), "{refused}");
    assert!(seen.lock().unwrap().is_empty());
    // Under the owner's grant it runs through the Phase Three transport.
    t.world
        .request_grant(
            &GrantRequest::Egress {
                origin: origin.clone(),
                methods: vec!["GET".into()],
                allow_private: true,
            },
            60,
            &Owner(true, AtomicU32::new(0)),
        )
        .unwrap();
    let out = executor.execute(&agent, &fetch, &mut audit, false).unwrap();
    assert!(out.contains("governed"), "{out}");
    let request = seen.lock().unwrap().join("");
    assert!(
        request.contains("NexusOS-GovernedControl/1"),
        "the Phase Three transport made the request: {request}"
    );
    // Closed actions keep their Phase Zero refusal, approved or not.
    let shell = PlannedAction::ShellCommand {
        command: "touch".into(),
        args: vec![],
    };
    assert_eq!(
        executor.execute(&agent, &shell, &mut audit, true),
        Err(crate::phase0_surface::closed(
            "shell_command",
            crate::phase0_surface::Closure::AgentExecution
        ))
    );
}

/// One confirmation window at a time, without a lock held across it: while
/// one is on screen the next waits (within its bound) and, the bound
/// passed, gets no window at all (no confirmation); once it is answered the
/// next opens.
#[test]
fn one_confirmation_at_a_time_and_a_wait_is_bounded() {
    use super::DialogTurn;
    use std::time::{Duration, Instant};
    let first = DialogTurn::take_within(Duration::from_secs(1)).expect("free");
    let began = Instant::now();
    assert!(DialogTurn::take_within(Duration::from_millis(200)).is_none());
    assert!(began.elapsed() >= Duration::from_millis(200));
    let waiting = std::thread::spawn(|| DialogTurn::take_within(Duration::from_secs(10)).is_some());
    std::thread::sleep(Duration::from_millis(50));
    drop(first);
    assert!(
        waiting.join().unwrap(),
        "the next opens once one is answered"
    );
    assert!(DialogTurn::take_within(Duration::from_millis(10)).is_some());
}

/// The answer of a confirmation arms only once the delay has passed and
/// the end and the right edge of its text have been reached; a view not
/// laid out yet has reached nothing.
#[test]
fn a_confirmation_arms_only_after_the_delay_and_the_whole_text() {
    use super::{reached, Arming};
    assert!(!reached(0.0, 0.0, 0.0), "not laid out yet");
    assert!(reached(0.0, 400.0, 400.0), "it fits");
    assert!(!reached(0.0, 400.0, 1200.0));
    assert!(reached(800.0, 400.0, 1200.0));
    let all = Arming {
        delay_passed: true,
        end_reached: true,
        edge_reached: true,
    };
    assert!(all.ready());
    for missing in [
        Arming {
            delay_passed: false,
            ..all
        },
        Arming {
            end_reached: false,
            ..all
        },
        Arming {
            edge_reached: false,
            ..all
        },
    ] {
        assert!(!missing.ready(), "{missing:?}");
    }
}

/// A paused or stopped agent acts in the real world no more, however it
/// was paused or stopped.
#[test]
fn an_agent_that_is_not_running_acts_no_more() {
    let t = isolated();
    let owner = Owner(true, AtomicU32::new(0));
    t.world
        .request_grant(
            &GrantRequest::Tool {
                tool: "text.sha256".into(),
            },
            60,
            &owner,
        )
        .unwrap();
    let bridge = AgentBridge::for_tests(t.world.clone(), || false, |_| false);
    let intent = Intent::Tool(ToolIntent {
        tool: "text.sha256".into(),
        input: json!({ "text": "abc" }),
    });
    let refused = bridge
        .act(&uuid::Uuid::new_v4().to_string(), &intent, false)
        .unwrap_err();
    assert!(refused.contains("not running"), "{refused}");
}

/// The owner's stop reaches a loop even before it opened its run: no loop
/// begun before the stop acts again, and a loop begun after it does.
#[test]
fn a_stop_reaches_a_loop_that_had_not_opened_its_run() {
    let t = isolated();
    let owner = Owner(true, AtomicU32::new(0));
    t.world
        .request_grant(
            &GrantRequest::Tool {
                tool: "text.sha256".into(),
            },
            60,
            &owner,
        )
        .unwrap();
    let agent = uuid::Uuid::new_v4().to_string();
    let intent = Intent::Tool(ToolIntent {
        tool: "text.sha256".into(),
        input: json!({ "text": "abc" }),
    });
    let before = AgentBridge::with_warden(t.world.clone(), || false);
    t.world.cancel_agent(&agent);
    let refused = before.act(&agent, &intent, false).unwrap_err();
    assert!(refused.contains("stopped"), "{refused}");
    let after = AgentBridge::with_warden(t.world.clone(), || false);
    assert!(after.act(&agent, &intent, false).is_ok());
}

/// Quitting cancels every run that is still open.
#[test]
fn quitting_cancels_every_open_run() {
    let t = isolated();
    let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", server.local_addr().unwrap());
    let owner = Owner(true, AtomicU32::new(0));
    t.world
        .request_grant(
            &GrantRequest::Egress {
                origin: origin.clone(),
                methods: vec!["POST".into()],
                allow_private: true,
            },
            60,
            &owner,
        )
        .unwrap();
    let bridge = AgentBridge::with_warden(t.world.clone(), || false);
    let agent = uuid::Uuid::new_v4().to_string();
    let post = Intent::Request(nexus_governed_control::egress::EgressIntent {
        method: "POST".into(),
        url: format!("{origin}/x"),
        headers: vec![],
        body: Some("{}".into()),
    });
    let waiting: Value = serde_json::from_str(&bridge.act(&agent, &post, true).unwrap()).unwrap();
    let commitment = waiting["awaiting_owner_approval"]
        .as_str()
        .unwrap()
        .to_string();
    t.world.shutdown();
    assert!(t.world.approve(&commitment, &owner).is_err());
    assert!(t.world.status()["runs"]
        .as_array()
        .unwrap()
        .iter()
        .all(|run| run["cancelled"] == true || run["finished"] == true));
}

/// Q8 (re-audit of candidate 4): quitting waits, within its bound, until
/// nothing executes: an agent's request in flight when the desktop quits
/// has ended (cancelled, recorded) by the time `shutdown` returns.
#[test]
fn quitting_waits_until_nothing_executes() {
    let t = isolated();
    // An upstream that accepts and never answers.
    let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", server.local_addr().unwrap());
    let owner = Owner(true, AtomicU32::new(0));
    t.world
        .request_grant(
            &GrantRequest::Egress {
                origin: origin.clone(),
                methods: vec!["GET".into()],
                allow_private: true,
            },
            60,
            &owner,
        )
        .unwrap();
    let states = |world: &RealWorld| -> Vec<String> {
        world.status()["commitments"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["state"].as_str().unwrap().to_string())
            .collect()
    };
    let world = t.world.clone();
    let worker = std::thread::spawn(move || {
        let bridge = AgentBridge::with_warden(world, || false);
        let get = Intent::Request(nexus_governed_control::egress::EgressIntent {
            method: "GET".into(),
            url: format!("{origin}/slow"),
            headers: vec![],
            body: None,
        });
        bridge.act(&uuid::Uuid::new_v4().to_string(), &get, true)
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while !states(&t.world).iter().any(|s| s == "executing") {
        assert!(
            std::time::Instant::now() < deadline,
            "{:?}",
            states(&t.world)
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    t.world.shutdown();
    let after = states(&t.world);
    assert!(after.iter().all(|s| s != "executing"), "{after:?}");
    assert!(worker.join().unwrap().is_err());
    // From then on no run opens: nothing new starts while the desktop quits.
    let late = AgentBridge::with_warden(t.world.clone(), || false);
    let again = Intent::Request(nexus_governed_control::egress::EgressIntent {
        method: "GET".into(),
        url: "http://127.0.0.1:9/again".into(),
        headers: vec![],
        body: None,
    });
    let refused = late
        .act(&uuid::Uuid::new_v4().to_string(), &again, true)
        .unwrap_err();
    assert!(refused.contains("quitting"), "{refused}");
}

/// V1, V10 (verification of candidate 5): "Stop all" stops every agent before
/// any loop is waited for, and an agent whose actions and schedule were kept
/// under another spelling of its id is stopped all the same.
#[test]
fn every_agent_stops_at_once_under_any_spelling_of_its_id() {
    use nexus_kernel::lifecycle::AgentState;
    // Registration schedules nothing here; the scheduler's tasks are not polled.
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let _entered = runtime.enter();
    let t = isolated();
    let mut state = crate::AppState::new_in_memory();
    state.real_world = Ok(t.world.clone());
    let register = |name: &str| {
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
    };
    let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", server.local_addr().unwrap());
    let owner = Owner(true, AtomicU32::new(0));
    t.world
        .request_grant(
            &GrantRequest::Egress {
                origin: origin.clone(),
                methods: vec!["POST".into()],
                allow_private: true,
            },
            60,
            &owner,
        )
        .unwrap();
    let post = Intent::Request(nexus_governed_control::egress::EgressIntent {
        method: "POST".into(),
        url: format!("{origin}/x"),
        headers: vec![],
        body: Some("{}".into()),
    });
    // Each agent's loop has its own bridge.
    let waiting = |agent: &str| -> String {
        let bridge = AgentBridge::with_warden(t.world.clone(), || false);
        let reply: Value = serde_json::from_str(&bridge.act(agent, &post, true).unwrap()).unwrap();
        reply["awaiting_owner_approval"]
            .as_str()
            .unwrap()
            .to_string()
    };
    let (first, second) = (register("spelled-first"), register("spelled-second"));
    // The first acts, and is scheduled, under its upper-case spelling.
    let upper = first.to_string().to_uppercase();
    let first_waiting = waiting(&upper);
    let second_waiting = waiting(&second.to_string());
    crate::start_autonomous_loop(&state, upper.clone(), Some(60), None).unwrap();
    assert_eq!(state.agent_scheduler.list().len(), 1);
    assert_eq!(
        crate::admin_agent_stop_all(&state, "default".into()).unwrap(),
        2
    );
    for (agent, commitment) in [(first, first_waiting), (second, second_waiting)] {
        let stopped = state
            .supervisor
            .lock()
            .unwrap()
            .get_agent(agent)
            .unwrap()
            .state;
        assert_eq!(stopped, AgentState::Stopped);
        // Its waiting action ended with its run (revoked), however its id
        // was written.
        assert_eq!(commitment_state(&t.world, &commitment), "revoked");
        assert!(
            t.world.approve(&commitment, &owner).is_err(),
            "a stopped agent's waiting action cannot be approved"
        );
    }
    assert!(
        state.agent_scheduler.list().is_empty(),
        "the schedule kept under another spelling is gone"
    );
}

/// Q1 (re-audit of candidate 4): the Admin "Stop all" and a bulk "Stop"
/// stop each agent everywhere, as the per-agent Stop does: by the
/// supervisor's state (the list reports "Running", never "running"), its
/// loop and the supervisor, and what it left waiting in Phase Three.
#[test]
fn stop_all_and_bulk_stop_stop_agents_everywhere() {
    use nexus_kernel::lifecycle::AgentState;
    let t = isolated();
    let mut state = crate::AppState::new_in_memory();
    state.real_world = Ok(t.world.clone());
    let register = |name: &str| {
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
    };
    let agent_state = |id: uuid::Uuid| {
        state
            .supervisor
            .lock()
            .unwrap()
            .get_agent(id)
            .unwrap()
            .state
    };
    // A running agent with an R2 action waiting for the owner.
    let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", server.local_addr().unwrap());
    let owner = Owner(true, AtomicU32::new(0));
    t.world
        .request_grant(
            &GrantRequest::Egress {
                origin: origin.clone(),
                methods: vec!["POST".into()],
                allow_private: true,
            },
            60,
            &owner,
        )
        .unwrap();
    let first = register("stop-all-first");
    assert_eq!(agent_state(first), AgentState::Running);
    let bridge = AgentBridge::with_warden(t.world.clone(), || false);
    let post = Intent::Request(nexus_governed_control::egress::EgressIntent {
        method: "POST".into(),
        url: format!("{origin}/x"),
        headers: vec![],
        body: Some("{}".into()),
    });
    let waiting: Value =
        serde_json::from_str(&bridge.act(&first.to_string(), &post, true).unwrap()).unwrap();
    let commitment = waiting["awaiting_owner_approval"]
        .as_str()
        .unwrap()
        .to_string();
    let stopped = crate::admin_agent_stop_all(&state, "default".into()).unwrap();
    assert_eq!(stopped, 1);
    assert_eq!(agent_state(first), AgentState::Stopped);
    // The waiting action ended with its run (revoked), before any approval.
    assert_eq!(commitment_state(&t.world, &commitment), "revoked");
    assert!(
        t.world.approve(&commitment, &owner).is_err(),
        "a stopped agent's waiting action cannot be approved"
    );
    // A bulk "Stop" of one named agent stops that agent, and only it; any
    // other bulk action is reported as not done.
    let second = register("bulk-stop-second");
    let third = register("bulk-stop-third");
    let reply: Value = serde_json::from_str(
        &crate::admin_agent_bulk_update(
            &state,
            vec![format!("did:nexus:{second}")],
            r#"{"action":"stop"}"#.into(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(reply, json!({ "succeeded": 1, "failed": 0 }));
    assert_eq!(agent_state(second), AgentState::Stopped);
    assert_eq!(agent_state(third), AgentState::Running);
    let reply: Value = serde_json::from_str(
        &crate::admin_agent_bulk_update(
            &state,
            vec![format!("did:nexus:{third}")],
            r#"{"action":"restart"}"#.into(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(reply, json!({ "succeeded": 0, "failed": 1 }));
    // The fleet view sees the running agent as running.
    let fleet: Value = serde_json::from_str(&crate::admin_fleet_status(&state).unwrap()).unwrap();
    assert_eq!(fleet["total_running"], 1, "{fleet}");
}

/// Candidate 9 (C8-3): clearing every agent first stops each one as the
/// owner's Stop does, everywhere and under any spelling of its id, whatever
/// knows of it (the supervisor, a schedule, a loop's driver, a loop, Phase
/// Three), and only then clears their records: no loop runs on, no schedule
/// is left, and nothing they left running or waiting in Phase Three stays
/// open.
#[test]
fn clearing_all_agents_stops_each_one_everywhere_first() {
    // Registration schedules nothing here; the scheduler's tasks are not polled.
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let _entered = runtime.enter();
    let t = isolated();
    let mut state = crate::AppState::new_in_memory();
    state.real_world = Ok(t.world.clone());
    let register = |name: &str| {
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
    };
    let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", server.local_addr().unwrap());
    let owner = Owner(true, AtomicU32::new(0));
    t.world
        .request_grant(
            &GrantRequest::Egress {
                origin: origin.clone(),
                methods: vec!["POST".into()],
                allow_private: true,
            },
            60,
            &owner,
        )
        .unwrap();
    let post = Intent::Request(nexus_governed_control::egress::EgressIntent {
        method: "POST".into(),
        url: format!("{origin}/x"),
        headers: vec![],
        body: Some("{}".into()),
    });
    // An action waiting for the owner, from a loop of its own.
    let waiting = |agent: &str| -> String {
        let bridge = AgentBridge::with_warden(t.world.clone(), || false);
        let reply: Value = serde_json::from_str(&bridge.act(agent, &post, true).unwrap()).unwrap();
        reply["awaiting_owner_approval"]
            .as_str()
            .unwrap()
            .to_string()
    };
    let unknown = || uuid::Uuid::new_v4().to_string().to_uppercase();

    // Agents only one place still knows of, each under an upper-case
    // spelling: a loop with no driver whose record is already gone, a
    // schedule, a loop's driver, and Phase Three.
    let looping = register("cleared-loop-only").to_string().to_uppercase();
    crate::execute_agent_goal(&state, looping.clone(), "keep going".into(), 5, None).unwrap();
    state.supervisor.lock().unwrap().clear_all_agents();
    let scheduled = unknown();
    crate::start_autonomous_loop(&state, scheduled.clone(), Some(60), None).unwrap();
    let driven = unknown();
    let (driven_cancel, _driven) =
        crate::commands::cognitive::CognitiveCancelGuard::register(&state, &driven);
    let acting = unknown();
    let acting_waiting = waiting(&acting);
    // And a registered agent known everywhere, under its upper-case
    // spelling: an action waiting, a schedule, and a loop with its driver.
    let known = register("cleared-everywhere").to_string().to_uppercase();
    let known_waiting = waiting(&known);
    crate::start_autonomous_loop(&state, known.clone(), Some(60), None).unwrap();
    crate::execute_agent_goal(&state, known.clone(), "keep going".into(), 5, None).unwrap();
    let (known_cancel, _known) =
        crate::commands::cognitive::CognitiveCancelGuard::register(&state, &known);
    let mut loops = state.cognitive_runtime.loop_agents();
    loops.sort();
    let mut expected = vec![looping.clone(), known.clone()];
    expected.sort();
    assert_eq!(loops, expected);
    assert_eq!(state.agent_scheduler.list().len(), 2);

    crate::clear_all_agents(&state).unwrap();

    // Phase Three: what they left waiting ended with their runs (revoked),
    // and no agent's run is open.
    for commitment in [&known_waiting, &acting_waiting] {
        assert_eq!(
            commitment_state(&t.world, commitment),
            "revoked",
            "a cleared agent's waiting action stayed open"
        );
        assert!(
            t.world.approve(commitment, &owner).is_err(),
            "a cleared agent's waiting action cannot be approved"
        );
    }
    let open: Vec<_> = t
        .world
        .control
        .authority()
        .runs()
        .views()
        .into_iter()
        .filter(|run| run.origin == RunOrigin::AgentGoal && !run.cancelled && !run.finished)
        .collect();
    assert!(
        open.is_empty(),
        "a cleared agent's run stayed open: {open:?}"
    );
    // No schedule, every driver told to stop, and every loop removed (on
    // the stop's own thread, which may wait for a cycle).
    assert!(
        state.agent_scheduler.list().is_empty(),
        "a cleared agent's schedule was left: {:?}",
        state.agent_scheduler.list()
    );
    for cancel in [&known_cancel, &driven_cancel] {
        assert!(
            cancel.load(Ordering::Relaxed),
            "a cleared agent's loop driver was not told to stop"
        );
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !state.cognitive_runtime.loop_agents().is_empty() {
        assert!(
            std::time::Instant::now() < deadline,
            "a cleared agent's loop was left: {:?}",
            state.cognitive_runtime.loop_agents()
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    // The records are gone, and with them any authority to run.
    assert!(state.supervisor.lock().unwrap().health_check().is_empty());
    for agent in [&looping, &scheduled, &driven, &acting, &known] {
        for spelling in [agent.clone(), agent.to_lowercase()] {
            assert!(
                crate::commands::cognitive::agent_stopped(&state, &spelling),
                "a cleared agent kept authority to run"
            );
        }
    }
    server.set_nonblocking(true).unwrap();
    assert!(server.accept().is_err(), "nothing was sent");
}

/// The confirmation window itself, live, under an isolated X display: run
/// as is under one (`xvfb-run -a cargo test ...`), the test drives the
/// window directly; run without one, it runs itself again, alone, under
/// `xvfb-run` (a missing `xvfb-run` fails it under CI). See
/// `live_confirmation_window`.
#[cfg(target_os = "linux")]
#[test]
fn the_confirmation_window_keeps_its_header_fixed_and_the_requests_content_marked() {
    const ISOLATED: &str = "NEXUS_P3_ISOLATED_DISPLAY";
    if std::env::var_os("DISPLAY").is_some() {
        live_confirmation_window();
        return;
    }
    assert!(
        std::env::var_os(ISOLATED).is_none(),
        "no display, even under xvfb-run"
    );
    let xvfb_run = std::path::Path::new("/usr/bin/xvfb-run");
    if !xvfb_run.exists() {
        // Linux CI installs Xvfb with xvfb-run (ci.yml): there a missing one
        // fails the test.
        assert!(
            std::env::var_os("CI").is_none(),
            "xvfb-run is missing: CI must show the confirmation window"
        );
        eprintln!("xvfb-run is not installed: the confirmation window is not exercised here");
        return;
    }
    let module = module_path!()
        .split_once("::")
        .map_or(module_path!(), |(_, rest)| rest);
    let name = format!(
        "{module}::the_confirmation_window_keeps_its_header_fixed_and_the_requests_content_marked"
    );
    let shown = std::process::Command::new(xvfb_run)
        .args(["-a", "-s", "-screen 0 1280x1024x24"])
        .arg(std::env::current_exe().expect("this test binary"))
        .args(["--exact", &name, "--test-threads=1", "--nocapture"])
        .env(ISOLATED, "1")
        .output()
        .expect("xvfb-run runs");
    let out = String::from_utf8_lossy(&shown.stdout);
    let err = String::from_utf8_lossy(&shown.stderr);
    assert!(
        shown.status.success(),
        "the window test failed under xvfb-run ({}):\n{out}\n{err}",
        shown.status
    );
    // It ran, and ran this test only, read from the report's own lines: the
    // window's diagnostics (GTK's) may come between its name and its result.
    if let Err(reason) = the_report_passes_this_test_alone(&out, &name) {
        panic!("{reason}:\n{out}\n{err}");
    }
}

/// Whether libtest's report of a run under `xvfb-run` shows exactly one
/// test run, `name`, with the result `ok` and a summary of one passed and
/// nothing else. Success is read only from lines that begin as libtest's
/// do; any other line (a diagnostic) is allowed between the test's name and
/// its result and never counts. The text after `test <name> ... ` on the
/// test's own line is its result only if it is exactly `ok`, `FAILED` or
/// `ignored`; any other text there, or none, is a diagnostic (GLib prints
/// its messages without a newline first), and the result is the first later
/// line that is exactly one of the three.
#[cfg(target_os = "linux")]
fn the_report_passes_this_test_alone(report: &str, name: &str) -> Result<(), String> {
    let lines: Vec<&str> = report.lines().collect();
    let at = |is: fn(&str) -> bool| -> Vec<usize> {
        (0..lines.len()).filter(|&i| is(lines[i])).collect()
    };
    if !at(|line| line == "failures:").is_empty() {
        return Err("the report lists failures".into());
    }
    let running = at(|line| {
        line.strip_prefix("running ")
            .and_then(|rest| {
                rest.strip_suffix(" tests")
                    .or_else(|| rest.strip_suffix(" test"))
            })
            .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
    });
    let [running] = running[..] else {
        return Err(format!("{} \"running\" lines, not one", running.len()));
    };
    if lines[running] != "running 1 test" {
        return Err(format!("not one test ran: {:?}", lines[running]));
    }
    let tests = at(|line| line.starts_with("test ") && line.contains(" ... "));
    let [test] = tests[..] else {
        return Err(format!("{} test lines, not one", tests.len()));
    };
    let Some(rest) = lines[test].strip_prefix(&format!("test {name} ... ")) else {
        return Err(format!("another test ran: {:?}", lines[test]));
    };
    // libtest writes the result after the name; a diagnostic printed in
    // between, on this line or after it, moves it to a later line of its own.
    let is_result = |text: &str| matches!(text, "ok" | "FAILED" | "ignored");
    let (result_at, result) = if is_result(rest) {
        (test, rest)
    } else {
        match (test + 1..lines.len()).find(|&i| is_result(lines[i])) {
            Some(i) => (i, lines[i]),
            None => return Err("the test has no result".into()),
        }
    };
    if result != "ok" {
        return Err(format!("the test's result is {result:?}, not \"ok\""));
    }
    let summaries = at(|line| line.starts_with("test result:"));
    let [summary] = summaries[..] else {
        return Err(format!("{} summaries, not one", summaries.len()));
    };
    if !lines[summary].starts_with("test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured;") {
        return Err(format!(
            "the summary is not one passed: {:?}",
            lines[summary]
        ));
    }
    if !(running < test && result_at < summary) {
        return Err("the report is out of order".into());
    }
    Ok(())
}

// The reader of the window test's report under `xvfb-run`, on plain
// reports: what passes it, and what it rejects (CI-144-01).

#[cfg(target_os = "linux")]
#[test]
fn the_window_report_reader_passes_a_clean_report() {
    let name = "governed_real_world::tests::the_confirmation_window_keeps_its_header_fixed_and_the_requests_content_marked";
    let report = format!(
        "\nrunning 1 test\ntest {name} ... ok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 567 filtered out; finished in 5.33s\n\n"
    );
    assert_eq!(the_report_passes_this_test_alone(&report, name), Ok(()));
}

#[cfg(target_os = "linux")]
#[test]
fn the_window_report_reader_passes_the_hosted_report_with_a_warning_before_ok() {
    let name = "governed_real_world::tests::the_confirmation_window_keeps_its_header_fixed_and_the_requests_content_marked";
    // Run 37934143554, job test-linux: the child's stdout as captured.
    let report = concat!(
        "running 1 test\n",
        "test governed_real_world::tests::the_confirmation_window_keeps_its_header_fixed_and_the_requests_content_marked ... \n",
        "(nexus_desktop_backend-eab892096531ff79:38472): dbind-WARNING **: 13:20:58.262: AT-SPI: Error retrieving accessibility bus address: org.freedesktop.DBus.Error.ServiceUnknown: The name org.a11y.Bus was not provided by any .service files\n",
        "ok\n",
        "\n",
        "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 567 filtered out; finished in 5.33s\n",
    );
    // The old check required this unbroken text, which the report lacks.
    assert!(!report.contains(&format!("test {name} ... ok")));
    assert_eq!(the_report_passes_this_test_alone(report, name), Ok(()));
}

#[cfg(target_os = "linux")]
#[test]
fn the_window_report_reader_rejects_zero_tests() {
    let name = "governed_real_world::tests::the_confirmation_window_keeps_its_header_fixed_and_the_requests_content_marked";
    let report = "\nrunning 0 tests\n\ntest result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 568 filtered out; finished in 0.00s\n\n";
    let reason = the_report_passes_this_test_alone(report, name).unwrap_err();
    assert!(reason.contains("not one test ran"), "{reason}");
}

#[cfg(target_os = "linux")]
#[test]
fn the_window_report_reader_rejects_two_tests() {
    let name = "governed_real_world::tests::the_confirmation_window_keeps_its_header_fixed_and_the_requests_content_marked";
    let report = format!(
        "\nrunning 2 tests\ntest {name} ... ok\ntest {name}_too ... ok\n\ntest result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 566 filtered out; finished in 5.33s\n\n"
    );
    let reason = the_report_passes_this_test_alone(&report, name).unwrap_err();
    assert!(reason.contains("not one test ran"), "{reason}");
}

#[cfg(target_os = "linux")]
#[test]
fn the_window_report_reader_rejects_another_test() {
    let name = "governed_real_world::tests::the_confirmation_window_keeps_its_header_fixed_and_the_requests_content_marked";
    let report = "\nrunning 1 test\ntest governed_real_world::tests::another_test ... ok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 567 filtered out; finished in 0.01s\n\n";
    let reason = the_report_passes_this_test_alone(report, name).unwrap_err();
    assert!(reason.contains("another test ran"), "{reason}");
}

#[cfg(target_os = "linux")]
#[test]
fn the_window_report_reader_rejects_a_failed_test() {
    let name = "governed_real_world::tests::the_confirmation_window_keeps_its_header_fixed_and_the_requests_content_marked";
    let report = format!(
        "\nrunning 1 test\ntest {name} ... FAILED\n\nfailures:\n\n---- {name} stdout ----\nthe header moved\n\nfailures:\n    {name}\n\ntest result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 567 filtered out; finished in 5.33s\n\n"
    );
    assert!(the_report_passes_this_test_alone(&report, name).is_err());
    // Without its failures list, the result alone still rejects it.
    let report = format!(
        "\nrunning 1 test\ntest {name} ... FAILED\n\ntest result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 567 filtered out; finished in 5.33s\n\n"
    );
    let reason = the_report_passes_this_test_alone(&report, name).unwrap_err();
    assert!(reason.contains("\"FAILED\""), "{reason}");
}

#[cfg(target_os = "linux")]
#[test]
fn the_window_report_reader_rejects_a_summary_with_a_failure() {
    let name = "governed_real_world::tests::the_confirmation_window_keeps_its_header_fixed_and_the_requests_content_marked";
    let report = format!(
        "\nrunning 1 test\ntest {name} ... ok\n\ntest result: ok. 1 passed; 1 failed; 0 ignored; 0 measured; 567 filtered out; finished in 5.33s\n\n"
    );
    let reason = the_report_passes_this_test_alone(&report, name).unwrap_err();
    assert!(reason.contains("the summary is not one passed"), "{reason}");
}

#[cfg(target_os = "linux")]
#[test]
fn the_window_report_reader_rejects_a_missing_summary() {
    let name = "governed_real_world::tests::the_confirmation_window_keeps_its_header_fixed_and_the_requests_content_marked";
    let report = format!("\nrunning 1 test\ntest {name} ... ok\n");
    let reason = the_report_passes_this_test_alone(&report, name).unwrap_err();
    assert!(reason.contains("0 summaries"), "{reason}");
}

#[cfg(target_os = "linux")]
#[test]
fn the_window_report_reader_rejects_a_test_with_no_result() {
    let name = "governed_real_world::tests::the_confirmation_window_keeps_its_header_fixed_and_the_requests_content_marked";
    let report = format!(
        "\nrunning 1 test\ntest {name} ... \n(diagnostic): a warning\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 567 filtered out; finished in 5.33s\n\n"
    );
    let reason = the_report_passes_this_test_alone(&report, name).unwrap_err();
    assert!(reason.contains("no result"), "{reason}");
}

#[cfg(target_os = "linux")]
#[test]
fn the_window_report_reader_rejects_two_summaries() {
    let name = "governed_real_world::tests::the_confirmation_window_keeps_its_header_fixed_and_the_requests_content_marked";
    let report = format!(
        "\nrunning 1 test\ntest {name} ... ok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 567 filtered out; finished in 5.33s\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 567 filtered out; finished in 5.33s\n\n"
    );
    let reason = the_report_passes_this_test_alone(&report, name).unwrap_err();
    assert!(reason.contains("2 summaries"), "{reason}");
}

#[cfg(target_os = "linux")]
#[test]
fn the_window_report_reader_rejects_an_ignored_test() {
    let name = "governed_real_world::tests::the_confirmation_window_keeps_its_header_fixed_and_the_requests_content_marked";
    let report = format!(
        "\nrunning 1 test\ntest {name} ... ignored\n\ntest result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 567 filtered out; finished in 0.00s\n\n"
    );
    let reason = the_report_passes_this_test_alone(&report, name).unwrap_err();
    assert!(reason.contains("\"ignored\""), "{reason}");
}

#[cfg(target_os = "linux")]
#[test]
fn the_window_report_reader_rejects_ok_only_inside_a_diagnostic() {
    let name = "governed_real_world::tests::the_confirmation_window_keeps_its_header_fixed_and_the_requests_content_marked";
    let report = format!(
        "\nrunning 1 test\ntest {name} ... \n(diagnostic): echo: test {name} ... ok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 567 filtered out; finished in 5.33s\n\n"
    );
    // The old check would have found its text here.
    assert!(report.contains(&format!("test {name} ... ok")));
    let reason = the_report_passes_this_test_alone(&report, name).unwrap_err();
    assert!(reason.contains("no result"), "{reason}");
}

#[cfg(target_os = "linux")]
#[test]
fn the_window_report_reader_passes_a_diagnostic_on_the_test_line() {
    let name = "governed_real_world::tests::the_confirmation_window_keeps_its_header_fixed_and_the_requests_content_marked";
    // GLib prints message-level output without a newline first.
    let report = format!(
        "\nrunning 1 test\ntest {name} ... Gtk-Message: 13:20:58.262: Failed to load module \"canberra-gtk-module\"\nok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 567 filtered out; finished in 5.33s\n\n"
    );
    assert_eq!(the_report_passes_this_test_alone(&report, name), Ok(()));
}

#[cfg(target_os = "linux")]
#[test]
fn the_window_report_reader_rejects_a_diagnostic_on_the_test_line_then_failed() {
    let name = "governed_real_world::tests::the_confirmation_window_keeps_its_header_fixed_and_the_requests_content_marked";
    let report = format!(
        "\nrunning 1 test\ntest {name} ... Gtk-Message: 13:20:58.262: Failed to load module \"canberra-gtk-module\"\nFAILED\n\ntest result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 567 filtered out; finished in 5.33s\n\n"
    );
    let reason = the_report_passes_this_test_alone(&report, name).unwrap_err();
    assert!(reason.contains("\"FAILED\""), "{reason}");
}

#[cfg(target_os = "linux")]
#[test]
fn the_window_report_reader_rejects_a_diagnostic_on_the_test_line_and_no_result() {
    let name = "governed_real_world::tests::the_confirmation_window_keeps_its_header_fixed_and_the_requests_content_marked";
    let report = format!(
        "\nrunning 1 test\ntest {name} ... Gtk-Message: 13:20:58.262: Failed to load module \"canberra-gtk-module\"\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 567 filtered out; finished in 5.33s\n\n"
    );
    let reason = the_report_passes_this_test_alone(&report, name).unwrap_err();
    assert!(reason.contains("no result"), "{reason}");
}

#[cfg(target_os = "linux")]
#[test]
fn the_window_report_reader_rejects_two_test_lines_under_one_running() {
    let name = "governed_real_world::tests::the_confirmation_window_keeps_its_header_fixed_and_the_requests_content_marked";
    let report = format!(
        "\nrunning 1 test\ntest {name} ... ok\ntest {name}_too ... ok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 567 filtered out; finished in 5.33s\n\n"
    );
    let reason = the_report_passes_this_test_alone(&report, name).unwrap_err();
    assert!(reason.contains("2 test lines"), "{reason}");
}

#[cfg(target_os = "linux")]
#[test]
fn the_window_report_reader_rejects_a_summary_before_the_test() {
    let name = "governed_real_world::tests::the_confirmation_window_keeps_its_header_fixed_and_the_requests_content_marked";
    let report = format!(
        "\nrunning 1 test\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 567 filtered out; finished in 5.33s\ntest {name} ... ok\n\n"
    );
    let reason = the_report_passes_this_test_alone(&report, name).unwrap_err();
    assert!(reason.contains("out of order"), "{reason}");
}

/// Drive the production confirmation window (`owner_window`) under a
/// display. With `NEXUS_P3_DIALOG_SHOT` set, screenshots are saved there.
///
/// 1. The audit's production-path spoof: content quoted by the crate's own
///    `quoted`, padded with `U+3000` and carrying a fake header for another
///    destination. At the moment Allow arms (delay passed, end and right
///    edge of the details reached), the real target is in view in the
///    header, which has not moved and is in no scrolled view; no header row
///    carries the fake destination; the details sit below the header in
///    their captioned frame, beside a gutter bar that spans their view; and
///    every detail line keeps its marker.
/// 2. Content that a dialog would re-wrap (a long padded line, a
///    right-to-left line) keeps every line whole, starting at the left edge
///    with its marker, and the answer arms only after the end and the
///    right edge of the details.
#[cfg(target_os = "linux")]
fn live_confirmation_window() {
    use gtk::gdk::prelude::*;
    use gtk::prelude::*;
    use nexus_governed_control::authority::approval::{ActionConfirmation, ConfirmationText};
    use nexus_governed_control::authority::effect::{CapabilityKind, EffectClass};
    use nexus_governed_control::authority::evidence::quoted;
    gtk::init().expect("a display");
    let spin = |ms: u64| {
        let until = std::time::Instant::now() + std::time::Duration::from_millis(ms);
        while std::time::Instant::now() < until {
            while gtk::events_pending() {
                gtk::main_iteration();
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    };
    let shot = |dialog: &gtk::Dialog, name: &str| {
        if let Ok(base) = std::env::var("NEXUS_P3_DIALOG_SHOT") {
            let window = dialog.window().expect("mapped");
            let (width, height) = (window.width(), window.height());
            let pixbuf = window.pixbuf(0, 0, width, height).expect("pixels");
            pixbuf
                .savev(format!("{base}-{name}.png"), "png", &[])
                .expect("saved");
        }
    };
    /// The window's parts, found by their kinds.
    struct Parts {
        header: gtk::Grid,
        frame: gtk::Frame,
        gutter: gtk::Separator,
        scroll: gtk::ScrolledWindow,
        details: gtk::Label,
    }
    let parts = |dialog: &gtk::Dialog| {
        let children = dialog.content_area().children();
        let header = children
            .iter()
            .find_map(|c| c.clone().downcast::<gtk::Grid>().ok())
            .expect("the header");
        let frame = children
            .iter()
            .find_map(|c| c.clone().downcast::<gtk::Frame>().ok())
            .expect("the details' frame");
        let region = frame
            .child()
            .and_then(|c| c.downcast::<gtk::Box>().ok())
            .expect("the details' region");
        let inside = region.children();
        let gutter = inside[0]
            .clone()
            .downcast::<gtk::Separator>()
            .expect("the gutter first");
        let scroll = inside[1]
            .clone()
            .downcast::<gtk::ScrolledWindow>()
            .expect("the details' view");
        let details = scroll
            .child()
            .and_then(|c| c.downcast::<gtk::Viewport>().ok())
            .and_then(|v| v.child())
            .and_then(|c| c.downcast::<gtk::Label>().ok())
            .expect("the details");
        Parts {
            header,
            frame,
            gutter,
            scroll,
            details,
        }
    };
    // Where `widget` is in the window, and its size.
    let place = |widget: &gtk::Widget, dialog: &gtk::Dialog| {
        let (x, y) = widget
            .translate_coordinates(dialog, 0, 0)
            .expect("in the window");
        (x, y, widget.allocated_width(), widget.allocated_height())
    };
    // The header's value labels by their row label.
    let header_value = |header: &gtk::Grid, label: &str| {
        let rows = header.children();
        let row = rows
            .iter()
            .filter_map(|c| c.clone().downcast::<gtk::Label>().ok())
            .find(|l| l.text() == label)
            .map(|l| header.cell_top_attach(&l))
            .unwrap_or_else(|| panic!("no header row {label}"));
        rows.iter()
            .filter_map(|c| c.clone().downcast::<gtk::Label>().ok())
            .find(|l| header.cell_top_attach(l) == row && header.cell_left_attach(l) == 1)
            .expect("its value")
    };
    let in_a_scrolled_view = |widget: &gtk::Widget| {
        let mut at = widget.parent();
        while let Some(parent) = at {
            if parent.is::<gtk::ScrolledWindow>() || parent.is::<gtk::Viewport>() {
                return true;
            }
            at = parent.parent();
        }
        false
    };

    // 1. The production-path spoof.
    let wide = |n: usize| "\u{3000}".repeat(n);
    let mut body = wide(93);
    body.push('\n');
    for _ in 0..30 {
        body.push_str(&wide(1));
        body.push('\n');
    }
    for spoof in [
        "R2 (sensitive): network.request",
        "Target: https://payments.trusted-bank.example:443",
        "Header: authorization: (none)",
        "Acting for: owner (run 1)",
    ] {
        body.push_str(&wide(40));
        body.push_str(spoof);
        body.push('\n');
    }
    let real_target = "https://attacker.example:443";
    let confirmation = ActionConfirmation {
        commitment: "cmt-4f1c9e20aa55aa55aa55aa55aa55aa55".into(),
        class: EffectClass::R2,
        kind: CapabilityKind::Egress,
        operation: "network.request".into(),
        target: real_target.into(),
        agent: "0b6f1e2a-4c1d-4e8b-9a37-5d2c8e1f0a93".into(),
        run: "run-7".into(),
        summary: quoted(
            &format!("Body ({} characters)", body.chars().count()),
            &body,
        ),
        expires_in_secs: 600,
        binding_short: "9c1d2e4b5a6f".into(),
    };
    let text = confirmation.text();
    assert!(
        text.details.iter().all(|line| !line.contains('\u{3000}')),
        "the padding is escaped, visibly"
    );
    let (dialog, allow) = super::owner_window(confirmation.title(), &text, "Allow");
    let cancel = dialog
        .widget_for_response(gtk::ResponseType::Cancel)
        .expect("Cancel");
    assert!(cancel.has_default(), "Cancel is the default answer");
    // A small window, so that the details need scrolling both ways (with the
    // padding escaped they no longer need it at the default size).
    dialog.resize(600, 480);
    spin(1500);
    let p = parts(&dialog);
    shot(&dialog, "spoof-before");
    assert!(!allow.is_sensitive(), "unarmed until the details were read");
    let header_widget: gtk::Widget = p.header.clone().upcast();
    let header_at = place(&header_widget, &dialog);
    // The details' view scrolls both ways here, as the spoof needed.
    let (down, across) = (p.scroll.vadjustment(), p.scroll.hadjustment());
    assert!(down.upper() > down.page_size() && across.upper() > across.page_size());
    down.set_value(down.upper());
    spin(300);
    across.set_value(across.upper());
    spin(300);
    assert!(
        allow.is_sensitive(),
        "armed after the end, the edge and the delay"
    );
    shot(&dialog, "spoof-armed");
    // At the armed view: the header is where it was, in view, in no
    // scrolled view, and shows the real target whole.
    assert!(!in_a_scrolled_view(&header_widget));
    assert!(p.header.is_mapped() && p.header.is_visible());
    assert_eq!(
        place(&header_widget, &dialog),
        header_at,
        "the header moved"
    );
    let (hx, hy, hw, hh) = header_at;
    let (dw, dh) = (dialog.allocated_width(), dialog.allocated_height());
    assert!(
        hx >= 0 && hy >= 0 && hx + hw <= dw && hy + hh <= dh,
        "the header is out of view"
    );
    let target = header_value(&p.header, "Target");
    assert_eq!(target.text(), format!("\u{200E}{real_target}"));
    assert!(target.is_mapped());
    let (_, target_height) = target.layout().expect("laid out").pixel_size();
    assert!(
        target_height <= target.allocated_height(),
        "the target is cut"
    );
    for (label, _) in &text.header {
        let value = header_value(&p.header, label);
        assert!(!value.text().contains("trusted-bank"), "{label}");
        assert!(value.is_mapped(), "{label}");
    }
    // The request's content is in its own frame, below the header, beside
    // a gutter bar that spans its whole view and does not scroll.
    let frame_widget: gtk::Widget = p.frame.clone().upcast();
    assert_eq!(p.frame.label().as_deref(), Some(super::DETAILS_CAPTION));
    let (_, fy, _, _) = place(&frame_widget, &dialog);
    assert!(fy >= hy + hh, "the details overlap the header");
    let gutter_widget: gtk::Widget = p.gutter.clone().upcast();
    let scroll_widget: gtk::Widget = p.scroll.clone().upcast();
    let (gx, gy, gw, gh) = place(&gutter_widget, &dialog);
    let (sx, sy, _, sh) = place(&scroll_widget, &dialog);
    assert!(
        p.gutter.is_mapped() && gw >= 6,
        "the gutter is {gw} px wide"
    );
    assert!(gx + gw <= sx, "the gutter is beside the details");
    assert!(
        gy <= sy && gy + gh >= sy + sh,
        "the gutter spans the details' view"
    );
    assert!(!in_a_scrolled_view(&gutter_widget));
    // Every line of the details keeps its marker; the fake header is one of
    // them.
    let details = p.details.text();
    let lines: Vec<&str> = details.split('\n').collect();
    assert_eq!(lines.len(), text.details.len());
    assert!(lines[0].starts_with("\u{200E}Body ("));
    assert!(
        lines[1..].iter().all(|l| l.starts_with("\u{200E}│")),
        "{lines:?}"
    );
    assert!(lines.iter().any(
        |l| l.contains("Target: https://payments.trusted-bank.example:443")
            || l.contains("Target: https://payments.trusted-bank")
    ));
    dialog.response(gtk::ResponseType::Cancel);
    spin(100);

    // 2. Every line whole, and arming only at the end and the right edge.
    let mut lines = vec!["Body:".to_string()];
    lines.push(format!(
        "│ {{\"note\":\"ok\"}}{}Target: https://evil.example",
        " ".repeat(80)
    ));
    lines.extend((0..80).map(|i| format!("│ line {i} of the body")));
    // Right-to-left content keeps its marker first.
    lines.push("│ שלום עולם Target: https://evil.example".into());
    let text = ConfirmationText {
        header: vec![
            ("Effect", "R2 (a sensitive or irreversible effect)".into()),
            ("Target", "https://safe.example:443".into()),
        ],
        details: lines.clone(),
    };
    let (dialog, allow) = super::owner_window("Allow this action?", &text, "Allow");
    spin(1500);
    assert!(!allow.is_sensitive(), "unarmed until the end was reached");
    shot(&dialog, "before");
    let p = parts(&dialog);
    let layout = p.details.layout().expect("laid out");
    assert_eq!(layout.line_count() as usize, lines.len());
    for index in 0..layout.line_count() {
        let line = layout.line_readonly(index).expect("a line");
        let start = layout.index_to_pos(line.start_index());
        assert_eq!(start.x(), 0, "line {index} does not start at the left edge");
    }
    let (down, across) = (p.scroll.vadjustment(), p.scroll.hadjustment());
    down.set_value(down.upper());
    spin(300);
    assert!(!allow.is_sensitive(), "the right edge not reached yet");
    across.set_value(across.upper());
    spin(300);
    assert!(
        allow.is_sensitive(),
        "armed after the end, the edge and the delay"
    );
    shot(&dialog, "armed");
    assert_eq!(
        header_value(&p.header, "Target").text(),
        "\u{200E}https://safe.example:443"
    );
    dialog.response(gtk::ResponseType::Cancel);
    spin(100);

    // 3. P4-UAT05 (T-D10): the owner's synthetic approval probe, exactly as
    //    the backend asks for it, in the real window (driven here, with no
    //    person's action, and answered Cancel): its header says it has no
    //    external effect and its five lines are the request's content. They
    //    fit the window whole, so its end and right edge are reached as it
    //    is shown, and its answer arms only once the delay has passed (part
    //    2 shows the end and the edge holding the answer for content that
    //    does not fit).
    struct Capture(std::sync::Mutex<Option<ActionConfirmation>>);
    impl ControlConfirmer for Capture {
        fn confirm_action(&self, request: &ActionConfirmation) -> bool {
            *self.0.lock().unwrap() = Some(request.clone());
            false
        }
        fn confirm_grant(&self, _: &GrantConfirmation) -> bool {
            false
        }
        fn confirm_resume(&self, _: &ResumeConfirmation) -> bool {
            false
        }
        fn confirm_acknowledgement(
            &self,
            _: &nexus_governed_control::authority::approval::AcknowledgementConfirmation,
        ) -> bool {
            false
        }
    }
    let t = isolated();
    let commitment = granted_probe(&t.world);
    let id = commitment["id"].as_str().unwrap().to_string();
    let capture = Capture(std::sync::Mutex::new(None));
    let declined = t.world.approve(&id, &capture).unwrap_err();
    assert!(declined.contains("declined"), "{declined}");
    let shown = capture
        .0
        .lock()
        .unwrap()
        .clone()
        .expect("the backend asked");
    let text = shown.text();
    let (dialog, allow) = super::owner_window(shown.title(), &text, "Allow");
    spin(300);
    assert!(!allow.is_sensitive(), "armed before the delay");
    let p = parts(&dialog);
    for (label, value) in [
        ("Effect", "Synthetic R2-approval test: no external effect"),
        ("Operation", "synthetic.receipt"),
        (
            "Target",
            "none (synthetic test receipt; no external target)",
        ),
        ("Acting for", "owner-session"),
        ("Commitment", id.as_str()),
    ] {
        assert_eq!(
            header_value(&p.header, label).text(),
            format!("\u{200E}{value}"),
            "{label}"
        );
    }
    let details = p.details.text();
    let lines: Vec<String> = details.split('\n').map(str::to_string).collect();
    let summary: Vec<String> = commitment["summary"]
        .as_array()
        .unwrap()
        .iter()
        .map(|line| format!("\u{200E}{}", line.as_str().unwrap()))
        .collect();
    assert_eq!(lines.len(), 5);
    assert_eq!(lines, summary);
    // Whole in view: its end and right edge are where it is shown.
    let (down, across) = (p.scroll.vadjustment(), p.scroll.hadjustment());
    assert!(down.value() + down.page_size() >= down.upper() - 1.0);
    assert!(across.value() + across.page_size() >= across.upper() - 1.0);
    assert!(!allow.is_sensitive(), "armed before the delay");
    spin(1200);
    assert!(allow.is_sensitive(), "armed after the delay");
    shot(&dialog, "synthetic-armed");
    dialog.response(gtk::ResponseType::Cancel);
    spin(100);
    assert_eq!(commitment_state(&t.world, &id), "denied");
}

/// P3-HM-CONTRACT-02 §13.2 LK-3 (mission P3-C11-HM-M4 N5): the effect the
/// cognitive loop records for an external step it refuses as stale comes
/// from the Phase Three pipeline's own answer, through the production
/// executor: a done action executed; an R2 commitment left for the owner's
/// approval unknown (it has not run, and may yet); a refused action failed.
#[test]
fn m4_n5_an_external_steps_effect_comes_from_the_pipelines_own_answer() {
    use nexus_kernel::cognitive::loop_runtime::{ActionExecutor, StepEffect};
    use nexus_kernel::cognitive::PlannedAction;
    use std::io::{Read, Write};

    // A loopback page server that answers one request.
    let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", server.local_addr().unwrap());
    std::thread::spawn(move || {
        for mut stream in server.incoming().flatten().take(1) {
            let mut request = [0u8; 4096];
            let _ = stream.read(&mut request);
            let _ = stream.write_all(
                b"HTTP/1.1 200 OK\r\nContent-Length: 8\r\nConnection: close\r\n\r\ngoverned",
            );
        }
    });
    let t = isolated();
    let state = crate::AppState::new_in_memory();
    let memory = Arc::new(nexus_kernel::cognitive::AgentMemoryManager::new(Box::new(
        crate::DbMemoryStore {
            db: state.db.clone(),
        },
    )));
    let executor = crate::phase0_agent_executor(
        &state,
        memory,
        crate::hm_dispatch::GatewayPlannerLlm::new(&state),
    )
    .with_real_world(t.world.clone());
    t.world
        .request_grant(
            &GrantRequest::Egress {
                origin: origin.clone(),
                methods: vec!["GET".into(), "POST".into()],
                allow_private: true,
            },
            60,
            &Owner(true, AtomicU32::new(0)),
        )
        .unwrap();
    let mut audit = state.audit.clone();
    let agent = uuid::Uuid::new_v4().to_string();
    let mut effect_of = |action: &PlannedAction| {
        let output = executor.execute(&agent, action, &mut audit, true);
        (
            executor.step_effect(action, output.as_deref().map_err(String::as_str)),
            output,
        )
    };

    let fetch = PlannedAction::WebFetch {
        url: format!("{origin}/page"),
    };
    let (effect, output) = effect_of(&fetch);
    assert_eq!(effect, StepEffect::Executed, "{output:?}");

    let post = PlannedAction::ApiCall {
        method: "POST".into(),
        url: format!("{origin}/x"),
        body: Some("{}".into()),
        headers: None,
    };
    let (effect, output) = effect_of(&post);
    let answer: Value = serde_json::from_str(output.as_deref().unwrap()).unwrap();
    assert!(
        answer["awaiting_owner_approval"].is_string(),
        "precondition: left for the owner's approval: {answer}"
    );
    assert_eq!(effect, StepEffect::Unknown, "{answer}");

    let ungranted = PlannedAction::WebFetch {
        url: "http://127.0.0.1:9/never".into(),
    };
    let (effect, output) = effect_of(&ungranted);
    assert!(output.is_err(), "precondition: refused: {output:?}");
    assert_eq!(effect, StepEffect::Failed, "{output:?}");
}

// ── P3-C11-M5 N3: the evidence of record (contract EL-1 §1, §3, §6, §8) ────

/// A private test home (AM2 of the mission, AM2.1): a fresh directory under
/// /tmp, mode 0700 set at creation (never the umask's), asserted not to be
/// the user's real home and to start empty.
fn private_home() -> std::path::PathBuf {
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
    let home = std::env::temp_dir().join(format!("nexus-p3-home-{}", uuid::Uuid::new_v4()));
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&home)
        .unwrap();
    let meta = std::fs::metadata(&home).unwrap();
    assert_eq!(meta.permissions().mode() & 0o777, 0o700);
    let passwd = std::fs::read_to_string("/etc/passwd").unwrap();
    let real = passwd
        .lines()
        .map(|line| line.split(':').collect::<Vec<_>>())
        .find(|f| f.len() > 5 && f[2] == meta.uid().to_string())
        .map(|f| std::path::PathBuf::from(f[5]))
        .expect("the user's passwd entry");
    assert_ne!(home, real, "the private home is the real home");
    assert!(
        !home.starts_with(&real),
        "the private home is under the real home"
    );
    assert!(
        std::fs::read_dir(&home).unwrap().next().is_none(),
        "not empty"
    );
    home
}

const SETUP_CHILD: &str = "NEXUS_P3_EL1_SETUP_CHILD";

/// The child of `p3_el1_n3_setup_opens_the_store_or_governed_control_is_unavailable`:
/// under its own HOME (a private test home) it runs `RealWorld::setup` and
/// reports what it found. Without its task it does nothing.
#[test]
#[ignore = "run by p3_el1_n3_setup_opens_the_store_or_governed_control_is_unavailable"]
fn p3_el1_n3_setup_child() {
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    let Ok(mode) = std::env::var(SETUP_CHILD) else {
        return;
    };
    let state = nexus_kernel::identity_home::nexus_state_dir().unwrap();
    let store = state.join("p3-evidence").join("evidence.sqlite3");
    if mode == "unqualified" {
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(store.parent().unwrap())
            .unwrap();
        std::fs::write(&store, b"not an evidence store").unwrap();
    }
    let audit = Arc::new(Mutex::new(AuditTrail::new()));
    let db = Arc::new(NexusDatabase::in_memory().unwrap());
    match RealWorld::setup(audit.clone(), db) {
        Ok(world) => {
            let mode = std::fs::metadata(store.parent().unwrap())
                .unwrap()
                .permissions()
                .mode();
            println!(
                "SETUP ok file={} dir_mode={:o} evidence={}",
                store.is_file(),
                mode & 0o777,
                world.status()["evidence"]
            );
        }
        Err(error) => println!(
            "SETUP err unchanged={} runtime={} audited={} error={error}",
            std::fs::read(&store).ok().as_deref() == Some(b"not an evidence store".as_slice()),
            state.join("p3-runtime").exists(),
            evidence_events(&audit)
        ),
    }
}

/// Run the setup child under a fresh private home; its report line.
fn setup_child(mode: &str) -> String {
    let home = private_home();
    let out = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "governed_real_world::tests::p3_el1_n3_setup_child",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("HOME", &home)
        .env(SETUP_CHILD, mode)
        .output()
        .unwrap();
    std::fs::remove_dir_all(&home).unwrap();
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    text.lines()
        .find_map(|l| l.find("SETUP ").map(|at| l[at..].to_string()))
        .unwrap_or_else(|| panic!("the child reported nothing: {text}"))
}

/// EL-1 §1, §3, §6 (AM2.3; mutant 4): the production `RealWorld::setup`,
/// under a private test home, (a) opens the store at
/// `<home>/.nexus/p3-evidence/evidence.sqlite3` in a 0700 directory and
/// reports it qualified; (b) with an unqualifiable file there, returns an
/// error: the file is unchanged, no control is built (no runtime root), and
/// nothing is written to the audit trail instead.
#[test]
fn p3_el1_n3_setup_opens_the_store_or_governed_control_is_unavailable() {
    let clean = setup_child("clean");
    assert_eq!(
        clean,
        "SETUP ok file=true dir_mode=700 evidence={\"owed\":0,\"recovery_required\":false,\
         \"store\":\"qualified\",\"unclean_sessions\":0,\"unsettled\":0}",
        "{clean}"
    );
    let refused = setup_child("unqualified");
    assert!(
        refused.starts_with("SETUP err unchanged=true runtime=false audited=0 error=")
            && refused.contains("evidence store unqualified"),
        "{refused}"
    );
}

/// EL-1 §8: an effect's result says what became of its records; a
/// commitment that never started keeps today's exact result.
#[test]
fn p3_el1_n3_a_result_carries_its_evidence() {
    let t = isolated();
    let owner = Owner(true, AtomicU32::new(0));
    t.world
        .request_grant(
            &GrantRequest::Tool {
                tool: "text.sha256".into(),
            },
            60,
            &owner,
        )
        .unwrap();
    let submit = |text: &str| {
        t.world
            .submit(CommandEnvelope {
                text: Some(text.into()),
                ..Default::default()
            })
            .unwrap()["commitment"]["id"]
            .as_str()
            .unwrap()
            .to_string()
    };
    let output = t.world.approve(&submit("hash abc"), &owner).unwrap();
    let evidence = &output["evidence"];
    assert_eq!(evidence["start"]["status"], "recorded");
    assert_eq!(evidence["end"]["status"], "recorded");
    assert!(evidence["end"]["seq"].as_u64().unwrap() > evidence["start"]["seq"].as_u64().unwrap());
    assert_eq!(
        output.as_object().unwrap().keys().collect::<Vec<_>>(),
        ["bytes", "evidence", "meta", "text"]
    );
    assert_eq!(t.world.status()["evidence"]["store"], "qualified");
    // Denied, it never started: its result is today's, exactly.
    let denied = submit("hash abc");
    t.world.deny(&denied).unwrap();
    assert_eq!(
        t.world.approve(&denied, &owner).unwrap_err(),
        "governed control: refused: not_pending"
    );
}

// ── P3-C11-M5 N4: sessions, Recovery required, the acknowledgement (EL-1 §7) ─

/// The owner's scripted answers to an acknowledgement and to a resume, and
/// what was shown.
struct Recovering {
    acknowledge: bool,
    resume: bool,
    acknowledgements:
        Mutex<Vec<nexus_governed_control::authority::approval::AcknowledgementConfirmation>>,
    resumes: AtomicU32,
}

impl Recovering {
    fn answering(acknowledge: bool, resume: bool) -> Self {
        Self {
            acknowledge,
            resume,
            acknowledgements: Mutex::new(Vec::new()),
            resumes: AtomicU32::new(0),
        }
    }
}

impl ControlConfirmer for Recovering {
    fn confirm_action(&self, _: &ActionConfirmation) -> bool {
        false
    }
    fn confirm_grant(&self, _: &GrantConfirmation) -> bool {
        false
    }
    fn confirm_resume(&self, _: &ResumeConfirmation) -> bool {
        self.resumes.fetch_add(1, Ordering::SeqCst);
        self.resume
    }
    fn confirm_acknowledgement(
        &self,
        request: &nexus_governed_control::authority::approval::AcknowledgementConfirmation,
    ) -> bool {
        self.acknowledgements.lock().unwrap().push(request.clone());
        self.acknowledge
    }
}

const RECOVERY_CHILD: &str = "NEXUS_P3_EL1_RECOVERY_CHILD";

/// The child of `p3_el1_n4_a_crash_mid_effect_is_acknowledged_through_p3_resume`:
/// over the root it is given, an agent's governed request to an upstream
/// that accepts and never answers starts (its start record committed); it
/// reports `READY` and waits to be killed. Without its task it does nothing.
#[test]
#[ignore = "run by p3_el1_n4_a_crash_mid_effect_is_acknowledged_through_p3_resume"]
fn p3_el1_n4_recovery_child() {
    let Ok(root) = std::env::var(RECOVERY_CHILD) else {
        return;
    };
    let audit = Arc::new(Mutex::new(AuditTrail::new()));
    let db = Arc::new(NexusDatabase::in_memory().unwrap());
    let world = RealWorld::for_tests(std::path::Path::new(&root), audit, db);
    let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", server.local_addr().unwrap());
    world
        .request_grant(
            &GrantRequest::Egress {
                origin: origin.clone(),
                methods: vec!["GET".into()],
                allow_private: true,
            },
            60,
            &Owner(true, AtomicU32::new(0)),
        )
        .unwrap();
    let acting = world.clone();
    std::thread::spawn(move || {
        let bridge = AgentBridge::with_warden(acting, || false);
        let get = Intent::Request(nexus_governed_control::egress::EgressIntent {
            method: "GET".into(),
            url: format!("{origin}/never"),
            headers: vec![],
            body: None,
        });
        bridge.act(&uuid::Uuid::new_v4().to_string(), &get, true)
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    let executing = loop {
        let found = world.status()["commitments"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["state"] == "executing")
            .map(|c| c["id"].as_str().unwrap().to_string());
        if let Some(id) = found {
            break id;
        }
        assert!(std::time::Instant::now() < deadline, "nothing executed");
        std::thread::sleep(std::time::Duration::from_millis(5));
    };
    {
        use std::io::Write as _;
        let mut out = std::io::stdout().lock();
        writeln!(out, "READY {executing}").unwrap();
        out.flush().unwrap();
    }
    for _ in 0..1200 {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    drop(server);
}

/// Run the recovery child over `root`, kill it (SIGKILL, through its own
/// handle) once its effect has started, and reap it; the commitment.
fn crash_mid_effect(root: &std::path::Path) -> String {
    use std::io::BufRead as _;
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "governed_real_world::tests::p3_el1_n4_recovery_child",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(RECOVERY_CHILD, root)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::inherit())
        .spawn()
        .unwrap();
    let stdout = child.stdout.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    let reader = std::thread::spawn(move || {
        for line in std::io::BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            if let Some(at) = line.find("READY ") {
                let _ = tx.send(line[at + "READY ".len()..].trim().to_string());
                break;
            }
        }
    });
    let started = rx
        .recv_timeout(std::time::Duration::from_secs(120))
        .expect("the child's effect started");
    child.kill().unwrap();
    let status = child.wait().unwrap();
    let _ = reader.join();
    assert!(!status.success(), "the child was killed");
    started
}

/// The store's rows as (seq, phase, commitment, body), read through a
/// read-only connection once nothing holds the file.
fn store_rows(root: &std::path::Path) -> Vec<(i64, String, Option<String>, String)> {
    let conn = rusqlite::Connection::open_with_flags(
        super::evidence_path(root),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap();
    let mut statement = conn
        .prepare("SELECT seq, phase, commitment, body FROM record ORDER BY seq")
        .unwrap();
    statement
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
}

/// EL-1 §7 through the desktop (mutants 10, 13, 14): a process killed while
/// an effect runs leaves one unsettled action; at the next start governed
/// control is emergency-stopped with recovery required. `p3_resume`
/// (`RealWorld::resume`) asks the owner's acknowledgement first: declined,
/// nothing changes and the resume is not asked; confirmed, the store writes
/// `acknowledged{covers_through: N}` (N the highest statement shown) and
/// only then is the existing resume confirmation asked, which lifts the
/// stop.
#[test]
fn p3_el1_n4_a_crash_mid_effect_is_acknowledged_through_p3_resume() {
    let root = private_home();
    let started = crash_mid_effect(&root);
    let audit = Arc::new(Mutex::new(AuditTrail::new()));
    let db = Arc::new(NexusDatabase::in_memory().unwrap());
    let world = RealWorld::for_tests(&root, audit, db);
    let status = world.status();
    assert_eq!(status["status"]["emergency_stopped"], true);
    assert_eq!(status["evidence"]["recovery_required"], true);
    assert_eq!(status["evidence"]["unsettled"], 1);
    assert_eq!(status["evidence"]["unclean_sessions"], 1);
    // Nothing new is admitted.
    let owner = Owner(true, AtomicU32::new(0));
    assert!(world
        .request_grant(
            &GrantRequest::Tool {
                tool: "text.sha256".into()
            },
            60,
            &owner
        )
        .is_err());
    // Declined: nothing changes, the resume is never asked.
    let no = Recovering::answering(false, true);
    assert!(world.resume(&no).is_err());
    assert_eq!(no.resumes.load(Ordering::SeqCst), 0);
    assert_eq!(world.status()["evidence"]["recovery_required"], true);
    assert_eq!(world.status()["status"]["emergency_stopped"], true);
    // Confirmed: acknowledged, then the existing resume.
    let yes = Recovering::answering(true, true);
    world.resume(&yes).expect("acknowledged and resumed");
    let shown = yes.acknowledgements.lock().unwrap().clone();
    assert_eq!(shown.len(), 1);
    assert_eq!(
        shown[0].unsettled,
        [started.chars().take(12).collect::<String>()]
    );
    assert_eq!(yes.resumes.load(Ordering::SeqCst), 1);
    assert_eq!(world.status()["evidence"]["recovery_required"], false);
    assert_eq!(world.status()["status"]["emergency_stopped"], false);
    world.shutdown();
    drop(world);
    let rows = store_rows(&root);
    let unsettled: Vec<_> = rows.iter().filter(|r| r.1 == "unsettled").collect();
    assert_eq!(unsettled.len(), 1);
    assert_eq!(unsettled[0].2.as_deref(), Some(started.as_str()));
    let shown_through = rows
        .iter()
        .filter(|r| r.1 == "unsettled" || r.1 == "unclean_session")
        .map(|r| r.0)
        .max()
        .unwrap();
    assert_eq!(
        shown[0].covers_through,
        u64::try_from(shown_through).unwrap()
    );
    let acknowledged: Vec<serde_json::Value> = rows
        .iter()
        .filter(|r| r.1 == "acknowledged")
        .map(|r| serde_json::from_str(&r.3).unwrap())
        .collect();
    assert_eq!(acknowledged.len(), 1);
    assert_eq!(acknowledged[0]["covers_through"], shown_through);
    // The start stopped governed control through the stop, saying why; the
    // clean shutdown closed its session.
    assert!(rows
        .iter()
        .any(|r| r.1 == "emergency_stop" && r.3.contains("\"reason\":\"recovery_required\"")));
    assert_eq!(rows.last().unwrap().1, "session_closed");
    std::fs::remove_dir_all(&root).unwrap();
}

/// EL-1 §7: a clean shutdown writes `session_closed`; the next start reports
/// no unclean session and requires no recovery.
#[test]
fn p3_el1_n4_a_clean_shutdown_leaves_no_unclean_session() {
    let root = private_home();
    for _ in 0..2 {
        let audit = Arc::new(Mutex::new(AuditTrail::new()));
        let db = Arc::new(NexusDatabase::in_memory().unwrap());
        let world = RealWorld::for_tests(&root, audit, db);
        assert_eq!(world.status()["evidence"]["unclean_sessions"], 0);
        assert_eq!(world.status()["evidence"]["recovery_required"], false);
        assert_eq!(world.status()["status"]["emergency_stopped"], false);
        world.shutdown();
    }
    let phases: Vec<String> = store_rows(&root).into_iter().map(|r| r.1).collect();
    assert_eq!(
        phases,
        [
            "session_opened",
            "session_closed",
            "session_opened",
            "session_closed"
        ]
    );
    std::fs::remove_dir_all(&root).unwrap();
}

// ── P3-C11-M6: corrections to EL-1 (desktop parts of C-2 and C-7) ──────────

/// EL-1 §6, §8 as amended by C-2 (P3-C11-M6): a result whose end record is
/// uncertain (the store latched) is shown as `"end": {"status":
/// "uncertain"}`, never as recorded.
#[test]
fn p3_el1_m6_c2_an_uncertain_end_is_shown_uncertain() {
    use nexus_governed_control::authority::evidence::{EndStatus, ExecutionEvidence};
    let output = nexus_governed_control::control::EffectOutput {
        text: Some("done".into()),
        evidence: Some(ExecutionEvidence {
            start: 7,
            end: EndStatus::Uncertain,
        }),
        ..Default::default()
    };
    let shown = super::output_json(&output);
    assert_eq!(shown["evidence"]["end"], json!({ "status": "uncertain" }));
    assert_eq!(
        shown["evidence"]["start"],
        json!({ "status": "recorded", "seq": 7 })
    );
}

/// EL-1 §8 as amended by C-7 (P3-C11-M6), through the desktop: after the
/// owner's acknowledgement (`p3_resume`) the status shows `unsettled` 0,
/// recovery not required, the unclean sessions unchanged; a restart after
/// it does not start stopped.
#[test]
fn p3_el1_m6_c7_after_the_acknowledgement_the_status_shows_nothing_unsettled() {
    let root = private_home();
    crash_mid_effect(&root);
    {
        let audit = Arc::new(Mutex::new(AuditTrail::new()));
        let db = Arc::new(NexusDatabase::in_memory().unwrap());
        let world = RealWorld::for_tests(&root, audit, db);
        let before = world.status()["evidence"].clone();
        assert_eq!(before["unsettled"], 1);
        assert_eq!(before["recovery_required"], true);
        world
            .resume(&Recovering::answering(true, true))
            .expect("acknowledged and resumed");
        let after = world.status()["evidence"].clone();
        assert_eq!(after["unsettled"], 0, "{after}");
        assert_eq!(after["recovery_required"], false);
        assert_eq!(after["unclean_sessions"], before["unclean_sessions"]);
        world.shutdown();
    }
    let audit = Arc::new(Mutex::new(AuditTrail::new()));
    let db = Arc::new(NexusDatabase::in_memory().unwrap());
    let world = RealWorld::for_tests(&root, audit, db);
    let status = world.status();
    assert_eq!(status["status"]["emergency_stopped"], false);
    assert_eq!(status["evidence"]["unsettled"], 0);
    assert_eq!(status["evidence"]["recovery_required"], false);
    world.shutdown();
    drop(world);
    std::fs::remove_dir_all(&root).unwrap();
}

// ── P3-C11-M6 C-9: what the desktop shows (M-B3, M-B4, M-B5) ──────────────

/// EL-1 §8 (C-9 M-B3): a result whose end record is owed is shown as
/// `"end": {"status": "owed"}`, never as recorded.
#[test]
fn p3_el1_m6_mb3_an_owed_end_is_shown_owed() {
    use nexus_governed_control::authority::evidence::{EndStatus, ExecutionEvidence};
    let output = nexus_governed_control::control::EffectOutput {
        text: Some("done".into()),
        evidence: Some(ExecutionEvidence {
            start: 7,
            end: EndStatus::Owed,
        }),
        ..Default::default()
    };
    assert_eq!(
        super::output_json(&output)["evidence"]["end"],
        json!({ "status": "owed" })
    );
}

/// EL-1 §8 (C-9 M-B4): the start shown with a result is its commitment's
/// `started` row in the store.
#[test]
fn p3_el1_m6_mb4_the_start_shown_is_the_started_row() {
    let root = private_home();
    let audit = Arc::new(Mutex::new(AuditTrail::new()));
    let db = Arc::new(NexusDatabase::in_memory().unwrap());
    let world = RealWorld::for_tests(&root, audit, db);
    let owner = Owner(true, AtomicU32::new(0));
    world
        .request_grant(
            &GrantRequest::Tool {
                tool: "text.sha256".into(),
            },
            60,
            &owner,
        )
        .unwrap();
    let id = world
        .submit(CommandEnvelope {
            text: Some("hash abc".into()),
            ..Default::default()
        })
        .unwrap()["commitment"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let output = world.approve(&id, &owner).unwrap();
    world.shutdown();
    drop(world);
    let rows = store_rows(&root);
    let started = rows
        .iter()
        .find(|r| r.1 == "started" && r.2.as_deref() == Some(id.as_str()))
        .expect("the started row")
        .0;
    assert!(started > 1);
    assert_eq!(output["evidence"]["start"]["seq"], started);
    std::fs::remove_dir_all(&root).unwrap();
}

/// EL-1 §8 (C-9 M-B5): the status says `"store": "latched"` for a latched
/// store (no desktop test can latch a real one: the status's evidence
/// object is a function of the store's health alone, and the status is
/// built from it).
#[test]
fn p3_el1_m6_mb5_a_latched_store_is_shown_latched() {
    use nexus_governed_control::evidence_store::EvidenceHealth;
    let health = |latched| EvidenceHealth {
        latched,
        owed: 2,
        recovery_required: false,
        unsettled: 1,
        unclean_sessions: 3,
    };
    assert_eq!(
        super::evidence_status(health(true)),
        json!({
            "store": "latched",
            "owed": 2,
            "recovery_required": false,
            "unsettled": 1,
            "unclean_sessions": 3,
        })
    );
    assert_eq!(super::evidence_status(health(false))["store"], "qualified");
    let t = isolated();
    assert_eq!(
        t.world.status()["evidence"],
        super::evidence_status(t.world.evidence.health())
    );
}

// ── P4-UAT05: the owner's synthetic approval probe, through the desktop ──
//
// The Owner-adopted Phase Four amendment, end to end through `RealWorld`
// (T-D1 to T-D11, N-6). The owner's answers are scripted here (the live
// window is driven in `live_confirmation_window`, with no person's action):
// these are probe-execution (PX) and window-content proofs only, never a
// physical owner's approval, a real agent's request or a real-world effect.

use nexus_governed_control::authority::approval::{OWNER_ONLY_USE, SYNTHETIC_EFFECT};
use nexus_governed_control::synthetic::{SyntheticIntent, TARGET_TEXT};

/// The owner's answer, keeping every window shown.
struct Shown {
    answer: bool,
    actions: Mutex<Vec<ActionConfirmation>>,
    grants: Mutex<Vec<GrantConfirmation>>,
}

impl Shown {
    fn new(answer: bool) -> Self {
        Self {
            answer,
            actions: Mutex::new(Vec::new()),
            grants: Mutex::new(Vec::new()),
        }
    }

    fn actions(&self) -> Vec<ActionConfirmation> {
        self.actions.lock().unwrap().clone()
    }
}

impl ControlConfirmer for Shown {
    fn confirm_action(&self, request: &ActionConfirmation) -> bool {
        self.actions.lock().unwrap().push(request.clone());
        self.answer
    }
    fn confirm_grant(&self, request: &GrantConfirmation) -> bool {
        self.grants.lock().unwrap().push(request.clone());
        self.answer
    }
    fn confirm_resume(&self, _: &ResumeConfirmation) -> bool {
        self.answer
    }
    fn confirm_acknowledgement(
        &self,
        _: &nexus_governed_control::authority::approval::AcknowledgementConfirmation,
    ) -> bool {
        false
    }
}

/// The owner's one test command, as the launcher sends it.
fn synthetic_command() -> CommandEnvelope {
    CommandEnvelope {
        text: Some("synthetic-receipt".into()),
        ..Default::default()
    }
}

/// A synthetic grant the owner gave natively, and one probe submitted: the
/// commitment as the interface lists it.
fn granted_probe(world: &RealWorld) -> Value {
    world
        .request_grant(
            &GrantRequest::Synthetic {},
            3600,
            &Owner(true, AtomicU32::new(0)),
        )
        .unwrap();
    let submitted = world.submit(synthetic_command()).unwrap();
    assert_eq!(submitted["understood"], true, "{submitted}");
    submitted["commitment"].clone()
}

/// The audit trail's evidence entries of one commitment, in order.
fn audited(audit: &Arc<Mutex<AuditTrail>>, commitment: &str) -> Vec<Value> {
    audit
        .lock()
        .unwrap()
        .events()
        .iter()
        .filter(|e| {
            e.payload["event_kind"] == "p3.action.evidence" && e.payload["commitment"] == commitment
        })
        .map(|e| e.payload.clone())
        .collect()
}

fn phases(rows: &[Value]) -> Vec<String> {
    rows.iter()
        .map(|row| row["phase"].as_str().unwrap().to_string())
        .collect()
}

/// The receipt a listed commitment's summary shows.
fn receipt_of(commitment: &Value) -> String {
    let line = commitment["summary"][3].as_str().unwrap();
    let receipt = line.strip_prefix("Receipt: ").expect("the receipt line");
    let hex = receipt.strip_prefix("rcpt-").expect("rcpt-");
    assert!(
        hex.len() == 32
            && hex
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()),
        "{line}"
    );
    receipt.to_string()
}

const GRANT_LINES: [&str; 4] = [
    "Synthetic test receipts only: Nexus performs no external effect",
    "Usable only by your own commands, never by an agent",
    "Each receipt also needs your approval in a separate Nexus approval window",
    "A receipt is never evidence that a real-world action happened",
];

/// T-D1 and N-6: `p3_submit` commits an authenticated, prepared synthetic R2
/// request of an owner-session command; `p3_status` lists it as it is; the
/// native windows show its exact fields and wording; the native Allow
/// issues exactly one receipt, the one shown; nothing decides it twice;
/// every audit entry and evidence row of it says `synthetic`, and the
/// finished one says no external effect was performed.
#[test]
fn p4_uat05_t_d1_n6_the_probe_is_approved_natively_once_and_audited_as_synthetic() {
    let t = isolated();
    let granting = Shown::new(true);
    let grant = t
        .world
        .request_grant(&GrantRequest::Synthetic {}, 3600, &granting)
        .unwrap();
    // The native grant window.
    let window = granting.grants.lock().unwrap()[0].clone();
    assert_eq!(window.title(), "Grant this capability?");
    assert_eq!(
        window.text().header,
        vec![
            ("Capability", "synthetic".to_string()),
            ("Who may use it", OWNER_ONLY_USE.to_string()),
            ("Expires in", "900 s".to_string()),
        ]
    );
    assert_eq!(window.text().details, GRANT_LINES);
    assert_eq!(
        OWNER_ONLY_USE,
        "Owner-session commands only; agent runs are refused; every use requires separate native approval."
    );
    let status = t.world.status();
    let listed = status["grants"]
        .as_array()
        .unwrap()
        .iter()
        .find(|g| g["id"] == grant.as_str())
        .unwrap()
        .clone();
    assert_eq!(listed["kind"], "synthetic");
    assert_eq!(listed["live"], true);
    assert_eq!(listed["lines"], json!(GRANT_LINES));

    // p3_submit.
    let submitted = t.world.submit(synthetic_command()).unwrap();
    assert_eq!(submitted["understood"], true);
    let c = submitted["commitment"].clone();
    let id = c["id"].as_str().unwrap().to_string();
    assert_eq!(c["agent"], "owner-session");
    assert_eq!(c["kind"], "synthetic");
    assert_eq!(c["class"], "R2");
    assert_eq!(c["operation"], "synthetic.receipt");
    assert_eq!(c["target"], TARGET_TEXT);
    assert_eq!(c["state"], "prepared");
    assert_eq!(c["requires_approval"], true);
    let receipt = receipt_of(&c);
    assert_eq!(
        c["summary"],
        json!([
            "SYNTHETIC TEST: Nexus performs no external effect",
            "Allow issues one test receipt; nothing is written outside Nexus's own records",
            "No network, process, browser, connector, display or input action",
            format!("Receipt: {receipt}"),
            "A receipt is never evidence that a real-world action happened",
        ])
    );
    // p3_status lists exactly that.
    let listed = t.world.status()["commitments"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["id"] == id.as_str())
        .unwrap()
        .clone();
    assert_eq!(listed, c);

    // The native approval window, and the native Allow.
    let owner = Shown::new(true);
    let output = t.world.approve(&id, &owner).unwrap();
    let windows = owner.actions();
    assert_eq!(windows.len(), 1);
    let window = &windows[0];
    assert_eq!(window.title(), "Allow this action?");
    assert!(window.expires_in_secs <= 180);
    let text = window.text();
    assert_eq!(
        text.header,
        vec![
            ("Effect", SYNTHETIC_EFFECT.to_string()),
            ("Operation", "synthetic.receipt".to_string()),
            ("Target", TARGET_TEXT.to_string()),
            ("Acting for", "owner-session".to_string()),
            ("Run", c["run"].as_str().unwrap().to_string()),
            ("Commitment", id.clone()),
            ("Binding", c["binding"].as_str().unwrap().to_string()),
            ("Expires in", format!("{} s", window.expires_in_secs)),
        ]
    );
    assert_eq!(json!(text.details), c["summary"]);
    assert_eq!(
        output["text"],
        format!("SYNTHETIC TEST RECEIPT {receipt} for {id}: no external effect was performed.")
    );
    assert_eq!(output["bytes"], Value::Null);
    assert_eq!(
        output["meta"],
        json!([
            ["synthetic", "true"],
            ["external_effect", "none"],
            ["receipt", receipt]
        ])
    );
    assert_eq!(output["evidence"]["start"]["status"], "recorded");
    assert_eq!(output["evidence"]["end"]["status"], "recorded");
    assert_eq!(commitment_state(&t.world, &id), "succeeded");
    // No second decision, and no second window.
    let again = t.world.approve(&id, &owner).unwrap_err();
    assert!(again.contains("not_pending"), "{again}");
    assert_eq!(owner.actions().len(), 1);

    // N-6: the audit trail.
    let rows = audited(&t.audit, &id);
    assert_eq!(
        phases(&rows),
        ["prepared", "approved", "authorized", "started", "finished"]
    );
    for row in &rows {
        assert_eq!(row["kind"], "synthetic", "{row}");
        assert_eq!(row["operation"], "synthetic.receipt", "{row}");
        assert_eq!(row["class"], "R2", "{row}");
        assert_eq!(row["target"], TARGET_TEXT, "{row}");
        assert_eq!(row["agent"], "owner-session", "{row}");
    }
    let finished = rows.last().unwrap();
    assert_eq!(finished["outcome"], "succeeded");
    assert_eq!(
        finished["detail"],
        json!({ "synthetic": "true", "external_effect": "none", "receipt": receipt })
    );
    // N-6: the interface's evidence list (p3_evidence).
    let evidence: Vec<Value> = t
        .world
        .evidence()
        .into_iter()
        .filter(|row| row["commitment"] == id.as_str())
        .collect();
    assert_eq!(evidence.len(), 5);
    for row in &evidence {
        assert_eq!(row["kind"], "synthetic", "{row}");
        assert_eq!(row["operation"], "synthetic.receipt", "{row}");
    }
    assert!(t
        .audit
        .lock()
        .unwrap()
        .events()
        .iter()
        .any(|e| { e.payload["phase"] == "grant_issued" && e.payload["kind"] == "synthetic" }));
    // The existing kinds' R2 windows keep their wording.
    use nexus_governed_control::authority::effect::{CapabilityKind, EffectClass};
    for kind in [
        CapabilityKind::Tool,
        CapabilityKind::Egress,
        CapabilityKind::Browser,
        CapabilityKind::Perception,
        CapabilityKind::Input,
        CapabilityKind::Connector,
    ] {
        let real = ActionConfirmation {
            kind,
            class: EffectClass::R2,
            ..window.clone()
        };
        assert_eq!(
            real.text().header[0],
            (
                "Effect",
                "R2 (a sensitive or irreversible effect)".to_string()
            )
        );
    }
}

/// T-D2: issuing the receipt creates, changes or removes nothing under the
/// runtime root but the evidence store's own records (which do change).
#[test]
fn p4_uat05_t_d2_a_receipt_writes_nothing_outside_nexus_records() {
    fn listing(root: &std::path::Path) -> Vec<(String, bool, u64)> {
        let mut out = Vec::new();
        let mut stack = vec![root.to_path_buf()];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).unwrap() {
                let path = entry.unwrap().path();
                let relative = path
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned();
                if relative == "p3-evidence" {
                    continue;
                }
                let meta = std::fs::symlink_metadata(&path).unwrap();
                if meta.is_dir() {
                    stack.push(path);
                }
                out.push((
                    relative,
                    meta.is_dir(),
                    if meta.is_dir() { 0 } else { meta.len() },
                ));
            }
        }
        out.sort();
        out
    }
    let t = isolated();
    let c = granted_probe(&t.world);
    let before = listing(&t.root);
    let output = t
        .world
        .approve(c["id"].as_str().unwrap(), &Owner(true, AtomicU32::new(0)))
        .unwrap();
    assert!(output["text"]
        .as_str()
        .unwrap()
        .starts_with("SYNTHETIC TEST RECEIPT rcpt-"));
    assert_eq!(listing(&t.root), before);
    assert!(t
        .root
        .join("p3-evidence")
        .join("evidence.sqlite3")
        .is_file());
}

/// T-D3: Cancel (or any answer but Allow) in the native window: declined,
/// truthfully refused, no receipt.
#[test]
fn p4_uat05_t_d3_cancel_in_the_window_issues_no_receipt() {
    let t = isolated();
    let c = granted_probe(&t.world);
    let id = c["id"].as_str().unwrap();
    let owner = Owner(false, AtomicU32::new(0));
    let refused = t.world.approve(id, &owner).unwrap_err();
    assert!(refused.contains("declined"), "{refused}");
    assert_eq!(owner.1.load(Ordering::SeqCst), 1);
    assert_eq!(commitment_state(&t.world, id), "denied");
    assert_eq!(
        phases(&audited(&t.audit, id)),
        ["prepared", "approval_declined"]
    );
}

/// T-D4: Deny: refused before any window, no receipt.
#[test]
fn p4_uat05_t_d4_deny_issues_no_receipt() {
    let t = isolated();
    let c = granted_probe(&t.world);
    let id = c["id"].as_str().unwrap();
    t.world.deny(id).unwrap();
    assert_eq!(commitment_state(&t.world, id), "denied");
    let owner = Owner(true, AtomicU32::new(0));
    let refused = t.world.approve(id, &owner).unwrap_err();
    assert!(refused.contains("not_pending"), "{refused}");
    assert_eq!(owner.1.load(Ordering::SeqCst), 0);
    assert_eq!(phases(&audited(&t.audit, id)), ["prepared", "denied"]);
}

/// T-D5: Stop this run: revoked, no receipt.
#[test]
fn p4_uat05_t_d5_stopping_the_run_issues_no_receipt() {
    let t = isolated();
    let c = granted_probe(&t.world);
    let id = c["id"].as_str().unwrap();
    t.world.cancel_run(c["run"].as_str().unwrap()).unwrap();
    assert_eq!(commitment_state(&t.world, id), "revoked");
    let owner = Owner(true, AtomicU32::new(0));
    assert!(t.world.approve(id, &owner).is_err());
    assert_eq!(owner.1.load(Ordering::SeqCst), 0);
    assert!(!phases(&audited(&t.audit, id)).contains(&"started".to_string()));
}

/// T-D6: the emergency stop revokes it and refuses new commands until the
/// owner's native Resume.
#[test]
fn p4_uat05_t_d6_the_emergency_stop_refuses_until_a_native_resume() {
    let t = isolated();
    let c = granted_probe(&t.world);
    let id = c["id"].as_str().unwrap();
    t.world.emergency_stop();
    assert_eq!(commitment_state(&t.world, id), "revoked");
    let refused = t.world.submit(synthetic_command()).unwrap_err();
    assert!(refused.contains("emergency_stopped"), "{refused}");
    assert!(t.world.resume(&Owner(false, AtomicU32::new(0))).is_err());
    assert!(t.world.submit(synthetic_command()).is_err());
    t.world.resume(&Owner(true, AtomicU32::new(0))).unwrap();
    let submitted = t.world.submit(synthetic_command()).unwrap();
    assert_eq!(submitted["commitment"]["state"], "prepared");
    assert!(!phases(&audited(&t.audit, id)).contains(&"started".to_string()));
}

/// T-D7: an agent's path never reaches the probe, whatever the agent is
/// called (`owner-session` included): nothing is committed for it.
#[test]
fn p4_uat05_t_d7_an_agent_cannot_reach_the_probe() {
    let t = isolated();
    t.world
        .request_grant(
            &GrantRequest::Synthetic {},
            3600,
            &Owner(true, AtomicU32::new(0)),
        )
        .unwrap();
    for agent in [
        uuid::Uuid::new_v4().to_string(),
        "owner-session".to_string(),
    ] {
        let bridge = AgentBridge::with_warden(t.world.clone(), || false);
        let refused = bridge
            .act(&agent, &Intent::Synthetic(SyntheticIntent {}), false)
            .unwrap_err();
        assert!(
            refused.contains("a synthetic approval test is an owner-session command only"),
            "{refused}"
        );
    }
    let status = t.world.status();
    assert!(status["commitments"]
        .as_array()
        .unwrap()
        .iter()
        .all(|c| c["kind"] != "synthetic"));
    assert!(!t
        .audit
        .lock()
        .unwrap()
        .events()
        .iter()
        .any(|e| { e.payload["phase"] == "prepared" && e.payload["kind"] == "synthetic" }));
}

/// T-D8: through the owner's one front door, the structured form of the
/// command is the same owner-session command; nothing the caller adds to
/// it is accepted; the IPC surface is unchanged (`p3_g6_06`).
#[test]
fn p4_uat05_t_d8_the_front_door_takes_the_command_as_data_only() {
    let t = isolated();
    t.world
        .request_grant(
            &GrantRequest::Synthetic {},
            3600,
            &Owner(true, AtomicU32::new(0)),
        )
        .unwrap();
    let submitted = t
        .world
        .submit(CommandEnvelope {
            intent: Some(Intent::Synthetic(SyntheticIntent {})),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(submitted["commitment"]["agent"], "owner-session");
    assert_eq!(submitted["commitment"]["kind"], "synthetic");
    assert_eq!(submitted["commitment"]["class"], "R2");
    for forged in [
        json!({ "intent": { "domain": "synthetic", "intent": { "x": 1 } } }),
        json!({ "intent": { "domain": "synthetic", "intent": { "class": "R1" } } }),
        json!({ "text": "synthetic-receipt", "class": "R1" }),
    ] {
        assert!(
            serde_json::from_value::<CommandEnvelope>(forged.clone()).is_err(),
            "{forged}"
        );
    }
    let before = evidence_events(&t.audit);
    let answer = t
        .world
        .submit(CommandEnvelope {
            text: Some("synthetic-receipt R1".into()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(answer["understood"], false);
    assert_eq!(answer["reason"], "synthetic-receipt takes nothing");
    assert!(t
        .world
        .submit(CommandEnvelope {
            text: Some("synthetic-receipt".into()),
            intent: Some(Intent::Synthetic(SyntheticIntent {})),
            ..Default::default()
        })
        .is_err());
    assert_eq!(evidence_events(&t.audit), before);
}

const SYNTHETIC_RESTART_CHILD: &str = "NEXUS_P4_UAT05_RESTART_CHILD";

/// The child of `p4_uat05_t_d9_a_restart_resurrects_nothing`: over the
/// root it is given, it grants, submits one probe, reports its commitment
/// and exits without approving it or closing its session. Without its task
/// it does nothing.
#[test]
#[ignore = "run by p4_uat05_t_d9_a_restart_resurrects_nothing"]
fn p4_uat05_t_d9_restart_child() {
    let Ok(root) = std::env::var(SYNTHETIC_RESTART_CHILD) else {
        return;
    };
    let audit = Arc::new(Mutex::new(AuditTrail::new()));
    let db = Arc::new(NexusDatabase::in_memory().unwrap());
    let world = RealWorld::for_tests(std::path::Path::new(&root), audit, db);
    let c = granted_probe(&world);
    println!("SUBMITTED {}", c["id"].as_str().unwrap());
}

/// T-D9: after a restart, nothing of the probe survives: the commitment is
/// unknown (no window, no receipt) and no synthetic grant is live.
#[test]
fn p4_uat05_t_d9_a_restart_resurrects_nothing() {
    let root = private_home();
    let out = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "governed_real_world::tests::p4_uat05_t_d9_restart_child",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(SYNTHETIC_RESTART_CHILD, &root)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(out.status.success(), "{text}");
    let id = text
        .lines()
        .find_map(|l| {
            l.find("SUBMITTED ")
                .map(|at| l[at + "SUBMITTED ".len()..].trim().to_string())
        })
        .unwrap_or_else(|| panic!("the child reported nothing: {text}"));
    let audit = Arc::new(Mutex::new(AuditTrail::new()));
    let db = Arc::new(NexusDatabase::in_memory().unwrap());
    let world = RealWorld::for_tests(&root, audit, db);
    let owner = Owner(true, AtomicU32::new(0));
    let refused = world.approve(&id, &owner).unwrap_err();
    assert!(refused.contains("unknown commitment"), "{refused}");
    assert_eq!(owner.1.load(Ordering::SeqCst), 0);
    let status = world.status();
    assert!(status["grants"]
        .as_array()
        .unwrap()
        .iter()
        .all(|g| !(g["kind"] == "synthetic" && g["live"] == true)));
    assert!(status["commitments"]
        .as_array()
        .unwrap()
        .iter()
        .all(|c| c["id"] != id.as_str()));
    drop(world);
    std::fs::remove_dir_all(&root).unwrap();
}

/// T-D11: two approvals of one request at once (direct calls; the card
/// sends one review at a time): never two receipts. The desktop ends an
/// owner command's run when any approval of it returns, so the one that
/// is refused first may end the other's run too (an existing behaviour of
/// `RealWorld::approve`, outside this amendment): then neither issues a
/// receipt. Whatever the interleaving, every refusal is the backend's,
/// at most one effect starts, and every start is finished and recorded.
#[test]
fn p4_uat05_t_d11_concurrent_approvals_never_issue_two_receipts() {
    for _ in 0..8 {
        let t = isolated();
        let c = granted_probe(&t.world);
        let id = c["id"].as_str().unwrap().to_string();
        let owner = Arc::new(Owner(true, AtomicU32::new(0)));
        let handles: Vec<_> = (0..2)
            .map(|_| {
                let (world, owner, id) = (t.world.clone(), owner.clone(), id.clone());
                std::thread::spawn(move || world.approve(&id, owner.as_ref()))
            })
            .collect();
        let results: Vec<Result<Value, String>> =
            handles.into_iter().map(|h| h.join().unwrap()).collect();
        let receipts: Vec<&Value> = results.iter().filter_map(|r| r.as_ref().ok()).collect();
        assert!(receipts.len() <= 1, "{results:?}");
        for refused in results.iter().filter_map(|r| r.as_ref().err()) {
            assert!(
                ["not_pending", "run_not_active", "run_cancelled"]
                    .iter()
                    .any(|class| refused.contains(class)),
                "{refused}"
            );
        }
        let rows = audited(&t.audit, &id);
        let started = phases(&rows).iter().filter(|p| *p == "started").count();
        let finished: Vec<&Value> = rows.iter().filter(|r| r["phase"] == "finished").collect();
        assert!(started <= 1, "{rows:?}");
        assert_eq!(finished.len(), started, "{rows:?}");
        let succeeded = finished
            .iter()
            .filter(|r| r["outcome"] == "succeeded")
            .count();
        assert_eq!(succeeded, receipts.len(), "{rows:?}");
        let state = commitment_state(&t.world, &id);
        if let Some(receipt) = receipts.first() {
            assert_eq!(state, "succeeded");
            assert!(receipt["text"]
                .as_str()
                .unwrap()
                .starts_with("SYNTHETIC TEST RECEIPT rcpt-"));
        } else {
            assert!(
                ["revoked", "cancelled", "failed"].contains(&state.as_str()),
                "{state}"
            );
        }
    }
}
