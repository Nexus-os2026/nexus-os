use crate::WebAgentContext;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use hmac::{Hmac, Mac};
use nexus_connectors_core::idempotency::IdempotencyManager;
use nexus_connectors_core::rate_limit::{RateLimitDecision, RateLimiter};
use nexus_kernel::audit::{AuditTrail, EventType};
use nexus_kernel::errors::AgentError;
use nexus_kernel::secrets::SecretError;
use percent_encoding::{utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use reqwest::blocking::{Client, Response};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha1::Sha1;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const OAUTH_ENCODE_SET: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

const X_UPDATE_URL: &str = "https://api.twitter.com/1.1/statuses/update.json";
const X_HOME_TIMELINE_URL: &str = "https://api.twitter.com/1.1/statuses/home_timeline.json";
const X_MENTIONS_URL: &str = "https://api.twitter.com/1.1/statuses/mentions_timeline.json";

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

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RateLimitStatus {
    pub limit: Option<u64>,
    pub remaining: Option<u64>,
    pub reset_epoch_seconds: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TwitterCredentials {
    consumer_key: String,
    consumer_secret: String,
    access_token: String,
    access_secret: String,
}

pub struct TwitterConnector {
    pub audit_trail: AuditTrail,
    timeline: Vec<Tweet>,
    metrics: HashMap<String, EngagementMetrics>,
    rate_limiter: RateLimiter,
    credentials: Option<TwitterCredentials>,
    client: Client,
    pub last_rate_limit: RateLimitStatus,
    /// Bug AF: persistent dedup for retried publishes. `IdempotencyManager`
    /// is in-memory by default (`::new` path) and SQLite-backed when
    /// constructed via `::with_db`. Retries that present the same
    /// `request_id` short-circuit the API call and replay the cached
    /// `tweet_id`, closing Bug V's documented duplicate-tweet-on-retry
    /// gap.
    idempotency: IdempotencyManager,
}

impl TwitterConnector {
    pub fn new() -> Self {
        Self::build(IdempotencyManager::new(3_600))
    }

    /// Bug AF: persistent variant. Same as `::new` but the
    /// idempotency cache survives process restart via SQLite. Pass an
    /// `Arc<NexusDatabase>` shared with the rest of the app.
    pub fn with_db(db: Arc<nexus_persistence::NexusDatabase>) -> Self {
        Self::build(IdempotencyManager::with_db(3_600, db))
    }

    fn build(idempotency: IdempotencyManager) -> Self {
        let limiter = RateLimiter::new();
        limiter.configure("social.x", 60, 60);

        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| Client::new());

        Self {
            audit_trail: AuditTrail::new(),
            timeline: Vec::new(),
            metrics: HashMap::new(),
            rate_limiter: limiter,
            credentials: load_twitter_credentials().ok().flatten(),
            client,
            last_rate_limit: RateLimitStatus::default(),
            idempotency,
        }
    }

    pub fn post_tweet(
        &mut self,
        agent: &mut WebAgentContext,
        text: &str,
    ) -> Result<TweetResult, AgentError> {
        self.post_status_update(agent, text)
    }

    /// Bug AF: idempotent publish. Returns the cached `TweetResult` if
    /// `request_id` was already used (short-circuits the API call).
    /// Otherwise calls `post_status_update` and records the result
    /// against `request_id` so a retry with the same id replays.
    /// Stable cross-restart when the connector was built via `::with_db`.
    pub fn post_status_update_idempotent(
        &mut self,
        agent: &mut WebAgentContext,
        text: &str,
        request_id: &str,
    ) -> Result<TweetResult, AgentError> {
        if let Some(cached) = self.idempotency.check_duplicate(request_id) {
            // Cached value is the JSON-encoded TweetResult. Parse-or-
            // refuse: a corrupt cached row should be a hard failure
            // rather than a silent re-publish, since a re-publish is
            // exactly what idempotency is supposed to prevent.
            let result: TweetResult = serde_json::from_str(&cached).map_err(|e| {
                AgentError::SupervisorError(format!(
                    "idempotency cache decode failed for request_id={request_id}: {e}"
                ))
            })?;
            return Ok(result);
        }
        let result = self.post_status_update(agent, text)?;
        match serde_json::to_string(&result) {
            Ok(encoded) => self.idempotency.record_completion(request_id, encoded),
            Err(e) => {
                // The publish landed; only the audit cache write
                // failed. Same defensive policy as the AL/AH
                // tracing::error path: log and continue.
                eprintln!("idempotency cache encode failed for request_id={request_id}: {e}");
            }
        }
        Ok(result)
    }

    pub fn post_status_update(
        &mut self,
        agent: &mut WebAgentContext,
        text: &str,
    ) -> Result<TweetResult, AgentError> {
        self.ensure_capability(agent, "social.x.post")?;
        self.consume_fuel(agent, 10)?;
        self.check_rate_limit()?;

        if self.credentials.is_none() {
            return self.post_tweet_mock(agent, text);
        }

        let params = vec![("status".to_string(), text.to_string())];
        let response = self.send_oauth_request("POST", X_UPDATE_URL, &params, true)?;
        self.capture_rate_limit(&response);

        let payload = response.json::<TwitterStatusResponse>().map_err(|error| {
            AgentError::SupervisorError(format!("x update parse failed: {error}"))
        })?;

        let tweet_id = payload.id_str.unwrap_or_else(|| Uuid::new_v4().to_string());
        let now = current_unix_timestamp();
        self.audit_trail.append_event(
            agent.agent_id,
            EventType::ToolCall,
            json!({
                "event": "social_x_post",
                "tweet_id": tweet_id,
                "length": text.chars().count()
            }),
        )?;

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
        self.get_home_timeline(agent, count)
    }

    pub fn get_home_timeline(
        &mut self,
        agent: &mut WebAgentContext,
        count: usize,
    ) -> Result<Vec<Tweet>, AgentError> {
        self.ensure_capability(agent, "social.x.read")?;
        self.consume_fuel(agent, (count as u64).max(1))?;
        self.check_rate_limit()?;

        if self.credentials.is_none() {
            return Ok(self.mock_home_timeline(count));
        }

        let params = vec![("count".to_string(), count.to_string())];
        let response = self.send_oauth_request("GET", X_HOME_TIMELINE_URL, &params, false)?;
        self.capture_rate_limit(&response);
        let statuses = response
            .json::<Vec<TwitterStatusResponse>>()
            .map_err(|error| {
                AgentError::SupervisorError(format!("x timeline parse failed: {error}"))
            })?;

        Ok(statuses.into_iter().map(status_to_tweet).collect())
    }

    pub fn get_replies(
        &mut self,
        agent: &mut WebAgentContext,
        tweet_id: &str,
    ) -> Result<Vec<Tweet>, AgentError> {
        let mentions = self.get_mentions_timeline(agent, 50)?;
        Ok(mentions
            .into_iter()
            .filter(|tweet| tweet.reply_to.as_deref() == Some(tweet_id))
            .collect())
    }

    pub fn get_mentions_timeline(
        &mut self,
        agent: &mut WebAgentContext,
        count: usize,
    ) -> Result<Vec<Tweet>, AgentError> {
        self.ensure_capability(agent, "social.x.read")?;
        self.consume_fuel(agent, (count as u64).max(1))?;
        self.check_rate_limit()?;

        if self.credentials.is_none() {
            return Ok(Vec::new());
        }

        let params = vec![("count".to_string(), count.to_string())];
        let response = self.send_oauth_request("GET", X_MENTIONS_URL, &params, false)?;
        self.capture_rate_limit(&response);
        let statuses = response
            .json::<Vec<TwitterStatusResponse>>()
            .map_err(|error| {
                AgentError::SupervisorError(format!("x mentions parse failed: {error}"))
            })?;

        Ok(statuses.into_iter().map(status_to_tweet).collect())
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

    fn post_tweet_mock(
        &mut self,
        agent: &mut WebAgentContext,
        text: &str,
    ) -> Result<TweetResult, AgentError> {
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

        self.audit_trail.append_event(
            agent.agent_id,
            EventType::ToolCall,
            json!({
                "event": "social_x_post",
                "tweet_id": tweet_id,
                "length": text.chars().count(),
                "mode": "mock"
            }),
        )?;

        Ok(TweetResult {
            tweet_id,
            posted_at: now,
        })
    }

    fn mock_home_timeline(&self, count: usize) -> Vec<Tweet> {
        let mut tweets = self.timeline.clone();
        tweets.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        if tweets.len() > count {
            tweets.truncate(count);
        }
        tweets
    }

    fn send_oauth_request(
        &self,
        method: &str,
        url: &str,
        params: &[(String, String)],
        form_encoded: bool,
    ) -> Result<Response, AgentError> {
        let Some(creds) = self.credentials.as_ref() else {
            return Err(AgentError::SupervisorError(
                "X credentials are not configured".to_string(),
            ));
        };

        let auth = build_oauth_header(method, url, params, creds)?;
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(auth.as_str()).map_err(|error| {
                AgentError::SupervisorError(format!("invalid authorization header: {error}"))
            })?,
        );

        let request = self.client.request(
            reqwest::Method::from_bytes(method.as_bytes()).map_err(|error| {
                AgentError::SupervisorError(format!("invalid HTTP method '{method}': {error}"))
            })?,
            url,
        );

        let request = if form_encoded {
            request.headers(headers).form(params)
        } else {
            request.headers(headers).query(params)
        };

        let response = request
            .send()
            .map_err(|error| AgentError::SupervisorError(format!("x request failed: {error}")))?;
        if !response.status().is_success() {
            return Err(AgentError::SupervisorError(format!(
                "x request failed with status {}",
                response.status()
            )));
        }
        Ok(response)
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

    fn capture_rate_limit(&mut self, response: &Response) {
        self.last_rate_limit = RateLimitStatus {
            limit: response
                .headers()
                .get("x-rate-limit-limit")
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u64>().ok()),
            remaining: response
                .headers()
                .get("x-rate-limit-remaining")
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u64>().ok()),
            reset_epoch_seconds: response
                .headers()
                .get("x-rate-limit-reset")
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u64>().ok()),
        };
    }
}

