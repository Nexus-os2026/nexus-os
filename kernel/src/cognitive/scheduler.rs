//! Agent Scheduler — activates agents on cron schedules.
//!
//! Parses cron expressions from agent manifests and spawns background schedule tasks.
//! that sleep until the next tick, then call `CognitiveRuntime::assign_goal()`.

use super::loop_runtime::CognitiveRuntime;
use super::types::AgentGoal;
use crate::audit::{AuditTrail, EventType};
use crate::errors::AgentError;
use chrono::Utc;
use cron::Schedule;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::task::JoinHandle;
use tokio::time;
use uuid::Uuid;

/// Entry tracking a scheduled agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledAgent {
    pub agent_id: String,
    pub cron_expression: String,
    pub default_goal: String,
    /// Unix timestamp of the next scheduled run (0 if unknown).
    pub next_run_epoch: i64,
}

pub trait ScheduledGoalExecutor: Send + Sync {
    fn execute(&self, agent_id: &str, default_goal: &str) -> Result<(), String>;
}

/// Handle for a running schedule loop — holds the cancel flag and join handle.
struct ScheduleHandle {
    handle: JoinHandle<()>,
    cron_expression: String,
    default_goal: String,
    next_run: Arc<AtomicI64>,
}

/// Manages cron-based agent scheduling.
///
/// Holds an `Arc<CognitiveRuntime>` and spawns one background task per
/// scheduled agent. Each task sleeps until the next cron tick and then
/// assigns the agent's default goal via the cognitive runtime.
pub struct AgentScheduler {
    runtime: Arc<CognitiveRuntime>,
    audit: Arc<Mutex<AuditTrail>>,
    handles: Mutex<HashMap<String, ScheduleHandle>>,
    executor: Mutex<Option<Arc<dyn ScheduledGoalExecutor>>>,
}

impl AgentScheduler {
    /// Create a new scheduler backed by the given cognitive runtime.
    pub fn new(runtime: Arc<CognitiveRuntime>, audit: Arc<Mutex<AuditTrail>>) -> Self {
        Self {
            runtime,
            audit,
            handles: Mutex::new(HashMap::new()),
            executor: Mutex::new(None),
        }
    }

    pub fn set_executor(&self, executor: Arc<dyn ScheduledGoalExecutor>) {
        let mut guard = self.executor.lock().unwrap();
        *guard = Some(executor);
    }

    /// Validate a cron expression without registering anything.
    pub fn validate_cron(expression: &str) -> Result<(), String> {
        Schedule::from_str(&normalize_cron_expression(expression)?)
            .map(|_| ())
            .map_err(|e| format!("invalid cron expression: {e}"))
    }

    /// Register an agent for scheduled execution.
    ///
    /// Parses `cron_expression`, spawns a background task that sleeps until
    /// each successive cron tick and calls `assign_goal()` with `default_goal`.
    pub fn register_agent(
        &self,
        agent_id: &str,
        cron_expression: &str,
        default_goal: &str,
    ) -> Result<(), AgentError> {
        let normalized_cron = normalize_cron_expression(cron_expression)
            .map_err(|e| AgentError::SupervisorError(e.to_string()))?;
        let schedule = Schedule::from_str(&normalized_cron).map_err(|e| {
            AgentError::SupervisorError(format!("invalid cron expression '{cron_expression}': {e}"))
        })?;

        // Cancel any existing schedule for this agent.
        self.unregister_agent(agent_id);

        let next_run = Arc::new(AtomicI64::new(0));
        let runtime = self.runtime.clone();
        let audit = self.audit.clone();
        let aid = agent_id.to_string();
        let goal_text = default_goal.to_string();

        let executor = self.executor.lock().unwrap().clone();
        let handle = tokio::spawn(schedule_loop(
            aid,
            goal_text,
            schedule,
            next_run.clone(),
            runtime,
            audit,
            executor,
        ));

        // Log the registration.
        if let Ok(agent_uuid) = Uuid::parse_str(agent_id) {
            let mut trail = match self.audit.lock() {
                Ok(g) => g,
                Err(p) => p.into_inner(),
            };
            let _ = trail.append_event(
                agent_uuid,
                EventType::StateChange,
                serde_json::json!({
                    "event": "schedule_registered",
                    "cron": cron_expression,
                    "goal": default_goal,
                }),
            );
        }

        let mut handles = self.handles.lock().unwrap();
        handles.insert(
            agent_id.to_string(),
            ScheduleHandle {
                handle,
                cron_expression: cron_expression.to_string(),
                default_goal: default_goal.to_string(),
                next_run,
            },
        );

        Ok(())
    }

