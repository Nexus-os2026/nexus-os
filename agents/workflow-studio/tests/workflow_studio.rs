use nexus_kernel::audit::AuditTrail;
use nexus_kernel::autonomy::{AutonomyGuard, AutonomyLevel};
use nexus_kernel::consent::{
    ApprovalQueue, ConsentPolicyEngine, ConsentRuntime, GovernedOperation, HitlTier,
};
use nexus_kernel::errors::AgentError;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use uuid::Uuid;
use workflow_studio_agent::engine::{NodeExecutor, NodeRunStatus, WorkflowContext, WorkflowEngine};
use workflow_studio_agent::nodes::{
    ActionNode, NodeErrorStrategy, NodeKind, NodePort, Workflow, WorkflowConnection, WorkflowNode,
};
use workflow_studio_agent::serialize::{load_workflow, save_workflow, WorkflowArchive};

#[derive(Debug)]
struct RecordingExecutor {
    sleeps_ms: HashMap<String, u64>,
    fail_counts: HashMap<String, u8>,
    attempts: Mutex<HashMap<String, u8>>,
    starts: Mutex<HashMap<String, Instant>>,
    finishes: Mutex<HashMap<String, Instant>>,
}

impl RecordingExecutor {
    fn new(sleeps_ms: HashMap<String, u64>, fail_counts: HashMap<String, u8>) -> Self {
        Self {
            sleeps_ms,
            fail_counts,
            attempts: Mutex::new(HashMap::new()),
            starts: Mutex::new(HashMap::new()),
            finishes: Mutex::new(HashMap::new()),
        }
    }
}

impl NodeExecutor for RecordingExecutor {
    fn execute(&self, node: &WorkflowNode, input: &Value) -> Result<Value, AgentError> {
        {
            let mut starts = self
                .starts
                .lock()
                .map_err(|_| AgentError::SupervisorError("starts lock poisoned".to_string()))?;
            starts.entry(node.id.clone()).or_insert_with(Instant::now);
        }

        let attempt = {
            let mut attempts = self
                .attempts
                .lock()
                .map_err(|_| AgentError::SupervisorError("attempts lock poisoned".to_string()))?;
            let value = attempts.entry(node.id.clone()).or_insert(0);
            *value = value.saturating_add(1);
            *value
        };

        if let Some(delay) = self.sleeps_ms.get(node.id.as_str()) {
            std::thread::sleep(Duration::from_millis(*delay));
        }

        if let Some(fail_count) = self.fail_counts.get(node.id.as_str()) {
            if attempt <= *fail_count {
                return Err(AgentError::SupervisorError(format!(
                    "intentional failure at {} attempt {}",
                    node.id, attempt
                )));
            }
        }

        if node.id == "D" {
            let object = input.as_object().ok_or_else(|| {
                AgentError::SupervisorError("node D expects merged object input".to_string())
            })?;
            if !(object.contains_key("B") && object.contains_key("C")) {
                return Err(AgentError::SupervisorError(
                    "node D missing B/C merged inputs".to_string(),
                ));
            }
        }

        {
            let mut finishes = self
                .finishes
                .lock()
                .map_err(|_| AgentError::SupervisorError("finishes lock poisoned".to_string()))?;
            finishes.insert(node.id.clone(), Instant::now());
        }

        Ok(json!({
            "node": node.id,
            "attempt": attempt,
            "input": input,
        }))
    }
}

#[test]
fn test_workflow_dag_execution() {
    let workflow = Workflow {
        id: "wf-dag".to_string(),
        name: "Linear".to_string(),
        description: "A -> B -> C".to_string(),
        nodes: vec![node("A"), node("B"), node("C")],
        connections: vec![
            edge("A", "result", "B", "input"),
            edge("B", "result", "C", "input"),
        ],
    };

    let executor = Arc::new(RecordingExecutor::new(HashMap::new(), HashMap::new()));
    let engine = WorkflowEngine::new(executor);
    let mut context = context_with_caps();

    let report = engine
        .execute(&workflow, json!({"seed": "value"}), &mut context)
        .expect("workflow should execute");

    assert_eq!(report.execution_order, vec!["A", "B", "C"]);
    let c_output = report.outputs.get("C").expect("C output should exist");
    assert_eq!(c_output["input"]["node"], "B");
    assert_eq!(c_output["input"]["input"]["node"], "A");
}

