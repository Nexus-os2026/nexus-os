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

/// An endpoint built from a configured or caller-supplied base URL, checked
/// as an http(s) URL before it reaches curl, which receives only its
/// normalized spelling after `--` (P0-002C5B).
pub(crate) fn checked_endpoint(endpoint: &str) -> Result<String, AgentError> {
    nexus_kernel::governed_http::http_url(endpoint)
        .map(|url| url.as_str().to_string())
        .map_err(|error| AgentError::SupervisorError(format!("invalid endpoint: {error}")))
}

pub(crate) fn curl_get_status(endpoint: &str) -> Result<u16, AgentError> {
    let endpoint = checked_endpoint(endpoint)?;
    eprintln!("[nexus-llm][governance] curl_get_status endpoint={endpoint}");
    // Final Gate item B: no redirect is followed; the checked address is the
    // one contacted.
    let output = Command::new("curl")
        .args(nexus_kernel::governed_http::CURL_HTTP_ONLY)
        .args([
            "-sS",
            "-m",
            "5",
            "--max-filesize",
            "1048576",
            "-o",
            "/dev/null",
            "-w",
            "%{http_code}",
            "--",
        ])
        .arg(&endpoint)
        .output()
        .map_err(|error| AgentError::SupervisorError(format!("curl execution failed: {error}")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout_preview = String::from_utf8_lossy(&output.stdout);
        let stdout_short = if stdout_preview.len() > 300 {
            &stdout_preview[..stdout_preview.floor_char_boundary(300)]
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

/// Header names that carry a credential (Final Gate item C). Such a header is
/// never placed on a process command line, where other local processes can
/// read it: [`curl_post_json_with_timeout`] refuses it, and
/// [`post_json_in_process`] sends it from this process.
const CREDENTIAL_HEADERS: [&str; 7] = [
    "authorization",
    "proxy-authorization",
    "x-api-key",
    "api-key",
    "x-goog-api-key",
    "x-subscription-token",
    "cookie",
];

/// Whether `name` is a credential-bearing header, in any letter case.
pub(crate) fn is_credential_header(name: &str) -> bool {
    CREDENTIAL_HEADERS
        .iter()
        .any(|credential| credential.eq_ignore_ascii_case(name))
}

/// The curl invocation for a JSON POST whose body is written to stdin, not
/// yet started. A credential-bearing header is refused before anything else
/// (Final Gate item C), and no redirect is followed (item B): the checked
/// address is the one contacted, and nothing is re-sent to a redirect target.
fn curl_post_command(
    endpoint: &str,
    headers: &BTreeMap<String, String>,
    timeout_secs: u32,
    status_marker: &str,
) -> Result<Command, AgentError> {
    if headers.keys().any(|name| is_credential_header(name)) {
        return Err(AgentError::SupervisorError(
            "a credential header is never passed on a process command line".to_string(),
        ));
    }
    let endpoint = checked_endpoint(endpoint)?;
    eprintln!("[nexus-llm][governance] curl_post_json endpoint={endpoint} timeout={timeout_secs}s");
    let timeout_str = timeout_secs.to_string();
    let mut command = Command::new("curl");
    command
        .args(nexus_kernel::governed_http::CURL_HTTP_ONLY)
        .args(["-sS", "-m", &timeout_str, "--max-filesize", "33554432"]);
    for (header_name, header_value) in headers {
        let header = nexus_kernel::governed_http::http_header(header_name, header_value)
            .map_err(|error| AgentError::SupervisorError(error.to_string()))?;
        command.arg("-H").arg(header);
    }
    // The body is backend-serialized JSON on stdin: `@-` is a fixed stdin
    // marker, never caller text.
    command
        .arg("--data-binary")
        .arg("@-")
        .arg("-w")
        .arg(format!("\n{status_marker}%{{http_code}}"))
        .arg("--")
        .arg(&endpoint)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    Ok(command)
}

pub(crate) fn curl_post_json_with_timeout(
    endpoint: &str,
    headers: &BTreeMap<String, String>,
    body: &Value,
    timeout_secs: u32,
) -> Result<(u16, Value), AgentError> {
    let marker = "__NEXUS_STATUS__:";
    let mut command = curl_post_command(endpoint, headers, timeout_secs, marker)?;
    let encoded_body = serde_json::to_string(body).map_err(|error| {
        AgentError::SupervisorError(format!("failed to encode request body: {error}"))
    })?;

    let mut child = command
        .spawn()
        .map_err(|error| AgentError::SupervisorError(format!("curl execution failed: {error}")))?;

    // Pipe body via stdin to avoid OS ARG_MAX limits with large payloads (e.g. base64 images)
    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        if let Err(error) = stdin.write_all(encoded_body.as_bytes()) {
            drop(stdin);
            reap_child(&mut child);
            return Err(AgentError::SupervisorError(format!(
                "failed to write body to curl stdin: {error}"
            )));
        }
    }

    let output = child
        .wait_with_output()
        .map_err(|error| AgentError::SupervisorError(format!("curl execution failed: {error}")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout_preview = String::from_utf8_lossy(&output.stdout);
        let stdout_short = if stdout_preview.len() > 300 {
            &stdout_preview[..stdout_preview.floor_char_boundary(300)]
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
    Ok((status, json_response(body_raw.trim())?))
}

/// Stop and reap a curl child that an early error leaves behind (Final Gate
/// item I), so no request keeps running unowned and no child stays unreaped.
/// Killing a child that already exited is not an error here; the caller
/// reports its own failure either way.
pub(crate) fn reap_child(child: &mut std::process::Child) {
    let _ = child.kill();
    let _ = child.wait();
}

/// A provider's JSON response body: `null` when empty, and otherwise JSON,
/// or an error quoting at most the first 200 characters of the body.
fn json_response(trimmed_body: &str) -> Result<Value, AgentError> {
    if trimmed_body.is_empty() {
        return Ok(Value::Null);
    }
    serde_json::from_str::<Value>(trimmed_body).map_err(|error| {
        // Show first 200 chars of the raw response for debugging
        let preview = if trimmed_body.len() > 200 {
            format!(
                "{}...",
                &trimmed_body[..trimmed_body.floor_char_boundary(200)]
            )
        } else {
            trimmed_body.to_string()
        };
        AgentError::SupervisorError(format!(
            "failed to parse JSON response: {error}\nRaw response (first 200 chars): {preview}"
        ))
    })
}

/// The largest provider response read in process, as for curl (32 MiB).
const MAX_PROVIDER_RESPONSE_BYTES: u64 = 32 * 1024 * 1024;

/// Final Gate item C: POST backend-serialized JSON to a provider endpoint
/// with credential-bearing headers, from this process. No credential is
/// placed on a process command line. The request keeps the curl helper's
/// bounds and adds none of its reach:
/// - the endpoint passes the same http(s) check, and headers the same token
///   and line-break checks; credential header values are marked sensitive;
/// - no redirect is followed, so a credential reaches only the checked
///   endpoint (a redirect response is returned as its status);
/// - `timeout_secs` bounds the whole exchange, and at most 32 MiB of
///   response is read;
/// - TLS certificates are verified;
/// - an error names what failed, never a header value or the URL.
///
/// The request runs on its own thread: the blocking client must not run on
/// an async runtime's thread, and providers are called from those (the
/// agent loop).
pub(crate) fn post_json_in_process(
    endpoint: &str,
    headers: &BTreeMap<String, String>,
    body: &Value,
    timeout_secs: u32,
) -> Result<(u16, Value), AgentError> {
    post_json_bounded(
        endpoint,
        headers,
        body,
        std::time::Duration::from_secs(u64::from(timeout_secs)),
        MAX_PROVIDER_RESPONSE_BYTES,
    )
}

fn post_json_bounded(
    endpoint: &str,
    headers: &BTreeMap<String, String>,
    body: &Value,
    timeout: std::time::Duration,
    max_bytes: u64,
) -> Result<(u16, Value), AgentError> {
    let endpoint = checked_endpoint(endpoint)?;
    let invalid_header = || AgentError::SupervisorError("invalid HTTP header".to_string());
    let mut header_map = reqwest::header::HeaderMap::new();
    for (name, value) in headers {
        nexus_kernel::governed_http::http_header(name, value)
            .map_err(|error| AgentError::SupervisorError(error.to_string()))?;
        let header_name = reqwest::header::HeaderName::from_bytes(name.as_bytes())
            .map_err(|_| invalid_header())?;
        let mut header_value =
            reqwest::header::HeaderValue::from_str(value).map_err(|_| invalid_header())?;
        header_value.set_sensitive(is_credential_header(name));
        header_map.insert(header_name, header_value);
    }
    let encoded = serde_json::to_vec(body).map_err(|error| {
        AgentError::SupervisorError(format!("failed to encode request body: {error}"))
    })?;
    eprintln!(
        "[nexus-llm][governance] post_json endpoint={endpoint} timeout={}s",
        timeout.as_secs()
    );
    let request = std::thread::Builder::new()
        .name("nexus-llm-request".to_string())
        .spawn(move || send_bounded(&endpoint, header_map, encoded, timeout, max_bytes))
        .map_err(|error| {
            AgentError::SupervisorError(format!("failed to start the request: {error}"))
        })?;
    let (status, raw) = request
        .join()
        .map_err(|_| AgentError::SupervisorError("the request ended without a result".to_string()))?
        .map_err(AgentError::SupervisorError)?;
    let text = String::from_utf8(raw).map_err(|error| {
        AgentError::SupervisorError(format!("response was not valid UTF-8: {error}"))
    })?;
    Ok((status, json_response(text.trim())?))
}

/// Send one bounded request (see [`post_json_in_process`]) and read its
/// status and at most `max_bytes` of body.
fn send_bounded(
    endpoint: &str,
    headers: reqwest::header::HeaderMap,
    body: Vec<u8>,
    timeout: std::time::Duration,
    max_bytes: u64,
) -> Result<(u16, Vec<u8>), String> {
    use std::io::Read;
    let client = reqwest::blocking::Client::builder()
        .timeout(timeout)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|error| request_error("could not start", error))?;
    let response = client
        .post(endpoint)
        .headers(headers)
        .body(body)
        .send()
        .map_err(|error| request_error("failed", error))?;
    let status = response.status().as_u16();
    let too_large = || format!("the response is larger than {max_bytes} bytes");
    if response
        .content_length()
        .is_some_and(|length| length > max_bytes)
    {
        return Err(too_large());
    }
    let mut raw = Vec::new();
    response
        .take(max_bytes + 1)
        .read_to_end(&mut raw)
        .map_err(|error| format!("the response could not be read: {}", error.kind()))?;
    if raw.len() as u64 > max_bytes {
        return Err(too_large());
    }
    Ok((status, raw))
}

/// A request failure without the URL (it is dropped from the error) and
/// without any header, with the causes that explain it (a refused
/// connection, a name that did not resolve, a certificate that did not
/// verify).
fn request_error(what: &str, error: reqwest::Error) -> String {
    let kind = if error.is_timeout() {
        " (timed out)"
    } else if error.is_connect() {
        " (could not connect)"
    } else {
        ""
    };
    let error = error.without_url();
    let mut message = format!("the request {what}{kind}: {error}");
    let mut source = std::error::Error::source(&error);
    while let Some(cause) = source {
        message.push_str(&format!(": {cause}"));
        source = cause.source();
    }
    message
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

    #[test]
    fn p0_002c5b_endpoints_are_http_urls_before_curl_runs() {
        use std::collections::BTreeMap;
        for endpoint in [
            "file:///etc/passwd",
            "-K/etc/passwd",
            "@/etc/passwd",
            "gopher://example.com/",
            "https://a@example.com/x",
        ] {
            assert!(super::checked_endpoint(endpoint).is_err(), "{endpoint:?}");
            let status = super::curl_get_status(endpoint).unwrap_err().to_string();
            assert!(
                status.contains("invalid endpoint"),
                "{endpoint:?}: {status}"
            );
            let posted =
                super::curl_post_json_with_timeout(endpoint, &BTreeMap::new(), &json!({}), 1)
                    .unwrap_err()
                    .to_string();
            assert!(
                posted.contains("invalid endpoint"),
                "{endpoint:?}: {posted}"
            );
        }
        let header = BTreeMap::from([("x-a".to_string(), "v\r\nx-b: 1".to_string())]);
        let err = super::curl_post_json_with_timeout("http://127.0.0.1:9/", &header, &json!({}), 1)
            .unwrap_err()
            .to_string();
        assert!(err.contains("invalid HTTP header"), "{err}");
        assert_eq!(
            super::checked_endpoint("http://127.0.0.1:11434/api/tags").unwrap(),
            "http://127.0.0.1:11434/api/tags"
        );
    }

    // ── Final Gate item C: credentials never reach a process command line ──

    use serde_json::Value;
    use std::collections::BTreeMap;
    use std::time::Duration;

    /// A loopback listener that must never be contacted, and its URL.
    fn quiet_listener() -> (std::net::TcpListener, String) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        (listener, base)
    }

    fn assert_never_contacted(listener: &std::net::TcpListener) {
        assert!(matches!(
            listener.accept(),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock
        ));
    }

    /// Answer one loopback request with `answer` (or, when it is empty, hold
    /// the connection unanswered until the client gives up), and return the
    /// request received: its head and body as text.
    fn serve_once(answer: Vec<u8>) -> (String, std::thread::JoinHandle<String>) {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(30)))
                .unwrap();
            let mut request = Vec::new();
            let mut byte = [0u8; 1];
            while !request.ends_with(b"\r\n\r\n") && matches!(stream.read(&mut byte), Ok(1)) {
                request.push(byte[0]);
            }
            let head = String::from_utf8_lossy(&request).to_ascii_lowercase();
            let length = head
                .lines()
                .find_map(|line| line.strip_prefix("content-length:"))
                .and_then(|value| value.trim().parse::<usize>().ok())
                .unwrap_or(0);
            let mut body = vec![0u8; length];
            stream.read_exact(&mut body).unwrap();
            request.extend_from_slice(&body);
            if answer.is_empty() {
                let mut rest = [0u8; 64];
                while matches!(stream.read(&mut rest), Ok(n) if n > 0) {}
            } else {
                let _ = stream.write_all(&answer);
            }
            String::from_utf8_lossy(&request).into_owned()
        });
        (base, handle)
    }

    /// Final Gate item C: the curl helper refuses every credential-bearing
    /// header, in any letter case, before a command exists, so no process
    /// starts and nothing is contacted. The command it builds for other
    /// headers follows no redirect and ends with the checked endpoint after
    /// `--`.
    #[test]
    fn p0_fg_credential_headers_never_reach_a_process_command_line() {
        let (listener, base) = quiet_listener();
        let endpoint = format!("{base}/v1");
        for name in [
            "authorization",
            "Authorization",
            "PROXY-AUTHORIZATION",
            "x-api-key",
            "X-Api-Key",
            "api-key",
            "x-goog-api-key",
            "X-Subscription-Token",
            "cookie",
        ] {
            let headers = BTreeMap::from([(name.to_string(), "fg-secret-value".to_string())]);
            let error = super::curl_post_json_with_timeout(&endpoint, &headers, &json!({}), 1)
                .unwrap_err()
                .to_string();
            assert!(error.contains("credential header"), "{name}: {error}");
            assert!(!error.contains("fg-secret-value"), "{name}: {error}");
            assert!(
                super::curl_post_command(&endpoint, &headers, 1, "M").is_err(),
                "{name}"
            );
        }
        assert_never_contacted(&listener);

        let headers =
            BTreeMap::from([("content-type".to_string(), "application/json".to_string())]);
        let command = super::curl_post_command(&endpoint, &headers, 7, "__M__:").unwrap();
        assert_eq!(command.get_program(), "curl");
        let args: Vec<String> = command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        assert_eq!(args[..6], nexus_kernel::governed_http::CURL_HTTP_ONLY);
        for redirect in ["-L", "--location", "--location-trusted"] {
            assert!(!args.iter().any(|arg| arg == redirect), "{args:?}");
        }
        assert!(args
            .windows(2)
            .any(|pair| pair[0] == "-H" && pair[1] == "content-type: application/json"));
        assert_eq!(args[args.len() - 2..], ["--".to_string(), endpoint.clone()]);
    }

    /// Final Gate item C: a credentialed POST runs in this process: the
    /// credential and the body reach the endpoint, and the status and JSON
    /// come back.
    #[test]
    fn p0_fg_credentialed_posts_run_in_process() {
        let body = r#"{"ok":true}"#;
        let answer = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let (base, server) = serve_once(answer.into_bytes());
        let headers = BTreeMap::from([
            ("authorization".to_string(), "Bearer fg-secret".to_string()),
            ("content-type".to_string(), "application/json".to_string()),
        ]);
        let (status, payload) = super::post_json_in_process(
            &format!("{base}/v1/chat"),
            &headers,
            &json!({"model": "m"}),
            10,
        )
        .unwrap();
        assert_eq!((status, payload), (200, json!({"ok": true})));
        let request = server.join().unwrap();
        assert!(
            request.starts_with("POST /v1/chat HTTP/1.1\r\n"),
            "{request}"
        );
        assert!(
            request
                .to_ascii_lowercase()
                .contains("authorization: bearer fg-secret"),
            "{request}"
        );
        assert!(request.ends_with(r#"{"model":"m"}"#), "{request}");
    }

    /// No redirect is followed, so the credential reaches only the checked
    /// endpoint; the redirect comes back as its status.
    #[test]
    fn p0_fg_credentialed_posts_follow_no_redirect() {
        let (target, target_url) = quiet_listener();
        for code in [
            "301 Moved Permanently",
            "302 Found",
            "307 Temporary Redirect",
            "308 Permanent Redirect",
        ] {
            let answer = format!(
                "HTTP/1.1 {code}\r\nLocation: {target_url}/collect\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            );
            let (base, server) = serve_once(answer.into_bytes());
            let headers =
                BTreeMap::from([("authorization".to_string(), "Bearer fg-secret".to_string())]);
            let (status, payload) =
                super::post_json_in_process(&format!("{base}/v1"), &headers, &json!({}), 10)
                    .unwrap();
            assert_eq!(status.to_string(), code[..3], "{code}");
            assert_eq!(payload, Value::Null);
            server.join().unwrap();
        }
        assert_never_contacted(&target);
    }

    /// The response read is bounded, whether it declares its length or not.
    #[test]
    fn p0_fg_credentialed_posts_read_a_bounded_response() {
        let declared = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: 2048\r\nConnection: close\r\n\r\n{}",
            "a".repeat(2048)
        );
        let chunk = "b".repeat(1000);
        let streamed = format!(
            "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n3e8\r\n{chunk}\r\n3e8\r\n{chunk}\r\n0\r\n\r\n"
        );
        for answer in [declared, streamed] {
            let (base, server) = serve_once(answer.into_bytes());
            let error = super::post_json_bounded(
                &format!("{base}/v1"),
                &BTreeMap::new(),
                &json!({}),
                Duration::from_secs(10),
                1024,
            )
            .unwrap_err()
            .to_string();
            assert!(error.contains("larger than 1024 bytes"), "{error}");
            server.join().unwrap();
        }
    }

    /// A stalled endpoint is abandoned at the timeout, and neither that nor
    /// a refused connection names the credential or the URL.
    #[test]
    fn p0_fg_credentialed_post_failures_are_bounded_and_name_no_secret() {
        let (base, server) = serve_once(Vec::new());
        let headers = BTreeMap::from([("x-api-key".to_string(), "fg-secret".to_string())]);
        let started = std::time::Instant::now();
        let stalled = super::post_json_bounded(
            &format!("{base}/v1/secret-path"),
            &headers,
            &json!({}),
            Duration::from_secs(1),
            1024,
        )
        .unwrap_err()
        .to_string();
        assert!(started.elapsed() < Duration::from_secs(30), "{stalled}");
        assert!(stalled.contains("timed out"), "{stalled}");
        server.join().unwrap();

        let closed = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap();
        let refused = super::post_json_in_process(
            &format!("http://{closed}/v1/secret-path"),
            &headers,
            &json!({}),
            5,
        )
        .unwrap_err()
        .to_string();
        for error in [&stalled, &refused] {
            assert!(!error.contains("fg-secret"), "{error}");
            assert!(!error.contains("secret-path"), "{error}");
        }
    }

    /// The migrated providers post their credential to a constant https
    /// endpoint, in process.
    #[test]
    fn p0_fg_credentialed_providers_post_to_constant_https_endpoints_in_process() {
        let key = || Some("k".to_string());
        for request in [
            OpenAiProvider::new(key()).build_request("p", 1, "m"),
            DeepSeekProvider::new(key()).build_request("p", 1, "m"),
            GeminiProvider::new(key()).build_request("p", 1, "m"),
            NvidiaProvider::new(key()).build_request("p", 1, "m"),
        ] {
            assert!(
                request.endpoint.starts_with("https://"),
                "{}",
                request.endpoint
            );
            assert!(request
                .headers
                .keys()
                .any(|name| super::is_credential_header(name)));
        }
        for (file, source) in [
            ("openai.rs", include_str!("openai.rs")),
            ("deepseek.rs", include_str!("deepseek.rs")),
            ("gemini.rs", include_str!("gemini.rs")),
            ("nvidia.rs", include_str!("nvidia.rs")),
        ] {
            assert!(!source.contains("curl_post_json"), "{file}");
            assert!(source.contains("post_json_in_process("), "{file}");
        }
    }

    /// Final Gate item I: every early error after a curl child starts stops
    /// and reaps it (`reap_child`) instead of returning with the child still
    /// running and unreaped. The pre-repair `?` returns are gone.
    #[test]
    fn p0_fg_curl_children_are_reaped_on_early_errors() {
        let production = |source: &'static str| {
            source
                .split("#[cfg(test)]\nmod tests")
                .next()
                .unwrap()
                .replace("\r\n", "\n")
        };
        let helpers = production(include_str!("mod.rs"));
        let ollama = production(include_str!("ollama.rs"));
        assert_eq!(helpers.matches("reap_child(&mut child);").count(), 1);
        assert_eq!(ollama.matches("super::reap_child(&mut child);").count(), 5);
        for gone in [
            "\"no stdout from curl\".to_string()))?",
            "read error during chat: {e}\")))?",
            "read error during pull: {e}\")))?",
            "failed to write request body to curl: {e}\"))\n            })?",
            "failed to write body to curl stdin: {error}\"))\n        })?",
        ] {
            assert!(!ollama.contains(gone) && !helpers.contains(gone), "{gone}");
        }
    }
}