    /// Cancel a scheduled agent. No-op if not registered.
    pub fn unregister_agent(&self, agent_id: &str) {
        let mut handles = self.handles.lock().unwrap();
        if let Some(handle) = handles.remove(agent_id) {
            handle.handle.abort();
        }
    }

    /// List all currently scheduled agents with their cron expressions and next run times.
    pub fn list(&self) -> Vec<ScheduledAgent> {
        let handles = self.handles.lock().unwrap();
        handles
            .iter()
            .map(|(id, h)| ScheduledAgent {
                agent_id: id.clone(),
                cron_expression: h.cron_expression.clone(),
                default_goal: h.default_goal.clone(),
                next_run_epoch: h.next_run.load(Ordering::SeqCst),
            })
            .collect()
    }

    /// Shut down all scheduled loops.
    pub fn shutdown(&self) {
        let mut handles = self.handles.lock().unwrap();
        for (_, handle) in handles.drain() {
            handle.handle.abort();
        }
    }
}

/// Background loop: sleep until the next cron tick, then assign the goal.
async fn schedule_loop(
    agent_id: String,
    default_goal: String,
    schedule: Schedule,
    next_run: Arc<AtomicI64>,
    runtime: Arc<CognitiveRuntime>,
    audit: Arc<Mutex<AuditTrail>>,
    executor: Option<Arc<dyn ScheduledGoalExecutor>>,
) {
    loop {
        let now = Utc::now();
        let upcoming = schedule.upcoming(Utc).next();
        let next = match upcoming {
            Some(t) => t,
            None => break, // No more ticks — expression exhausted.
        };

        // Publish next-run for external queries.
        next_run.store(next.timestamp(), Ordering::SeqCst);

        let wait = (next - now).to_std().unwrap_or(Duration::from_secs(1));
        time::sleep(wait).await;

        // Fire the goal.
        let goal = AgentGoal::new(default_goal.to_string(), 5);

        let agent_uuid = Uuid::parse_str(&agent_id).ok();

        let execution = match &executor {
            Some(executor) => executor.execute(&agent_id, &default_goal),
            None => runtime
                .assign_goal(&agent_id, goal)
                .map_err(|e| e.to_string()),
        };

        match execution {
            Ok(()) => {
                if let Some(uuid) = agent_uuid {
                    if let Ok(mut trail) = audit.lock() {
                        let _ = trail.append_event(
                            uuid,
                            EventType::StateChange,
                            serde_json::json!({
                                "event": "scheduled_goal_fired",
                                "goal": default_goal,
                            }),
                        );
                    }
                }
            }
            Err(e) => {
                if let Some(uuid) = agent_uuid {
                    if let Ok(mut trail) = audit.lock() {
                        let _ = trail.append_event(
                            uuid,
                            EventType::Error,
                            serde_json::json!({
                                "event": "scheduled_goal_failed",
                                "error": e.to_string(),
                            }),
                        );
                    }
                }
            }
        }
    }
}

fn normalize_cron_expression(expression: &str) -> Result<String, String> {
    let trimmed = expression.trim();
    if trimmed.is_empty() {
        return Err("invalid cron expression: empty".to_string());
    }

    let parts = trimmed.split_whitespace().collect::<Vec<_>>();
    match parts.len() {
        5 => Ok(format!("0 {}", parts.join(" "))),
        6 | 7 => Ok(trimmed.to_string()),
        other => Err(format!(
            "invalid cron expression: expected 5, 6, or 7 fields, got {other}"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_cron_accepts_valid() {
        assert!(AgentScheduler::validate_cron("*/10 * * * * *").is_ok());
        assert!(AgentScheduler::validate_cron("0 9 * * * *").is_ok());
    }

    #[test]
    fn validate_cron_rejects_invalid() {
        assert!(AgentScheduler::validate_cron("not a cron").is_err());
        assert!(AgentScheduler::validate_cron("").is_err());
    }

    #[test]
    fn list_empty_by_default() {
        use super::super::loop_runtime::{CognitiveRuntime, NoOpEmitter};
        use super::super::types::LoopConfig;
        use crate::supervisor::Supervisor;

        let sup = Arc::new(Mutex::new(Supervisor::new()));
        let rt = Arc::new(CognitiveRuntime::new(
            sup,
            LoopConfig::default(),
            Arc::new(NoOpEmitter),
        ));
        let audit = Arc::new(Mutex::new(AuditTrail::new()));
        let sched = AgentScheduler::new(rt, audit);
        assert!(sched.list().is_empty());
    }
}
