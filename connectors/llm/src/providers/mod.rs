use nexus_kernel::errors::AgentError;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::env;
use std::process::Command;

pub mod claude;
pub mod deepseek;
#[cfg(feature = "local-slm")]
pub mod local_slm;
pub mod mock;
pub mod ollama;

pub use claude::ClaudeProvider;
pub use deepseek::DeepSeekProvider;
#[cfg(feature = "local-slm")]
pub use local_slm::LocalSlmProvider;
pub use mock::MockProvider;
pub use ollama::OllamaProvider;

const REAL_API_DISABLED_ERROR: &str =
    "Real API disabled. Set ENABLE_REAL_API=1 to allow external calls.";

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
}

pub trait LlmProvider: Send + Sync {
    fn query(&self, prompt: &str, max_tokens: u32, model: &str) -> Result<LlmResponse, AgentError>;
    fn name(&self) -> &str;
    fn cost_per_token(&self) -> f64;

    fn is_paid(&self) -> bool {
        self.cost_per_token() > 0.0
    }

    fn requires_real_api_opt_in(&self) -> bool {
        false
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

    fn requires_real_api_opt_in(&self) -> bool {
        (**self).requires_real_api_opt_in()
    }

    fn estimate_input_tokens(&self, prompt: &str) -> u32 {
        (**self).estimate_input_tokens(prompt)
    }

    fn endpoint_url(&self) -> String {
        (**self).endpoint_url()
    }
}

pub(crate) fn require_real_api(feature_enabled: bool) -> Result<(), AgentError> {
    require_real_api_with(feature_enabled, env::var("ENABLE_REAL_API").ok().as_deref())
}

pub(crate) fn require_real_api_with(
    feature_enabled: bool,
    real_api_value: Option<&str>,
) -> Result<(), AgentError> {
    if !feature_enabled || real_api_value != Some("1") {
        return Err(AgentError::SupervisorError(
            REAL_API_DISABLED_ERROR.to_string(),
        ));
    }
    Ok(())
}

pub(crate) fn curl_get_status(endpoint: &str) -> Result<u16, AgentError> {
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
        return Err(AgentError::SupervisorError(
            "curl request failed".to_string(),
        ));
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
    let marker = "__NEXUS_STATUS__:";
    let encoded_body = serde_json::to_string(body).map_err(|error| {
        AgentError::SupervisorError(format!("failed to encode request body: {error}"))
    })?;

    let mut command = Command::new("curl");
    command.args(["-sS", "-L", "-m", "20"]);
    for (header_name, header_value) in headers {
        command
            .arg("-H")
            .arg(format!("{header_name}: {header_value}"));
    }
    command
        .arg("-d")
        .arg(encoded_body)
        .arg("-w")
        .arg(format!("\n{marker}%{{http_code}}"))
        .arg(endpoint);

    let output = command
        .output()
        .map_err(|error| AgentError::SupervisorError(format!("curl execution failed: {error}")))?;
    if !output.status.success() {
        return Err(AgentError::SupervisorError(
            "curl request failed".to_string(),
        ));
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
    let response_json = if body_raw.trim().is_empty() {
        Value::Null
    } else {
        serde_json::from_str::<Value>(body_raw).map_err(|error| {
            AgentError::SupervisorError(format!("failed to parse JSON response: {error}"))
        })?
    };
    Ok((status, response_json))
}

#[cfg(test)]
mod tests {
    use super::{
        require_real_api_with, ClaudeProvider, DeepSeekProvider, LlmProvider, OllamaProvider,
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
                    "num_predict": 64
                }
            })
        );
    }

    #[test]
    fn test_real_api_guard_blocks_without_env() {
        let provider = DeepSeekProvider::new(Some("key".to_string()));
        let result = provider.query("hello", 10, "deepseek-chat");
        assert!(result.is_err());
        if let Err(error) = result {
            assert!(error
                .to_string()
                .contains("Real API disabled. Set ENABLE_REAL_API=1 to allow external calls."));
        }

        let guard = require_real_api_with(true, None);
        assert!(guard.is_err());
    }
}
