use nexus_kernel::audit::{AuditTrail, EventType};
use nexus_kernel::errors::AgentError;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FailureStrategy {
    Retry,
    Skip,
    Escalate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecoveryActionResult {
    RetryScheduled { delay_seconds: u64 },
    StepSkipped,
    EscalatedToUser,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetryPolicy {
    pub max_retries: u32,
    pub max_backoff_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetryExecution<T> {
    pub value: T,
    pub delay_schedule_seconds: Vec<u64>,
}

pub struct FailureRecoveryManager {
    pub audit_trail: AuditTrail,
    paused_workflows: HashSet<String>,
    executed_compensations: HashSet<String>,
}

impl FailureRecoveryManager {
    pub fn new() -> Self {
        Self {
            audit_trail: AuditTrail::new(),
            paused_workflows: HashSet::new(),
            executed_compensations: HashSet::new(),
        }
    }

    pub fn handle_failure(
        &mut self,
        workflow_id: &str,
        step_id: &str,
        strategy: FailureStrategy,
        attempt: u32,
    ) -> RecoveryActionResult {
        match strategy {
            FailureStrategy::Retry => {
                let delay = backoff_delay_seconds(attempt, 60);
                let _ = self.audit_trail.append_event(
                    uuid::Uuid::nil(),
                    EventType::Error,
                    json!({
                        "event": "workflow_retry_scheduled",
                        "workflow_id": workflow_id,
                        "step_id": step_id,
                        "attempt": attempt,
                        "delay_seconds": delay
                    }),
                );
                RecoveryActionResult::RetryScheduled {
                    delay_seconds: delay,
                }
            }
            FailureStrategy::Skip => {
                let _ = self.audit_trail.append_event(
                    uuid::Uuid::nil(),
                    EventType::Error,
                    json!({
                        "event": "workflow_step_skipped",
                        "workflow_id": workflow_id,
                        "step_id": step_id
                    }),
                );
                RecoveryActionResult::StepSkipped
            }
            FailureStrategy::Escalate => {
                self.paused_workflows.insert(workflow_id.to_string());
                let _ = self.audit_trail.append_event(
                    uuid::Uuid::nil(),
                    EventType::Error,
                    json!({
                        "event": "workflow_escalated",
                        "workflow_id": workflow_id,
                        "step_id": step_id,
                        "notify": "messaging_bridge"
                    }),
                );
                RecoveryActionResult::EscalatedToUser
            }
        }
    }

    pub fn is_paused(&self, workflow_id: &str) -> bool {
        self.paused_workflows.contains(workflow_id)
    }

    pub fn run_compensating_action(
        &mut self,
        action_id: &str,
        action: impl FnOnce() -> Result<(), AgentError>,
    ) -> Result<bool, AgentError> {
        if self.executed_compensations.contains(action_id) {
            return Ok(false);
        }

        action()?;
        self.executed_compensations
            .insert(action_id.to_string());

        let _ = self.audit_trail.append_event(
            uuid::Uuid::nil(),
            EventType::ToolCall,
            json!({
                "event": "workflow_compensation_applied",
                "action_id": action_id
            }),
        );

        Ok(true)
    }
}

impl Default for FailureRecoveryManager {
    fn default() -> Self {
        Self::new()
    }
}

pub fn retry_with_backoff<T>(
    policy: &RetryPolicy,
    mut operation: impl FnMut(u32) -> Result<T, AgentError>,
) -> Result<RetryExecution<T>, AgentError> {
    let mut attempt = 0_u32;
    let mut delays = Vec::new();

    loop {
        attempt += 1;
        match operation(attempt) {
            Ok(value) => {
                return Ok(RetryExecution {
                    value,
                    delay_schedule_seconds: delays,
                });
            }
            Err(error) => {
                if attempt > policy.max_retries {
                    return Err(error);
                }
                let delay = backoff_delay_seconds(attempt, policy.max_backoff_seconds);
                delays.push(delay);
            }
        }
    }
}

pub fn backoff_delay_seconds(attempt: u32, max_backoff_seconds: u64) -> u64 {
    let exp = attempt.saturating_sub(1);
    let calculated = 2_u64.saturating_pow(exp);
    calculated.clamp(1, max_backoff_seconds)
}

#[cfg(test)]
mod tests {
    use super::{retry_with_backoff, RetryPolicy};
    use nexus_kernel::errors::AgentError;
    use std::cell::Cell;

    #[test]
    fn test_retry_with_backoff() {
        let policy = RetryPolicy {
            max_retries: 5,
            max_backoff_seconds: 60,
        };
        let failures = Cell::new(0_u32);

        let result = retry_with_backoff(&policy, |_| {
            let current = failures.get();
            if current < 3 {
                failures.set(current + 1);
                Err(AgentError::SupervisorError("temporary failure".to_string()))
            } else {
                Ok("ok")
            }
        });

        assert!(result.is_ok());
        if let Ok(execution) = result {
            assert_eq!(execution.value, "ok");
            assert_eq!(execution.delay_schedule_seconds, vec![1, 2, 4]);
        }
    }
}
