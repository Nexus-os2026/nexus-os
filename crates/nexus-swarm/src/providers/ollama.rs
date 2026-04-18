//! Ollama swarm provider.
//!
//! Wraps the existing `nexus_connectors_llm::providers::ollama::OllamaProvider`
//! (adds async + health + dynamic model discovery on top of its synchronous
//! `LlmProvider::query`).
//!
//! - Base URL from `OLLAMA_URL` or `http://localhost:11434`.
//! - `health_check()` probes `GET /api/tags` and enumerates installed models;
//!   tier classification follows the spec:
//!   ≤ 3B = Light, ≤ 7B = Medium, ≤ 14B = Heavy, > 14B = Expert.
//!   Tag cache TTL: 60 seconds.
//! - Privacy class: `StrictLocal`.

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

const DEFAULT_BASE_URL: &str = "http://localhost:11434";
const CACHE_TTL: Duration = Duration::from_secs(60);

pub struct OllamaSwarmProvider {
    base_url: String,
    client: Client,
    cache: Mutex<Option<(Instant, Vec<ModelDescriptor>)>>,
}

impl OllamaSwarmProvider {
    pub fn new(base_url: impl Into<String>) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| Client::new());
        Self {
            base_url: base_url.into(),
            client,
            cache: Mutex::new(None),
        }
    }

    pub fn from_env() -> Self {
        let url = std::env::var("OLLAMA_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.into());
        Self::new(url)
    }

    async fn list_models_uncached(&self) -> Result<Vec<ModelDescriptor>, ProviderError> {
        #[derive(Deserialize)]
        struct TagsResp {
            models: Vec<TagEntry>,
        }
        #[derive(Deserialize)]
        struct TagEntry {
            name: String,
            #[serde(default)]
            details: Option<TagDetails>,
        }
        #[derive(Deserialize)]
        struct TagDetails {
            #[serde(default)]
            parameter_size: Option<String>,
        }

        let url = format!("{}/api/tags", self.base_url.trim_end_matches('/'));
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| ProviderError::Transport("ollama".into(), e.to_string()))?;
        if !resp.status().is_success() {
            let code = resp.status().as_u16();
            let text = resp.text().await.unwrap_or_default();
            return Err(ProviderError::Http("ollama".into(), code, text));
        }
        let parsed: TagsResp = resp
            .json()
            .await
            .map_err(|e| ProviderError::Malformed("ollama".into(), e.to_string()))?;
        let models = parsed
            .models
            .into_iter()
            .map(|m| {
                let params = m
                    .details
                    .as_ref()
                    .and_then(|d| d.parameter_size.as_ref())
                    .and_then(|s| parse_param_count_b(s))
                    .or_else(|| parse_param_count_b(&m.name));
                let tier = classify_tier(params);
                ModelDescriptor {
                    id: m.name,
                    param_count_b: params,
                    tier,
                    context_window: 8192,
                }
            })
            .collect();
        Ok(models)
    }

    async fn list_models_cached(&self) -> Result<Vec<ModelDescriptor>, ProviderError> {
        if let Ok(guard) = self.cache.lock() {
            if let Some((ts, ref models)) = *guard {
                if ts.elapsed() < CACHE_TTL {
                    return Ok(models.clone());
                }
            }
        }
        let fresh = self.list_models_uncached().await?;
        if let Ok(mut guard) = self.cache.lock() {
            *guard = Some((Instant::now(), fresh.clone()));
        }
        Ok(fresh)
    }
}

pub(crate) fn parse_param_count_b(s: &str) -> Option<u32> {
    let re = regex::Regex::new(r"(?i)(\d+(?:\.\d+)?)\s*[bB]").ok()?;
    let cap = re.captures(s)?;
    let value: f32 = cap.get(1)?.as_str().parse().ok()?;
    Some(value.round() as u32)
}

pub(crate) fn classify_tier(param_b: Option<u32>) -> ReasoningTier {
    match param_b {
        Some(n) if n <= 3 => ReasoningTier::Light,
        Some(n) if n <= 7 => ReasoningTier::Medium,
        Some(n) if n <= 14 => ReasoningTier::Heavy,
        Some(_) => ReasoningTier::Expert,
        None => ReasoningTier::Light,
    }
}

#[async_trait]
impl Provider for OllamaSwarmProvider {
    fn id(&self) -> &str {
        "ollama"
    }

    fn capabilities(&self) -> ProviderCapabilities {
        // Capabilities are snapshot at call time from the cache if populated;
        // otherwise we return an empty model list (health_check will populate).
        let models = self
            .cache
            .lock()
            .ok()
            .and_then(|g| g.as_ref().map(|(_, m)| m.clone()))
            .unwrap_or_default();
        ProviderCapabilities {
            models,
            supports_tool_use: false,
            supports_streaming: true,
            max_context: 8192,
            cost_class: CostClass::Free,
            privacy_class: PrivacyClass::StrictLocal,
        }
    }

