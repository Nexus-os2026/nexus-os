//! The mission journal: append-only and hash-chained.
//!
//! Every transition and every lane invocation and effect is an [`Entry`].
//! Each entry's hash covers the previous entry's hash, its sequence number,
//! its mission and its event, so an edited, reordered, inserted or removed
//! entry breaks the chain and the journal is refused. A coordinator is
//! rebuilt from its journal alone; a journal that does not verify, or
//! whose triggers the state machine does not accept, opens nothing.
//!
//! P0 (privacy closure): an entry keeps only evidence (ids, digests, sizes,
//! counts, kinds, roles, states, times and the closed codes of
//! [`crate::evidence`]). The contract and a paused phase's rest are kept in
//! the private store ([`crate::private`]) and named here by reference
//! ([`ContractRef`], [`RestRef`]). [`crate::evidence::check`] refuses an
//! entry whose digest or id field is not one, before it is appended and when
//! it is replayed. Journals are format 2; a v1 journal is refused
//! ([`JournalError::Legacy`]) and left as it is.
//!
//! Non-claim: the chain is unkeyed. It detects edits that do not rewrite
//! every later hash, and a torn last line, but not someone with write access
//! to Mother's home who rewrites the whole chain or cuts entries off its
//! end. Anchoring the head in the broker's evidence store is a later
//! mission.

use crate::contract::{sha256, ContractHash, EffectKind};
use crate::effect::PendingAction;
use crate::evidence::{
    ConsentRefusedReason, HeldReason, MotherRefusal, SettleCause, SettledOutcome,
};
use crate::lane::{InvocationId, LaneRole};
use crate::private::ContentId;
use crate::state::Trigger;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::{Arc, Mutex};

const ENTRY_DOMAIN: &[u8] = b"nexus-mother-journal-v2\n";
/// The journal format every entry carries. A journal without it (v1) is
/// refused.
pub const FORMAT: u8 = 2;
/// The `prev` of the first entry.
pub const GENESIS: &str = "0000000000000000000000000000000000000000000000000000000000000000";

/// The name session records carry: no mission is ever drafted with it.
pub const SESSION: MissionId = MissionId(0);

/// A mission's identity: a name, never authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct MissionId(pub u64);

impl fmt::Display for MissionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "msn-{:06}", self.0)
    }
}

/// A contract, by reference (design §3 I3): its hash, the private store's
/// content id of its canonical bytes, and their size. The contract itself
/// is in the private store.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractRef {
    pub contract_hash: ContractHash,
    pub content: ContentId,
    pub bytes: u64,
}

/// A paused phase's rest (the asking lane's remaining effect requests), by
/// reference (I3): the private store's content id, how many requests, and
/// the size of their canonical bytes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RestRef {
    pub content: ContentId,
    pub count: u64,
    pub bytes: u64,
}

/// What happened.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case", deny_unknown_fields)]
pub enum Event {
    /// A mission was drafted with this contract (by reference).
    Drafted { contract: ContractRef },
    /// A state transition.
    Transition { trigger: Trigger },
    /// A consent answer came for a contract or state that had moved on.
    ConsentRefused { reason: ConsentRefusedReason },
    /// A lane call: its role, the digest of its identity (name and
    /// version: `evidence::lane_identity_digest`) and of its input.
    LaneInvoked {
        invocation: InvocationId,
        role: LaneRole,
        identity_sha256: String,
        input_hash: String,
    },
    LaneReturned {
        invocation: InvocationId,
        output_hash: String,
    },
    /// A lane's answer came after the mission had moved on (a Stop won).
    LaneOvertaken { invocation: InvocationId },
    /// Mother refused a lane's effect before it reached the port.
    EffectRefused {
        kind: EffectKind,
        reason: MotherRefusal,
    },
    /// About to hand an effect to the port.
    EffectDispatched {
        effect: u64,
        kind: EffectKind,
        request_hash: String,
    },
    /// What the port said: `done`, `awaiting_approval` or a refusal class.
    EffectSettled {
        effect: u64,
        outcome: SettledOutcome,
    },
    /// M2a: the broker's identity of the action effect `effect` waits on,
    /// journaled before the wait so that a rebuilt port can withdraw it.
    ApprovalPending { effect: u64, pending: PendingAction },
    /// M2a: the port confirmed that the broker no longer holds that action
    /// open (final, or gone). Written only after the confirmation; until
    /// it is, every reopen withdraws the action again.
    ApprovalClosed { effect: u64 },
    /// M2a-S (session record, [`SESSION`]): a coordinator opened this
    /// journal. Written on every open, before anything else.
    SessionOpened,
    /// M2a-S (session record): the coordinator shut down in order, not
    /// broken. Nothing follows it but the next `SessionOpened`.
    SessionClosed,
    /// M2a-S: the session before this one did not close cleanly, so a
    /// transition (a Stop, a denial) may be missing: the mission does
    /// nothing until the Owner resumes or stops it.
    Held { reason: HeldReason },
    /// M2a-S: the Owner resumed a held mission.
    Resumed,
    /// M2d-C round 4 (settlement design v2, rule 4): a Stop, the time
    /// budget or another end began settling the mission's File `effect`:
    /// from this record on the mission is never runnable again, and `cause`
    /// is its first cause. Only the settlement's terminal line follows.
    SettlementStarted { effect: u64, cause: SettleCause },
}

