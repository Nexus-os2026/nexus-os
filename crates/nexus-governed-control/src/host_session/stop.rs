//! Bounded event debt, separate from native worker and work-resource capacity.
//! Ingress has already denied before registering its event. Neither registration
//! nor receipt observation acquires a backend, evidence or resource hold.
use super::*;
use crate::authority::evidence::EndStatus;
use crate::authority::halt::BatchOutcome;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicUsize};

const CAPACITY: usize = 256;
const EMPTY: u8 = 0;
const PENDING: u8 = 1;
const IN_FLIGHT: u8 = 2;
const FINISHED: u8 = 3;
const UNCERTAIN: u8 = 4;
const FAILED_WORKER: u8 = 5;

struct Slot {
    generation: AtomicU64,
    phase: AtomicU8,
    receipt: OnceLock<StopReceipt>,
}
impl Default for Slot {
    fn default() -> Self {
        Self {
            generation: AtomicU64::new(0),
            phase: AtomicU8::new(EMPTY),
            receipt: OnceLock::new(),
        }
    }
}

pub(super) struct Stops {
    slots: [Slot; CAPACITY],
    next: AtomicUsize,
    active: AtomicUsize,
    overflowed: AtomicBool,
}
impl Default for Stops {
    fn default() -> Self {
        Self {
            slots: std::array::from_fn(|_| Slot::default()),
            next: AtomicUsize::new(0),
            active: AtomicUsize::new(0),
            overflowed: AtomicBool::new(false),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StopReceipt {
    pub event: IngressEvent,
    pub snapshot: crate::governed::StopSnapshot,
    pub native_stop_number: u64,
    pub ownership_established: bool,
    pub cancelled: usize,
    /// The exact existing-schema emergency_stop record, independently of
    /// synchronous return or records for consequences of the same narrowing.
    pub record: EndStatus,
    pub consequences: BatchOutcome,
}
impl StopReceipt {
    fn recorded(&self) -> bool {
        self.ownership_established
            && self.snapshot.kernel_stopped
            && self.snapshot.broker_stopped
            && matches!(self.record, EndStatus::Recorded { .. })
            && self.consequences.owed == 0
            && self.consequences.uncertain == 0
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StopObligations {
    pub received: usize,
    pub pending: usize,
    pub in_flight: usize,
    pub recorded: usize,
    pub owed: usize,
    pub uncertain: usize,
    pub failed: usize,
    pub overflowed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StopPhase {
    Pending,
    InFlight,
    Finished(StopReceipt),
    Uncertain,
    Failed,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StopObservation {
    pub event: IngressEvent,
    pub phase: StopPhase,
}

impl Stops {
    pub(super) fn quiescent(&self) -> bool {
        self.active.load(Ordering::SeqCst) == 0
    }
    pub(super) fn register(
        &self,
        core: &Arc<Core>,
        generation: u64,
    ) -> Result<IngressEvent, SessionError> {
        let index = self
            .next
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |old| {
                (old < CAPACITY).then_some(old + 1)
            })
            .map_err(|_| {
                self.overflowed.store(true, Ordering::SeqCst);
                SessionIngress { core: core.clone() }.fail();
                SessionError::Exhausted
            })?;
        let slot = &self.slots[index];
        slot.generation.store(generation, Ordering::SeqCst);
        slot.phase.store(PENDING, Ordering::SeqCst);
        Ok(IngressEvent {
            session: core.identity,
            generation,
            index,
        })
    }

    fn slot(&self, core: &Core, event: &IngressEvent) -> Result<&Slot, SessionError> {
        if event.session != core.identity || event.index >= self.next.load(Ordering::SeqCst) {
            return Err(SessionError::Restricted);
        }
        self.slots
            .get(event.index)
            .filter(|slot| slot.generation.load(Ordering::SeqCst) == event.generation)
            .ok_or(SessionError::Restricted)
    }
}

struct Worker {
    core: Arc<Core>,
    index: usize,
    finished: bool,
}
impl Drop for Worker {
    fn drop(&mut self) {
        if !self.finished {
            self.core.stops.slots[self.index]
                .phase
                .store(UNCERTAIN, Ordering::SeqCst);
            SessionIngress {
                core: self.core.clone(),
            }
            .fail();
        }
        self.core.stops.active.fetch_sub(1, Ordering::SeqCst);
    }
}

impl HostSession {
    /// One already accepted event. Native narrowing is not another ingress.
    /// The exact receipt is retained before return, including owed/uncertain
    /// evidence. A duplicate can only return that same retained receipt.
    pub fn record_stop(&self, event: &IngressEvent) -> Result<StopReceipt, SessionError> {
        if self.core.cell.session_word() & SEALED != 0 {
            return Err(SessionError::SessionEnded);
        }
        let slot = self.core.stops.slot(&self.core, event)?;
        if slot.phase.load(Ordering::SeqCst) == FINISHED {
            return slot.receipt.get().copied().ok_or(SessionError::Restricted);
        }
        if slot.phase.load(Ordering::SeqCst) != PENDING {
            return Err(SessionError::Unavailable);
        }
        self.core
            .stops
            .active
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |old| {
                (old < 4).then_some(old + 1)
            })
            .map_err(|_| SessionError::Backend("stop_workers_full"))?;
        if slot
            .phase
            .compare_exchange(PENDING, IN_FLIGHT, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            self.core.stops.active.fetch_sub(1, Ordering::SeqCst);
            return Err(SessionError::Unavailable);
        }
        let mut worker = Worker {
            core: self.core.clone(),
            index: event.index,
            finished: false,
        };
        #[cfg(test)]
        self.control
            .authority()
            .hooks()
            .fire("session.stop.started");
        let native = self.control.stop_session_event();
        #[cfg(test)]
        self.control
            .authority()
            .hooks()
            .fire("session.stop.returned");
        let receipt = StopReceipt {
            event: *event,
            snapshot: self.control.stop_snapshot(),
            native_stop_number: native.number,
            ownership_established: native.ownership_established,
            cancelled: native.cancelled,
            record: native.record,
            consequences: native.consequences,
        };
        slot.receipt
            .set(receipt)
            .map_err(|_| SessionError::Restricted)?;
        slot.phase.store(FINISHED, Ordering::SeqCst);
        worker.finished = true;
        if !receipt.ownership_established {
            self.ingress().fail();
        }
        Ok(receipt)
    }

    /// A spawn failure is a failed recording obligation, not completion.
    /// No event can be silently removed from the fixed receipt inventory.
    pub fn stop_spawn_failed(&self, event: &IngressEvent) -> Result<(), SessionError> {
        if self.core.cell.session_word() & SEALED != 0 {
            return Err(SessionError::SessionEnded);
        }
        let slot = self.core.stops.slot(&self.core, event)?;
        slot.phase
            .compare_exchange(PENDING, FAILED_WORKER, Ordering::SeqCst, Ordering::SeqCst)
            .map_err(|_| SessionError::Restricted)?;
        self.ingress().fail();
        Ok(())
    }

    pub fn stop_events(&self) -> Result<Vec<StopObservation>, SessionError> {
        let mut events = Vec::with_capacity(self.core.stops.next.load(Ordering::SeqCst));
        for (index, slot) in self
            .core
            .stops
            .slots
            .iter()
            .take(self.core.stops.next.load(Ordering::SeqCst))
            .enumerate()
        {
            let phase = match slot.phase.load(Ordering::SeqCst) {
                // Registration follows denial without a lock. Do not invent
                // an event receipt while its producer has not published it.
                EMPTY => continue,
                PENDING => StopPhase::Pending,
                IN_FLIGHT => StopPhase::InFlight,
                FINISHED => {
                    StopPhase::Finished(*slot.receipt.get().ok_or(SessionError::Restricted)?)
                }
                FAILED_WORKER => StopPhase::Failed,
                _ => StopPhase::Uncertain,
            };
            events.push(StopObservation {
                event: IngressEvent {
                    session: self.core.identity,
                    generation: slot.generation.load(Ordering::SeqCst),
                    index,
                },
                phase,
            });
        }
        events.sort_by_key(|event| event.event.generation);
        Ok(events)
    }

    pub fn stop_obligations(&self) -> Result<StopObligations, SessionError> {
        let mut result = StopObligations {
            overflowed: self.core.stops.overflowed.load(Ordering::SeqCst),
            ..StopObligations::default()
        };
        for event in self.stop_events()? {
            result.received += 1;
            match event.phase {
                StopPhase::Pending => result.pending += 1,
                StopPhase::InFlight => result.in_flight += 1,
                StopPhase::Uncertain => result.uncertain += 1,
                StopPhase::Failed => result.failed += 1,
                StopPhase::Finished(receipt) if receipt.recorded() => result.recorded += 1,
                StopPhase::Finished(receipt)
                    if !receipt.ownership_established
                        || receipt.record == EndStatus::Uncertain
                        || receipt.consequences.uncertain > 0 =>
                {
                    result.uncertain += 1
                }
                StopPhase::Finished(receipt)
                    if !receipt.snapshot.kernel_stopped || !receipt.snapshot.broker_stopped =>
                {
                    result.failed += 1
                }
                StopPhase::Finished(_) => result.owed += 1,
            }
        }
        let reserved = self.core.stops.next.load(Ordering::SeqCst);
        result.uncertain += reserved.saturating_sub(result.received);
        result.received = reserved;
        Ok(result)
    }
}