impl Default for TwitterConnector {
    fn default() -> Self {
        Self::new()
    }
}

fn load_twitter_credentials() -> Result<Option<TwitterCredentials>, AgentError> {
    // Bug AK Commit 2: SocialConfig fields no longer live in
    // ~/.nexus/config.toml after migration. Resolve via the kernel
    // SecretsFacade singleton installed by
    // `kernel::startup::run_migrations`. If the facade has not yet
    // been installed (e.g., a test that bypassed startup wiring),
    // treat as "no creds" — Ok(None) — matching the prior empty-
    // field semantics. See ADR 0004 Implementation phasing.
    let Some(facade) = nexus_kernel::secrets::global::try_facade() else {
        return Ok(None);
    };

    let read = |name: &str| -> Result<Option<String>, AgentError> {
        match facade.get_secret("social", name) {
            Ok(secret) => Ok(Some(secret.value.to_string().trim().to_string())),
            Err(SecretError::NotFound) => Ok(None),
            Err(e) => Err(AgentError::SupervisorError(format!(
                "vault read failed for social.{name}: {e}"
            ))),
        }
    };

    let key = read("x_consumer_key")?;
    let secret = read("x_consumer_secret")?;
    let token = read("x_access_token")?;
    let token_secret = read("x_access_token_secret")?;

    let (Some(key), Some(secret), Some(token), Some(token_secret)) =
        (key, secret, token, token_secret)
    else {
        return Ok(None);
    };
    if key.is_empty() || secret.is_empty() || token.is_empty() || token_secret.is_empty() {
        return Ok(None);
    }

    Ok(Some(TwitterCredentials {
        consumer_key: key,
        consumer_secret: secret,
        access_token: token,
        access_secret: token_secret,
    }))
}

