//! Frontend integration types.

use std::sync::RwLock;

use crate::consensus::{ConsensusDetector, ConsensusState};
use crate::governance::CollaborationPolicy;
use crate::message::{CollaborationMessage, MessageType};
use crate::patterns::CollaborationPattern;
use crate::protocol::CollaborationProtocol;
use crate::roles::CollaborationRole;
use crate::session::{CollaborationSession, VoteChoice};

/// In-memory state held by the Tauri app.
pub struct CollabState {
    pub protocol: RwLock<CollaborationProtocol>,
    pub policy: CollaborationPolicy,
}

impl Default for CollabState {
    fn default() -> Self {
        let policy = CollaborationPolicy::default();
        Self {
            protocol: RwLock::new(CollaborationProtocol::new(policy.max_active_sessions)),
            policy,
        }
    }
}

// ── Handlers ─────────────────────────────────────────────────────────────────

pub fn collab_create_session(
    state: &CollabState,
    title: &str,
    goal: &str,
    pattern: &str,
    lead_agent_id: &str,
    lead_autonomy: u8,
) -> Result<String, String> {
    let pat = parse_pattern(pattern)?;
    state
        .protocol
        .write()
        .map_err(|e| format!("lock: {e}"))?
        .create_session(title.into(), goal.into(), pat, lead_agent_id, lead_autonomy)
        .map_err(|e| e.to_string())
}

pub fn collab_add_participant(
    state: &CollabState,
    session_id: &str,
    agent_id: &str,
    autonomy: u8,
    role: &str,
) -> Result<(), String> {
    let r = parse_role(role)?;
    let mut proto = state.protocol.write().map_err(|e| format!("lock: {e}"))?;
    let session = proto
        .get_session_mut(session_id)
        .ok_or("Session not found")?;
    session
        .add_participant(agent_id, autonomy, r)
        .map_err(|e| e.to_string())
}

pub fn collab_start(state: &CollabState, session_id: &str) -> Result<(), String> {
    let mut proto = state.protocol.write().map_err(|e| format!("lock: {e}"))?;
    let session = proto
        .get_session_mut(session_id)
        .ok_or("Session not found")?;
    session.start().map_err(|e| e.to_string())
}

pub fn collab_send_message(
    state: &CollabState,
    session_id: &str,
    from_agent: &str,
    to_agent: Option<String>,
    message_type: &str,
    text: &str,
    confidence: f64,
) -> Result<String, String> {
    let mtype = parse_message_type(message_type)?;
    let msg = CollaborationMessage::new(
        session_id,
        from_agent,
        to_agent.as_deref(),
        mtype,
        text,
        confidence,
    );
    let mut proto = state.protocol.write().map_err(|e| format!("lock: {e}"))?;
    let session = proto
        .get_session_mut(session_id)
        .ok_or("Session not found")?;
    session.send_message(msg).map_err(|e| e.to_string())
}

pub fn collab_call_vote(
    state: &CollabState,
    session_id: &str,
    proposal_msg_id: &str,
    majority: f64,
    deadline_secs: u64,
) -> Result<(), String> {
    let mut proto = state.protocol.write().map_err(|e| format!("lock: {e}"))?;
    let session = proto
        .get_session_mut(session_id)
        .ok_or("Session not found")?;
    session
        .call_vote(proposal_msg_id, majority, deadline_secs)
        .map_err(|e| e.to_string())
}

pub fn collab_cast_vote(
    state: &CollabState,
    session_id: &str,
    agent_id: &str,
    vote: &str,
    reason: Option<String>,
) -> Result<(), String> {
    let v = match vote.to_lowercase().as_str() {
        "approve" => VoteChoice::Approve,
        "reject" => VoteChoice::Reject,
        "abstain" => VoteChoice::Abstain,
        other => return Err(format!("Unknown vote: {other}")),
    };
    let mut proto = state.protocol.write().map_err(|e| format!("lock: {e}"))?;
    let session = proto
        .get_session_mut(session_id)
        .ok_or("Session not found")?;
    session
        .cast_vote(agent_id, v, reason)
        .map_err(|e| e.to_string())
}

pub fn collab_declare_consensus(
    state: &CollabState,
    session_id: &str,
    agent_id: &str,
    decision: &str,
    key_points: Vec<String>,
) -> Result<(), String> {
    let mut proto = state.protocol.write().map_err(|e| format!("lock: {e}"))?;
    let session = proto
        .get_session_mut(session_id)
        .ok_or("Session not found")?;
    session
        .declare_consensus(agent_id, decision, key_points)
        .map_err(|e| e.to_string())
}

