//! Memory-only Owner answers for the real session fixture.
use nexus_governed_control::authority::approval::*;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Default)]
pub(crate) struct Owner {
    pub decline: AtomicBool,
}
impl ControlConfirmer for Owner {
    fn confirm_action(&self, _: &ActionConfirmation) -> bool {
        !self.decline.load(Ordering::SeqCst)
    }
    fn confirm_grant(&self, _: &GrantConfirmation) -> bool {
        true
    }
    fn confirm_resume(&self, _: &ResumeConfirmation) -> bool {
        false
    }
    fn confirm_acknowledgement(&self, _: &AcknowledgementConfirmation) -> bool {
        false
    }
    fn confirm_agent_door(&self, _: &AgentDoorConfirmation) -> bool {
        true
    }
    fn confirm_mission(&self, _: &MissionConfirmation) -> bool {
        true
    }
}
