//! OpenAI swarm provider.
//!
//! Models: gpt-4o-mini, gpt-4o, o1. API key from keyring entry
//! `nexus.openai.api_key`. Missing key → provider is `Unhealthy` (no panic).

use crate::events::{ProviderHealth, ProviderHealthStatus};
use crate::profile::{CostClass, PrivacyClass, ReasoningTier};
use crate::provider::{
    InvokeRequest, InvokeResponse, ModelDescriptor, Provider, ProviderCapabilities, ProviderError,
};
use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use std::time::{Duration, Instant};

const KEYRING_SERVICE: &str = "nexus.openai";
const KEYRING_USER: &str = "api_key";
const DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";

pub struct OpenAiSwarmProvider {
    base_url: String,
    client: Client,
    /// Optional override for tests — bypasses keyring.
    key_override: Option<String>,
}

impl OpenAiSwarmProvider {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .unwrap_or_else(|_| Client::new());
        Self {
            base_url: DEFAULT_BASE_URL.into(),
            client,
            key_override: None,
        }
    }

    /// Test helper — point at a mock server with a dummy key.
    pub fn with_base_and_key(base_url: impl Into<String>, key: impl Into<String>) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .unwrap_or_else(|_| Client::new());
        Self {
            base_url: base_url.into(),
            client,
            key_override: Some(key.into()),
        }
    }

    fn api_key(&self) -> Result<String, ProviderError> {
        if let Some(k) = &self.key_override {
            return Ok(k.clone());
        }
        let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
            .map_err(|e| ProviderError::NotConfigured(format!("openai keyring: {e}")))?;
        entry
            .get_password()
            .map_err(|_| ProviderError::NotConfigured("openai api key missing from keyring".into()))
    }
}

impl Default for OpenAiSwarmProvider {
    fn default() -> Self {
        Self::new()
    }
}

fn known_models() -> Vec<ModelDescriptor> {
    vec![
        ModelDescriptor {
            id: "gpt-4o-mini".into(),
            param_count_b: None,
            tier: ReasoningTier::Medium,
            context_window: 128_000,
        },
        ModelDescriptor {
            id: "gpt-4o".into(),
            param_count_b: None,
            tier: ReasoningTier::Heavy,
            context_window: 128_000,
        },
        ModelDescriptor {
            id: "o1".into(),
            param_count_b: None,
            tier: ReasoningTier::Expert,
            context_window: 128_000,
        },
    ]
}