pub fn collab_detect_consensus(
    state: &CollabState,
    session_id: &str,
) -> Result<ConsensusState, String> {
    let proto = state.protocol.read().map_err(|e| format!("lock: {e}"))?;
    let session = proto.get_session(session_id).ok_or("Session not found")?;
    Ok(ConsensusDetector::detect_consensus(&session.messages))
}

pub fn collab_get_session(
    state: &CollabState,
    session_id: &str,
) -> Result<CollaborationSession, String> {
    let proto = state.protocol.read().map_err(|e| format!("lock: {e}"))?;
    proto
        .get_session(session_id)
        .cloned()
        .ok_or_else(|| "Session not found".into())
}

pub fn collab_list_active(state: &CollabState) -> Result<Vec<CollaborationSession>, String> {
    let proto = state.protocol.read().map_err(|e| format!("lock: {e}"))?;
    Ok(proto.active_sessions().to_vec())
}

pub fn collab_get_policy(state: &CollabState) -> CollaborationPolicy {
    state.policy.clone()
}

pub fn collab_get_patterns() -> Vec<PatternInfo> {
    vec![
        PatternInfo::new(
            "PeerReview",
            "One proposes, others review and critique",
            CollaborationPattern::PeerReview,
        ),
        PatternInfo::new(
            "Debate",
            "Structured pro/con argumentation",
            CollaborationPattern::Debate,
        ),
        PatternInfo::new(
            "Brainstorm",
            "Open ideation without judgment",
            CollaborationPattern::Brainstorm,
        ),
        PatternInfo::new(
            "ExpertPanel",
            "Domain experts contribute perspectives",
            CollaborationPattern::ExpertPanel,
        ),
        PatternInfo::new(
            "Pipeline",
            "Sequential processing chain",
            CollaborationPattern::Pipeline,
        ),
        PatternInfo::new(
            "RedTeam",
            "Adversarial security review",
            CollaborationPattern::RedTeam,
        ),
    ]
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PatternInfo {
    pub id: String,
    pub description: String,
    pub max_participants: usize,
    pub recommended_majority: f64,
    pub roles: Vec<(String, String)>,
}

impl PatternInfo {
    fn new(id: &str, desc: &str, pattern: CollaborationPattern) -> Self {
        Self {
            id: id.into(),
            description: desc.into(),
            max_participants: pattern.max_participants(),
            recommended_majority: pattern.recommended_majority(),
            roles: pattern
                .recommended_roles()
                .into_iter()
                .map(|(name, role)| (name, format!("{:?}", role)))
                .collect(),
        }
    }
}

// ── Parsers ──────────────────────────────────────────────────────────────────

fn parse_pattern(s: &str) -> Result<CollaborationPattern, String> {
    match s {
        "PeerReview" => Ok(CollaborationPattern::PeerReview),
        "Debate" => Ok(CollaborationPattern::Debate),
        "Brainstorm" => Ok(CollaborationPattern::Brainstorm),
        "ExpertPanel" => Ok(CollaborationPattern::ExpertPanel),
        "Pipeline" => Ok(CollaborationPattern::Pipeline),
        "RedTeam" => Ok(CollaborationPattern::RedTeam),
        other => Ok(CollaborationPattern::Custom { name: other.into() }),
    }
}

fn parse_role(s: &str) -> Result<CollaborationRole, String> {
    match s.to_lowercase().as_str() {
        "lead" => Ok(CollaborationRole::Lead),
        "reviewer" => Ok(CollaborationRole::Reviewer),
        "contributor" => Ok(CollaborationRole::Contributor),
        "observer" => Ok(CollaborationRole::Observer),
        other => Ok(CollaborationRole::Expert {
            domain: other.into(),
        }),
    }
}

fn parse_message_type(s: &str) -> Result<MessageType, String> {
    match s {
        "ShareReasoning" => Ok(MessageType::ShareReasoning),
        "Propose" => Ok(MessageType::Propose),
        "Agree" => Ok(MessageType::Agree),
        "Disagree" => Ok(MessageType::Disagree),
        "Question" => Ok(MessageType::Question),
        "Answer" => Ok(MessageType::Answer),
        "RaiseRisk" => Ok(MessageType::RaiseRisk),
        "AddContext" => Ok(MessageType::AddContext),
        "CallVote" => Ok(MessageType::CallVote),
        "Vote" => Ok(MessageType::Vote),
        "DeclareConsensus" => Ok(MessageType::DeclareConsensus),
        "EscalateToHuman" => Ok(MessageType::EscalateToHuman),
        other => Err(format!("Unknown message type: {other}")),
    }
}
