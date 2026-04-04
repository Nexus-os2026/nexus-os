use super::{
    curl_get_status, curl_post_json, curl_post_json_with_timeout, EmbeddingResponse, LlmProvider,
    LlmResponse, ProviderRequest,
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
    /// Timeout for streaming requests in seconds (default: 900 = 15 minutes).
    streaming_timeout_secs: u32,
    /// Timeout for non-streaming requests in seconds (default: 300 = 5 minutes).
    request_timeout_secs: u32,
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
            streaming_timeout_secs: 900,
            request_timeout_secs: 300,
        }
    }

    /// Set timeout for streaming requests (chat, pull) in seconds.
    pub fn with_streaming_timeout(mut self, secs: u32) -> Self {
        self.streaming_timeout_secs = secs;
        self
    }

    /// Set timeout for non-streaming requests in seconds.
    pub fn with_request_timeout(mut self, secs: u32) -> Self {
        self.request_timeout_secs = secs;
        self
    }

    pub fn from_env() -> Self {
        let base_url =
            env::var("OLLAMA_URL").unwrap_or_else(|_| "http://localhost:11434".to_string());
        Self::new(base_url)
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn available_vision_models(&self) -> Result<Vec<OllamaModel>, AgentError> {
        Ok(Self::filter_vision_models(self.list_models()?))
    }

    fn filter_vision_models(models: Vec<OllamaModel>) -> Vec<OllamaModel> {
        models
            .into_iter()
            .filter(|model| {
                let lowered = model.name.to_ascii_lowercase();
                lowered.contains("llava")
                    || lowered.contains("vision")
                    || lowered.contains("moondream")
                    || lowered.contains("bakllava")
            })
            .collect()
    }

    fn resolve_vision_model(&self, requested: &str) -> Result<String, AgentError> {
        if !requested.trim().is_empty() {
            return Ok(requested.to_string());
        }

        Self::pick_vision_model_name(&self.available_vision_models()?)
    }

    fn pick_vision_model_name(models: &[OllamaModel]) -> Result<String, AgentError> {
        models
            .iter()
            .map(|model| model.name.clone())
            .next()
            .ok_or_else(|| {
                AgentError::SupervisorError(
                    "No vision model available. Install one with: ollama pull llava".to_string(),
                )
            })
    }

    pub fn build_image_request_body(&self, prompt: &str, image_base64: &str, model: &str) -> Value {
        json!({
            "model": model,
            "stream": false,
            "messages": [{
                "role": "user",
                "content": prompt,
                "images": [image_base64],
            }]
        })
    }

    pub fn query_with_image(
        &self,
        prompt: &str,
        image_base64: &str,
        model: &str,
    ) -> Result<String, AgentError> {
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

        let model_name = self.resolve_vision_model(model)?;
        let endpoint = format!("{}/api/chat", self.base_url.trim_end_matches('/'));
        let mut headers = BTreeMap::new();
        headers.insert("content-type".to_string(), "application/json".to_string());
        let body = self.build_image_request_body(prompt, image_base64, &model_name);
        let (status, payload) =
            curl_post_json_with_timeout(&endpoint, &headers, &body, self.request_timeout_secs)?;
        if !(200..300).contains(&status) {
            return Err(AgentError::SupervisorError(format!(
                "ollama vision request failed with status {status}"
            )));
        }

        payload
            .pointer("/message/content")
            .and_then(Value::as_str)
            .or_else(|| payload.get("response").and_then(Value::as_str))
            .map(str::to_string)
            .filter(|text| !text.trim().is_empty())
            .ok_or_else(|| {
                AgentError::SupervisorError(
                    "ollama vision response missing message content".to_string(),
                )
            })
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
    /// Uses a fast TCP connect probe (200ms timeout) instead of a full HTTP request,
    /// so a dead Ollama is detected in milliseconds, not seconds.
    pub fn health_check(&self) -> Result<bool, AgentError> {
        // Fast TCP probe: try connecting to the port. If Ollama isn't running,
        // this fails in < 1ms instead of the 5s curl timeout.
        let addr = self
            .base_url
            .trim_start_matches("http://")
            .trim_start_matches("https://")
            .trim_end_matches('/');
        let socket_addr: std::net::SocketAddr = addr
            .parse()
            .or_else(|_| {
                // If it's a hostname without port, try with default port
                format!("{addr}:11434")
                    .parse()
                    .or_else(|_| "127.0.0.1:11434".parse())
            })
            .map_err(|e| {
                AgentError::SupervisorError(format!("invalid Ollama address '{addr}': {e}"))
            })?;

        match std::net::TcpStream::connect_timeout(
            &socket_addr,
            std::time::Duration::from_millis(200),
        ) {
            Ok(_) => Ok(true),
            Err(e) => Err(AgentError::SupervisorError(format!(
                "Ollama not reachable at {addr}: {e}"
            ))),
        }
    }

    /// List all locally installed Ollama models.
    pub fn list_models(&self) -> Result<Vec<OllamaModel>, AgentError> {
        let endpoint = self.tags_endpoint();
        let (status, payload) = curl_get_json(&endpoint)?;
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

        let timeout_str = self.streaming_timeout_secs.to_string();
        let mut child = Command::new("curl")
            .args(["-sS", "-N", "-X", "POST", "-m", &timeout_str])
            .arg("-H")
            .arg("content-type: application/json")
            .arg("-d")
            .arg("@-")
            .arg(&endpoint)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| AgentError::SupervisorError(format!("curl spawn failed: {e}")))?;

        // Write the JSON body via stdin to avoid OS argument length limits
        // (base64-encoded images can exceed ARG_MAX).
        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            stdin.write_all(encoded.as_bytes()).map_err(|e| {
                AgentError::SupervisorError(format!("failed to write request body to curl: {e}"))
            })?;
            // stdin is dropped here, closing the pipe so curl proceeds
        }

        let stdout = child
            .stdout
            .take()
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
                    // Panic-safe callback invocation
                    let token_owned = token.to_string();
                    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        on_token(&token_owned);
                    }))
                    .is_err()
                    {
                        eprintln!("warning: on_token callback panicked, continuing stream");
                    }
                }
            }
        }

        // Best-effort: wait for curl child process to avoid zombie processes
        let _ = child.wait();

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

        let timeout_str = self.streaming_timeout_secs.to_string();
        let mut child = Command::new("curl")
            .args(["-sS", "-N", "-X", "POST", "-m", &timeout_str])
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
            .take()
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

        // Best-effort: wait for curl child process to avoid zombie processes
        let _ = child.wait();

        Ok(last_status)
    }
}

