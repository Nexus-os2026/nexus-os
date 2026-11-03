//! The local Ollama runtime: the only paths requested, registration, the
//! per-call identity checks, the frozen chat body and the reply's
//! refusals.

use std::time::Instant;

use serde_json::Value;

use super::endpoint::LoopbackEndpoint;
use super::exchange::{exchange, Guard, Reply};
use super::identity::ModelIdentity;
use super::request::{json_string, ModelReply, ModelRequest};
use super::{
    CallCancel, CallContext, CallDeadline, CallMeter, ModelError, ModelPinSpec, ModelTransport,
    Think,
};
use crate::emergency_stop::StopWatch;

/// The bound on a metadata reply (version, status, tags, show).
const MAX_METADATA_BYTES: usize = 1024 * 1024;
/// The bound on a chat reply's envelope.
const MAX_CHAT_ENVELOPE_BYTES: usize = 256 * 1024;
/// The bound on a chat reply's content.
const MAX_CONTENT_BYTES: usize = 32 * 1024;

/// The only requests ever made to the runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ApiPath {
    Version,
    Status,
    Tags,
    Show,
    Chat,
}

impl ApiPath {
    /// The path, relative to the runtime's root.
    pub(super) fn as_str(self) -> &'static str {
        match self {
            ApiPath::Version => "api/version",
            ApiPath::Status => "api/status",
            ApiPath::Tags => "api/tags",
            ApiPath::Show => "api/show",
            ApiPath::Chat => "api/chat",
        }
    }
}

/// Whether an Ollama model name is one of the runtime's cloud models (for
/// example `gpt-oss:120b-cloud`), which the runtime relays to a remote
/// service. Any name with a `cloud` component counts.
pub fn name_is_cloud(model: &str) -> bool {
    model
        .to_ascii_lowercase()
        .split([':', '-', '/', '.', '_'])
        .any(|part| part == "cloud")
}

/// Whether an Ollama model description (a `/api/show` reply or a
/// `/api/tags` entry) says the model runs elsewhere: it names a remote host
/// or a remote model.
pub fn show_names_remote(show: &Value) -> bool {
    ["remote_host", "remote_model"].iter().any(|field| {
        show.get(field).is_some_and(|value| {
            !value.is_null() && value.as_str().is_none_or(|text| !text.trim().is_empty())
        })
    })
}

/// The local Ollama, verified to serve one pinned model identity. The only
/// [`ModelTransport`] in a default build.
#[derive(Debug)]
pub struct VerifiedOllama {
    endpoint: LoopbackEndpoint,
    identity: ModelIdentity,
}

impl VerifiedOllama {
    /// Register the pinned model: the runtime's version equals the pin, the
    /// runtime reports its cloud features disabled, the name has no cloud
    /// component, `/api/tags` lists exactly the name with the pinned digest
    /// and no remote, and `/api/show` names no remote and lists the
    /// `thinking` capability. Every request goes through the cancellable
    /// wrapper, under the caller's cancellation, deadline and stop watch.
    pub fn verify(
        endpoint: LoopbackEndpoint,
        spec: &ModelPinSpec,
        cancel: &dyn CallCancel,
        deadline: &dyn CallDeadline,
        stop: &StopWatch,
    ) -> Result<Self, ModelError> {
        let identity = ModelIdentity::from_spec(spec)?;
        let guard = Guard {
            cancel,
            deadline,
            stop,
        };
        check_runtime(&endpoint, &identity, &guard)?;
        check_show(&endpoint, &identity, &guard)?;
        Ok(Self { endpoint, identity })
    }

    pub fn endpoint(&self) -> &LoopbackEndpoint {
        &self.endpoint
    }
}

impl ModelTransport for VerifiedOllama {
    fn identity(&self) -> &ModelIdentity {
        &self.identity
    }

