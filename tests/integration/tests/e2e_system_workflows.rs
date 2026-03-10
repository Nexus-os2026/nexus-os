use coder_agent::analyzer::{analyze, ProjectType};
use coder_agent::fix_loop::{fix_until_pass, FixResult};
use coder_agent::scanner::scan_project;
use coder_agent::test_runner::run_tests;
use coder_agent::writer::FileChange as CoderFileChange;
use nexus_kernel::audit::AuditTrail;
use nexus_kernel::autonomy::{AutonomyGuard, AutonomyLevel};
use nexus_kernel::consent::{
    ApprovalQueue, ConsentPolicyEngine, ConsentRuntime, GovernedOperation, HitlTier,
};
use screen_poster_agent::approval::{
    ApprovalDecision, ApprovalError, HumanApprovalGate, InMemoryApprovalChannel,
};
use screen_poster_agent::composer::ContentComposer;
use screen_poster_agent::navigator::SocialPlatform;
use self_improve_agent::learner::analyze_history;
use self_improve_agent::tracker::{OutcomeResult, PerformanceTracker, TaskMetrics, TaskType};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::fs;
use tempfile::tempdir;
use uuid::Uuid;
use web_builder_agent::codegen::{generate_website, FileChange as WebFileChange};
use web_builder_agent::interpreter::interpret;
use workflow_studio_agent::engine::{WorkflowContext, WorkflowEngine};
use workflow_studio_agent::nodes::{
    ActionNode, NodeErrorStrategy, NodeKind, NodePort, Workflow, WorkflowConnection, WorkflowNode,
};

#[test]
    #[ignore]
fn test_integration_coding_agent_end_to_end() {
    let project_dir = tempdir().expect("temp project directory should be created");
    write_file(
        project_dir.path().join("Cargo.toml"),
        r#"[package]
name = "sample_project"
version = "0.1.0"
edition = "2021"
"#,
    );
    write_file(
        project_dir.path().join("src/lib.rs"),
        r#"pub fn add(a: i32, b: i32) -> i32 {
    a + b
}
"#,
    );
    write_file(
        project_dir.path().join("tests/basic.rs"),
        r#"use sample_project::{add, double};

#[test]
fn test_add() {
    assert_eq!(add(2, 3), 5);
}

#[test]
fn test_double() {
    assert_eq!(double(4), 8);
}
"#,
    );

    let project_map = scan_project(project_dir.path()).expect("project scan should succeed");
    let architecture = analyze(&project_map).expect("project analysis should succeed");
    assert_eq!(architecture.project_type, ProjectType::RustCrate);

    let failing = run_tests(project_dir.path()).expect("initial test run should complete");
    assert!(
        failing.failed > 0 || !failing.errors.is_empty(),
        "expected initial tests to fail before adding missing function"
    );

    let old_lib = fs::read_to_string(project_dir.path().join("src/lib.rs"))
        .expect("source file should be readable");
    let new_lib = format!(
        "{old_lib}\n\
         pub fn double(value: i32) -> i32 {{\n\
             value * 2\n\
         }}\n"
    );
    let changes = vec![CoderFileChange::Modify(
        "src/lib.rs".to_string(),
        old_lib,
        new_lib,
    )];

    let result = fix_until_pass(project_dir.path(), changes, 3).expect("fix loop should run");
    match result {
        FixResult::Success { last_result, .. } => {
            assert_eq!(last_result.failed, 0);
            assert!(last_result.errors.is_empty());
        }
        other => panic!("expected successful fix result, got {other:?}"),
    }
}

#[test]
fn test_integration_screen_poster_draft_flow() {
    let mut composer = ContentComposer::new();
    let draft = composer
        .compose("NexusOS coding updates", SocialPlatform::X, "professional")
        .expect("draft should be composed");

    let mut gate = HumanApprovalGate::new(InMemoryApprovalChannel::default());
    let ticket = gate
        .present_draft(draft.clone())
        .expect("draft should be sent for approval");

    let pending = gate.approved_draft(ticket);
    assert!(matches!(pending, Err(ApprovalError::Pending)));

    gate.decide(ticket, ApprovalDecision::Approve)
        .expect("approval should succeed");
    let approved = gate
        .approved_draft(ticket)
        .expect("approved draft should be available");
    assert_eq!(approved.draft.text, draft.text);

    let steps = gate
        .audit_events()
        .iter()
        .filter_map(|event| event.payload.get("step").and_then(Value::as_str))
        .collect::<Vec<_>>();
    assert!(steps.contains(&"present_for_approval"));
    assert!(steps.contains(&"approval_decision"));
}

#[test]
fn test_integration_website_builder_generation() {
    let spec = interpret("Build a landing page for a startup with hero, features, and contact")
        .expect("description should be interpreted");
    assert!(!spec.pages.is_empty());

    let files = generate_website(&spec).expect("website code should generate");
    let index_html = created_file_content(&files, "index.html").expect("index.html should exist");
    assert!(index_html.contains("<!doctype html>"));
    assert!(index_html.contains("<div id=\"root\"></div>"));

    let app_tsx = created_file_content(&files, "src/App.tsx").expect("App.tsx should exist");
    assert!(app_tsx.contains("export default function App(): JSX.Element"));

    for (path, content) in created_files(&files) {
        if path.ends_with(".ts") || path.ends_with(".tsx") {
            assert!(
                !content.contains(": any") && !content.contains("<any>"),
                "expected strongly typed output in {path}"
            );
        }
    }
}

