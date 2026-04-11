use super::{LlmProvider, LlmResponse, ProviderRequest};
use crate::streaming::{
    new_usage_cell, StreamChunk, StreamUsage, StreamingLlmProvider, StreamingResponse, UsageCell,
};
use nexus_kernel::errors::AgentError;
use reqwest::blocking::Client;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::env;
use std::io::BufRead;
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
        // Optional: API key may not be configured in environment
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
        let Some(api_key) = self.api_key() else {
            return Err(AgentError::SupervisorError(
                "ANTHROPIC_API_KEY is not set".to_string(),
            ));
        };
        let request = ClaudeProvider::new(Some(api_key)).build_request(prompt, max_tokens, model);

        let client = Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .map_err(|error| {
                AgentError::SupervisorError(format!("failed to build HTTP client: {error}"))
            })?;

        // Retry loop for overloaded (529) and rate-limited (429) errors
        let mut delay_secs = 5u64;
        let (status, payload) = loop {
            let response = client
                .post(&request.endpoint)
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

            if is_retryable_status(status) && delay_secs <= 20 {
                eprintln!(
                    "[claude] API overloaded ({}), retrying in {}s",
                    status.as_u16(),
                    delay_secs
                );
                std::thread::sleep(Duration::from_secs(delay_secs));
                delay_secs *= 2;
                continue;
            }

            let payload = response.json::<Value>().map_err(|error| {
                AgentError::SupervisorError(format!("claude response parse failed: {error}"))
            })?;
            break (status, payload);
        };

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

        let usage = payload.get("usage");
        let output_tokens = usage
            .and_then(|u| u.get("output_tokens"))
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
            .unwrap_or(max_tokens.min(256));
        let input_tokens = usage
            .and_then(|u| u.get("input_tokens"))
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok());

        let tool_calls = payload
            .get("content")
            .and_then(Value::as_array)
            .map(|content| {
                content
                    .iter()
                    .filter(|item| item.get("type").and_then(Value::as_str) == Some("tool_use"))
                    .filter_map(|item| serde_json::to_string(item).ok())
                    .collect()
            })
            .unwrap_or_default();

        Ok(LlmResponse {
            output_text,
            token_count: output_tokens,
            model_name: model.to_string(),
            tool_calls,
            input_tokens,
        })
    }

    fn name(&self) -> &str {
        "claude"
    }

    fn cost_per_token(&self) -> f64 {
        0.000_015
    }

    fn endpoint_url(&self) -> String {
        "https://api.anthropic.com".to_string()
    }
}

/// Maximum retry attempts for overloaded (529) or rate-limited (429) errors.
const MAX_RETRIES: u32 = 3;

/// Check if an HTTP status code is retryable (overloaded/rate-limited).
fn is_retryable_status(status: reqwest::StatusCode) -> bool {
    status.as_u16() == 529 || status.as_u16() == 429
}

impl StreamingLlmProvider for ClaudeProvider {
    fn stream_query(
        &self,
        prompt: &str,
        system_prompt: &str,
        max_tokens: u32,
        model: &str,
    ) -> Result<StreamingResponse, AgentError> {
        let api_key = self.api_key().ok_or_else(|| {
            AgentError::SupervisorError("ANTHROPIC_API_KEY is not set".to_string())
        })?;

        let mut body = json!({
            "model": model,
            "max_tokens": max_tokens,
            "stream": true,
            "messages": [
                {
                    "role": "user",
                    "content": prompt
                }
            ]
        });

        if !system_prompt.is_empty() {
            body["system"] = Value::String(system_prompt.to_string());
        }

        let client = Client::builder()
            .timeout(Duration::from_secs(300)) // longer timeout for streaming
            .build()
            .map_err(|e| {
                AgentError::SupervisorError(format!("failed to build HTTP client: {e}"))
            })?;

        // Retry loop for overloaded (529) and rate-limited (429) errors
        let mut delay_secs = 5u64;
        let mut last_error = String::new();

        for attempt in 0..=MAX_RETRIES {
            let response = client
                .post(&self.endpoint)
                .header("x-api-key", &api_key)
                .header("anthropic-version", "2023-06-01")
                .header("content-type", "application/json")
                .json(&body)
                .send()
                .map_err(|e| {
                    AgentError::SupervisorError(format!("claude streaming request failed: {e}"))
                })?;

            let status = response.status();
            if status.is_success() {
                // Successful response — proceed to stream parsing below
                return self.parse_streaming_response(response);
            }

            let error_body = response.text().unwrap_or_default();

            if is_retryable_status(status) && attempt < MAX_RETRIES {
                eprintln!(
                    "[claude] API overloaded ({}), retry {}/{} in {}s",
                    status.as_u16(),
                    attempt + 1,
                    MAX_RETRIES,
                    delay_secs
                );
                std::thread::sleep(Duration::from_secs(delay_secs));
                delay_secs *= 2; // 5s, 10s, 20s
                last_error =
                    format!("claude streaming request failed with status {status}: {error_body}");
                continue;
            }

            return Err(AgentError::SupervisorError(format!(
                "claude streaming request failed with status {status}: {error_body}"
            )));
        }

        Err(AgentError::SupervisorError(format!(
            "claude streaming request failed after {} retries: {last_error}",
            MAX_RETRIES
        )))
    }

