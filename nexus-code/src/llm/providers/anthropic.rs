use async_trait::async_trait;
use std::time::Duration;
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

/// Connecting to the API may take at most this long.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

/// No wait for a response's headers, and no read of its body (a stream's
/// included), may take longer than this. A non-streaming answer arrives only
/// when it is complete, so this also bounds its generation.
const READ_TIMEOUT: Duration = Duration::from_secs(600);

/// A non-streaming completion ends within this, from connecting until its
/// last byte. (A stream may run longer; each of its reads is bounded.)
const COMPLETE_TIMEOUT: Duration = Duration::from_secs(600);

impl AnthropicProvider {
    /// Create a new Anthropic provider.
    pub fn new() -> Self {
        let api_key = std::env::var("ANTHROPIC_API_KEY").ok();
        Self::with(api_key, "https://api.anthropic.com", READ_TIMEOUT)
    }

    /// A provider for `base_url` whose client waits at most `read_timeout`
    /// for any read. Final Gate items B and C: the client follows no
    /// redirect, so `x-api-key` reaches only `base_url`. On a redirect to
    /// another host reqwest drops only authorization, cookie,
    /// proxy-authorization and www-authenticate, and would re-send
    /// `x-api-key` (and, on 307 and 308, the prompt) to the target; a
    /// redirect is reported as its status instead. Like `Client::new()`
    /// before, building the client panics only when no TLS backend can be
    /// initialized.
    fn with(api_key: Option<String>, base_url: &str, read_timeout: Duration) -> Self {
        let client = reqwest::Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .read_timeout(read_timeout)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("failed to initialize the HTTP client");
        Self {
            client,
            api_key,
            base_url: base_url.to_string(),
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
            .timeout(COMPLETE_TIMEOUT)
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
            "claude-haiku-4-5-20251001",
        ]
    }

    fn is_configured(&self) -> bool {
        self.api_key.is_some()
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
            model: "claude-haiku-4-5-20251001".to_string(),
            max_tokens: 16,
            temperature: None,
            system: None,
            tools: None,
            stream: false,
        }
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

    /// Final Gate items B and C: the Anthropic calls follow no redirect, so
    /// neither `x-api-key` nor the prompt reaches the redirect target, which
    /// is never contacted; the redirect is reported as its status.
    #[tokio::test]
    async fn p0_fg_anthropic_calls_follow_no_redirect() {
        let target = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        target.set_nonblocking(true).unwrap();
        let location = format!("http://{}/v1/messages", target.local_addr().unwrap());
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
            let provider = AnthropicProvider::with(
                Some("fg-secret".to_string()),
                &base,
                Duration::from_secs(5),
            );
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
    async fn p0_fg_anthropic_reads_are_bounded() {
        let (base, release, server) = serve(1, String::new());
        let provider =
            AnthropicProvider::with(Some("k".to_string()), &base, Duration::from_secs(1));
        let started = std::time::Instant::now();
        assert!(provider.complete(&request()).await.is_err());
        assert!(started.elapsed() < Duration::from_secs(20));
        drop(release);
        server.join().unwrap();

        let head = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\n\r\n".to_string();
        let (base, release, server) = serve(1, head);
        let provider =
            AnthropicProvider::with(Some("k".to_string()), &base, Duration::from_secs(1));
        let (tx, _rx) = mpsc::unbounded_channel();
        let started = std::time::Instant::now();
        assert!(provider.stream(&request(), tx).await.is_err());
        assert!(started.elapsed() < Duration::from_secs(20));
        drop(release);
        server.join().unwrap();
    }
}
