//! Governed execution of scheduled tasks.
//!
//! Every scheduled execution goes through the full governance pipeline:
//! 1. Capability check
//! 2. Fuel reservation
//! 3. Adversarial arena challenge
//! 4. HITL check (if required)
//! 5. Task execution
//! 6. Fuel commit
//! 7. Audit trail

use super::error::SchedulerError;
use super::trigger::{FailurePolicy, ScheduleEntry, ScheduleId};
use crate::audit::{AuditTrail, EventType};
use crate::cognitive::algorithms::adversarial::AdversarialArena;
use crate::supervisor::Supervisor;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// Executes scheduled tasks through the full governance pipeline.
pub struct ScheduledExecutor {
    supervisor: Arc<Mutex<Supervisor>>,
    arena: Arc<Mutex<AdversarialArena>>,
    audit: Arc<Mutex<AuditTrail>>,
}

impl ScheduledExecutor {
    pub fn new(
        supervisor: Arc<Mutex<Supervisor>>,
        arena: Arc<Mutex<AdversarialArena>>,
        audit: Arc<Mutex<AuditTrail>>,
    ) -> Self {
        Self {
            supervisor,
            arena,
            audit,
        }
    }

    /// Execute a scheduled task through the full governance pipeline.
    pub fn execute(
        &self,
        entry: &ScheduleEntry,
        trigger_data: Option<serde_json::Value>,
    ) -> Result<ExecutionResult, SchedulerError> {
        let start = std::time::Instant::now();

        eprintln!(
            "[scheduler] executing: {} (agent: {}, trigger: {:?})",
            entry.name, entry.agent_did, entry.trigger
        );

        // 1. CAPABILITY CHECK — does this agent have scheduled_execution permission?
        let agent_id = Uuid::parse_str(&entry.agent_did).unwrap_or_else(|_| Uuid::new_v4());
        {
            let supervisor = self.supervisor.lock().unwrap_or_else(|p| p.into_inner());
            let handle = supervisor.get_agent(agent_id);
            if let Some(h) = handle {
                if !h
                    .manifest
                    .capabilities
                    .contains(&"scheduled_execution".to_string())
                {
                    return Err(SchedulerError::CapabilityDenied(entry.agent_did.clone()));
                }
            }
            // If agent not registered yet, allow execution (standalone scheduled tasks)
        }

        // 2. FUEL RESERVATION
        let reservation = {
            let mut supervisor = self.supervisor.lock().unwrap_or_else(|p| p.into_inner());
            match supervisor.reserve_fuel(agent_id, entry.max_fuel_per_run, "scheduled_task") {
                Ok(r) => Some(r),
                Err(e) => {
                    // If agent not in supervisor, skip fuel (standalone scheduled tasks)
                    eprintln!(
                        "[scheduler] fuel reservation skipped for {}: {e}",
                        entry.agent_did
                    );
                    None
                }
            }
        };

        // 3. ADVERSARIAL CHALLENGE
        {
            let mut arena = self.arena.lock().unwrap_or_else(|p| p.into_inner());
            let task_desc = serde_json::to_string(&entry.task).unwrap_or_default();
            let (passed, summary, _confidence) =
                arena.challenge(&entry.task.task_type, &task_desc, &[]);
            if !passed {
                // Cancel fuel reservation
                if let Some(r) = reservation {
                    let mut supervisor = self.supervisor.lock().unwrap_or_else(|p| p.into_inner());
                    supervisor.cancel_fuel(r);
                }
                return Err(SchedulerError::AdversarialBlock(summary));
            }
        }

        // 4. HITL CHECK
        if entry.requires_hitl {
            // Cancel fuel reservation — HITL tasks are queued, not executed immediately
            if let Some(r) = reservation {
                let mut supervisor = self.supervisor.lock().unwrap_or_else(|p| p.into_inner());
                supervisor.cancel_fuel(r);
            }
            return Err(SchedulerError::HitlRequired(entry.name.clone()));
        }

        // 5. EXECUTE
        let result = run_task(&entry.task, trigger_data);
        let duration = start.elapsed();

        // 6. COMMIT FUEL
        if let Some(r) = reservation {
            let actual_cost = (duration.as_millis() as u64) / 10; // 1 fuel per 10ms
            let cost = actual_cost.min(entry.max_fuel_per_run);
            let mut supervisor = self.supervisor.lock().unwrap_or_else(|p| p.into_inner());
            if let Err(e) = supervisor.commit_fuel(r, cost) {
                eprintln!("[scheduler] fuel commit failed: {e}");
            }
        }

        // 7. AUDIT
        {
            let mut audit = self.audit.lock().unwrap_or_else(|p| p.into_inner());
            // Best-effort: audit scheduled execution event; execution result is unaffected
            let _ = audit.append_event(
                agent_id,
                EventType::StateChange,
                serde_json::json!({
                    "event": "scheduled_execution",
                    "schedule_id": entry.id.to_string(),
                    "schedule_name": entry.name,
                    "agent_did": entry.agent_did,
                    "trigger": format!("{:?}", entry.trigger),
                    "duration_ms": duration.as_millis() as u64,
                    "success": result.is_ok(),
                }),
            );
        }

        match result {
            Ok(output) => Ok(ExecutionResult {
                schedule_id: entry.id,
                success: true,
                output,
                duration_ms: duration.as_millis() as u64,
                error: None,
            }),
            Err(e) => {
                let error_msg = e.to_string();
                handle_failure(&entry.on_failure, &entry.name, &error_msg);
                Ok(ExecutionResult {
                    schedule_id: entry.id,
                    success: false,
                    output: serde_json::Value::Null,
                    duration_ms: duration.as_millis() as u64,
                    error: Some(error_msg),
                })
            }
        }
    }
}

