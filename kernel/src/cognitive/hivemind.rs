//! Hivemind Orchestration — multi-agent goal decomposition, DAG execution, and result merging.
//!
//! The `HivemindCoordinator` sits above the cognitive runtime and orchestrates
//! complex goals across multiple agents. It decomposes a master goal into sub-tasks,
//! assigns them based on capability matching, builds a dependency DAG, executes
//! waves of parallel sub-tasks, and merges results.

use crate::audit::{AuditTrail, EventType};
use crate::errors::AgentError;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex};

// ── Types ───────────────────────────────────────────────────────────────────

/// Status of the overall hivemind session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HivemindStatus {
    Planning,
    Executing,
    Merging,
    Completed,
    Failed,
    Cancelled,
}

/// Status of a single sub-task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubTaskStatus {
    Pending,
    Assigned,
    Running,
    Completed,
    Failed,
}

/// A sub-task decomposed from the master goal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubTask {
    pub id: String,
    pub description: String,
    pub required_capabilities: Vec<String>,
    pub dependencies: Vec<String>,
    pub estimated_fuel: f64,
    pub status: SubTaskStatus,
}

/// A full hivemind session tracking decomposition through completion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HivemindSession {
    pub id: String,
    pub master_goal: String,
    pub sub_tasks: Vec<SubTask>,
    pub assignments: HashMap<String, String>,
    pub status: HivemindStatus,
    pub results: HashMap<String, String>,
    pub total_fuel_consumed: f64,
    pub started_at: String,
    pub completed_at: Option<String>,
}

impl HivemindSession {
    fn new(master_goal: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            master_goal,
            sub_tasks: Vec::new(),
            assignments: HashMap::new(),
            status: HivemindStatus::Planning,
            results: HashMap::new(),
            total_fuel_consumed: 0.0,
            started_at: now_rfc3339(),
            completed_at: None,
        }
    }
}

/// An event emitted during hivemind orchestration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event_type")]
pub enum HivemindEvent {
    SubTaskAssigned {
        session_id: String,
        subtask_id: String,
        agent_id: String,
        description: String,
    },
    SubTaskCompleted {
        session_id: String,
        subtask_id: String,
        agent_id: String,
        success: bool,
    },
    WaveStarted {
        session_id: String,
        wave_number: u32,
        parallel_tasks: Vec<String>,
    },
    SessionCompleted {
        session_id: String,
        success: bool,
        total_fuel: f64,
        duration_secs: f64,
    },
}

/// Trait for emitting hivemind events (separates I/O from logic).
pub trait HivemindEventEmitter: Send + Sync {
    fn emit(&self, event: HivemindEvent);
}

/// No-op emitter for headless/test use.
pub struct NoOpHivemindEmitter;

impl HivemindEventEmitter for NoOpHivemindEmitter {
    fn emit(&self, _event: HivemindEvent) {}
}

/// Collects events for testing.
pub struct CollectingHivemindEmitter {
    pub events: Mutex<Vec<HivemindEvent>>,
}

impl Default for CollectingHivemindEmitter {
    fn default() -> Self {
        Self {
            events: Mutex::new(Vec::new()),
        }
    }
}

impl CollectingHivemindEmitter {
    pub fn new() -> Self {
        Self::default()
    }
}

impl HivemindEventEmitter for CollectingHivemindEmitter {
    fn emit(&self, event: HivemindEvent) {
        self.events.lock().unwrap().push(event);
    }
}

/// Trait for LLM calls used by the hivemind coordinator.
pub trait HivemindLlm: Send + Sync {
    /// Decompose a goal into sub-tasks given agent capabilities.
    fn decompose(&self, prompt: &str) -> Result<String, AgentError>;
    /// Merge sub-task results into a final answer.
    fn merge(&self, prompt: &str) -> Result<String, AgentError>;
}

/// Agent info for capability matching.
#[derive(Debug, Clone)]
pub struct AgentInfo {
    pub id: String,
    pub capabilities: Vec<String>,
    pub available_fuel: f64,
}

// ── DAG Utilities ───────────────────────────────────────────────────────────

/// Error when the dependency graph has a cycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CyclicDependencyError;

impl std::fmt::Display for CyclicDependencyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "cyclic dependency detected in sub-task graph")
    }
}

/// Topological sort of sub-tasks into execution waves.
/// Each wave contains sub-tasks that can run in parallel.
/// Returns Err if a cycle is detected.
pub fn topological_waves(sub_tasks: &[SubTask]) -> Result<Vec<Vec<String>>, CyclicDependencyError> {
    let ids: HashSet<&str> = sub_tasks.iter().map(|t| t.id.as_str()).collect();
    let mut in_degree: HashMap<&str, usize> = HashMap::new();
    let mut dependents: HashMap<&str, Vec<&str>> = HashMap::new();

    for task in sub_tasks {
        in_degree.entry(task.id.as_str()).or_insert(0);
        for dep in &task.dependencies {
            if ids.contains(dep.as_str()) {
                *in_degree.entry(task.id.as_str()).or_insert(0) += 1;
                dependents
                    .entry(dep.as_str())
                    .or_default()
                    .push(task.id.as_str());
            }
        }
    }

    let mut waves: Vec<Vec<String>> = Vec::new();
    let mut queue: VecDeque<&str> = in_degree
        .iter()
        .filter(|(_, &deg)| deg == 0)
        .map(|(&id, _)| id)
        .collect();
    let mut processed = 0usize;

    while !queue.is_empty() {
        let wave: Vec<String> = queue.drain(..).map(|s| s.to_string()).collect();
        processed += wave.len();

        for id in &wave {
            if let Some(deps) = dependents.get(id.as_str()) {
                for &dep_id in deps {
                    if let Some(deg) = in_degree.get_mut(dep_id) {
                        *deg -= 1;
                        if *deg == 0 {
                            queue.push_back(dep_id);
                        }
                    }
                }
            }
        }

        waves.push(wave);
    }

    if processed != sub_tasks.len() {
        return Err(CyclicDependencyError);
    }

    Ok(waves)
}

