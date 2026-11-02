//! M2c-A (A0, the mechanical commit): the one place where Mother's tests
//! build a port, ask the Owner's consent, resume a held mission and name
//! the test home. Every call site states its answer explicitly.
//!
//! On `main` each helper does exactly what its call sites did
//! (`GovernedPort::new(control.clone())`, `request_consent(id, &c)`,
//! `resume(id)`, the adapter's `Home`). When the port goes over the agent
//! door, only these bodies change: the door dialog is answered with
//! `door_answer`, the consent dialog by `confirmer`, the held-mission
//! dialog with `answer`.

use super::GovernedPort;
use crate::coordinator::{Coordinator, CoordinatorError};
use crate::effect::EffectPort;
use crate::journal::MissionId;
use crate::state::MissionState;
use nexus_governed_control::authority::approval::ControlConfirmer;
use nexus_governed_control::governed::GovernedControl;
use std::sync::Arc;

/// The home Mother's tests build their broker in.
pub(crate) use super::Home;

/// A port over `control`, its door issued with the Owner's `door_answer`.
/// On `main` there is no door: nothing is asked.
pub(crate) fn port_for(control: &Arc<GovernedControl>, door_answer: bool) -> GovernedPort {
    let _ = door_answer;
    GovernedPort::new(control.clone())
}

/// Ask the Owner's consent to `id`; `confirmer` answers the mission dialog.
pub(crate) fn consent<P: EffectPort>(
    mother: &Coordinator<P>,
    id: MissionId,
    confirmer: &(dyn ControlConfirmer + Sync),
) -> Result<MissionState, CoordinatorError> {
    mother.request_consent(id, confirmer)
}

/// The Owner resumes the held mission `id`; `answer` is the Owner's answer
/// to the held-mission dialog. On `main` nothing is asked.
pub(crate) fn resume<P: EffectPort>(
    mother: &Coordinator<P>,
    id: MissionId,
    answer: bool,
) -> Result<MissionState, CoordinatorError> {
    let _ = answer;
    mother.resume(id)
}
