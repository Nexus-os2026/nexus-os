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

use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::emergency_stop::StopWatch;
use crate::inference::{
    CallCancel, CallDeadline, CallMeter, LoopbackEndpoint, ModelTransport, RuntimeConfig,
    VerifiedOllama, CONVERSATION_CONFIG,
};

/// The local model errors: the kernel transport's codes.
pub use crate::inference::ModelError;
/// The only provider a coding run can use.
pub use crate::inference::LOCAL_PROVIDER;

/// A checked loopback Ollama address (see [`loopback_endpoint`]).
pub type LocalEndpoint = LoopbackEndpoint;

const MAX_MODEL_NAME_BYTES: usize = 128;
const MAX_LIST_RESPONSE_BYTES: u64 = 1024 * 1024;
const LIST_TIMEOUT: Duration = Duration::from_secs(5);

/// The provider, loopback endpoint and full identity of the model a run is
/// bound to: the model's name and manifest digest, the runtime's version
/// and the digest of the qualified request configuration.
#[derive(Debug, Clone)]
pub struct ModelPin {
    provider: &'static str,
    endpoint: String,
    model: String,
    digest: String,
    runtime: String,
    config_digest: [u8; 32],
}

impl PartialEq for ModelPin {
    fn eq(&self, other: &Self) -> bool {
        self.provider == other.provider
            && self.endpoint == other.endpoint
            && self.model == other.model
    }
}

impl Eq for ModelPin {}

impl ModelPin {
    /// The pin of a verified local model.
    pub fn of(transport: &VerifiedOllama) -> Self {
        let identity = transport.identity();
        Self {
            provider: LOCAL_PROVIDER,
            endpoint: format!("http://{}", transport.endpoint().addr()),
            model: identity.name().to_string(),
            digest: identity.model_digest().to_string(),
            runtime: identity.runtime_version().to_string(),
            config_digest: model_config_digest(identity.config()),
        }
    }

    pub fn provider(&self) -> &str {
        self.provider
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    /// The model's manifest digest (hex, as the runtime lists it).
    pub fn digest(&self) -> &str {
        &self.digest
    }

    /// The runtime's version.
    pub fn runtime(&self) -> &str {
        &self.runtime
    }

    /// SHA-256 of the qualified request configuration: the runtime
    /// configuration and the conversation configuration.
    pub fn config_digest(&self) -> [u8; 32] {
        self.config_digest
    }

    /// A pin to the local provider at a checked loopback endpoint.
    fn local(endpoint: &LocalEndpoint, model: &str) -> Result<Self, ModelError> {
        Ok(Self {
            provider: LOCAL_PROVIDER,
            endpoint: format!("http://{}", endpoint.addr()),
            model: checked_model_name(model)?.to_string(),
            digest: String::new(),
            runtime: String::new(),
            config_digest: [0; 32],
        })
    }
}

/// SHA-256 over `nexus.coding_run.model_config.v1` and the length-prefixed
/// digests of the runtime configuration and the conversation configuration.
pub(crate) fn model_config_digest(config: &RuntimeConfig) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"nexus.coding_run.model_config.v1");
    for part in [config.digest(), CONVERSATION_CONFIG.digest()] {
        hash.update(32u32.to_be_bytes());
        hash.update(part);
    }
    hash.finalize().into()
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

/// Check the operator-authorized Ollama address and require loopback: the
/// kernel transport's [`LoopbackEndpoint::parse`] (`127.0.0.1` or `::1`
/// with a written port; `localhost` becomes `127.0.0.1`). Any other host —
/// a LAN or remote Ollama, a cloud endpoint — is refused: coding runs use
/// local models only.
pub fn loopback_endpoint(raw: &str) -> Result<LocalEndpoint, ModelError> {
    LoopbackEndpoint::parse(raw)
}

/// The operator's local Ollama.
#[derive(Clone)]
pub struct LocalOllama {
    pin: ModelPin,
    base: url::Url,
    #[allow(dead_code)]
    bound: Option<Bound>,
}

/// What a bound model call is charged to and ended by.
#[derive(Clone)]
#[allow(dead_code)]
struct Bound {
    transport: Arc<VerifiedOllama>,
    meter: Arc<dyn CallMeter>,
    cancel: Arc<dyn CallCancel>,
    deadline: Arc<dyn CallDeadline>,
    stop: StopWatch,
}

impl std::fmt::Debug for LocalOllama {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalOllama")
            .field("pin", &self.pin)
            .finish_non_exhaustive()
    }
}

fn legacy_base(endpoint: &LocalEndpoint) -> Result<url::Url, ModelError> {
    url::Url::parse(&format!("http://{}/", endpoint.addr())).map_err(|_| ModelError::InvalidAddress)
}

impl LocalOllama {
    /// Bind the verified local model for one run's worker: every call is
    /// charged to `meter` and ended by `cancel`, `deadline` and `stop`, the
    /// watch the run was admitted under.
    pub fn bind(
        transport: Arc<VerifiedOllama>,
        meter: Arc<dyn CallMeter>,
        cancel: Arc<dyn CallCancel>,
        deadline: Arc<dyn CallDeadline>,
        stop: StopWatch,
    ) -> Result<Self, ModelError> {
        Ok(Self {
            pin: ModelPin::of(&transport),
            base: legacy_base(transport.endpoint())?,
            bound: Some(Bound {
                transport,
                meter,
                cancel,
                deadline,
                stop,
            }),
        })
    }

