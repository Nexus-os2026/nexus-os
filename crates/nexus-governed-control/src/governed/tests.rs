//! The front door and the final PlannedAction classification.

use super::{AgentOutcome, GovernedControl, GrantRequest, Intent};
use crate::authority::clock::SystemClock;
use crate::authority::commitment::CommitmentState;
use crate::authority::evidence::MemoryEvidence;
use crate::authority::ids::AgentId;
use crate::authority::run::RunOrigin;
use crate::authority::AuthorityError;
use crate::broker::Vault;
use crate::egress::EgressIntent;
use crate::harness_tests::{temp_root, Reply, TempRoot, TestServer, Yes};
use crate::planned::{classify, Disposition};
use crate::tool::ToolIntent;
use nexus_kernel::cognitive::types::BrowserAction;
use nexus_kernel::cognitive::PlannedAction;
use nexus_kernel::computer_control::ScreenRegion;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;

fn control() -> (GovernedControl, Arc<MemoryEvidence>, TempRoot) {
    let root = temp_root("front-door");
    let evidence = Arc::new(MemoryEvidence::new(10_000));
    let control = GovernedControl::new(
        root.0.path(),
        Vault::Disabled,
        evidence.clone(),
        Arc::new(SystemClock::default()),
    )
    .unwrap();
    (control, evidence, root)
}

fn s(text: &str) -> String {
    text.to_string()
}

/// One of every PlannedAction variant, with its expected disposition kind.
fn every_action() -> Vec<(PlannedAction, &'static str)> {
    use PlannedAction as P;
    vec![
        (
            P::LlmQuery {
                prompt: s("p"),
                context: vec![],
            },
            "inert",
        ),
        (
            P::FileRead {
                path: s("/etc/passwd"),
            },
            "closed",
        ),
        (
            P::FileWrite {
                path: s("/tmp/x"),
                content: s("x"),
            },
            "closed",
        ),
        (
            P::ShellCommand {
                command: s("sh"),
                args: vec![],
            },
            "closed",
        ),
        (
            P::DockerCommand {
                subcommand: s("run"),
                args: vec![],
            },
            "closed",
        ),
        (P::WebSearch { query: s("q") }, "inert"),
        (
            P::WebFetch {
                url: s("https://example.com/"),
            },
            "governed",
        ),
        (
            P::ApiCall {
                method: s("post"),
                url: s("https://example.com/"),
                body: None,
                headers: None,
            },
            "governed",
        ),
        (
            P::ImageGenerate {
                prompt: s("p"),
                output_path: s("o"),
                provider: None,
                model: None,
                size: None,
            },
            "closed",
        ),
        (
            P::TextToSpeech {
                text: s("hi"),
                output_path: s(""),
                provider: None,
                voice: None,
                model: None,
            },
            "governed",
        ),
        (
            P::KnowledgeGraphUpdate {
                entities: vec![],
                relationships: vec![],
            },
            "inert",
        ),
        (P::KnowledgeGraphQuery { query: s("q") }, "inert"),
        (
            P::BrowserAutomate {
                start_url: s("https://example.com/"),
                actions: vec![BrowserAction::ExtractText { selector: s("h1") }],
                screenshot_dir: None,
            },
            "governed",
        ),
        (P::CaptureScreen { region: None }, "governed"),
        (
            P::CaptureWindow {
                window_title: s("w"),
            },
            "governed",
        ),
        (P::AnalyzeScreen { query: s("q") }, "governed"),
        (P::MouseMove { x: 1, y: 2 }, "governed"),
        (
            P::MouseClick {
                x: 1,
                y: 2,
                button: s("left"),
            },
            "governed",
        ),
        (P::MouseDoubleClick { x: 1, y: 2 }, "governed"),
        (
            P::MouseDrag {
                from_x: 1,
                from_y: 2,
                to_x: 3,
                to_y: 4,
            },
            "governed",
        ),
        (P::KeyboardType { text: s("t") }, "governed"),
        (P::KeyboardPress { key: s("Return") }, "governed"),
        (
            P::KeyboardShortcut {
                keys: vec![s("ctrl"), s("l")],
            },
            "governed",
        ),
        (
            P::ScrollWheel {
                direction: s("down"),
                amount: 3,
            },
            "governed",
        ),
        (
            P::ComputerAction {
                description: s("d"),
                max_steps: 5,
            },
            "orchestrated",
        ),
        (
            P::AgentMessage {
                target_agent: s("a"),
                message: s("m"),
            },
            "inert",
        ),
        (
            P::HitlRequest {
                question: s("q"),
                options: vec![],
            },
            "inert",
        ),
        (
            P::MemoryStore {
                key: s("k"),
                value: s("v"),
                memory_type: s("t"),
            },
            "inert",
        ),
        (
            P::MemoryRecall {
                query: s("q"),
                memory_type: None,
            },
            "inert",
        ),
        (
            P::SendNotification {
                title: s("t"),
                body: s("b"),
                level: s("info"),
            },
            "inert",
        ),
        (
            P::CodeExecute {
                language: s("py"),
                code: s("1"),
                timeout_secs: None,
            },
            "closed",
        ),
        (P::Noop, "inert"),
        (
            P::SelfModifyDescription {
                new_description: s("d"),
            },
            "closed",
        ),
        (
            P::SelfModifyStrategy {
                strategy_key: s("k"),
                new_strategy: s("s"),
            },
            "closed",
        ),
        (
            P::CreateSubAgent {
                manifest_json: s("{}"),
            },
            "closed",
        ),
        (P::DestroySubAgent { agent_id: s("a") }, "closed"),
        (
            P::RunEvolutionTournament {
                variants: vec![],
                task: s("t"),
                rounds: 1,
            },
            "closed",
        ),
        (
            P::ModifyGovernancePolicy {
                policy_key: s("k"),
                policy_value: s("v"),
            },
            "closed",
        ),
        (
            P::AllocateEcosystemFuel {
                agent_id: s("a"),
                amount: 1.0,
            },
            "closed",
        ),
        (
            P::ModifyCognitiveParams {
                param_key: s("k"),
                param_value: s("v"),
            },
            "closed",
        ),
        (
            P::SelectLlmProvider {
                phase: s("p"),
                provider: s("p"),
                model: s("m"),
            },
            "closed",
        ),
        (
            P::SelectAlgorithm {
                algorithm: s("a"),
                config_json: s("{}"),
            },
            "closed",
        ),
        (
            P::DesignAgentEcosystem {
                ecosystem_json: s("{}"),
            },
            "closed",
        ),
        (
            P::RunCounterfactual {
                decision_id: s("d"),
                alternatives: vec![],
            },
            "closed",
        ),
        (
            P::TemporalPlan {
                immediate: s("i"),
                short_term: s("s"),
                medium_term: s("m"),
                long_term: s("l"),
            },
            "closed",
        ),
        (
            P::A2aDelegation {
                agent_url: s("https://a.example/"),
                message: s("m"),
            },
            "closed",
        ),
    ]
}

