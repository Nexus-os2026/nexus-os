use super::{curl_post_json, LlmProvider, LlmResponse, ProviderRequest};
use nexus_kernel::errors::AgentError;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::env;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeepSeekProvider {
    api_key: Option<String>,
    endpoint: String,
}

impl DeepSeekProvider {
    pub fn new(api_key: Option<String>) -> Self {
        Self {
            api_key,
            endpoint: "https://api.deepseek.com/v1/chat/completions".to_string(),
        }
    }

    pub fn from_env() -> Self {
        let endpoint = env::var("DEEPSEEK_URL")
            .unwrap_or_else(|_| "https://api.deepseek.com/v1/chat/completions".to_string());
        // Optional: API key may not be configured in environment
        let mut provider = Self::new(env::var("DEEPSEEK_API_KEY").ok());
        provider.endpoint = endpoint;
        provider
    }

    pub fn build_request(&self, prompt: &str, max_tokens: u32, model: &str) -> ProviderRequest {
        let api_key = self.api_key.clone().unwrap_or_default();
        let mut headers = BTreeMap::new();
        headers.insert(
            "authorization".to_string(),
            format!("Bearer {}", api_key.trim()),
        );
        headers.insert("content-type".to_string(), "application/json".to_string());

        ProviderRequest {
            endpoint: self.endpoint.clone(),
            headers,
            body: json!({
                "model": model,
                "messages": [
                    {
                        "role": "user",
                        "content": prompt
                    }
                ],
                "max_tokens": max_tokens
            }),
        }
    }

    fn api_key(&self) -> Option<String> {
        self.api_key
            .clone()
            .or_else(|| env::var("DEEPSEEK_API_KEY").ok())
            .map(|key| key.trim().to_string())
            .filter(|key| !key.is_empty())
    }
}

impl LlmProvider for DeepSeekProvider {
    fn query(&self, prompt: &str, max_tokens: u32, model: &str) -> Result<LlmResponse, AgentError> {
        let Some(api_key) = self.api_key() else {
            return Err(AgentError::SupervisorError(
                "DEEPSEEK_API_KEY is not set".to_string(),
            ));
        };
        let request = DeepSeekProvider::new(Some(api_key)).build_request(prompt, max_tokens, model);

        let (status, payload) =
            curl_post_json(request.endpoint.as_str(), &request.headers, &request.body)?;
        if !(200..300).contains(&status) {
            return Err(AgentError::SupervisorError(format!(
                "deepseek request failed with status {status}"
            )));
        }

        let output_text = payload
            .get("choices")
            .and_then(Value::as_array)
            .and_then(|choices| choices.first())
            .and_then(|choice| choice.get("message"))
            .and_then(|message| message.get("content"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();

        let token_count = payload
            .get("usage")
            .and_then(|usage| usage.get("total_tokens"))
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
            .unwrap_or(max_tokens.min(256));

        Ok(LlmResponse {
            output_text,
            token_count,
            model_name: model.to_string(),
            tool_calls: Vec::new(),
        })
    }

    fn name(&self) -> &str {
        "deepseek"
    }

    fn cost_per_token(&self) -> f64 {
        0.000_002
    }

    fn endpoint_url(&self) -> String {
        "https://api.deepseek.com".to_string()
    }
}
