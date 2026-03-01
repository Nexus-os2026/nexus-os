use nexus_kernel::errors::AgentError;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

pub type MessageId = String;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RichMessage {
    pub text: String,
    pub buttons: Vec<String>,
    pub images: Vec<String>,
    pub attachments: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IncomingMessage {
    pub chat_id: String,
    pub sender_id: String,
    pub text: String,
    pub voice_note_url: Option<String>,
    pub timestamp: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RateLimitConfig {
    pub max_messages: usize,
    pub window_seconds: u64,
    pub quality_tier: Option<String>,
}

pub struct IncomingMessageStream {
    queue: VecDeque<IncomingMessage>,
}

impl IncomingMessageStream {
    pub fn new(messages: Vec<IncomingMessage>) -> Self {
        Self {
            queue: messages.into_iter().collect(),
        }
    }

    pub fn empty() -> Self {
        Self {
            queue: VecDeque::new(),
        }
    }
}

impl Iterator for IncomingMessageStream {
    type Item = IncomingMessage;

    fn next(&mut self) -> Option<Self::Item> {
        self.queue.pop_front()
    }
}

pub trait MessagingPlatform: Send + Sync {
    fn send_message(&mut self, chat_id: &str, text: &str) -> Result<MessageId, AgentError>;
    fn send_rich_message(
        &mut self,
        chat_id: &str,
        message: RichMessage,
    ) -> Result<MessageId, AgentError>;
    fn receive_messages(&mut self) -> IncomingMessageStream;
    fn platform_name(&self) -> &str;
    fn rate_limit(&self) -> RateLimitConfig;
}
