use nexus_kernel::errors::AgentError;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::process::{Command, Stdio};
use std::sync::Arc;

pub mod claude;
pub mod cohere;
pub mod deepseek;
pub mod fireworks;
pub mod flash;
pub mod gemini;
pub mod groq;
#[cfg(feature = "local-slm")]
pub mod local_slm;
pub mod mistral;
pub mod mock;
pub mod nvidia;
pub mod ollama;
pub mod openai;
pub mod openai_compatible;
pub mod openrouter;
pub mod perplexity;
pub mod together;

pub mod claude_code;
pub mod codex_cli;
pub use claude::ClaudeProvider;
pub use claude_code::ClaudeCodeProvider;
pub use codex_cli::CodexCliProvider;
pub use cohere::CohereProvider;
pub use deepseek::DeepSeekProvider;
pub use fireworks::FireworksProvider;
pub use flash::FlashProvider;
pub use gemini::GeminiProvider;
pub use groq::GroqProvider;
#[cfg(feature = "local-slm")]
pub use local_slm::LocalSlmProvider;
pub use mistral::MistralProvider;
pub use mock::MockProvider;
pub use nvidia::NvidiaProvider;
pub use ollama::OllamaProvider;
pub use openai::OpenAiProvider;
pub use openrouter::OpenRouterProvider;
pub use perplexity::PerplexityProvider;
pub use together::TogetherProvider;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderRequest {
    pub endpoint: String,
    pub headers: BTreeMap<String, String>,
    pub body: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LlmResponse {
    pub output_text: String,
    pub token_count: u32,
    pub model_name: String,
    pub tool_calls: Vec<String>,
    /// Input token count from the API response (if available).
    #[serde(default)]
    pub input_tokens: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EmbeddingResponse {
    pub embeddings: Vec<Vec<f32>>,
    pub model_name: String,
    pub token_count: u32,
}

pub trait LlmProvider: Send + Sync {
    fn query(&self, prompt: &str, max_tokens: u32, model: &str) -> Result<LlmResponse, AgentError>;
    fn name(&self) -> &str;
    fn cost_per_token(&self) -> f64;

    fn is_paid(&self) -> bool {
        self.cost_per_token() > 0.0
    }

    fn estimate_input_tokens(&self, prompt: &str) -> u32 {
        // Lightweight approximation for gating/cost checks.
        let chars = prompt.chars().count();
        u32::try_from(chars.saturating_div(4).saturating_add(1)).unwrap_or(u32::MAX)
    }

    /// The base URL this provider calls. Used by egress governor for allowlisting.
    fn endpoint_url(&self) -> String {
        format!("provider://{}", self.name())
    }

    /// Generate embeddings for the given texts. Returns one vector per input text.
    /// Default implementation returns an error — providers must opt in.
    fn embed(&self, _texts: &[&str], _model: &str) -> Result<EmbeddingResponse, AgentError> {
        Err(AgentError::SupervisorError(format!(
            "{} does not support embeddings",
            self.name()
        )))
    }
}

impl<T: LlmProvider + ?Sized> LlmProvider for Box<T> {
    fn query(&self, prompt: &str, max_tokens: u32, model: &str) -> Result<LlmResponse, AgentError> {
        (**self).query(prompt, max_tokens, model)
    }

    fn name(&self) -> &str {
        (**self).name()
    }

    fn cost_per_token(&self) -> f64 {
        (**self).cost_per_token()
    }

    fn is_paid(&self) -> bool {
        (**self).is_paid()
    }

    fn estimate_input_tokens(&self, prompt: &str) -> u32 {
        (**self).estimate_input_tokens(prompt)
    }

    fn endpoint_url(&self) -> String {
        (**self).endpoint_url()
    }

    fn embed(&self, texts: &[&str], model: &str) -> Result<EmbeddingResponse, AgentError> {
        (**self).embed(texts, model)
    }
}

impl<T: LlmProvider + ?Sized> LlmProvider for Arc<T> {
    fn query(&self, prompt: &str, max_tokens: u32, model: &str) -> Result<LlmResponse, AgentError> {
        (**self).query(prompt, max_tokens, model)
    }

    fn name(&self) -> &str {
        (**self).name()
    }

    fn cost_per_token(&self) -> f64 {
        (**self).cost_per_token()
    }

    fn is_paid(&self) -> bool {
        (**self).is_paid()
    }

    fn estimate_input_tokens(&self, prompt: &str) -> u32 {
        (**self).estimate_input_tokens(prompt)
    }

    fn endpoint_url(&self) -> String {
        (**self).endpoint_url()
    }

    fn embed(&self, texts: &[&str], model: &str) -> Result<EmbeddingResponse, AgentError> {
        (**self).embed(texts, model)
    }
}

pub(crate) fn curl_get_status(endpoint: &str) -> Result<u16, AgentError> {
    eprintln!("[nexus-llm][governance] curl_get_status endpoint={endpoint}");
    let output = Command::new("curl")
        .args([
            "-sS",
            "-L",
            "-m",
            "5",
            "-o",
            "/dev/null",
            "-w",
            "%{http_code}",
        ])
        .arg(endpoint)
        .output()
        .map_err(|error| AgentError::SupervisorError(format!("curl execution failed: {error}")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout_preview = String::from_utf8_lossy(&output.stdout);
        let stdout_short = if stdout_preview.len() > 300 {
            &stdout_preview[..300]
        } else {
            &stdout_preview
        };
        return Err(AgentError::SupervisorError(format!(
            "curl request failed (exit {:?}): stderr={}, response={}",
            output.status.code(),
            stderr.trim(),
            stdout_short.trim()
        )));
    }
    let status_raw = String::from_utf8_lossy(&output.stdout);
    status_raw.trim().parse::<u16>().map_err(|error| {
        AgentError::SupervisorError(format!("invalid HTTP status from curl: {error}"))
    })
}

pub(crate) fn curl_post_json(
    endpoint: &str,
    headers: &BTreeMap<String, String>,
    body: &Value,
) -> Result<(u16, Value), AgentError> {
    curl_post_json_with_timeout(endpoint, headers, body, 20)
}

pub(crate) fn curl_post_json_with_timeout(
    endpoint: &str,
    headers: &BTreeMap<String, String>,
    body: &Value,
    timeout_secs: u32,
) -> Result<(u16, Value), AgentError> {
    eprintln!("[nexus-llm][governance] curl_post_json endpoint={endpoint} timeout={timeout_secs}s");
    let marker = "__NEXUS_STATUS__:";
    let encoded_body = serde_json::to_string(body).map_err(|error| {
        AgentError::SupervisorError(format!("failed to encode request body: {error}"))
    })?;

    let timeout_str = timeout_secs.to_string();
    let mut command = Command::new("curl");
    command.args(["-sS", "-L", "-m", &timeout_str]);
    for (header_name, header_value) in headers {
        command
            .arg("-H")
            .arg(format!("{header_name}: {header_value}"));
    }
    command
        .arg("-d")
        .arg("@-")
        .arg("-w")
        .arg(format!("\n{marker}%{{http_code}}"))
        .arg(endpoint)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = command
        .spawn()
        .map_err(|error| AgentError::SupervisorError(format!("curl execution failed: {error}")))?;

    // Pipe body via stdin to avoid OS ARG_MAX limits with large payloads (e.g. base64 images)
    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        stdin.write_all(encoded_body.as_bytes()).map_err(|error| {
            AgentError::SupervisorError(format!("failed to write body to curl stdin: {error}"))
        })?;
    }

    let output = child
        .wait_with_output()
        .map_err(|error| AgentError::SupervisorError(format!("curl execution failed: {error}")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout_preview = String::from_utf8_lossy(&output.stdout);
        let stdout_short = if stdout_preview.len() > 300 {
            &stdout_preview[..300]
        } else {
            &stdout_preview
        };
        return Err(AgentError::SupervisorError(format!(
            "curl request failed (exit {:?}): stderr={}, response={}",
            output.status.code(),
            stderr.trim(),
            stdout_short.trim()
        )));
    }

    let raw = String::from_utf8(output.stdout).map_err(|error| {
        AgentError::SupervisorError(format!("response was not valid UTF-8: {error}"))
    })?;
    let (body_raw, status_raw) = raw.rsplit_once(marker).ok_or_else(|| {
        AgentError::SupervisorError("missing status marker in curl response".to_string())
    })?;
    let status = status_raw.trim().parse::<u16>().map_err(|error| {
        AgentError::SupervisorError(format!("invalid HTTP status from curl: {error}"))
    })?;
    let trimmed_body = body_raw.trim();
    let response_json = if trimmed_body.is_empty() {
        Value::Null
    } else {
        serde_json::from_str::<Value>(trimmed_body).map_err(|error| {
            // Show first 200 chars of the raw response for debugging
            let preview = if trimmed_body.len() > 200 {
                format!("{}...", &trimmed_body[..200])
            } else {
                trimmed_body.to_string()
            };
            AgentError::SupervisorError(format!(
                "failed to parse JSON response: {error}\nRaw response (first 200 chars): {preview}"
            ))
        })?
    };
    Ok((status, response_json))
}

#[cfg(test)]
mod tests {
    use super::{
        ClaudeProvider, CohereProvider, DeepSeekProvider, FireworksProvider, GeminiProvider,
        GroqProvider, LlmProvider, MistralProvider, NvidiaProvider, OllamaProvider, OpenAiProvider,
        OpenRouterProvider, PerplexityProvider, TogetherProvider,
    };
    use serde_json::json;

    #[test]
    fn test_claude_request_format() {
        let provider = ClaudeProvider::new(Some("test-key".to_string()));
        let request = provider.build_request("Summarize this.", 128, "claude-sonnet-4-5");

        assert_eq!(request.endpoint, "https://api.anthropic.com/v1/messages");
        assert_eq!(
            request.headers.get("x-api-key").map(String::as_str),
            Some("test-key")
        );
        assert_eq!(
            request.headers.get("anthropic-version").map(String::as_str),
            Some("2023-06-01")
        );
        assert_eq!(
            request.body,
            json!({
                "model": "claude-sonnet-4-5",
                "max_tokens": 128,
                "messages": [
                    {
                        "role": "user",
                        "content": "Summarize this."
                    }
                ]
            })
        );
    }

    #[test]
    fn test_deepseek_request_format() {
        let provider = DeepSeekProvider::new(Some("deepseek-key".to_string()));
        let request = provider.build_request("Lower cost option", 96, "deepseek-chat");

        assert_eq!(
            request.endpoint,
            "https://api.deepseek.com/v1/chat/completions"
        );
        assert_eq!(
            request.headers.get("authorization").map(String::as_str),
            Some("Bearer deepseek-key")
        );
        assert_eq!(
            request.body,
            json!({
                "model": "deepseek-chat",
                "messages": [
                    {
                        "role": "user",
                        "content": "Lower cost option"
                    }
                ],
                "max_tokens": 96
            })
        );
    }

    #[test]
    fn test_ollama_request_format() {
        let provider = OllamaProvider::new("http://localhost:11434");
        let request = provider.build_request("hello local model", 64, "llama3");

        assert_eq!(request.endpoint, "http://localhost:11434/api/generate");
        assert_eq!(
            request.body,
            json!({
                "model": "llama3",
                "prompt": "hello local model",
                "stream": false,
                "options": {
                    "num_predict": 64,
                    "num_ctx": 8192,
                    "temperature": 0.2,
                    "top_p": 0.9,
                    "repeat_penalty": 1.1
                }
            })
        );
    }

    #[test]
    fn test_openai_request_format() {
        let provider = OpenAiProvider::new(Some("sk-openai-test".to_string()));
        let request = provider.build_request("Hello world", 64, "gpt-4o");

        assert_eq!(
            request.endpoint,
            "https://api.openai.com/v1/chat/completions"
        );
        assert_eq!(
            request.headers.get("authorization").map(String::as_str),
            Some("Bearer sk-openai-test")
        );
        assert_eq!(
            request.body,
            json!({
                "model": "gpt-4o",
                "messages": [{"role": "user", "content": "Hello world"}],
                "max_tokens": 64
            })
        );
    }

    #[test]
    fn test_openai_provider_traits() {
        let provider = OpenAiProvider::new(Some("key".to_string()));
        assert_eq!(provider.name(), "openai");
        assert!(provider.cost_per_token() > 0.0);
        assert!(provider.is_paid());
    }

    #[test]
    fn test_gemini_request_format() {
        let provider = GeminiProvider::new(Some("gemini-key-test".to_string()));
        let request = provider.build_request("Explain rust", 96, "gemini-2.0-flash");

        assert!(request
            .endpoint
            .contains("generativelanguage.googleapis.com"));
        assert_eq!(
            request.headers.get("authorization").map(String::as_str),
            Some("Bearer gemini-key-test")
        );
        assert_eq!(
            request.body,
            json!({
                "model": "gemini-2.0-flash",
                "messages": [{"role": "user", "content": "Explain rust"}],
                "max_tokens": 96
            })
        );
    }

    #[test]
    fn test_gemini_provider_traits() {
        let provider = GeminiProvider::new(Some("key".to_string()));
        assert_eq!(provider.name(), "gemini");
        assert!(provider.cost_per_token() > 0.0);
        assert!(provider.is_paid());
    }

    #[test]
    fn test_openai_custom_endpoint() {
        let provider = OpenAiProvider::with_endpoint(
            Some("key".to_string()),
            "http://my-proxy.local/v1/chat/completions".to_string(),
        );
        let request = provider.build_request("test", 32, "local-model");
        assert_eq!(
            request.endpoint,
            "http://my-proxy.local/v1/chat/completions"
        );
    }

    #[test]
    fn test_groq_request_format() {
        let provider = GroqProvider::new(Some("groq-key".to_string()));
        let request = provider.build_request("Fast response", 32, "llama-3.3-70b-versatile");

        assert_eq!(
            request.endpoint,
            "https://api.groq.com/openai/v1/chat/completions"
        );
        assert_eq!(
            request.headers.get("authorization").map(String::as_str),
            Some("Bearer groq-key")
        );
        assert_eq!(
            request.body,
            json!({
                "model": "llama-3.3-70b-versatile",
                "messages": [{"role": "user", "content": "Fast response"}],
                "max_tokens": 32
            })
        );
    }

    #[test]
    fn test_mistral_request_format() {
        let provider = MistralProvider::new(Some("mistral-key".to_string()));
        let request = provider.build_request("Reason carefully", 128, "mistral-large-latest");

        assert_eq!(
            request.endpoint,
            "https://api.mistral.ai/v1/chat/completions"
        );
        assert_eq!(
            request.headers.get("authorization").map(String::as_str),
            Some("Bearer mistral-key")
        );
        assert_eq!(request.body["model"], "mistral-large-latest");
    }

    #[test]
    fn test_together_request_format() {
        let provider = TogetherProvider::new(Some("together-key".to_string()));
        let request = provider.build_request(
            "Open weights",
            48,
            "meta-llama/Llama-3.3-70B-Instruct-Turbo",
        );

        assert_eq!(
            request.endpoint,
            "https://api.together.xyz/v1/chat/completions"
        );
        assert_eq!(
            request.headers.get("authorization").map(String::as_str),
            Some("Bearer together-key")
        );
        assert_eq!(request.body["max_tokens"], 48);
    }

    #[test]
    fn test_fireworks_request_format() {
        let provider = FireworksProvider::new(Some("fireworks-key".to_string()));
        let request = provider.build_request(
            "Serve fast",
            72,
            "accounts/fireworks/models/llama-v3p1-70b-instruct",
        );

        assert_eq!(
            request.endpoint,
            "https://api.fireworks.ai/inference/v1/chat/completions"
        );
        assert_eq!(
            request.headers.get("authorization").map(String::as_str),
            Some("Bearer fireworks-key")
        );
        assert_eq!(request.body["max_tokens"], 72);
    }

    #[test]
    fn test_perplexity_request_format() {
        let provider = PerplexityProvider::new(Some("pplx-key".to_string()));
        let request = provider.build_request("Search the web", 60, "sonar-pro");

        assert_eq!(
            request.endpoint,
            "https://api.perplexity.ai/chat/completions"
        );
        assert_eq!(
            request.headers.get("authorization").map(String::as_str),
            Some("Bearer pplx-key")
        );
        assert_eq!(request.body["model"], "sonar-pro");
    }

    #[test]
    fn test_cohere_request_format() {
        let provider = CohereProvider::new(Some("cohere-key".to_string()));
        let request = provider.build_request("Narrate this", 80, "command-r-plus");

        assert_eq!(request.endpoint, "https://api.cohere.ai/v2/chat");
        assert_eq!(
            request.headers.get("authorization").map(String::as_str),
            Some("Bearer cohere-key")
        );
        assert_eq!(
            request.body,
            json!({
                "model": "command-r-plus",
                "message": "Narrate this",
                "max_tokens": 80
            })
        );
    }

    #[test]
    fn test_openrouter_request_format() {
        let provider = OpenRouterProvider::new(Some("openrouter-key".to_string()));
        let request = provider.build_request("Route this", 40, "openai/gpt-4o-mini");

        assert_eq!(
            request.endpoint,
            "https://openrouter.ai/api/v1/chat/completions"
        );
        assert_eq!(
            request.headers.get("authorization").map(String::as_str),
            Some("Bearer openrouter-key")
        );
        assert_eq!(request.body["model"], "openai/gpt-4o-mini");
    }

    #[test]
    fn test_mock_embedding_deterministic() {
        let provider = super::MockProvider::new();
        let result_a = provider.embed(&["hello world"], "mock-embed").unwrap();
        let result_b = provider.embed(&["hello world"], "mock-embed").unwrap();
        assert_eq!(result_a.embeddings[0], result_b.embeddings[0]);
    }

    #[test]
    fn test_mock_embedding_different_texts() {
        let provider = super::MockProvider::new();
        let result = provider
            .embed(&["hello world", "goodbye world"], "mock-embed")
            .unwrap();
        assert_eq!(result.embeddings.len(), 2);
        assert_ne!(result.embeddings[0], result.embeddings[1]);
    }

    #[test]
    fn test_mock_embedding_normalized() {
        let provider = super::MockProvider::new();
        let result = provider
            .embed(&["test normalization"], "mock-embed")
            .unwrap();
        let vec = &result.embeddings[0];
        let norm: f32 = vec.iter().map(|v| v * v).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-5, "expected unit norm, got {norm}");
    }

    #[test]
    fn test_mock_embedding_dimensions() {
        let provider = super::MockProvider::new();
        let result = provider.embed(&["dimension check"], "mock-embed").unwrap();
        assert_eq!(result.embeddings[0].len(), 384);
    }

    #[test]
    fn test_embedding_default_returns_error() {
        let provider = ClaudeProvider::new(Some("key".to_string()));
        let result = provider.embed(&["test"], "claude-sonnet-4-5");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("does not support embeddings"), "got: {err}");
    }

    #[test]
    fn test_nvidia_request_format() {
        let provider = NvidiaProvider::new(Some("nvapi-test-key".to_string()));
        let request = provider.build_request("Hello NIM", 64, "meta/llama-3.3-70b-instruct");

        assert_eq!(
            request.endpoint,
            "https://integrate.api.nvidia.com/v1/chat/completions"
        );
        assert_eq!(
            request.headers.get("authorization").map(String::as_str),
            Some("Bearer nvapi-test-key")
        );
        assert_eq!(
            request.body,
            json!({
                "model": "meta/llama-3.3-70b-instruct",
                "messages": [{"role": "user", "content": "Hello NIM"}],
                "max_tokens": 64
            })
        );
    }

    #[test]
    fn test_nvidia_provider_traits() {
        let provider = NvidiaProvider::new(Some("key".to_string()));
        assert_eq!(provider.name(), "nvidia");
        assert!(provider.cost_per_token() > 0.0);
        assert_eq!(provider.endpoint_url(), "https://integrate.api.nvidia.com");
    }

    #[test]
    fn test_nvidia_model_list_not_empty() {
        assert_eq!(super::nvidia::NVIDIA_MODELS.len(), 93);
    }

    #[test]
    fn test_nvidia_default_model_in_list() {
        use crate::gateway::{NIM_FALLBACK_MODEL, NIM_PRIMARY_MODEL, NIM_SECONDARY_MODEL};
        for model in [NIM_PRIMARY_MODEL, NIM_SECONDARY_MODEL, NIM_FALLBACK_MODEL] {
            assert!(
                super::nvidia::NVIDIA_MODELS
                    .iter()
                    .any(|(id, _)| *id == model),
                "recommended NIM model {model} not in NVIDIA_MODELS catalog"
            );
        }
    }

    #[test]
    fn test_nvidia_vision_model_list() {
        assert_eq!(super::nvidia::NVIDIA_VISION_MODELS.len(), 8);
        assert!(super::nvidia::NVIDIA_VISION_MODELS.contains(&"meta/llama-3.2-90b-vision-instruct"));
    }
}
