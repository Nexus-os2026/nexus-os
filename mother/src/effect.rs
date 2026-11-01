//! The EffectPort: Mother's only way to the world.
//!
//! Every file, network, process, model-tool or spend effect a lane asks for
//! is an [`EffectRequest`] (data, never authority). The coordinator checks
//! it against the mission's state, consent, forbidden effects and budget,
//! journals it, and only then hands it to the port. The trait is sealed:
//! outside this crate only the adapter over the governed-control broker
//! ([`crate::adapter::GovernedPort`]) is a port.
//!
//! The port also reaches the broker's authority, where the Owner is asked
//! to consent to a mission (`Authority::confirm_mission`), and reports what
//! became of an action that waits for the Owner's native approval. Mother
//! never approves an action herself: the Owner does, at the broker.

use crate::contract::EffectKind;
use crate::journal::MissionId;
use nexus_governed_control::authority::Authority;
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

/// What became of the action a mission waits on, as the broker says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApprovalStatus {
    /// The Owner has not answered yet (or the approved action is still
    /// running at the broker).
    Waiting,
    /// The Owner approved it and it ran, once, at the broker.
    Approved,
    /// The Owner denied it: it never ran.
    Denied,
    /// Its approval expired: it never ran.
    Expired,
    /// It ended otherwise (revoked, failed, cancelled, or its outcome is
    /// unknown): only a stable class name.
    Lapsed { class: String },
}

/// The identity of an action a mission waits on at the broker: opaque
/// names of its run and commitment (never authority: the port checks them
/// against the broker's own records before using them), and the port's
/// monotonic deadline for it, never later than the broker's. Mother
/// journals it so that a rebuilt port can still withdraw the action.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PendingAction {
    pub run: String,
    pub commitment: String,
    pub deadline_mono_ms: u64,
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
    /// The broker's authority, where the Owner consents to a mission.
    fn broker(&self) -> &Authority;
    /// What became of the action `mission` waits on (an effect that came
    /// back [`EffectOutcome::AwaitingApproval`]). Never approves it.
    fn approval(&self, mission: MissionId) -> ApprovalStatus;
    /// The identity of the action `mission` waits on, if the port knows it.
    fn pending(&self, mission: MissionId) -> Option<PendingAction>;
    /// A reopened coordinator hands back the journaled action of a mission
    /// that has not ended, so that this port can still withdraw it (cancel)
    /// and ask about it. The port uses it only if the broker's records
    /// agree: Mother's agent run, and that run's commitment.
    fn adopt(&self, mission: MissionId, pending: &PendingAction);
    /// Whether the broker no longer holds `pending` open: it is Mother's
    /// and in a final state, or the broker holds no such action. Only then
    /// does Mother journal it closed.
    fn settled(&self, pending: &PendingAction) -> bool;
    /// Cancel the mission at the port, as [`EffectPort::cancel`], and say
    /// whether the broker took the cancellation. Recovery and every end of
    /// a mission use it, so that a withdrawal that failed is an error.
    fn withdraw(&self, mission: MissionId) -> Result<(), EffectRefusal>;
    /// M2a-S: the coordinator broke (a journal append failed): cancel every
    /// broker run this port holds and open or take back no new one.
    fn fence(&self);
    /// M2a-S: a journaled action this port is not given back (claimed by
    /// two missions): recorded, so that withdrawing it is an error while
    /// the broker still holds it open.
    fn hand(&self, mission: MissionId, pending: &PendingAction);
}