fn kind(disposition: &Disposition) -> &'static str {
    match disposition {
        Disposition::Inert => "inert",
        Disposition::Governed(_) => "governed",
        Disposition::Orchestrated { .. } => "orchestrated",
        Disposition::Closed(_) => "closed",
    }
}

#[test]
fn every_planned_action_has_its_final_classification() {
    let actions = every_action();
    assert_eq!(actions.len(), 46);
    let mut counts = std::collections::BTreeMap::new();
    for (action, expected) in &actions {
        assert_eq!(
            kind(&classify(action)),
            *expected,
            "{}",
            action.action_type()
        );
        *counts.entry(*expected).or_insert(0) += 1;
    }
    assert_eq!(counts["inert"], 10);
    assert_eq!(counts["governed"] + counts["orchestrated"], 16);
    assert_eq!(counts["closed"], 20);
    // Every variant is covered once (by its action type).
    let mut types: Vec<String> = actions
        .iter()
        .map(|(a, _)| a.action_type().to_string())
        .collect();
    types.sort();
    types.dedup();
    assert_eq!(types.len(), 46);
}

#[test]
fn governed_mappings_carry_data_and_refuse_side_doors() {
    assert_eq!(
        classify(&PlannedAction::ApiCall {
            method: s(" post "),
            url: s("https://example.com/x"),
            body: Some(s("{}")),
            headers: Some([(s("Accept"), s("application/json"))].into_iter().collect()),
        }),
        Disposition::Governed(Intent::Request(EgressIntent {
            method: s("POST"),
            url: s("https://example.com/x"),
            headers: vec![(s("Accept"), s("application/json"))],
            body: Some(s("{}")),
        }))
    );
    assert!(matches!(
        classify(&PlannedAction::TextToSpeech {
            text: s("hi"),
            output_path: s("/home/me/x.wav"),
            provider: None,
            voice: None,
            model: None
        }),
        Disposition::Closed(_)
    ));
    assert!(matches!(
        classify(&PlannedAction::TextToSpeech {
            text: s("hi"),
            output_path: s(""),
            provider: Some(s("cloud")),
            voice: None,
            model: None
        }),
        Disposition::Closed(_)
    ));
    assert!(matches!(
        classify(&PlannedAction::BrowserAutomate {
            start_url: s("https://example.com/"),
            actions: vec![],
            screenshot_dir: Some(s("/tmp/shots"))
        }),
        Disposition::Closed(_)
    ));
    assert!(matches!(
        classify(&PlannedAction::CaptureScreen {
            region: Some(ScreenRegion {
                x: 0,
                y: 0,
                width: 70_000,
                height: 10
            })
        }),
        Disposition::Closed(_)
    ));
    assert!(matches!(
        classify(&PlannedAction::MouseClick {
            x: 1,
            y: 1,
            button: s("fourth")
        }),
        Disposition::Closed(_)
    ));
    assert_eq!(
        classify(&PlannedAction::ComputerAction {
            description: s("d"),
            max_steps: 1_000_000
        }),
        Disposition::Orchestrated {
            max_steps: crate::display::MAX_INPUT_STEPS
        }
    );
}