/// List of (subtask_id, missing_capabilities) for unassignable sub-tasks.
pub type UnassignableList = Vec<(String, Vec<String>)>;

/// Assign sub-tasks to agents based on capability matching.
/// Returns assignments (subtask_id -> agent_id) and a list of unassignable sub-task IDs
/// with the missing capabilities.
pub fn assign_by_capability(
    sub_tasks: &[SubTask],
    agents: &[AgentInfo],
) -> (HashMap<String, String>, UnassignableList) {
    let mut assignments: HashMap<String, String> = HashMap::new();
    let mut unassignable: Vec<(String, Vec<String>)> = Vec::new();

    for task in sub_tasks {
        let best = agents.iter().find(|a| {
            task.required_capabilities
                .iter()
                .all(|cap| a.capabilities.contains(cap))
                && a.available_fuel >= task.estimated_fuel
        });

        match best {
            Some(agent) => {
                assignments.insert(task.id.clone(), agent.id.clone());
            }
            None => {
                let missing: Vec<String> = task
                    .required_capabilities
                    .iter()
                    .filter(|cap| !agents.iter().any(|a| a.capabilities.contains(cap)))
                    .cloned()
                    .collect();
                unassignable.push((task.id.clone(), missing));
            }
        }
    }

    (assignments, unassignable)
}