/// GET request using curl (returns status + json body).
fn curl_get_json(endpoint: &str) -> Result<(u16, Value), AgentError> {
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
        // Fast check: is Ollama even running? (200ms TCP probe instead of 5s curl)
        if self.health_check().is_err() {
            return Err(AgentError::SupervisorError(format!(
                "Ollama not running at {} — is `ollama serve` started?",
                self.base_url
            )));
        }

        let request = self.build_request(prompt, max_tokens, model);
        let (status, payload) = curl_post_json_with_timeout(
            request.endpoint.as_str(),
            &request.headers,
            &request.body,
            self.request_timeout_secs,
        )?;
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
            input_tokens: None,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vision_request_uses_ollama_images_format() {
        let provider = OllamaProvider::new("http://localhost:11434");
        let body =
            provider.build_image_request_body("What do you see?", "ZmFrZS1pbWFnZQ==", "llava");
        assert_eq!(body["model"], "llava");
        assert_eq!(body["messages"][0]["role"], "user");
        assert_eq!(body["messages"][0]["content"], "What do you see?");
        assert_eq!(body["messages"][0]["images"][0], "ZmFrZS1pbWFnZQ==");
    }

    #[test]
    fn vision_model_error_when_none_available() {
        let err = OllamaProvider::pick_vision_model_name(&[]).unwrap_err();
        assert!(err
            .to_string()
            .contains("No vision model available. Install one with: ollama pull llava"));
    }
}
