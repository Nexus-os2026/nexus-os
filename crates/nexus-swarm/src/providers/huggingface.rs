//! HuggingFace Inference API provider.
//!
//! Token from keyring `nexus.hf.token`. Default model configurable via
//! `NEXUS_HF_DEFAULT_MODEL` env var; fallback
//! `meta-llama/Llama-3.2-3B-Instruct` (picked over 70B so healthcheck probes
//! don't trip free-tier rate limits).

use crate::events::{ProviderHealth, ProviderHealthStatus};
use crate::profile::{CostClass, PrivacyClass, ReasoningTier};
use crate::provider::{
    InvokeRequest, InvokeResponse, ModelDescriptor, Provider, ProviderCapabilities, ProviderError,
};
use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use std::time::{Duration, Instant};

const KEYRING_SERVICE: &str = "nexus.hf";
const KEYRING_USER: &str = "token";
const DEFAULT_BASE_URL: &str = "https://api-inference.huggingface.co";
const DEFAULT_MODEL_FALLBACK: &str = "meta-llama/Llama-3.2-3B-Instruct";

pub struct HuggingFaceProvider {
    base_url: String,
    client: Client,
    token_override: Option<String>,
    default_model: String,
}

impl HuggingFaceProvider {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .unwrap_or_else(|_| Client::new());
        let default_model = std::env::var("NEXUS_HF_DEFAULT_MODEL")
            .unwrap_or_else(|_| DEFAULT_MODEL_FALLBACK.into());
        Self {
            base_url: DEFAULT_BASE_URL.into(),
            client,
            token_override: None,
            default_model,
        }
    }

    pub fn with_base_and_token(base_url: impl Into<String>, token: impl Into<String>) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .unwrap_or_else(|_| Client::new());
        let default_model = std::env::var("NEXUS_HF_DEFAULT_MODEL")
            .unwrap_or_else(|_| DEFAULT_MODEL_FALLBACK.into());
        Self {
            base_url: base_url.into(),
            client,
            token_override: Some(token.into()),
            default_model,
        }
    }

    fn token(&self) -> Result<String, ProviderError> {
        if let Some(t) = &self.token_override {
            return Ok(t.clone());
        }
        let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
            .map_err(|e| ProviderError::NotConfigured(format!("hf keyring: {e}")))?;
        entry
            .get_password()
            .map_err(|_| ProviderError::NotConfigured("hf token missing from keyring".into()))
    }
}

