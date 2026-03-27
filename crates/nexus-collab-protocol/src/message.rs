use serde::{Deserialize, Serialize};

/// A structured message between collaborating agents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollaborationMessage {
    pub id: String,
    pub session_id: String,
    pub from_agent: String,
    pub to_agent: Option<String>,
    pub message_type: MessageType,
    pub content: MessageContent,
    pub timestamp: u64,
    pub acknowledged: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageType {
    ShareReasoning,
    Propose,
    Agree,
    Disagree,
    Question,
    Answer,
    RaiseRisk,
    AddContext,
    CallVote,
    Vote,
    DeclareConsensus,
    EscalateToHuman,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageContent {
    pub text: String,
    pub data: Option<serde_json::Value>,
    pub confidence: f64,
    pub references: Vec<String>,
    pub reasoning: Option<String>,
}

impl CollaborationMessage {
    pub fn new(
        session_id: &str,
        from_agent: &str,
        to_agent: Option<&str>,
        message_type: MessageType,
        text: &str,
        confidence: f64,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            session_id: session_id.into(),
            from_agent: from_agent.into(),
            to_agent: to_agent.map(|s| s.into()),
            message_type,
            content: MessageContent {
                text: text.into(),
                data: None,
                confidence: confidence.clamp(0.0, 1.0),
                references: Vec::new(),
                reasoning: None,
            },
            timestamp: epoch_now(),
            acknowledged: false,
        }
    }

    pub fn with_data(mut self, data: serde_json::Value) -> Self {
        self.content.data = Some(data);
        self
    }

    pub fn with_reasoning(mut self, reasoning: &str) -> Self {
        self.content.reasoning = Some(reasoning.into());
        self
    }

    pub fn with_references(mut self, refs: Vec<String>) -> Self {
        self.content.references = refs;
        self
    }

    pub fn is_broadcast(&self) -> bool {
        self.to_agent.is_none()
    }
}

fn epoch_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
