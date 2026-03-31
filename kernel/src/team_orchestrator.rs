//! Team Orchestrator — Director-driven multi-agent collaboration.
//!
//! A Director agent leads a team of Workers (Researcher, Writer, Publisher).
//! Uses HivemindCoordinator for goal decomposition and DAG execution,
//! with fuel reallocation and conflict resolution.

use crate::audit::{AuditTrail, EventType};
use crate::cognitive::hivemind::{
    AgentInfo, CollectingHivemindEmitter, HivemindCoordinator, HivemindEvent, HivemindEventEmitter,
    HivemindLlm, HivemindSession, HivemindStatus,
};
use crate::cognitive::loop_runtime::LlmQueryHandler;
use crate::errors::AgentError;
use crate::supervisor::Supervisor;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// A team member definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamMember {
    pub agent_id: String,
    pub name: String,
    pub role: String,
    pub capabilities: Vec<String>,
    pub fuel_budget: u64,
}

/// Team configuration — defines the director and workers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamConfig {
    pub director_id: String,
    pub director_name: String,
    pub members: Vec<TeamMember>,
}

/// Result of a team workflow execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamWorkflowResult {
    pub success: bool,
    pub session_id: String,
    pub goal: String,
    pub task_results: Vec<TaskAssignmentResult>,
    pub total_fuel: f64,
    pub duration_secs: f64,
    pub summary: String,
    pub error: Option<String>,
}

/// Result of a single task assignment within the team.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskAssignmentResult {
    pub task_id: String,
    pub agent_id: String,
    pub agent_name: String,
    pub description: String,
    pub success: bool,
    pub output: String,
    pub fuel_cost: f64,
}

/// The team orchestrator — manages Director → Worker workflows.
pub struct TeamOrchestrator {
    supervisor: Arc<Mutex<Supervisor>>,
    audit: Arc<Mutex<AuditTrail>>,
    llm_handler: Arc<dyn LlmQueryHandler>,
    events: Arc<CollectingHivemindEmitter>,
}

impl TeamOrchestrator {
    pub fn new(
        supervisor: Arc<Mutex<Supervisor>>,
        audit: Arc<Mutex<AuditTrail>>,
        llm_handler: Arc<dyn LlmQueryHandler>,
    ) -> Self {
        Self {
            supervisor,
            audit,
            llm_handler,
            events: Arc::new(CollectingHivemindEmitter::new()),
        }
    }

