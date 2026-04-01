use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::error::NxError;
use crate::llm::provider::LlmProvider;
use crate::llm::streaming::parse_anthropic_sse_stream;
use crate::llm::types::{LlmRequest, LlmResponse, Role, StreamChunk, TokenUsage};

/// Anthropic Messages API provider (unique format — NOT OpenAI-compatible).
pub struct AnthropicProvider {
    client: reqwest::Client,
    api_key: Option<String>,
    base_url: String,
}

impl AnthropicProvider {
    /// Create a new Anthropic provider.
    pub fn new() -> Self {
        let api_key = std::env::var("ANTHROPIC_API_KEY").ok();
        Self {
            client: reqwest::Client::new(),
            api_key,
            base_url: "https://api.anthropic.com".to_string(),
        }
    }

    /// Build the Anthropic-format request body.
    fn build_body(&self, request: &LlmRequest, streaming: bool) -> serde_json::Value {
        // Extract system prompt
        let system_text = request.system.clone().unwrap_or_else(|| {
            request
                .messages
                .iter()
                .filter(|m| m.role == Role::System)
                .map(|m| m.content.clone())
                .collect::<Vec<_>>()
                .join("\n")
        });

        // Build messages (exclude System role — Anthropic uses top-level system field)
        let messages: Vec<serde_json::Value> = request
            .messages
            .iter()
            .filter(|m| m.role != Role::System)
            .map(|m| {
                let role = match m.role {
                    Role::User => "user",
                    Role::Assistant => "assistant",
                    Role::System => unreachable!(),
                };
                serde_json::json!({"role": role, "content": m.content})
            })
            .collect();

        let mut body = serde_json::json!({
            "model": request.model,
            "max_tokens": request.max_tokens,
            "messages": messages,
            "stream": streaming,
        });

        if !system_text.is_empty() {
            body["system"] = serde_json::Value::String(system_text);
        }
        if let Some(temp) = request.temperature {
            body["temperature"] = serde_json::json!(temp);
        }

        body
    }
}

impl Default for AnthropicProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LlmProvider for AnthropicProvider {
    fn name(&self) -> &str {
        "anthropic"
    }

    async fn complete(&self, request: &LlmRequest) -> Result<LlmResponse, NxError> {
        let api_key = self
            .api_key
            .as_ref()
            .ok_or_else(|| NxError::ProviderError {
                provider: "anthropic".to_string(),
                message: "ANTHROPIC_API_KEY not set".to_string(),
            })?;

        let body = self.build_body(request, false);

        let response = self
            .client
            .post(format!("{}/v1/messages", self.base_url))
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(NxError::ProviderError {
                provider: "anthropic".to_string(),
                message: format!("HTTP {}: {}", status, text),
            });
        }

        let json: serde_json::Value = response.json().await?;

        // Parse content blocks (may contain text and/or tool_use blocks)
        let content_blocks = json.get("content").and_then(|c| c.as_array()).cloned();

        // Extract text from content blocks
        let content = content_blocks
            .as_ref()
            .map(|blocks| {
                blocks
                    .iter()
                    .filter_map(|b| {
                        if b.get("type")?.as_str()? == "text" {
                            b.get("text")?.as_str().map(|s| s.to_string())
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("")
            })
            .unwrap_or_default();

        let input_tokens = json
            .get("usage")
            .and_then(|u| u.get("input_tokens"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let output_tokens = json
            .get("usage")
            .and_then(|u| u.get("output_tokens"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        let stop_reason = json
            .get("stop_reason")
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
            finish_reason: stop_reason.clone(),
            content_blocks,
            tool_calls: None,
            stop_reason,
        })
    }

    async fn stream(
        &self,
        request: &LlmRequest,
        tx: mpsc::UnboundedSender<StreamChunk>,
    ) -> Result<(), NxError> {
        let api_key = self
            .api_key
            .as_ref()
            .ok_or_else(|| NxError::ProviderError {
                provider: "anthropic".to_string(),
                message: "ANTHROPIC_API_KEY not set".to_string(),
            })?;

        let body = self.build_body(request, true);

        let response = self
            .client
            .post(format!("{}/v1/messages", self.base_url))
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(NxError::ProviderError {
                provider: "anthropic".to_string(),
                message: format!("HTTP {}: {}", status, text),
            });
        }

        parse_anthropic_sse_stream(response, tx).await
    }

    async fn stream_raw(&self, request: &LlmRequest) -> Result<Option<reqwest::Response>, NxError> {
        let api_key = self
            .api_key
            .as_ref()
            .ok_or_else(|| NxError::ProviderError {
                provider: "anthropic".to_string(),
                message: "ANTHROPIC_API_KEY not set".to_string(),
            })?;

        let body = self.build_body(request, true);

        let response = self
            .client
            .post(format!("{}/v1/messages", self.base_url))
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(NxError::ProviderError {
                provider: "anthropic".to_string(),
                message: format!("HTTP {}: {}", status, text),
            });
        }

        Ok(Some(response))
    }

    fn available_models(&self) -> Vec<&str> {
        vec![
            "claude-opus-4-20250514",
            "claude-sonnet-4-20250514",
            "claude-haiku-4-20250514",
        ]
    }

    fn is_configured(&self) -> bool {
        self.api_key.is_some()
    }
}