#[test]
fn test_parallel_execution() {
    let workflow = Workflow {
        id: "wf-parallel".to_string(),
        name: "Parallel".to_string(),
        description: "A -> (B,C) -> D".to_string(),
        nodes: vec![node("A"), node("B"), node("C"), node("D")],
        connections: vec![
            edge("A", "result", "B", "input"),
            edge("A", "result", "C", "input"),
            edge("B", "result", "D", "input"),
            edge("C", "result", "D", "input"),
        ],
    };

    let mut sleeps = HashMap::new();
    sleeps.insert("A".to_string(), 20);
    sleeps.insert("B".to_string(), 160);
    sleeps.insert("C".to_string(), 160);
    sleeps.insert("D".to_string(), 20);

    let executor = Arc::new(RecordingExecutor::new(sleeps, HashMap::new()));
    let engine = WorkflowEngine::new(executor.clone());
    let mut context = context_with_caps();

    let report = engine
        .execute(&workflow, json!({"seed": "value"}), &mut context)
        .expect("parallel workflow should execute");
    assert_eq!(
        report
            .records
            .iter()
            .filter(|record| record.status == NodeRunStatus::Success)
            .count(),
        4
    );

    let index_a = report
        .execution_order
        .iter()
        .position(|node| node == "A")
        .expect("A should be in execution order");
    let index_b = report
        .execution_order
        .iter()
        .position(|node| node == "B")
        .expect("B should be in execution order");
    let index_c = report
        .execution_order
        .iter()
        .position(|node| node == "C")
        .expect("C should be in execution order");
    let index_d = report
        .execution_order
        .iter()
        .position(|node| node == "D")
        .expect("D should be in execution order");
    assert!(index_a < index_b, "A must execute before B");
    assert!(index_a < index_c, "A must execute before C");
    assert!(index_b < index_d, "B must execute before D");
    assert!(index_c < index_d, "C must execute before D");
    assert!(report.outputs.contains_key("B"), "B output should exist");
    assert!(report.outputs.contains_key("C"), "C output should exist");
    assert!(report.outputs.contains_key("D"), "D output should exist");

    let starts = executor
        .starts
        .lock()
        .expect("starts lock must be available");
    let finishes = executor
        .finishes
        .lock()
        .expect("finishes lock must be available");
    let end_a = finishes.get("A").expect("A finish missing");
    let start_b = starts.get("B").expect("B start missing");
    let start_c = starts.get("C").expect("C start missing");
    let end_b = finishes.get("B").expect("B finish missing");
    let end_c = finishes.get("C").expect("C finish missing");
    let start_d = starts.get("D").expect("D start missing");
    assert!(
        *start_b >= *end_a,
        "B should not start until A has completed"
    );
    assert!(
        *start_c >= *end_a,
        "C should not start until A has completed"
    );
    assert!(
        *start_b <= *end_c && *start_c <= *end_b,
        "B and C should overlap to demonstrate parallel execution"
    );

    let bc_done = if end_b > end_c { end_b } else { end_c };
    assert!(*start_d >= *bc_done, "D should wait for B and C to finish");
}

#[test]
fn test_error_handling_retry() {
    let mut retry_node = node("B");
    retry_node.retry_limit = 3;
    retry_node.error_strategy = NodeErrorStrategy::Retry;

    let workflow = Workflow {
        id: "wf-retry".to_string(),
        name: "Retry".to_string(),
        description: "A -> B with retries".to_string(),
        nodes: vec![node("A"), retry_node],
        connections: vec![edge("A", "result", "B", "input")],
    };

    let mut fail_counts = HashMap::new();
    fail_counts.insert("B".to_string(), 2);

    let executor = Arc::new(RecordingExecutor::new(HashMap::new(), fail_counts));
    let engine = WorkflowEngine::new(executor);
    let mut context = context_with_caps();

    let report = engine
        .execute(&workflow, json!({"seed": "value"}), &mut context)
        .expect("workflow should eventually succeed after retries");

    let record_b = report
        .records
        .iter()
        .find(|record| record.node_id == "B")
        .expect("B record should exist");
    assert_eq!(record_b.status, NodeRunStatus::Success);
    assert_eq!(record_b.attempts, 3);
}

#[test]
fn test_workflow_serialization() {
    let workflow = Workflow {
        id: "wf-serialize".to_string(),
        name: "Serialize".to_string(),
        description: "serialization roundtrip".to_string(),
        nodes: vec![node("A"), node("B")],
        connections: vec![edge("A", "result", "B", "input")],
    };

    let archive = WorkflowArchive::from_workflow(workflow.clone(), "initial");
    let path = temp_path("workflow-archive");

    save_workflow(&path, &archive).expect("save should succeed");
    let loaded = load_workflow(&path).expect("load should succeed");

    assert_eq!(archive.schema_version, loaded.schema_version);
    assert_eq!(archive.versions.len(), loaded.versions.len());
    assert_eq!(loaded.current_workflow(), Some(&workflow));

    let _ = std::fs::remove_file(path);
}

fn node(id: &str) -> WorkflowNode {
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

fn edge(from_node: &str, from_output: &str, to_node: &str, to_input: &str) -> WorkflowConnection {
    WorkflowConnection {
        from_node: from_node.to_string(),
        from_output: from_output.to_string(),
        to_node: to_node.to_string(),
        to_input: to_input.to_string(),
    }
}

fn context_with_caps() -> WorkflowContext {
    let capabilities = ["workflow.execute".to_string()]
        .into_iter()
        .collect::<HashSet<_>>();
    let mut policy = ConsentPolicyEngine::default();
    policy.set_policy(
        GovernedOperation::TerminalCommand,
        HitlTier::Tier0,
        Vec::new(),
    );
    WorkflowContext {
        capabilities,
        fuel_remaining: 1_000,
        agent_id: Uuid::new_v4(),
        autonomy_guard: AutonomyGuard::new(AutonomyLevel::L1),
        audit_trail: AuditTrail::new(),
        consent_runtime: ConsentRuntime::new(
            policy,
            ApprovalQueue::in_memory(),
            "workflow.tests".to_string(),
        ),
    }
}

fn temp_path(prefix: &str) -> PathBuf {
    std::env::temp_dir().join(format!("{}-{}.json", prefix, Uuid::new_v4()))
}