    /// One call: the request's bound; the runtime's version, cloud status
    /// and the model's digest again; the charge; the chat; the late check;
    /// the reply's refusals. Settled on every path after the charge.
    fn call(
        &self,
        request: &ModelRequest,
        ctx: &CallContext<'_>,
    ) -> Result<ModelReply, ModelError> {
        request.check()?;
        let guard = Guard::of(ctx);
        check_runtime(&self.endpoint, &self.identity, &guard)?;
        let charged = ctx.meter.charge().map_err(|_| ModelError::OverBudget)?;
        let _settle = Settle {
            meter: ctx.meter,
            call: charged.call,
        };
        guard.check()?;
        let body = chat_body(&self.identity, request);
        let started = Instant::now();
        let reply = exchange(
            &self.endpoint,
            ApiPath::Chat,
            Some(body),
            MAX_CHAT_ENVELOPE_BYTES,
            &guard,
        )?;
        let latency_ms = u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX);
        #[cfg(test)]
        super::exchange::seams::after_reply();
        // A reply that arrives after the call was ended is never used.
        if let Err(ended) = guard.check() {
            if ended == ModelError::Stopped {
                guard.record_stop();
            }
            return Err(ModelError::Late);
        }
        read_chat(&self.identity, reply, latency_ms)
    }
}

/// Settles a charged call on every path, unwinding included.
struct Settle<'a> {
    meter: &'a dyn CallMeter,
    call: u32,
}

impl Drop for Settle<'_> {
    fn drop(&mut self) {
        self.meter.settle(self.call);
    }
}

/// The checks made at registration and before every charge: version, cloud
/// status, then the model's tags entry.
fn check_runtime(
    endpoint: &LoopbackEndpoint,
    identity: &ModelIdentity,
    guard: &Guard<'_>,
) -> Result<(), ModelError> {
    let version = metadata(endpoint, ApiPath::Version, None, guard)?;
    if version.get("version").and_then(Value::as_str) != Some(identity.runtime_version()) {
        return Err(if version.get("version").is_some_and(Value::is_string) {
            ModelError::RuntimeMismatch
        } else {
            ModelError::Malformed
        });
    }
    check_status(endpoint, guard)?;
    check_tags(endpoint, identity, guard)
}

/// The runtime reports its cloud features disabled: `cloud.disabled` is
/// `true`. Anything else (enabled, missing, malformed, an error status) is
/// [`ModelError::CloudEnabled`].
fn check_status(endpoint: &LoopbackEndpoint, guard: &Guard<'_>) -> Result<(), ModelError> {
    let reply = exchange(endpoint, ApiPath::Status, None, MAX_METADATA_BYTES, guard)?;
    let disabled = reply.status == 200
        && serde_json::from_slice::<Value>(&reply.body)
            .ok()
            .and_then(|status| status.pointer("/cloud/disabled").and_then(Value::as_bool))
            == Some(true);
    if disabled {
        Ok(())
    } else {
        Err(ModelError::CloudEnabled)
    }
}

/// `/api/tags` lists the name exactly once, with the pinned digest and no
/// remote.
fn check_tags(
    endpoint: &LoopbackEndpoint,
    identity: &ModelIdentity,
    guard: &Guard<'_>,
) -> Result<(), ModelError> {
    let tags = metadata(endpoint, ApiPath::Tags, None, guard)?;
    let models = tags
        .get("models")
        .and_then(Value::as_array)
        .ok_or(ModelError::Malformed)?;
    let mut entries = models
        .iter()
        .filter(|model| model.get("name").and_then(Value::as_str) == Some(identity.name()));
    let entry = entries.next().ok_or(ModelError::NotInstalled)?;
    if entries.next().is_some() {
        return Err(ModelError::Malformed);
    }
    if show_names_remote(entry) {
        return Err(ModelError::Remote);
    }
    if entry.get("digest").and_then(Value::as_str) != Some(identity.model_digest()) {
        return Err(ModelError::DigestMismatch);
    }
    Ok(())
}

/// `/api/show` names no remote, and the capabilities agree with the
/// thinking setting.
fn check_show(
    endpoint: &LoopbackEndpoint,
    identity: &ModelIdentity,
    guard: &Guard<'_>,
) -> Result<(), ModelError> {
    let body = format!("{{\"model\":{}}}", json_string(identity.name())).into_bytes();
    let show = metadata(endpoint, ApiPath::Show, Some(body), guard)?;
    if !show.is_object() {
        return Err(ModelError::Malformed);
    }
    if show_names_remote(&show) {
        return Err(ModelError::Remote);
    }
    let thinking = show
        .get("capabilities")
        .and_then(Value::as_array)
        .is_some_and(|capabilities| {
            capabilities
                .iter()
                .any(|capability| capability.as_str() == Some("thinking"))
        });
    let wanted = match identity.config().think {
        Think::Off => true,
    };
    if thinking != wanted {
        return Err(ModelError::ConfigMismatch);
    }
    Ok(())
}

