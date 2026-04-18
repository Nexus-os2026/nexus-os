//! OpenRouter swarm provider.
//!
//! Uses the OpenAI-compatible chat/completions endpoint at
//! `https://openrouter.ai/api/v1`. Model catalog fetched from
//! `GET /api/v1/models` and cached for 1h.

use crate::events::{ProviderHealth, ProviderHealthStatus};
use crate::profile::{CostClass, PrivacyClass, ReasoningTier};
use crate::provider::{
    InvokeRequest, InvokeResponse, ModelDescriptor, Provider, ProviderCapabilities, ProviderError,
};
use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const KEYRING_SERVICE: &str = "nexus.openrouter";
const KEYRING_USER: &str = "api_key";
const DEFAULT_BASE_URL: &str = "https://openrouter.ai/api/v1";
const CACHE_TTL: Duration = Duration::from_secs(3600);

pub struct OpenRouterSwarmProvider {
    base_url: String,
    client: Client,
    key_override: Option<String>,
    cache: Mutex<Option<(Instant, Vec<ModelDescriptor>)>>,
}

impl OpenRouterSwarmProvider {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .unwrap_or_else(|_| Client::new());
        Self {
            base_url: DEFAULT_BASE_URL.into(),
            client,
            key_override: None,
            cache: Mutex::new(None),
        }
    }

    pub fn with_base_and_key(base_url: impl Into<String>, key: impl Into<String>) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .unwrap_or_else(|_| Client::new());
        Self {
            base_url: base_url.into(),
            client,
            key_override: Some(key.into()),
            cache: Mutex::new(None),
        }
    }

    fn api_key(&self) -> Result<String, ProviderError> {
        if let Some(k) = &self.key_override {
            return Ok(k.clone());
        }
        let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
            .map_err(|e| ProviderError::NotConfigured(format!("openrouter keyring: {e}")))?;
        entry.get_password().map_err(|_| {
            ProviderError::NotConfigured("openrouter api key missing from keyring".into())
        })
    }

    async fn list_models_cached(&self) -> Result<Vec<ModelDescriptor>, ProviderError> {
        if let Ok(g) = self.cache.lock() {
            if let Some((ts, ref ms)) = *g {
                if ts.elapsed() < CACHE_TTL {
                    return Ok(ms.clone());
                }
            }
        }
        #[derive(Deserialize)]
        struct ModelsResp {
            data: Vec<Entry>,
        }
        #[derive(Deserialize)]
        struct Entry {
            id: String,
            #[serde(default)]
            context_length: Option<u32>,
        }
        let url = format!("{}/models", self.base_url.trim_end_matches('/'));
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| ProviderError::Transport("openrouter".into(), e.to_string()))?;
        if !resp.status().is_success() {
            return Err(ProviderError::Http(
                "openrouter".into(),
                resp.status().as_u16(),
                resp.text().await.unwrap_or_default(),
            ));
        }
        let parsed: ModelsResp = resp
            .json()
            .await
            .map_err(|e| ProviderError::Malformed("openrouter".into(), e.to_string()))?;
        let models = parsed
            .data
            .into_iter()
            .map(|e| ModelDescriptor {
                id: e.id,
                param_count_b: None,
                tier: ReasoningTier::Medium,
                context_window: e.context_length.unwrap_or(32_000),
            })
            .collect::<Vec<_>>();
        if let Ok(mut g) = self.cache.lock() {
            *g = Some((Instant::now(), models.clone()));
        }
        Ok(models)
    }
}

impl Default for OpenRouterSwarmProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Provider for OpenRouterSwarmProvider {
    fn id(&self) -> &str {
        "openrouter"
    }

