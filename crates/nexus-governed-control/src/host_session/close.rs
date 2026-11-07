//! Closed lifecycle accounting. Producers are retained until their owned
//! handles have joined; only three explicitly reserved operations follow seal.
use super::*;
use std::sync::atomic::AtomicU8;
use std::sync::Mutex;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProducerKind {
    Input,
    Startup,
    Work,
    StopWorker,
    Reporter,
    Observation,
    Output,
}
struct ProducerSlot {
    generation: u64,
    live: bool,
    lost: bool,
}
pub(super) struct Producers {
    next: u64,
    slots: [ProducerSlot; 9],
}
impl Default for Producers {
    fn default() -> Self {
        Self {
            next: 0,
            slots: std::array::from_fn(|_| ProducerSlot {
                generation: 0,
                live: false,
                lost: false,
            }),
        }
    }
}
/// A join obligation, never permission to perform work. The trusted executor
/// consumes this only after joining the exact thread that owns it.
pub struct Producer {
    core: Arc<Core>,
    index: usize,
    generation: u64,
    joined: bool,
}
impl Producer {
    pub fn joined(mut self) {
        match self.core.producers.lock() {
            Ok(mut producers) => {
                let slot = &mut producers.slots[self.index];
                if slot.live && !slot.lost && slot.generation == self.generation {
                    slot.live = false;
                    self.joined = true;
                }
            }
            Err(_) => SessionIngress {
                core: self.core.clone(),
            }
            .fail(),
        }
    }
}
impl Drop for Producer {
    fn drop(&mut self) {
        if !self.joined {
            // Storage is retained even when this metadata lock is poisoned.
            SessionIngress {
                core: self.core.clone(),
            }
            .fail();
            if let Ok(mut producers) = self.core.producers.lock() {
                let slot = &mut producers.slots[self.index];
                if slot.generation == self.generation {
                    slot.lost = true;
                }
            }
        }
    }
}
impl SessionIngress {
    pub fn state(&self) -> SessionState {
        state(self.core.cell.session_word())
    }
    pub fn generation(&self) -> u64 {
        self.core.cell.session_word() / EPOCH_STEP
    }
    pub fn failed(&self) -> bool {
        self.core.cell.session_word() & FAILED != 0
    }
    pub fn emergency_seen(&self) -> bool {
        self.core.cell.session_word() & EMERGENCY != 0
    }
    pub fn close_deadline(&self) -> Option<Instant> {
        let nanos = self.core.shutdown_nanos.load(Ordering::SeqCst);
        (nanos != 0).then(|| self.core.created + Duration::from_nanos(nanos))
    }
    pub(super) fn begin_shutdown(&self, accepted: Instant) -> Instant {
        let nanos = accepted
            .duration_since(self.core.created)
            .as_nanos()
            .min(u128::from(u64::MAX - 3_000_000_000)) as u64
            + 3_000_000_000;
        let first = self
            .core
            .shutdown_nanos
            .compare_exchange(0, nanos, Ordering::SeqCst, Ordering::SeqCst)
            .unwrap_or_else(|existing| existing);
        self.core.created + Duration::from_nanos(if first == 0 { nanos } else { first })
    }
    pub fn producer(&self, kind: ProducerKind) -> Result<Producer, SessionError> {
        self.register(kind, false)
    }
    fn register(&self, kind: ProducerKind, settlement: bool) -> Result<Producer, SessionError> {
        let mut producers = self.core.producers.lock().map_err(|_| {
            self.fail();
            SessionError::Restricted
        })?;
        let word = self.core.cell.session_word();
        if word & SEALED != 0 {
            return Err(SessionError::SessionEnded);
        }
        if !settlement
            && kind != ProducerKind::StopWorker
            && (word & CLOSING != 0 || word & FAILED != 0 && kind != ProducerKind::Observation)
        {
            return Err(SessionError::Restricted);
        }
        let index = producers
            .slots
            .iter()
            .position(|s| !s.live && !s.lost)
            .ok_or(SessionError::Exhausted)?;
        let generation = producers.next.checked_add(1).ok_or_else(|| {
            self.fail();
            SessionError::Exhausted
        })?;
        self.core
            .cell
            .session_compare(word, word)
            .map_err(|_| SessionError::Restricted)?;
        producers.next = generation;
        producers.slots[index] = ProducerSlot {
            generation,
            live: true,
            lost: false,
        };
        Ok(Producer {
            core: self.core.clone(),
            index,
            generation,
            joined: false,
        })
    }
}

