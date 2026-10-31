//! The EffectPort: Mother's only way to the world.
//!
//! Every file, network, process, model-tool or spend effect a lane asks for
//! is an [`EffectRequest`] (data, never authority). The coordinator checks
//! it against the mission's state, consent, forbidden effects and budget,
//! journals it, and only then hands it to the port. The trait is sealed:
//! outside this crate only the adapter over the governed-control broker
//! ([`crate::adapter::GovernedPort`]) is a port.

use crate::contract::EffectKind;
use crate::journal::MissionId;
use nexus_governed_control::connector::ConnectorIntent;
use nexus_governed_control::egress::EgressIntent;
use nexus_governed_control::tool::ToolIntent;
use serde::{Deserialize, Serialize};

/// An effect a lane asks for.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "request", rename_all = "snake_case")]
pub enum EffectRequest {
    /// A path relative to the requested folder: a request, never a path.
    File { relative_path: String, write: bool },
    /// A network request (the broker's egress route).
    Network(EgressIntent),
    /// A governed process (the broker's tool route).
    Process(ToolIntent),
    /// An API tool a model asks for (the broker's connector route).
    ModelTool(ConnectorIntent),
    /// Spending money. M1's money budget is zero, so it never leaves Mother.
    Spend { amount_minor: u64, purpose: String },
}

impl EffectRequest {
    pub fn kind(&self) -> EffectKind {
        match self {
            EffectRequest::File { .. } => EffectKind::File,
            EffectRequest::Network(_) => EffectKind::Network,
            EffectRequest::Process(_) => EffectKind::Process,
            EffectRequest::ModelTool(_) => EffectKind::ModelTool,
            EffectRequest::Spend { .. } => EffectKind::Spend,
        }
    }
}

/// What a performed effect came to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EffectOutcome {
    /// The effect ran; its bounded text output, if any.
    Done { text: Option<String> },
    /// The broker committed it; the Owner approves or denies it natively.
    AwaitingApproval,
}

/// The port or the broker refused: only a stable class name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EffectRefusal {
    pub class: String,
}

mod sealed {
    pub trait Sealed {}
    impl Sealed for crate::adapter::GovernedPort {}
    #[cfg(test)]
    impl Sealed for crate::oracles::RecordingPort {}
}

/// The calls Mother makes to reach the world.
pub trait EffectPort: Send + Sync + sealed::Sealed {
    /// Perform one effect for `mission`.
    fn perform(
        &self,
        mission: MissionId,
        effect: &EffectRequest,
    ) -> Result<EffectOutcome, EffectRefusal>;
    /// The mission stopped, was denied, blocked or failed: nothing more of
    /// it may start, and what runs sees its cancellation.
    fn cancel(&self, mission: MissionId);
    /// The mission is done: finish what it opened once nothing waits.
    fn finish(&self, mission: MissionId);
    /// Whether the Owner's emergency stop holds.
    fn emergency_stopped(&self) -> bool;
}