    fn streaming_provider_name(&self) -> &str {
        "claude"
    }
}

impl ClaudeProvider {
    /// Parse a successful streaming response into a `StreamingResponse`.
    fn parse_streaming_response(
        &self,
        response: reqwest::blocking::Response,
    ) -> Result<StreamingResponse, AgentError> {
        let usage_cell = new_usage_cell();
        let iter_usage = usage_cell.clone();

        let reader = std::io::BufReader::new(response);
        let lines = reader.lines();

        let iter = ClaudeSseIterator {
            lines,
            finished: false,
            usage_cell: iter_usage,
        };

        Ok(StreamingResponse::new(Box::new(iter), usage_cell))
    }
}

/// Iterator that parses Anthropic SSE events into StreamChunks.
struct ClaudeSseIterator<R: BufRead> {
    lines: std::io::Lines<R>,
    finished: bool,
    usage_cell: UsageCell,
}

impl<R: BufRead + Send> Iterator for ClaudeSseIterator<R> {
    type Item = Result<StreamChunk, AgentError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }

        loop {
            let line = match self.lines.next() {
                Some(Ok(l)) => l,
                Some(Err(e)) => {
                    self.finished = true;
                    return Some(Err(AgentError::SupervisorError(format!(
                        "stream read error: {e}"
                    ))));
                }
                None => {
                    self.finished = true;
                    return None;
                }
            };

            let trimmed = line.trim();

            if trimmed.is_empty() {
                continue;
            }

            // Parse "data: {...}" lines
            let Some(data) = trimmed.strip_prefix("data: ") else {
                continue;
            };

            if data == "[DONE]" {
                self.finished = true;
                return None;
            }

            let parsed: Value = match serde_json::from_str(data) {
                Ok(v) => v,
                Err(_) => continue,
            };

            let event_type = parsed["type"].as_str().unwrap_or("");

            match event_type {
                "content_block_delta" => {
                    if let Some(text) = parsed["delta"]["text"].as_str() {
                        if !text.is_empty() {
                            return Some(Ok(StreamChunk {
                                text: text.to_string(),
                                token_count: Some(1),
                            }));
                        }
                    }
                }
                "message_delta" => {
                    // Final usage with output_tokens
                    if let Some(usage) = parsed.get("usage") {
                        let output_tokens = usage["output_tokens"].as_u64().unwrap_or(0) as usize;
                        if let Ok(mut cell) = self.usage_cell.lock() {
                            if let Some(ref mut existing) = *cell {
                                existing.output_tokens = output_tokens;
                            } else {
                                *cell = Some(StreamUsage {
                                    input_tokens: 0,
                                    output_tokens,
                                });
                            }
                        }
                    }
                }
                "message_start" => {
                    if let Some(usage) = parsed.get("message").and_then(|m| m.get("usage")) {
                        let input_tokens = usage["input_tokens"].as_u64().unwrap_or(0) as usize;
                        if let Ok(mut cell) = self.usage_cell.lock() {
                            *cell = Some(StreamUsage {
                                input_tokens,
                                output_tokens: 0,
                            });
                        }
                    }
                }
                "message_stop" => {
                    self.finished = true;
                    return None;
                }
                "error" => {
                    let error_msg = parsed["error"]["message"]
                        .as_str()
                        .unwrap_or("unknown streaming error");
                    self.finished = true;
                    return Some(Err(AgentError::SupervisorError(format!(
                        "claude stream error: {error_msg}"
                    ))));
                }
                _ => continue,
            }
        }
    }
}
