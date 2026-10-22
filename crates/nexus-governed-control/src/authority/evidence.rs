//! The real-world action evidence contract.
//!
//! Every attempted governed action produces records: when it is prepared,
//! denied, approved or declined, started, finished (success, failure or
//! cancellation), expired or revoked. A record carries identities, classes,
//! digests and bounded display text, never a secret, a payload body, raw
//! pixels or audio. If the sink cannot record, the action does not start:
//! evidence comes first. The evidence of record is the Phase Three evidence
//! store ([`crate::evidence_store`], contract EL-1); the audit trail and the
//! interface's recent history are projections of it.

use super::effect::{CapabilityKind, EffectClass};
use serde_json::{json, Value};
use std::sync::Mutex;

/// The point in an action's life a record describes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EvidencePhase {
    RunOpened,
    RunCancelled,
    EmergencyStop,
    Resumed,
    GrantIssued,
    GrantDeclined,
    GrantRevoked,
    Prepared,
    Denied,
    ApprovalDeclined,
    Approved,
    Authorized,
    Started,
    Finished,
    Expired,
    Revoked,
    LeaseIssued,
    CredentialReleased,
    DisplayStarted,
    DisplayStopped,
}

impl EvidencePhase {
    pub fn as_str(self) -> &'static str {
        match self {
            EvidencePhase::RunOpened => "run_opened",
            EvidencePhase::RunCancelled => "run_cancelled",
            EvidencePhase::EmergencyStop => "emergency_stop",
            EvidencePhase::Resumed => "resumed",
            EvidencePhase::GrantIssued => "grant_issued",
            EvidencePhase::GrantDeclined => "grant_declined",
            EvidencePhase::GrantRevoked => "grant_revoked",
            EvidencePhase::Prepared => "prepared",
            EvidencePhase::Denied => "denied",
            EvidencePhase::ApprovalDeclined => "approval_declined",
            EvidencePhase::Approved => "approved",
            EvidencePhase::Authorized => "authorized",
            EvidencePhase::Started => "started",
            EvidencePhase::Finished => "finished",
            EvidencePhase::Expired => "expired",
            EvidencePhase::Revoked => "revoked",
            EvidencePhase::LeaseIssued => "lease_issued",
            EvidencePhase::DisplayStarted => "display_started",
            EvidencePhase::DisplayStopped => "display_stopped",
            EvidencePhase::CredentialReleased => "credential_released",
        }
    }
}

/// One evidence record. Every text field is bounded by `bounded`.
#[derive(Clone, Debug)]
pub struct EvidenceRecord {
    pub phase: EvidencePhase,
    pub at_wall_ms: u64,
    pub policy_generation: u64,
    pub commitment: Option<String>,
    pub agent: Option<String>,
    pub run: Option<String>,
    pub kind: Option<CapabilityKind>,
    pub class: Option<EffectClass>,
    pub operation: Option<&'static str>,
    pub target: Option<String>,
    pub target_digest: Option<String>,
    pub parameters_digest: Option<String>,
    pub approval: Option<String>,
    pub started_wall_ms: Option<u64>,
    pub finished_wall_ms: Option<u64>,
    pub outcome: Option<&'static str>,
    pub failure: Option<&'static str>,
    pub cancelled: bool,
    pub detail: Vec<(String, String)>,
}

/// Longest text a record keeps for one field.
pub const MAX_FIELD: usize = 256;
/// Most detail entries a record keeps.
pub const MAX_DETAIL: usize = 16;

