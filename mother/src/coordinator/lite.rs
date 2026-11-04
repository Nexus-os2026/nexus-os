//! Typed ownership boundary for the one File mission. No Deref or route back
//! to the count-only legacy Stop API is exposed.
use super::*;
use crate::effect::Admission;
#[cfg(test)]
use nexus_governed_control::authority::file_session::HaltCause;
use nexus_governed_control::authority::file_session::{HaltCell, HaltTag};
use std::sync::atomic::{AtomicU64, Ordering};

pub(super) struct FileLife {
    pub(super) cell: HaltCell,
    pub(super) terminal: std::sync::OnceLock<super::lite_intent::FileTerminal>,
    #[cfg(test)]
    after_engagement: crate::oracles::lite_engagement_hooks::SiteHook,
    pub(super) mission: AtomicU64,
    mono_deadline: AtomicU64,
    wall_deadline: AtomicU64,
}
impl FileLife {
    pub(super) fn consent_at(&self, mono: u64, wall: u64, duration: u64) {
        self.mono_deadline
            .store(mono.saturating_add(duration), Ordering::SeqCst);
        self.wall_deadline
            .store(wall.saturating_add(duration), Ordering::SeqCst);
    }
    pub(super) fn check_budget(&self, clock: &dyn Clock) {
        if clock.monotonic_ms() >= self.mono_deadline.load(Ordering::SeqCst)
            || clock.wall_ms() >= self.wall_deadline.load(Ordering::SeqCst)
        {
            self.cell.engage_tagged(HaltTag::TimeBudget);
            #[cfg(test)]
            self.after_engagement.fire("budget");
        }
    }
}

/// Owns the coordinator exclusively. Its Stop calls return recording errors.
pub struct FileSession<P: EffectPort> {
    coordinator: Coordinator<P>,
}

#[path = "../adapter/lite_review3.rs"]
#[cfg(test)]
mod review3;

#[path = "../adapter/lite_review4.rs"]
#[cfg(test)]
mod review4;

#[path = "../adapter/lite_review5.rs"]
#[cfg(test)]
mod review5;

impl<P: EffectPort> Coordinator<P> {
    /// Consume the legacy interface before selecting File mode. This cannot
    /// race a call through another shared coordinator reference: ownership is
    /// transferred, and the complete mission inventory is checked under lock.
    pub fn into_file_session(mut self) -> Result<FileSession<P>, CoordinatorError> {
        {
            let inner = self.inner.lock().map_err(|_| CoordinatorError::Poisoned)?;
            inner.usable()?;
            if inner.missions.values().any(|m| !m.state.is_terminal()) {
                return Err(CoordinatorError::FileExclusive);
            }
        }
        let attachment = self
            .file_attachment
            .clone()
            .ok_or(CoordinatorError::FileSession(FileSessionError::WrongOwner))?;
        let cell = self
            .port
            .file_claim(attachment)
            .map_err(CoordinatorError::FileSession)?;
        #[cfg(test)]
        if self.hooks.file_selection_poison.load(Ordering::SeqCst) {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _held = self.inner.lock().unwrap();
                panic!("selection lock poison");
            }));
        }
        match self.inner.lock() {
            Ok(mut inner) => inner.file_cell = Some(cell.clone()),
            Err(_) => {
                cell.engage_tagged(HaltTag::DoorHalted);
                #[cfg(test)]
                self.hooks.file_engagement.fire("selection-poison");
                return Err(CoordinatorError::Poisoned);
            }
        }
        self.file = Some(FileLife {
            terminal: std::sync::OnceLock::new(),
            #[cfg(test)]
            after_engagement: self.hooks.file_engagement.clone(),
            cell,
            mission: AtomicU64::new(0),
            mono_deadline: AtomicU64::new(u64::MAX),
            wall_deadline: AtomicU64::new(u64::MAX),
        });
        Ok(FileSession { coordinator: self })
    }

    pub(super) fn file_admit(&self, id: MissionId) -> Result<(), CoordinatorError> {
        let admission = self.step(|inner| {
            let m = inner.mission(id)?;
            if !m.file_mode || !m.consented() {
                return Err(CoordinatorError::NoConsent);
            }
            Ok(Admission {
                contract_hash: m.contract_ref.contract_hash.to_hex(),
                consent_request: m
                    .consent_request
                    .clone()
                    .ok_or(CoordinatorError::NoConsent)?,
                folder: m
                    .contract()?
                    .requested_folder
                    .clone()
                    .ok_or(CoordinatorError::NoConsent)?,
            })
        })?;
        self.port
            .file_admit(id, &admission)
            .map_err(|e| CoordinatorError::Broker(e.class))
    }
}

impl<P: EffectPort> FileSession<P> {
    #[cfg(test)]
    pub(crate) fn end_for_test(
        &self,
        id: MissionId,
        reason: BlockReason,
    ) -> Result<MissionState, CoordinatorError> {
        self.call(|c| c.end(id, reason))
    }
    #[cfg(test)]
    pub(crate) fn hooks_for_test(&self) -> &CoordinatorHooks {
        &self.coordinator.hooks
    }
    #[cfg(test)]
    pub(crate) fn invocations_for_test(&self) -> usize {
        self.coordinator
            .hooks
            .file_invocations
            .load(Ordering::SeqCst)
    }
    #[cfg(test)]
    pub(crate) fn cell_for_test(&self) -> HaltCell {
        self.life().cell.clone()
    }
    #[cfg(test)]
    pub(crate) fn poison_for_test(&self) {
        let _held = self.coordinator.inner.lock().unwrap();
        panic!("lite coordinator poison");
    }

