use serde::{Deserialize, Serialize};

pub const COLLABORATION_CAPABILITY: &str = "agent_collaboration";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollaborationPolicy {
    pub min_autonomy_level: u8,
    pub max_participants: usize,
    pub max_active_sessions: usize,
    pub max_messages_per_session: usize,
    pub vote_timeout_secs: u64,
    pub session_creation_cost: u64,
    pub message_cost: u64,
    pub vote_cost: u64,
}

impl Default for CollaborationPolicy {
    fn default() -> Self {
        Self {
            min_autonomy_level: 2,
            max_participants: 8,
            max_active_sessions: 10,
            max_messages_per_session: 200,
            vote_timeout_secs: 300,
            session_creation_cost: 5_000_000,
            message_cost: 100_000,
            vote_cost: 200_000,
        }
    }
}

impl CollaborationPolicy {
    pub fn check_authorization(&self, autonomy_level: u8) -> Result<(), String> {
        if autonomy_level < self.min_autonomy_level {
            return Err(format!(
                "Collaboration requires L{}+, agent is L{}",
                self.min_autonomy_level, autonomy_level
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_governance_min_autonomy() {
        let policy = CollaborationPolicy::default();
        assert!(policy.check_authorization(1).is_err());
        assert!(policy.check_authorization(2).is_ok());
        assert!(policy.check_authorization(3).is_ok());
    }
}