impl Default for HuggingFaceProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Provider for HuggingFaceProvider {
    fn id(&self) -> &str {
        "huggingface"
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            models: vec![ModelDescriptor {
                id: self.default_model.clone(),
                param_count_b: Some(3),
                tier: ReasoningTier::Light,
                context_window: 8192,
            }],
            supports_tool_use: false,
            supports_streaming: false,
            max_context: 8192,
            cost_class: CostClass::Low,
            privacy_class: PrivacyClass::Public,
        }
    }

    async fn health_check(&self) -> ProviderHealth {
        let start = Instant::now();
        let tok = match self.token() {
            Ok(t) => t,
            Err(e) => {
                return ProviderHealth {
                    provider_id: "huggingface".into(),
                    status: ProviderHealthStatus::Unhealthy,
                    latency_ms: None,
                    models: vec![],
                    notes: e.to_string(),
                    checked_at_secs: chrono::Utc::now().timestamp(),
                }
            }
        };
        let url = format!(
            "{}/models/{}",
            self.base_url.trim_end_matches('/'),
            self.default_model
        );
        let body = serde_json::json!({
            "inputs": ".",
            "parameters": {"max_new_tokens": 1}
        });
        let resp = self
            .client
            .post(&url)
            .bearer_auth(&tok)
            .json(&body)
            .send()
            .await;
        match resp {
            Ok(r) if r.status().is_success() || r.status().as_u16() == 503 => ProviderHealth {
                // 503 on HF means "model loading" — treat as Degraded.
                provider_id: "huggingface".into(),
                status: if r.status().as_u16() == 503 {
                    ProviderHealthStatus::Degraded
                } else {
                    ProviderHealthStatus::Ok
                },
                latency_ms: Some(start.elapsed().as_millis() as u64),
                models: vec![self.default_model.clone()],
                notes: String::new(),
                checked_at_secs: chrono::Utc::now().timestamp(),
            },
            Ok(r) => ProviderHealth {
                provider_id: "huggingface".into(),
                status: ProviderHealthStatus::Unhealthy,
                latency_ms: Some(start.elapsed().as_millis() as u64),
                models: vec![],
                notes: format!("http {}", r.status()),
                checked_at_secs: chrono::Utc::now().timestamp(),
            },
            Err(e) => ProviderHealth {
                provider_id: "huggingface".into(),
                status: ProviderHealthStatus::Unhealthy,
                latency_ms: None,
                models: vec![],
                notes: e.to_string(),
                checked_at_secs: chrono::Utc::now().timestamp(),
            },
        }
    }

    async fn invoke(&self, req: InvokeRequest) -> Result<InvokeResponse, ProviderError> {
        let tok = self.token()?;
        #[derive(serde::Serialize)]
        struct InferReq<'a> {
            inputs: &'a str,
            parameters: InferParams,
        }
        #[derive(serde::Serialize)]
        struct InferParams {
            max_new_tokens: u32,
            temperature: f32,
            return_full_text: bool,
        }
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum InferResp {
            Array(Vec<GenEntry>),
            Single(GenEntry),
        }
        #[derive(Deserialize)]
        struct GenEntry {
            #[serde(default)]
            generated_text: String,
        }

        let url = format!(
            "{}/models/{}",
            self.base_url.trim_end_matches('/'),
            req.model_id
        );
        let body = InferReq {
            inputs: &req.prompt,
            parameters: InferParams {
                max_new_tokens: req.max_tokens,
                temperature: req.temperature.unwrap_or(0.2),
                return_full_text: false,
            },
        };
        let start = Instant::now();
        let resp = self
            .client
            .post(&url)
            .bearer_auth(&tok)
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::Transport("huggingface".into(), e.to_string()))?;
        let status = resp.status();
        if status == 401 {
            return Err(ProviderError::AuthFailed("huggingface".into()));
        }
        if status == 429 {
            return Err(ProviderError::Http(
                "huggingface".into(),
                429,
                "rate limited".into(),
            ));
        }
        if !status.is_success() {
            return Err(ProviderError::Http(
                "huggingface".into(),
                status.as_u16(),
                resp.text().await.unwrap_or_default(),
            ));
        }
        let parsed: InferResp = resp
            .json()
            .await
            .map_err(|e| ProviderError::Malformed("huggingface".into(), e.to_string()))?;
        let text = match parsed {
            InferResp::Array(mut v) => v.pop().map(|g| g.generated_text).unwrap_or_default(),
            InferResp::Single(g) => g.generated_text,
        };
        Ok(InvokeResponse {
            text,
            tokens_in: 0,
            tokens_out: 0,
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
    async fn invoke_happy_path_parses_array_response() {
        let mock = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/models/meta-llama/Llama-3.2-3B-Instruct"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!(
                [{"generated_text": "hello"}]
            )))
            .mount(&mock)
            .await;
        let p = HuggingFaceProvider::with_base_and_token(mock.uri(), "t");
        let r = p
            .invoke(InvokeRequest {
                model_id: "meta-llama/Llama-3.2-3B-Instruct".into(),
                prompt: "hi".into(),
                max_tokens: 8,
                temperature: None,
                metadata: serde_json::Value::Null,
            })
            .await
            .unwrap();
        assert_eq!(r.text, "hello");
    }

    #[tokio::test]
    async fn auth_failure_maps_cleanly() {
        let mock = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&mock)
            .await;
        let p = HuggingFaceProvider::with_base_and_token(mock.uri(), "bad");
        let err = p
            .invoke(InvokeRequest {
                model_id: "meta-llama/Llama-3.2-3B-Instruct".into(),
                prompt: "x".into(),
                max_tokens: 1,
                temperature: None,
                metadata: serde_json::Value::Null,
            })
            .await
            .unwrap_err();
        assert!(matches!(err, ProviderError::AuthFailed(_)));
    }

    #[tokio::test]
    async fn health_degraded_on_503_loading() {
        let mock = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&mock)
            .await;
        let p = HuggingFaceProvider::with_base_and_token(mock.uri(), "t");
        let h = p.health_check().await;
        assert_eq!(h.status, ProviderHealthStatus::Degraded);
    }
}