#[test]
fn test_integration_workflow_execution() {
    let workflow = Workflow {
        id: "wf-e2e".to_string(),
        name: "E2E Workflow".to_string(),
        description: "A -> B -> C".to_string(),
        nodes: vec![workflow_node("A"), workflow_node("B"), workflow_node("C")],
        connections: vec![
            workflow_edge("A", "result", "B", "input"),
            workflow_edge("B", "result", "C", "input"),
        ],
    };

    let mut context = WorkflowContext {
        capabilities: ["workflow.execute".to_string()]
            .into_iter()
            .collect::<HashSet<_>>(),
        fuel_remaining: 200,
        agent_id: Uuid::new_v4(),
        autonomy_guard: AutonomyGuard::new(AutonomyLevel::L1),
        audit_trail: AuditTrail::new(),
        consent_runtime: {
            let mut policy = ConsentPolicyEngine::default();
            policy.set_policy(
                GovernedOperation::TerminalCommand,
                HitlTier::Tier0,
                Vec::new(),
            );
            ConsentRuntime::new(
                policy,
                ApprovalQueue::in_memory(),
                "workflow.integration".to_string(),
            )
        },
    };

    let engine = WorkflowEngine::default();
    let report = engine
        .execute(&workflow, json!({"seed": "nexus"}), &mut context)
        .expect("workflow should execute");

    assert_eq!(report.execution_order, vec!["A", "B", "C"]);
    let c_output = report.outputs.get("C").expect("C output should exist");
    assert_eq!(c_output["input"]["node_id"], "B");
    assert_eq!(c_output["input"]["input"]["node_id"], "A");
}

#[test]
fn test_integration_self_improvement_learning() {
    let mut tracker = PerformanceTracker::new_in_memory();
    for _ in 0..5 {
        tracker
            .track_outcome(
                "agent-social",
                TaskType::Posting,
                "AI launch post 9am",
                OutcomeResult::Success,
                TaskMetrics {
                    engagement_rate: Some(0.85),
                    approval_rate: Some(0.95),
                    ..TaskMetrics::default()
                },
            )
            .expect("morning post should be tracked");
    }
    for _ in 0..5 {
        tracker
            .track_outcome(
                "agent-social",
                TaskType::Posting,
                "AI launch post 3pm",
                OutcomeResult::Success,
                TaskMetrics {
                    engagement_rate: Some(0.28),
                    approval_rate: Some(0.95),
                    ..TaskMetrics::default()
                },
            )
            .expect("afternoon post should be tracked");
    }

    let insights = analyze_history(&tracker, "agent-social", TaskType::Posting)
        .expect("strategy learner should produce insights");
    assert!(!insights.recommendations.is_empty());
    assert!(
        insights
            .recommendations
            .iter()
            .any(|item| item.to_ascii_lowercase().contains("before 10am")),
        "expected timing recommendation from 10 tracked outcomes"
    );
}

fn workflow_node(id: &str) -> WorkflowNode {
    WorkflowNode {
        id: id.to_string(),
        label: format!("Node {id}"),
        kind: NodeKind::Action(ActionNode::RunCode),
        inputs: vec![NodePort {
            name: "input".to_string(),
            data_type: "json".to_string(),
            required: false,
        }],
        outputs: vec![NodePort {
            name: "result".to_string(),
            data_type: "json".to_string(),
            required: true,
        }],
        config: json!({}),
        capabilities_required: vec!["workflow.execute".to_string()],
        fuel_cost: 1,
        retry_limit: 0,
        error_strategy: NodeErrorStrategy::Halt,
    }
}

fn workflow_edge(
    from_node: &str,
    from_output: &str,
    to_node: &str,
    to_input: &str,
) -> WorkflowConnection {
    WorkflowConnection {
        from_node: from_node.to_string(),
        from_output: from_output.to_string(),
        to_node: to_node.to_string(),
        to_input: to_input.to_string(),
    }
}

fn created_file_content<'a>(changes: &'a [WebFileChange], path: &str) -> Option<&'a str> {
    changes.iter().find_map(|change| match change {
        WebFileChange::Create(candidate, content) if candidate == path => Some(content.as_str()),
        WebFileChange::Modify(candidate, _, content) if candidate == path => Some(content.as_str()),
        _ => None,
    })
}

fn created_files(changes: &[WebFileChange]) -> Vec<(String, String)> {
    let mut files = Vec::new();
    for change in changes {
        if let WebFileChange::Create(path, content) = change {
            files.push((path.clone(), content.clone()));
        }
    }
    files
}

fn write_file(path: impl AsRef<std::path::Path>, contents: &str) {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("parent directory should be created");
    }
    fs::write(path, contents).expect("file should be written");
}
