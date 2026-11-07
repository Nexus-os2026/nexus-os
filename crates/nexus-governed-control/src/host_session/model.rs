//! SB-2a inert private surface. No route is exposed.
use super::*;
use crate::inference::{ModelIdentityView, ModelPinSpec};
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;
#[derive(Default)]
pub(super) struct ModelSlot {
    pub(super) state: Mutex<State>,
    pub(super) uncertain: AtomicBool,
}
#[derive(Default)]
pub(super) struct State {
    pub(super) attempted: bool,
}
impl ModelSlot {
    #[cfg(test)]
    pub(super) fn installed(&self) -> bool {
        false
    }
    pub(super) fn ready(&self) -> bool {
        true
    }
    pub(super) fn quiescent(&self) -> bool {
        true
    }
    pub(super) fn snapshot(&self) -> Option<close::ModelObligations> {
        let attempted = self.state.lock().ok()?.attempted;
        Some(close::ModelObligations {
            operation: if attempted { "registration" } else { "none" },
            active: false,
            owner_complete: true,
            child_complete: true,
            record: "not_required",
            uncertain: self.uncertain.load(Ordering::SeqCst),
        })
    }
}
pub(super) struct Owner {
    pub(super) core: Arc<Core>,
}
pub(super) fn reserve(core: &Arc<Core>, _: &Arc<RequestScope>) -> Result<Owner, SessionError> {
    let _ = core;
    Err(SessionError::Unavailable)
}
impl Owner {
    pub(super) fn complete(self, _: &'static str) -> Result<(), SessionError> {
        let _ = &self.core;
        Err(SessionError::Unavailable)
    }
}
impl HostSession {
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "SB-2a: private, wired by SB-3; session route unavailable"
        )
    )]
    pub(super) fn register_model_private(
        &self,
        _: &str,
        _: &ModelPinSpec,
    ) -> Result<ModelIdentityView, SessionError> {
        let owner = reserve(&self.core, &self.bootstrap.scope)?;
        let watch = self
            .control
            .kernel_stop_authority()
            .admit()
            .map_err(|_| SessionError::Restricted)?;
        let deadline = crate::inference::AbsoluteDeadline::within(
            crate::inference::SystemClocks,
            Duration::from_secs(30),
        );
        let gate = RegistrationGate {
            core: &self.core,
            scope: &self.bootstrap.scope,
            primary: &self.evidence,
            watch: &watch,
            deadline: &deadline,
        };
        let _ = nexus_kernel::inference::CallCancel::admit_request(
            &gate,
            nexus_kernel::inference::RequestKind::Metadata,
        );
        owner.complete("not_required")?;
        Err(SessionError::Unavailable)
    }
}

pub(super) struct RegistrationGate<'a> {
    pub(super) core: &'a Arc<Core>,
    pub(super) scope: &'a Arc<RequestScope>,
    pub(super) primary: &'a crate::evidence_store::PrimaryEvidence,
    pub(super) watch: &'a nexus_kernel::emergency_stop::StopWatch,
    pub(super) deadline: &'a dyn crate::inference::CallDeadline,
}
impl nexus_kernel::inference::CallCancel for RegistrationGate<'_> {
    fn cancelled(&self) -> Option<nexus_kernel::inference::CancelCause> {
        let _ = (
            self.core,
            self.scope,
            self.primary,
            self.watch,
            self.deadline,
        );
        None
    }
    fn admit_request(
        &self,
        _: nexus_kernel::inference::RequestKind,
    ) -> Result<(), crate::inference::ModelError> {
        Err(crate::inference::ModelError::Cancelled)
    }
}
