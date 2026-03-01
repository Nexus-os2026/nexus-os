use crate::strategy::StrategyDocument;
use nexus_kernel::audit::{AuditTrail, EventType};
use nexus_kernel::errors::AgentError;
use nexus_kernel::lifecycle::{transition_state, AgentState};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReviewDecision {
    Approve,
    Reject,
    RequestChanges(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReviewStatus {
    Pending,
    Approved,
    Rejected,
    ChangesRequested,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewRequest {
    pub request_id: String,
    pub status: ReviewStatus,
    pub strategy: StrategyDocument,
}

pub struct UserReviewGate {
    decisions: HashMap<String, ReviewDecision>,
    pub audit_trail: AuditTrail,
}

impl UserReviewGate {
    pub fn new() -> Self {
        Self {
            decisions: HashMap::new(),
            audit_trail: AuditTrail::new(),
        }
    }

    pub fn present_for_review(&mut self, strategy: StrategyDocument) -> ReviewRequest {
        let request_id = Uuid::new_v4().to_string();
        ReviewRequest {
            request_id,
            status: ReviewStatus::Pending,
            strategy,
        }
    }

    pub fn record_decision(
        &mut self,
        agent_id: Uuid,
        request_id: &str,
        decision: ReviewDecision,
    ) {
        self.decisions
            .insert(request_id.to_string(), decision.clone());

        let _ = self.audit_trail.append_event(
            agent_id,
            EventType::UserAction,
            json!({
                "event": "research_review_decision",
                "request_id": request_id,
                "decision": format!("{decision:?}")
            }),
        );
    }

    pub fn enforce_approval_for_running(
        &self,
        request_id: &str,
        current_state: AgentState,
    ) -> Result<AgentState, AgentError> {
        match self.decisions.get(request_id) {
            Some(ReviewDecision::Approve) => transition_state(current_state, AgentState::Running),
            Some(ReviewDecision::Reject) => Err(AgentError::SupervisorError(
                "strategy rejected; cannot transition to Running".to_string(),
            )),
            Some(ReviewDecision::RequestChanges(_)) => Err(AgentError::SupervisorError(
                "strategy requires changes; cannot transition to Running".to_string(),
            )),
            None => Err(AgentError::SupervisorError(
                "review decision missing; approval required".to_string(),
            )),
        }
    }
}

impl Default for UserReviewGate {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{ReviewDecision, UserReviewGate};
    use crate::pipeline::Citation;
    use crate::strategy::StrategyDocument;
    use nexus_kernel::lifecycle::AgentState;
    use uuid::Uuid;

    fn sample_strategy() -> StrategyDocument {
        StrategyDocument {
            executive_summary: "Use educational hooks".to_string(),
            key_findings: vec!["Competitors post daily".to_string()],
            recommended_actions: vec!["Post weekly technical deep-dives".to_string()],
            risks: vec!["Audience fatigue".to_string()],
            citations: vec![Citation {
                title: "source".to_string(),
                url: "https://example.com".to_string(),
                snippet: "snippet".to_string(),
            }],
        }
    }

    #[test]
    fn test_user_review_gate_blocks_unapproved() {
        let mut gate = UserReviewGate::new();
        let request = gate.present_for_review(sample_strategy());

        let blocked = gate.enforce_approval_for_running(request.request_id.as_str(), AgentState::Starting);
        assert!(blocked.is_err());

        gate.record_decision(Uuid::new_v4(), request.request_id.as_str(), ReviewDecision::Reject);
        let still_blocked =
            gate.enforce_approval_for_running(request.request_id.as_str(), AgentState::Starting);
        assert!(still_blocked.is_err());

        gate.record_decision(Uuid::new_v4(), request.request_id.as_str(), ReviewDecision::Approve);
        let allowed = gate.enforce_approval_for_running(request.request_id.as_str(), AgentState::Starting);
        assert_eq!(allowed, Ok(AgentState::Running));
    }
}
