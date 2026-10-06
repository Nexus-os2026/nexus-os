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
    let executor = crate::phase0_agent_executor(&state, memory).with_real_world(t.world.clone());
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
    assert!(shown.status.success(), "{out}\n{err}");
    // It ran, and ran this test only.
    assert!(out.contains(&format!("test {name} ... ok")), "{out}\n{err}");
    assert!(out.contains("test result: ok. 1 passed"), "{out}\n{err}");
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
}
