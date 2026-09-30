//! Local-only model access for coding-run workers (Phase One, decision D5).
//!
//! A coding run talks to exactly one model, pinned into the run before the
//! worker starts: a model served by the operator's local Ollama. The address
//! is the operator-authorized Ollama address (the backend passes it in) and
//! must be a loopback address; any other host, including a remote Ollama,
//! makes local models unavailable. There is no provider choice, no fallback
//! and no cloud route: if the local model cannot answer, the run fails.
//!
//! Requests are made in-process (no command-line HTTP tool, no child
//! process), without a proxy (a proxy variable could otherwise carry project
//! source off the machine), without following redirects, with a bounded wait
//! and a bounded response.

use std::io::Read;
use std::net::{IpAddr, Ipv4Addr};
use std::time::Duration;

use serde_json::{json, Value};
use thiserror::Error;

/// The only provider a Phase One coding run can use.
pub const LOCAL_PROVIDER: &str = "ollama";

/// A checked loopback Ollama address (see [`loopback_endpoint`]).
pub type LocalEndpoint = url::Url;

const MAX_MODEL_NAME_BYTES: usize = 128;
const MAX_LIST_RESPONSE_BYTES: u64 = 1024 * 1024;
const LIST_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ModelError {
    #[error("local model address is not a loopback address")]
    NotLocal,
    #[error("local model address is invalid")]
    InvalidAddress,
    #[error("model name is invalid")]
    InvalidModelName,
    #[error("the local model is not installed")]
    NotInstalled,
    #[error("local model unavailable (is Ollama running?)")]
    Unavailable,
    #[error("local model timed out")]
    Timeout,
    #[error("local model response exceeds the size cap")]
    ResponseTooLarge,
    #[error("local model response is malformed")]
    Malformed,
}

/// The provider, loopback endpoint and model a run is bound to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelPin {
    provider: &'static str,
    endpoint: String,
    model: String,
}

