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

// ------------------------------------------------- M2c-B oracle helpers
//
// Pre-registered with the PR-B oracles, against PR-A's API (dd503cdc). On
// PR-A there is no halt, no record batch and no counter: those helpers
// answer "none" there, and the oracles that need them are red.

/// A port over `control`, or none if the Owner says no to its door (or the
/// broker refuses it).
pub(crate) fn try_port_for(
    control: &Arc<GovernedControl>,
    door_answer: bool,
) -> Option<GovernedPort> {
    answering(&DoorAnswer(door_answer), || {
        control.issue_agent_door(mother_agent(), delegating())
    })
    .ok()
    .map(GovernedPort::new)
}

/// What the port's door answers when asked to open a run (an acting call),
/// as the broker's stable class name: a halted door says "halted".
pub(crate) fn door_open_run(port: &GovernedPort) -> Result<(), String> {
    port.door
        .open_run()
        .map(|_| ())
        .map_err(|e| e.class().to_string())
}

/// The halt records the broker counted lost (none on PR-A: no counter).
pub(crate) fn halt_lost(control: &GovernedControl) -> u64 {
    let _ = control;
    0
}

/// The halt records the broker counts as still to write.
pub(crate) fn halt_pending(control: &GovernedControl) -> u64 {
    let _ = control;
    0
}

/// Where a test may stop a Mother call (the child-process crash points and
/// the forced schedules of design §7.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CallPoint {
    /// In a journal failure: after the fence, before the poison marker.
    FenceToMarker,
    /// In `with_lock`: the coordinator lock released, before its batches
    /// are written (also when there are none).
    BeforeOwedWrite,
    /// In `with_lock`: after its batches were written.
    AfterOwedWrite,
}

/// Run `hook` once at `point` of the next Mother call that reaches it (PR-A
/// has none of these points: nothing runs).
pub(crate) fn arm_call_point<P: EffectPort>(
    mother: &Coordinator<P>,
    point: CallPoint,
    hook: impl FnOnce() + Send + 'static,
) {
    let _ = (mother, point);
    drop(hook);
}

/// The port's fence, as the coordinator's break calls it (on PR-B its
/// records are written here, with no lock held).
pub(crate) fn fence_port(port: &GovernedPort) {
    port.fence();
}
