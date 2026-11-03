//! The local Ollama runtime: the only paths requested, registration and
//! the call.
//!
//! M2e-A1 interface stub: the inherited behaviour. The coding-run transport
//! binds a model whose name `/api/tags` lists, with no digest, version,
//! cloud-status or description check; its chat body is `format: "json"`
//! and `temperature: 0` only; it reads only `/message/content`, charges
//! nothing and refuses nothing (F1–F5). This stub does the same.

use std::time::Instant;

use serde_json::{json, Value};

use super::endpoint::LoopbackEndpoint;
use super::exchange::{exchange, Guard};
use super::identity::ModelIdentity;
use super::request::{ModelReply, ModelRequest};
use super::{CallCancel, CallContext, CallDeadline, ModelError, ModelPinSpec, ModelTransport};
use crate::emergency_stop::StopWatch;

const MAX_METADATA_BYTES: usize = 1024 * 1024;
const MAX_CHAT_ENVELOPE_BYTES: usize = 256 * 1024;

/// The only requests ever made to the runtime.
#[allow(dead_code)]
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

/// Whether an Ollama model name is one of the runtime's cloud models. Any
/// name with a `cloud` component counts.
pub fn name_is_cloud(model: &str) -> bool {
    model
        .to_ascii_lowercase()
        .split([':', '-', '/', '.', '_'])
        .any(|part| part == "cloud")
}

/// Whether an Ollama model description names a remote host or model.
pub fn show_names_remote(show: &Value) -> bool {
    ["remote_host", "remote_model"].iter().any(|field| {
        show.get(field).is_some_and(|value| {
            !value.is_null() && value.as_str().is_none_or(|text| !text.trim().is_empty())
        })
    })
}

/// The local Ollama (stub: bound as the inherited transport binds).
#[derive(Debug)]
pub struct VerifiedOllama {
    endpoint: LoopbackEndpoint,
    identity: ModelIdentity,
}

impl VerifiedOllama {
    /// Stub: the model's name must be listed by `/api/tags`.
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
        let reply = exchange(&endpoint, ApiPath::Tags, None, MAX_METADATA_BYTES, &guard)?;
        let tags: Value = serde_json::from_slice(&reply.body).map_err(|_| ModelError::Malformed)?;
        let listed = tags
            .get("models")
            .and_then(Value::as_array)
            .ok_or(ModelError::Malformed)?
            .iter()
            .any(|model| model.get("name").and_then(Value::as_str) == Some(identity.name()));
        if !listed {
            return Err(ModelError::NotInstalled);
        }
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

    /// Stub: the inherited chat.
    fn call(
        &self,
        request: &ModelRequest,
        ctx: &CallContext<'_>,
    ) -> Result<ModelReply, ModelError> {
        let guard = Guard::of(ctx);
        let body = json!({
            "model": self.identity.name(),
            "stream": false,
            "format": "json",
            "options": { "temperature": 0 },
            "messages": [
                { "role": "system", "content": request.system() },
                { "role": "user", "content": request.user() },
            ],
        });
        let body = serde_json::to_vec(&body).map_err(|_| ModelError::Malformed)?;
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
        if !(200..300).contains(&reply.status) {
            return Err(ModelError::Unavailable);
        }
        let value: Value =
            serde_json::from_slice(&reply.body).map_err(|_| ModelError::Malformed)?;
        let content = value
            .pointer("/message/content")
            .and_then(Value::as_str)
            .ok_or(ModelError::Malformed)?;
        Ok(ModelReply::new(content.to_string(), 0, 0, latency_ms))
    }
}