#[test]
fn intents_are_exact_data() {
    let intent: Intent = serde_json::from_value(json!({
        "domain": "request",
        "intent": { "method": "GET", "url": "https://example.com/" }
    }))
    .unwrap();
    assert!(matches!(intent, Intent::Request(_)));
    for refused in [
        json!({ "domain": "shell", "intent": { "command": "sh" } }),
        json!({ "domain": "request", "intent": { "method": "GET", "url": "https://e.example/", "credential": "x" } }),
        json!({ "domain": "request", "intent": { "method": "GET", "url": "https://e.example/" }, "grant": "forged" }),
    ] {
        assert!(
            serde_json::from_value::<Intent>(refused.clone()).is_err(),
            "{refused}"
        );
    }
}

#[test]
fn the_front_door_grants_and_runs_a_tool() {
    let (control, evidence, _root) = control();
    let owner = Yes::new(true);
    control
        .request_grant(
            &GrantRequest::Tool {
                tool: s("text.sha256"),
            },
            Duration::from_secs(60),
            &owner,
        )
        .unwrap();
    let agent = AgentId::owner_session();
    let run = control
        .open_run(
            agent.clone(),
            RunOrigin::Command {
                modalities: vec![s("text")],
            },
        )
        .unwrap();
    let intent = Intent::Tool(ToolIntent {
        tool: s("text.sha256"),
        input: json!({ "text": "abc" }),
    });
    let view = control.propose(&agent, run, &intent).unwrap();
    control.authorize(view.id, &agent, run, &owner).unwrap();
    let out = control.execute(view.id, &agent, run).unwrap();
    assert!(out
        .text
        .unwrap()
        .starts_with("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"));
    assert!(evidence
        .records()
        .iter()
        .any(|r| r.outcome == Some("succeeded")));
    // A declined grant creates nothing.
    assert_eq!(
        control
            .request_grant(
                &GrantRequest::Perception,
                Duration::from_secs(60),
                &Yes::new(false)
            )
            .unwrap_err(),
        AuthorityError::Declined
    );
}

