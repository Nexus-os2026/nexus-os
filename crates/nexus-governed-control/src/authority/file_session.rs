//! M2d-C-lite: one original File session for the broker's lifetime.
//!
//! Handles are issued by the broker, never reconstructed from a journal or
//! caller identity. Dropping a handle releases no eligibility. The shared
//! atomic stores emergency history and the first halt cause in ONE update:
//! ingress cannot block, and resume cannot clear either. The history mutex
//! contains counters and handles only; no I/O, callback or nested lock.
use super::run::GroupId;
use std::num::NonZeroU16;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

const EMERGENCY: u64 = 0x80;
const CAUSE: u64 = 0x07;
// Low 24 bits keep the immutable cause/tag and emergency history. The high
// 40 bits count EVERY ingress, including repeated engagements. One atomic
// publishes both; saturation permanently refuses restorative requests.
const EPOCH_STEP: u64 = 1 << 24;
const EPOCH_MASK: u64 = !(EPOCH_STEP - 1);

/// First reason this File session stopped. A cell grants no authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HaltCause {
    Stop,
    Emergency,
    TimeBudget,
    Fence,
}
impl HaltCause {
    fn code(self) -> u64 {
        match self {
            Self::Stop => 1,
            Self::Emergency => 2,
            Self::TimeBudget => 3,
            Self::Fence => 4,
        }
    }
    fn of(code: u64) -> Option<Self> {
        match code & CAUSE {
            1 => Some(Self::Stop),
            2 => Some(Self::Emergency),
            3 => Some(Self::TimeBudget),
            4 => Some(Self::Fence),
            _ => None,
        }
    }
}

/// Exact built-in disposition or a client's immutable, process-local intent.
/// The four high tags are reserved for broker dispositions. Client tags below
/// that range identify deny-only data; they never authorize execution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HaltTag {
    OwnerStop,
    EmergencyStop,
    TimeBudget,
    DoorHalted,
    Client {
        cause: HaltCause,
        reason: NonZeroU16,
    },
}
impl HaltTag {
    fn builtin(cause: HaltCause) -> Self {
        match cause {
            HaltCause::Stop => Self::OwnerStop,
            HaltCause::Emergency => Self::EmergencyStop,
            HaltCause::TimeBudget => Self::TimeBudget,
            HaltCause::Fence => Self::DoorHalted,
        }
    }
    fn bits(self) -> u64 {
        let (cause, tag) = match self {
            Self::OwnerStop => (HaltCause::Stop, u16::MAX),
            Self::EmergencyStop => (HaltCause::Emergency, u16::MAX - 1),
            Self::TimeBudget => (HaltCause::TimeBudget, u16::MAX - 2),
            Self::DoorHalted => (HaltCause::Fence, u16::MAX - 3),
            Self::Client { cause, reason } => (cause, reason.get()),
        };
        cause.code() | (u64::from(tag) << 8)
    }
}

/// Deny-only, immutable identity, never reset or retired before broker drop.
#[derive(Clone)]
#[cfg_attr(not(test), derive(Debug))]
pub struct HaltCell(
    Arc<AtomicU64>,
    #[cfg(test)] pub(crate) Arc<crate::test_hooks::Hooks>,
    bool,
);
pub(crate) fn check_halt(cell: Option<&HaltCell>) -> Result<(), super::AuthorityError> {
    if cell.is_some_and(HaltCell::is_engaged) {
        Err(super::AuthorityError::Halted)
    } else {
        Ok(())
    }
}

#[cfg(test)]
impl std::fmt::Debug for HaltCell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
impl HaltCell {
    fn host(&self) -> bool {
        #[cfg(test)]
        {
            self.2
        }
        #[cfg(not(test))]
        {
            self.1
        }
    }