    /// Execute a team workflow: Director decomposes goal, assigns to workers, collects results.
    pub fn execute_team_workflow(
        &self,
        config: &TeamConfig,
        goal: &str,
    ) -> Result<TeamWorkflowResult, AgentError> {
        let start = std::time::Instant::now();

        // Log the start
        {
            let mut audit = self.audit.lock().unwrap_or_else(|p| p.into_inner());
            // Best-effort: workflow start audit is informational; workflow execution proceeds regardless
            let _ = audit.append_event(
                uuid::Uuid::parse_str(&config.director_id).unwrap_or_default(),
                EventType::StateChange,
                json!({
                    "event": "team.workflow_start",
                    "director": config.director_name,
                    "goal": goal,
                    "team_size": config.members.len(),
                    "members": config.members.iter().map(|m| &m.name).collect::<Vec<_>>(),
                }),
            );
        }

        // Build AgentInfo for each team member
        let agents: Vec<AgentInfo> = config
            .members
            .iter()
            .map(|m| AgentInfo {
                id: m.agent_id.clone(),
                capabilities: m.capabilities.clone(),
                available_fuel: m.fuel_budget as f64,
            })
            .collect();

        // Create a name lookup for results
        let name_map: HashMap<String, String> = config
            .members
            .iter()
            .map(|m| (m.agent_id.clone(), m.name.clone()))
            .collect();

        // Create the HivemindCoordinator with our LLM
        let llm_bridge = TeamLlmBridge {
            handler: self.llm_handler.clone(),
        };
        let coordinator = HivemindCoordinator::new(
            Box::new(llm_bridge),
            self.events.clone() as Arc<dyn HivemindEventEmitter>,
            self.audit.clone(),
        );

        // Execute with a custom executor that uses the LLM for each sub-task
        let llm = self.llm_handler.clone();
        let session_results: Arc<Mutex<Vec<TaskAssignmentResult>>> =
            Arc::new(Mutex::new(Vec::new()));
        let results_ref = session_results.clone();

        let session = coordinator.execute_with_executor(
            goal,
            agents,
            |task_id: &str, agent_id: &str, task_desc: &str| {
                let agent_name = name_map
                    .get(agent_id)
                    .cloned()
                    .unwrap_or_else(|| agent_id.to_string());

                eprintln!(
                    "[team] {} ({}) executing: {}",
                    agent_name, agent_id, task_desc
                );

                // Use the LLM to execute the task in the agent's persona
                let prompt = format!(
                    "You are {agent_name}, a team member agent. \
                     Execute this task assigned by the Director:\n\n\
                     Task: {task_desc}\n\n\
                     Provide your result concisely. If this is a research task, provide key findings. \
                     If this is a writing task, write the content. \
                     If this is a publishing task, confirm the actions taken."
                );

                let result = llm.query(&prompt).unwrap_or_else(|e| {
                    format!("Task execution failed: {e}")
                });

                let fuel_cost = estimate_task_fuel(task_desc);

                results_ref
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .push(TaskAssignmentResult {
                        task_id: task_id.to_string(),
                        agent_id: agent_id.to_string(),
                        agent_name: agent_name.clone(),
                        description: task_desc.to_string(),
                        success: true,
                        output: result.clone(),
                        fuel_cost,
                    });

                Ok(result)
            },
        )?;

        let duration = start.elapsed().as_secs_f64();
        let task_results = session_results
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();

        let success = session.status == HivemindStatus::Completed;

        // Generate summary using LLM
        let summary = self.generate_team_summary(config, goal, &session, &task_results);

        // Log completion
        {
            let mut audit = self.audit.lock().unwrap_or_else(|p| p.into_inner());
            // Best-effort: workflow completion audit is informational; result is already computed
            let _ = audit.append_event(
                uuid::Uuid::parse_str(&config.director_id).unwrap_or_default(),
                EventType::StateChange,
                json!({
                    "event": "team.workflow_complete",
                    "director": config.director_name,
                    "goal": goal,
                    "success": success,
                    "tasks_completed": task_results.iter().filter(|t| t.success).count(),
                    "tasks_total": task_results.len(),
                    "total_fuel": session.total_fuel_consumed,
                    "duration_secs": duration,
                }),
            );
        }

        Ok(TeamWorkflowResult {
            success,
            session_id: session.id,
            goal: goal.to_string(),
            task_results,
            total_fuel: session.total_fuel_consumed,
            duration_secs: duration,
            summary,
            error: if success {
                None
            } else {
                Some("one or more sub-tasks failed".into())
            },
        })
    }