fn build_oauth_header(
    method: &str,
    url: &str,
    request_params: &[(String, String)],
    creds: &TwitterCredentials,
) -> Result<String, AgentError> {
    let nonce = generate_nonce();
    let timestamp = current_unix_timestamp().to_string();

    let mut oauth_params = vec![
        ("oauth_consumer_key".to_string(), creds.consumer_key.clone()),
        ("oauth_nonce".to_string(), nonce),
        (
            "oauth_signature_method".to_string(),
            "HMAC-SHA1".to_string(),
        ),
        ("oauth_timestamp".to_string(), timestamp),
        ("oauth_token".to_string(), creds.access_token.clone()),
        ("oauth_version".to_string(), "1.0".to_string()),
    ];

    let mut all_params = oauth_params.clone();
    all_params.extend_from_slice(request_params);
    all_params.sort_by(|a, b| {
        if a.0 == b.0 {
            a.1.cmp(&b.1)
        } else {
            a.0.cmp(&b.0)
        }
    });

    let normalized = all_params
        .iter()
        .map(|(k, v)| format!("{}={}", oauth_encode(k), oauth_encode(v)))
        .collect::<Vec<_>>()
        .join("&");

    let base = format!(
        "{}&{}&{}",
        method.to_uppercase(),
        oauth_encode(url),
        oauth_encode(normalized.as_str())
    );
    let signing_key = format!(
        "{}&{}",
        oauth_encode(creds.consumer_secret.as_str()),
        oauth_encode(creds.access_secret.as_str())
    );

    let mut mac = Hmac::<Sha1>::new_from_slice(signing_key.as_bytes())
        .map_err(|error| AgentError::SupervisorError(format!("oauth hmac init failed: {error}")))?;
    mac.update(base.as_bytes());
    let signature = STANDARD.encode(mac.finalize().into_bytes());

    oauth_params.push(("oauth_signature".to_string(), signature));
    oauth_params.sort_by(|a, b| a.0.cmp(&b.0));

    let value = oauth_params
        .iter()
        .map(|(k, v)| format!(r#"{}="{}""#, oauth_encode(k), oauth_encode(v)))
        .collect::<Vec<_>>()
        .join(", ");
    Ok(format!("OAuth {value}"))
}

fn status_to_tweet(status: TwitterStatusResponse) -> Tweet {
    Tweet {
        id: status.id_str.unwrap_or_else(|| Uuid::new_v4().to_string()),
        text: status.text.unwrap_or_default(),
        author: status
            .user
            .and_then(|user| user.screen_name)
            .unwrap_or_else(|| "unknown".to_string()),
        reply_to: status.in_reply_to_status_id_str,
        created_at: current_unix_timestamp(),
    }
}

fn oauth_encode(value: &str) -> String {
    utf8_percent_encode(value, OAUTH_ENCODE_SET).to_string()
}

fn generate_nonce() -> String {
    thread_rng()
        .sample_iter(&Alphanumeric)
        .take(16)
        .map(char::from)
        .collect::<String>()
}

fn current_unix_timestamp() -> u64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_secs(),
        Err(_) => 0,
    }
}