#[async_trait]
impl Provider for OpenAiSwarmProvider {
    fn id(&self) -> &str {
        "openai"
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            models: known_models(),
            supports_tool_use: true,
            supports_streaming: true,
            max_context: 128_000,
            cost_class: CostClass::Standard,
            privacy_class: PrivacyClass::Public,
        }
    }

    async fn health_check(&self) -> ProviderHealth {
        let start = Instant::now();
        let key = match self.api_key() {
            Ok(k) => k,
            Err(e) => {
                return ProviderHealth {
                    provider_id: "openai".into(),
                    status: ProviderHealthStatus::Unhealthy,
                    latency_ms: None,
                    models: vec![],
                    notes: e.to_string(),
                    checked_at_secs: chrono::Utc::now().timestamp(),
                }
            }
        };
        let url = format!("{}/models", self.base_url.trim_end_matches('/'));
        let resp = self.client.get(&url).bearer_auth(&key).send().await;
        match resp {
            Ok(r) if r.status().is_success() => ProviderHealth {
                provider_id: "openai".into(),
                status: ProviderHealthStatus::Ok,
                latency_ms: Some(start.elapsed().as_millis() as u64),
                models: known_models().into_iter().map(|m| m.id).collect(),
                notes: String::new(),
                checked_at_secs: chrono::Utc::now().timestamp(),
            },
            Ok(r) => ProviderHealth {
                provider_id: "openai".into(),
                status: ProviderHealthStatus::Unhealthy,
                latency_ms: Some(start.elapsed().as_millis() as u64),
                models: vec![],
                notes: format!("http {}", r.status()),
                checked_at_secs: chrono::Utc::now().timestamp(),
            },
            Err(e) => ProviderHealth {
                provider_id: "openai".into(),
                status: ProviderHealthStatus::Unhealthy,
                latency_ms: None,
                models: vec![],
                notes: e.to_string(),
                checked_at_secs: chrono::Utc::now().timestamp(),
            },
        }
    }

    async fn invoke(&self, req: InvokeRequest) -> Result<InvokeResponse, ProviderError> {
        let key = self.api_key()?;
        #[derive(serde::Serialize)]
        struct ChatReq<'a> {
            model: &'a str,
            max_tokens: u32,
            temperature: f32,
            messages: Vec<ChatMsg<'a>>,
        }
        #[derive(serde::Serialize)]
        struct ChatMsg<'a> {
            role: &'a str,
            content: &'a str,
        }
        #[derive(Deserialize)]
        struct ChatResp {
            choices: Vec<Choice>,
            #[serde(default)]
            usage: Option<Usage>,
        }
        #[derive(Deserialize)]
        struct Choice {
            message: ChatMsgResp,
        }
        #[derive(Deserialize)]
        struct ChatMsgResp {
            content: String,
        }
        #[derive(Deserialize)]
        struct Usage {
            prompt_tokens: u32,
            completion_tokens: u32,
        }

        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
        let body = ChatReq {
            model: &req.model_id,
            max_tokens: req.max_tokens,
            temperature: req.temperature.unwrap_or(0.2),
            messages: vec![ChatMsg {
                role: "user",
                content: &req.prompt,
            }],
        };
        let start = Instant::now();
        let resp = self
            .client
            .post(&url)
            .bearer_auth(&key)
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::Transport("openai".into(), e.to_string()))?;

        let status = resp.status();
        if status == 401 {
            return Err(ProviderError::AuthFailed("openai".into()));
        }
        if status == 429 {
            return Err(ProviderError::Http(
                "openai".into(),
                429,
                "rate limited".into(),
            ));
        }
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(ProviderError::Http("openai".into(), status.as_u16(), text));
        }
        let parsed: ChatResp = resp
            .json()
            .await
            .map_err(|e| ProviderError::Malformed("openai".into(), e.to_string()))?;
        let text = parsed
            .choices
            .into_iter()
            .next()
            .map(|c| c.message.content)
            .unwrap_or_default();
        let (tin, tout) = parsed
            .usage
            .map(|u| (u.prompt_tokens, u.completion_tokens))
            .unwrap_or((0, 0));
        Ok(InvokeResponse {
            text,
            tokens_in: tin,
            tokens_out: tout,
            cost_cents: 0,
            latency_ms: start.elapsed().as_millis() as u64,
            model_id: req.model_id,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn invoke_happy_path() {
        let mock = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "choices": [{"message": {"content": "pong"}}],
                "usage": {"prompt_tokens": 2, "completion_tokens": 1}
            })))
            .mount(&mock)
            .await;
        let p = OpenAiSwarmProvider::with_base_and_key(mock.uri(), "sk-test");
        let resp = p
            .invoke(InvokeRequest {
                model_id: "gpt-4o-mini".into(),
                prompt: "ping".into(),
                max_tokens: 8,
                temperature: Some(0.1),
                metadata: serde_json::Value::Null,
            })
            .await
            .unwrap();
        assert_eq!(resp.text, "pong");
        assert_eq!(resp.tokens_in, 2);
    }

    #[tokio::test]
    async fn auth_failure_returns_auth_error() {
        let mock = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&mock)
            .await;
        let p = OpenAiSwarmProvider::with_base_and_key(mock.uri(), "sk-bad");
        let err = p
            .invoke(InvokeRequest {
                model_id: "gpt-4o-mini".into(),
                prompt: "ping".into(),
                max_tokens: 8,
                temperature: None,
                metadata: serde_json::Value::Null,
            })
            .await
            .unwrap_err();
        assert!(matches!(err, ProviderError::AuthFailed(_)));
    }

    #[tokio::test]
    async fn rate_limit_returns_http_429() {
        let mock = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(429))
            .mount(&mock)
            .await;
        let p = OpenAiSwarmProvider::with_base_and_key(mock.uri(), "sk-test");
        let err = p
            .invoke(InvokeRequest {
                model_id: "gpt-4o-mini".into(),
                prompt: "x".into(),
                max_tokens: 1,
                temperature: None,
                metadata: serde_json::Value::Null,
            })
            .await
            .unwrap_err();
        assert!(matches!(err, ProviderError::Http(_, 429, _)));
    }
}