    /// Transfer fuel from one agent to another (Director redistributes resources).
    pub fn transfer_fuel(
        &self,
        from_agent: &str,
        to_agent: &str,
        amount: u64,
    ) -> Result<(), AgentError> {
        let from_id = uuid::Uuid::parse_str(from_agent)
            .map_err(|e| AgentError::SupervisorError(format!("invalid from_agent: {e}")))?;
        let to_id = uuid::Uuid::parse_str(to_agent)
            .map_err(|e| AgentError::SupervisorError(format!("invalid to_agent: {e}")))?;

        let mut sup = self.supervisor.lock().unwrap_or_else(|p| p.into_inner());

        // Check source has enough fuel
        let from_fuel = sup
            .get_agent(from_id)
            .map(|h| h.remaining_fuel)
            .ok_or_else(|| {
                AgentError::SupervisorError(format!("source agent '{from_agent}' not found"))
            })?;

        if from_fuel < amount {
            return Err(AgentError::SupervisorError(format!(
                "source agent has {from_fuel} fuel, need {amount}"
            )));
        }

        // Verify target exists
        sup.get_agent(to_id).ok_or_else(|| {
            AgentError::SupervisorError(format!("target agent '{to_agent}' not found"))
        })?;

        // Transfer: deduct from source, add to target
        if let Some(from) = sup.get_agent_mut(from_id) {
            from.remaining_fuel -= amount;
        }
        if let Some(to) = sup.get_agent_mut(to_id) {
            to.remaining_fuel += amount;
        }

        // Audit the transfer
        drop(sup);
        let mut audit = self.audit.lock().unwrap_or_else(|p| p.into_inner());
        // Best-effort: fuel transfer audit is informational; balances were already updated in supervisor
        let _ = audit.append_event(
            from_id,
            EventType::StateChange,
            json!({
                "event": "team.fuel_transfer",
                "from_agent": from_agent,
                "to_agent": to_agent,
                "amount": amount,
                "timestamp": Utc::now().to_rfc3339(),
            }),
        );

        eprintln!("[team] fuel transfer: {amount} units from {from_agent} to {to_agent}");

        Ok(())
    }

    /// Read all team members' memories (Director privilege).
    pub fn read_team_memories(
        &self,
        memory_mgr: &crate::cognitive::memory_manager::AgentMemoryManager,
        team: &TeamConfig,
    ) -> HashMap<String, Vec<crate::cognitive::memory_manager::MemoryEntry>> {
        let mut all_memories = HashMap::new();
        for member in &team.members {
            if let Ok(memories) = memory_mgr.recall_relevant(&member.agent_id, "", 10) {
                all_memories.insert(member.name.clone(), memories);
            }
        }
        all_memories
    }

    /// Get collected hivemind events for dashboard display.
    pub fn get_events(&self) -> Vec<HivemindEvent> {
        self.events
            .events
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    fn generate_team_summary(
        &self,
        config: &TeamConfig,
        goal: &str,
        session: &HivemindSession,
        results: &[TaskAssignmentResult],
    ) -> String {
        let completed = results.iter().filter(|r| r.success).count();
        let total = results.len();
        let member_summaries: Vec<String> = results
            .iter()
            .map(|r| {
                let preview = if r.output.len() > 100 {
                    format!("{}...", &r.output[..100])
                } else {
                    r.output.clone()
                };
                format!("- {} ({}): {}", r.agent_name, r.task_id, preview)
            })
            .collect();

        format!(
            "Team Workflow Summary\n\
             Director: {}\n\
             Goal: {}\n\
             Status: {}\n\
             Tasks: {}/{} completed\n\
             Fuel consumed: {:.1}\n\
             \nTask Results:\n{}",
            config.director_name,
            goal,
            if session.status == HivemindStatus::Completed {
                "SUCCESS"
            } else {
                "PARTIAL"
            },
            completed,
            total,
            session.total_fuel_consumed,
            member_summaries.join("\n"),
        )
    }
}

/// Bridge from LlmQueryHandler to HivemindLlm.
struct TeamLlmBridge {
    handler: Arc<dyn LlmQueryHandler>,
}

impl HivemindLlm for TeamLlmBridge {
    fn decompose(&self, prompt: &str) -> Result<String, AgentError> {
        self.handler
            .query(prompt)
            .map_err(|e| AgentError::SupervisorError(format!("LLM decompose failed: {e}")))
    }

