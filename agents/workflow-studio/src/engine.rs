use crate::nodes::{NodeErrorStrategy, NodeKind, Workflow, WorkflowConnection, WorkflowNode};
use nexus_kernel::audit::AuditTrail;
use nexus_kernel::autonomy::{AutonomyGuard, AutonomyLevel};
use nexus_kernel::errors::AgentError;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::sync::Arc;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct WorkflowContext {
    pub capabilities: HashSet<String>,
    pub fuel_remaining: u64,
    pub agent_id: uuid::Uuid,
    pub autonomy_guard: AutonomyGuard,
    pub audit_trail: AuditTrail,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeRunStatus {
    Success,
    Skipped,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeExecutionRecord {
    pub node_id: String,
    pub status: NodeRunStatus,
    pub attempts: u8,
    pub fuel_before: u64,
    pub fuel_after: u64,
    pub started_at: u64,
    pub finished_at: u64,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowCheckpoint {
    pub index: u32,
    pub node_id: String,
    pub status: NodeRunStatus,
    pub fuel_remaining: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowExecutionReport {
    pub workflow_id: String,
    pub execution_order: Vec<String>,
    pub records: Vec<NodeExecutionRecord>,
    pub outputs: BTreeMap<String, Value>,
    pub checkpoints: Vec<WorkflowCheckpoint>,
    pub halted: bool,
}

pub trait NodeExecutor: Send + Sync {
    fn execute(&self, node: &WorkflowNode, input: &Value) -> Result<Value, AgentError>;
}

#[derive(Debug, Default)]
pub struct DefaultNodeExecutor;

impl NodeExecutor for DefaultNodeExecutor {
    fn execute(&self, node: &WorkflowNode, input: &Value) -> Result<Value, AgentError> {
        let mut payload = Map::new();
        payload.insert("node_id".to_string(), Value::String(node.id.clone()));
        payload.insert("label".to_string(), Value::String(node.label.clone()));
        payload.insert(
            "kind".to_string(),
            Value::String(format!("{:?}", node.kind)),
        );
        payload.insert("input".to_string(), input.clone());

        // Keep behavior deterministic while still differentiating basic node classes.
        if matches!(node.kind, NodeKind::Logic(_)) {
            payload.insert("logic".to_string(), Value::Bool(true));
        }

        Ok(Value::Object(payload))
    }
}

pub struct WorkflowEngine {
    executor: Arc<dyn NodeExecutor>,
}

impl Default for WorkflowEngine {
    fn default() -> Self {
        Self::new(Arc::new(DefaultNodeExecutor))
    }
}

impl WorkflowEngine {
    pub fn new(executor: Arc<dyn NodeExecutor>) -> Self {
        Self { executor }
    }

    pub fn execute(
        &self,
        workflow: &Workflow,
        initial_input: Value,
        context: &mut WorkflowContext,
    ) -> Result<WorkflowExecutionReport, AgentError> {
        validate_workflow(workflow)?;

        let node_map = workflow
            .nodes
            .iter()
            .map(|node| (node.id.clone(), node.clone()))
            .collect::<HashMap<_, _>>();

        let mut incoming = HashMap::<String, Vec<WorkflowConnection>>::new();
        let mut outgoing = HashMap::<String, Vec<String>>::new();
        let mut indegree = HashMap::<String, usize>::new();

        for node in &workflow.nodes {
            indegree.insert(node.id.clone(), 0);
            incoming.insert(node.id.clone(), Vec::new());
            outgoing.insert(node.id.clone(), Vec::new());
        }

        for edge in &workflow.connections {
            outgoing
                .entry(edge.from_node.clone())
                .or_default()
                .push(edge.to_node.clone());
            incoming
                .entry(edge.to_node.clone())
                .or_default()
                .push(edge.clone());
            *indegree.entry(edge.to_node.clone()).or_default() += 1;
        }

        let mut pending = indegree
            .iter()
            .filter_map(|(node_id, count)| {
                if *count == 0 {
                    Some(node_id.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        pending.sort();

        let mut outputs = BTreeMap::<String, Value>::new();
        let mut records = Vec::<NodeExecutionRecord>::new();
        let mut checkpoints = Vec::<WorkflowCheckpoint>::new();
        let mut execution_order = Vec::<String>::new();
        let mut completed = BTreeSet::<String>::new();
        let mut halted = false;

        while !pending.is_empty() {
            let current_wave = pending.clone();
            pending.clear();

            let mut workers = Vec::new();
            for node_id in current_wave {
                let Some(node) = node_map.get(&node_id).cloned() else {
                    return Err(AgentError::SupervisorError(format!(
                        "node '{}' missing from map",
                        node_id
                    )));
                };

                let node_input =
                    build_node_input(&node, incoming.get(&node_id), &outputs, &initial_input);
                let fuel_before = context.fuel_remaining;
                context
                    .autonomy_guard
                    .require_tool_call(context.agent_id, &mut context.audit_trail)
                    .map_err(|error| AgentError::SupervisorError(error.to_string()))?;

                if !has_required_capabilities(context, &node) {
                    let record = NodeExecutionRecord {
                        node_id: node.id.clone(),
                        status: NodeRunStatus::Failed,
                        attempts: 0,
                        fuel_before,
                        fuel_after: context.fuel_remaining,
                        started_at: now_secs(),
                        finished_at: now_secs(),
                        error: Some("missing required capabilities".to_string()),
                    };
                    execution_order.push(node.id.clone());
                    records.push(record.clone());
                    checkpoints.push(WorkflowCheckpoint {
                        index: checkpoints.len() as u32,
                        node_id: node.id.clone(),
                        status: record.status.clone(),
                        fuel_remaining: context.fuel_remaining,
                    });
                    halted = true;
                    continue;
                }

                if context.fuel_remaining < node.fuel_cost {
                    let record = NodeExecutionRecord {
                        node_id: node.id.clone(),
                        status: NodeRunStatus::Failed,
                        attempts: 0,
                        fuel_before,
                        fuel_after: context.fuel_remaining,
                        started_at: now_secs(),
                        finished_at: now_secs(),
                        error: Some("insufficient fuel".to_string()),
                    };
                    execution_order.push(node.id.clone());
                    records.push(record.clone());
                    checkpoints.push(WorkflowCheckpoint {
                        index: checkpoints.len() as u32,
                        node_id: node.id.clone(),
                        status: record.status.clone(),
                        fuel_remaining: context.fuel_remaining,
                    });
                    halted = true;
                    continue;
                }

                context.fuel_remaining -= node.fuel_cost;
                let fuel_after = context.fuel_remaining;
                let executor = Arc::clone(&self.executor);

                workers.push(thread::spawn(move || {
                    run_node_with_policy(
                        executor.as_ref(),
                        node,
                        node_input,
                        fuel_before,
                        fuel_after,
                    )
                }));
            }

            let mut completed_in_wave = Vec::<String>::new();
            for worker in workers {
                let outcome = worker.join().map_err(|_| {
                    AgentError::SupervisorError("workflow worker thread panicked".to_string())
                })?;

                execution_order.push(outcome.record.node_id.clone());
                let node_id = outcome.record.node_id.clone();
                let node_status = outcome.record.status.clone();
                let node_error_strategy = outcome.error_strategy;

                if matches!(node_status, NodeRunStatus::Success | NodeRunStatus::Skipped) {
                    outputs.insert(node_id.clone(), outcome.output);
                    completed.insert(node_id.clone());
                    completed_in_wave.push(node_id.clone());
                } else {
                    halted = true;
                }

                records.push(outcome.record.clone());
                checkpoints.push(WorkflowCheckpoint {
                    index: checkpoints.len() as u32,
                    node_id,
                    status: node_status.clone(),
                    fuel_remaining: outcome.record.fuel_after,
                });

                if node_status == NodeRunStatus::Failed
                    && (node_error_strategy == NodeErrorStrategy::Halt
                        || node_error_strategy == NodeErrorStrategy::Retry)
                {
                    halted = true;
                }
            }

            for done_node in completed_in_wave {
                if let Some(neighbors) = outgoing.get(done_node.as_str()) {
                    for next in neighbors {
                        if let Some(value) = indegree.get_mut(next.as_str()) {
                            if *value > 0 {
                                *value -= 1;
                            }
                        }
                    }
                }
            }

            if halted {
                break;
            }

            let mut next_wave = indegree
                .iter()
                .filter_map(|(node_id, count)| {
                    if *count == 0 && !completed.contains(node_id) {
                        Some(node_id.clone())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            next_wave.sort();
            pending = next_wave;
        }

        if !halted {
            let expected = workflow.nodes.len();
            if completed.len() != expected {
                return Err(AgentError::SupervisorError(
                    "workflow execution stopped before all nodes completed; possible cycle"
                        .to_string(),
                ));
            }
        }

        Ok(WorkflowExecutionReport {
            workflow_id: workflow.id.clone(),
            execution_order,
            records,
            outputs,
            checkpoints,
            halted,
        })
    }
}

#[derive(Debug, Clone)]
struct WorkerOutcome {
    record: NodeExecutionRecord,
    output: Value,
    error_strategy: NodeErrorStrategy,
}

fn run_node_with_policy(
    executor: &dyn NodeExecutor,
    node: WorkflowNode,
    input: Value,
    fuel_before: u64,
    fuel_after: u64,
) -> WorkerOutcome {
    let started = now_secs();
    let mut attempts = 0_u8;

    loop {
        attempts = attempts.saturating_add(1);
        match executor.execute(&node, &input) {
            Ok(output) => {
                return WorkerOutcome {
                    record: NodeExecutionRecord {
                        node_id: node.id,
                        status: NodeRunStatus::Success,
                        attempts,
                        fuel_before,
                        fuel_after,
                        started_at: started,
                        finished_at: now_secs(),
                        error: None,
                    },
                    output,
                    error_strategy: node.error_strategy,
                };
            }
            Err(error) => {
                let retry_allowed = node.error_strategy == NodeErrorStrategy::Retry
                    && attempts <= node.retry_limit.saturating_add(1);
                if retry_allowed && attempts <= node.retry_limit {
                    continue;
                }

                if node.error_strategy == NodeErrorStrategy::Skip {
                    return WorkerOutcome {
                        record: NodeExecutionRecord {
                            node_id: node.id,
                            status: NodeRunStatus::Skipped,
                            attempts,
                            fuel_before,
                            fuel_after,
                            started_at: started,
                            finished_at: now_secs(),
                            error: Some(error.to_string()),
                        },
                        output: Value::Null,
                        error_strategy: node.error_strategy,
                    };
                }

                return WorkerOutcome {
                    record: NodeExecutionRecord {
                        node_id: node.id,
                        status: NodeRunStatus::Failed,
                        attempts,
                        fuel_before,
                        fuel_after,
                        started_at: started,
                        finished_at: now_secs(),
                        error: Some(error.to_string()),
                    },
                    output: Value::Null,
                    error_strategy: node.error_strategy,
                };
            }
        }
    }
}

fn build_node_input(
    node: &WorkflowNode,
    incoming: Option<&Vec<WorkflowConnection>>,
    outputs: &BTreeMap<String, Value>,
    initial_input: &Value,
) -> Value {
    let Some(incoming) = incoming else {
        return initial_input.clone();
    };

    if incoming.is_empty() {
        return initial_input.clone();
    }

    if incoming.len() == 1 {
        let predecessor = incoming[0].from_node.as_str();
        return outputs
            .get(predecessor)
            .cloned()
            .unwrap_or_else(|| Value::Object(Map::new()));
    }

    let mut payload = Map::new();
    for edge in incoming {
        payload.insert(
            edge.from_node.clone(),
            outputs
                .get(edge.from_node.as_str())
                .cloned()
                .unwrap_or_else(|| Value::Object(Map::new())),
        );
    }
    payload.insert("node".to_string(), Value::String(node.id.clone()));
    Value::Object(payload)
}

fn has_required_capabilities(context: &WorkflowContext, node: &WorkflowNode) -> bool {
    node.capabilities_required
        .iter()
        .all(|capability| context.capabilities.contains(capability))
}

fn validate_workflow(workflow: &Workflow) -> Result<(), AgentError> {
    if workflow.nodes.is_empty() {
        return Err(AgentError::ManifestError(
            "workflow must contain at least one node".to_string(),
        ));
    }

    let node_ids = workflow
        .nodes
        .iter()
        .map(|node| node.id.clone())
        .collect::<HashSet<_>>();

    for edge in &workflow.connections {
        if !node_ids.contains(edge.from_node.as_str()) || !node_ids.contains(edge.to_node.as_str())
        {
            return Err(AgentError::ManifestError(format!(
                "connection references unknown node: {} -> {}",
                edge.from_node, edge.to_node
            )));
        }
    }

    if has_cycle(workflow) {
        return Err(AgentError::ManifestError(
            "workflow graph must be acyclic".to_string(),
        ));
    }

    Ok(())
}

fn has_cycle(workflow: &Workflow) -> bool {
    let mut indegree = HashMap::<String, usize>::new();
    let mut outgoing = HashMap::<String, Vec<String>>::new();

    for node in &workflow.nodes {
        indegree.insert(node.id.clone(), 0);
        outgoing.insert(node.id.clone(), Vec::new());
    }

    for edge in &workflow.connections {
        outgoing
            .entry(edge.from_node.clone())
            .or_default()
            .push(edge.to_node.clone());
        *indegree.entry(edge.to_node.clone()).or_default() += 1;
    }

    let mut queue = indegree
        .iter()
        .filter_map(|(node_id, count)| {
            if *count == 0 {
                Some(node_id.clone())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    let mut visited = 0_usize;
    while let Some(node) = queue.pop() {
        visited += 1;
        if let Some(neighbors) = outgoing.get(node.as_str()) {
            for neighbor in neighbors {
                if let Some(degree) = indegree.get_mut(neighbor.as_str()) {
                    *degree = degree.saturating_sub(1);
                    if *degree == 0 {
                        queue.push(neighbor.clone());
                    }
                }
            }
        }
    }

    visited != workflow.nodes.len()
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

impl Default for WorkflowContext {
    fn default() -> Self {
        Self {
            capabilities: HashSet::new(),
            fuel_remaining: 0,
            agent_id: uuid::Uuid::nil(),
            autonomy_guard: AutonomyGuard::new(AutonomyLevel::L0),
            audit_trail: AuditTrail::new(),
        }
    }
}