/// Whether `c` is shown as itself: not a control character, not a space
/// other than the ASCII space (`U+3000` and the other Unicode spaces draw
/// blank, some twice as wide, so a line could be padded out of view with
/// nothing visible), and not an invisible formatting or direction-changing
/// character that would make a line read differently from what it is
/// (`U+202E` reverses what follows).
pub fn shown(c: char) -> bool {
    !c.is_control()
        && (c == ' ' || !c.is_whitespace())
        && !matches!(
            c,
            '\u{00AD}'
                | '\u{034F}'
                | '\u{0600}'..='\u{0605}'
                | '\u{061C}'
                | '\u{06DD}'
                | '\u{070F}'
                | '\u{0890}'..='\u{0891}'
                | '\u{08E2}'
                | '\u{115F}'..='\u{1160}'
                | '\u{17B4}'..='\u{17B5}'
                | '\u{180B}'..='\u{180F}'
                | '\u{200B}'..='\u{200F}'
                | '\u{2028}'..='\u{202E}'
                | '\u{2060}'..='\u{206F}'
                | '\u{2800}'
                | '\u{3164}'
                | '\u{FE00}'..='\u{FE0F}'
                | '\u{FEFF}'
                | '\u{FFA0}'
                | '\u{FFF0}'..='\u{FFFB}'
                | '\u{110BD}'
                | '\u{110CD}'
                | '\u{13430}'..='\u{1345F}'
                | '\u{1BCA0}'..='\u{1BCA3}'
                | '\u{1D173}'..='\u{1D17A}'
                | '\u{E0000}'..='\u{E0FFF}'
        )
}

/// Whether `text` can be shown to the owner exactly as it is: at most
/// `MAX_FIELD` characters, each shown as itself. Everything a native
/// confirmation displays must be plain.
pub fn is_plain(text: &str) -> bool {
    text.chars().count() <= MAX_FIELD && text.chars().all(shown)
}

/// The width, in characters, of a line of the owner's confirmation window:
/// [`wrapped`] and [`quoted`] break text there themselves, so the window
/// (which never wraps) shows every line whole, beginning with its marker.
pub const DIALOG_COLUMNS: usize = 96;

/// `text` made plain for display, in full: hidden characters escaped (`\n`,
/// `\u{202e}`), a backslash shown doubled (so text cannot pass for an
/// escape), and in each user-perceived character (an extended grapheme
/// cluster: a character with everything drawn onto it, in any script) all
/// but the first three code points escaped, since a stack of marks draws
/// over the lines around it. Nothing is cut: the authority refuses a line
/// longer than `MAX_FIELD`, so domains put long values through [`wrapped`]
/// and content the owner must read through [`quoted`]. What the owner is
/// shown is never shortened.
pub fn escaped(text: &str) -> String {
    use unicode_segmentation::UnicodeSegmentation;
    let mut out = String::new();
    for cluster in text.graphemes(true) {
        for (index, c) in cluster.chars().enumerate() {
            if c == '\\' {
                out.push_str("\\\\");
            } else if shown(c) && index < 3 {
                out.push(c);
            } else if c == '"' || c == '\'' {
                // Never `\"`: a quoted value (a window title) escapes its
                // own quotes, and an escaped quote must not look like one.
                out.extend(c.escape_unicode());
            } else {
                out.extend(c.escape_default());
            }
        }
    }
    out
}

/// One long value (a URL, a selector) on as many lines as it needs, never
/// cut: its first line, then continuations marked `↳ `; every line at most
/// `MAX_FIELD` characters.
pub fn wrapped(text: &str) -> Vec<String> {
    let chars: Vec<char> = escaped(text).chars().collect();
    let first = chars.len().min(DIALOG_COLUMNS);
    let mut lines = vec![chars[..first].iter().collect::<String>()];
    for chunk in chars[first..].chunks(DIALOG_COLUMNS - 2) {
        lines.push(format!("↳ {}", chunk.iter().collect::<String>()));
    }
    lines
}

/// Content the owner must read in full (a body, filled or typed text):
/// `label:`, then each of its lines marked `│ ` (continued `│↳ `), so no
/// line of it can pass for the dialog's own text. Hidden characters are
/// escaped; nothing is cut.
pub fn quoted(label: &str, text: &str) -> Vec<String> {
    let mut lines = vec![format!("{label}:")];
    for line in text.split('\n') {
        let chars: Vec<char> = escaped(line).chars().collect();
        let first = chars.len().min(DIALOG_COLUMNS - 2);
        lines.push(format!("│ {}", chars[..first].iter().collect::<String>()));
        for chunk in chars[first..].chunks(DIALOG_COLUMNS - 3) {
            lines.push(format!("│↳ {}", chunk.iter().collect::<String>()));
        }
    }
    lines
}

