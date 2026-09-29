use async_trait::async_trait;
use std::time::Duration;
use tokio::sync::mpsc;

use crate::error::NxError;
use crate::llm::provider::LlmProvider;
use crate::llm::streaming::parse_openai_sse_stream;
use crate::llm::types::{LlmRequest, LlmResponse, Role, StreamChunk, TokenUsage};

/// Connecting to the provider may take at most this long.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

/// No wait for a response's headers, and no read of its body (a stream's
/// included), may take longer than this. A non-streaming answer arrives only
/// when it is complete, so this also bounds its generation.
const READ_TIMEOUT: Duration = Duration::from_secs(600);

/// A non-streaming completion ends within this, from connecting until its
/// last byte. (A stream may run longer; each of its reads is bounded.)
const COMPLETE_TIMEOUT: Duration = Duration::from_secs(600);

/// The provider's client (Final Gate items B and C): bounded as above, with
/// reads bounded by `read_timeout`, and following no redirect. reqwest drops
/// the bearer token on a redirect to another host, but on 307 and 308 it
/// would re-send the prompt there; a redirect is reported as its status
/// instead. Like `Client::new()` before, building it panics only when no TLS
/// backend can be initialized.
fn bounded_client(read_timeout: Duration) -> reqwest::Client {
    reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .read_timeout(read_timeout)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("failed to initialize the HTTP client")
}

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
            client: bounded_client(READ_TIMEOUT),
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
        let response = self
            .build_http_request(&body)?
            .timeout(COMPLETE_TIMEOUT)
            .send()
            .await?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::types::Message;
    use std::io::{Read, Write};

    fn request() -> LlmRequest {
        LlmRequest {
            messages: vec![Message {
                role: Role::User,
                content: "fg prompt".to_string(),
            }],
            model: "m".to_string(),
            max_tokens: 16,
            temperature: None,
            system: None,
            tools: None,
            stream: false,
        }
    }

    /// A provider for `base` (no API key is read: the variable is unset and
    /// none is required) whose reads wait at most `read_timeout`.
    fn stand_in_provider(base: &str, read_timeout: Duration) -> OpenAiCompatibleProvider {
        let mut provider = OpenAiCompatibleProvider::new(
            "fg",
            base,
            "NEXUS_FG_UNSET_API_KEY",
            "m",
            vec![],
            vec!["m".to_string()],
            false,
        );
        provider.client = bounded_client(read_timeout);
        provider
    }

    /// Serve `connections` loopback requests: read each request whole, answer
    /// it with `answer` (nothing when it is empty) and keep the connection
    /// open until the returned sender is dropped. Returns the base URL.
    fn serve(
        connections: usize,
        answer: String,
    ) -> (
        String,
        std::sync::mpsc::Sender<()>,
        std::thread::JoinHandle<()>,
    ) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let (release, released) = std::sync::mpsc::channel::<()>();
        let server = std::thread::spawn(move || {
            let mut open = Vec::new();
            for _ in 0..connections {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(30)))
                    .unwrap();
                let mut head = Vec::new();
                let mut byte = [0u8; 1];
                while !head.ends_with(b"\r\n\r\n") && matches!(stream.read(&mut byte), Ok(1)) {
                    head.push(byte[0]);
                }
                let length = String::from_utf8_lossy(&head)
                    .to_ascii_lowercase()
                    .lines()
                    .find_map(|line| line.strip_prefix("content-length:").map(str::to_string))
                    .and_then(|value| value.trim().parse::<usize>().ok())
                    .unwrap_or(0);
                let mut body = vec![0u8; length];
                stream.read_exact(&mut body).unwrap();
                stream.write_all(answer.as_bytes()).unwrap();
                open.push(stream);
            }
            // Returns when the test drops the sender.
            let _ = released.recv();
        });
        (base, release, server)
    }

    /// Final Gate items B and C: the calls follow no redirect, so the prompt
    /// (which reqwest re-sends on 307 and 308) never reaches the redirect
    /// target, which is never contacted; the redirect is reported as its
    /// status.
    #[tokio::test]
    async fn p0_fg_openai_compatible_calls_follow_no_redirect() {
        let target = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        target.set_nonblocking(true).unwrap();
        let location = format!("http://{}/chat/completions", target.local_addr().unwrap());
        for code in [
            "301 Moved Permanently",
            "302 Found",
            "307 Temporary Redirect",
            "308 Permanent Redirect",
        ] {
            let answer = format!(
                "HTTP/1.1 {code}\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            );
            let (base, release, server) = serve(2, answer);
            // A short read timeout: a followed redirect to the silent target
            // fails in seconds rather than after READ_TIMEOUT.
            let provider = stand_in_provider(&base, Duration::from_secs(5));
            let complete = provider.complete(&request()).await.unwrap_err().to_string();
            assert!(
                complete.contains(&format!("HTTP {}", &code[..3])),
                "{complete}"
            );
            let streamed = provider
                .stream_raw(&request())
                .await
                .unwrap_err()
                .to_string();
            assert!(
                streamed.contains(&format!("HTTP {}", &code[..3])),
                "{streamed}"
            );
            drop(release);
            server.join().unwrap();
        }
        assert!(matches!(
            target.accept(),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock
        ));
    }

    /// The client's read timeout bounds a server that never answers and a
    /// stream that stalls after its head (here 1 s instead of 600 s).
    #[tokio::test]
    async fn p0_fg_openai_compatible_reads_are_bounded() {
        let (base, release, server) = serve(1, String::new());
        let provider = stand_in_provider(&base, Duration::from_secs(1));
        let started = std::time::Instant::now();
        assert!(provider.complete(&request()).await.is_err());
        assert!(started.elapsed() < Duration::from_secs(20));
        drop(release);
        server.join().unwrap();

        let head = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\n\r\n".to_string();
        let (base, release, server) = serve(1, head);
        let provider = stand_in_provider(&base, Duration::from_secs(1));
        let (tx, _rx) = mpsc::unbounded_channel();
        let started = std::time::Instant::now();
        assert!(provider.stream(&request(), tx).await.is_err());
        assert!(started.elapsed() < Duration::from_secs(20));
        drop(release);
        server.join().unwrap();
    }
}
