//! M2c-A: the Owner at the broker's own dialogs, for the real-broker
//! oracles in `adapter/m2c_door.rs` (test code lives here so that the
//! source scans keep confirmers out of the adapter's own files).

use nexus_governed_control::authority::approval::{
    AcknowledgementConfirmation, ActionConfirmation, ControlConfirmer, GrantConfirmation,
    ResumeConfirmation,
};

/// The Owner lifting an emergency stop natively; every other dialog no.
pub(crate) struct OwnerLiftsTheStop;
impl ControlConfirmer for OwnerLiftsTheStop {
    fn confirm_action(&self, _: &ActionConfirmation) -> bool {
        false
    }
    fn confirm_grant(&self, _: &GrantConfirmation) -> bool {
        false
    }
    fn confirm_resume(&self, _: &ResumeConfirmation) -> bool {
        true
    }
    fn confirm_acknowledgement(&self, _: &AcknowledgementConfirmation) -> bool {
        false
    }
}