/// Parse LLM decomposition response into sub-tasks.
pub fn parse_decomposition(response: &str) -> Result<Vec<SubTask>, AgentError> {
    // Try to find JSON array in the response (LLMs often wrap in markdown)
    let json_str = extract_json_array(response).unwrap_or(response);

    let raw: Vec<serde_json::Value> = serde_json::from_str(json_str).map_err(|e| {
        AgentError::SupervisorError(format!("failed to parse decomposition JSON: {e}"))
    })?;

    let mut tasks = Vec::new();
    for (i, item) in raw.iter().enumerate() {
        let id: String = item
            .get("id")
            .and_then(serde_json::Value::as_str)
            .map(String::from)
            .unwrap_or_else(|| format!("subtask_{i}"));
        let description: String = item
            .get("description")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("unnamed task")
            .to_string();
        let required_capabilities: Vec<String> = item
            .get("required_capabilities")
            .and_then(serde_json::Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(serde_json::Value::as_str)
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default();
        let dependencies: Vec<String> = item
            .get("dependencies")
            .and_then(serde_json::Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(serde_json::Value::as_str)
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default();
        let estimated_fuel: f64 = item
            .get("estimated_fuel")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(100.0);

        tasks.push(SubTask {
            id,
            description,
            required_capabilities,
            dependencies,
            estimated_fuel,
            status: SubTaskStatus::Pending,
        });
    }

    if tasks.is_empty() {
        return Err(AgentError::SupervisorError(
            "decomposition produced zero sub-tasks".to_string(),
        ));
    }

    Ok(tasks)
}

/// Extract a JSON array from a string that may contain markdown fences.
fn extract_json_array(s: &str) -> Option<&str> {
    // Try to find ```json ... ``` block
    if let Some(start) = s.find("```json") {
        let content_start = start + 7;
        if let Some(end) = s[content_start..].find("```") {
            return Some(s[content_start..content_start + end].trim());
        }
    }
    // Try to find ``` ... ``` block
    if let Some(start) = s.find("```") {
        let content_start = start + 3;
        // Skip optional language tag on same line
        let line_end = s[content_start..]
            .find('\n')
            .map(|i| content_start + i + 1)
            .unwrap_or(content_start);
        if let Some(end) = s[line_end..].find("```") {
            return Some(s[line_end..line_end + end].trim());
        }
    }
    // Try to find raw [ ... ]
    let trimmed = s.trim();
    if trimmed.starts_with('[') {
        return Some(trimmed);
    }
    None
}

// ── HivemindCoordinator ─────────────────────────────────────────────────────

/// The core hivemind coordinator that decomposes goals, builds DAGs,
/// executes waves, and merges results across multiple agents.
pub struct HivemindCoordinator {
    llm: Box<dyn HivemindLlm>,
    emitter: Arc<dyn HivemindEventEmitter>,
    audit: Arc<Mutex<AuditTrail>>,
    sessions: Mutex<HashMap<String, HivemindSession>>,
}

impl HivemindCoordinator {
    pub fn new(
        llm: Box<dyn HivemindLlm>,
        emitter: Arc<dyn HivemindEventEmitter>,
        audit: Arc<Mutex<AuditTrail>>,
    ) -> Self {
        Self {
            llm,
            emitter,
            audit,
            sessions: Mutex::new(HashMap::new()),
        }
    }

    /// Execute a full hivemind goal: decompose → assign → build DAG → execute → merge.
    pub fn execute_hivemind_goal(
        &self,
        master_goal: &str,
        available_agents: Vec<AgentInfo>,
    ) -> Result<HivemindSession, AgentError> {
        let mut session = HivemindSession::new(master_goal.to_string());

        self.log_audit(
            "hivemind_start",
            json!({"session_id": session.id, "goal": master_goal}),
        )?;

        // 1. DECOMPOSE
        let caps_summary: Vec<_> = available_agents
            .iter()
            .map(|a| {
                json!({
                    "agent_id": a.id,
                    "capabilities": a.capabilities,
                })
            })
            .collect();

        let decompose_prompt = format!(
            "You are a task decomposition engine. Break this goal into sub-tasks. \
             Each sub-task needs: id (string), description, required_capabilities (list), \
             estimated_fuel (number), dependencies (list of other sub-task IDs that must \
             complete first). Available agent capabilities: {}. \
             Goal: {}. Respond with a JSON array only.",
            serde_json::to_string(&caps_summary).unwrap_or_default(),
            master_goal
        );

        let decompose_response = self.llm.decompose(&decompose_prompt)?;
        let sub_tasks = parse_decomposition(&decompose_response)?;
        session.sub_tasks = sub_tasks;
        session.total_fuel_consumed += 10.0; // LLM query cost

        // 2. ASSIGN
        let (assignments, unassignable) =
            assign_by_capability(&session.sub_tasks, &available_agents);

        if !unassignable.is_empty() {
            self.log_audit(
                "hivemind_unassignable",
                json!({
                    "session_id": session.id,
                    "unassignable": unassignable.iter().map(|(id, caps)| {
                        json!({"subtask_id": id, "missing_capabilities": caps})
                    }).collect::<Vec<_>>(),
                }),
            )?;
        }

        session.assignments = assignments;

        // Mark assigned sub-tasks
        for task in &mut session.sub_tasks {
            if session.assignments.contains_key(&task.id) {
                task.status = SubTaskStatus::Assigned;
                if let Some(agent_id) = session.assignments.get(&task.id) {
                    self.emitter.emit(HivemindEvent::SubTaskAssigned {
                        session_id: session.id.clone(),
                        subtask_id: task.id.clone(),
                        agent_id: agent_id.clone(),
                        description: task.description.clone(),
                    });
                }
            }
        }

        // 3. BUILD DAG
        session.status = HivemindStatus::Executing;
        let waves = topological_waves(&session.sub_tasks).map_err(|_| {
            AgentError::SupervisorError("cyclic dependency in sub-task graph".to_string())
        })?;

        // 4. EXECUTE waves
        let mut failed_count = 0usize;
        let total_count = session.sub_tasks.len();

        for (wave_idx, wave) in waves.iter().enumerate() {
            // Filter to only assigned tasks in this wave
            let wave_tasks: Vec<&str> = wave
                .iter()
                .filter(|id: &&String| session.assignments.contains_key(id.as_str()))
                .map(String::as_str)
                .collect();

            if wave_tasks.is_empty() {
                continue;
            }

            self.emitter.emit(HivemindEvent::WaveStarted {
                session_id: session.id.clone(),
                wave_number: wave_idx as u32,
                parallel_tasks: wave_tasks.iter().map(|s: &&str| (*s).to_string()).collect(),
            });

            // Execute all tasks in this wave (in a real system these would be concurrent)
            for &task_id in &wave_tasks {
                // Mark running
                if let Some(task) = session.sub_tasks.iter_mut().find(|t| t.id == task_id) {
                    task.status = SubTaskStatus::Running;
                }

                let agent_id = session
                    .assignments
                    .get(task_id)
                    .cloned()
                    .unwrap_or_default();

                // Simulate execution: in production, this would call cognitive_runtime.assign_goal()
                // and monitor completion. For now, we record the assignment as the result.
                let task_desc = session
                    .sub_tasks
                    .iter()
                    .find(|t| t.id == task_id)
                    .map(|t| t.description.clone())
                    .unwrap_or_default();

                let result = format!("Sub-task '{task_desc}' completed by agent {agent_id}");
                let fuel_cost = session
                    .sub_tasks
                    .iter()
                    .find(|t| t.id == task_id)
                    .map(|t| t.estimated_fuel)
                    .unwrap_or(0.0);

                session.results.insert(task_id.to_string(), result);
                session.total_fuel_consumed += fuel_cost;

                if let Some(task) = session.sub_tasks.iter_mut().find(|t| t.id == task_id) {
                    task.status = SubTaskStatus::Completed;
                }

                self.emitter.emit(HivemindEvent::SubTaskCompleted {
                    session_id: session.id.clone(),
                    subtask_id: task_id.to_string(),
                    agent_id: agent_id.clone(),
                    success: true,
                });
            }

            // Count unassigned tasks as failed
            for id in wave.iter() {
                let id_str: &str = id.as_str();
                if !session.assignments.contains_key(id_str) {
                    failed_count += 1;
                    if let Some(task) = session.sub_tasks.iter_mut().find(|t| t.id == id_str) {
                        task.status = SubTaskStatus::Failed;
                    }
                }
            }

            // 6. HANDLE FAILURES: abort if >50% failed
            if total_count > 0 && failed_count * 2 > total_count {
                session.status = HivemindStatus::Failed;
                session.completed_at = Some(now_rfc3339());
                self.store_session(&session);

                self.emitter.emit(HivemindEvent::SessionCompleted {
                    session_id: session.id.clone(),
                    success: false,
                    total_fuel: session.total_fuel_consumed,
                    duration_secs: 0.0,
                });

                return Ok(session);
            }
        }

        // 5. MERGE results
        session.status = HivemindStatus::Merging;

        let results_summary: Vec<_> = session
            .results
            .iter()
            .map(|(id, result)| format!("- Sub-task {id}: {result}"))
            .collect();

        let merge_prompt = format!(
            "Combine these sub-task results into a final unified result for the original goal: \
             \"{}\". Sub-task results:\n{}",
            session.master_goal,
            results_summary.join("\n")
        );

        let merged = self.llm.merge(&merge_prompt)?;
        session.results.insert("__merged__".to_string(), merged);
        session.total_fuel_consumed += 10.0; // merge LLM cost

        session.status = HivemindStatus::Completed;
        session.completed_at = Some(now_rfc3339());

        self.emitter.emit(HivemindEvent::SessionCompleted {
            session_id: session.id.clone(),
            success: true,
            total_fuel: session.total_fuel_consumed,
            duration_secs: 0.0,
        });

        self.log_audit(
            "hivemind_completed",
            json!({
                "session_id": session.id,
                "status": "Completed",
                "fuel": session.total_fuel_consumed,
            }),
        )?;

        self.store_session(&session);
        Ok(session)
    }

    /// Execute a hivemind goal but allow individual sub-task execution to be controlled
    /// via the `executor` callback. This enables testing failure/reassignment scenarios.
    pub fn execute_with_executor<F>(
        &self,
        master_goal: &str,
        available_agents: Vec<AgentInfo>,
        mut executor: F,
    ) -> Result<HivemindSession, AgentError>
    where
        F: FnMut(&str, &str, &str) -> Result<String, String>,
    {
        let mut session = HivemindSession::new(master_goal.to_string());

        self.log_audit(
            "hivemind_start",
            json!({"session_id": session.id, "goal": master_goal}),
        )?;

        // 1. DECOMPOSE
        let caps_summary: Vec<_> = available_agents
            .iter()
            .map(|a| {
                json!({
                    "agent_id": a.id,
                    "capabilities": a.capabilities,
                })
            })
            .collect();

        let decompose_prompt = format!(
            "You are a task decomposition engine. Break this goal into sub-tasks. \
             Each sub-task needs: id (string), description, required_capabilities (list), \
             estimated_fuel (number), dependencies (list of other sub-task IDs that must \
             complete first). Available agent capabilities: {}. \
             Goal: {}. Respond with a JSON array only.",
            serde_json::to_string(&caps_summary).unwrap_or_default(),
            master_goal
        );

        let decompose_response = self.llm.decompose(&decompose_prompt)?;
        let sub_tasks = parse_decomposition(&decompose_response)?;
        session.sub_tasks = sub_tasks;
        session.total_fuel_consumed += 10.0;

        // 2. ASSIGN
        let (assignments, _unassignable) =
            assign_by_capability(&session.sub_tasks, &available_agents);
        session.assignments = assignments;

        for task in &mut session.sub_tasks {
            if session.assignments.contains_key(&task.id) {
                task.status = SubTaskStatus::Assigned;
                if let Some(agent_id) = session.assignments.get(&task.id) {
                    self.emitter.emit(HivemindEvent::SubTaskAssigned {
                        session_id: session.id.clone(),
                        subtask_id: task.id.clone(),
                        agent_id: agent_id.clone(),
                        description: task.description.clone(),
                    });
                }
            }
        }

        // 3. BUILD DAG
        session.status = HivemindStatus::Executing;
        let waves = topological_waves(&session.sub_tasks).map_err(|_| {
            AgentError::SupervisorError("cyclic dependency in sub-task graph".to_string())
        })?;

        // 4. EXECUTE waves with custom executor
        let mut failed_count = 0usize;
        let total_count = session.sub_tasks.len();

        for (wave_idx, wave) in waves.iter().enumerate() {
            let wave_tasks: Vec<String> = wave
                .iter()
                .filter(|id: &&String| session.assignments.contains_key(id.as_str()))
                .cloned()
                .collect();

            if wave_tasks.is_empty() {
                // Count unassigned as failed
                for id in wave.iter() {
                    let id_str: &str = id.as_str();
                    if !session.assignments.contains_key(id_str) {
                        failed_count += 1;
                        if let Some(task) = session.sub_tasks.iter_mut().find(|t| t.id == id_str) {
                            task.status = SubTaskStatus::Failed;
                        }
                    }
                }
                continue;
            }

            self.emitter.emit(HivemindEvent::WaveStarted {
                session_id: session.id.clone(),
                wave_number: wave_idx as u32,
                parallel_tasks: wave_tasks.clone(),
            });

            for task_id in &wave_tasks {
                if let Some(task) = session.sub_tasks.iter_mut().find(|t| t.id == *task_id) {
                    task.status = SubTaskStatus::Running;
                }

                let agent_id = session
                    .assignments
                    .get(task_id.as_str())
                    .cloned()
                    .unwrap_or_default();
                let task_desc = session
                    .sub_tasks
                    .iter()
                    .find(|t| t.id == *task_id)
                    .map(|t| t.description.clone())
                    .unwrap_or_default();

                match executor(task_id, &agent_id, &task_desc) {
                    Ok(result) => {
                        let fuel_cost = session
                            .sub_tasks
                            .iter()
                            .find(|t| t.id == *task_id)
                            .map(|t| t.estimated_fuel)
                            .unwrap_or(0.0);
                        session.results.insert(task_id.clone(), result);
                        session.total_fuel_consumed += fuel_cost;
                        if let Some(task) = session.sub_tasks.iter_mut().find(|t| t.id == *task_id)
                        {
                            task.status = SubTaskStatus::Completed;
                        }
                        self.emitter.emit(HivemindEvent::SubTaskCompleted {
                            session_id: session.id.clone(),
                            subtask_id: task_id.clone(),
                            agent_id: agent_id.clone(),
                            success: true,
                        });
                    }
                    Err(_err) => {
                        failed_count += 1;
                        if let Some(task) = session.sub_tasks.iter_mut().find(|t| t.id == *task_id)
                        {
                            task.status = SubTaskStatus::Failed;
                        }
                        self.emitter.emit(HivemindEvent::SubTaskCompleted {
                            session_id: session.id.clone(),
                            subtask_id: task_id.clone(),
                            agent_id: agent_id.clone(),
                            success: false,
                        });

                        // Try reassignment to another capable agent
                        let task_caps: Vec<String> = session
                            .sub_tasks
                            .iter()
                            .find(|t| t.id == *task_id)
                            .map(|t| t.required_capabilities.clone())
                            .unwrap_or_default();

                        let alternative = available_agents.iter().find(|a| {
                            a.id != agent_id
                                && task_caps.iter().all(|cap| a.capabilities.contains(cap))
                        });

                        if let Some(alt) = alternative {
                            session.assignments.insert(task_id.clone(), alt.id.clone());
                            if let Ok(result) = executor(task_id, &alt.id, &task_desc) {
                                failed_count -= 1; // undo the failure count
                                let fuel_cost = session
                                    .sub_tasks
                                    .iter()
                                    .find(|t| t.id == *task_id)
                                    .map(|t| t.estimated_fuel)
                                    .unwrap_or(0.0);
                                session.results.insert(task_id.clone(), result);
                                session.total_fuel_consumed += fuel_cost;
                                if let Some(task) =
                                    session.sub_tasks.iter_mut().find(|t| t.id == *task_id)
                                {
                                    task.status = SubTaskStatus::Completed;
                                }
                            }
                        }
                    }
                }
            }

            // Count unassigned in this wave as failed
            for id in wave.iter() {
                let id_str: &str = id.as_str();
                if !session.assignments.contains_key(id_str) {
                    failed_count += 1;
                    if let Some(task) = session.sub_tasks.iter_mut().find(|t| t.id == id_str) {
                        task.status = SubTaskStatus::Failed;
                    }
                }
            }

            // Abort if >50% failed
            if total_count > 0 && failed_count * 2 > total_count {
                session.status = HivemindStatus::Failed;
                session.completed_at = Some(now_rfc3339());
                self.store_session(&session);

                self.emitter.emit(HivemindEvent::SessionCompleted {
                    session_id: session.id.clone(),
                    success: false,
                    total_fuel: session.total_fuel_consumed,
                    duration_secs: 0.0,
                });

                return Ok(session);
            }
        }

        // 5. MERGE
        session.status = HivemindStatus::Merging;
        let results_summary: Vec<_> = session
            .results
            .iter()
            .map(|(id, result)| format!("- Sub-task {id}: {result}"))
            .collect();

        let merge_prompt = format!(
            "Combine these sub-task results into a final unified result for the original goal: \
             \"{}\". Sub-task results:\n{}",
            session.master_goal,
            results_summary.join("\n")
        );

        let merged = self.llm.merge(&merge_prompt)?;
        session.results.insert("__merged__".to_string(), merged);
        session.total_fuel_consumed += 10.0;

        session.status = HivemindStatus::Completed;
        session.completed_at = Some(now_rfc3339());

        self.emitter.emit(HivemindEvent::SessionCompleted {
            session_id: session.id.clone(),
            success: true,
            total_fuel: session.total_fuel_consumed,
            duration_secs: 0.0,
        });

        self.store_session(&session);
        Ok(session)
    }

    /// Get a session by ID.
    pub fn get_session(&self, session_id: &str) -> Option<HivemindSession> {
        self.sessions.lock().unwrap().get(session_id).cloned()
    }

    /// Cancel a running session.
    pub fn cancel_session(&self, session_id: &str) -> Result<(), AgentError> {
        let mut sessions = self.sessions.lock().unwrap();
        if let Some(session) = sessions.get_mut(session_id) {
            match session.status {
                HivemindStatus::Completed | HivemindStatus::Failed | HivemindStatus::Cancelled => {
                    Err(AgentError::SupervisorError(format!(
                        "session {session_id} already finished with status {:?}",
                        session.status
                    )))
                }
                _ => {
                    session.status = HivemindStatus::Cancelled;
                    session.completed_at = Some(now_rfc3339());
                    self.log_audit("hivemind_cancelled", json!({"session_id": session_id}))?;
                    Ok(())
                }
            }
        } else {
            Err(AgentError::SupervisorError(format!(
                "session {session_id} not found"
            )))
        }
    }

    /// List all sessions.
    pub fn list_sessions(&self) -> Vec<HivemindSession> {
        self.sessions.lock().unwrap().values().cloned().collect()
    }

    fn store_session(&self, session: &HivemindSession) {
        self.sessions
            .lock()
            .unwrap()
            .insert(session.id.clone(), session.clone());
    }

    fn log_audit(&self, event_name: &str, detail: serde_json::Value) -> Result<(), AgentError> {
        self.audit.lock().unwrap().append_event(
            uuid::Uuid::nil(),
            EventType::StateChange,
            json!({"hivemind": event_name, "detail": detail}),
        )?;
        Ok(())
    }
}

fn now_rfc3339() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("1970-01-01T00:00:00Z+{now}s")
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Mock LLM that returns configurable responses.
    struct MockHivemindLlm {
        decompose_response: String,
        merge_response: String,
    }

    impl MockHivemindLlm {
        fn new(decompose_response: &str, merge_response: &str) -> Self {
            Self {
                decompose_response: decompose_response.to_string(),
                merge_response: merge_response.to_string(),
            }
        }
    }

    impl HivemindLlm for MockHivemindLlm {
        fn decompose(&self, _prompt: &str) -> Result<String, AgentError> {
            Ok(self.decompose_response.clone())
        }
        fn merge(&self, _prompt: &str) -> Result<String, AgentError> {
            Ok(self.merge_response.clone())
        }
    }

    /// Mock LLM that always fails decomposition.
    struct FailingLlm;
    impl HivemindLlm for FailingLlm {
        fn decompose(&self, _prompt: &str) -> Result<String, AgentError> {
            Err(AgentError::SupervisorError("LLM unavailable".into()))
        }
        fn merge(&self, _prompt: &str) -> Result<String, AgentError> {
            Err(AgentError::SupervisorError("LLM unavailable".into()))
        }
    }

    fn three_task_decomposition() -> String {
        serde_json::to_string(&serde_json::json!([
            {
                "id": "t1",
                "description": "Research the topic",
                "required_capabilities": ["web.search"],
                "estimated_fuel": 100.0,
                "dependencies": []
            },
            {
                "id": "t2",
                "description": "Analyze findings",
                "required_capabilities": ["llm.query"],
                "estimated_fuel": 200.0,
                "dependencies": ["t1"]
            },
            {
                "id": "t3",
                "description": "Write report",
                "required_capabilities": ["fs.write"],
                "estimated_fuel": 50.0,
                "dependencies": ["t2"]
            }
        ]))
        .unwrap()
    }

    fn three_parallel_decomposition() -> String {
        serde_json::to_string(&serde_json::json!([
            {
                "id": "t1",
                "description": "Task A",
                "required_capabilities": ["web.search"],
                "estimated_fuel": 100.0,
                "dependencies": []
            },
            {
                "id": "t2",
                "description": "Task B",
                "required_capabilities": ["llm.query"],
                "estimated_fuel": 100.0,
                "dependencies": []
            },
            {
                "id": "t3",
                "description": "Task C",
                "required_capabilities": ["fs.write"],
                "estimated_fuel": 100.0,
                "dependencies": []
            }
        ]))
        .unwrap()
    }

    fn make_agents() -> Vec<AgentInfo> {
        vec![
            AgentInfo {
                id: "agent-search".into(),
                capabilities: vec!["web.search".into(), "web.fetch".into()],
                available_fuel: 500.0,
            },
            AgentInfo {
                id: "agent-llm".into(),
                capabilities: vec!["llm.query".into()],
                available_fuel: 500.0,
            },
            AgentInfo {
                id: "agent-writer".into(),
                capabilities: vec!["fs.write".into(), "fs.read".into()],
                available_fuel: 500.0,
            },
        ]
    }

    fn make_coordinator(
        decompose_resp: &str,
        merge_resp: &str,
    ) -> (HivemindCoordinator, Arc<CollectingHivemindEmitter>) {
        let emitter = Arc::new(CollectingHivemindEmitter::new());
        let audit = Arc::new(Mutex::new(AuditTrail::new()));
        let llm = Box::new(MockHivemindLlm::new(decompose_resp, merge_resp));
        let coord = HivemindCoordinator::new(llm, emitter.clone(), audit);
        (coord, emitter)
    }

    // ── Test 1: Decompose produces sub-tasks with dependencies ──

    #[test]
    fn test_decompose_produces_subtasks_with_dependencies() {
        let resp = three_task_decomposition();
        let tasks = parse_decomposition(&resp).unwrap();
        assert_eq!(tasks.len(), 3);
        assert!(tasks[0].dependencies.is_empty());
        assert_eq!(tasks[1].dependencies, vec!["t1"]);
        assert_eq!(tasks[2].dependencies, vec!["t2"]);
    }

    // ── Test 2: Capability matching works correctly ──

    #[test]
    fn test_assign_capability_matching() {
        let resp = three_task_decomposition();
        let tasks = parse_decomposition(&resp).unwrap();
        let agents = make_agents();
        let (assignments, unassignable) = assign_by_capability(&tasks, &agents);

        assert_eq!(assignments.get("t1").unwrap(), "agent-search");
        assert_eq!(assignments.get("t2").unwrap(), "agent-llm");
        assert_eq!(assignments.get("t3").unwrap(), "agent-writer");
        assert!(unassignable.is_empty());
    }

    // ── Test 3: Missing capability is reported ──

    #[test]
    fn test_assign_missing_capability_reported() {
        let decomp = serde_json::to_string(&serde_json::json!([
            {
                "id": "t1",
                "description": "Hack the mainframe",
                "required_capabilities": ["quantum.compute"],
                "estimated_fuel": 100.0,
                "dependencies": []
            }
        ]))
        .unwrap();

        let tasks = parse_decomposition(&decomp).unwrap();
        let agents = make_agents();
        let (assignments, unassignable) = assign_by_capability(&tasks, &agents);

        assert!(assignments.is_empty());
        assert_eq!(unassignable.len(), 1);
        assert_eq!(unassignable[0].0, "t1");
        assert!(unassignable[0].1.contains(&"quantum.compute".to_string()));
    }

    // ── Test 4: Topological sort produces correct execution waves ──

    #[test]
    fn test_dag_topological_sort_waves() {
        let resp = three_task_decomposition();
        let tasks = parse_decomposition(&resp).unwrap();
        let waves = topological_waves(&tasks).unwrap();

        assert_eq!(waves.len(), 3);
        assert_eq!(waves[0], vec!["t1"]);
        assert_eq!(waves[1], vec!["t2"]);
        assert_eq!(waves[2], vec!["t3"]);
    }

    // ── Test 5: Circular dependencies detected and rejected ──

    #[test]
    fn test_dag_circular_dependency_detected() {
        let tasks = vec![
            SubTask {
                id: "a".into(),
                description: "task a".into(),
                required_capabilities: vec![],
                dependencies: vec!["b".into()],
                estimated_fuel: 10.0,
                status: SubTaskStatus::Pending,
            },
            SubTask {
                id: "b".into(),
                description: "task b".into(),
                required_capabilities: vec![],
                dependencies: vec!["a".into()],
                estimated_fuel: 10.0,
                status: SubTaskStatus::Pending,
            },
        ];

        let result = topological_waves(&tasks);
        assert_eq!(result, Err(CyclicDependencyError));
    }

    // ── Test 6: Execute 3 agents on 3 sub-tasks, all complete ──

    #[test]
    fn test_execute_three_agents_three_subtasks() {
        let decomp = three_task_decomposition();
        let (coord, _emitter) = make_coordinator(&decomp, "Merged result");
        let agents = make_agents();

        let session = coord
            .execute_hivemind_goal("Research and write report", agents)
            .unwrap();

        assert_eq!(session.status, HivemindStatus::Completed);
        assert_eq!(session.sub_tasks.len(), 3);
        assert!(session
            .sub_tasks
            .iter()
            .all(|t| t.status == SubTaskStatus::Completed));
        assert!(session.results.contains_key("__merged__"));
    }

    // ── Test 7: Parallel sub-tasks run in same wave ──

    #[test]
    fn test_execute_parallel_subtasks_same_wave() {
        let decomp = three_parallel_decomposition();
        let tasks = parse_decomposition(&decomp).unwrap();
        let waves = topological_waves(&tasks).unwrap();

        // All three should be in wave 0 (no dependencies)
        assert_eq!(waves.len(), 1);
        assert_eq!(waves[0].len(), 3);
    }

    // ── Test 8: Dependent sub-task waits for prerequisite ──

    #[test]
    fn test_execute_dependent_subtask_waits() {
        let decomp = three_task_decomposition();
        let (coord, emitter) = make_coordinator(&decomp, "Merged result");
        let agents = make_agents();

        let session = coord
            .execute_hivemind_goal("Test dependency ordering", agents)
            .unwrap();

        let events = emitter.events.lock().unwrap();
        let wave_events: Vec<_> = events
            .iter()
            .filter_map(|e| match e {
                HivemindEvent::WaveStarted {
                    wave_number,
                    parallel_tasks,
                    ..
                } => Some((*wave_number, parallel_tasks.clone())),
                _ => None,
            })
            .collect();

        // Should have 3 waves with 1 task each
        assert_eq!(wave_events.len(), 3);
        assert!(wave_events[0].1.contains(&"t1".to_string()));
        assert!(wave_events[1].1.contains(&"t2".to_string()));
        assert!(wave_events[2].1.contains(&"t3".to_string()));

        assert_eq!(session.status, HivemindStatus::Completed);
    }

    // ── Test 9: Agent A's result readable by agent B (blackboard concept) ──

    #[test]
    fn test_blackboard_result_sharing() {
        let decomp = three_task_decomposition();
        let (coord, _) = make_coordinator(&decomp, "Merged");
        let agents = make_agents();

        let session = coord
            .execute_hivemind_goal("Share results", agents)
            .unwrap();

        // t1's result should be in session.results before t2 runs
        assert!(session.results.contains_key("t1"));
        assert!(session.results.contains_key("t2"));
        assert!(session.results.contains_key("t3"));
    }

    // ── Test 10: Reassignment when first agent fails ──

    #[test]
    fn test_reassignment_on_failure() {
        let decomp = serde_json::to_string(&serde_json::json!([
            {
                "id": "t1",
                "description": "Do work",
                "required_capabilities": ["web.search"],
                "estimated_fuel": 50.0,
                "dependencies": []
            }
        ]))
        .unwrap();

        let (coord, _) = make_coordinator(&decomp, "Merged");
        let agents = vec![
            AgentInfo {
                id: "agent-a".into(),
                capabilities: vec!["web.search".into()],
                available_fuel: 500.0,
            },
            AgentInfo {
                id: "agent-b".into(),
                capabilities: vec!["web.search".into()],
                available_fuel: 500.0,
            },
        ];

        let mut attempt = 0;
        let session = coord
            .execute_with_executor("Reassign test", agents, |_task_id, agent_id, _desc| {
                attempt += 1;
                if attempt == 1 {
                    // First agent fails
                    assert_eq!(agent_id, "agent-a");
                    Err("agent-a crashed".into())
                } else {
                    // Reassigned to agent-b
                    assert_eq!(agent_id, "agent-b");
                    Ok("completed by agent-b".into())
                }
            })
            .unwrap();

        assert_eq!(session.status, HivemindStatus::Completed);
        assert!(session.results.contains_key("t1"));
    }

    // ── Test 11: Abort when >50% sub-tasks fail ──

    #[test]
    fn test_abort_when_majority_fail() {
        let decomp = serde_json::to_string(&serde_json::json!([
            {
                "id": "t1",
                "description": "Fail A",
                "required_capabilities": ["cap.a"],
                "estimated_fuel": 50.0,
                "dependencies": []
            },
            {
                "id": "t2",
                "description": "Fail B",
                "required_capabilities": ["cap.b"],
                "estimated_fuel": 50.0,
                "dependencies": []
            },
            {
                "id": "t3",
                "description": "Succeed C",
                "required_capabilities": ["cap.c"],
                "estimated_fuel": 50.0,
                "dependencies": []
            }
        ]))
        .unwrap();

        let (coord, _) = make_coordinator(&decomp, "Merged");
        // Only provide agent for cap.c — t1 and t2 will be unassignable (>50% fail)
        let agents = vec![AgentInfo {
            id: "agent-c".into(),
            capabilities: vec!["cap.c".into()],
            available_fuel: 500.0,
        }];

        let session = coord
            .execute_hivemind_goal("Majority fail test", agents)
            .unwrap();

        assert_eq!(session.status, HivemindStatus::Failed);
    }

    // ── Test 12: Merge combines sub-task outputs ──

    #[test]
    fn test_merge_combines_results() {
        let decomp = three_parallel_decomposition();
        let (coord, _) = make_coordinator(&decomp, "All tasks merged successfully");
        let agents = make_agents();

        let session = coord.execute_hivemind_goal("Merge test", agents).unwrap();

        assert_eq!(
            session.results.get("__merged__").unwrap(),
            "All tasks merged successfully"
        );
    }

    // ── Test 13: Session saved and retrievable ──

    #[test]
    fn test_session_persistence() {
        let decomp = three_task_decomposition();
        let (coord, _) = make_coordinator(&decomp, "Done");
        let agents = make_agents();

        let session = coord.execute_hivemind_goal("Persist test", agents).unwrap();
        let session_id = session.id.clone();

        let retrieved = coord.get_session(&session_id).unwrap();
        assert_eq!(retrieved.id, session_id);
        assert_eq!(retrieved.status, HivemindStatus::Completed);
    }

    // ── Test 14: Total fuel is sum of all agent fuel ──

    #[test]
    fn test_fuel_accounting() {
        let decomp = three_task_decomposition();
        let (coord, _) = make_coordinator(&decomp, "Done");
        let agents = make_agents();

        let session = coord.execute_hivemind_goal("Fuel test", agents).unwrap();

        // Sub-task fuel: 100 + 200 + 50 = 350
        // LLM costs: 10 (decompose) + 10 (merge) = 20
        // Total: 370
        assert!((session.total_fuel_consumed - 370.0).abs() < f64::EPSILON);
    }

    // ── Test 15: Events emitted in correct order ──

    #[test]
    fn test_events_emitted_in_order() {
        let decomp = three_task_decomposition();
        let (coord, emitter) = make_coordinator(&decomp, "Done");
        let agents = make_agents();

        let _session = coord.execute_hivemind_goal("Events test", agents).unwrap();

        let events = emitter.events.lock().unwrap();

        // Expect: 3 SubTaskAssigned, then for each wave: WaveStarted + SubTaskCompleted,
        // then SessionCompleted
        let mut saw_assigned = 0;
        let mut saw_wave_started = 0;
        let mut saw_subtask_completed = 0;
        let mut saw_session_completed = 0;

        for event in events.iter() {
            match event {
                HivemindEvent::SubTaskAssigned { .. } => saw_assigned += 1,
                HivemindEvent::WaveStarted { .. } => saw_wave_started += 1,
                HivemindEvent::SubTaskCompleted { .. } => saw_subtask_completed += 1,
                HivemindEvent::SessionCompleted { .. } => saw_session_completed += 1,
            }
        }

        assert_eq!(saw_assigned, 3);
        assert_eq!(saw_wave_started, 3); // 3 sequential waves
        assert_eq!(saw_subtask_completed, 3);
        assert_eq!(saw_session_completed, 1);
    }

    // ── Test 16: Cancel running session ──

    #[test]
    fn test_cancel_session() {
        let decomp = three_task_decomposition();
        let (coord, _) = make_coordinator(&decomp, "Done");

        // Manually insert a session in executing state
        let mut session = HivemindSession::new("cancel me".to_string());
        session.status = HivemindStatus::Executing;
        let session_id = session.id.clone();
        coord.store_session(&session);

        coord.cancel_session(&session_id).unwrap();

        let cancelled = coord.get_session(&session_id).unwrap();
        assert_eq!(cancelled.status, HivemindStatus::Cancelled);
    }

    // ── Test 17: Cancel completed session fails ──

    #[test]
    fn test_cancel_completed_session_fails() {
        let decomp = three_task_decomposition();
        let (coord, _) = make_coordinator(&decomp, "Done");

        let mut session = HivemindSession::new("already done".to_string());
        session.status = HivemindStatus::Completed;
        let session_id = session.id.clone();
        coord.store_session(&session);

        let result = coord.cancel_session(&session_id);
        assert!(result.is_err());
    }

    // ── Test 18: Parse decomposition with markdown fences ──

    #[test]
    fn test_parse_decomposition_with_markdown() {
        let response = r#"Here are the sub-tasks:

```json
[
  {"id": "s1", "description": "Step 1", "required_capabilities": ["fs.read"], "estimated_fuel": 50, "dependencies": []}
]
```
"#;
        let tasks = parse_decomposition(response).unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].id, "s1");
    }

    // ── Test 19: Empty decomposition returns error ──

    #[test]
    fn test_empty_decomposition_error() {
        let result = parse_decomposition("[]");
        assert!(result.is_err());
    }

    // ── Test 20: LLM failure propagates ──

    #[test]
    fn test_llm_failure_propagates() {
        let emitter = Arc::new(NoOpHivemindEmitter);
        let audit = Arc::new(Mutex::new(AuditTrail::new()));
        let llm = Box::new(FailingLlm);
        let coord = HivemindCoordinator::new(llm, emitter, audit);

        let result = coord.execute_hivemind_goal("Fail", vec![]);
        assert!(result.is_err());
    }

    // ── Test 21: List sessions returns all sessions ──

    #[test]
    fn test_list_sessions() {
        let decomp = three_parallel_decomposition();
        let (coord, _) = make_coordinator(&decomp, "Done");
        let agents = make_agents();

        let s1 = coord
            .execute_hivemind_goal("Goal 1", agents.clone())
            .unwrap();
        let s2 = coord.execute_hivemind_goal("Goal 2", agents).unwrap();

        let sessions = coord.list_sessions();
        assert_eq!(sessions.len(), 2);

        let ids: HashSet<_> = sessions.iter().map(|s| s.id.clone()).collect();
        assert!(ids.contains(&s1.id));
        assert!(ids.contains(&s2.id));
    }

    // ── Test 22: Session not found returns None ──

    #[test]
    fn test_get_session_not_found() {
        let decomp = three_task_decomposition();
        let (coord, _) = make_coordinator(&decomp, "Done");
        assert!(coord.get_session("nonexistent").is_none());
    }

    // ── Test 23: Sub-tasks have correct fields from JSON ──

    #[test]
    fn test_subtask_fields_parsed_correctly() {
        let resp = three_task_decomposition();
        let tasks = parse_decomposition(&resp).unwrap();

        assert_eq!(tasks[0].id, "t1");
        assert_eq!(tasks[0].description, "Research the topic");
        assert_eq!(tasks[0].required_capabilities, vec!["web.search"]);
        assert!((tasks[0].estimated_fuel - 100.0).abs() < f64::EPSILON);
        assert_eq!(tasks[0].status, SubTaskStatus::Pending);
    }

    // ── Test 24: Parallel tasks produce single wave in DAG ──

    #[test]
    fn test_dag_parallel_single_wave() {
        let tasks = vec![
            SubTask {
                id: "a".into(),
                description: "A".into(),
                required_capabilities: vec![],
                dependencies: vec![],
                estimated_fuel: 10.0,
                status: SubTaskStatus::Pending,
            },
            SubTask {
                id: "b".into(),
                description: "B".into(),
                required_capabilities: vec![],
                dependencies: vec![],
                estimated_fuel: 10.0,
                status: SubTaskStatus::Pending,
            },
        ];

        let waves = topological_waves(&tasks).unwrap();
        assert_eq!(waves.len(), 1);
        assert_eq!(waves[0].len(), 2);
    }

    // ── Test 25: Diamond dependency DAG ──

    #[test]
    fn test_dag_diamond_dependency() {
        // a -> b, a -> c, b -> d, c -> d
        let tasks = vec![
            SubTask {
                id: "a".into(),
                description: "A".into(),
                required_capabilities: vec![],
                dependencies: vec![],
                estimated_fuel: 10.0,
                status: SubTaskStatus::Pending,
            },
            SubTask {
                id: "b".into(),
                description: "B".into(),
                required_capabilities: vec![],
                dependencies: vec!["a".into()],
                estimated_fuel: 10.0,
                status: SubTaskStatus::Pending,
            },
            SubTask {
                id: "c".into(),
                description: "C".into(),
                required_capabilities: vec![],
                dependencies: vec!["a".into()],
                estimated_fuel: 10.0,
                status: SubTaskStatus::Pending,
            },
            SubTask {
                id: "d".into(),
                description: "D".into(),
                required_capabilities: vec![],
                dependencies: vec!["b".into(), "c".into()],
                estimated_fuel: 10.0,
                status: SubTaskStatus::Pending,
            },
        ];

        let waves = topological_waves(&tasks).unwrap();
        assert_eq!(waves.len(), 3);
        assert_eq!(waves[0], vec!["a"]);
        assert_eq!(waves[1].len(), 2); // b and c in parallel
        assert!(waves[1].contains(&"b".to_string()));
        assert!(waves[1].contains(&"c".to_string()));
        assert_eq!(waves[2], vec!["d"]);
    }
}
