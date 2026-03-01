use nexus_kernel::errors::AgentError;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderRequest {
    pub endpoint: String,
    pub headers: BTreeMap<String, String>,
    pub body: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LlmResponse {
    pub output_text: String,
    pub token_count: u32,
    pub model_name: String,
    pub tool_calls: Vec<String>,
}

pub trait LlmProvider {
    fn query(&self, prompt: &str, max_tokens: u32, model: &str) -> Result<LlmResponse, AgentError>;
    fn name(&self) -> &str;
    fn cost_per_token(&self) -> f64;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaudeProvider {
    api_key: String,
}

impl ClaudeProvider {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
        }
    }

    pub fn build_request(&self, prompt: &str, max_tokens: u32, model: &str) -> ProviderRequest {
        let mut headers = BTreeMap::new();
        headers.insert("content-type".to_string(), "application/json".to_string());
        headers.insert("x-api-key".to_string(), self.api_key.clone());
        headers.insert("anthropic-version".to_string(), "2023-06-01".to_string());

        ProviderRequest {
            endpoint: "https://api.anthropic.com/v1/messages".to_string(),
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
}

impl LlmProvider for ClaudeProvider {
    fn query(&self, prompt: &str, max_tokens: u32, model: &str) -> Result<LlmResponse, AgentError> {
        let _request = self.build_request(prompt, max_tokens, model);
        Ok(LlmResponse {
            output_text: "Mock Claude response".to_string(),
            token_count: max_tokens,
            model_name: model.to_string(),
            tool_calls: Vec::new(),
        })
    }

    fn name(&self) -> &str {
        "anthropic"
    }

    fn cost_per_token(&self) -> f64 {
        0.000_015
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenAiProvider {
    api_key: String,
}

impl OpenAiProvider {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
        }
    }

    pub fn build_request(&self, prompt: &str, max_tokens: u32, model: &str) -> ProviderRequest {
        let mut headers = BTreeMap::new();
        headers.insert("content-type".to_string(), "application/json".to_string());
        headers.insert(
            "authorization".to_string(),
            format!("Bearer {}", self.api_key),
        );

        ProviderRequest {
            endpoint: "https://api.openai.com/v1/chat/completions".to_string(),
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
}

impl LlmProvider for OpenAiProvider {
    fn query(&self, prompt: &str, max_tokens: u32, model: &str) -> Result<LlmResponse, AgentError> {
        let _request = self.build_request(prompt, max_tokens, model);
        Ok(LlmResponse {
            output_text: "Mock OpenAI response".to_string(),
            token_count: max_tokens,
            model_name: model.to_string(),
            tool_calls: Vec::new(),
        })
    }

    fn name(&self) -> &str {
        "openai"
    }

    fn cost_per_token(&self) -> f64 {
        0.000_010
    }
}

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

    pub fn local_default() -> Self {
        Self::new("http://localhost:11434")
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
}

impl LlmProvider for OllamaProvider {
    fn query(&self, prompt: &str, max_tokens: u32, model: &str) -> Result<LlmResponse, AgentError> {
        let _request = self.build_request(prompt, max_tokens, model);
        Ok(LlmResponse {
            output_text: "Mock Ollama response".to_string(),
            token_count: max_tokens,
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

#[cfg(test)]
mod tests {
    use super::{ClaudeProvider, OllamaProvider};
    use serde_json::json;

    #[test]
    fn test_claude_provider_format() {
        let provider = ClaudeProvider::new("test-key");
        let request = provider.build_request("Summarize this.", 128, "claude-sonnet-4-5");

        assert_eq!(request.endpoint, "https://api.anthropic.com/v1/messages");
        assert_eq!(
            request.headers.get("x-api-key").map(String::as_str),
            Some("test-key")
        );
        assert_eq!(
            request.headers.get("anthropic-version").map(String::as_str),
            Some("2023-06-01")
        );
        assert_eq!(
            request.body,
            json!({
                "model": "claude-sonnet-4-5",
                "max_tokens": 128,
                "messages": [
                    {
                        "role": "user",
                        "content": "Summarize this."
                    }
                ]
            })
        );
    }

    #[test]
    fn test_ollama_provider_local() {
        let provider = OllamaProvider::local_default();
        let request = provider.build_request("hello local model", 64, "llama3");

        assert_eq!(request.endpoint, "http://localhost:11434/api/generate");
        assert_eq!(
            request.body,
            json!({
                "model": "llama3",
                "prompt": "hello local model",
                "stream": false,
                "options": {
                    "num_predict": 64
                }
            })
        );
    }
}
