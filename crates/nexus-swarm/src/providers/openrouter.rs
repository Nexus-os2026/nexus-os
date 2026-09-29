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

// Bug AK Commit 3: per-provider keyring service-string / user
// constants removed; the namespace is owned by `service_string()`
// in kernel/src/secrets/backend_keyring.rs (Commit 4 lands the
// real keyring backend on top of that scheme). See anthropic.rs
// for the rewrite rationale.
const DEFAULT_BASE_URL: &str = "https://openrouter.ai/api/v1";
const CACHE_TTL: Duration = Duration::from_secs(3600);

/// The client for the credentialed calls (Final Gate item C): a 60 s
/// timeout and no redirect. reqwest drops the bearer token on a redirect to
/// another host, but on 307 and 308 it would re-send the prompt there; a
/// redirect is reported as its status instead. Like the `Client::new()`
/// fallback before, building it panics only when no TLS backend can be
/// initialized.
fn credential_client() -> Client {
    let no_redirect = || Client::builder().redirect(reqwest::redirect::Policy::none());
    no_redirect()
        .timeout(Duration::from_secs(60))
        .build()
        .or_else(|_| no_redirect().build())
        .expect("failed to initialize the HTTP client")
}

pub struct OpenRouterSwarmProvider {
    base_url: String,
    client: Client,
    key_override: Option<String>,
    cache: Mutex<Option<(Instant, Vec<ModelDescriptor>)>>,
}

impl OpenRouterSwarmProvider {
    pub fn new() -> Self {
        Self {
            base_url: DEFAULT_BASE_URL.into(),
            client: credential_client(),
            key_override: None,
            cache: Mutex::new(None),
        }
    }

    pub fn with_base_and_key(base_url: impl Into<String>, key: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            client: credential_client(),
            key_override: Some(key.into()),
            cache: Mutex::new(None),
        }
    }

    fn api_key(&self) -> Result<String, ProviderError> {
        if let Some(k) = &self.key_override {
            return Ok(k.clone());
        }
        // Bug AK Commit 3: route through the kernel SecretsFacade.
        // See anthropic.rs for the rationale.
        let facade = nexus_kernel::secrets::global::try_facade().ok_or_else(|| {
            ProviderError::NotConfigured(
                "openrouter: vault not initialized (kernel::startup::run_migrations not run)"
                    .into(),
            )
        })?;
        facade
            .get_secret(
                &nexus_kernel::secrets::SecretAuditCtx::provider_init(),
                "llm",
                "openrouter",
            )
            .map(|s| s.value.to_string())
            .map_err(|_| {
                ProviderError::NotConfigured("openrouter api key missing from vault".into())
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
                super::error_text(resp, super::MAX_ERROR_BODY_BYTES).await,
            ));
        }
        let raw = super::read_capped(resp, super::MAX_RESPONSE_BYTES)
            .await
            .map_err(|e| ProviderError::Malformed("openrouter".into(), e))?;
        let parsed: ModelsResp = serde_json::from_slice(&raw)
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
                super::error_text(resp, super::MAX_ERROR_BODY_BYTES).await,
            ));
        }
        let raw = super::read_capped(resp, super::MAX_RESPONSE_BYTES)
            .await
            .map_err(|e| ProviderError::Malformed("openrouter".into(), e))?;
        let parsed: ChatResp = serde_json::from_slice(&raw)
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

    /// Final Gate item C: the calls follow no redirect, so the prompt (which
    /// reqwest re-sends on 307 and 308) never reaches the redirect target,
    /// which is never contacted; the redirect is reported as its status.
    #[tokio::test]
    async fn p0_fg_calls_follow_no_redirect() {
        let target = MockServer::start().await;
        for code in [301u16, 302, 307, 308] {
            let origin = MockServer::start().await;
            for route in ["/chat/completions", "/models"] {
                Mock::given(path(route))
                    .respond_with(
                        ResponseTemplate::new(code)
                            .insert_header("location", format!("{}{route}", target.uri())),
                    )
                    .mount(&origin)
                    .await;
            }
            let p = OpenRouterSwarmProvider::with_base_and_key(origin.uri(), "sk-fg-secret");
            let err = p
                .invoke(InvokeRequest {
                    model_id: "openai/gpt-4o-mini".into(),
                    prompt: "fg prompt".into(),
                    max_tokens: 8,
                    temperature: None,
                    metadata: serde_json::Value::Null,
                })
                .await
                .unwrap_err();
            assert!(
                matches!(&err, ProviderError::Http(_, status, _) if *status == code),
                "{code}: {err:?}"
            );
            let health = p.health_check().await;
            assert_eq!(health.status, ProviderHealthStatus::Unhealthy, "{code}");
            assert_eq!(origin.received_requests().await.unwrap().len(), 2, "{code}");
        }
        assert!(
            target.received_requests().await.unwrap().is_empty(),
            "the redirect target was contacted"
        );
    }
}