#[test]
fn an_agent_r2_action_waits_for_the_owner_and_r1_runs_under_the_grant() {
    let (control, _evidence, _root) = control();
    let server = TestServer::start(|_| Reply::ok("done"));
    let owner = Yes::new(true);
    control
        .request_grant(
            &GrantRequest::Egress {
                origin: server.origin(),
                methods: vec![s("GET"), s("POST")],
                allow_private: true,
            },
            Duration::from_secs(60),
            &owner,
        )
        .unwrap();
    let agent = AgentId::new("5c1d7a3e-0000-4000-8000-000000000001").unwrap();
    let run = control
        .open_run(agent.clone(), RunOrigin::AgentGoal)
        .unwrap();
    let never = Yes::new(true);
    let read = Intent::Request(EgressIntent {
        method: s("GET"),
        url: format!("{}/r", server.origin()),
        headers: vec![],
        body: None,
    });
    assert!(matches!(
        control.agent_action(&agent, run, &read).unwrap(),
        AgentOutcome::Done(_)
    ));
    assert_eq!(never.asked(), 0, "R1 under a grant asks nothing");
    let write = Intent::Request(EgressIntent {
        method: s("POST"),
        url: format!("{}/w", server.origin()),
        headers: vec![],
        body: Some(s("x")),
    });
    let AgentOutcome::AwaitingApproval(view) = control.agent_action(&agent, run, &write).unwrap()
    else {
        panic!("R2 waits")
    };
    assert_eq!(
        never.asked(),
        0,
        "the agent never triggers the owner's dialog itself"
    );
    assert!(server.received().iter().all(|r| r.path != "/w"));
    // The owner approves natively from the interface, then it runs.
    control.authorize(view.id, &agent, run, &owner).unwrap();
    control.execute(view.id, &agent, run).unwrap();
    assert!(server.received().iter().any(|r| r.path == "/w"));
}

#[test]
fn the_emergency_stop_ends_everything_until_the_owner_resumes() {
    let (control, _evidence, _root) = control();
    let owner = Yes::new(true);
    control
        .request_grant(
            &GrantRequest::Tool {
                tool: s("text.sha256"),
            },
            Duration::from_secs(60),
            &owner,
        )
        .unwrap();
    let agent = AgentId::owner_session();
    let run = control
        .open_run(agent.clone(), RunOrigin::AgentGoal)
        .unwrap();
    let intent = Intent::Tool(ToolIntent {
        tool: s("text.sha256"),
        input: json!({ "text": "x" }),
    });
    let view = control.propose(&agent, run, &intent).unwrap();
    assert_eq!(control.emergency_stop(), 1);
    assert_eq!(
        control
            .authority()
            .commitments()
            .view(view.id)
            .unwrap()
            .state,
        CommitmentState::Revoked
    );
    assert!(control.status().emergency_stopped);
    assert!(control
        .open_run(agent.clone(), RunOrigin::AgentGoal)
        .is_err());
    assert_eq!(
        control.resume(&Yes::new(false)).unwrap_err(),
        AuthorityError::Declined
    );
    control.resume(&owner).unwrap();
    assert!(control.open_run(agent, RunOrigin::AgentGoal).is_ok());
}

/// A run whose agent has ended finishes as soon as nothing of it waits: here
/// once the owner denies what waited. Nothing was sent.
#[test]
fn a_detached_run_finishes_when_nothing_of_it_waits() {
    let (control, _evidence, _root) = control();
    let server = TestServer::start(|_| Reply::ok("ok"));
    control
        .request_grant(
            &GrantRequest::Egress {
                origin: server.origin(),
                methods: vec!["POST".into()],
                allow_private: true,
            },
            Duration::from_secs(600),
            &Yes::new(true),
        )
        .unwrap();
    let agent = AgentId::new("agent-detached").unwrap();
    let run = control
        .open_run(agent.clone(), RunOrigin::AgentGoal)
        .unwrap();
    let post = Intent::Request(EgressIntent {
        method: "POST".into(),
        url: format!("{}/x", server.origin()),
        headers: vec![],
        body: Some("{}".into()),
    });
    let AgentOutcome::AwaitingApproval(view) = control.agent_action(&agent, run, &post).unwrap()
    else {
        panic!("an R2 request waits for the owner");
    };
    control.finish_when_settled(run);
    assert!(
        control.authority().runs().check(run, &agent).is_ok(),
        "the run stays while the owner can still approve"
    );
    control.deny(view.id, &agent, run).unwrap();
    assert_eq!(
        control.authority().runs().check(run, &agent).unwrap_err(),
        AuthorityError::RunNotActive
    );
    assert!(server.received().is_empty());
}

