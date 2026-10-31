//! The mission journal: append-only and hash-chained.
//!
//! Every transition and every lane invocation and effect is an [`Entry`].
//! Each entry's hash covers the previous entry's hash, its sequence number,
//! its mission and its event, so an edited, reordered, inserted or removed
//! entry breaks the chain and the journal is refused. A coordinator is
//! rebuilt from its journal alone; a journal that does not verify, or
//! whose triggers the state machine does not accept, opens nothing.
//!
//! Non-claim: the chain is unkeyed. It detects edits that do not rewrite
//! every later hash, and a torn last line, but not someone with write access
//! to Mother's home who rewrites the whole chain or cuts entries off its
//! end. Anchoring the head in the broker's evidence store is a later
//! mission.

use crate::contract::{sha256, EffectKind, MissionContract};
use crate::effect::PendingAction;
use crate::lane::{InvocationId, LaneIdentity};
use crate::state::Trigger;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::{Arc, Mutex};

const ENTRY_DOMAIN: &[u8] = b"nexus-mother-journal-v1\n";
/// The `prev` of the first entry.
pub const GENESIS: &str = "0000000000000000000000000000000000000000000000000000000000000000";

/// A mission's identity: a name, never authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct MissionId(pub u64);

impl fmt::Display for MissionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "msn-{:06}", self.0)
    }
}

/// What happened.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case", deny_unknown_fields)]
pub enum Event {
    /// A mission was drafted with this contract.
    Drafted { contract: MissionContract },
    /// A state transition.
    Transition { trigger: Trigger },
    /// A consent answer came for a contract or state that had moved on.
    ConsentRefused { reason: String },
    LaneInvoked {
        invocation: InvocationId,
        identity: LaneIdentity,
        input_hash: String,
    },
    LaneReturned {
        invocation: InvocationId,
        output_hash: String,
    },
    /// A lane's answer came after the mission had moved on (a Stop won).
    LaneOvertaken { invocation: InvocationId },
    /// Mother refused a lane's effect before it reached the port.
    EffectRefused { kind: EffectKind, reason: String },
    /// About to hand an effect to the port.
    EffectDispatched {
        effect: u64,
        kind: EffectKind,
        request_hash: String,
    },
    /// What the port said: `done`, `awaiting_approval` or a refusal class.
    EffectSettled { effect: u64, outcome: String },
    /// M2a: the broker's identity of the action effect `effect` waits on,
    /// journaled before the wait so that a rebuilt port can withdraw it.
    ApprovalPending { effect: u64, pending: PendingAction },
    /// M2a: the port confirmed that the broker no longer holds that action
    /// open (final, or gone). Written only after the confirmation; until
    /// it is, every reopen withdraws the action again.
    ApprovalClosed { effect: u64 },
}

/// One journal entry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub seq: u64,
    pub prev: String,
    pub mission: MissionId,
    pub event: Event,
    pub hash: String,
}

/// Why a journal cannot be read or written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JournalError {
    /// The store could not be read or written.
    Store(String),
    /// A line is not an entry, or the last line is torn.
    Malformed { line: usize },
    /// An entry's hash, `prev` or sequence does not follow from the one
    /// before.
    Tampered { line: usize },
    /// The entries verify but describe something the state machine
    /// forbids.
    Inconsistent { line: usize, reason: String },
    /// An append failed earlier: the journal may lack a transition (a
    /// Stop), so it opens nothing until the Owner reconciles it.
    Poisoned,
}

impl Entry {
    /// The entry that follows `prev` (the previous hash) at `seq`.
    pub fn chained(seq: u64, prev: &str, mission: MissionId, event: Event) -> Self {
        let mut entry = Self {
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
    /// sequence number, the mission and the event.
    pub fn expected_hash(&self) -> String {
        let event = serde_json::to_vec(&self.event).expect("an event serialises");
        hex::encode(sha256(
            ENTRY_DOMAIN,
            &[
                self.prev.as_bytes(),
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

/// Parse and verify a journal's lines: every entry well formed and in its
/// canonical form, numbered from 0, chained from [`GENESIS`] and hashed
/// correctly.
pub fn verify(lines: &[String]) -> Result<Vec<Entry>, JournalError> {
    let mut entries = Vec::with_capacity(lines.len());
    let mut prev = GENESIS.to_string();
    for (index, line) in lines.iter().enumerate() {
        let number = index + 1;
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
}

/// A journal kept in memory. Clones share the same lines, so a test can
/// rebuild a coordinator from what an earlier one wrote (a restart) or
/// edit a line (tampering).
#[derive(Clone, Default)]
pub struct MemoryJournal {
    lines: Arc<Mutex<Vec<String>>>,
    poisoned: Arc<Mutex<bool>>,
    /// Tests: appends fail once this many more have succeeded.
    fail_after: Arc<Mutex<Option<usize>>>,
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
    fn load(&self) -> Result<Vec<String>, JournalError> {
        if self.is_poisoned() {
            return Err(JournalError::Poisoned);
        }
        Ok(self.lines())
    }

    fn append(&mut self, line: &str) -> Result<(), JournalError> {
        let mut fail_after = self.fail_after.lock().expect("journal");
        match fail_after.as_mut() {
            Some(0) => return Err(JournalError::Store("injected failure".into())),
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
