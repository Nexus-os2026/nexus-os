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

// Bug AK Commit 3: per-provider keyring service-string / user
// constants removed; the namespace is owned by `service_string()`
// in kernel/src/secrets/backend_keyring.rs (Commit 4 lands the
// real keyring backend on top of that scheme). See anthropic.rs
// for the rewrite rationale.
const DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";

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

pub struct OpenAiSwarmProvider {
    base_url: String,
    client: Client,
    /// Optional override for tests — bypasses keyring.
    key_override: Option<String>,
}

impl OpenAiSwarmProvider {
    pub fn new() -> Self {
        Self {
            base_url: DEFAULT_BASE_URL.into(),
            client: credential_client(),
            key_override: None,
        }
    }

    /// Test helper — point at a mock server with a dummy key.
    pub fn with_base_and_key(base_url: impl Into<String>, key: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            client: credential_client(),
            key_override: Some(key.into()),
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
                "openai: vault not initialized (kernel::startup::run_migrations not run)".into(),
            )
        })?;
        facade
            .get_secret(
                &nexus_kernel::secrets::SecretAuditCtx::provider_init(),
                "llm",
                "openai",
            )
            .map(|s| s.value.to_string())
            .map_err(|_| ProviderError::NotConfigured("openai api key missing from vault".into()))
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
            let text = super::error_text(resp, super::MAX_ERROR_BODY_BYTES).await;
            return Err(ProviderError::Http("openai".into(), status.as_u16(), text));
        }
        let raw = super::read_capped(resp, super::MAX_RESPONSE_BYTES)
            .await
            .map_err(|e| ProviderError::Malformed("openai".into(), e))?;
        let parsed: ChatResp = serde_json::from_slice(&raw)
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
            let p = OpenAiSwarmProvider::with_base_and_key(origin.uri(), "sk-fg-secret");
            let err = p
                .invoke(InvokeRequest {
                    model_id: "gpt-4o-mini".into(),
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
