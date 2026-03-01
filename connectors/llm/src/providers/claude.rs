use super::{LlmProvider, LlmResponse, ProviderRequest};
use nexus_kernel::errors::AgentError;
use serde_json::json;
use std::collections::BTreeMap;
use std::env;

#[cfg(feature = "real-claude")]
use reqwest::blocking::Client;
#[cfg(feature = "real-claude")]
use serde_json::Value;
#[cfg(feature = "real-claude")]
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaudeProvider {
    api_key: Option<String>,
    endpoint: String,
}

impl ClaudeProvider {
    pub fn new(api_key: Option<String>) -> Self {
        Self {
            api_key,
            endpoint: "https://api.anthropic.com/v1/messages".to_string(),
        }
    }

    pub fn from_env() -> Self {
        let endpoint = env::var("ANTHROPIC_URL")
            .unwrap_or_else(|_| "https://api.anthropic.com/v1/messages".to_string());
        let mut provider = Self::new(env::var("ANTHROPIC_API_KEY").ok());
        provider.endpoint = endpoint;
        provider
    }

    pub fn build_request(&self, prompt: &str, max_tokens: u32, model: &str) -> ProviderRequest {
        let mut headers = BTreeMap::new();
        headers.insert("content-type".to_string(), "application/json".to_string());
        headers.insert(
            "x-api-key".to_string(),
            self.api_key.clone().unwrap_or_default(),
        );
        headers.insert("anthropic-version".to_string(), "2023-06-01".to_string());

        ProviderRequest {
            endpoint: self.endpoint.clone(),
            headers,
            body: json!({
                "model": model,
                "max_tokens": max_tokens,
                "messages": [
                    {
                        "role": "user",
                        "content": prompt
                    }
                ]
            }),
        }
    }

    #[cfg(feature = "real-claude")]
    fn api_key(&self) -> Option<String> {
        self.api_key
            .clone()
            .or_else(|| env::var("ANTHROPIC_API_KEY").ok())
            .map(|key| key.trim().to_string())
            .filter(|key| !key.is_empty())
    }
}

impl LlmProvider for ClaudeProvider {
    fn query(&self, prompt: &str, max_tokens: u32, model: &str) -> Result<LlmResponse, AgentError> {
        #[cfg(not(feature = "real-claude"))]
        {
            let _ = (prompt, max_tokens, model);
            return Err(AgentError::SupervisorError(
                "Claude provider is disabled. Rebuild with feature 'real-claude'.".to_string(),
            ));
        }

        #[cfg(feature = "real-claude")]
        {
            super::require_real_api(true)?;

            let Some(api_key) = self.api_key() else {
                return Err(AgentError::SupervisorError(
                    "ANTHROPIC_API_KEY is not set".to_string(),
                ));
            };
            let request =
                ClaudeProvider::new(Some(api_key)).build_request(prompt, max_tokens, model);

            let client = Client::builder()
                .timeout(Duration::from_secs(20))
                .build()
                .map_err(|error| {
                    AgentError::SupervisorError(format!("failed to build HTTP client: {error}"))
                })?;
            let response = client
                .post(request.endpoint)
                .header("x-api-key", request.headers["x-api-key"].as_str())
                .header(
                    "anthropic-version",
                    request.headers["anthropic-version"].as_str(),
                )
                .header("content-type", request.headers["content-type"].as_str())
                .json(&request.body)
                .send()
                .map_err(|error| {
                    AgentError::SupervisorError(format!("claude request failed: {error}"))
                })?;
            let status = response.status();
            let payload = response.json::<Value>().map_err(|error| {
                AgentError::SupervisorError(format!("claude response parse failed: {error}"))
            })?;
            if !status.is_success() {
                return Err(AgentError::SupervisorError(format!(
                    "claude request failed with status {status}"
                )));
            }

            let output_text = payload
                .get("content")
                .and_then(Value::as_array)
                .and_then(|content| content.first())
                .and_then(|item| item.get("text"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();

            let output_tokens = payload
                .get("usage")
                .and_then(|usage| usage.get("output_tokens"))
                .and_then(Value::as_u64)
                .and_then(|value| u32::try_from(value).ok())
                .unwrap_or(max_tokens.min(256));

            Ok(LlmResponse {
                output_text,
                token_count: output_tokens,
                model_name: model.to_string(),
                tool_calls: Vec::new(),
            })
        }
    }

    fn name(&self) -> &str {
        "claude"
    }

    fn cost_per_token(&self) -> f64 {
        0.000_015
    }

    fn requires_real_api_opt_in(&self) -> bool {
        true
    }
}