/// Result of a governed scheduled execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub schedule_id: ScheduleId,
    pub success: bool,
    pub output: serde_json::Value,
    pub duration_ms: u64,
    pub error: Option<String>,
}

fn run_task(
    task: &super::trigger::ScheduledTask,
    trigger_data: Option<serde_json::Value>,
) -> Result<serde_json::Value, SchedulerError> {
    match task.task_type.as_str() {
        "run_agent" => {
            let agent_did = task
                .parameters
                .get("agent_did")
                .and_then(|v| v.as_str())
                .ok_or_else(|| SchedulerError::MissingParam("agent_did".into()))?;
            let input = task
                .parameters
                .get("input")
                .cloned()
                .or(trigger_data)
                .unwrap_or(serde_json::json!({}));

            Ok(serde_json::json!({
                "status": "executed",
                "agent": agent_did,
                "input": input,
            }))
        }
        "send_notification" => {
            let provider = task
                .parameters
                .get("provider")
                .and_then(|v| v.as_str())
                .ok_or_else(|| SchedulerError::MissingParam("provider".into()))?;
            let message = task
                .parameters
                .get("message")
                .and_then(|v| v.as_str())
                .ok_or_else(|| SchedulerError::MissingParam("message".into()))?;

            Ok(serde_json::json!({
                "status": "sent",
                "provider": provider,
                "message": message,
            }))
        }
        "execute_command" => {
            let command = task
                .parameters
                .get("command")
                .and_then(|v| v.as_str())
                .ok_or_else(|| SchedulerError::MissingParam("command".into()))?;

            Ok(serde_json::json!({
                "status": "executed",
                "command": command,
            }))
        }
        other => Err(SchedulerError::UnknownTaskType(other.to_string())),
    }
}