/// Quitting cancels every open run and refuses new ones from then on, so
/// nothing starts while the desktop waits for what still executes.
#[test]
fn quitting_cancels_every_run_and_opens_no_more() {
    let (control, _evidence, _root) = control();
    let agent = AgentId::new("agent-quit").unwrap();
    let run = control
        .open_run(agent.clone(), RunOrigin::AgentGoal)
        .unwrap();
    assert_eq!(control.shut_down(), 1);
    assert!(control
        .authority()
        .runs()
        .views()
        .iter()
        .all(|view| view.id != run || view.cancelled));
    assert_eq!(
        control.open_run(agent, RunOrigin::AgentGoal).unwrap_err(),
        AuthorityError::Closed("the desktop is quitting")
    );
    // Nor does the agent display start.
    control
        .request_grant(
            &GrantRequest::Perception,
            Duration::from_secs(600),
            &Yes::new(true),
        )
        .unwrap();
    assert_eq!(
        control.start_display().unwrap_err(),
        AuthorityError::Closed("the desktop is quitting")
    );
}

/// A display start that was recorded but did not complete is recorded as
/// ended (here the runtime root refuses the display's directory).
#[cfg(target_os = "linux")]
#[test]
fn a_failed_display_start_is_recorded_as_ended() {
    use crate::authority::evidence::EvidencePhase;
    use std::os::unix::fs::PermissionsExt;
    let (control, evidence, root) = control();
    control
        .request_grant(
            &GrantRequest::Perception,
            Duration::from_secs(600),
            &Yes::new(true),
        )
        .unwrap();
    let path = root.0.path().to_path_buf();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o500)).unwrap();
    let started = control.start_display();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
    assert!(started.is_err(), "{started:?}");
    assert!(control.status().display.is_none());
    let display: Vec<EvidencePhase> = evidence
        .records()
        .iter()
        .map(|r| r.phase)
        .filter(|p| {
            matches!(
                p,
                EvidencePhase::DisplayStarted | EvidencePhase::DisplayStopped
            )
        })
        .collect();
    assert_eq!(
        display,
        [EvidencePhase::DisplayStarted, EvidencePhase::DisplayStopped]
    );
}

/// A display start counts as under way (quitting waits for it) until its
/// end is recorded, here the end of a start that failed.
#[cfg(target_os = "linux")]
#[test]
fn a_display_start_counts_until_its_end_is_recorded() {
    use crate::authority::evidence::{
        EvidencePhase, EvidenceRecord, EvidenceSink, EvidenceUnavailable,
    };
    use std::os::unix::fs::PermissionsExt;
    use std::sync::{Mutex, OnceLock, Weak};
    /// Notes, as each display end is recorded, whether a start counts.
    struct Watch {
        control: OnceLock<Weak<GovernedControl>>,
        starting: Mutex<Vec<bool>>,
    }
    impl EvidenceSink for Watch {
        fn record(&self, record: &EvidenceRecord) -> Result<(), EvidenceUnavailable> {
            if record.phase == EvidencePhase::DisplayStopped {
                if let Some(control) = self.control.get().and_then(Weak::upgrade) {
                    let starting = control.is_starting_display();
                    self.starting.lock().unwrap().push(starting);
                }
            }
            Ok(())
        }
    }
    let root = temp_root("display-start-count");
    let watch = Arc::new(Watch {
        control: OnceLock::new(),
        starting: Mutex::new(Vec::new()),
    });
    let control = Arc::new(
        GovernedControl::new(
            root.0.path(),
            Vault::Disabled,
            watch.clone(),
            Arc::new(SystemClock::default()),
        )
        .unwrap(),
    );
    let _ = watch.control.set(Arc::downgrade(&control));
    control
        .request_grant(
            &GrantRequest::Perception,
            Duration::from_secs(600),
            &Yes::new(true),
        )
        .unwrap();
    assert!(!control.is_starting_display());
    // The runtime root refuses the display's directory: the start fails
    // after it was recorded.
    let path = root.0.path().to_path_buf();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o500)).unwrap();
    let started = control.start_display();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
    assert!(started.is_err(), "{started:?}");
    assert_eq!(*watch.starting.lock().unwrap(), [true]);
    assert!(!control.is_starting_display());
}

