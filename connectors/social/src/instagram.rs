use nexus_connectors_core::connector::{Connector, HealthStatus, RetryPolicy};
use nexus_connectors_core::idempotency::IdempotencyManager;
use nexus_kernel::audit::{AuditTrail, EventType};
use nexus_kernel::errors::AgentError;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstagramPublishResult {
    pub media_id: String,
    pub request_id: String,
    pub cached: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstagramMetrics {
    pub likes: u64,
    pub comments: u64,
    pub saves: u64,
}

pub struct InstagramConnector {
    idempotency: IdempotencyManager,
    pub audit_trail: AuditTrail,
    published_count: usize,
}

impl InstagramConnector {
    pub fn new() -> Self {
        Self {
            idempotency: IdempotencyManager::new(3_600),
            audit_trail: AuditTrail::new(),
            published_count: 0,
        }
    }

    pub fn publish(
        &mut self,
        caption: &str,
        request_id: &str,
    ) -> Result<InstagramPublishResult, AgentError> {
        if caption.trim().is_empty() {
            return Err(AgentError::SupervisorError(
                "instagram caption cannot be empty".to_string(),
            ));
        }

        if let Some(cached) = self.idempotency.check_duplicate(request_id) {
            return Ok(InstagramPublishResult {
                media_id: cached,
                request_id: request_id.to_string(),
                cached: true,
            });
        }

        let media_id = format!("ig-{}", Uuid::new_v4());
        self.idempotency
            .record_completion(request_id, media_id.clone());
        self.published_count += 1;

        let _ = self.audit_trail.append_event(
            Uuid::nil(),
            EventType::ToolCall,
            json!({
                "event": "instagram_publish",
                "request_id": request_id,
                "media_id": media_id
            }),
        );

        Ok(InstagramPublishResult {
            media_id,
            request_id: request_id.to_string(),
            cached: false,
        })
    }

    pub fn publish_story(
        &mut self,
        story_text: &str,
        request_id: &str,
    ) -> Result<InstagramPublishResult, AgentError> {
        self.publish(story_text, request_id)
    }

    pub fn metrics(&self) -> InstagramMetrics {
        InstagramMetrics {
            likes: (self.published_count as u64) * 200,
            comments: (self.published_count as u64) * 30,
            saves: (self.published_count as u64) * 15,
        }
    }

    pub fn published_count(&self) -> usize {
        self.published_count
    }
}

impl Default for InstagramConnector {
    fn default() -> Self {
        Self::new()
    }
}

impl Connector for InstagramConnector {
    fn id(&self) -> &str {
        "instagram"
    }

    fn name(&self) -> &str {
        "Instagram Connector"
    }

    fn required_capabilities(&self) -> Vec<String> {
        vec![
            "social.instagram.post".to_string(),
            "social.instagram.read".to_string(),
        ]
    }

    fn health_check(&self) -> Result<HealthStatus, AgentError> {
        Ok(HealthStatus::Healthy)
    }

    fn retry_policy(&self) -> RetryPolicy {
        RetryPolicy {
            max_retries: 3,
            backoff_ms: 200,
            backoff_multiplier: 2.0,
        }
    }

    fn degrade_gracefully(&self) -> bool {
        true
    }
}
