use crate::messaging::{IncomingMessage, IncomingMessageStream, MessageId, MessagingPlatform, RateLimitConfig, RichMessage};
use nexus_connectors_core::rate_limit::{RateLimitDecision, RateLimiter};
use nexus_kernel::errors::AgentError;
use std::sync::Arc;
use uuid::Uuid;

/// WhatsApp adapter for specialized workflow actions, per Jan 2026 policy framing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhatsAppQualityTier {
    Low,
    Medium,
    High,
}

pub struct WhatsAppAdapter {
    incoming: Vec<IncomingMessage>,
    limiter: RateLimiter,
    quality_tier: WhatsAppQualityTier,
}

impl WhatsAppAdapter {
    pub fn new(quality_tier: WhatsAppQualityTier) -> Self {
        Self::with_clock(quality_tier, None)
    }

    pub fn with_clock(
        quality_tier: WhatsAppQualityTier,
        clock: Option<Arc<dyn Fn() -> u64 + Send + Sync>>,
    ) -> Self {
        let limiter = match clock {
            Some(clock_fn) => RateLimiter::with_clock(clock_fn),
            None => RateLimiter::new(),
        };

        let (max_messages, window_seconds) = match quality_tier {
            WhatsAppQualityTier::Low => (1, 2),
            WhatsAppQualityTier::Medium => (3, 2),
            WhatsAppQualityTier::High => (10, 2),
        };
        limiter.configure("whatsapp", max_messages, window_seconds);

        Self {
            incoming: Vec::new(),
            limiter,
            quality_tier,
        }
    }

    pub fn push_incoming(&mut self, message: IncomingMessage) {
        self.incoming.push(message);
    }

    fn check_rate_limit(&self) -> Result<(), AgentError> {
        match self.limiter.check("whatsapp") {
            RateLimitDecision::Allowed => Ok(()),
            RateLimitDecision::RateLimited { retry_after_ms } => Err(AgentError::SupervisorError(
                format!(
                    "whatsapp quality tier {:?} rate limit exceeded; retry after {retry_after_ms} ms",
                    self.quality_tier
                ),
            )),
        }
    }
}

impl MessagingPlatform for WhatsAppAdapter {
    fn send_message(&mut self, chat_id: &str, text: &str) -> Result<MessageId, AgentError> {
        self.check_rate_limit()?;
        if chat_id.is_empty() || text.is_empty() {
            return Err(AgentError::SupervisorError(
                "whatsapp message requires non-empty chat_id and text".to_string(),
            ));
        }
        Ok(format!("wa-{}", Uuid::new_v4()))
    }

    fn send_rich_message(&mut self, chat_id: &str, message: RichMessage) -> Result<MessageId, AgentError> {
        self.send_message(chat_id, message.text.as_str())
    }

    fn receive_messages(&mut self) -> IncomingMessageStream {
        let drained = self.incoming.drain(..).collect::<Vec<_>>();
        IncomingMessageStream::new(drained)
    }

    fn platform_name(&self) -> &str {
        "whatsapp"
    }

    fn rate_limit(&self) -> RateLimitConfig {
        let (max_messages, window_seconds, tier) = match self.quality_tier {
            WhatsAppQualityTier::Low => (1, 2, "low"),
            WhatsAppQualityTier::Medium => (3, 2, "medium"),
            WhatsAppQualityTier::High => (10, 2, "high"),
        };

        RateLimitConfig {
            max_messages,
            window_seconds,
            quality_tier: Some(tier.to_string()),
        }
    }
}