/// A hint that only selects (a window title): at most 64 characters,
/// shortened visibly with `…`.
pub fn hint(text: &str) -> String {
    let chars: Vec<char> = escaped(text).chars().collect();
    if chars.len() <= 64 {
        return chars.into_iter().collect();
    }
    let mut out: String = chars[..63].iter().collect();
    out.push('…');
    out
}

/// `text` for evidence: control characters become spaces, hidden ones
/// `U+FFFD`, at most `MAX_FIELD` characters.
pub fn bounded(text: &str) -> String {
    truncated(
        text.chars()
            .map(|c| {
                if c.is_control() {
                    ' '
                } else if shown(c) {
                    c
                } else {
                    '\u{FFFD}'
                }
            })
            .collect(),
    )
}

fn truncated(text: String) -> String {
    if text.chars().count() <= MAX_FIELD {
        return text;
    }
    let mut out: String = text.chars().take(MAX_FIELD - 1).collect();
    out.push('…');
    out
}

impl EvidenceRecord {
    pub fn new(phase: EvidencePhase, at_wall_ms: u64, policy_generation: u64) -> Self {
        Self {
            phase,
            at_wall_ms,
            policy_generation,
            commitment: None,
            agent: None,
            run: None,
            kind: None,
            class: None,
            operation: None,
            target: None,
            target_digest: None,
            parameters_digest: None,
            approval: None,
            started_wall_ms: None,
            finished_wall_ms: None,
            outcome: None,
            failure: None,
            cancelled: false,
            detail: Vec::new(),
        }
    }

    /// The record as bounded JSON, as the audit chain stores it.
    pub fn to_json(&self) -> Value {
        let detail: serde_json::Map<String, Value> = self
            .detail
            .iter()
            .take(MAX_DETAIL)
            .map(|(k, v)| (bounded(k), Value::String(bounded(v))))
            .collect();
        json!({
            "event_kind": "p3.action.evidence",
            "phase": self.phase.as_str(),
            "at_ms": self.at_wall_ms,
            "policy_generation": self.policy_generation,
            "commitment": self.commitment,
            "agent": self.agent.as_deref().map(bounded),
            "run": self.run,
            "kind": self.kind.map(|k| k.as_str()),
            "class": self.class.map(|c| c.as_str()),
            "operation": self.operation,
            "target": self.target.as_deref().map(bounded),
            "target_digest": self.target_digest,
            "parameters_digest": self.parameters_digest,
            "approval": self.approval,
            "started_ms": self.started_wall_ms,
            "finished_ms": self.finished_wall_ms,
            "outcome": self.outcome,
            "failure": self.failure,
            "cancelled": self.cancelled,
            "detail": detail,
        })
    }
}

/// The sink could not record; the action must not proceed.
#[derive(Debug)]
pub struct EvidenceUnavailable;

/// What became of an end record (contract EL-1 §6): a transition that has
/// already taken effect is never undone and never waits for the owner; its
/// record is written, owed (kept, with its original content, and written
/// before any later record), or uncertain (the store latched).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndStatus {
    Recorded { seq: u64 },
    Owed,
    Uncertain,
}

impl EndStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            EndStatus::Recorded { .. } => "recorded",
            EndStatus::Owed => "owed",
            EndStatus::Uncertain => "uncertain",
        }
    }
}

/// The evidence of one effect that started (EL-1 §8): its start record
/// (written before it started) and what became of its end record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExecutionEvidence {
    pub start: u64,
    pub end: EndStatus,
}

