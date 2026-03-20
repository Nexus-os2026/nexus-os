use nexus_kernel::audit::{AuditTrail, EventType};
use nexus_kernel::errors::AgentError;
use nexus_kernel::lifecycle::{transition_state, AgentState};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChallengeType {
    Captcha,
    RateLimit,
    AuthExpired,
    BotBlock,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EscalationEvent {
    pub agent_id: Uuid,
    pub challenge_type: ChallengeType,
    pub message: String,
    pub timestamp: u64,
}

pub fn detect_challenge(http_response: &str) -> Option<ChallengeType> {
    let normalized = http_response.to_lowercase();

    if normalized.contains("captcha") || normalized.contains("recaptcha") {
        return Some(ChallengeType::Captcha);
    }
    if normalized.contains("429") || normalized.contains("rate limit") {
        return Some(ChallengeType::RateLimit);
    }
    if normalized.contains("401")
        || normalized.contains("auth expired")
        || normalized.contains("token expired")
    {
        return Some(ChallengeType::AuthExpired);
    }
    if normalized.contains("bot block")
        || normalized.contains("bot detected")
        || normalized.contains("access denied")
    {
        return Some(ChallengeType::BotBlock);
    }

    None
}

pub fn handle_challenge(
    agent_id: Uuid,
    http_response: &str,
    state: &mut AgentState,
    audit_trail: &mut AuditTrail,
) -> Option<EscalationEvent> {
    let challenge_type = detect_challenge(http_response)?;

    if *state == AgentState::Running {
        if let Ok(next) = transition_state(*state, AgentState::Paused) {
            *state = next;
        } else {
            *state = AgentState::Paused;
        }
    }

    let timestamp = current_unix_timestamp();
    let message = format!(
        "challenge detected: {:?}; user intervention required",
        challenge_type
    );

    if let Err(e) = audit_trail.append_event(
        agent_id,
        EventType::Error,
        json!({
            "event": "challenge_detected",
            "challenge_type": format!("{:?}", challenge_type),
            "message": message,
            "timestamp": timestamp
        }),
    ) {
        tracing::error!("Audit append failed: {e}");
    }

    Some(EscalationEvent {
        agent_id,
        challenge_type,
        message,
        timestamp,
    })
}

pub fn resume_after_resolution(
    agent_id: Uuid,
    state: &mut AgentState,
    audit_trail: &mut AuditTrail,
    user_evidence: &str,
) -> Result<(), AgentError> {
    if *state != AgentState::Paused {
        return Err(AgentError::InvalidTransition {
            from: *state,
            to: AgentState::Running,
        });
    }

    *state = transition_state(*state, AgentState::Running)?;

    if let Err(e) = audit_trail.append_event(
        agent_id,
        EventType::UserAction,
        json!({
            "event": "challenge_resolved",
            "evidence": user_evidence,
            "timestamp": current_unix_timestamp()
        }),
    ) {
        tracing::error!("Audit append failed: {e}");
    }

    Ok(())
}

fn current_unix_timestamp() -> u64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_secs(),
        Err(_) => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::{detect_challenge, ChallengeType};

    #[test]
    fn test_detect_captcha() {
        let html = "<html><body><h1>Please complete CAPTCHA verification</h1></body></html>";
        let challenge = detect_challenge(html);
        assert_eq!(challenge, Some(ChallengeType::Captcha));
    }
}
