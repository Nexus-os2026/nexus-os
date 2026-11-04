//! The Hands session's original identity and lock-free input boundary.
//!
//! A seed is issued before input or Home opening. It carries no raw broker,
//! door or kernel object. Acceptance binds a request; it does not admit work.
use crate::authority::file_session::HaltCell;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

pub(crate) const CLOSING: u64 = 0x08;
pub(crate) const FAILED: u64 = 0x10;
pub(crate) const SEALED: u64 = 0x20;
pub(crate) const READY: u64 = 0x40;
pub(crate) const EMERGENCY: u64 = 0x80;
pub(crate) const EPOCH_STEP: u64 = 1 << 24;
pub(crate) const EPOCH_MASK: u64 = !(EPOCH_STEP - 1);

/// Opaque process-local identity, never reconstructed from persistent data.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct SessionIdentity([u8; 16]);
impl fmt::Debug for SessionIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SessionIdentity(opaque)")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionState {
    Bootstrap,
    Normal,
    Emergency,
    Failed,
    ClosingNormal,
    ClosingRestricted,
    SealedFinalizing,
    ClosedClean,
    ClosedUnclean,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionError {
    Unavailable,
    Restricted,
    SessionEnded,
    NoTarget,
    Sequence,
    Exhausted,
}
impl fmt::Display for SessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "session refused: {self:?}")
    }
}
impl std::error::Error for SessionError {}

/// Closed command vocabulary; unsupported routes cannot become generic intent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandKind {
    Run,
    Approve,
    Undo,
    Cleanup,
    Detail,
    Status,
    Evidence,
    Quit,
    Lift,
}

struct CloseIdentity {
    sequence: u64,
    generation: u64,
    deadline: Instant,
}

/// All authority restrictions and the original first cause reside in cell's
/// single atomic word. The other atomics publish non-authoritative indices.
struct Core {
    identity: SessionIdentity,
    cell: HaltCell,
    sequence: AtomicU64,
    pending_context: AtomicU64,
    undo_context: AtomicU64,
    close: OnceLock<CloseIdentity>,
}

/// One bootstrap owner. Its ingress may be cloned, but its identity cannot be
/// adopted by another session or replaced after a Stop.
pub struct SessionSeed {
    core: Arc<Core>,
}

/// Input producer's memory-only interface. It never acquires broker,
/// coordinator, resource or output locks and never performs recording.
#[derive(Clone)]
pub struct SessionIngress {
    core: Arc<Core>,
}