    fn merge(&self, prompt: &str) -> Result<String, AgentError> {
        self.handler
            .query(prompt)
            .map_err(|e| AgentError::SupervisorError(format!("LLM merge failed: {e}")))
    }
}

/// Estimate fuel cost based on task description keywords.
fn estimate_task_fuel(description: &str) -> f64 {
    let lower = description.to_lowercase();
    if lower.contains("research") || lower.contains("search") || lower.contains("fetch") {
        25.0 // Web searches + fetches
    } else if lower.contains("write") || lower.contains("article") || lower.contains("content") {
        15.0 // LLM generation
    } else if lower.contains("publish") || lower.contains("save") || lower.contains("commit") {
        8.0 // File I/O + git
    } else {
        10.0 // Default
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::AuditTrail;
    use crate::cognitive::loop_runtime::LlmQueryHandler;
    use crate::manifest::AgentManifest;
    use crate::supervisor::Supervisor;

    /// Mock LLM that handles decomposition and task execution.
    struct TeamMockLlm;

    impl LlmQueryHandler for TeamMockLlm {
        fn query(&self, prompt: &str) -> Result<String, String> {
            let lower = prompt.to_lowercase();

            // Hivemind decomposition prompt
            if lower.contains("task decomposition") || lower.contains("break this goal") {
                return Ok(r#"[
                    {
                        "id": "research",
                        "description": "Research trending tech topics on HN and Reddit",
                        "required_capabilities": ["web.search", "web.read"],
                        "dependencies": [],
                        "estimated_fuel": 25.0
                    },
                    {
                        "id": "write",
                        "description": "Write a 1500-word article on the best trending topic",
                        "required_capabilities": ["llm.query", "fs.write"],
                        "dependencies": ["research"],
                        "estimated_fuel": 15.0
                    },
                    {
                        "id": "publish",
                        "description": "Save article as HTML and Markdown, commit to git",
                        "required_capabilities": ["fs.write", "process.exec"],
                        "dependencies": ["write"],
                        "estimated_fuel": 10.0
                    }
                ]"#
                .to_string());
            }

            // Merge results prompt
            if lower.contains("merge") || lower.contains("synthesize") || lower.contains("combine")
            {
                return Ok("Team workflow completed successfully. Research identified 5 trending topics. Writer produced a 1500-word article on AI coding assistants. Publisher saved HTML and Markdown files.".to_string());
            }

            // Researcher task
            if lower.contains("nexus-researcher") || lower.contains("research") {
                return Ok("RESEARCH FINDINGS:\n\
                     1. AI Coding Assistants (trending #1 on HN)\n\
                     2. Rust in Enterprise (Reddit r/programming)\n\
                     3. WebAssembly Edge Computing (growing fast)\n\
                     4. Open Source LLMs (Claude, Llama, Mistral)\n\
                     5. Developer Productivity Tools (record VC investment)\n\
                     \nRecommendation: Topic #1 has highest monetization potential."
                    .to_string());
            }

            // Writer task
            if lower.contains("nexus-writer") || lower.contains("write") {
                return Ok("# AI Coding Assistants Are Reshaping Development\n\n\
                     The software industry is undergoing a transformation...\n\n\
                     ## The Current Landscape\n\n\
                     78% of developers now use AI tools daily.\n\n\
                     ## Key Takeaways\n\n\
                     - AI assistants boost productivity by 55%\n\
                     - Enterprise adoption at 92% of Fortune 500\n\n\
                     ## Conclusion\n\n\
                     Embrace AI tools or fall behind."
                    .to_string());
            }

            // Publisher task
            if lower.contains("nexus-publisher")
                || lower.contains("publish")
                || lower.contains("save")
            {
                return Ok("Published successfully:\n\
                     - HTML: articles/2026-03-24/ai-coding-assistants.html\n\
                     - Markdown: articles/2026-03-24/ai-coding-assistants.md\n\
                     - Metadata: articles/2026-03-24/ai-coding-assistants.meta.json\n\
                     - Git commit: abc123"
                    .to_string());
            }

            Ok("Task executed successfully.".to_string())
        }
    }

    fn make_team_config(
        director_id: &str,
        researcher_id: &str,
        writer_id: &str,
        publisher_id: &str,
    ) -> TeamConfig {
        TeamConfig {
            director_id: director_id.to_string(),
            director_name: "nexus-director".to_string(),
            members: vec![
                TeamMember {
                    agent_id: researcher_id.to_string(),
                    name: "nexus-researcher".to_string(),
                    role: "researcher".to_string(),
                    capabilities: vec![
                        "web.search".into(),
                        "web.read".into(),
                        "llm.query".into(),
                        "fs.write".into(),
                    ],
                    fuel_budget: 10000,
                },
                TeamMember {
                    agent_id: writer_id.to_string(),
                    name: "nexus-writer".to_string(),
                    role: "writer".to_string(),
                    capabilities: vec!["llm.query".into(), "fs.read".into(), "fs.write".into()],
                    fuel_budget: 10000,
                },
                TeamMember {
                    agent_id: publisher_id.to_string(),
                    name: "nexus-publisher".to_string(),
                    role: "publisher".to_string(),
                    capabilities: vec![
                        "fs.read".into(),
                        "fs.write".into(),
                        "process.exec".into(),
                        "llm.query".into(),
                    ],
                    fuel_budget: 5000,
                },
            ],
        }
    }

    fn create_agent(sup: &mut Supervisor, name: &str, caps: Vec<&str>, fuel: u64) -> String {
        let manifest = AgentManifest {
            name: name.into(),
            version: "1.0.0".into(),
            capabilities: caps.into_iter().map(String::from).collect(),
            fuel_budget: fuel,
            autonomy_level: Some(3),
            consent_policy_path: None,
            requester_id: None,
            schedule: None,
            default_goal: None,
            llm_model: None,
            fuel_period_id: None,
            monthly_fuel_cap: None,
            allowed_endpoints: None,
            domain_tags: vec![],
            filesystem_permissions: vec![],
        };
        sup.start_agent(manifest).unwrap().to_string()
    }

    /// Full team collaboration test: Director → Researcher → Writer → Publisher.
    #[test]
    fn test_full_team_workflow() {
        let mut sup = Supervisor::new();

        let director_id = create_agent(
            &mut sup,
            "nexus-director",
            vec!["web.search", "llm.query", "self.modify"],
            50000,
        );
        let researcher_id = create_agent(
            &mut sup,
            "nexus-researcher",
            vec!["web.search", "web.read", "llm.query", "fs.write"],
            10000,
        );
        let writer_id = create_agent(
            &mut sup,
            "nexus-writer",
            vec!["llm.query", "fs.read", "fs.write"],
            10000,
        );
        let publisher_id = create_agent(
            &mut sup,
            "nexus-publisher",
            vec!["fs.read", "fs.write", "process.exec", "llm.query"],
            5000,
        );

        let supervisor = Arc::new(Mutex::new(sup));
        let audit = Arc::new(Mutex::new(AuditTrail::new()));
        let llm: Arc<dyn LlmQueryHandler> = Arc::new(TeamMockLlm);

        let orchestrator = TeamOrchestrator::new(supervisor.clone(), audit.clone(), llm);

        let config = make_team_config(&director_id, &researcher_id, &writer_id, &publisher_id);

        let result = orchestrator
            .execute_team_workflow(
                &config,
                "Research trending topics, write an article on the best one, and publish it",
            )
            .unwrap();

        // Verify success
        assert!(result.success, "team workflow should succeed");
        assert!(!result.session_id.is_empty());

        // Verify all 3 sub-tasks completed
        assert_eq!(
            result.task_results.len(),
            3,
            "expected 3 task results, got {}",
            result.task_results.len()
        );
        for tr in &result.task_results {
            assert!(tr.success, "task {} should succeed", tr.task_id);
            assert!(
                !tr.output.is_empty(),
                "task {} should have output",
                tr.task_id
            );
        }

        // Verify fuel was consumed
        assert!(result.total_fuel > 0.0, "should have consumed fuel");

        // Verify summary was generated
        assert!(
            result.summary.contains("Team Workflow Summary"),
            "summary should be present"
        );

        // Verify audit trail has team events
        let audit_guard = audit.lock().unwrap();
        let team_events: Vec<_> = audit_guard
            .events()
            .iter()
            .filter(|e| {
                e.payload
                    .get("event")
                    .and_then(|v| v.as_str())
                    .map(|s| s.starts_with("team."))
                    .unwrap_or(false)
            })
            .collect();
        assert!(
            team_events.len() >= 2,
            "expected team.workflow_start and team.workflow_complete events"
        );

        // Verify hivemind events were emitted
        let events = orchestrator.get_events();
        assert!(!events.is_empty(), "should have hivemind events");

        eprintln!(
            "Team workflow: {} tasks, {:.1} fuel, {:.1}s",
            result.task_results.len(),
            result.total_fuel,
            result.duration_secs
        );
    }

    /// Test fuel transfer between agents.
    #[test]
    fn test_fuel_transfer() {
        let mut sup = Supervisor::new();
        let agent_a = create_agent(&mut sup, "agent-a", vec!["llm.query"], 1000);
        let agent_b = create_agent(&mut sup, "agent-b", vec!["llm.query"], 500);

        let supervisor = Arc::new(Mutex::new(sup));
        let audit = Arc::new(Mutex::new(AuditTrail::new()));
        let llm: Arc<dyn LlmQueryHandler> = Arc::new(TeamMockLlm);
        let orchestrator = TeamOrchestrator::new(supervisor.clone(), audit, llm);

        // Read starting fuel (start_agent may deduct a small amount)
        let (fuel_a_before, fuel_b_before) = {
            let sup = supervisor.lock().unwrap();
            let a_id = uuid::Uuid::parse_str(&agent_a).unwrap();
            let b_id = uuid::Uuid::parse_str(&agent_b).unwrap();
            (
                sup.get_agent(a_id).unwrap().remaining_fuel,
                sup.get_agent(b_id).unwrap().remaining_fuel,
            )
        };

        // Transfer 200 fuel from A to B
        orchestrator.transfer_fuel(&agent_a, &agent_b, 200).unwrap();

        let sup = supervisor.lock().unwrap();
        let a_id = uuid::Uuid::parse_str(&agent_a).unwrap();
        let b_id = uuid::Uuid::parse_str(&agent_b).unwrap();
        assert_eq!(
            sup.get_agent(a_id).unwrap().remaining_fuel,
            fuel_a_before - 200
        );
        assert_eq!(
            sup.get_agent(b_id).unwrap().remaining_fuel,
            fuel_b_before + 200
        );
    }

    /// Test fuel transfer fails if source lacks funds.
    #[test]
    fn test_fuel_transfer_insufficient() {
        let mut sup = Supervisor::new();
        let agent_a = create_agent(&mut sup, "agent-a", vec!["llm.query"], 100);
        let agent_b = create_agent(&mut sup, "agent-b", vec!["llm.query"], 500);

        let supervisor = Arc::new(Mutex::new(sup));
        let audit = Arc::new(Mutex::new(AuditTrail::new()));
        let llm: Arc<dyn LlmQueryHandler> = Arc::new(TeamMockLlm);
        let orchestrator = TeamOrchestrator::new(supervisor, audit, llm);

        let result = orchestrator.transfer_fuel(&agent_a, &agent_b, 500);
        assert!(result.is_err(), "should fail with insufficient fuel");
    }

    /// Test task fuel estimation.
    #[test]
    fn test_estimate_task_fuel() {
        assert_eq!(estimate_task_fuel("Research trending topics"), 25.0);
        assert_eq!(estimate_task_fuel("Write a 1500-word article"), 15.0);
        assert_eq!(estimate_task_fuel("Publish and save to git"), 8.0);
        assert_eq!(estimate_task_fuel("unknown task"), 10.0);
    }
}
