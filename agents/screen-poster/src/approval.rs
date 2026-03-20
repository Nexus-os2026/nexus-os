use crate::comments::ReplyDraft;
use crate::composer::DraftPost;
use nexus_sdk::audit::{AuditEvent, AuditTrail, EventType};
use nexus_sdk::errors::AgentError;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApprovalPayload {
    Post(DraftPost),
    Reply(ReplyDraft),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApprovalState {
    Pending,
    Approved,
    Rejected,
    Edited,
    Expired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApprovalDecision {
    Approve,
    Reject,
    Edit { replacement_text: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalTicket {
    pub id: Uuid,
    pub payload: ApprovalPayload,
    pub state: ApprovalState,
    pub created_at: u64,
    pub resolved_at: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovedDraft {
    pub ticket_id: Uuid,
    pub draft: DraftPost,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovedReply {
    pub ticket_id: Uuid,
    pub reply: ReplyDraft,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalError {
    NotFound,
    Pending,
    Rejected,
    Expired,
    WrongPayloadType,
}

impl Display for ApprovalError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ApprovalError::NotFound => write!(f, "approval ticket not found"),
            ApprovalError::Pending => write!(f, "approval ticket still pending"),
            ApprovalError::Rejected => write!(f, "approval ticket was rejected"),
            ApprovalError::Expired => write!(f, "approval ticket expired"),
            ApprovalError::WrongPayloadType => {
                write!(f, "approval payload type does not match requested output")
            }
        }
    }
}

impl std::error::Error for ApprovalError {}

pub trait ApprovalChannel {
    fn send_desktop(&mut self, ticket: &ApprovalTicket) -> Result<(), AgentError>;
    fn send_telegram(&mut self, ticket: &ApprovalTicket) -> Result<(), AgentError>;
}

#[derive(Debug, Default)]
pub struct InMemoryApprovalChannel {
    pub desktop_messages: Vec<Uuid>,
    pub telegram_messages: Vec<Uuid>,
}

impl ApprovalChannel for InMemoryApprovalChannel {
    fn send_desktop(&mut self, ticket: &ApprovalTicket) -> Result<(), AgentError> {
        self.desktop_messages.push(ticket.id);
        Ok(())
    }

    fn send_telegram(&mut self, ticket: &ApprovalTicket) -> Result<(), AgentError> {
        self.telegram_messages.push(ticket.id);
        Ok(())
    }
}

pub struct HumanApprovalGate<C: ApprovalChannel> {
    channel: C,
    tickets: HashMap<Uuid, ApprovalTicket>,
    ttl: Duration,
    audit_trail: AuditTrail,
    agent_id: Uuid,
}

impl<C: ApprovalChannel> HumanApprovalGate<C> {
    pub fn new(channel: C) -> Self {
        Self {
            channel,
            tickets: HashMap::new(),
            ttl: Duration::from_secs(24 * 60 * 60),
            audit_trail: AuditTrail::new(),
            agent_id: Uuid::new_v4(),
        }
    }

    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.ttl = ttl;
        self
    }

    pub fn present_draft(&mut self, draft: DraftPost) -> Result<Uuid, AgentError> {
        self.present(ApprovalPayload::Post(draft), "draft")
    }

    pub fn present_reply(&mut self, reply: ReplyDraft) -> Result<Uuid, AgentError> {
        self.present(ApprovalPayload::Reply(reply), "reply")
    }

    pub fn decide(
        &mut self,
        ticket_id: Uuid,
        decision: ApprovalDecision,
    ) -> Result<(), AgentError> {
        self.expire_if_needed(ticket_id)?;
        let ticket = self
            .tickets
            .get_mut(&ticket_id)
            .ok_or_else(|| AgentError::SupervisorError("approval ticket not found".to_string()))?;

        match decision {
            ApprovalDecision::Approve => {
                ticket.state = ApprovalState::Approved;
            }
            ApprovalDecision::Reject => {
                ticket.state = ApprovalState::Rejected;
            }
            ApprovalDecision::Edit { replacement_text } => {
                match &mut ticket.payload {
                    ApprovalPayload::Post(draft) => {
                        draft.text = replacement_text;
                    }
                    ApprovalPayload::Reply(reply) => {
                        reply.text = replacement_text;
                    }
                }
                ticket.state = ApprovalState::Edited;
            }
        }

        ticket.resolved_at = Some(now_secs());

        if let Err(e) = self.audit_trail.append_event(
            self.agent_id,
            EventType::UserAction,
            json!({
                "step": "approval_decision",
                "ticket_id": ticket_id,
                "state": format!("{:?}", ticket.state),
            }),
        ) {
            tracing::error!("Audit append failed: {e}");
        }
        Ok(())
    }

    pub fn approval_state(&mut self, ticket_id: Uuid) -> Result<ApprovalState, ApprovalError> {
        self.expire_if_needed(ticket_id)
            .map_err(|_| ApprovalError::NotFound)?;
        self.tickets
            .get(&ticket_id)
            .map(|ticket| ticket.state.clone())
            .ok_or(ApprovalError::NotFound)
    }

    pub fn approved_draft(&mut self, ticket_id: Uuid) -> Result<ApprovedDraft, ApprovalError> {
        self.expire_if_needed(ticket_id)
            .map_err(|_| ApprovalError::NotFound)?;
        let ticket = self
            .tickets
            .get(&ticket_id)
            .ok_or(ApprovalError::NotFound)?;

        match ticket.state {
            ApprovalState::Pending => Err(ApprovalError::Pending),
            ApprovalState::Rejected => Err(ApprovalError::Rejected),
            ApprovalState::Expired => Err(ApprovalError::Expired),
            ApprovalState::Approved | ApprovalState::Edited => match &ticket.payload {
                ApprovalPayload::Post(draft) => Ok(ApprovedDraft {
                    ticket_id,
                    draft: draft.clone(),
                }),
                ApprovalPayload::Reply(_) => Err(ApprovalError::WrongPayloadType),
            },
        }
    }

    pub fn approved_reply(&mut self, ticket_id: Uuid) -> Result<ApprovedReply, ApprovalError> {
        self.expire_if_needed(ticket_id)
            .map_err(|_| ApprovalError::NotFound)?;
        let ticket = self
            .tickets
            .get(&ticket_id)
            .ok_or(ApprovalError::NotFound)?;

        match ticket.state {
            ApprovalState::Pending => Err(ApprovalError::Pending),
            ApprovalState::Rejected => Err(ApprovalError::Rejected),
            ApprovalState::Expired => Err(ApprovalError::Expired),
            ApprovalState::Approved | ApprovalState::Edited => match &ticket.payload {
                ApprovalPayload::Reply(reply) => Ok(ApprovedReply {
                    ticket_id,
                    reply: reply.clone(),
                }),
                ApprovalPayload::Post(_) => Err(ApprovalError::WrongPayloadType),
            },
        }
    }

    pub fn channel(&self) -> &C {
        &self.channel
    }

    pub fn audit_events(&self) -> &[AuditEvent] {
        self.audit_trail.events()
    }

    fn present(&mut self, payload: ApprovalPayload, kind: &str) -> Result<Uuid, AgentError> {
        let id = Uuid::new_v4();
        let ticket = ApprovalTicket {
            id,
            payload,
            state: ApprovalState::Pending,
            created_at: now_secs(),
            resolved_at: None,
        };

        self.channel.send_desktop(&ticket)?;
        self.channel.send_telegram(&ticket)?;
        self.tickets.insert(id, ticket.clone());

        if let Err(e) = self.audit_trail.append_event(
            self.agent_id,
            EventType::UserAction,
            json!({
                "step": "present_for_approval",
                "kind": kind,
                "ticket_id": id,
                "state": "Pending",
            }),
        ) {
            tracing::error!("Audit append failed: {e}");
        }
        Ok(id)
    }

    fn expire_if_needed(&mut self, ticket_id: Uuid) -> Result<(), AgentError> {
        let now = now_secs();
        let Some(ticket) = self.tickets.get_mut(&ticket_id) else {
            return Err(AgentError::SupervisorError(
                "approval ticket not found".to_string(),
            ));
        };
        if ticket.state == ApprovalState::Pending
            && now.saturating_sub(ticket.created_at) > self.ttl.as_secs()
        {
            ticket.state = ApprovalState::Expired;
            ticket.resolved_at = Some(now);
            if let Err(e) = self.audit_trail.append_event(
                self.agent_id,
                EventType::UserAction,
                json!({
                    "step": "approval_expired",
                    "ticket_id": ticket_id,
                }),
            ) {
                tracing::error!("Audit append failed: {e}");
            }
        }

        Ok(())
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}
