//! M2a-S (Codex REJECT, oracle first): hooks for deterministic
//! interleavings, and the Owner at the broker's dialogs, for the real-broker
//! oracles in `adapter/m2as_*.rs` (test code lives here so that the source
//! scans keep confirmers out of the adapter's own files).

use nexus_governed_control::authority::approval::{
    AcknowledgementConfirmation, ActionConfirmation, ControlConfirmer, GrantConfirmation,
    MissionConfirmation, ResumeConfirmation,
};
use std::sync::{Arc, Mutex};

type Hook = Box<dyn FnOnce() + Send>;

/// One armed hook: it runs on the call after `skip` more calls, once.
#[derive(Clone, Default)]
pub(crate) struct HookSlot(Arc<Mutex<Option<(usize, Hook)>>>);

impl HookSlot {
    pub fn arm(&self, skip: usize, hook: impl FnOnce() + Send + 'static) {
        *self.0.lock().unwrap() = Some((skip, Box::new(hook)));
    }

    pub fn disarm(&self) {
        *self.0.lock().unwrap() = None;
    }

    pub fn armed(&self) -> bool {
        self.0.lock().unwrap().is_some()
    }

    pub(crate) fn fire(&self) {
        let hook = {
            let mut slot = self.0.lock().unwrap();
            match slot.as_mut() {
                Some((0, _)) => slot.take().map(|(_, hook)| hook),
                Some((n, _)) => {
                    *n -= 1;
                    None
                }
                None => None,
            }
        };
        if let Some(hook) = hook {
            hook();
        }
    }
}

/// The Owner at the mission dialog: runs its hook while asked, then
/// answers `yes`.
pub(crate) struct HookConfirmer {
    pub hook: HookSlot,
    pub yes: bool,
}

impl ControlConfirmer for HookConfirmer {
    fn confirm_action(&self, _: &ActionConfirmation) -> bool {
        false
    }
    fn confirm_grant(&self, _: &GrantConfirmation) -> bool {
        false
    }
    fn confirm_resume(&self, _: &ResumeConfirmation) -> bool {
        false
    }
    fn confirm_acknowledgement(&self, _: &AcknowledgementConfirmation) -> bool {
        false
    }
    fn confirm_mission(&self, _: &MissionConfirmation) -> bool {
        self.hook.fire();
        self.yes
    }
}

/// The Owner approving an action at the broker's native dialog.
pub(crate) struct OwnerApproves;
impl ControlConfirmer for OwnerApproves {
    fn confirm_action(&self, _: &ActionConfirmation) -> bool {
        true
    }
    fn confirm_grant(&self, _: &GrantConfirmation) -> bool {
        false
    }
    fn confirm_resume(&self, _: &ResumeConfirmation) -> bool {
        false
    }
    fn confirm_acknowledgement(&self, _: &AcknowledgementConfirmation) -> bool {
        false
    }
}