    fn capabilities(&self) -> ProviderCapabilities {
        let models = self
            .cache
            .lock()
            .ok()
            .and_then(|g| g.as_ref().map(|(_, m)| m.clone()))
            .unwrap_or_default();
        ProviderCapabilities {
            models,
            supports_tool_use: true,
            supports_streaming: true,
            max_context: 128_000,
            cost_class: CostClass::Standard,
            privacy_class: PrivacyClass::Public,
        }
    }

    async fn health_check(&self) -> ProviderHealth {
        let start = Instant::now();
        match self.list_models_cached().await {
            Ok(models) => ProviderHealth {
                provider_id: "openrouter".into(),
                status: if models.is_empty() {
                    ProviderHealthStatus::Degraded
                } else {
                    ProviderHealthStatus::Ok
                },
                latency_ms: Some(start.elapsed().as_millis() as u64),
                models: models.iter().map(|m| m.id.clone()).collect(),
                notes: String::new(),
                checked_at_secs: chrono::Utc::now().timestamp(),
            },
            Err(e) => ProviderHealth {
                provider_id: "openrouter".into(),
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
            message: Msg,
        }
        #[derive(Deserialize)]
        struct Msg {
            content: String,
        }
        #[derive(Deserialize)]
        struct Usage {
            prompt_tokens: u32,
            completion_tokens: u32,
        }

        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
        let start = Instant::now();
        let body = ChatReq {
            model: &req.model_id,
            max_tokens: req.max_tokens,
            temperature: req.temperature.unwrap_or(0.2),
            messages: vec![ChatMsg {
                role: "user",
                content: &req.prompt,
            }],
        };
        let resp = self
            .client
            .post(&url)
            .bearer_auth(&key)
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::Transport("openrouter".into(), e.to_string()))?;
        let status = resp.status();
        if status == 401 {
            return Err(ProviderError::AuthFailed("openrouter".into()));
        }
        if status == 429 {
            return Err(ProviderError::Http(
                "openrouter".into(),
                429,
                "rate limited".into(),
            ));
        }
        if !status.is_success() {
            return Err(ProviderError::Http(
                "openrouter".into(),
                status.as_u16(),
                resp.text().await.unwrap_or_default(),
            ));
        }
        let parsed: ChatResp = resp
            .json()
            .await
            .map_err(|e| ProviderError::Malformed("openrouter".into(), e.to_string()))?;
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
    async fn models_list_caches() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [
                    {"id": "openai/gpt-4o", "context_length": 128000},
                    {"id": "anthropic/claude-sonnet-4-6", "context_length": 200000}
                ]
            })))
            .expect(1) // second call must hit the cache
            .mount(&mock)
            .await;
        let p = OpenRouterSwarmProvider::with_base_and_key(mock.uri(), "k");
        let _ = p.list_models_cached().await.unwrap();
        let _ = p.list_models_cached().await.unwrap();
    }

    #[tokio::test]
    async fn invoke_happy_path() {
        let mock = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "choices": [{"message": {"content": "ok"}}],
                "usage": {"prompt_tokens": 3, "completion_tokens": 1}
            })))
            .mount(&mock)
            .await;
        let p = OpenRouterSwarmProvider::with_base_and_key(mock.uri(), "k");
        let r = p
            .invoke(InvokeRequest {
                model_id: "openai/gpt-4o-mini".into(),
                prompt: "hi".into(),
                max_tokens: 1,
                temperature: None,
                metadata: serde_json::Value::Null,
            })
            .await
            .unwrap();
        assert_eq!(r.text, "ok");
    }

    #[tokio::test]
    async fn auth_failure_maps_cleanly() {
        let mock = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&mock)
            .await;
        let p = OpenRouterSwarmProvider::with_base_and_key(mock.uri(), "bad");
        let err = p
            .invoke(InvokeRequest {
                model_id: "x".into(),
                prompt: "y".into(),
                max_tokens: 1,
                temperature: None,
                metadata: serde_json::Value::Null,
            })
            .await
            .unwrap_err();
        assert!(matches!(err, ProviderError::AuthFailed(_)));
    }
}