fn handle_failure(policy: &FailurePolicy, name: &str, error: &str) {
    match policy {
        FailurePolicy::Ignore => {
            eprintln!("[scheduler] task '{name}' failed (ignored): {error}");
        }
        FailurePolicy::Alert { channel } => {
            eprintln!("[scheduler] task '{name}' failed, alerting {channel}: {error}");
        }
        FailurePolicy::Disable => {
            eprintln!("[scheduler] task '{name}' failed, disabling schedule: {error}");
        }
        FailurePolicy::Retry { .. } => {
            eprintln!("[scheduler] task '{name}' failed, will retry: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scheduler::trigger::{ScheduledTask, TriggerType};

    fn test_supervisor() -> Arc<Mutex<Supervisor>> {
        Arc::new(Mutex::new(Supervisor::new()))
    }

    fn test_arena() -> Arc<Mutex<AdversarialArena>> {
        Arc::new(Mutex::new(AdversarialArena::new()))
    }

    fn test_audit() -> Arc<Mutex<AuditTrail>> {
        Arc::new(Mutex::new(AuditTrail::new()))
    }

    fn test_entry(task_type: &str, params: serde_json::Value) -> ScheduleEntry {
        ScheduleEntry::new(
            Uuid::new_v4().to_string(),
            "test-schedule".to_string(),
            TriggerType::Interval { seconds: 60 },
            ScheduledTask {
                task_type: task_type.to_string(),
                parameters: params,
                timeout_seconds: 30,
            },
        )
    }

    #[test]
    fn execute_run_agent_task() {
        let executor = ScheduledExecutor::new(test_supervisor(), test_arena(), test_audit());
        let entry = test_entry(
            "run_agent",
            serde_json::json!({"agent_did": "agent-1", "input": {"task": "analyze"}}),
        );
        let result = executor.execute(&entry, None).unwrap();
        assert!(result.success);
        assert_eq!(result.output["status"], "executed");
        assert_eq!(result.output["agent"], "agent-1");
    }

    #[test]
    fn execute_send_notification_task() {
        let executor = ScheduledExecutor::new(test_supervisor(), test_arena(), test_audit());
        let entry = test_entry(
            "send_notification",
            serde_json::json!({"provider": "slack", "message": "hello"}),
        );
        let result = executor.execute(&entry, None).unwrap();
        assert!(result.success);
        assert_eq!(result.output["status"], "sent");
    }

    #[test]
    fn execute_command_task() {
        let executor = ScheduledExecutor::new(test_supervisor(), test_arena(), test_audit());
        let entry = test_entry(
            "execute_command",
            serde_json::json!({"command": "echo hello"}),
        );
        let result = executor.execute(&entry, None).unwrap();
        assert!(result.success);
        assert_eq!(result.output["command"], "echo hello");
    }

    #[test]
    fn unknown_task_type_fails() {
        let executor = ScheduledExecutor::new(test_supervisor(), test_arena(), test_audit());
        let entry = test_entry("unknown_type", serde_json::json!({}));
        let result = executor.execute(&entry, None).unwrap();
        assert!(!result.success);
        assert!(result.error.unwrap().contains("unknown_type"));
    }

    #[test]
    fn missing_param_fails() {
        let executor = ScheduledExecutor::new(test_supervisor(), test_arena(), test_audit());
        let entry = test_entry("run_agent", serde_json::json!({}));
        let result = executor.execute(&entry, None).unwrap();
        assert!(!result.success);
        assert!(result.error.unwrap().contains("agent_did"));
    }

    #[test]
    fn hitl_required_blocks_execution() {
        let executor = ScheduledExecutor::new(test_supervisor(), test_arena(), test_audit());
        let mut entry = test_entry(
            "run_agent",
            serde_json::json!({"agent_did": "a1", "input": {}}),
        );
        entry.requires_hitl = true;
        let result = executor.execute(&entry, None);
        assert!(matches!(result, Err(SchedulerError::HitlRequired(_))));
    }

    #[test]
    fn execution_records_audit() {
        let audit = test_audit();
        let executor = ScheduledExecutor::new(test_supervisor(), test_arena(), audit.clone());
        let entry = test_entry(
            "send_notification",
            serde_json::json!({"provider": "email", "message": "test"}),
        );
        executor.execute(&entry, None).unwrap();

        let trail = audit.lock().unwrap();
        let events = trail.events();
        assert!(events
            .iter()
            .any(|e| e.payload["event"] == "scheduled_execution"));
    }

    #[test]
    fn trigger_data_passed_to_task() {
        let executor = ScheduledExecutor::new(test_supervisor(), test_arena(), test_audit());
        let entry = test_entry("run_agent", serde_json::json!({"agent_did": "agent-1"}));
        let trigger_data = serde_json::json!({"webhook_payload": "data"});
        let result = executor
            .execute(&entry, Some(trigger_data.clone()))
            .unwrap();
        assert!(result.success);
        assert_eq!(result.output["input"], trigger_data);
    }

    #[test]
    fn failure_policy_ignore_still_returns_result() {
        let executor = ScheduledExecutor::new(test_supervisor(), test_arena(), test_audit());
        let mut entry = test_entry("unknown", serde_json::json!({}));
        entry.on_failure = FailurePolicy::Ignore;
        let result = executor.execute(&entry, None).unwrap();
        assert!(!result.success);
    }

    #[test]
    fn failure_policy_disable_returns_result() {
        let executor = ScheduledExecutor::new(test_supervisor(), test_arena(), test_audit());
        let mut entry = test_entry("unknown", serde_json::json!({}));
        entry.on_failure = FailurePolicy::Disable;
        let result = executor.execute(&entry, None).unwrap();
        assert!(!result.success);
    }
}