/// One journal entry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    /// [`FORMAT`].
    pub format: u8,
    pub seq: u64,
    pub prev: String,
    pub mission: MissionId,
    pub event: Event,
    pub hash: String,
}

/// An I/O error, by kind only (never a path or the system's text).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IoKind {
    NotFound,
    PermissionDenied,
    AlreadyExists,
    InvalidInput,
    InvalidData,
    UnexpectedEof,
    WriteZero,
    Interrupted,
    StorageFull,
    ReadOnlyFilesystem,
    Other,
}

impl IoKind {
    pub fn of(kind: std::io::ErrorKind) -> Self {
        use std::io::ErrorKind as K;
        match kind {
            K::NotFound => IoKind::NotFound,
            K::PermissionDenied => IoKind::PermissionDenied,
            K::AlreadyExists => IoKind::AlreadyExists,
            K::InvalidInput => IoKind::InvalidInput,
            K::InvalidData => IoKind::InvalidData,
            K::UnexpectedEof => IoKind::UnexpectedEof,
            K::WriteZero => IoKind::WriteZero,
            K::Interrupted => IoKind::Interrupted,
            K::StorageFull => IoKind::StorageFull,
            K::ReadOnlyFilesystem => IoKind::ReadOnlyFilesystem,
            _ => IoKind::Other,
        }
    }
}

/// Why a journal store failed: a closed code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StoreFailure {
    /// The file system refused.
    Io(IoKind),
    /// The journal is not a regular file (a link, a directory).
    NotRegularFile,
    /// A line to append holds a newline.
    LineHasNewline,
    /// An append failed earlier in this coordinator: it journals nothing
    /// more.
    FailedEarlier,
    /// An append failed and so did the poisoning: the next open holds
    /// every unfinished mission.
    PoisonFailed,
    /// An append failed during recovery.
    FailedDuringRecovery,
    /// The journal changed under this coordinator (another writer).
    ChangedUnderCoordinator,
    /// A store's simulated failure ([`MemoryJournal::fail_after`]).
    Injected,
}

/// Why replay refused a verified entry: a closed code (design §5.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplayViolation {
    /// A digest or id field is not one ([`crate::evidence::check`]).
    NotEvidence,
    SessionRecordNamesMission,
    CloseOfNoSession,
    CloseWithUnheldMission,
    RecordAfterClose,
    DraftedOutOfOrder,
    TooManyMissions,
    NeverDrafted,
    RecordWhileHeld,
    RecordBeforeHold,
    IllegalTransition,
    TransitionWhileUnsettled,
    SubmitOtherHash,
    ConsentOtherHash,
    WorkWithoutConsent,
    WaitNotAwaiting,
    WaitNotWritten,
    ApprovalOfOtherWait,
    EndOfOtherWait,
    LaneOutOfPlace,
    LaneAnswerWithoutCall,
    DispatchOutOfPlace,
    SettledNotDispatched,
    PendingOutOfPlace,
    ClosureOfNoAction,
    HoldOutOfPlace,
    ResumeNotHeld,
    /// M2d-C round 4: a settlement record of no File the mission waits on.
    SettlementOutOfPlace,
    /// M2d-C round 4: a second start, a terminal line of another File, an
    /// end its first cause does not give, an action other than the
    /// journaled one, or a transition other than the settlement's own.
    SettlementConflict,
}

/// Why a journal cannot be read or written: closed codes and line numbers
/// only.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JournalError {
    /// The store could not be read or written.
    Store(StoreFailure),
    /// A line is not an entry, or the last line is torn.
    Malformed { line: usize },
    /// A line is not of this format ([`FORMAT`]): a v1 journal. Refused,
    /// with no migration; the file is left as it is.
    Legacy { line: usize },
    /// An entry's hash, `prev` or sequence does not follow from the one
    /// before.
    Tampered { line: usize },
    /// The entries verify but describe something the state machine
    /// forbids, or an entry is not evidence (refused also at append, as
    /// the line it would have been).
    Inconsistent { line: usize, code: ReplayViolation },
    /// An append failed earlier: the journal may lack a transition (a
    /// Stop), so it opens nothing until the Owner reconciles it.
    Poisoned,
}

impl Entry {
    /// The entry that follows `prev` (the previous hash) at `seq`.
    pub fn chained(seq: u64, prev: &str, mission: MissionId, event: Event) -> Self {
        let mut entry = Self {
            format: FORMAT,
            seq,
            prev: prev.to_string(),
            mission,
            event,
            hash: String::new(),
        };
        entry.hash = entry.expected_hash();
        entry
    }

