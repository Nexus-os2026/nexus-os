//! M2c-A (A0, the mechanical commit): the one place where Mother's tests
//! build a port, ask the Owner's consent, resume a held mission and name
//! the test home. Every call site states its answer explicitly.
//!
//! The port goes over the agent door (commit 4): only these bodies changed.
//! The door dialog is answered with `door_answer`, the consent dialog by
//! `confirmer`, the held-mission dialog with `answer`, each through the
//! delegating confirmer bound into every test door
//! (`oracles::m2c_confirmers`), for that one call. On `main` each helper did
//! exactly what its call sites did (`GovernedPort::new(control.clone())`,
//! `request_consent(id, &c)`, `resume(id)`, the adapter's `Home`).

use super::{mother_agent, GovernedPort};
use crate::coordinator::{Coordinator, CoordinatorError};
use crate::effect::EffectPort;
use crate::journal::MissionId;
use crate::oracles::m2as_hooks::HookSlot;
use crate::oracles::m2c_confirmers::{
    answering, delegating, unexpected, DoorAnswer, HeldConfirmer,
};
use crate::state::MissionState;
use nexus_governed_control::authority::approval::{
    ControlConfirmer, MissionResumeApproval, MissionResumeConfirmation,
};
use nexus_governed_control::authority::AuthorityError;
use nexus_governed_control::governed::GovernedControl;
use std::sync::{Arc, Mutex};

/// The home Mother's tests build their broker in (test code only: Mother's
/// production code builds no broker).
pub(crate) use crate::test_home::TestHome as Home;

/// A port over `control`, its door issued with the Owner's `door_answer`.
/// A door the Owner refused gives no port.
pub(crate) fn port_for(control: &Arc<GovernedControl>, door_answer: bool) -> GovernedPort {
    let door = answering(&DoorAnswer(door_answer), || {
        control.issue_agent_door(mother_agent(), delegating())
    })
    .expect("no door: the Owner said no, or the broker refused it");
    GovernedPort::new(door)
}

/// Ask the Owner's consent to `id`; `confirmer` answers the mission dialog.
pub(crate) fn consent<P: EffectPort>(
    mother: &Coordinator<P>,
    id: MissionId,
    confirmer: &(dyn ControlConfirmer + Sync),
) -> Result<MissionState, CoordinatorError> {
    answering(confirmer, || mother.request_consent(id))
}

/// The Owner resumes the held mission `id`; `answer` is the Owner's answer
/// to the held-mission dialog.
pub(crate) fn resume<P: EffectPort>(
    mother: &Coordinator<P>,
    id: MissionId,
    answer: bool,
) -> Result<MissionState, CoordinatorError> {
    resume_with(mother, id, &HeldOwner::new(answer))
}

// ------------------------------------------------- M2c-A oracle helpers
//
// Pre-registered with the oracles (commit 2), against main's API; their
// bodies now go through the door.

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

    /// What the dialog showed (the held-mission confirmer keeps it).
    pub(crate) fn show(&self, shown: HeldShown) {
        self.shown.lock().unwrap().push(shown);
    }
}

/// The Owner resumes the held mission `id`, answering its dialog as
/// `owner` says.
pub(crate) fn resume_with<P: EffectPort>(
    mother: &Coordinator<P>,
    id: MissionId,
    owner: &HeldOwner,
) -> Result<MissionState, CoordinatorError> {
    answering(&HeldConfirmer(owner), || mother.resume(id))
}

/// Run `hook` once in `port`, after the broker answered a held-mission
/// request and before the coordinator takes the answer.
pub(crate) fn after_held_answer(port: &GovernedPort, hook: impl FnOnce() + Send + 'static) {
    port.after_held_answer.arm(0, hook);
}

/// A reserved held-mission request (the coordinator's resume, step 1).
pub(crate) struct ResumeAsk {
    request: MissionResumeConfirmation,
    generation: u64,
}

/// The broker's answer to a reserved request (step 2).
pub(crate) struct ResumeReply(Result<MissionResumeApproval, AuthorityError>);

pub(crate) fn resume_request<P: EffectPort>(
    mother: &Coordinator<P>,
    id: MissionId,
) -> Result<ResumeAsk, CoordinatorError> {
    let (request, generation) = mother.resume_request(id)?;
    Ok(ResumeAsk {
        request,
        generation,
    })
}

/// Ask the broker, through `port`'s door, about `ask`; the Owner answers
/// `yes`.
pub(crate) fn resume_reply(port: &GovernedPort, ask: &ResumeAsk, yes: bool) -> ResumeReply {
    let owner = HeldOwner::new(yes);
    ResumeReply(answering(&HeldConfirmer(&owner), || {
        port.confirm_held_mission(&ask.request)
    }))
}

/// Take `reply` for the request `ask` reserved (step 3).
pub(crate) fn take_resume<P: EffectPort>(
    mother: &Coordinator<P>,
    id: MissionId,
    ask: &ResumeAsk,
    reply: ResumeReply,
) -> Result<MissionState, CoordinatorError> {
    mother.take_resume(id, ask.generation, reply.0)
}

/// Dialogs the test door's confirmer was asked on this thread with no
/// answer installed (answered no).
pub(crate) fn unexpected_dialogs() -> usize {
    unexpected()
}
