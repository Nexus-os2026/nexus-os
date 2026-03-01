use crate::WebAgentContext;
use nexus_connectors_core::rate_limit::{RateLimitDecision, RateLimiter};
use nexus_kernel::audit::{AuditTrail, EventType};
use nexus_kernel::errors::AgentError;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tweet {
    pub id: String,
    pub text: String,
    pub author: String,
    pub reply_to: Option<String>,
    pub created_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TweetResult {
    pub tweet_id: String,
    pub posted_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngagementMetrics {
    pub likes: u64,
    pub retweets: u64,
    pub replies: u64,
}

pub struct TwitterConnector {
    pub audit_trail: AuditTrail,
    timeline: Vec<Tweet>,
    metrics: HashMap<String, EngagementMetrics>,
    rate_limiter: RateLimiter,
}

impl TwitterConnector {
    pub fn new() -> Self {
        let limiter = RateLimiter::new();
        limiter.configure("social.x", 60, 60);

        Self {
            audit_trail: AuditTrail::new(),
            timeline: Vec::new(),
            metrics: HashMap::new(),
            rate_limiter: limiter,
        }
    }

    pub fn post_tweet(
        &mut self,
        agent: &mut WebAgentContext,
        text: &str,
    ) -> Result<TweetResult, AgentError> {
        self.ensure_capability(agent, "social.x.post")?;
        self.consume_fuel(agent, 10)?;
        self.check_rate_limit()?;

        let now = current_unix_timestamp();
        let tweet_id = Uuid::new_v4().to_string();
        let tweet = Tweet {
            id: tweet_id.clone(),
            text: text.to_string(),
            author: "nexus-agent".to_string(),
            reply_to: None,
            created_at: now,
        };

        self.timeline.push(tweet);
        self.metrics.insert(
            tweet_id.clone(),
            EngagementMetrics {
                likes: 0,
                retweets: 0,
                replies: 0,
            },
        );

        let _ = self.audit_trail.append_event(
            agent.agent_id,
            EventType::ToolCall,
            json!({
                "event": "social_x_post",
                "tweet_id": tweet_id,
                "length": text.chars().count()
            }),
        );

        Ok(TweetResult {
            tweet_id,
            posted_at: now,
        })
    }

    pub fn get_timeline(
        &mut self,
        agent: &mut WebAgentContext,
        count: usize,
    ) -> Result<Vec<Tweet>, AgentError> {
        self.ensure_capability(agent, "social.x.read")?;
        self.consume_fuel(agent, (count as u64).max(1))?;
        self.check_rate_limit()?;

        let mut tweets = self.timeline.clone();
        tweets.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        if tweets.len() > count {
            tweets.truncate(count);
        }

        Ok(tweets)
    }

    pub fn get_replies(
        &mut self,
        agent: &mut WebAgentContext,
        tweet_id: &str,
    ) -> Result<Vec<Tweet>, AgentError> {
        self.ensure_capability(agent, "social.x.read")?;
        self.consume_fuel(agent, 2)?;
        self.check_rate_limit()?;

        let replies = self
            .timeline
            .iter()
            .filter(|tweet| tweet.reply_to.as_deref() == Some(tweet_id))
            .cloned()
            .collect::<Vec<_>>();

        Ok(replies)
    }

    pub fn get_metrics(
        &mut self,
        agent: &mut WebAgentContext,
        tweet_id: &str,
    ) -> Result<EngagementMetrics, AgentError> {
        self.ensure_capability(agent, "social.x.read")?;
        self.consume_fuel(agent, 2)?;
        self.check_rate_limit()?;

        let metrics =
            self.metrics.get(tweet_id).cloned().ok_or_else(|| {
                AgentError::SupervisorError(format!("tweet '{tweet_id}' not found"))
            })?;

        Ok(metrics)
    }

    fn ensure_capability(
        &self,
        agent: &WebAgentContext,
        capability: &str,
    ) -> Result<(), AgentError> {
        if !agent.has_capability(capability) {
            return Err(AgentError::CapabilityDenied(capability.to_string()));
        }
        Ok(())
    }

    fn consume_fuel(&self, agent: &mut WebAgentContext, amount: u64) -> Result<(), AgentError> {
        if !agent.consume_fuel(amount) {
            return Err(AgentError::FuelExhausted);
        }
        Ok(())
    }

    fn check_rate_limit(&self) -> Result<(), AgentError> {
        match self.rate_limiter.check("social.x") {
            RateLimitDecision::Allowed => Ok(()),
            RateLimitDecision::RateLimited { retry_after_ms } => Err(AgentError::SupervisorError(
                format!("social.x rate limited, retry after {retry_after_ms} ms"),
            )),
        }
    }
}

impl Default for TwitterConnector {
    fn default() -> Self {
        Self::new()
    }
}

fn current_unix_timestamp() -> u64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_secs(),
        Err(_) => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::TwitterConnector;
    use crate::WebAgentContext;
    use nexus_kernel::errors::AgentError;
    use std::collections::HashSet;
    use uuid::Uuid;

    fn capability_set(values: &[&str]) -> HashSet<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn test_tweet_post_governed() {
        let mut connector = TwitterConnector::new();

        let mut allowed = WebAgentContext::new(
            Uuid::new_v4(),
            capability_set(&["social.x.post", "social.x.read"]),
            100,
        );
        let allowed_result = connector.post_tweet(&mut allowed, "hello from NEXUS");
        assert!(allowed_result.is_ok());

        let mut denied =
            WebAgentContext::new(Uuid::new_v4(), capability_set(&["social.x.read"]), 100);
        let denied_result = connector.post_tweet(&mut denied, "should fail");
        assert_eq!(
            denied_result,
            Err(AgentError::CapabilityDenied("social.x.post".to_string()))
        );
    }
}