impl std::fmt::Display for ExecutionEvidence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "evidence: start recorded {}, end ", self.start)?;
        match self.end {
            EndStatus::Recorded { seq } => write!(f, "recorded {seq}"),
            other => write!(f, "{}", other.as_str()),
        }
    }
}

/// Where evidence goes. Production: [`crate::evidence_store::PrimaryEvidence`],
/// the Phase Three evidence store (EL-1).
pub trait EvidenceSink: Send + Sync {
    /// A gating record, written before the action it permits: the action
    /// happens only if this returns its sequence number.
    fn record(&self, record: &EvidenceRecord) -> Result<u64, EvidenceUnavailable>;

    /// An end record: a transition that has already taken effect (an end, a
    /// stop, a cancellation, a revocation). It never fails and never waits
    /// for the owner. A sink with no owed list (a test's) cannot promise a
    /// late write: a record it fails to take is uncertain.
    fn record_end(&self, record: &EvidenceRecord) -> EndStatus {
        match self.record(record) {
            Ok(seq) => EndStatus::Recorded { seq },
            Err(EvidenceUnavailable) => EndStatus::Uncertain,
        }
    }

    /// The evidence of record behind this sink, when it is the store.
    fn primary(&self) -> Option<&crate::evidence_store::PrimaryEvidence> {
        None
    }
}

/// A copy of the evidence of record (the audit trail, the interface's
/// recent history): written only after the store committed the record,
/// best effort; what it does never changes an outcome (EL-1 §6).
pub trait EvidenceProjection: Send + Sync {
    fn project(&self, record: &EvidenceRecord);
}

/// An in-memory sink: the bounded recent history the interface shows (as a
/// projection), and the sink tests use.
pub struct MemoryEvidence {
    records: Mutex<Vec<EvidenceRecord>>,
    capacity: usize,
    failing: std::sync::atomic::AtomicBool,
    /// The sequence number of the last record taken.
    taken: std::sync::atomic::AtomicU64,
}

impl MemoryEvidence {
    pub fn new(capacity: usize) -> Self {
        Self {
            records: Mutex::new(Vec::new()),
            capacity: capacity.max(1),
            failing: std::sync::atomic::AtomicBool::new(false),
            taken: std::sync::atomic::AtomicU64::new(0),
        }
    }

    pub fn records(&self) -> Vec<EvidenceRecord> {
        self.records.lock().expect("evidence").clone()
    }

    /// Keep `record`, the oldest dropped at capacity; returns its sequence
    /// number.
    fn keep(&self, record: &EvidenceRecord) -> u64 {
        let mut records = self.records.lock().expect("evidence");
        if records.len() == self.capacity {
            records.remove(0);
        }
        records.push(record.clone());
        self.taken
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
            .saturating_add(1)
    }

    fn is_failing(&self) -> bool {
        self.failing.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Make every later `record` fail (tests of the evidence-first rule).
    pub fn fail_from_now(&self) {
        self.failing
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }
}

impl EvidenceSink for MemoryEvidence {
    fn record(&self, record: &EvidenceRecord) -> Result<u64, EvidenceUnavailable> {
        if self.is_failing() {
            return Err(EvidenceUnavailable);
        }
        Ok(self.keep(record))
    }
}

impl EvidenceProjection for MemoryEvidence {
    /// A copy: a failing history keeps nothing and changes nothing.
    fn project(&self, record: &EvidenceRecord) {
        if !self.is_failing() {
            self.keep(record);
        }
    }
}

/// Fan one record out to several sinks; it fails if any of them fails (tests;
/// the production sink is the evidence store, with its projections).
pub struct TeeEvidence(pub Vec<std::sync::Arc<dyn EvidenceSink>>);

impl EvidenceSink for TeeEvidence {
    fn record(&self, record: &EvidenceRecord) -> Result<u64, EvidenceUnavailable> {
        let mut seq = 0;
        for sink in &self.0 {
            seq = sink.record(record)?;
        }
        Ok(seq)
    }
}