    pub(crate) fn new_host() -> Self {
        Self(
            Arc::new(AtomicU64::new(0)),
            #[cfg(test)]
            Arc::default(),
            true,
        )
    }
    pub(crate) fn session_word(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
    pub(crate) fn session_compare(&self, old: u64, new: u64) -> Result<(), u64> {
        #[cfg(test)]
        self.1.fire("session.before_compare");
        self.0
            .compare_exchange(old, new, Ordering::SeqCst, Ordering::SeqCst)
            .map(|_| ())
    }
    /// Compatibility shorthand with an exact built-in Owner disposition.
    pub fn engage(&self) {
        self.engage_tagged(HaltTag::OwnerStop);
    }
    /// Compatibility shorthand; every built-in cause has an explicit tag.
    pub fn engage_with(&self, cause: HaltCause) {
        self.engage_tagged(HaltTag::builtin(cause));
    }
    pub fn engage_reason(&self, cause: HaltCause, reason: Option<NonZeroU16>) {
        self.engage_tagged(match reason {
            Some(reason) => HaltTag::Client { cause, reason },
            None => HaltTag::builtin(cause),
        });
    }
    /// Cause and exact intent become visible in the same atomic write. There
    /// is no intermediate engaged state with a missing disposition.
    pub fn engage_tagged(&self, intent: HaltTag) {
        self.latch(intent, false);
    }
    /// Owner ingress: engage this original deny-only cell and its monotone
    /// emergency history in one atomic update, without locks or I/O.
    pub fn engage_emergency(&self) {
        self.latch(HaltTag::EmergencyStop, true);
    }
    fn latch(&self, intent: HaltTag, emergency: bool) {
        let bits = intent.bits();
        let history = if emergency { EMERGENCY } else { 0 };
        let _ = self
            .0
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |old| {
                if self.host() && old & crate::host_session::SEALED != 0 {
                    return None;
                }
                // Host terminal settlement preserves ingress generation. A
                // mission's cause remains distinct from a new input event.
                let epoch = if self.host() && !emergency {
                    old & EPOCH_MASK
                } else {
                    (old & EPOCH_MASK).saturating_add(EPOCH_STEP) & EPOCH_MASK
                };
                Some(
                    epoch | (old & !EPOCH_MASK) | history | if old & CAUSE == 0 { bits } else { 0 },
                )
            });
        #[cfg(test)]
        self.1.fire("file.engaged");
    }
    /// Monotone, memory-only ingress observation; it grants no authority.
    pub fn ingress_epoch(&self) -> u64 {
        self.0.load(Ordering::SeqCst) / EPOCH_STEP
    }
    pub fn is_engaged(&self) -> bool {
        let word = self.0.load(Ordering::SeqCst);
        HaltCause::of(word).is_some()
            || (self.host()
                && word
                    & (crate::host_session::CLOSING
                        | crate::host_session::FAILED
                        | crate::host_session::SEALED
                        | EMERGENCY)
                    != 0)
    }
    pub fn cause(&self) -> Option<HaltCause> {
        self.reason().map(|(cause, _)| cause)
    }
    /// Both fields come from the same atomic snapshot and never change after
    /// engagement. Emergency ingress may only add its separate history bit.
    pub fn reason(&self) -> Option<(HaltCause, Option<NonZeroU16>)> {
        let value = self.0.load(Ordering::SeqCst);
        HaltCause::of(value).map(|cause| (cause, NonZeroU16::new((value >> 8) as u16)))
    }
    pub(crate) fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

/// Request-local deny-only lifetime for Owner recovery. A permanently engaged
/// session may confirm a NEW undo/cleanup/lift, but no request survives another
/// ingress. The original cell and its complete atomic snapshot are retained.
pub(crate) struct RecoveryCheck {
    cell: Option<HaltCell>,
    snapshot: u64,
}
impl RecoveryCheck {
    pub(crate) fn take(cell: Option<&HaltCell>) -> Result<Self, super::AuthorityError> {
        let check = Self {
            cell: cell.cloned(),
            snapshot: cell.map_or(0, |c| c.0.load(Ordering::SeqCst)),
        };
        check.check()?;
        Ok(check)
    }
    pub(crate) fn check(&self) -> Result<(), super::AuthorityError> {
        if self.cell.as_ref().is_some_and(|cell| {
            self.snapshot & EPOCH_MASK == EPOCH_MASK
                || cell.0.load(Ordering::SeqCst) != self.snapshot
        }) {
            Err(super::AuthorityError::Halted)
        } else {
            Ok(())
        }
    }
}

/// File lifecycle refusal; distinct from the legacy broker's error vocabulary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileSessionError {
    Reopened,
    EmergencyHistory,
    AlreadySelected,
    WrongOwner,
    Poisoned,
    Capacity,
}
impl std::fmt::Display for FileSessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "File session refused: {self:?}")
    }
}
impl std::error::Error for FileSessionError {}

#[derive(Default)]
struct History {
    original: Option<GroupId>,
    ports: u64,
    attachments: u64,
    selected: bool,
}

