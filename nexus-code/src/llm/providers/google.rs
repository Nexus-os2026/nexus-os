use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::error::NxError;
use crate::llm::provider::LlmProvider;
use crate::llm::streaming::parse_google_sse_stream;
use crate::llm::types::{LlmRequest, LlmResponse, Role, StreamChunk, TokenUsage};

/// Google Gemini GenerateContent API provider (unique format — NOT OpenAI-compatible).
pub struct GoogleProvider {
    client: reqwest::Client,
    api_key: Option<String>,
    base_url: String,
}

impl GoogleProvider {
    /// Create a new Google Gemini provider.
    pub fn new() -> Self {
        let api_key = std::env::var("GOOGLE_API_KEY").ok();
        Self {
            client: reqwest::Client::new(),
            api_key,
            base_url: "https://generativelanguage.googleapis.com/v1beta".to_string(),
        }
    }

    /// Build the Gemini-format request body.
    fn build_body(&self, request: &LlmRequest) -> serde_json::Value {
        let system_text = request.system.clone().unwrap_or_else(|| {
            request
                .messages
                .iter()
                .filter(|m| m.role == Role::System)
                .map(|m| m.content.clone())
                .collect::<Vec<_>>()
                .join("\n")
        });

        let contents: Vec<serde_json::Value> = request
            .messages
            .iter()
            .filter(|m| m.role != Role::System)
            .map(|m| {
                let role = match m.role {
                    Role::User => "user",
                    Role::Assistant => "model",
                    Role::System => unreachable!(),
                };
                serde_json::json!({
                    "role": role,
                    "parts": [{"text": m.content}],
                })
            })
            .collect();

        let mut body = serde_json::json!({
            "contents": contents,
            "generationConfig": {
                "maxOutputTokens": request.max_tokens,
            },
        });

        if !system_text.is_empty() {
            body["systemInstruction"] = serde_json::json!({
                "parts": [{"text": system_text}],
            });
        }
        if let Some(temp) = request.temperature {
            body["generationConfig"]["temperature"] = serde_json::json!(temp);
        }

        body
    }
}

impl Default for GoogleProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LlmProvider for GoogleProvider {
    fn name(&self) -> &str {
        "google"
    }

    async fn complete(&self, request: &LlmRequest) -> Result<LlmResponse, NxError> {
        let api_key = self
            .api_key
            .as_ref()
            .ok_or_else(|| NxError::ProviderError {
                provider: "google".to_string(),
                message: "GOOGLE_API_KEY not set".to_string(),
            })?;

        let body = self.build_body(request);
        let url = format!(
            "{}/models/{}:generateContent?key={}",
            self.base_url, request.model, api_key
        );

        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(NxError::ProviderError {
                provider: "google".to_string(),
                message: format!("HTTP {}: {}", status, text),
            });
        }

        let json: serde_json::Value = response.json().await?;

        // Parse content parts (may contain text and/or functionCall)
        let content_blocks = json
            .get("candidates")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|cand| cand.get("content"))
            .and_then(|content| content.get("parts"))
            .and_then(|parts| parts.as_array())
            .cloned();

        // Extract text content from parts
        let content = content_blocks
            .as_ref()
            .map(|parts| {
                parts
                    .iter()
                    .filter_map(|part| part.get("text")?.as_str().map(|s| s.to_string()))
                    .collect::<Vec<_>>()
                    .join("")
            })
            .unwrap_or_default();

        let input_tokens = json
            .get("usageMetadata")
            .and_then(|u| u.get("promptTokenCount"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let output_tokens = json
            .get("usageMetadata")
            .and_then(|u| u.get("candidatesTokenCount"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        let finish_reason = json
            .get("candidates")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|cand| cand.get("finishReason"))
            .and_then(|r| r.as_str())
            .map(|s| s.to_string());

        Ok(LlmResponse {
            content,
            model: request.model.clone(),
            usage: TokenUsage {
                input_tokens,
                output_tokens,
                total_tokens: input_tokens + output_tokens,
            },
            finish_reason: finish_reason.clone(),
            content_blocks,
            tool_calls: None,
            stop_reason: finish_reason,
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
                provider: "google".to_string(),
                message: "GOOGLE_API_KEY not set".to_string(),
            })?;

        let body = self.build_body(request);
        let url = format!(
            "{}/models/{}:streamGenerateContent?alt=sse&key={}",
            self.base_url, request.model, api_key
        );

        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(NxError::ProviderError {
                provider: "google".to_string(),
                message: format!("HTTP {}: {}", status, text),
            });
        }

        parse_google_sse_stream(response, tx).await
    }

    fn available_models(&self) -> Vec<&str> {
        vec!["gemini-2.5-pro", "gemini-2.5-flash"]
    }

    fn is_configured(&self) -> bool {
        self.api_key.is_some()
    }
}