/// A metadata request: status 200 and a JSON body.
fn metadata(
    endpoint: &LoopbackEndpoint,
    path: ApiPath,
    body: Option<Vec<u8>>,
    guard: &Guard<'_>,
) -> Result<Value, ModelError> {
    let reply = exchange(endpoint, path, body, MAX_METADATA_BYTES, guard)?;
    if reply.status != 200 {
        return Err(ModelError::Unavailable);
    }
    serde_json::from_slice(&reply.body).map_err(|_| ModelError::Malformed)
}

/// The frozen chat body, built from the identity; the caller supplies only
/// the two messages and the schema.
fn chat_body(identity: &ModelIdentity, request: &ModelRequest) -> Vec<u8> {
    let config = identity.config();
    let think = match config.think {
        Think::Off => "false",
    };
    format!(
        "{{\"model\":{},\"stream\":false,\"think\":{think},\"truncate\":{},\"format\":{},\
         \"options\":{{\"temperature\":{},\"num_ctx\":{},\"seed\":{}}},\
         \"messages\":[{{\"role\":\"system\",\"content\":{}}},{{\"role\":\"user\",\"content\":{}}}]}}",
        json_string(identity.name()),
        config.truncate,
        request.format().as_json(),
        temperature(config.temperature_milli),
        config.num_ctx,
        config.seed,
        json_string(request.system()),
        json_string(request.user()),
    )
    .into_bytes()
}

/// A temperature in thousandths, as a JSON number (`0`, `0.7`, `1.25`).
fn temperature(milli: u16) -> String {
    let (whole, fraction) = (milli / 1000, milli % 1000);
    if fraction == 0 {
        whole.to_string()
    } else {
        format!("{whole}.{fraction:03}")
            .trim_end_matches('0')
            .to_string()
    }
}

/// The reply's refusals. Never retried.
fn read_chat(
    identity: &ModelIdentity,
    reply: Reply,
    latency_ms: u32,
) -> Result<ModelReply, ModelError> {
    if reply.status != 200 {
        return Err(ModelError::Unavailable);
    }
    let value: Value = serde_json::from_slice(&reply.body).map_err(|_| ModelError::Malformed)?;
    if !value.is_object()
        || value.get("error").is_some()
        || value.get("model").and_then(Value::as_str) != Some(identity.name())
    {
        return Err(ModelError::Malformed);
    }
    let message = value.get("message").ok_or(ModelError::Malformed)?;
    if message.get("role").and_then(Value::as_str) != Some("assistant") {
        return Err(ModelError::Malformed);
    }
    if message.get("tool_calls").is_some() {
        return Err(ModelError::ToolCalls);
    }
    match message.get("thinking") {
        None | Some(Value::Null) => {}
        Some(Value::String(thinking)) if thinking.is_empty() => {}
        Some(Value::String(_)) => return Err(ModelError::ThinkingLeaked),
        Some(_) => return Err(ModelError::Malformed),
    }
    if value.get("done") != Some(&Value::Bool(true))
        || value.get("done_reason").and_then(Value::as_str) != Some("stop")
    {
        return Err(ModelError::Truncated);
    }
    let count = |key: &str| {
        value
            .get(key)
            .and_then(Value::as_u64)
            .ok_or(ModelError::Malformed)
    };
    let (prompt, output) = (count("prompt_eval_count")?, count("eval_count")?);
    let num_ctx = u64::from(identity.config().num_ctx);
    if prompt >= num_ctx / 2 || prompt.saturating_add(output) >= num_ctx {
        return Err(ModelError::Truncated);
    }
    let content = message
        .get("content")
        .and_then(Value::as_str)
        .ok_or(ModelError::Malformed)?;
    if content.is_empty() {
        return Err(ModelError::EmptyContent);
    }
    if content.len() > MAX_CONTENT_BYTES {
        return Err(ModelError::ResponseTooLarge);
    }
    Ok(ModelReply::new(
        content.to_string(),
        u32::try_from(prompt).unwrap_or(u32::MAX),
        u32::try_from(output).unwrap_or(u32::MAX),
        latency_ms,
    ))
}
