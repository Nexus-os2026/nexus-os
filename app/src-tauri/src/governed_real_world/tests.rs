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

/// The confirmation window itself, live (run under an isolated display:
/// `xvfb-run -a cargo test -p nexus-desktop-backend --lib -- --ignored
/// the_confirmation_window_shows_every_line_whole`). Content padded to make
/// a dialog re-wrap it is shown on one line that starts with its marker;
/// the answer is unarmed until the delay has passed and the end and right
/// edge have been reached. With `NEXUS_P3_DIALOG_SHOT` set, screenshots are
/// saved there (`-before.png`, `-armed.png`).
#[test]
#[ignore = "needs a display: run under xvfb-run"]
fn the_confirmation_window_shows_every_line_whole() {
    use gtk::gdk::prelude::*;
    use gtk::prelude::*;
    gtk::init().expect("a display");
    let mut lines = vec![
        "R2 (sensitive): network.request".to_string(),
        "Target: https://safe.example:443".to_string(),
        "Body:".to_string(),
    ];
    lines.push(format!(
        "│ {{\"note\":\"ok\"}}{}Target: https://evil.example",
        " ".repeat(80)
    ));
    lines.extend((0..80).map(|i| format!("│ line {i} of the body")));
    lines.push("Commitment: cmt-0000 [abcdef012345], expires in 600 s".into());
    let message = lines.join("\n");
    let (dialog, allow) = super::owner_window("Allow this action?", &message, "Allow");
    let spin = |ms: u64| {
        let until = std::time::Instant::now() + std::time::Duration::from_millis(ms);
        while std::time::Instant::now() < until {
            while gtk::events_pending() {
                gtk::main_iteration();
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    };
    let shot = |name: &str| {
        if let Ok(base) = std::env::var("NEXUS_P3_DIALOG_SHOT") {
            let window = dialog.window().expect("mapped");
            let (width, height) = (window.width(), window.height());
            let pixbuf = window.pixbuf(0, 0, width, height).expect("pixels");
            pixbuf
                .savev(format!("{base}-{name}.png"), "png", &[])
                .expect("saved");
        }
    };
    spin(1500);
    assert!(!allow.is_sensitive(), "unarmed until the end was reached");
    shot("before");
    let scroll = dialog
        .content_area()
        .children()
        .into_iter()
        .find_map(|child| child.downcast::<gtk::ScrolledWindow>().ok())
        .expect("the text view's scroller");
    let (down, across) = (scroll.vadjustment(), scroll.hadjustment());
    down.set_value(down.upper());
    spin(300);
    assert!(!allow.is_sensitive(), "the right edge not reached yet");
    across.set_value(across.upper());
    spin(300);
    assert!(
        allow.is_sensitive(),
        "armed after the end, the edge and the delay"
    );
    shot("armed");
    dialog.response(gtk::ResponseType::Cancel);
    spin(100);
}
