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
use crate::oracles::m2as_hooks::HookSlot;
use crate::state::MissionState;
use nexus_governed_control::authority::approval::ControlConfirmer;
use nexus_governed_control::governed::GovernedControl;
use std::sync::{Arc, Mutex};

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

// ------------------------------------------------- M2c-A oracle helpers
//
// Pre-registered with the oracles (commit 2), against main's API. Main has
// no held-mission dialog: there `resume_with` resumes without asking (A5
// is red there), and the hook after the broker's answer runs before the
// resume, the nearest point main has.

/// What the held-mission dialog showed the Owner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct HeldShown {
    pub mission: String,
    pub contract_hash: String,
    pub request: String,
    pub held_reason: String,
}

/// The Owner at the held-mission dialog: answers `yes`, keeps what it was
/// shown, and runs its `during` hook once while it is asked.
pub(crate) struct HeldOwner {
    pub yes: bool,
    pub during: HookSlot,
    shown: Mutex<Vec<HeldShown>>,
}

impl HeldOwner {
    pub fn new(yes: bool) -> Self {
        Self {
            yes,
            during: HookSlot::default(),
            shown: Mutex::new(Vec::new()),
        }
    }

    pub fn shown(&self) -> Vec<HeldShown> {
        self.shown.lock().unwrap().clone()
    }
}

/// The Owner resumes the held mission `id`, answering its dialog as
/// `owner` says.
pub(crate) fn resume_with<P: EffectPort>(
    mother: &Coordinator<P>,
    id: MissionId,
    owner: &HeldOwner,
) -> Result<MissionState, CoordinatorError> {
    let _ = (owner.yes, &owner.during, &owner.shown);
    mother.resume(id)
}

/// Run `hook` once in `port`, after the broker answered a held-mission
/// request and before the coordinator takes the answer.
pub(crate) fn after_held_answer(port: &GovernedPort, hook: impl FnOnce() + Send + 'static) {
    let _ = port;
    hook();
}

/// A reserved held-mission request (the coordinator's resume, step 1).
pub(crate) struct ResumeAsk(());

/// The broker's answer to a reserved request (step 2).
pub(crate) struct ResumeReply(());

pub(crate) fn resume_request<P: EffectPort>(
    mother: &Coordinator<P>,
    id: MissionId,
) -> Result<ResumeAsk, CoordinatorError> {
    let _ = (mother, id);
    Ok(ResumeAsk(()))
}

/// Ask the broker, through `port`'s door, about `ask`; the Owner answers
/// `yes`.
pub(crate) fn resume_reply(port: &GovernedPort, ask: &ResumeAsk, yes: bool) -> ResumeReply {
    let _ = (port, ask, yes);
    ResumeReply(())
}

/// Take `reply` for the request `ask` reserved (step 3).
pub(crate) fn take_resume<P: EffectPort>(
    mother: &Coordinator<P>,
    id: MissionId,
    ask: &ResumeAsk,
    reply: ResumeReply,
) -> Result<MissionState, CoordinatorError> {
    let _ = (ask, reply);
    mother.resume(id)
}

/// Dialogs the test door's confirmer was asked on this thread with no
/// answer installed (answered no). Main has no door: none.
pub(crate) fn unexpected_dialogs() -> usize {
    0
}
