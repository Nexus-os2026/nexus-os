use serde::{Deserialize, Serialize};

/// Roles that agents can play in a collaboration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CollaborationRole {
    Lead,
    Expert { domain: String },
    Reviewer,
    Contributor,
    Observer,
}

impl CollaborationRole {
    pub fn can_propose(&self) -> bool {
        matches!(self, Self::Lead | Self::Expert { .. } | Self::Contributor)
    }

    pub fn can_vote(&self) -> bool {
        !matches!(self, Self::Observer)
    }

    pub fn can_call_vote(&self) -> bool {
        matches!(self, Self::Lead)
    }

    pub fn can_declare_consensus(&self) -> bool {
        matches!(self, Self::Lead)
    }

    pub fn can_escalate(&self) -> bool {
        !matches!(self, Self::Observer)
    }

    pub fn can_send_messages(&self) -> bool {
        !matches!(self, Self::Observer)
    }
}

/// A participant in a collaboration session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Participant {
    pub agent_id: String,
    pub autonomy_level: u8,
    pub role: CollaborationRole,
    pub joined_at: u64,
    pub messages_sent: u32,
    pub votes_cast: u32,
}