/// Immutable command envelope. A child must carry this same snapshot; a fresh
/// code, dequeue or callback does not create a fresh envelope.
pub struct SessionRequest {
    core: Arc<Core>,
    kind: CommandKind,
    sequence: u64,
    snapshot: u64,
    target: u64,
    deadline: Instant,
    close: Option<u64>,
}
impl SessionRequest {
    pub fn session(&self) -> SessionIdentity {
        self.core.identity
    }
    pub fn kind(&self) -> CommandKind {
        self.kind
    }
    pub fn sequence(&self) -> u64 {
        self.sequence
    }
    pub fn generation(&self) -> u64 {
        self.snapshot / EPOCH_STEP
    }
    pub fn context(&self) -> Option<u64> {
        (self.target != 0).then_some(self.target)
    }
    pub fn deadline(&self) -> Instant {
        self.deadline
    }
    /// This is a correlation identity, not a cleanup capability.
    pub fn close_identity(&self) -> Option<u64> {
        self.close
    }
    pub fn close_generation(&self) -> Option<u64> {
        self.close
            .and_then(|_| self.core.close.get().map(|close| close.generation))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IngressEvent {
    generation: u64,
}
impl IngressEvent {
    pub fn generation(&self) -> u64 {
        self.generation
    }
}

impl SessionSeed {
    pub fn new() -> Result<Self, SessionError> {
        let mut identity = [0; 16];
        getrandom::getrandom(&mut identity).map_err(|_| SessionError::Unavailable)?;
        Ok(Self {
            core: Arc::new(Core {
                identity: SessionIdentity(identity),
                cell: HaltCell::new_host(),
                sequence: AtomicU64::new(0),
                pending_context: AtomicU64::new(0),
                undo_context: AtomicU64::new(0),
                close: OnceLock::new(),
            }),
        })
    }
    pub fn ingress(&self) -> SessionIngress {
        SessionIngress {
            core: self.core.clone(),
        }
    }
    pub fn identity(&self) -> SessionIdentity {
        self.core.identity
    }
    pub fn state(&self) -> SessionState {
        state(self.core.cell.session_word())
    }
    pub fn failed(&self) -> bool {
        self.core.cell.session_word() & FAILED != 0
    }
    pub fn emergency_seen(&self) -> bool {
        self.core.cell.session_word() & EMERGENCY != 0
    }
    pub fn close_deadline(&self) -> Option<Instant> {
        self.core.close.get().map(|close| close.deadline)
    }
}

fn state(word: u64) -> SessionState {
    if word & SEALED != 0 {
        return if word & READY == 0 {
            SessionState::SealedFinalizing
        } else if word & FAILED != 0 {
            SessionState::ClosedUnclean
        } else {
            SessionState::ClosedClean
        };
    }
    if word & CLOSING != 0 {
        return if word & (EMERGENCY | FAILED) == 0 {
            SessionState::ClosingNormal
        } else {
            SessionState::ClosingRestricted
        };
    }
    if word & FAILED != 0 {
        SessionState::Failed
    } else if word & EMERGENCY != 0 {
        SessionState::Emergency
    } else if word & READY != 0 {
        SessionState::Normal
    } else {
        SessionState::Bootstrap
    }
}

impl SessionIngress {
    pub fn emergency(&self) -> Result<IngressEvent, SessionError> {
        loop {
            let old = self.core.cell.session_word();
            if old & SEALED != 0 {
                return Err(SessionError::SessionEnded);
            }
            if old & EPOCH_MASK == EPOCH_MASK {
                self.fail();
                return Err(SessionError::Exhausted);
            }
            // The original immutable emergency cause/tag shares this CAS.
            let cause = if old & 7 == 0 {
                2 | (u64::from(u16::MAX - 1) << 8)
            } else {
                0
            };
            let next = (old + EPOCH_STEP) | EMERGENCY | cause;
            if self.core.cell.session_compare(old, next).is_ok() {
                return Ok(IngressEvent {
                    generation: next / EPOCH_STEP,
                });
            }
        }
    }

    pub fn fail(&self) {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let old = self.core.cell.session_word();
            if old & SEALED != 0 || old & FAILED != 0 {
                return;
            }
            if self.core.cell.session_compare(old, old | FAILED).is_ok() {
                self.core.close.get_or_init(|| CloseIdentity {
                    sequence: self.core.sequence.load(Ordering::SeqCst),
                    generation: old / EPOCH_STEP,
                    deadline,
                });
                return;
            }
        }
    }

    pub fn accept(&self, kind: CommandKind, sequence: u64) -> Result<SessionRequest, SessionError> {
        let accepted = Instant::now();
        let snapshot = self.core.cell.session_word();
        if snapshot & SEALED != 0 {
            return Err(SessionError::SessionEnded);
        }
        if sequence == u64::MAX {
            self.fail();
            return Err(SessionError::Exhausted);
        }
        self.core
            .sequence
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |old| {
                (sequence > old).then_some(sequence)
            })
            .map_err(|_| SessionError::Sequence)?;
        if kind == CommandKind::Lift {
            return Err(SessionError::Unavailable);
        }
        let target = match kind {
            CommandKind::Approve => self.core.pending_context.load(Ordering::SeqCst),
            CommandKind::Undo => self.core.undo_context.load(Ordering::SeqCst),
            _ => 0,
        };
        if kind == CommandKind::Quit {
            // Publish the first shutdown identity exactly once. Input has one
            // producer; concurrent fatal shutdown can only narrow this close.
            let close = self.core.close.get_or_init(|| CloseIdentity {
                sequence,
                generation: snapshot / EPOCH_STEP,
                deadline: accepted + Duration::from_secs(3),
            });
            let mut current = snapshot;
            loop {
                if current & SEALED != 0 {
                    return Err(SessionError::SessionEnded);
                }
                if current & CLOSING != 0 {
                    break;
                }
                match self.core.cell.session_compare(current, current | CLOSING) {
                    Ok(()) => break,
                    Err(now) => current = now,
                }
            }
            // Close generation is preserved; repeated Quit cannot refresh it.
            return Ok(SessionRequest {
                core: self.core.clone(),
                kind,
                sequence,
                snapshot,
                target: 0,
                deadline: close.deadline,
                close: Some(close.sequence),
            });
        }
        if !matches!(kind, CommandKind::Status | CommandKind::Evidence)
            && snapshot & (CLOSING | FAILED | EMERGENCY) != 0
        {
            return Err(SessionError::Restricted);
        }
        if matches!(kind, CommandKind::Approve | CommandKind::Undo) && target == 0 {
            return Err(SessionError::NoTarget);
        }
        let seconds = if matches!(kind, CommandKind::Run | CommandKind::Approve) {
            300
        } else {
            120
        };
        Ok(SessionRequest {
            core: self.core.clone(),
            kind,
            sequence,
            snapshot,
            target,
            deadline: accepted + Duration::from_secs(seconds),
            close: None,
        })
    }
}
