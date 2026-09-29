//! Six provider wrappers over the existing `nexus-connectors-llm` providers,
//! plus one fresh HuggingFace implementation.
//!
//! No Claude CLI — that is an interactive-only integration and explicitly
//! excluded from the autonomous swarm.

pub mod anthropic;
pub mod codex_cli;
pub mod huggingface;
pub mod ollama;
pub mod openai;
pub mod openrouter;

pub use anthropic::AnthropicProvider;
pub use codex_cli::CodexCliProvider;
pub use huggingface::HuggingFaceProvider;
pub use ollama::OllamaSwarmProvider;
pub use openai::OpenAiSwarmProvider;
pub use openrouter::OpenRouterSwarmProvider;

/// The most response bytes a swarm provider reads into memory (Final Gate
/// decision E).
pub(crate) const MAX_RESPONSE_BYTES: usize = 32 * 1024 * 1024;

/// The most bytes of a failed request's body kept for its error.
pub(crate) const MAX_ERROR_BODY_BYTES: usize = 64 * 1024;

/// At most `max` bytes of `resp`'s body. A longer body, declared or
/// streamed without a length, is refused without the rest being read.
/// Errors name no URL.
pub(crate) async fn read_capped(
    mut resp: reqwest::Response,
    max: usize,
) -> Result<Vec<u8>, String> {
    let too_large = || format!("the response is larger than {max} bytes");
    if resp
        .content_length()
        .is_some_and(|length| length > max as u64)
    {
        return Err(too_large());
    }
    let mut body = Vec::new();
    while let Some(chunk) = resp
        .chunk()
        .await
        .map_err(|e| format!("the response could not be read: {}", e.without_url()))?
    {
        if chunk.len() > max - body.len() {
            return Err(too_large());
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

/// At most `max` bytes of a failed request's body, as text, for its error;
/// the rest is not read.
pub(crate) async fn error_text(mut resp: reqwest::Response, max: usize) -> String {
    let mut body = Vec::new();
    while let Ok(Some(chunk)) = resp.chunk().await {
        let room = max - body.len();
        body.extend_from_slice(&chunk[..chunk.len().min(room)]);
        if body.len() == max {
            break;
        }
    }
    String::from_utf8_lossy(&body).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::{InvokeRequest, Provider, ProviderError};
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    /// Answer one loopback request with `answer` (a body without a declared
    /// length, ended by closing the connection).
    fn serve_undeclared(answer: Vec<u8>) -> (String, std::thread::JoinHandle<()>) {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut head = Vec::new();
            let mut byte = [0u8; 1];
            while !head.ends_with(b"\r\n\r\n") && matches!(stream.read(&mut byte), Ok(1)) {
                head.push(byte[0]);
            }
            let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n");
            let _ = stream.write_all(&answer);
        });
        (base, server)
    }

    /// Final Gate decision E: a body is read only up to its cap, whether its
    /// length is declared or not; a body at the cap is read whole, and an
    /// error body is cut to its cap.
    #[tokio::test]
    async fn p0_r1_swarm_bodies_are_read_within_their_cap() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_string("a".repeat(2048)))
            .mount(&mock)
            .await;
        let get = || async { reqwest::get(mock.uri()).await.unwrap() };
        assert_eq!(
            read_capped(get().await, 1024).await,
            Err("the response is larger than 1024 bytes".to_string())
        );
        assert_eq!(read_capped(get().await, 2048).await.unwrap().len(), 2048);
        assert_eq!(error_text(get().await, 100).await, "a".repeat(100));

        let (base, server) = serve_undeclared(vec![b'b'; 2048]);
        let response = reqwest::get(base).await.unwrap();
        assert_eq!(response.content_length(), None);
        assert_eq!(
            read_capped(response, 1024).await,
            Err("the response is larger than 1024 bytes".to_string())
        );
        server.join().unwrap();
    }

    /// Every credentialed swarm provider reads its answer through the capped
    /// reader: an answer one byte past `MAX_RESPONSE_BYTES` is refused as
    /// malformed, not parsed. (The OpenAI, OpenRouter, Anthropic and
    /// Hugging Face providers share the reader; the source check below keeps
    /// them on it.)
    #[tokio::test]
    async fn p0_r1_credentialed_swarm_answers_are_size_bounded() {
        let mock = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string("x".repeat(MAX_RESPONSE_BYTES + 1)),
            )
            .mount(&mock)
            .await;
        let providers: Vec<Box<dyn Provider>> = vec![
            Box::new(openai::OpenAiSwarmProvider::with_base_and_key(
                mock.uri(),
                "k",
            )),
            Box::new(openrouter::OpenRouterSwarmProvider::with_base_and_key(
                mock.uri(),
                "k",
            )),
            Box::new(huggingface::HuggingFaceProvider::with_base_and_token(
                mock.uri(),
                "k",
            )),
        ];
        for provider in providers {
            let error = provider
                .invoke(InvokeRequest {
                    model_id: "m".into(),
                    prompt: "p".into(),
                    max_tokens: 8,
                    temperature: None,
                    metadata: serde_json::Value::Null,
                })
                .await
                .unwrap_err();
            assert!(
                matches!(&error, ProviderError::Malformed(_, reason) if reason.contains("larger than")),
                "{}: {error:?}",
                provider.id()
            );
        }
        let dir = tempfile::tempdir().unwrap();
        let anthropic = anthropic::AnthropicProvider::with(
            mock.uri(),
            Some("k".into()),
            dir.path().join("spend.json"),
            anthropic::HARD_CAP_USD,
        );
        let error = anthropic
            .invoke(InvokeRequest {
                model_id: anthropic::HAIKU_MODEL.into(),
                prompt: "p".into(),
                max_tokens: 8,
                temperature: None,
                metadata: serde_json::Value::Null,
            })
            .await
            .unwrap_err();
        assert!(
            matches!(&error, ProviderError::Malformed(_, reason) if reason.contains("larger than")),
            "anthropic: {error:?}"
        );

        for (file, source) in [
            ("openai.rs", include_str!("openai.rs")),
            ("openrouter.rs", include_str!("openrouter.rs")),
            ("anthropic.rs", include_str!("anthropic.rs")),
            ("huggingface.rs", include_str!("huggingface.rs")),
        ] {
            let source = source.replace("\r\n", "\n");
            let production = source.split("#[cfg(test)]\nmod tests").next().unwrap();
            for unbounded in [".json()", ".text()", ".bytes()"] {
                assert!(!production.contains(unbounded), "{file}: {unbounded}");
            }
        }
    }
}