    /// The hash this entry must carry: over the previous hash, the
    /// format, the sequence number, the mission and the event.
    pub fn expected_hash(&self) -> String {
        let event = serde_json::to_vec(&self.event).expect("an event serialises");
        hex::encode(sha256(
            ENTRY_DOMAIN,
            &[
                self.prev.as_bytes(),
                &[self.format],
                &self.seq.to_be_bytes(),
                &self.mission.0.to_be_bytes(),
                &event,
            ],
        ))
    }

    pub fn to_line(&self) -> String {
        serde_json::to_string(self).expect("an entry serialises")
    }
}

/// Parse and verify a journal's lines: every entry of this [`FORMAT`] (a
/// line without it is [`JournalError::Legacy`]), well formed and in its
/// canonical form, numbered from 0, chained from [`GENESIS`] and hashed
/// correctly.
pub fn verify(lines: &[String]) -> Result<Vec<Entry>, JournalError> {
    let mut entries = Vec::with_capacity(lines.len());
    let mut prev = GENESIS.to_string();
    for (index, line) in lines.iter().enumerate() {
        let number = index + 1;
        let value: serde_json::Value =
            serde_json::from_str(line).map_err(|_| JournalError::Malformed { line: number })?;
        if value.get("format").and_then(serde_json::Value::as_u64) != Some(u64::from(FORMAT)) {
            return Err(JournalError::Legacy { line: number });
        }
        let entry: Entry =
            serde_json::from_str(line).map_err(|_| JournalError::Malformed { line: number })?;
        if entry.seq != index as u64
            || entry.prev != prev
            || entry.hash != entry.expected_hash()
            || entry.to_line() != *line
        {
            return Err(JournalError::Tampered { line: number });
        }
        prev = entry.hash.clone();
        entries.push(entry);
    }
    Ok(entries)
}

/// Where a journal lives: complete lines, appended only.
pub trait JournalStore: Send {
    /// Every line, in order. A torn last line is an error.
    fn load(&self) -> Result<Vec<String>, JournalError>;
    /// Append one line durably.
    fn append(&mut self, line: &str) -> Result<(), JournalError>;
    /// Mark the journal unreliable, durably: every later load is refused
    /// with [`JournalError::Poisoned`].
    fn poison(&mut self) -> Result<(), JournalError>;
    /// M2d-C round 5 (C1): an identity of this journal, stable across
    /// reopening it and distinct from every other journal's, naming its
    /// missions' halt cells at the broker. `None`: the coordinator takes one
    /// of its own (good for one coordinator's lifetime only).
    fn identity(&self) -> Option<String> {
        None
    }
}

/// M2d-C round 5: identities for journals that have none of their own.
static NEXT_IDENTITY: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// A process-unique identity (`label:n`).
pub(crate) fn fresh_identity(label: &str) -> String {
    let n = NEXT_IDENTITY.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    format!("{label}:{n}")
}

/// A journal kept in memory. Clones share the same lines, so a test can
/// rebuild a coordinator from what an earlier one wrote (a restart) or
/// edit a line (tampering).
#[derive(Clone)]
pub struct MemoryJournal {
    lines: Arc<Mutex<Vec<String>>>,
    poisoned: Arc<Mutex<bool>>,
    /// Tests: appends fail once this many more have succeeded.
    fail_after: Arc<Mutex<Option<usize>>>,
    /// Round 5: this journal's identity, shared by its clones.
    identity: Arc<String>,
}

impl Default for MemoryJournal {
    fn default() -> Self {
        Self {
            lines: Arc::default(),
            poisoned: Arc::default(),
            fail_after: Arc::default(),
            identity: Arc::new(fresh_identity("memory")),
        }
    }
}

impl MemoryJournal {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn lines(&self) -> Vec<String> {
        self.lines.lock().expect("journal").clone()
    }

    /// Replace every line (tests: tampering).
    pub fn set_lines(&self, lines: Vec<String>) {
        *self.lines.lock().expect("journal") = lines;
    }

    /// Make appends fail after `n` more succeed (tests: a failing disk).
    pub fn fail_after(&self, n: usize) {
        *self.fail_after.lock().expect("journal") = Some(n);
    }

    pub fn is_poisoned(&self) -> bool {
        *self.poisoned.lock().expect("journal")
    }
}

impl JournalStore for MemoryJournal {
    fn identity(&self) -> Option<String> {
        Some(self.identity.to_string())
    }

    fn load(&self) -> Result<Vec<String>, JournalError> {
        if self.is_poisoned() {
            return Err(JournalError::Poisoned);
        }
        Ok(self.lines())
    }

    fn append(&mut self, line: &str) -> Result<(), JournalError> {
        let mut fail_after = self.fail_after.lock().expect("journal");
        match fail_after.as_mut() {
            Some(0) => return Err(JournalError::Store(StoreFailure::Injected)),
            Some(n) => *n -= 1,
            None => {}
        }
        self.lines.lock().expect("journal").push(line.to_string());
        Ok(())
    }

    fn poison(&mut self) -> Result<(), JournalError> {
        *self.poisoned.lock().expect("journal") = true;
        Ok(())
    }
}
