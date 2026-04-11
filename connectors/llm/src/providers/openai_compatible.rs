use super::{LlmResponse, ProviderRequest};
use nexus_kernel::errors::AgentError;
use reqwest::blocking::Client;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::time::Duration;

const REQUEST_TIMEOUT_SECS: u64 = 120;

pub(crate) struct OpenAiCompatibleQuery<'a> {
    pub provider_name: &'a str,
    pub missing_key_error: &'a str,
    pub api_key: Option<String>,
    pub endpoint: &'a str,
    pub prompt: &'a str,
    pub max_tokens: u32,
    pub model: &'a str,
    pub extra_headers: &'a [(&'a str, String)],
}

pub(crate) fn bearer_headers(api_key: &str) -> BTreeMap<String, String> {
    let mut headers = BTreeMap::new();
    headers.insert(
        "authorization".to_string(),
        format!("Bearer {}", api_key.trim()),
    );
    headers.insert("content-type".to_string(), "application/json".to_string());
    headers
}

pub(crate) fn build_openai_chat_request(
    endpoint: &str,
    api_key: &str,
    prompt: &str,
    max_tokens: u32,
    model: &str,
) -> ProviderRequest {
    ProviderRequest {
        endpoint: endpoint.to_string(),
        headers: bearer_headers(api_key),
        body: json!({
            "model": model,
            "messages": [
                {
                    "role": "user",
                    "content": prompt
                }
            ],
            "max_tokens": max_tokens
        }),
    }
}

pub(crate) fn execute_openai_compatible_query(
    query: OpenAiCompatibleQuery<'_>,
) -> Result<LlmResponse, AgentError> {
    let Some(api_key) = query.api_key.filter(|value| !value.trim().is_empty()) else {
        return Err(AgentError::SupervisorError(
            query.missing_key_error.to_string(),
        ));
    };

    let mut request = build_openai_chat_request(
        query.endpoint,
        &api_key,
        query.prompt,
        query.max_tokens,
        query.model,
    );
    for (header_name, header_value) in query.extra_headers {
        request
            .headers
            .insert((*header_name).to_string(), header_value.clone());
    }

    let client = Client::builder()
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .build()
        .map_err(|error| {
            AgentError::SupervisorError(format!(
                "failed to build HTTP client for {}: {error}",
                query.provider_name
            ))
        })?;

    eprintln!(
        "[nexus-llm][governance] {}::complete endpoint={}",
        query.provider_name, request.endpoint
    );
    let mut call = client.post(request.endpoint.clone());
    for (header_name, header_value) in &request.headers {
        call = call.header(header_name, header_value);
    }

    let response = call.json(&request.body).send().map_err(|error| {
        AgentError::SupervisorError(format!("{} request failed: {error}", query.provider_name))
    })?;
    let status = response.status();
    let raw_text = response.text().map_err(|error| {
        AgentError::SupervisorError(format!(
            "{} response read failed: {error}",
            query.provider_name
        ))
    })?;
    let payload: Value = serde_json::from_str(&raw_text).map_err(|error| {
        let preview = if raw_text.len() > 200 {
            &raw_text[..200]
        } else {
            &raw_text
        };
        AgentError::SupervisorError(format!(
            "{} response parse failed: {error}. Raw (200 chars): {preview}",
            query.provider_name
        ))
    })?;
    if !status.is_success() {
        return Err(AgentError::SupervisorError(format!(
            "{} request failed with status {status}: {}",
            query.provider_name,
            compact_error(&payload)
        )));
    }

    Ok(LlmResponse {
        output_text: extract_openai_text(&payload),
        token_count: extract_total_tokens(&payload).unwrap_or(query.max_tokens.min(256)),
        model_name: query.model.to_string(),
        tool_calls: extract_tool_calls(&payload),
        input_tokens: None,
    })
}

/// Extract tool_calls from an OpenAI-compatible API response.
pub(crate) fn extract_tool_calls(payload: &serde_json::Value) -> Vec<String> {
    payload
        .get("choices")
        .and_then(serde_json::Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("tool_calls"))
        .and_then(serde_json::Value::as_array)
        .map(|calls| {
            calls
                .iter()
                .filter_map(|c| serde_json::to_string(c).ok())
                .collect()
        })
        .unwrap_or_default()
}

pub(crate) fn extract_openai_text(payload: &Value) -> String {
    payload
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .map(extract_content_text)
        .unwrap_or_default()
}

pub(crate) fn extract_total_tokens(payload: &Value) -> Option<u32> {
    payload
        .get("usage")
        .and_then(|usage| usage.get("total_tokens"))
        .and_then(Value::as_u64)
        // Optional: token count may exceed u32 range for very large responses
        .and_then(|value| u32::try_from(value).ok())
}

pub(crate) fn extract_content_text(value: &Value) -> String {
    if let Some(text) = value.as_str() {
        return text.to_string();
    }

    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}

fn compact_error(payload: &Value) -> String {
    payload
        .get("error")
        .map(|error| {
            if let Some(message) = error.get("message").and_then(Value::as_str) {
                message.to_string()
            } else if let Some(message) = error.as_str() {
                message.to_string()
            } else {
                error.to_string()
            }
        })
        .unwrap_or_else(|| payload.to_string())
}
