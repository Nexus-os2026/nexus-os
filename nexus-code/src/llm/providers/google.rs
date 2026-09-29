use async_trait::async_trait;
use std::time::Duration;
use tokio::sync::mpsc;

use super::openai_compat::{bounded_client, COMPLETE_TIMEOUT, READ_TIMEOUT};

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
        Self::with(
            api_key,
            "https://generativelanguage.googleapis.com/v1beta",
            READ_TIMEOUT,
        )
    }

    /// A provider for `base_url` whose reads wait at most `read_timeout`.
    /// Final Gate items B and C: the client is the bounded one the
    /// OpenAI-compatible providers use (no redirect, bounded connect and
    /// reads), and the key travels only in the `x-goog-api-key` header,
    /// never in the URL, so neither a redirect target, a log of the URL nor
    /// an error that names the URL can carry it.
    fn with(api_key: Option<String>, base_url: &str, read_timeout: Duration) -> Self {
        Self {
            client: bounded_client(read_timeout),
            api_key,
            base_url: base_url.to_string(),
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

/// A request error without the URL (reqwest names it in its errors).
fn url_free(error: reqwest::Error) -> NxError {
    NxError::Http(error.without_url())
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
        let url = format!("{}/models/{}:generateContent", self.base_url, request.model);

        let response = self
            .client
            .post(&url)
            .timeout(COMPLETE_TIMEOUT)
            .header("Content-Type", "application/json")
            .header("x-goog-api-key", api_key)
            .json(&body)
            .send()
            .await
            .map_err(url_free)?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(NxError::ProviderError {
                provider: "google".to_string(),
                message: format!("HTTP {}: {}", status, text),
            });
        }

        let json: serde_json::Value = response.json().await.map_err(url_free)?;

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
            "{}/models/{}:streamGenerateContent?alt=sse",
            self.base_url, request.model
        );

        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("x-goog-api-key", api_key)
            .json(&body)
            .send()
            .await
            .map_err(url_free)?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::types::Message;
    use std::io::{Read, Write};

    const KEY: &str = "fg-google-secret";

    fn request() -> LlmRequest {
        LlmRequest {
            messages: vec![Message {
                role: Role::User,
                content: "fg prompt".to_string(),
            }],
            model: "gemini-2.5-flash".to_string(),
            max_tokens: 16,
            temperature: None,
            system: None,
            tools: None,
            stream: false,
        }
    }

    /// Serve `connections` loopback requests: read each request whole,
    /// record its head, answer it with `answer` (nothing when it is empty)
    /// and keep the connection open until the returned sender is dropped.
    /// The handle returns the recorded request heads.
    fn serve(
        connections: usize,
        answer: String,
    ) -> (
        String,
        std::sync::mpsc::Sender<()>,
        std::thread::JoinHandle<Vec<String>>,
    ) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let (release, released) = std::sync::mpsc::channel::<()>();
        let server = std::thread::spawn(move || {
            let (mut open, mut heads) = (Vec::new(), Vec::new());
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
                let head = String::from_utf8_lossy(&head).into_owned();
                let length = head
                    .to_ascii_lowercase()
                    .lines()
                    .find_map(|line| line.strip_prefix("content-length:").map(str::to_string))
                    .and_then(|value| value.trim().parse::<usize>().ok())
                    .unwrap_or(0);
                let mut body = vec![0u8; length];
                stream.read_exact(&mut body).unwrap();
                stream.write_all(answer.as_bytes()).unwrap();
                heads.push(head);
                open.push(stream);
            }
            let _ = released.recv();
            heads
        });
        (base, release, server)
    }

    /// Final Gate item C: the key travels only in the `x-goog-api-key`
    /// header, never in the request URL.
    #[tokio::test]
    async fn p0_fg_the_google_key_is_a_header_never_part_of_the_url() {
        let body = r#"{"candidates":[{"content":{"parts":[{"text":"ok"}]}}]}"#;
        let answer = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let (base, release, server) = serve(1, answer);
        let provider = GoogleProvider::with(Some(KEY.to_string()), &base, Duration::from_secs(5));
        assert_eq!(provider.complete(&request()).await.unwrap().content, "ok");
        drop(release);
        let heads = server.join().unwrap();
        let request_line = heads[0].lines().next().unwrap();
        assert!(
            request_line.starts_with("POST /models/gemini-2.5-flash:generateContent HTTP/1.1"),
            "{request_line}"
        );
        assert!(!request_line.contains(KEY), "{request_line}");
        assert!(
            heads[0]
                .to_ascii_lowercase()
                .contains(&format!("x-goog-api-key: {KEY}")),
            "{}",
            heads[0]
        );
    }

    /// Redirects are not followed (the target is never contacted), and no
    /// failure (a redirect, a refused connection, a server that never
    /// answers, a stalled stream) names the key or the URL.
    #[tokio::test]
    async fn p0_fg_google_calls_follow_no_redirect_and_errors_name_no_url() {
        let target = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        target.set_nonblocking(true).unwrap();
        let location = format!("http://{}/collect", target.local_addr().unwrap());
        let mut errors = Vec::new();
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
            let provider =
                GoogleProvider::with(Some(KEY.to_string()), &base, Duration::from_secs(5));
            let complete = provider.complete(&request()).await.unwrap_err().to_string();
            assert!(
                complete.contains(&format!("HTTP {}", &code[..3])),
                "{complete}"
            );
            let (tx, _rx) = mpsc::unbounded_channel();
            let streamed = provider
                .stream(&request(), tx)
                .await
                .unwrap_err()
                .to_string();
            assert!(
                streamed.contains(&format!("HTTP {}", &code[..3])),
                "{streamed}"
            );
            errors.extend([complete, streamed]);
            drop(release);
            server.join().unwrap();
        }
        assert!(matches!(
            target.accept(),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock
        ));

        let closed = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap();
        let provider = GoogleProvider::with(
            Some(KEY.to_string()),
            &format!("http://{closed}/v1beta"),
            Duration::from_secs(5),
        );
        errors.push(provider.complete(&request()).await.unwrap_err().to_string());
        let (tx, _rx) = mpsc::unbounded_channel();
        errors.push(
            provider
                .stream(&request(), tx)
                .await
                .unwrap_err()
                .to_string(),
        );

        let (base, release, server) = serve(1, String::new());
        let provider = GoogleProvider::with(Some(KEY.to_string()), &base, Duration::from_secs(1));
        errors.push(provider.complete(&request()).await.unwrap_err().to_string());
        drop(release);
        server.join().unwrap();
        let head = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\n\r\n".to_string();
        let (base, release, server) = serve(1, head);
        let provider = GoogleProvider::with(Some(KEY.to_string()), &base, Duration::from_secs(1));
        let (tx, _rx) = mpsc::unbounded_channel();
        errors.push(
            provider
                .stream(&request(), tx)
                .await
                .unwrap_err()
                .to_string(),
        );
        drop(release);
        server.join().unwrap();

        for error in &errors {
            assert!(!error.contains(KEY), "{error}");
            assert!(!error.contains("generateContent"), "{error}");
            assert!(!error.contains("127.0.0.1"), "{error}");
        }
    }
}