impl ModelPin {
    pub fn provider(&self) -> &str {
        self.provider
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    /// A pin to the local provider at a checked loopback endpoint.
    fn local(endpoint: &url::Url, model: &str) -> Result<Self, ModelError> {
        Ok(Self {
            provider: LOCAL_PROVIDER,
            endpoint: endpoint.as_str().trim_end_matches('/').to_string(),
            model: checked_model_name(model)?.to_string(),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelRole {
    System,
    User,
    Assistant,
}

impl ModelRole {
    fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::User => "user",
            Self::Assistant => "assistant",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelMessage {
    pub role: ModelRole,
    pub content: String,
}

/// A model a coding-run worker may consult. Its answer is untrusted data.
pub trait LocalModel: Send + Sync {
    /// The pin this model answers under.
    fn pin(&self) -> &ModelPin;
    /// One chat completion, bounded in time and size.
    fn complete(
        &self,
        messages: &[ModelMessage],
        timeout: Duration,
        max_response_bytes: u64,
    ) -> Result<String, ModelError>;
}

/// A model name: 1–128 bytes of ASCII letters, digits and `._:-/`, not
/// starting with a punctuation character.
fn checked_model_name(name: &str) -> Result<&str, ModelError> {
    let valid = !name.is_empty()
        && name.len() <= MAX_MODEL_NAME_BYTES
        && name.starts_with(|c: char| c.is_ascii_alphanumeric())
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | ':' | '-' | '/'));
    if valid {
        Ok(name)
    } else {
        Err(ModelError::InvalidModelName)
    }
}

/// Check the operator-authorized Ollama address and require loopback. The
/// name `localhost` is replaced by `127.0.0.1`, so name resolution cannot
/// redirect the connection. Any other host — a LAN or remote Ollama, a cloud
/// endpoint — is refused: coding runs use local models only.
pub fn loopback_endpoint(raw: &str) -> Result<url::Url, ModelError> {
    let mut url = crate::governed_http::http_url(raw).map_err(|_| ModelError::InvalidAddress)?;
    let loopback = match url.host() {
        Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
        Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
        Some(url::Host::Domain(name)) if name.eq_ignore_ascii_case("localhost") => {
            url.set_ip_host(IpAddr::V4(Ipv4Addr::LOCALHOST))
                .map_err(|()| ModelError::InvalidAddress)?;
            true
        }
        _ => false,
    };
    if !loopback {
        return Err(ModelError::NotLocal);
    }
    if url.scheme() != "http" || url.query().is_some() || url.fragment().is_some() {
        return Err(ModelError::InvalidAddress);
    }
    if url.path() != "/" && !url.path().is_empty() {
        return Err(ModelError::InvalidAddress);
    }
    Ok(url)
}

/// The operator's local Ollama.
#[derive(Debug, Clone)]
pub struct LocalOllama {
    pin: ModelPin,
    base: url::Url,
}

impl LocalOllama {
    /// Models installed in the local Ollama.
    pub fn installed_models(endpoint: &url::Url) -> Result<Vec<String>, ModelError> {
        let url = endpoint
            .join("api/tags")
            .map_err(|_| ModelError::InvalidAddress)?;
        let raw = on_request_thread(move || {
            let response = client(LIST_TIMEOUT)?
                .get(url)
                .send()
                .map_err(request_error)?;
            read_body(response, MAX_LIST_RESPONSE_BYTES)
        })?;
        let value: Value = serde_json::from_slice(&raw).map_err(|_| ModelError::Malformed)?;
        let models = value
            .get("models")
            .and_then(Value::as_array)
            .ok_or(ModelError::Malformed)?;
        let mut names: Vec<String> = models
            .iter()
            .filter_map(|model| model.get("name").and_then(Value::as_str))
            .filter(|name| checked_model_name(name).is_ok())
            .map(str::to_string)
            .collect();
        names.sort();
        names.dedup();
        Ok(names)
    }

    /// Bind to one installed local model. The name must be installed in the
    /// local Ollama; a caller-supplied name is only a choice among those.
    pub fn select(endpoint: &url::Url, model: &str) -> Result<Self, ModelError> {
        let model = checked_model_name(model)?;
        if !Self::installed_models(endpoint)?
            .iter()
            .any(|name| name == model)
        {
            return Err(ModelError::NotInstalled);
        }
        Ok(Self {
            pin: ModelPin::local(endpoint, model)?,
            base: endpoint.clone(),
        })
    }
}

impl LocalModel for LocalOllama {
    fn pin(&self) -> &ModelPin {
        &self.pin
    }

    fn complete(
        &self,
        messages: &[ModelMessage],
        timeout: Duration,
        max_response_bytes: u64,
    ) -> Result<String, ModelError> {
        let url = self
            .base
            .join("api/chat")
            .map_err(|_| ModelError::InvalidAddress)?;
        let body = json!({
            "model": self.pin.model,
            "stream": false,
            "format": "json",
            "options": { "temperature": 0 },
            "messages": messages
                .iter()
                .map(|m| json!({ "role": m.role.as_str(), "content": m.content }))
                .collect::<Vec<_>>(),
        });
        let body = serde_json::to_vec(&body).map_err(|_| ModelError::Malformed)?;
        // Room for the JSON envelope around the answer.
        let envelope_cap = max_response_bytes
            .saturating_mul(2)
            .saturating_add(64 * 1024);
        let raw = on_request_thread(move || {
            let response = client(timeout)?
                .post(url)
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(body)
                .send()
                .map_err(request_error)?;
            if !response.status().is_success() {
                return Err(ModelError::Unavailable);
            }
            read_body(response, envelope_cap)
        })?;
        let value: Value = serde_json::from_slice(&raw).map_err(|_| ModelError::Malformed)?;
        let content = value
            .pointer("/message/content")
            .and_then(Value::as_str)
            .ok_or(ModelError::Malformed)?;
        if content.len() as u64 > max_response_bytes {
            return Err(ModelError::ResponseTooLarge);
        }
        Ok(content.to_string())
    }
}

/// A client that uses no proxy and follows no redirect.
fn client(timeout: Duration) -> Result<reqwest::blocking::Client, ModelError> {
    reqwest::blocking::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(timeout)
        .build()
        .map_err(|_| ModelError::Unavailable)
}

fn request_error(error: reqwest::Error) -> ModelError {
    if error.is_timeout() {
        ModelError::Timeout
    } else {
        ModelError::Unavailable
    }
}

fn read_body(response: reqwest::blocking::Response, cap: u64) -> Result<Vec<u8>, ModelError> {
    let mut raw = Vec::new();
    response
        .take(cap + 1)
        .read_to_end(&mut raw)
        .map_err(|_| ModelError::Unavailable)?;
    if raw.len() as u64 > cap {
        return Err(ModelError::ResponseTooLarge);
    }
    Ok(raw)
}

/// The blocking client must not run on an async runtime's thread; each
/// request runs on its own thread.
fn on_request_thread<T: Send + 'static>(
    request: impl FnOnce() -> Result<T, ModelError> + Send + 'static,
) -> Result<T, ModelError> {
    std::thread::Builder::new()
        .name("nexus-coding-model".to_string())
        .spawn(request)
        .map_err(|_| ModelError::Unavailable)?
        .join()
        .map_err(|_| ModelError::Unavailable)?
}

#[cfg(test)]
pub(crate) fn pin_for_test(model: &str) -> ModelPin {
    ModelPin::local(&loopback_endpoint("http://127.0.0.1:11434").unwrap(), model).unwrap()
}