/// The agent display is a process Phase Three owns: starting it needs a
/// live perception or input grant and no emergency stop, and its start and
/// stop are recorded.
#[test]
fn starting_the_agent_display_needs_a_grant_and_is_recorded() {
    use crate::authority::evidence::EvidencePhase;
    let (control, evidence, _root) = control();
    assert_eq!(
        control.start_display().unwrap_err(),
        AuthorityError::NoCoveringGrant
    );
    control
        .request_grant(
            &GrantRequest::Perception,
            Duration::from_secs(600),
            &Yes::new(true),
        )
        .unwrap();
    control.emergency_stop();
    assert_eq!(
        control.start_display().unwrap_err(),
        AuthorityError::EmergencyStopped
    );
    control.resume(&Yes::new(true)).unwrap();
    if !std::path::Path::new("/usr/bin/Xvfb").exists() {
        // Linux CI installs Xvfb (ci.yml): there a missing one fails the
        // test. (The agent display is Linux-only.)
        assert!(
            !cfg!(target_os = "linux") || std::env::var_os("CI").is_none(),
            "Xvfb is missing: CI must start the agent display"
        );
        eprintln!("Xvfb is not installed: the display start is not exercised here");
        return;
    }
    control.start_display().unwrap();
    control.stop_display();
    let phases: Vec<EvidencePhase> = evidence.records().iter().map(|r| r.phase).collect();
    assert!(phases.contains(&EvidencePhase::DisplayStarted));
    assert!(phases.contains(&EvidencePhase::DisplayStopped));
    // The emergency stop records the display it stops, after the stop.
    control.start_display().unwrap();
    let before = evidence.records().len();
    control.emergency_stop();
    assert!(control.status().display.is_none());
    let after: Vec<EvidencePhase> = evidence.records()[before..]
        .iter()
        .map(|r| r.phase)
        .collect();
    assert_eq!(
        after
            .iter()
            .filter(|p| **p == EvidencePhase::DisplayStopped)
            .count(),
        1,
        "{after:?}"
    );
}

/// The start of the agent display is recorded with no lock held: while its
/// record is written, the display's status and a stop go on (neither waits
/// for the start), and the start, overtaken, ends without a display.
#[cfg(target_os = "linux")]
#[test]
fn a_display_start_is_recorded_with_no_lock_held_and_a_stop_overtakes_it() {
    use crate::authority::evidence::EvidencePhase;
    use crate::authority::scripted::{within, ScriptedSink};
    let root = temp_root("display-start-lock");
    let sink = ScriptedSink::new();
    let control = Arc::new(
        GovernedControl::new(
            root.0.path(),
            Vault::Disabled,
            sink.clone(),
            Arc::new(SystemClock::default()),
        )
        .unwrap(),
    );
    let weak = Arc::downgrade(&control);
    sink.probe_with(move || {
        if let Some(control) = weak.upgrade() {
            control.probe_locks();
        }
    });
    control
        .request_grant(
            &GrantRequest::Perception,
            Duration::from_secs(600),
            &Yes::new(true),
        )
        .unwrap();
    sink.hold_next(EvidencePhase::DisplayStarted);
    let starting = {
        let control = control.clone();
        std::thread::spawn(move || control.start_display())
    };
    sink.wait_held();
    assert!(control.is_starting_display());
    {
        let control = control.clone();
        within(move || {
            assert!(control.status().display.is_none());
            control.stop_display();
        });
    }
    sink.release();
    let started = starting.join().unwrap();
    assert_eq!(
        started.unwrap_err(),
        AuthorityError::Closed("the agent display was stopped while it started")
    );
    assert!(control.status().display.is_none());
    assert!(!control.is_starting_display());
    // Its recorded start is recorded as ended.
    let phases = sink.phases();
    assert_eq!(
        phases
            .iter()
            .filter(|p| matches!(
                p,
                EvidencePhase::DisplayStarted | EvidencePhase::DisplayStopped
            ))
            .collect::<Vec<_>>(),
        [
            &EvidencePhase::DisplayStarted,
            &EvidencePhase::DisplayStopped
        ]
    );
    assert!(sink.probes() >= phases.len());
    assert_eq!(sink.violations(), 0);
}
