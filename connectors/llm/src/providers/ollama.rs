use super::{
    curl_get_status, curl_post_json, EmbeddingResponse, LlmProvider, LlmResponse, ProviderRequest,
};
use nexus_kernel::errors::AgentError;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::env;
use std::io::{BufRead, BufReader};
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OllamaProvider {
    base_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OllamaModel {
    pub name: String,
    pub size: u64,
    pub digest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OllamaPullProgress {
    pub status: String,
    pub total: u64,
    pub completed: u64,
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

    pub fn base_url(&self) -> &str {
        &self.base_url
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

    /// Check if Ollama is running and reachable.
    pub fn health_check(&self) -> Result<bool, AgentError> {
        match curl_get_status(self.tags_endpoint().as_str()) {
            Ok(code) if (200..300).contains(&code) => Ok(true),
            Ok(code) => Err(AgentError::SupervisorError(format!(
                "Ollama returned status {code}"
            ))),
            Err(e) => Err(e),
        }
    }

    /// List all locally installed Ollama models.
    pub fn list_models(&self) -> Result<Vec<OllamaModel>, AgentError> {
        let endpoint = self.tags_endpoint();
        let mut headers = BTreeMap::new();
        headers.insert("content-type".to_string(), "application/json".to_string());

        let (status, payload) = curl_post_json_get(&endpoint)?;
        if !(200..300).contains(&status) {
            return Err(AgentError::SupervisorError(format!(
                "Ollama list models failed with status {status}"
            )));
        }

        let models = payload
            .get("models")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| {
                        let name = m.get("name")?.as_str()?.to_string();
                        let size = m.get("size").and_then(Value::as_u64).unwrap_or(0);
                        let digest = m
                            .get("digest")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string();
                        Some(OllamaModel { name, size, digest })
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(models)
    }

    /// Stream a chat completion via Ollama's OpenAI-compatible endpoint.
    /// Uses `/v1/chat/completions` with SSE streaming.
    /// The `on_token` callback receives each text chunk as it arrives.
    /// Returns the full accumulated response text.
    pub fn chat_stream<F>(
        &self,
        messages: &[Value],
        model: &str,
        on_token: F,
    ) -> Result<String, AgentError>
    where
        F: FnMut(&str),
    {
        let endpoint = format!(
            "{}/v1/chat/completions",
            self.base_url.trim_end_matches('/')
        );
        let body = json!({
            "model": model,
            "messages": messages,
            "stream": true,
            "temperature": 0.7,
        });
        let encoded = serde_json::to_string(&body)
            .map_err(|e| AgentError::SupervisorError(format!("failed to encode chat body: {e}")))?;

        let child = Command::new("curl")
            .args(["-sS", "-N", "-X", "POST", "-m", "300"])
            .arg("-H")
            .arg("content-type: application/json")
            .arg("-d")
            .arg(&encoded)
            .arg(&endpoint)
            .stdout(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| AgentError::SupervisorError(format!("curl spawn failed: {e}")))?;

        let stdout = child
            .stdout
            .ok_or_else(|| AgentError::SupervisorError("no stdout from curl".to_string()))?;

        let reader = BufReader::new(stdout);
        let mut full_response = String::new();
        let mut on_token = on_token;

        for line in reader.lines() {
            let line = line
                .map_err(|e| AgentError::SupervisorError(format!("read error during chat: {e}")))?;
            let trimmed = line.trim();
            if trimmed.is_empty() || !trimmed.starts_with("data: ") {
                continue;
            }
            let data = &trimmed[6..];
            if data == "[DONE]" {
                break;
            }
            if let Ok(obj) = serde_json::from_str::<Value>(data) {
                if let Some(token) = obj
                    .pointer("/choices/0/delta/content")
                    .and_then(Value::as_str)
                {
                    full_response.push_str(token);
                    on_token(token);
                }
            }
        }

        Ok(full_response)
    }

    /// Pull a model from Ollama registry. Returns final status.
    /// The `on_progress` callback is called with (status, completed_bytes, total_bytes).
    pub fn pull_model<F>(&self, model_name: &str, mut on_progress: F) -> Result<String, AgentError>
    where
        F: FnMut(&str, u64, u64),
    {
        let endpoint = format!("{}/api/pull", self.base_url.trim_end_matches('/'));
        let body = json!({ "name": model_name, "stream": true });
        let encoded_body = serde_json::to_string(&body)
            .map_err(|e| AgentError::SupervisorError(format!("failed to encode pull body: {e}")))?;

        let child = Command::new("curl")
            .args(["-sS", "-N", "-X", "POST"])
            .arg("-H")
            .arg("content-type: application/json")
            .arg("-d")
            .arg(&encoded_body)
            .arg(&endpoint)
            .stdout(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| AgentError::SupervisorError(format!("curl spawn failed: {e}")))?;

        let stdout = child
            .stdout
            .ok_or_else(|| AgentError::SupervisorError("no stdout from curl".to_string()))?;

        let reader = BufReader::new(stdout);
        let mut last_status = "unknown".to_string();

        for line in reader.lines() {
            let line = line
                .map_err(|e| AgentError::SupervisorError(format!("read error during pull: {e}")))?;
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(obj) = serde_json::from_str::<Value>(&line) {
                let status = obj
                    .get("status")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown");
                let total = obj.get("total").and_then(Value::as_u64).unwrap_or(0);
                let completed = obj.get("completed").and_then(Value::as_u64).unwrap_or(0);
                last_status = status.to_string();
                on_progress(status, completed, total);
            }
        }

        Ok(last_status)
    }
}

/// GET request using curl (returns status + json body).
fn curl_post_json_get(endpoint: &str) -> Result<(u16, Value), AgentError> {
    let marker = "__NEXUS_STATUS__:";
    let output = Command::new("curl")
        .args(["-sS", "-L", "-m", "10"])
        .arg("-w")
        .arg(format!("\n{marker}%{{http_code}}"))
        .arg(endpoint)
        .output()
        .map_err(|e| AgentError::SupervisorError(format!("curl execution failed: {e}")))?;

    if !output.status.success() {
        return Err(AgentError::SupervisorError(
            "curl request failed".to_string(),
        ));
    }

    let raw = String::from_utf8(output.stdout)
        .map_err(|e| AgentError::SupervisorError(format!("response not utf-8: {e}")))?;

    let (body_raw, status_raw) = raw.rsplit_once(marker).ok_or_else(|| {
        AgentError::SupervisorError("missing status marker in curl response".to_string())
    })?;

    let status = status_raw
        .trim()
        .parse::<u16>()
        .map_err(|e| AgentError::SupervisorError(format!("invalid HTTP status: {e}")))?;

    let response_json = if body_raw.trim().is_empty() {
        Value::Null
    } else {
        serde_json::from_str::<Value>(body_raw)
            .map_err(|e| AgentError::SupervisorError(format!("failed to parse JSON: {e}")))?
    };

    Ok((status, response_json))
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

    fn endpoint_url(&self) -> String {
        self.base_url.clone()
    }

    fn embed(&self, texts: &[&str], model: &str) -> Result<EmbeddingResponse, AgentError> {
        let endpoint = format!("{}/api/embeddings", self.base_url.trim_end_matches('/'));
        let mut headers = BTreeMap::new();
        headers.insert("content-type".to_string(), "application/json".to_string());

        let mut embeddings = Vec::with_capacity(texts.len());
        let mut total_tokens = 0u32;

        for text in texts {
            let body = json!({
                "model": model,
                "prompt": *text,
            });
            let (status, payload) = curl_post_json(&endpoint, &headers, &body)?;
            if !(200..300).contains(&status) {
                return Err(AgentError::SupervisorError(format!(
                    "ollama embedding request failed with status {status}"
                )));
            }

            let embedding = payload
                .get("embedding")
                .and_then(Value::as_array)
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_f64().map(|f| f as f32))
                        .collect::<Vec<f32>>()
                })
                .ok_or_else(|| {
                    AgentError::SupervisorError(
                        "ollama embedding response missing 'embedding' field".to_string(),
                    )
                })?;

            let tokens = payload
                .get("prompt_eval_count")
                .and_then(Value::as_u64)
                .and_then(|v| u32::try_from(v).ok())
                .unwrap_or(0);
            total_tokens = total_tokens.saturating_add(tokens);

            embeddings.push(embedding);
        }

        Ok(EmbeddingResponse {
            embeddings,
            model_name: model.to_string(),
            token_count: total_tokens,
        })
    }
}
