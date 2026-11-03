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
    /// M0-FIX2 F10: kernel work (a model call) that the owner's emergency
    /// stop ended: its area, the stop's number and the outcome only.
    KernelHalt,
    /// Mother Nexus M2a: the owner's answer to an agent's mission: the
    /// contract hash, the request and a digest of the summary shown.
    MissionConsented,
    MissionDeclined,
    /// Mother Nexus M2c: the owner's answer to a door for an agent: the
    /// agent and the door's opaque id.
    AgentDoorOpened,
    AgentDoorDeclined,
    /// Mother Nexus M2c: the owner's answer to resuming a held mission: the
    /// mission, the contract hash, the request, the hold and a digest of
    /// the summary shown. A yes records the answer; it resumes nothing.
    MissionResumeConfirmed,
    MissionResumeDeclined,
    /// Mother Nexus M2c PR-B: an agent door was halted (it took effect
    /// first): the door's opaque id and the reason.
    DoorHalted,
    /// M2e-B: the Owner's host registered the process's one local model:
    /// its manifest digest and its identity digest.
    ModelRegistered,
    /// M2e-B: one attempted model call of a door's run: counts, digests,
    /// the outcome's code and the host monotonic return time; never a
    /// prompt, a reply or a model name.
    ModelCall,
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
            EvidencePhase::KernelHalt => "kernel_halt",
            EvidencePhase::MissionConsented => "mission_consented",
            EvidencePhase::MissionDeclined => "mission_declined",
            EvidencePhase::AgentDoorOpened => "agent_door_opened",
            EvidencePhase::AgentDoorDeclined => "agent_door_declined",
            EvidencePhase::MissionResumeConfirmed => "mission_resume_confirmed",
            EvidencePhase::MissionResumeDeclined => "mission_resume_declined",
            EvidencePhase::DoorHalted => "door_halted",
            EvidencePhase::ModelRegistered => "model_registered",
            EvidencePhase::ModelCall => "model_call",
        }
    }
}

/// Whose evidence a record is (P0, I6), fixed when its run is admitted and
/// copied, when they are created, into each commitment and credential
/// lease of the run: `Owner` only for a backend-issued owner run
/// ([`super::run::RunClass::Owner`]); `Restricted` for every other run, an
/// unknown one included. Never derived from door membership, a name or any
/// other text, and never relaxed. A record that names no run takes the
/// class its builder's inventory gives it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Privacy {
    /// Kept as written.
    Owner,
    /// Evidence only (I6): no target text; the registered counts, ids and
    /// codes; every other detail value as `<key>_sha256`.
    Restricted,
}

/// The digest domain of a Restricted record's text (I9).
pub(crate) const TEXT_DOMAIN: &str = "nexus.evidence.text.v1";

/// How a Restricted record keeps the value of a registered detail key
/// (I6). The registries are finite per key: a value that is not exactly
/// one of them is not kept, however much it looks like one.
enum Kept {
    /// A count: canonical decimal digits.
    Count,
    /// A credential lease's id (`lease-` and 32 lowercase hex).
    Lease,
    /// A door's opaque id (32 lowercase hex).
    Door,
    /// A SHA-256 digest (64 lowercase hex): a contract hash, a summary's
    /// digest.
    Sha256,
    /// Mother's mission id ([`is_mission_id`]).
    Mission,
    /// Mother's request label ([`is_mission_request`]).
    Request,
    /// One of these codes.
    Code(&'static [&'static str]),
}

/// The credential services the connector catalog names
/// (`connector::catalog`).
const SERVICES: &[&str] = &["Gmail", "Outlook", "Slack", "Discord"];
/// The kernel's halt areas (`nexus_kernel::emergency_stop::HaltArea`).
const AREAS: &[&str] = &["cognitive_loop", "coding", "model_call"];
/// The `class` a run-opened record names.
const RUN_CLASSES: &[&str] = &["owner"];
/// Operations named in detail (M2d: a Files create).
const OPERATIONS: &[&str] = &["create"];
/// What interrupted a Files publication after its final check (Codex
/// review 1 of PR #64, H3; the kernel's interruption causes).
const INTERRUPTIONS: &[&str] = &[
    "emergency_stop",
    "cancelled",
    "revoked",
    "expired",
    "commitment_expired",
    "write_grant_revoked",
    "write_grant_expired",
];
/// Why Mother holds a mission (its `HeldReason`).
const HELD_REASONS: &[&str] = &["unclean_shutdown"];

/// Mother's mission id, exactly as Mother writes it (`msn-{:06}`): `msn-`
/// and a number, in canonical decimal of at least six digits.
pub(crate) fn is_mission_id(text: &str) -> bool {
    text.strip_prefix("msn-").is_some_and(|digits| {
        (6..=20).contains(&digits.len())
            && digits.bytes().all(|b| b.is_ascii_digit())
            && digits
                .parse::<u64>()
                .is_ok_and(|n| format!("{n:06}") == digits)
    })
}

