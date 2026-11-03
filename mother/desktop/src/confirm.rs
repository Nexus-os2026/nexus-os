//! The desktop's one confirmer (M2b-A): no dialogs exist yet, so every
//! question the broker or Mother asks is answered no (M2c N4a). Mother
//! therefore gets no door from the desktop, and the window says so.

use nexus_governed_control::authority::approval::{
    AcknowledgementConfirmation, ActionConfirmation, AgentDoorConfirmation, ControlConfirmer,
    GrantConfirmation, MissionConfirmation, MissionResumeConfirmation, ResumeConfirmation,
};

/// Answers no to every question.
pub struct NoDialogs;

impl ControlConfirmer for NoDialogs {
    fn confirm_action(&self, _: &ActionConfirmation) -> bool {
        unimplemented!("M2b-A PR-2 code")
    }
    fn confirm_grant(&self, _: &GrantConfirmation) -> bool {
        unimplemented!("M2b-A PR-2 code")
    }
    fn confirm_resume(&self, _: &ResumeConfirmation) -> bool {
        unimplemented!("M2b-A PR-2 code")
    }
    fn confirm_acknowledgement(&self, _: &AcknowledgementConfirmation) -> bool {
        unimplemented!("M2b-A PR-2 code")
    }
    fn confirm_mission(&self, _: &MissionConfirmation) -> bool {
        unimplemented!("M2b-A PR-2 code")
    }
    fn confirm_agent_door(&self, _: &AgentDoorConfirmation) -> bool {
        unimplemented!("M2b-A PR-2 code")
    }
    fn confirm_held_mission(&self, _: &MissionResumeConfirmation) -> bool {
        unimplemented!("M2b-A PR-2 code")
    }
}