    async fn health_check(&self) -> ProviderHealth {
        let start = Instant::now();
        match self.list_models_cached().await {
            Ok(models) => {
                let ms = start.elapsed().as_millis() as u64;
                let status = if models.is_empty() {
                    ProviderHealthStatus::Degraded
                } else {
                    ProviderHealthStatus::Ok
                };
                ProviderHealth {
                    provider_id: "ollama".into(),
                    status,
                    latency_ms: Some(ms),
                    models: models.iter().map(|m| m.id.clone()).collect(),
                    notes: if models.is_empty() {
                        "no models installed".into()
                    } else {
                        String::new()
                    },
                    checked_at_secs: chrono::Utc::now().timestamp(),
                }
            }
            Err(e) => ProviderHealth {
                provider_id: "ollama".into(),
                status: ProviderHealthStatus::Unhealthy,
                latency_ms: None,
                models: vec![],
                notes: e.to_string(),
                checked_at_secs: chrono::Utc::now().timestamp(),
            },
        }
    }

    async fn invoke(&self, req: InvokeRequest) -> Result<InvokeResponse, ProviderError> {
        #[derive(serde::Serialize)]
        struct GenReq<'a> {
            model: &'a str,
            prompt: &'a str,
            stream: bool,
            options: GenOpts,
        }
        #[derive(serde::Serialize)]
        struct GenOpts {
            num_predict: u32,
            temperature: f32,
        }
        #[derive(Deserialize)]
        struct GenResp {
            #[serde(default)]
            response: String,
            #[serde(default)]
            prompt_eval_count: u32,
            #[serde(default)]
            eval_count: u32,
        }

        let url = format!("{}/api/generate", self.base_url.trim_end_matches('/'));
        let start = Instant::now();
        let body = GenReq {
            model: &req.model_id,
            prompt: &req.prompt,
            stream: false,
            options: GenOpts {
                num_predict: req.max_tokens,
                temperature: req.temperature.unwrap_or(0.2),
            },
        };
        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::Transport("ollama".into(), e.to_string()))?;
        if !resp.status().is_success() {
            let code = resp.status().as_u16();
            let text = resp.text().await.unwrap_or_default();
            return Err(ProviderError::Http("ollama".into(), code, text));
        }
        let parsed: GenResp = resp
            .json()
            .await
            .map_err(|e| ProviderError::Malformed("ollama".into(), e.to_string()))?;
        Ok(InvokeResponse {
            text: parsed.response,
            tokens_in: parsed.prompt_eval_count,
            tokens_out: parsed.eval_count,
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

    #[test]
    fn parse_param_count_handles_common_suffixes() {
        assert_eq!(parse_param_count_b("gemma4:e2b"), Some(2));
        assert_eq!(parse_param_count_b("llama3:70b"), Some(70));
        assert_eq!(parse_param_count_b("qwen2.5-coder:14B"), Some(14));
        assert_eq!(parse_param_count_b("nothing-here"), None);
    }

    #[test]
    fn tier_classification_boundaries() {
        assert_eq!(classify_tier(Some(3)), ReasoningTier::Light);
        assert_eq!(classify_tier(Some(4)), ReasoningTier::Medium);
        assert_eq!(classify_tier(Some(7)), ReasoningTier::Medium);
        assert_eq!(classify_tier(Some(8)), ReasoningTier::Heavy);
        assert_eq!(classify_tier(Some(14)), ReasoningTier::Heavy);
        assert_eq!(classify_tier(Some(70)), ReasoningTier::Expert);
        assert_eq!(classify_tier(None), ReasoningTier::Light);
    }

    #[tokio::test]
    async fn health_check_enumerates_eight_models() {
        let mock = MockServer::start().await;
        let body = serde_json::json!({
            "models": [
                {"name": "gemma4:e2b", "details": {"parameter_size": "2B"}},
                {"name": "gemma4:e4b", "details": {"parameter_size": "4B"}},
                {"name": "llama3:8b", "details": {"parameter_size": "8B"}},
                {"name": "llama3:13b", "details": {"parameter_size": "13B"}},
                {"name": "llama3:70b", "details": {"parameter_size": "70B"}},
                {"name": "qwen2.5-coder:7b", "details": {"parameter_size": "7B"}},
                {"name": "mistral:instruct", "details": {"parameter_size": "7B"}},
                {"name": "phi3:3.8b", "details": {"parameter_size": "3.8B"}}
            ]
        });
        Mock::given(method("GET"))
            .and(path("/api/tags"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(&mock)
            .await;

        let p = OllamaSwarmProvider::new(mock.uri());
        let h = p.health_check().await;
        assert_eq!(h.status, ProviderHealthStatus::Ok);
        assert_eq!(h.models.len(), 8);
    }

    #[tokio::test]
    async fn health_check_unhealthy_on_connection_refused() {
        // Unallocated port.
        let p = OllamaSwarmProvider::new("http://127.0.0.1:1");
        let h = p.health_check().await;
        assert_eq!(h.status, ProviderHealthStatus::Unhealthy);
    }

    #[tokio::test]
    async fn invoke_returns_response_and_tokens() {
        let mock = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/generate"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "response": "hello world",
                "prompt_eval_count": 7,
                "eval_count": 3
            })))
            .mount(&mock)
            .await;

        let p = OllamaSwarmProvider::new(mock.uri());
        let resp = p
            .invoke(InvokeRequest {
                model_id: "gemma4:e2b".into(),
                prompt: "hi".into(),
                max_tokens: 16,
                temperature: Some(0.1),
                metadata: serde_json::Value::Null,
            })
            .await
            .unwrap();
        assert_eq!(resp.text, "hello world");
        assert_eq!(resp.tokens_in, 7);
        assert_eq!(resp.tokens_out, 3);
    }
}
