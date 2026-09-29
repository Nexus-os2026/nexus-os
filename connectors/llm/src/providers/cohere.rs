use super::openai_compatible::{bearer_headers, extract_content_text, extract_tool_calls};
use super::{LlmProvider, LlmResponse, ProviderRequest};
use nexus_kernel::errors::AgentError;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::env;
use std::time::Duration;

const REQUEST_TIMEOUT_SECS: u64 = 20;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CohereProvider {
    api_key: Option<String>,
    endpoint: String,
}

impl CohereProvider {
    pub fn new(api_key: Option<String>) -> Self {
        Self {
            api_key,
            endpoint: "https://api.cohere.ai/v2/chat".to_string(),
        }
    }

    pub fn from_env() -> Self {
        let endpoint =
            env::var("COHERE_URL").unwrap_or_else(|_| "https://api.cohere.ai/v2/chat".to_string());
        // Optional: API key may not be configured in environment
        let mut provider = Self::new(env::var("COHERE_API_KEY").ok());
        provider.endpoint = endpoint;
        provider
    }

    pub fn build_request(&self, prompt: &str, max_tokens: u32, model: &str) -> ProviderRequest {
        let api_key = self.api_key.clone().unwrap_or_default();
        let mut headers: BTreeMap<String, String> = bearer_headers(&api_key);
        headers.insert("accept".to_string(), "application/json".to_string());

        ProviderRequest {
            endpoint: self.endpoint.clone(),
            headers,
            body: json!({
                "model": model,
                "message": prompt,
                "max_tokens": max_tokens,
            }),
        }
    }

    fn api_key(&self) -> Option<String> {
        self.api_key
            .clone()
            .or_else(|| env::var("COHERE_API_KEY").ok())
            .map(|key| key.trim().to_string())
            .filter(|key| !key.is_empty())
    }
}

impl LlmProvider for CohereProvider {
    fn query(&self, prompt: &str, max_tokens: u32, model: &str) -> Result<LlmResponse, AgentError> {
        self.query_bounded(
            None,
            prompt,
            max_tokens,
            model,
            Duration::from_secs(REQUEST_TIMEOUT_SECS),
            super::MAX_PROVIDER_RESPONSE_BYTES,
        )
    }

    fn name(&self) -> &str {
        "cohere"
    }

    fn cost_per_token(&self) -> f64 {
        0.000_002
    }

    fn endpoint_url(&self) -> String {
        self.endpoint.clone()
    }
}

impl CohereProvider {
    /// The query with its bounds (Final Gate decision E): the whole exchange
    /// ends within `timeout` and at most `max_bytes` of response is read.
    /// `endpoint` replaces the fixed endpoint only in tests.
    pub(super) fn query_bounded(
        &self,
        endpoint: Option<&str>,
        prompt: &str,
        max_tokens: u32,
        model: &str,
        timeout: Duration,
        max_bytes: u64,
    ) -> Result<LlmResponse, AgentError> {
        let Some(api_key) = self.api_key() else {
            return Err(AgentError::SupervisorError(
                "COHERE_API_KEY is not set".to_string(),
            ));
        };

        let mut request =
            CohereProvider::new(Some(api_key)).build_request(prompt, max_tokens, model);
        // The query goes to the fixed Cohere endpoint, as before; tests pass
        // a loopback stand-in.
        if let Some(endpoint) = endpoint {
            request.endpoint = endpoint.to_string();
        }
        let client = super::credential_client(timeout).map_err(|error| {
            AgentError::SupervisorError(format!("failed to build HTTP client for cohere: {error}"))
        })?;

        eprintln!(
            "[nexus-llm][governance] cohere::complete endpoint={}",
            request.endpoint
        );
        let mut call = client.post(request.endpoint.clone()).timeout(timeout);
        for (header_name, header_value) in &request.headers {
            call = call.header(header_name, header_value);
        }

        let response = call.json(&request.body).send().map_err(|error| {
            AgentError::SupervisorError(format!("cohere request failed: {error}"))
        })?;
        let status = response.status();
        let raw = super::read_bounded(response, max_bytes).map_err(|error| {
            AgentError::SupervisorError(format!("cohere response read failed: {error}"))
        })?;
        let payload = serde_json::from_slice::<Value>(&raw).map_err(|error| {
            AgentError::SupervisorError(format!("cohere response parse failed: {error}"))
        })?;
        if !status.is_success() {
            return Err(AgentError::SupervisorError(format!(
                "cohere request failed with status {status}: {payload}"
            )));
        }

        let output_text = payload
            .get("message")
            .and_then(|message| message.get("content"))
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .map(extract_content_text)
                    .filter(|chunk| !chunk.is_empty())
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default();

        let token_count = payload
            .get("usage")
            .and_then(|usage| usage.get("tokens"))
            .and_then(|tokens| tokens.get("output_tokens"))
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
            .unwrap_or(max_tokens.min(256));

        Ok(LlmResponse {
            output_text,
            token_count,
            model_name: model.to_string(),
            tool_calls: extract_tool_calls(&payload),
            input_tokens: None,
        })
    }
}
