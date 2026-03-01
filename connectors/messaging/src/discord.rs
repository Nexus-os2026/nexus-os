use crate::messaging::{
    IncomingMessage, IncomingMessageStream, MessageId, MessagingPlatform, RateLimitConfig,
    RichMessage,
};
use nexus_connectors_core::rate_limit::{RateLimitDecision, RateLimiter};
use nexus_kernel::errors::AgentError;
use uuid::Uuid;

pub struct DiscordAdapter {
    incoming: Vec<IncomingMessage>,
    limiter: RateLimiter,
}

impl DiscordAdapter {
    pub fn new() -> Self {
        let limiter = RateLimiter::new();
        limiter.configure("discord", 5, 1);

        Self {
            incoming: Vec::new(),
            limiter,
        }
    }

    pub fn push_incoming(&mut self, message: IncomingMessage) {
        self.incoming.push(message);
    }

    fn check_rate_limit(&self) -> Result<(), AgentError> {
        match self.limiter.check("discord") {
            RateLimitDecision::Allowed => Ok(()),
            RateLimitDecision::RateLimited { retry_after_ms } => Err(AgentError::SupervisorError(
                format!("discord rate limit exceeded; retry after {retry_after_ms} ms"),
            )),
        }
    }
}

impl Default for DiscordAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl MessagingPlatform for DiscordAdapter {
    fn send_message(&mut self, chat_id: &str, text: &str) -> Result<MessageId, AgentError> {
        self.check_rate_limit()?;
        if chat_id.is_empty() || text.is_empty() {
            return Err(AgentError::SupervisorError(
                "discord message requires non-empty chat_id and text".to_string(),
            ));
        }
        Ok(format!("dc-{}", Uuid::new_v4()))
    }

    fn send_rich_message(
        &mut self,
        chat_id: &str,
        message: RichMessage,
    ) -> Result<MessageId, AgentError> {
        self.send_message(chat_id, message.text.as_str())
    }

    fn receive_messages(&mut self) -> IncomingMessageStream {
        let drained = self.incoming.drain(..).collect::<Vec<_>>();
        IncomingMessageStream::new(drained)
    }

    fn platform_name(&self) -> &str {
        "discord"
    }

    fn rate_limit(&self) -> RateLimitConfig {
        RateLimitConfig {
            max_messages: 5,
            window_seconds: 1,
            quality_tier: Some("bot-standard".to_string()),
        }
    }
}