/// Mother's request label, exactly as Mother writes it
/// (`{mission}/{seq}/{number}`): a mission id ([`is_mission_id`]) and two
/// counts, separated by `/`.
pub(crate) fn is_mission_request(text: &str) -> bool {
    let parts: Vec<&str> = text.split('/').collect();
    matches!(
        parts.as_slice(),
        [mission, seq, number]
            if is_mission_id(mission) && Kept::Count.admits(seq) && Kept::Count.admits(number)
    )
}

/// One of the held reasons Mother names.
pub(crate) fn is_held_reason(text: &str) -> bool {
    HELD_REASONS.contains(&text)
}

/// The I6 registry: the detail keys a Restricted record keeps as they are,
/// each with its value class. Every other key's value becomes a digest.
fn registered(key: &str) -> Option<Kept> {
    Some(match key {
        "status" | "bytes" | "redirects" | "lines" => Kept::Count,
        "lease" => Kept::Lease,
        "door" => Kept::Door,
        // The Owner's answers at a door's mission and held-mission dialogs
        // (P0-C, Codex review 1): the controlled values only.
        "contract_hash" | "summary_digest" => Kept::Sha256,
        "mission" => Kept::Mission,
        "request" => Kept::Request,
        "held_reason" => Kept::Code(HELD_REASONS),
        "service" => Kept::Code(SERVICES),
        "reason" => Kept::Code(super::REASONS),
        "class" => Kept::Code(RUN_CLASSES),
        "area" => Kept::Code(AREAS),
        "op" => Kept::Code(OPERATIONS),
        "interrupted" => Kept::Code(INTERRUPTIONS),
        _ => return None,
    })
}

fn is_lower_hex(text: &str, len: usize) -> bool {
    text.len() == len && text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

impl Kept {
    fn admits(&self, value: &str) -> bool {
        match self {
            Kept::Count => {
                !value.is_empty()
                    && value.len() <= 20
                    && value.bytes().all(|b| b.is_ascii_digit())
                    && value.parse::<u64>().is_ok_and(|n| n.to_string() == value)
            }
            Kept::Lease => value
                .strip_prefix("lease-")
                .is_some_and(|hex| is_lower_hex(hex, 32)),
            Kept::Door => is_lower_hex(value, 32),
            Kept::Sha256 => is_lower_hex(value, 64),
            Kept::Mission => is_mission_id(value),
            Kept::Request => is_mission_request(value),
            Kept::Code(codes) => codes.contains(&value),
        }
    }
}

/// The digest a Restricted record keeps for `text` (I9).
pub(crate) fn text_digest(text: &str) -> String {
    super::ids::Digest::of(TEXT_DOMAIN, &[text.as_bytes()]).to_hex()
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
    /// Whose evidence it is (I6): given by its builder, never changed.
    privacy: Privacy,
    /// Sealed (I6 applied) on its way to a sink: sealing is done once.
    sealed: bool,
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
    /// A record of `privacy`'s class: its builder must say whose evidence
    /// it is (P0: all 15 builders do, by their inventory).
    pub(crate) fn new(
        phase: EvidencePhase,
        at_wall_ms: u64,
        policy_generation: u64,
        privacy: Privacy,
    ) -> Self {
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
            privacy,
            sealed: false,
        }
    }

    /// I6, applied once: an Owner record is kept as written; a Restricted
    /// one loses its target text, and each detail value that is not
    /// admitted by its key's registry is replaced by its digest, under
    /// `<key>_sha256`. Identities, classes, digests, times and fixed codes
    /// in the other fields are what the backend wrote and stay.
    fn seal(&mut self) {
        if std::mem::replace(&mut self.sealed, true) || self.privacy == Privacy::Owner {
            return;
        }
        self.target = None;
        for (key, value) in &mut self.detail {
            if !registered(key).is_some_and(|kept| kept.admits(value)) {
                *value = text_digest(value);
                key.push_str("_sha256");
            }
        }
    }

    /// This record as a sink receives it.
    fn sealed(&self) -> EvidenceRecord {
        let mut sealed = self.clone();
        sealed.seal();
        sealed
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

/// The one way a gating record reaches a sink (P0 §5.2): sealed first
/// (I6), so the store, its owed list, its projections and every later
/// reader see only what its privacy class keeps. A source scan permits no
/// other sink write in the crate (a guard, not a proof).
pub(crate) fn write_record(
    sink: &dyn EvidenceSink,
    record: &EvidenceRecord,
) -> Result<u64, EvidenceUnavailable> {
    sink.record(&record.sealed())
}

/// The one way an end record reaches a sink: sealed first, as
/// [`write_record`] does.
pub(crate) fn write_end(sink: &dyn EvidenceSink, record: &EvidenceRecord) -> EndStatus {
    sink.record_end(&record.sealed())
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

#[cfg(test)]
#[path = "evidence_tests.rs"]
mod tests;