#[derive(Debug, Deserialize)]
struct TwitterUser {
    screen_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TwitterStatusResponse {
    id_str: Option<String>,
    text: Option<String>,
    in_reply_to_status_id_str: Option<String>,
    user: Option<TwitterUser>,
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

    /// Bug AF: a second call with the same `request_id` must return
    /// the cached `TweetResult` without producing a second mock-mode
    /// publish (verified via timeline length).
    #[test]
    fn idempotent_publish_short_circuits_second_call() {
        let mut connector = TwitterConnector::new();
        let mut agent = WebAgentContext::new(
            Uuid::new_v4(),
            capability_set(&["social.x.post", "social.x.read"]),
            1_000,
        );
        let first = connector
            .post_status_update_idempotent(&mut agent, "hello from AF", "req-af-1")
            .expect("first call ok");
        // Second call with same request_id must return the cached
        // TweetResult and must NOT push a second tweet onto the
        // mock timeline.
        let timeline_len_before = connector.timeline.len();
        let second = connector
            .post_status_update_idempotent(&mut agent, "hello from AF", "req-af-1")
            .expect("second call ok");
        assert_eq!(first, second, "cached result must equal first result");
        assert_eq!(
            connector.timeline.len(),
            timeline_len_before,
            "second idempotent call must not produce a second mock publish"
        );
    }
}
