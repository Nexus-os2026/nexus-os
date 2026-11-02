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

/// M2c-B (the child-process crash oracles): the Owner granting a capability
/// natively; every other dialog no.
pub(crate) struct OwnerGrants;
impl ControlConfirmer for OwnerGrants {
    fn confirm_action(&self, _: &ActionConfirmation) -> bool {
        false
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
}

/// M2c-B: the Owner acknowledging unsettled actions (keeping how many were
/// shown) and lifting the emergency stop; every other dialog no.
#[derive(Default)]
pub(crate) struct OwnerRecovers {
    pub unsettled: std::sync::atomic::AtomicUsize,
}
impl ControlConfirmer for OwnerRecovers {
    fn confirm_action(&self, _: &ActionConfirmation) -> bool {
        false
    }
    fn confirm_grant(&self, _: &GrantConfirmation) -> bool {
        false
    }
    fn confirm_resume(&self, _: &ResumeConfirmation) -> bool {
        true
    }
    fn confirm_acknowledgement(&self, request: &AcknowledgementConfirmation) -> bool {
        self.unsettled
            .store(request.unsettled.len(), std::sync::atomic::Ordering::SeqCst);
        true
    }
}
