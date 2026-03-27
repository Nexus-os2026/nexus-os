use serde::{Deserialize, Serialize};

use crate::message::{CollaborationMessage, MessageType};
use crate::patterns::CollaborationPattern;
use crate::roles::{CollaborationRole, Participant};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollaborationSession {
    pub id: String,
    pub title: String,
    pub goal: String,
    pub pattern: CollaborationPattern,
    pub participants: Vec<Participant>,
    pub messages: Vec<CollaborationMessage>,
    pub status: SessionStatus,
    pub created_at: u64,
    pub completed_at: Option<u64>,
    pub outcome: Option<CollaborationOutcome>,
    pub active_vote: Option<ActiveVote>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionStatus {
    Forming,
    Active,
    Voting,
    Converging,
    Completed,
    Escalated,
    Failed { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveVote {
    pub proposal_message_id: String,
    pub proposal_text: String,
    pub votes: Vec<VoteRecord>,
    pub required_majority: f64,
    pub deadline: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoteRecord {
    pub agent_id: String,
    pub vote: VoteChoice,
    pub reason: Option<String>,
    pub timestamp: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum VoteChoice {
    Approve,
    Reject,
    Abstain,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollaborationOutcome {
    pub decision: String,
    pub method: ConsensusMethod,
    pub confidence: f64,
    pub dissents: Vec<Dissent>,
    pub key_points: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConsensusMethod {
    Unanimous,
    MajorityVote {
        for_count: u32,
        against_count: u32,
        abstain_count: u32,
    },
    LeadDecision,
    HumanDecision,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dissent {
    pub agent_id: String,
    pub reason: String,
}

#[derive(Debug, thiserror::Error)]
pub enum CollabError {
    #[error("Session not accepting participants")]
    SessionNotAccepting,
    #[error("Agent {0} is already a participant")]
    AlreadyParticipant(String),
    #[error("Agent {0} is not a participant")]
    NotParticipant(String),
    #[error("Insufficient participants (need at least 2)")]
    InsufficientParticipants,
    #[error("Invalid state: {0}")]
    InvalidState(String),
    #[error("Role denied: {0}")]
    RoleDenied(String),
    #[error("Message not found: {0}")]
    MessageNotFound(String),
    #[error("Vote already active")]
    VoteAlreadyActive,
    #[error("No active vote")]
    NoActiveVote,
    #[error("Agent {0} already voted")]
    AlreadyVoted(String),
    #[error("Governance denied: {0}")]
    GovernanceDenied(String),
}

impl CollaborationSession {
    pub fn new(
        title: String,
        goal: String,
        pattern: CollaborationPattern,
        lead_agent: &str,
        lead_autonomy: u8,
    ) -> Self {
        let lead = Participant {
            agent_id: lead_agent.into(),
            autonomy_level: lead_autonomy,
            role: CollaborationRole::Lead,
            joined_at: epoch_now(),
            messages_sent: 0,
            votes_cast: 0,
        };
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            title,
            goal,
            pattern,
            participants: vec![lead],
            messages: Vec::new(),
            status: SessionStatus::Forming,
            created_at: epoch_now(),
            completed_at: None,
            outcome: None,
            active_vote: None,
        }
    }

    pub fn add_participant(
        &mut self,
        agent_id: &str,
        autonomy_level: u8,
        role: CollaborationRole,
    ) -> Result<(), CollabError> {
        if self.status != SessionStatus::Forming && self.status != SessionStatus::Active {
            return Err(CollabError::SessionNotAccepting);
        }
        if self.participants.iter().any(|p| p.agent_id == agent_id) {
            return Err(CollabError::AlreadyParticipant(agent_id.into()));
        }
        self.participants.push(Participant {
            agent_id: agent_id.into(),
            autonomy_level,
            role,
            joined_at: epoch_now(),
            messages_sent: 0,
            votes_cast: 0,
        });
        Ok(())
    }

    pub fn start(&mut self) -> Result<(), CollabError> {
        if self.status != SessionStatus::Forming {
            return Err(CollabError::InvalidState("Not in Forming state".into()));
        }
        if self.participants.len() < 2 {
            return Err(CollabError::InsufficientParticipants);
        }
        self.status = SessionStatus::Active;
        Ok(())
    }

    pub fn send_message(&mut self, message: CollaborationMessage) -> Result<String, CollabError> {
        if self.status != SessionStatus::Active && self.status != SessionStatus::Voting {
            return Err(CollabError::InvalidState("Session not active".into()));
        }
        let participant = self
            .participants
            .iter_mut()
            .find(|p| p.agent_id == message.from_agent)
            .ok_or_else(|| CollabError::NotParticipant(message.from_agent.clone()))?;

        if !participant.role.can_send_messages() {
            return Err(CollabError::RoleDenied(
                "Observers cannot send messages".into(),
            ));
        }
        match &message.message_type {
            MessageType::CallVote if !participant.role.can_call_vote() => {
                return Err(CollabError::RoleDenied("Only Lead can call votes".into()));
            }
            MessageType::DeclareConsensus if !participant.role.can_declare_consensus() => {
                return Err(CollabError::RoleDenied(
                    "Only Lead can declare consensus".into(),
                ));
            }
            _ => {}
        }
        participant.messages_sent += 1;
        let msg_id = message.id.clone();
        self.messages.push(message);
        Ok(msg_id)
    }

    pub fn call_vote(
        &mut self,
        proposal_message_id: &str,
        required_majority: f64,
        deadline_secs: u64,
    ) -> Result<(), CollabError> {
        if self.active_vote.is_some() {
            return Err(CollabError::VoteAlreadyActive);
        }
        let proposal = self
            .messages
            .iter()
            .find(|m| m.id == proposal_message_id)
            .ok_or_else(|| CollabError::MessageNotFound(proposal_message_id.into()))?;
        self.active_vote = Some(ActiveVote {
            proposal_message_id: proposal_message_id.into(),
            proposal_text: proposal.content.text.clone(),
            votes: Vec::new(),
            required_majority: required_majority.clamp(0.5, 1.0),
            deadline: epoch_now() + deadline_secs,
        });
        self.status = SessionStatus::Voting;
        Ok(())
    }

    pub fn cast_vote(
        &mut self,
        agent_id: &str,
        vote: VoteChoice,
        reason: Option<String>,
    ) -> Result<(), CollabError> {
        let active_vote = self.active_vote.as_mut().ok_or(CollabError::NoActiveVote)?;

        let participant = self
            .participants
            .iter_mut()
            .find(|p| p.agent_id == agent_id)
            .ok_or_else(|| CollabError::NotParticipant(agent_id.into()))?;
        if !participant.role.can_vote() {
            return Err(CollabError::RoleDenied("Role cannot vote".into()));
        }
        if active_vote.votes.iter().any(|v| v.agent_id == agent_id) {
            return Err(CollabError::AlreadyVoted(agent_id.into()));
        }
        participant.votes_cast += 1;
        active_vote.votes.push(VoteRecord {
            agent_id: agent_id.into(),
            vote,
            reason,
            timestamp: epoch_now(),
        });

        let eligible = self
            .participants
            .iter()
            .filter(|p| p.role.can_vote())
            .count();
        if self
            .active_vote
            .as_ref()
            .map(|v| v.votes.len() >= eligible)
            .unwrap_or(false)
        {
            self.resolve_vote();
        }
        Ok(())
    }

    fn resolve_vote(&mut self) {
        if let Some(vote) = &self.active_vote {
            let for_count = vote
                .votes
                .iter()
                .filter(|v| v.vote == VoteChoice::Approve)
                .count() as u32;
            let against_count = vote
                .votes
                .iter()
                .filter(|v| v.vote == VoteChoice::Reject)
                .count() as u32;
            let abstain_count = vote
                .votes
                .iter()
                .filter(|v| v.vote == VoteChoice::Abstain)
                .count() as u32;
            let total_non_abstain = for_count + against_count;

            let passed = if total_non_abstain > 0 {
                (for_count as f64 / total_non_abstain as f64) >= vote.required_majority
            } else {
                false
            };

            if passed {
                let dissents: Vec<Dissent> = vote
                    .votes
                    .iter()
                    .filter(|v| v.vote == VoteChoice::Reject)
                    .map(|v| Dissent {
                        agent_id: v.agent_id.clone(),
                        reason: v.reason.clone().unwrap_or_else(|| "No reason given".into()),
                    })
                    .collect();
                self.outcome = Some(CollaborationOutcome {
                    decision: vote.proposal_text.clone(),
                    method: ConsensusMethod::MajorityVote {
                        for_count,
                        against_count,
                        abstain_count,
                    },
                    confidence: for_count as f64
                        / (for_count + against_count + abstain_count) as f64,
                    dissents,
                    key_points: Vec::new(),
                });
                self.status = SessionStatus::Completed;
                self.completed_at = Some(epoch_now());
            } else {
                self.status = SessionStatus::Active;
            }
        }
        self.active_vote = None;
    }

    pub fn declare_consensus(
        &mut self,
        agent_id: &str,
        decision: &str,
        key_points: Vec<String>,
    ) -> Result<(), CollabError> {
        let participant = self
            .participants
            .iter()
            .find(|p| p.agent_id == agent_id)
            .ok_or_else(|| CollabError::NotParticipant(agent_id.into()))?;
        if !participant.role.can_declare_consensus() {
            return Err(CollabError::RoleDenied(
                "Only Lead can declare consensus".into(),
            ));
        }

        let confidences: Vec<f64> = self
            .messages
            .iter()
            .filter(|m| m.message_type == MessageType::Agree)
            .map(|m| m.content.confidence)
            .collect();
        let avg_confidence = if confidences.is_empty() {
            0.5
        } else {
            confidences.iter().sum::<f64>() / confidences.len() as f64
        };

        self.outcome = Some(CollaborationOutcome {
            decision: decision.into(),
            method: ConsensusMethod::LeadDecision,
            confidence: avg_confidence,
            dissents: Vec::new(),
            key_points,
        });
        self.status = SessionStatus::Completed;
        self.completed_at = Some(epoch_now());
        Ok(())
    }

    pub fn escalate(&mut self, agent_id: &str, _reason: &str) -> Result<(), CollabError> {
        let participant = self
            .participants
            .iter()
            .find(|p| p.agent_id == agent_id)
            .ok_or_else(|| CollabError::NotParticipant(agent_id.into()))?;
        if !participant.role.can_escalate() {
            return Err(CollabError::RoleDenied("Role cannot escalate".into()));
        }
        self.status = SessionStatus::Escalated;
        Ok(())
    }

    pub fn participant_count(&self) -> usize {
        self.participants.len()
    }

    pub fn message_count(&self) -> usize {
        self.messages.len()
    }
}

fn epoch_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_session() -> CollaborationSession {
        CollaborationSession::new(
            "Design Review".into(),
            "Review API design".into(),
            CollaborationPattern::PeerReview,
            "lead-1",
            4,
        )
    }

    #[test]
    fn test_session_creation() {
        let s = make_session();
        assert_eq!(s.status, SessionStatus::Forming);
        assert_eq!(s.participant_count(), 1);
        assert_eq!(s.participants[0].role, CollaborationRole::Lead);
    }

    #[test]
    fn test_add_participants() {
        let mut s = make_session();
        s.add_participant("reviewer-1", 3, CollaborationRole::Reviewer)
            .unwrap();
        s.add_participant("reviewer-2", 3, CollaborationRole::Contributor)
            .unwrap();
        assert_eq!(s.participant_count(), 3);
    }

    #[test]
    fn test_start_requires_two() {
        let mut s = make_session();
        assert!(s.start().is_err());
        s.add_participant("other-1", 3, CollaborationRole::Contributor)
            .unwrap();
        assert!(s.start().is_ok());
    }

    #[test]
    fn test_send_message() {
        let mut s = make_session();
        s.add_participant("agent-2", 3, CollaborationRole::Contributor)
            .unwrap();
        s.start().unwrap();

        let msg = CollaborationMessage::new(
            &s.id,
            "lead-1",
            None,
            MessageType::Propose,
            "Use REST API design",
            0.9,
        );
        let id = s.send_message(msg).unwrap();
        assert!(!id.is_empty());
        assert_eq!(s.message_count(), 1);
    }

    #[test]
    fn test_observer_cannot_send() {
        let mut s = make_session();
        s.add_participant("observer-1", 3, CollaborationRole::Observer)
            .unwrap();
        s.start().unwrap();

        let msg = CollaborationMessage::new(
            &s.id,
            "observer-1",
            None,
            MessageType::ShareReasoning,
            "My thoughts",
            0.5,
        );
        assert!(s.send_message(msg).is_err());
    }

    #[test]
    fn test_only_lead_calls_vote() {
        let mut s = make_session();
        s.add_participant("agent-2", 3, CollaborationRole::Contributor)
            .unwrap();
        s.start().unwrap();

        let msg = CollaborationMessage::new(
            &s.id,
            "agent-2",
            None,
            MessageType::CallVote,
            "Let's vote",
            0.9,
        );
        assert!(s.send_message(msg).is_err());
    }

    #[test]
    fn test_vote_lifecycle() {
        let mut s = make_session();
        s.add_participant("a2", 3, CollaborationRole::Contributor)
            .unwrap();
        s.add_participant("a3", 3, CollaborationRole::Contributor)
            .unwrap();
        s.start().unwrap();

        let msg = CollaborationMessage::new(
            &s.id,
            "lead-1",
            None,
            MessageType::Propose,
            "Use microservices",
            0.8,
        );
        let msg_id = s.send_message(msg).unwrap();

        s.call_vote(&msg_id, 0.5, 300).unwrap();
        assert_eq!(s.status, SessionStatus::Voting);

        s.cast_vote("lead-1", VoteChoice::Approve, None).unwrap();
        s.cast_vote("a2", VoteChoice::Approve, None).unwrap();
        s.cast_vote("a3", VoteChoice::Reject, Some("Too complex".into()))
            .unwrap();

        // 2/3 approve with 0.5 majority = passes
        assert_eq!(s.status, SessionStatus::Completed);
        assert!(s.outcome.is_some());
    }

    #[test]
    fn test_majority_passes() {
        let mut s = make_session();
        s.add_participant("a2", 3, CollaborationRole::Contributor)
            .unwrap();
        s.add_participant("a3", 3, CollaborationRole::Contributor)
            .unwrap();
        s.start().unwrap();

        let msg =
            CollaborationMessage::new(&s.id, "lead-1", None, MessageType::Propose, "Plan A", 0.8);
        let msg_id = s.send_message(msg).unwrap();
        s.call_vote(&msg_id, 0.5, 300).unwrap();

        s.cast_vote("lead-1", VoteChoice::Approve, None).unwrap();
        s.cast_vote("a2", VoteChoice::Approve, None).unwrap();
        s.cast_vote("a3", VoteChoice::Reject, None).unwrap();
        assert_eq!(s.status, SessionStatus::Completed);
    }

    #[test]
    fn test_majority_fails() {
        let mut s = make_session();
        s.add_participant("a2", 3, CollaborationRole::Contributor)
            .unwrap();
        s.add_participant("a3", 3, CollaborationRole::Contributor)
            .unwrap();
        s.start().unwrap();

        let msg =
            CollaborationMessage::new(&s.id, "lead-1", None, MessageType::Propose, "Plan B", 0.8);
        let msg_id = s.send_message(msg).unwrap();
        s.call_vote(&msg_id, 0.5, 300).unwrap();

        s.cast_vote("lead-1", VoteChoice::Approve, None).unwrap();
        s.cast_vote("a2", VoteChoice::Reject, None).unwrap();
        s.cast_vote("a3", VoteChoice::Reject, None).unwrap();
        // 1/3 approve = fails, back to active
        assert_eq!(s.status, SessionStatus::Active);
    }

    #[test]
    fn test_duplicate_vote_rejected() {
        let mut s = make_session();
        s.add_participant("a2", 3, CollaborationRole::Contributor)
            .unwrap();
        s.start().unwrap();

        let msg =
            CollaborationMessage::new(&s.id, "lead-1", None, MessageType::Propose, "Plan", 0.8);
        let msg_id = s.send_message(msg).unwrap();
        s.call_vote(&msg_id, 0.5, 300).unwrap();

        s.cast_vote("lead-1", VoteChoice::Approve, None).unwrap();
        assert!(s.cast_vote("lead-1", VoteChoice::Reject, None).is_err());
    }

    #[test]
    fn test_declare_consensus() {
        let mut s = make_session();
        s.add_participant("a2", 3, CollaborationRole::Contributor)
            .unwrap();
        s.start().unwrap();

        s.declare_consensus(
            "lead-1",
            "Go with Plan A",
            vec!["Simple".into(), "Fast".into()],
        )
        .unwrap();
        assert_eq!(s.status, SessionStatus::Completed);
        assert!(s.outcome.is_some());
    }

    #[test]
    fn test_escalate_to_human() {
        let mut s = make_session();
        s.add_participant("a2", 3, CollaborationRole::Contributor)
            .unwrap();
        s.start().unwrap();

        s.escalate("a2", "Can't agree").unwrap();
        assert_eq!(s.status, SessionStatus::Escalated);
    }
}
