//! Swarm adapters — thin wrappers that expose real agent crates (or NYI
//! descriptor stubs) to the swarm as [`SwarmCapability`](crate::SwarmCapability)
//! implementations.
//!
//! ## Shipped adapters (Phase 1)
//!
//! Each adapter accepts a prompt through the invocation inputs and invokes
//! the resolved provider directly. The heavy-weight agent entry points
//! (coder's `fix_until_pass`, collaboration's blackboard, etc.) are not yet
//! hoisted into a single entry function — Phase 2 will wire those through
//! properly. For now the adapter pattern is: **prompt in → provider →
//! text out**, with the agent-specific prompt prefix encoded in the
//! adapter.
//!
//! - [`artisan`] ← wraps `coder-agent` (code generation / fix loop style)
//! - [`herald`] ← wraps `social-poster-agent` (social content style)
//! - [`broker`] ← wraps `nexus-collaboration` (agent coordination style)
//!
//! ## NYI stubs (Phase 1)
//!
//! Three descriptor stubs exist so the Director prompt and
//! `~/.nexus/swarm_routing.toml` can reference them without crashing.
//! `CapabilityRegistry::select_for_task` skips any stub. Calling `.run()` on
//! a stub returns [`crate::SwarmError::RegistryMiss`].
//!
//! - [`scout`]
//! - [`watchdog`]
//! - [`prospector`]

pub mod artisan;
pub mod broker;
pub mod herald;
pub mod prospector;
pub mod scout;
pub mod watchdog;

pub use artisan::ArtisanAdapter;
pub use broker::BrokerAdapter;
pub use herald::HeraldAdapter;
pub use prospector::ProspectorStub;
pub use scout::ScoutStub;
pub use watchdog::WatchdogStub;

use crate::capability::{AgentCapabilityDescriptor, CapabilityInvocation};
use crate::error::SwarmError;
use crate::provider::{InvokeRequest, Provider};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

/// Shared glue that real adapters use to pick the resolved provider and call
/// it. Kept in the module root so the three real adapters share it.
pub(crate) async fn invoke_resolved_provider(
    providers: &HashMap<String, Arc<dyn Provider>>,
    invocation: &CapabilityInvocation,
    prompt: String,
    max_tokens: u32,
) -> Result<Value, SwarmError> {
    let route = invocation
        .inputs
        .get("route")
        .ok_or_else(|| SwarmError::DirectorParse("invocation missing `route`".into()))?;
    let provider_id = route
        .get("provider_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SwarmError::DirectorParse("route missing provider_id".into()))?;
    let model_id = route
        .get("model_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SwarmError::DirectorParse("route missing model_id".into()))?;

    let provider = providers
        .get(provider_id)
        .ok_or_else(|| SwarmError::ProviderUnreachable {
            provider_id: provider_id.into(),
            reason: "provider not registered in adapter context".into(),
        })?;

    let resp = provider
        .invoke(InvokeRequest {
            model_id: model_id.into(),
            prompt,
            max_tokens,
            temperature: Some(0.2),
            metadata: Value::Null,
        })
        .await?;
    Ok(serde_json::json!({
        "text": resp.text,
        "tokens_in": resp.tokens_in,
        "tokens_out": resp.tokens_out,
        "cost_cents": resp.cost_cents,
        "model_id": resp.model_id,
    }))
}

/// Build a descriptor used by all three NYI stubs. Centralised so the stub
/// files are thin and the `todo_reason` discovery lands cleanly in `cargo doc`.
pub(crate) fn stub_descriptor(
    id: &'static str,
    name: &'static str,
    role: &'static str,
    todo_reason: &'static str,
) -> AgentCapabilityDescriptor {
    AgentCapabilityDescriptor {
        id: id.into(),
        name: name.into(),
        role: role.into(),
        task_profile_default: crate::profile::TaskProfile::local_light(),
        input_schema: serde_json::json!({}),
        output_schema: serde_json::json!({}),
        max_parallel: 0,
        cost_class: crate::profile::CostClass::Free,
        todo_reason: Some(todo_reason),
    }
}
