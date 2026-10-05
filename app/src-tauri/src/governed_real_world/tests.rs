//! Phase Three in the desktop, isolated: a temporary runtime root, an
//! in-memory audit trail and database, scripted owner answers. Nothing here
//! opens the owner's state directory or database.

use super::{AgentBridge, RealWorld};
use nexus_governed_control::authority::approval::{
    ActionConfirmation, ControlConfirmer, GrantConfirmation, ResumeConfirmation,
};
use nexus_governed_control::governed::{GrantRequest, Intent};
use nexus_governed_control::ingress::CommandEnvelope;
use nexus_governed_control::tool::ToolIntent;
use nexus_kernel::audit::AuditTrail;
use nexus_persistence::NexusDatabase;
use serde_json::json;
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
    let bridge = AgentBridge::new(t.world.clone());
    let agent = uuid::Uuid::new_v4().to_string();
    let intent = Intent::Tool(ToolIntent {
        tool: "text.sha256".into(),
        input: json!({ "text": "abc" }),
    });
    let out = bridge.act(&agent, &intent).unwrap();
    assert!(out.contains("ba7816bf"), "{out}");
    assert_eq!(owner.1.load(Ordering::SeqCst), asked);
    // Another agent cannot use this loop's run.
    assert!(bridge
        .act(&uuid::Uuid::new_v4().to_string(), &intent)
        .is_err());
    // Emergency stop: the agent's next action is refused.
    t.world.emergency_stop();
    assert!(bridge.act(&agent, &intent).is_err());
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
