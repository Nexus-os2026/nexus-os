use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::error::NxError;
use crate::llm::provider::LlmProvider;
use crate::llm::streaming::parse_openai_sse_stream;
use crate::llm::types::{LlmRequest, LlmResponse, Role, StreamChunk, TokenUsage};

/// A generic OpenAI-compatible provider.
/// Used by: OpenAI, Ollama, OpenRouter, Groq, DeepSeek.
pub struct OpenAiCompatibleProvider {
    provider_name: String,
    client: reqwest::Client,
    base_url: String,
    api_key_env: String,
    extra_headers: Vec<(String, String)>,
    models: Vec<String>,
    requires_api_key: bool,
}

impl OpenAiCompatibleProvider {
    /// Create a new OpenAI-compatible provider with the given configuration.
    pub fn new(
        name: &str,
        base_url: &str,
        api_key_env: &str,
        _default_model: &str,
        extra_headers: Vec<(String, String)>,
        models: Vec<String>,
        requires_api_key: bool,
    ) -> Self {
        Self {
            provider_name: name.to_string(),
            client: reqwest::Client::new(),
            base_url: base_url.to_string(),
            api_key_env: api_key_env.to_string(),
            extra_headers,
            models,
            requires_api_key,
        }
    }

    /// Get the API key from environment.
    fn api_key(&self) -> Option<String> {
        std::env::var(&self.api_key_env).ok()
    }

    /// Build the request body in OpenAI format.
    fn build_body(&self, request: &LlmRequest, streaming: bool) -> serde_json::Value {
        // Include system message in the messages array (OpenAI format)
        let mut messages: Vec<serde_json::Value> = Vec::new();

        // Add system prompt if provided
        if let Some(ref system) = request.system {
            messages.push(serde_json::json!({"role": "system", "content": system}));
        }

        for m in &request.messages {
            let role = match m.role {
                Role::System => "system",
                Role::User => "user",
                Role::Assistant => "assistant",
            };
            messages.push(serde_json::json!({"role": role, "content": m.content}));
        }

        let mut body = serde_json::json!({
            "model": request.model,
            "messages": messages,
            "max_tokens": request.max_tokens,
            "stream": streaming,
        });

        if streaming {
            body["stream_options"] = serde_json::json!({"include_usage": true});
        }
        if let Some(temp) = request.temperature {
            body["temperature"] = serde_json::json!(temp);
        }

        body
    }

    /// Build the HTTP request with auth and extra headers.
    fn build_http_request(
        &self,
        body: &serde_json::Value,
    ) -> Result<reqwest::RequestBuilder, NxError> {
        let url = format!("{}/chat/completions", self.base_url);

        let mut req = self
            .client
            .post(&url)
            .header("Content-Type", "application/json");

        if let Some(api_key) = self.api_key() {
            req = req.header("Authorization", format!("Bearer {}", api_key));
        }

        for (key, value) in &self.extra_headers {
            req = req.header(key.as_str(), value.as_str());
        }

        Ok(req.json(body))
    }
}

#[async_trait]
impl LlmProvider for OpenAiCompatibleProvider {
    fn name(&self) -> &str {
        &self.provider_name
    }

    async fn complete(&self, request: &LlmRequest) -> Result<LlmResponse, NxError> {
        if self.requires_api_key && self.api_key().is_none() {
            return Err(NxError::ProviderError {
                provider: self.provider_name.clone(),
                message: format!("{} not set", self.api_key_env),
            });
        }

        let body = self.build_body(request, false);
        let response = self.build_http_request(&body)?.send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(NxError::ProviderError {
                provider: self.provider_name.clone(),
                message: format!("HTTP {}: {}", status, text),
            });
        }

        let json: serde_json::Value = response.json().await?;

        let content = json
            .get("choices")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|choice| choice.get("message"))
            .and_then(|msg| msg.get("content"))
            .and_then(|c| c.as_str())
            .unwrap_or("")
            .to_string();

        let input_tokens = json
            .get("usage")
            .and_then(|u| u.get("prompt_tokens"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let output_tokens = json
            .get("usage")
            .and_then(|u| u.get("completion_tokens"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        // Parse tool_calls from the response message
        let tool_calls = json
            .get("choices")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|choice| choice.get("message"))
            .and_then(|msg| msg.get("tool_calls"))
            .and_then(|tc| tc.as_array())
            .cloned();

        let finish_reason = json
            .get("choices")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|choice| choice.get("finish_reason"))
            .and_then(|r| r.as_str())
            .map(|s| s.to_string());

        Ok(LlmResponse {
            content,
            model: json
                .get("model")
                .and_then(|m| m.as_str())
                .unwrap_or(&request.model)
                .to_string(),
            usage: TokenUsage {
                input_tokens,
                output_tokens,
                total_tokens: input_tokens + output_tokens,
            },
            finish_reason: finish_reason.clone(),
            content_blocks: None,
            tool_calls,
            stop_reason: finish_reason,
        })
    }

    async fn stream(
        &self,
        request: &LlmRequest,
        tx: mpsc::UnboundedSender<StreamChunk>,
    ) -> Result<(), NxError> {
        if self.requires_api_key && self.api_key().is_none() {
            return Err(NxError::ProviderError {
                provider: self.provider_name.clone(),
                message: format!("{} not set", self.api_key_env),
            });
        }

        let body = self.build_body(request, true);
        let response = self.build_http_request(&body)?.send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(NxError::ProviderError {
                provider: self.provider_name.clone(),
                message: format!("HTTP {}: {}", status, text),
            });
        }

        parse_openai_sse_stream(response, tx).await
    }

    async fn stream_raw(&self, request: &LlmRequest) -> Result<Option<reqwest::Response>, NxError> {
        if self.requires_api_key && self.api_key().is_none() {
            return Err(NxError::ProviderError {
                provider: self.provider_name.clone(),
                message: format!("{} not set", self.api_key_env),
            });
        }

        let body = self.build_body(request, true);
        let response = self.build_http_request(&body)?.send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(NxError::ProviderError {
                provider: self.provider_name.clone(),
                message: format!("HTTP {}: {}", status, text),
            });
        }

        Ok(Some(response))
    }

    fn available_models(&self) -> Vec<&str> {
        self.models.iter().map(|s| s.as_str()).collect()
    }

    fn is_configured(&self) -> bool {
        if !self.requires_api_key {
            return true;
        }
        self.api_key().is_some()
    }
}