pub(crate) struct FileSessions {
    state: HaltCell,
    history: Mutex<History>,
}
impl Default for FileSessions {
    fn default() -> Self {
        Self {
            state: HaltCell(
                Arc::new(AtomicU64::new(0)),
                #[cfg(test)]
                Arc::default(),
                false,
            ),
            history: Mutex::default(),
        }
    }
}
impl FileSessions {
    fn lock(&self) -> Result<MutexGuard<'_, History>, FileSessionError> {
        self.history.lock().map_err(|_| {
            self.state.engage_tagged(HaltTag::DoorHalted);
            FileSessionError::Poisoned
        })
    }
    /// First instruction at broker emergency ingress, before any lock or I/O.
    pub(crate) fn emergency(&self) {
        self.state.engage_emergency();
    }
    pub(crate) fn halt_cell(&self) -> HaltCell {
        self.state.clone()
    }
    fn fresh(&self) -> Result<(), FileSessionError> {
        if self.state.0.load(Ordering::SeqCst) & EMERGENCY != 0 {
            return Err(FileSessionError::EmergencyHistory);
        }
        if self.state.is_engaged() {
            return Err(FileSessionError::Reopened);
        }
        Ok(())
    }
    pub(crate) fn port(self: &Arc<Self>, group: GroupId) -> Result<FilePort, FileSessionError> {
        let mut h = self.lock()?;
        if h.selected {
            self.state.engage_tagged(HaltTag::DoorHalted);
            return Err(FileSessionError::Reopened);
        }
        h.ports = h.ports.checked_add(1).ok_or(FileSessionError::Capacity)?;
        h.original.get_or_insert(group);
        Ok(FilePort {
            sessions: self.clone(),
            group,
            number: h.ports,
        })
    }
    pub(crate) fn claim(
        self: &Arc<Self>,
        attachment: &FileAttachment,
        group: GroupId,
    ) -> Result<HaltCell, FileSessionError> {
        let mut h = self.lock()?;
        self.validate(&h, attachment, group)?;
        if h.selected {
            return Err(FileSessionError::AlreadySelected);
        }
        self.fresh()?;
        h.selected = true;
        // A concurrent emergency engages the SAME atomic already held here;
        // even one immediately after this read cannot leave a live token.
        self.fresh()?;
        Ok(self.state.clone())
    }
    fn validate(
        &self,
        h: &History,
        a: &FileAttachment,
        group: GroupId,
    ) -> Result<(), FileSessionError> {
        if !std::ptr::eq(self, Arc::as_ptr(&a.sessions))
            || a.group != group
            || h.original != Some(group)
        {
            return Err(FileSessionError::WrongOwner);
        }
        if h.ports != 1 || a.port != 1 || h.attachments != 1 || a.number != 1 {
            return Err(FileSessionError::Reopened);
        }
        Ok(())
    }
    pub(crate) fn cell(
        &self,
        attachment: &FileAttachment,
        group: GroupId,
    ) -> Result<HaltCell, FileSessionError> {
        let h = self.lock()?;
        self.validate(&h, attachment, group)?;
        if !h.selected {
            return Err(FileSessionError::WrongOwner);
        }
        self.fresh()?;
        Ok(self.state.clone())
    }
}

/// Broker-issued original-port handle. It has no execution authority.
pub struct FilePort {
    sessions: Arc<FileSessions>,
    group: GroupId,
    number: u64,
}
impl FilePort {
    /// Register EVERY coordinator before replay/adoption. Before selection,
    /// legacy reopening remains possible but permanently loses File eligibility.
    pub fn attach(&self) -> Result<FileAttachment, FileSessionError> {
        let mut h = self.sessions.lock()?;
        if h.selected {
            self.sessions.state.engage_tagged(HaltTag::DoorHalted);
            return Err(FileSessionError::Reopened);
        }
        h.attachments = h
            .attachments
            .checked_add(1)
            .ok_or(FileSessionError::Capacity)?;
        Ok(FileAttachment {
            sessions: self.sessions.clone(),
            group: self.group,
            port: self.number,
            number: h.attachments,
        })
    }
}

/// One coordinator attachment, retained by that coordinator, never transferable
/// to a different door and never reconstructible from caller material.
pub struct FileAttachment {
    sessions: Arc<FileSessions>,
    group: GroupId,
    port: u64,
    number: u64,
}

#[cfg(test)]
impl FileSessions {
    pub(crate) fn review5_cell(&self) -> HaltCell {
        self.state.clone()
    }
    pub(crate) fn review5_poison(&self) {
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _held = self.history.lock().unwrap();
            panic!("File history poison");
        }));
    }
}
