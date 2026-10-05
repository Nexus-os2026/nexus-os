//! The front door and the final PlannedAction classification.

use super::{AgentOutcome, GovernedControl, GrantRequest, Intent};
use crate::authority::clock::SystemClock;
use crate::authority::commitment::CommitmentState;
use crate::authority::evidence::MemoryEvidence;
use crate::authority::ids::AgentId;
use crate::authority::run::RunOrigin;
use crate::authority::AuthorityError;
use crate::broker::NoVault;
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
        Arc::new(NoVault),
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
        control.agent_action(&agent, run, &read, &never).unwrap(),
        AgentOutcome::Done(_)
    ));
    assert_eq!(never.asked(), 0, "R1 under a grant asks nothing");
    let write = Intent::Request(EgressIntent {
        method: s("POST"),
        url: format!("{}/w", server.origin()),
        headers: vec![],
        body: Some(s("x")),
    });
    let AgentOutcome::AwaitingApproval(view) =
        control.agent_action(&agent, run, &write, &never).unwrap()
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
