//! Typed ownership boundary for the one File mission. No Deref or route back
//! to the count-only legacy Stop API is exposed.
use super::*;
use crate::effect::Admission;
use nexus_governed_control::authority::file_session::{HaltCause, HaltCell};
use std::sync::atomic::{AtomicU64, Ordering};

pub(super) struct FileLife {
    pub(super) cell: HaltCell,
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
            self.cell.engage_with(HaltCause::TimeBudget);
        }
    }
}

/// Owns the coordinator exclusively. Its Stop calls return recording errors.
pub struct FileSession<P: EffectPort> {
    coordinator: Coordinator<P>,
}

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
        self.file = Some(FileLife {
            cell,
            mission: AtomicU64::new(0),
            mono_deadline: AtomicU64::new(u64::MAX),
            wall_deadline: AtomicU64::new(u64::MAX),
        });
        Ok(FileSession { coordinator: self })
    }

    pub(super) fn file_admit(&self, id: MissionId) -> Result<(), CoordinatorError> {
        let admission = self.with_lock(|inner| {
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
                self.life().cell.engage_with(HaltCause::Fence);
                Err(CoordinatorError::Poisoned)
            }
        }
    }
    fn touch(&self) -> Result<(), CoordinatorError> {
        self.life().check_budget(&*self.coordinator.clock);
        if self.life().cell.is_engaged() {
            let id = self.life().mission.load(Ordering::SeqCst);
            if id != 0 {
                self.call(|c| {
                    c.stop_for(
                        MissionId(id),
                        match self.life().cell.cause() {
                            Some(HaltCause::Emergency) => StopReason::EmergencyStop,
                            _ => StopReason::Owner,
                        },
                    )
                })?;
            }
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
        self.life().cell.engage();
        self.call(|c| c.stop(id))
    }
    pub fn stop_all(&self) -> Result<usize, CoordinatorError> {
        self.life().cell.engage();
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
    pub fn close(&self) -> Result<(), CoordinatorError> {
        self.touch()?;
        self.call(|c| {
            c.with_lock(|inner| {
                if inner
                    .missions
                    .values()
                    .any(|m| m.file_mode && !m.state.is_terminal())
                {
                    return Err(CoordinatorError::FileUnfinished);
                }
                Ok(())
            })?;
            c.close_ref()
        })
    }
}