/// Existing-obligation calls participate in seal ordering without borrowing a
/// work lifetime. Their returned batches remain independently counted natively.
pub(super) fn settlement(core: &Arc<Core>) -> Result<Producer, SessionFailure> {
    SessionIngress { core: core.clone() }
        .register(ProducerKind::Work, true)
        .map_err(|_| crate::authority::AuthorityError::Closed("the session has ended").into())
}

const PENDING: u8 = 0;
const UNKNOWN: u8 = 1;
const RECORDED: u8 = 2;
const FAILED_END: u8 = 3;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FinalStatus {
    Pending,
    Unknown,
    Recorded,
    Failed,
}
fn final_status(value: u8) -> FinalStatus {
    match value {
        PENDING => FinalStatus::Pending,
        RECORDED => FinalStatus::Recorded,
        FAILED_END => FinalStatus::Failed,
        _ => FinalStatus::Unknown,
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CloseRecords {
    pub mother: FinalStatus,
    pub primary: FinalStatus,
    pub delivery: FinalStatus,
}
pub(super) struct Finals {
    mother: AtomicU8,
    primary: AtomicU8,
    delivery: AtomicU8,
    history: Mutex<(usize, String)>,
}
pub struct CloseSeal {
    core: Arc<Core>,
    deadline: Instant,
    finished: bool,
}
impl CloseSeal {
    pub fn begin_mother_end(&self, session: SessionIdentity) -> Result<(), SessionError> {
        self.live()?;
        if self.core.identity != session {
            return Err(SessionError::Restricted);
        }
        self.finals()?
            .mother
            .compare_exchange(PENDING, UNKNOWN, Ordering::SeqCst, Ordering::SeqCst)
            .map(|_| ())
            .map_err(|_| SessionError::Restricted)
    }
    fn finals(&self) -> Result<&Finals, SessionError> {
        self.core.finals.get().ok_or(SessionError::Restricted)
    }
    fn live(&self) -> Result<(), SessionError> {
        if Instant::now() >= self.deadline
            || self.core.cell.session_word() & (SEALED | READY | FAILED) != SEALED
        {
            Err(SessionError::Restricted)
        } else {
            Ok(())
        }
    }
    /// Called only after the final frame was acknowledged, all terminal
    /// executors joined, and terminal modes restored before this same deadline.
    pub fn finish_delivery(mut self) -> bool {
        let success = self.live().is_ok()
            && self.finals().is_ok_and(|f| {
                f.mother.load(Ordering::SeqCst) == RECORDED
                    && f.primary.load(Ordering::SeqCst) == RECORDED
            });
        if let Ok(f) = self.finals() {
            f.delivery.store(
                if success { RECORDED } else { FAILED_END },
                Ordering::SeqCst,
            );
        }
        finish(&self.core, success);
        self.finished = true;
        success
    }
}
impl Drop for CloseSeal {
    fn drop(&mut self) {
        if !self.finished {
            finish(&self.core, false);
        }
    }
}
fn finish(core: &Core, success: bool) {
    loop {
        let word = core.cell.session_word();
        let next = word | SEALED | READY | if success { 0 } else { FAILED };
        if core.cell.session_compare(word, next).is_ok() {
            break;
        }
    }
}

impl HostSession {
    /// Observations only, collected on the bounded close worker. Failure to
    /// acquire a consistent snapshot is unknown, never zero obligations.
    pub fn close_obligations(&self) -> Result<CloseObligations, SessionError> {
        let actions_unacknowledged = self
            .core
            .actions
            .try_lock()
            .map_err(|_| SessionError::Unavailable)?
            .iter()
            .filter(|a| !a.acknowledged())
            .count();
        let (producers_live, producers_lost) = {
            let producers = self
                .core
                .producers
                .try_lock()
                .map_err(|_| SessionError::Unavailable)?;
            (
                producers.slots.iter().filter(|p| p.live).count(),
                producers.slots.iter().filter(|p| p.lost).count(),
            )
        };
        Ok(CloseObligations {
            model: self
                .core
                .model
                .snapshot()
                .ok_or(SessionError::Unavailable)?,
            actions_unacknowledged,
            producers_live,
            producers_lost,
            authority_closed: self.core.authority_closed.load(Ordering::SeqCst),
            halt: self.control.authority().halt_records(),
        })
    }
    pub fn close_records(&self) -> CloseRecords {
        match self.core.finals.get() {
            Some(f) => CloseRecords {
                mother: final_status(f.mother.load(Ordering::SeqCst)),
                primary: final_status(f.primary.load(Ordering::SeqCst)),
                delivery: final_status(f.delivery.load(Ordering::SeqCst)),
            },
            None => CloseRecords {
                mother: FinalStatus::Pending,
                primary: FinalStatus::Pending,
                delivery: FinalStatus::Pending,
            },
        }
    }
    fn close_request(&self, request: &SessionRequest) -> Result<(), SessionError> {
        if !Arc::ptr_eq(&self.core, &request.core)
            || request.kind != CommandKind::Quit
            || request.close != self.core.close.get().map(|close| close.sequence)
            || Instant::now() >= request.deadline
            || self.core.cell.session_word() & (CLOSING | SEALED) != CLOSING
        {
            return Err(SessionError::Restricted);
        }
        Ok(())
    }
    pub(super) fn close_operation(
        &self,
        request: &SessionRequest,
    ) -> Result<Producer, SessionError> {
        // Registration and seal share the producer hold. Revalidate the exact
        // close lifetime after registration, before any native call.
        self.close_request(request)?;
        let operation = self.ingress().register(ProducerKind::Work, true)?;
        if let Err(error) = self.close_request(request) {
            operation.joined();
            return Err(error);
        }
        Ok(operation)
    }
    pub fn settle_close_authority(&self, request: &SessionRequest) -> Result<(), SessionError> {
        let operation = self.close_operation(request)?;
        #[cfg(test)]
        crate::authority::file_session::probe(Some(&self.core.cell), "close.authority.native");
        let result = self.control.close_session_authority();
        let result = result.map_err(|e| {
            self.ingress().fail();
            SessionError::Backend(e.class())
        });
        if result.is_ok() {
            self.core.authority_closed.store(true, Ordering::SeqCst);
        }
        operation.joined();
        result
    }
    pub fn seal_close(&self, request: &SessionRequest) -> Result<CloseSeal, SessionError> {
        self.close_request(request)?;
        if !self
            .report
            .get()
            .is_some_and(recovery::StartupReport::ready)
        {
            return Err(SessionError::Unavailable);
        }
        let producers = self
            .core
            .producers
            .lock()
            .map_err(|_| SessionError::Restricted)?;
        let word = self.core.cell.session_word();
        if word & (FAILED | SEALED) != 0 || !self.core.authority_closed.load(Ordering::SeqCst) {
            return Err(SessionError::Restricted);
        }
        if producers.slots.iter().any(|s| s.live || s.lost) {
            return Err(SessionError::Unavailable);
        }
        if !self.core.model.quiescent() {
            return Err(SessionError::Unavailable);
        }
        let stop = self.stop_obligations()?;
        if stop.received != stop.recorded
            || stop.received as u64 != word / EPOCH_STEP
            || stop.overflowed
            || !self.core.stops.quiescent()
        {
            return Err(SessionError::Unavailable);
        }
        let resources = self.resource_obligations()?;
        if resources.unresolved != 0 || resources.uncertain != 0 {
            return Err(SessionError::Unavailable);
        }
        if self
            .inventory()
            .map_err(|_| SessionError::Unavailable)?
            .entries
            .iter()
            .any(|e| !e.discharged || e.in_flight || e.uncertain)
        {
            return Err(SessionError::Unavailable);
        }
        if self
            .core
            .actions
            .lock()
            .map_err(|_| SessionError::Restricted)?
            .iter()
            .any(|a| !a.acknowledged())
        {
            return Err(SessionError::Unavailable);
        }
        let halt = self.control.authority().halt_records();
        if halt.pending != 0 || halt.owed != 0 || halt.uncertain != 0 || halt.lost != 0 {
            return Err(SessionError::Unavailable);
        }
        let health = self.evidence.health();
        if health.latched
            || health.owed != 0
            || health.recovery_required
            || health.unsettled != 0
            || health.unclean_sessions != 0
        {
            return Err(SessionError::Unavailable);
        }
        let history = self.core.history.get().ok_or(SessionError::Unavailable)?;
        let report = recovery::history(history).map_err(|_| SessionError::Unavailable)?;
        if report.records == 0 || report.unresolved_missions != 0 || report.session_end_recorded {
            return Err(SessionError::Unavailable);
        }
        let (_, lines) = history
            .report_lines()
            .map_err(|_| SessionError::Unavailable)?;
        let head = lines.last().ok_or(SessionError::Unavailable)?;
        // All three final obligations are reserved before the one seal CAS.
        // No producer can register while this hold is owned.
        let finals = self.core.finals.get_or_init(|| Finals {
            mother: AtomicU8::new(PENDING),
            primary: AtomicU8::new(PENDING),
            delivery: AtomicU8::new(PENDING),
            history: Mutex::new((0, String::new())),
        });
        *finals
            .history
            .lock()
            .map_err(|_| SessionError::Restricted)? = (lines.len(), head.hash.clone());
        if Instant::now() >= request.deadline
            || self
                .core
                .cell
                .session_compare(word, (word | SEALED) & !READY)
                .is_err()
        {
            return Err(SessionError::Restricted);
        }
        drop(producers);
        Ok(CloseSeal {
            core: self.core.clone(),
            deadline: request.deadline,
            finished: false,
        })
    }
    pub fn observe_mother_end(&self, seal: &CloseSeal) -> Result<(), SessionError> {
        seal.live()?;
        if !Arc::ptr_eq(&self.core, &seal.core) {
            return Err(SessionError::Restricted);
        }
        let finals = seal.finals()?;
        if finals.mother.load(Ordering::SeqCst) != UNKNOWN {
            return Err(SessionError::Restricted);
        }
        let (records, head) = finals
            .history
            .lock()
            .map_err(|_| SessionError::Restricted)?
            .clone();
        self.core
            .history
            .get()
            .ok_or(SessionError::Unavailable)?
            .verify_end(records, &head)
            .map_err(|_| SessionError::Unavailable)?;
        finals.mother.store(RECORDED, Ordering::SeqCst);
        Ok(())
    }
    pub fn close_primary(&self, seal: &CloseSeal) -> Result<(), SessionError> {
        seal.live()?;
        if !Arc::ptr_eq(&self.core, &seal.core) {
            return Err(SessionError::Restricted);
        }
        let finals = seal.finals()?;
        if finals.mother.load(Ordering::SeqCst) != RECORDED {
            return Err(SessionError::Restricted);
        }
        finals
            .primary
            .compare_exchange(PENDING, UNKNOWN, Ordering::SeqCst, Ordering::SeqCst)
            .map_err(|_| SessionError::Restricted)?;
        use crate::authority::evidence::EndStatus;
        match self.evidence.close_session() {
            EndStatus::Recorded { .. } => {
                finals.primary.store(RECORDED, Ordering::SeqCst);
                Ok(())
            }
            EndStatus::Owed => {
                finals.primary.store(FAILED_END, Ordering::SeqCst);
                Err(SessionError::Unavailable)
            }
            EndStatus::Uncertain => Err(SessionError::Unavailable),
        }
    }
}

/// Current-process close debt; no operational handles.
#[derive(Clone, Copy, Debug)]
pub struct CloseObligations {
    pub model: ModelObligations,
    pub actions_unacknowledged: usize,
    pub producers_live: usize,
    pub producers_lost: usize,
    pub authority_closed: bool,
    pub halt: crate::authority::halt::HaltRecords,
}

/// Content-free current-process registration obligations; never authority.
#[derive(Clone, Copy, Debug)]
pub struct ModelObligations {
    pub operation: &'static str,
    pub active: bool,
    pub owner_complete: bool,
    pub child_complete: bool,
    pub record: &'static str,
    pub uncertain: bool,
}