    /// Models installed in the local Ollama.
    pub fn installed_models(endpoint: &LocalEndpoint) -> Result<Vec<String>, ModelError> {
        let url = legacy_base(endpoint)?
            .join("api/tags")
            .map_err(|_| ModelError::InvalidAddress)?;
        let raw = on_request_thread(move || async move {
            let response = client(LIST_TIMEOUT)?
                .get(url)
                .send()
                .await
                .map_err(request_error)?;
            read_body(response, MAX_LIST_RESPONSE_BYTES).await
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
    pub fn select(endpoint: &LocalEndpoint, model: &str) -> Result<Self, ModelError> {
        let model = checked_model_name(model)?;
        if !Self::installed_models(endpoint)?
            .iter()
            .any(|name| name == model)
        {
            return Err(ModelError::NotInstalled);
        }
        Ok(Self {
            pin: ModelPin::local(endpoint, model)?,
            base: legacy_base(endpoint)?,
            bound: None,
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
        let raw = on_request_thread(move || async move {
            let response = client(timeout)?
                .post(url)
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(body)
                .send()
                .await
                .map_err(request_error)?;
            if !response.status().is_success() {
                return Err(ModelError::Unavailable);
            }
            read_body(response, envelope_cap).await
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
fn client(timeout: Duration) -> Result<reqwest::Client, ModelError> {
    reqwest::Client::builder()
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

async fn read_body(mut response: reqwest::Response, cap: u64) -> Result<Vec<u8>, ModelError> {
    let mut raw = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(request_error)? {
        raw.extend_from_slice(&chunk);
        if raw.len() as u64 > cap {
            return Err(ModelError::ResponseTooLarge);
        }
    }
    Ok(raw)
}

/// Each request runs on its own thread, on its own single-threaded
/// runtime, as work admitted under the process's emergency-stop authority
/// (M0-FIX F5): refused while a stop is in force, and ended by a stop while
/// in flight. Ending it drops the request and then the runtime with its
/// connection, so the model server sees the connection close; the call
/// returns [`ModelError::Stopped`] and the halt is logged.
fn on_request_thread<T, F, Fut>(request: F) -> Result<T, ModelError>
where
    T: Send + 'static,
    F: FnOnce() -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Result<T, ModelError>>,
{
    let watch = crate::emergency_stop::StopAuthority::global()
        .admit()
        .map_err(|_| ModelError::Stopped)?;
    std::thread::Builder::new()
        .name("nexus-coding-model".to_string())
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|_| ModelError::Unavailable)?;
            let stopped = watch.clone();
            let result = runtime.block_on(async move {
                tokio::select! {
                    result = request() => result,
                    () = stopped.halted_async() => Err(ModelError::Stopped),
                }
            });
            // The connection ends with the runtime.
            drop(runtime);
            if matches!(result, Err(ModelError::Stopped)) {
                watch.record_halt(
                    crate::emergency_stop::HaltArea::ModelCall,
                    crate::emergency_stop::HaltOutcome::Ended,
                );
            }
            result
        })
        .map_err(|_| ModelError::Unavailable)?
        .join()
        .map_err(|_| ModelError::Unavailable)?
}

/// The qualified test pin's model manifest digest and runtime version.
#[cfg(test)]
pub(crate) const TEST_PIN_DIGEST: &str =
    "a7eb95c53bcf96b4bdd008d0fab4a5dac88047d9c1a7a9ab88ed453423fbd87c";
#[cfg(test)]
pub(crate) const TEST_PIN_RUNTIME: &str = "0.40.13";

/// The qualified runtime configuration, for test pins.
#[cfg(test)]
pub(crate) fn test_pin_config() -> RuntimeConfig {
    RuntimeConfig {
        num_ctx: 8192,
        temperature_milli: 0,
        seed: 7,
        think: crate::inference::Think::Off,
        truncate: false,
    }
}

#[cfg(test)]
pub(crate) fn pin_for_test(model: &str) -> ModelPin {
    let mut pin =
        ModelPin::local(&loopback_endpoint("http://127.0.0.1:11434").unwrap(), model).unwrap();
    pin.digest = TEST_PIN_DIGEST.to_string();
    pin.runtime = TEST_PIN_RUNTIME.to_string();
    pin.config_digest = model_config_digest(&test_pin_config());
    pin
}

/// A pin with any endpoint and model spelling, unchecked: the P0-B ledger
/// oracles plant canaries in both, and the record must not depend on the
/// spelling being one the constructor accepts.
#[cfg(test)]
pub(crate) fn pin_for_test_at(endpoint: &str, model: &str) -> ModelPin {
    ModelPin {
        provider: LOCAL_PROVIDER,
        endpoint: endpoint.to_string(),
        model: model.to_string(),
        digest: TEST_PIN_DIGEST.to_string(),
        runtime: TEST_PIN_RUNTIME.to_string(),
        config_digest: model_config_digest(&test_pin_config()),
    }
}
