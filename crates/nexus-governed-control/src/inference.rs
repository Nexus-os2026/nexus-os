//! The broker's local-model capability (Mother Nexus M2e-B, design v2 §8).
//!
//! INTERFACE STUBS (M2e-B commit 1): the frozen door API (I-E2) declared
//! with no guarantee, so that the pre-registered oracles compile and are
//! red for behavioural reasons. The next commits replace every stub.

use crate::authority::ids::{AgentId, RunId};
use crate::authority::AuthorityError;
use nexus_kernel::emergency_stop::StopAuthority;
use nexus_kernel::inference::{
    CallCancel, CallContext, CallDeadline, CallMeter, CancelCause, LoopbackEndpoint, ModelError,
    ModelPinSpec, ModelReply, ModelRequest, ModelTransport, VerifiedOllama,
};
use std::sync::{Arc, Mutex};

#[cfg(all(test, target_os = "linux"))]
#[path = "inference_tests.rs"]
mod tests;

/// The registered model, as a door may see it: its name, its manifest
/// digest and its identity digest (each plain hex or the name grammar).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelIdentityView {
    pub name: String,
    pub digest: String,
    /// The hex of `ModelIdentity::digest()`.
    pub identity: String,
}

/// The one local model of this process.
pub(crate) struct RegisteredModel {
    pub(crate) transport: Arc<VerifiedOllama>,
    pub(crate) view: ModelIdentityView,
    pub(crate) identity: [u8; 32],
}

/// STUB: the registered model, with no single-model rule.
#[derive(Default)]
pub(crate) struct LocalModels {
    slot: Mutex<Option<Arc<RegisteredModel>>>,
}

struct Never;

impl CallCancel for Never {
    fn cancelled(&self) -> Option<CancelCause> {
        None
    }
}

impl CallDeadline for Never {
    fn expired(&self) -> bool {
        false
    }
}

impl LocalModels {
    /// STUB: verified with no deadline and no cancellation; not recorded.
    pub(crate) fn register(
        &self,
        endpoint: &str,
        spec: &ModelPinSpec,
        stop: &Arc<StopAuthority>,
    ) -> Result<ModelIdentityView, AuthorityError> {
        let endpoint = LoopbackEndpoint::parse(endpoint).map_err(AuthorityError::ModelCall)?;
        let watch = stop.admit().map_err(|_| AuthorityError::EmergencyStopped)?;
        let transport = VerifiedOllama::verify(endpoint, spec, &Never, &Never, &watch)
            .map_err(AuthorityError::ModelCall)?;
        let identity = transport.identity().digest();
        let view = ModelIdentityView {
            name: transport.identity().name().to_string(),
            digest: transport.identity().model_digest().to_string(),
            identity: hex::encode(identity),
        };
        *self.slot.lock().expect("model slot") = Some(Arc::new(RegisteredModel {
            transport: Arc::new(transport),
            view: view.clone(),
            identity,
        }));
        Ok(view)
    }

    pub(crate) fn registered(&self) -> Option<Arc<RegisteredModel>> {
        self.slot.lock().expect("model slot").clone()
    }
}

/// STUB: one call with no admission, ceiling, cancellation or record.
pub(crate) fn call(
    model: &RegisteredModel,
    _run: RunId,
    _agent: &AgentId,
    request: &ModelRequest,
    meter: &dyn CallMeter,
    deadline: &dyn CallDeadline,
    stop: &Arc<StopAuthority>,
) -> Result<ModelReply, AuthorityError> {
    let watch = stop.admit().map_err(|_| AuthorityError::EmergencyStopped)?;
    let ctx = CallContext {
        meter,
        cancel: &Never,
        deadline,
        stop: &watch,
    };
    model
        .transport
        .call(request, &ctx)
        .map_err(AuthorityError::ModelCall)
}

/// The class of a model-call refusal: `model_` and the kernel's code.
pub(crate) fn model_class(error: ModelError) -> &'static str {
    MODEL_CLASSES[error as usize]
}

/// `model_` and each of `ModelError::CODES`, in its order (a test keeps
/// them in step; a new kernel code changes the length and fails to build
/// here).
pub(crate) const MODEL_CLASSES: [&str; ModelError::CODES.len()] = [
    "model_not_local",
    "model_invalid_address",
    "model_invalid_model_name",
    "model_cloud_name",
    "model_cloud_enabled",
    "model_not_installed",
    "model_digest_mismatch",
    "model_runtime_mismatch",
    "model_remote",
    "model_config_mismatch",
    "model_unavailable",
    "model_timeout",
    "model_request_too_large",
    "model_response_too_large",
    "model_malformed",
    "model_truncated",
    "model_tool_calls",
    "model_thinking_leaked",
    "model_empty_content",
    "model_over_budget",
    "model_stopped",
    "model_cancelled",
    "model_late",
];
