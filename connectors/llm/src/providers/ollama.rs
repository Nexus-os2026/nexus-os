use super::{curl_get_status, curl_post_json, LlmProvider, LlmResponse, ProviderRequest};
use nexus_kernel::errors::AgentError;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::env;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OllamaProvider {
    base_url: String,
}

impl OllamaProvider {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
        }
    }

    pub fn from_env() -> Self {
        let base_url =
            env::var("OLLAMA_URL").unwrap_or_else(|_| "http://localhost:11434".to_string());
        Self::new(base_url)
    }

    pub fn build_request(&self, prompt: &str, max_tokens: u32, model: &str) -> ProviderRequest {
        let mut headers = BTreeMap::new();
        headers.insert("content-type".to_string(), "application/json".to_string());

        ProviderRequest {
            endpoint: format!("{}/api/generate", self.base_url.trim_end_matches('/')),
            headers,
            body: json!({
                "model": model,
                "prompt": prompt,
                "stream": false,
                "options": {
                    "num_predict": max_tokens
                }
            }),
        }
    }

    fn tags_endpoint(&self) -> String {
        format!("{}/api/tags", self.base_url.trim_end_matches('/'))
    }
}

impl LlmProvider for OllamaProvider {
    fn query(&self, prompt: &str, max_tokens: u32, model: &str) -> Result<LlmResponse, AgentError> {
        let tags_status = curl_get_status(self.tags_endpoint().as_str());
        match tags_status {
            Ok(code) if (200..300).contains(&code) => {}
            _ => {
                return Err(AgentError::SupervisorError(format!(
                    "Ollama not running at {}",
                    self.base_url
                )));
            }
        }

        let request = self.build_request(prompt, max_tokens, model);
        let (status, payload) =
            curl_post_json(request.endpoint.as_str(), &request.headers, &request.body)?;
        if !(200..300).contains(&status) {
            return Err(AgentError::SupervisorError(format!(
                "ollama request failed with status {status}"
            )));
        }

        let output_text = payload
            .get("response")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let token_count = payload
            .get("eval_count")
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
            .unwrap_or(max_tokens.min(128));

        Ok(LlmResponse {
            output_text,
            token_count,
            model_name: model.to_string(),
            tool_calls: Vec::new(),
        })
    }

    fn name(&self) -> &str {
        "ollama"
    }

    fn cost_per_token(&self) -> f64 {
        0.0
    }
}