    fn life(&self) -> &FileLife {
        self.coordinator
            .file
            .as_ref()
            .expect("FileSession owns its selected lifecycle")
    }

    /// Also catches a poisoned later stage or an unwinding external callback:
    /// the original cell remains engaged, and a typed error reaches the caller.
    fn call<T>(
        &self,
        body: impl FnOnce(&Coordinator<P>) -> Result<T, CoordinatorError>,
    ) -> Result<T, CoordinatorError> {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| body(&self.coordinator))) {
            Ok(result) => result,
            Err(_) => {
                self.life().cell.engage_tagged(HaltTag::DoorHalted);
                #[cfg(test)]
                self.coordinator
                    .hooks
                    .file_engagement
                    .fire("callback-panic");
                Err(CoordinatorError::Poisoned)
            }
        }
    }
    fn touch(&self) -> Result<(), CoordinatorError> {
        self.life().check_budget(&*self.coordinator.clock);
        let id = self.life().mission.load(Ordering::SeqCst);
        if self.life().cell.is_engaged() && id != 0 {
            #[cfg(test)]
            self.coordinator.hooks.file_after_halt.fire();
            self.call(|c| c.file_stop(MissionId(id), SettleCause::Stop(StopReason::Owner)))?;
        }
        Ok(())
    }
    fn runnable(&self) -> Result<(), CoordinatorError> {
        self.touch()?;
        if self.life().cell.is_engaged() {
            return Err(CoordinatorError::Stopped);
        }
        Ok(())
    }
    pub fn draft(&self, contract: MissionContract) -> Result<MissionId, CoordinatorError> {
        self.runnable()?;
        self.call(|c| c.draft(contract))
    }
    pub fn revise(
        &self,
        id: MissionId,
        contract: MissionContract,
    ) -> Result<MissionState, CoordinatorError> {
        self.runnable()?;
        self.call(|c| c.revise(id, contract))
    }
    pub fn submit(&self, id: MissionId) -> Result<MissionState, CoordinatorError> {
        self.runnable()?;
        self.call(|c| c.submit(id))
    }
    pub fn request_consent(&self, id: MissionId) -> Result<MissionState, CoordinatorError> {
        self.runnable()?;
        self.call(|c| c.request_consent(id))
    }
    pub fn advance(&self, id: MissionId) -> Result<Advance, CoordinatorError> {
        self.runnable()?;
        self.call(|c| {
            c.file_admit(id)?;
            c.advance(id)
        })
    }
    pub fn stop(&self, id: MissionId) -> Result<MissionState, CoordinatorError> {
        self.life().cell.engage_tagged(HaltTag::OwnerStop);
        #[cfg(test)]
        self.coordinator.hooks.file_engagement.fire("public-stop");
        #[cfg(test)]
        self.coordinator.hooks.file_after_halt.fire();
        self.call(|c| c.stop(id))
    }
    pub fn stop_all(&self) -> Result<usize, CoordinatorError> {
        self.life().cell.engage_tagged(HaltTag::OwnerStop);
        #[cfg(test)]
        self.coordinator
            .hooks
            .file_engagement
            .fire("public-stop-all");
        #[cfg(test)]
        self.coordinator.hooks.file_after_halt.fire();
        let id = self.life().mission.load(Ordering::SeqCst);
        if id == 0 {
            return Ok(0);
        }
        self.call(|c| {
            let terminal = c.state(MissionId(id)).is_some_and(|s| s.is_terminal());
            c.stop(MissionId(id))?;
            Ok(usize::from(!terminal))
        })
    }
    pub fn state(&self, id: MissionId) -> Result<Option<MissionState>, CoordinatorError> {
        self.touch()?;
        self.call(|c| Ok(c.state(id)))
    }
    pub fn journal(&self) -> Result<Vec<Entry>, CoordinatorError> {
        self.touch()?;
        self.call(Coordinator::journal)
    }
    pub fn reason(&self, id: MissionId) -> Result<Option<String>, CoordinatorError> {
        self.touch()?;
        self.call(|c| Ok(c.reason(id)))
    }
    /// The exact Owner notice persists for uncertain terminal work.
    pub fn notice(&self, id: MissionId) -> Result<Option<&'static str>, CoordinatorError> {
        self.touch()?;
        self.call(|c| c.file_notice(id))
    }
    pub fn close(&self) -> Result<(), CoordinatorError> {
        self.touch()?;
        self.call(|c| {
            c.step(|inner| {
                if inner
                    .missions
                    .values()
                    .any(|m| m.file_mode && !m.state.is_terminal())
                {
                    return Err(CoordinatorError::FileUnfinished);
                }
                Ok(())
            })?;
            let id = self.life().mission.load(Ordering::SeqCst);
            if id != 0 {
                c.file_ack_all(MissionId(id))?;
            }
            c.close_ref()
        })
    }
}
