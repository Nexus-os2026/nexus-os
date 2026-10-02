//! Classification and restart knowledge (design section 11): the record
//! grammar audited against the core (section 11.4), the pure per-file
//! classifier (section 11.1), the restart report and its refusal rule
//! (section 11.2), and the pool-level and archive checks with the split of
//! incidents into current and history (sections 11.3 and 13.5).
//!
//! Every function here is pure and deterministic (INV-6): equal bytes give
//! equal classes, reports and bindings. Classification never names an
//! identity that no visible record holds, never treats a missing record as
//! proof, and never reads a visible `RunEnded` or a recorded-unsettled count
//! of zero as resolution: an unsealed generation that recorded any action
//! start is refused, and so is every malformed file and every claim gap
//! (INV-3, INV-11). A decision uses the complete incident set, never the
//! bounded report (INV-16).

use std::collections::{BTreeMap, BTreeSet};

use sha2::{Digest, Sha256};

use super::super::codec::RecordEvidence;
use super::super::{
    ActionId, ClosureReason, ControlFact, Generation, IncidentId, PriorOutcome, RecordKind,
    Settlement, Verdict,
};
use super::format::{
    self, bind_gap, bind_journal, bind_pool, parse_header, parse_record_block, parse_seal,
    ArchiveClass, ArchiveName, Disposition, Header, HeaderParse, IncidentClass, IncidentFacts,
    IncidentKind, PrefixFacts, RecordBlock, SealParse, BLOCK, REPORT_LIMIT,
};

// ---------------------------------------------------------------------------
// The record grammar (design section 11.4)
// ---------------------------------------------------------------------------

/// The grammar rule a record breaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Rule {
    /// Instants are non-decreasing.
    G1,
    /// One `RunStarted`, never after the closure, counting the header's
    /// applied dispositions.
    G2,
    /// Before `RunStarted`, only control facts (a shutdown refusal after a
    /// closure).
    G3,
    G4,
    G5,
    G6,
    G7,
    G8,
    G9,
    G10,
    G11,
    G12,
    G13,
    G14,
    G15,
    /// `RunEnded` is Failed if and only if a closure precedes it.
    G16,
    /// After `RunEnded`, only incidents, their recovery and a shutdown
    /// refusal.
    G17,
    /// An embedded identity of another generation.
    GId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ActionState {
    failed: bool,
    settled: Option<Settlement>,
}

/// What the grammar learned from a prefix it accepted (or from the part of
/// a prefix before the record that broke it).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrammarSummary {
    pub run_started: bool,
    /// The `RunEnded` record's sequence, verdict and `resolved_by`.
    pub run_ended: Option<(u64, Verdict, Option<u32>)>,
    /// At least one `ActionStarted`.
    pub action_started: bool,
    /// Started actions not settled, by sequence.
    pub unsettled_actions: Vec<u64>,
    /// Opened incidents not settled, by sequence.
    pub unsettled_incidents: Vec<u64>,
}

impl GrammarSummary {
    pub fn unsettled(&self) -> u64 {
        (self.unsettled_actions.len() + self.unsettled_incidents.len()) as u64
    }
}

/// The record grammar's state over one journal's prefix. Prefix-closed: each
/// rule refers only to earlier records and the current one, so a log that
/// stops anywhere is valid.
#[derive(Debug, Clone)]
pub struct Grammar {
    generation: Generation,
    applied: u32,
    at: u64,
    run_started: bool,
    run_ended: Option<(u64, Verdict, Option<u32>)>,
    closed: bool,
    shutdown_refused: bool,
    case: Option<u32>,
    case_failed: bool,
    case_actions: Vec<u64>,
    actions: Vec<ActionState>,
    incidents: Vec<Option<Settlement>>,
    recovery_required: u64,
    attempts: u32,
    resolved_attempts: u64,
    last_resolved: Option<u32>,
    incident_after_end: bool,
}

impl Grammar {
    /// The grammar of a journal of `generation` whose header lists `applied`
    /// dispositions.
    pub fn new(generation: Generation, applied: u32) -> Self {
        Self {
            generation,
            applied,
            at: 0,
            run_started: false,
            run_ended: None,
            closed: false,
            shutdown_refused: false,
            case: None,
            case_failed: false,
            case_actions: Vec::new(),
            actions: Vec::new(),
            incidents: Vec::new(),
            recovery_required: 0,
            attempts: 0,
            resolved_attempts: 0,
            last_resolved: None,
            incident_after_end: false,
        }
    }

    fn own_action(&self, action: ActionId) -> bool {
        action.generation() == self.generation
    }

    fn own_incident(&self, incident: IncidentId) -> bool {
        incident.generation() == self.generation
    }

    fn all_settled(&self) -> bool {
        self.actions.iter().all(|action| action.settled.is_some())
            && self.incidents.iter().all(Option::is_some)
    }

    fn action(&mut self, action: ActionId) -> Option<&mut ActionState> {
        let seq = action.seq();
        if seq == 0 {
            return None;
        }
        self.actions.get_mut((seq - 1) as usize)
    }

    /// Accept the next record of the prefix, or name the rule it breaks.
    pub fn step(&mut self, record: &RecordEvidence) -> Result<(), Rule> {
        if record.at.0 < self.at {
            return Err(Rule::G1);
        }
        self.at = record.at.0;
        let kind = record.kind;
        let after_end_allowed = matches!(
            kind,
            RecordKind::IncidentOpened { .. }
                | RecordKind::IncidentSettled { .. }
                | RecordKind::RecoveryAttempt { .. }
                | RecordKind::Control {
                    fact: ControlFact::ShutdownRefused
                }
        );
        if self.run_ended.is_some() && !after_end_allowed {
            return Err(Rule::G17);
        }
        let before_start_allowed = matches!(
            kind,
            RecordKind::RunStarted { .. } | RecordKind::Control { .. }
        );
        if !self.run_started && !before_start_allowed {
            return Err(Rule::G3);
        }
        match kind {
            RecordKind::RunStarted { dispositioned } => {
                if self.run_started || self.closed || dispositioned != self.applied {
                    return Err(Rule::G2);
                }
                self.run_started = true;
            }
            RecordKind::CaseStarted { case, .. } => {
                if self.case.is_some()
                    || self.closed
                    || self.recovery_required > 0
                    || !self.all_settled()
                {
                    return Err(Rule::G4);
                }
                self.case = Some(case.0);
                self.case_failed = false;
                self.case_actions.clear();
            }
            RecordKind::CaseEnded { case, passed } => {
                if self.case != Some(case.0) {
                    return Err(Rule::G5);
                }
                let unsettled = self.case_actions.iter().any(|seq| {
                    self.actions
                        .get((*seq - 1) as usize)
                        .is_none_or(|action| action.settled.is_none())
                });
                if passed && (self.case_failed || unsettled) {
                    return Err(Rule::G5);
                }
                self.case = None;
            }
            RecordKind::ActionStarted { action, .. } => {
                if !self.own_action(action) {
                    return Err(Rule::GId);
                }
                let next = self.actions.len() as u64 + 1;
                if self.case.is_none() || self.closed || action.seq() != next {
                    return Err(Rule::G6);
                }
                self.actions.push(ActionState {
                    failed: false,
                    settled: None,
                });
                self.case_actions.push(next);
            }
            RecordKind::ActionFailed { action } => {
                if !self.own_action(action) {
                    return Err(Rule::GId);
                }
                let in_case = self.case.is_some();
                match self.action(action) {
                    Some(state) if !state.failed && state.settled.is_none() => {
                        state.failed = true;
                    }
                    _ => return Err(Rule::G7),
                }
                if in_case {
                    self.case_failed = true;
                }
            }
            RecordKind::ActionSettled { action, how } => {
                if !self.own_action(action) {
                    return Err(Rule::GId);
                }
                match self.action(action) {
                    Some(state) if state.settled.is_none() => {
                        if how == Settlement::NotAdmitted && state.failed {
                            return Err(Rule::G8);
                        }
                        state.settled = Some(how);
                    }
                    _ => return Err(Rule::G8),
                }
            }
            RecordKind::Control {
                fact: ControlFact::AdmissionClosed { reason, after },
            } => {
                if self.closed || after > self.actions.len() as u64 {
                    return Err(Rule::G9);
                }
                self.closed = true;
                if matches!(reason, ClosureReason::Failed(_)) && self.case.is_some() {
                    self.case_failed = true;
                }
            }
            RecordKind::Control {
                fact: ControlFact::ShutdownRefused,
            } => {
                if self.shutdown_refused {
                    return Err(Rule::G10);
                }
                if !self.run_started && !self.closed {
                    return Err(Rule::G3);
                }
                self.shutdown_refused = true;
            }
            RecordKind::RecoveryRequired => {
                if self.case.is_some() {
                    return Err(Rule::G11);
                }
                self.recovery_required += 1;
            }
            RecordKind::RecoveryAttempt { attempt, resolved } => {
                if Some(attempt) != self.attempts.checked_add(1) {
                    return Err(Rule::G12);
                }
                if self.run_ended.is_none() && self.recovery_required <= self.resolved_attempts {
                    return Err(Rule::G12);
                }
                if self.run_ended.is_some() && !self.incident_after_end {
                    return Err(Rule::G12);
                }
                self.attempts = attempt;
                if resolved && self.run_ended.is_none() {
                    self.resolved_attempts += 1;
                    self.last_resolved = Some(attempt);
                }
            }
            RecordKind::RunEnded {
                verdict,
                resolved_by,
            } => {
                if self.case.is_some()
                    || !self.all_settled()
                    || !matches!(verdict, Verdict::Passed | Verdict::Failed)
                {
                    return Err(Rule::G15);
                }
                let expected = if self.recovery_required == 0 {
                    None
                } else {
                    self.last_resolved
                };
                if resolved_by != expected {
                    return Err(Rule::G15);
                }
                if (verdict == Verdict::Failed) != self.closed {
                    return Err(Rule::G16);
                }
                self.run_ended = Some((record.id.seq(), verdict, resolved_by));
            }
            RecordKind::IncidentOpened {
                incident, action, ..
            } => {
                if !self.own_incident(incident) || !self.own_action(action) {
                    return Err(Rule::GId);
                }
                let next = self.incidents.len() as u64 + 1;
                let started = action.seq() >= 1 && action.seq() <= self.actions.len() as u64;
                if incident.seq() != next || !started {
                    return Err(Rule::G13);
                }
                self.incidents.push(None);
                if self.run_ended.is_some() {
                    self.incident_after_end = true;
                }
            }
            RecordKind::IncidentSettled { incident, how } => {
                if !self.own_incident(incident) {
                    return Err(Rule::GId);
                }
                let slot = if incident.seq() == 0 {
                    None
                } else {
                    self.incidents.get_mut((incident.seq() - 1) as usize)
                };
                match slot {
                    Some(settled)
                        if settled.is_none()
                            && matches!(how, Settlement::Confirmed | Settlement::OutputLost) =>
                    {
                        *settled = Some(how);
                    }
                    _ => return Err(Rule::G14),
                }
            }
        }
        Ok(())
    }

    pub fn summary(&self) -> GrammarSummary {
        GrammarSummary {
            run_started: self.run_started,
            run_ended: self.run_ended,
            action_started: !self.actions.is_empty(),
            unsettled_actions: (1..=self.actions.len() as u64)
                .filter(|seq| self.actions[(*seq - 1) as usize].settled.is_none())
                .collect(),
            unsettled_incidents: (1..=self.incidents.len() as u64)
                .filter(|seq| self.incidents[(*seq - 1) as usize].is_none())
                .collect(),
        }
    }
}

/// Run the grammar over a whole prefix: the first violation (its rule and
/// the record's index), and the summary up to it.
pub fn check_grammar(
    records: &[RecordEvidence],
    generation: Generation,
    applied: u32,
) -> (Option<(Rule, usize)>, GrammarSummary) {
    let mut grammar = Grammar::new(generation, applied);
    for (index, record) in records.iter().enumerate() {
        if let Err(rule) = grammar.step(record) {
            return (Some((rule, index)), grammar.summary());
        }
    }
    (None, grammar.summary())
}

// ---------------------------------------------------------------------------
// Per-file classification (design section 11.1)
// ---------------------------------------------------------------------------

/// A pool file's class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileClass {
    /// The size is not the pool file size: the store is Invalid.
    SizeInvalid,
    /// Every byte zero: never claimed.
    Unused,
    /// No checksum-valid header and every other block zero: an interrupted
    /// claim. In domain H nothing was recorded; it does not prove that in
    /// domains S or A.
    AbandonedClaim,
    /// No valid header, other than an abandoned claim.
    MalformedPoolFile,
    /// A valid header, then anything invalid.
    MalformedJournal,
    UnsealedNoAction,
    UnsealedAction,
    Sealed,
}

impl FileClass {
    pub fn malformed(self) -> bool {
        matches!(
            self,
            FileClass::MalformedPoolFile | FileClass::MalformedJournal
        )
    }
}

/// One recorded-unsettled identity of the visible prefix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unsettled {
    Action(u64),
    Incident(u64),
}

/// The four separate facts the store reports for each file (design section
/// 11.2), with what they rest on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileReport {
    pub index: u32,
    pub class: FileClass,
    pub reason: String,
    pub header: Option<Header>,
    /// The valid record prefix (empty unless the header is valid).
    pub records: Vec<RecordEvidence>,
    pub summary: Option<GrammarSummary>,
    pub seal: Option<SealParse>,
    /// The SHA-256 of the whole file.
    pub content: [u8; 32],
    /// Exact for the visible valid prefix; never a bound on native state.
    pub recorded_unsettled: Vec<Unsettled>,
    /// Complete only if sealed.
    pub evidence_complete: bool,
    /// False only for Unused, an abandoned claim, and a valid prefix with no
    /// action start.
    pub native_work_possible: bool,
    pub refused: bool,
    pub notes: Vec<&'static str>,
}

/// The note every unsealed generation where native work is possible carries.
pub const NOTE_LATE: &str =
    "late owners or other native effects may exist with no durable record (core.rs:1924-1930)";
pub const NOTE_AFTER_CLOSE: &str = "an owner surfacing after close() is outside custody";
pub const NOTE_ABANDONED: &str =
    "interrupted claim: this does not prove that nothing happened in domains S or A";

/// The refusal rule: every malformed file and every unsealed generation
/// with an action start, whatever its recorded-unsettled count.
pub fn refused(class: FileClass) -> bool {
    class.malformed() || class == FileClass::UnsealedAction
}

/// A streaming classifier: the file's blocks are fed in order, one at a
/// time; it keeps one header, the seal block, a SHA-256 state and grammar
/// state proportional to the capacity (design section 6.6).
pub struct FileClassifier {
    root_id: [u8; 16],
    c_pool: u32,
    index: u32,
    expected_blocks: u64,
    blocks: u64,
    hasher: Sha256,
    all_zero: bool,
    rest_zero: bool,
    header: Option<HeaderParse>,
    seal_block: Option<Box<[u8; BLOCK]>>,
    records: Vec<RecordEvidence>,
    ended: bool,
    block_failure: Option<String>,
    grammar: Option<Grammar>,
    grammar_failure: Option<(Rule, GrammarSummary)>,
}

impl FileClassifier {
    pub fn new(root_id: [u8; 16], c_pool: u32, index: u32) -> Self {
        Self {
            root_id,
            c_pool,
            index,
            expected_blocks: u64::from(c_pool) + 2,
            blocks: 0,
            hasher: Sha256::new(),
            all_zero: true,
            rest_zero: true,
            header: None,
            seal_block: None,
            records: Vec::new(),
            ended: false,
            block_failure: None,
            grammar: None,
            grammar_failure: None,
        }
    }

    /// Feed the next block; a block beyond the expected count is refused.
    pub fn feed(&mut self, block: &[u8]) -> Result<(), &'static str> {
        if block.len() != BLOCK {
            return Err("block length");
        }
        if self.blocks >= self.expected_blocks {
            return Err("more blocks than the pool file size");
        }
        let n = self.blocks;
        self.blocks += 1;
        self.hasher.update(block);
        let zero = block.iter().all(|byte| *byte == 0);
        self.all_zero &= zero;
        if n == 0 {
            let parsed = parse_header(block, &self.root_id, Some(self.c_pool), Some(self.index));
            if let HeaderParse::Valid(header) = &parsed {
                self.grammar = Some(Grammar::new(
                    Generation::new(header.generation),
                    header.applied.len() as u32,
                ));
            }
            self.header = Some(parsed);
            return Ok(());
        }
        self.rest_zero &= zero;
        if n == 1 {
            let mut seal = Box::new([0u8; BLOCK]);
            seal.copy_from_slice(block);
            self.seal_block = Some(seal);
            return Ok(());
        }
        let Some(HeaderParse::Valid(header)) = &self.header else {
            return Ok(());
        };
        if self.block_failure.is_some() {
            return Ok(());
        }
        let record_n = n - 1;
        if record_n > u64::from(header.capacity) {
            if !zero {
                self.block_failure = Some("block beyond capacity".into());
            }
            return Ok(());
        }
        let generation = header.generation;
        match parse_record_block(block) {
            RecordBlock::Zero => self.ended = true,
            _ if self.ended => self.block_failure = Some("hole".into()),
            RecordBlock::Invalid(why) => {
                self.block_failure = Some(format!("record block {record_n}: {why}"));
            }
            RecordBlock::Valid(record) => {
                if record.id.generation().bytes() != generation || record.id.seq() != record_n {
                    self.block_failure = Some(format!("record identity {record_n}"));
                    return Ok(());
                }
                if self.grammar_failure.is_none() {
                    if let Some(grammar) = self.grammar.as_mut() {
                        if let Err(rule) = grammar.step(&record) {
                            self.grammar_failure = Some((rule, grammar.summary()));
                        }
                    }
                }
                self.records.push(record);
            }
        }
        Ok(())
    }

    /// The report, once every block was fed (fewer blocks: SizeInvalid).
    pub fn finish(self) -> FileReport {
        let content: [u8; 32] = self.hasher.finalize().into();
        let mut report = FileReport {
            index: self.index,
            class: FileClass::SizeInvalid,
            reason: String::new(),
            header: None,
            records: Vec::new(),
            summary: None,
            seal: None,
            content,
            recorded_unsettled: Vec::new(),
            evidence_complete: false,
            native_work_possible: true,
            refused: true,
            notes: Vec::new(),
        };
        if self.blocks != self.expected_blocks {
            report.reason = "size".into();
            return finish_report(report);
        }
        if self.all_zero {
            report.class = FileClass::Unused;
            return finish_report(report);
        }
        let header = match self.header {
            Some(HeaderParse::Valid(header)) => header,
            Some(HeaderParse::ChecksumInvalid(_)) if self.rest_zero => {
                report.class = FileClass::AbandonedClaim;
                return finish_report(report);
            }
            Some(HeaderParse::ChecksumInvalid(why)) => {
                report.class = FileClass::MalformedPoolFile;
                report.reason = format!("header checksum-invalid ({why})");
                return finish_report(report);
            }
            Some(HeaderParse::Invalid(why)) => {
                report.class = FileClass::MalformedPoolFile;
                report.reason = format!("header invalid ({why})");
                return finish_report(report);
            }
            Some(HeaderParse::Zero) | None => {
                report.class = FileClass::MalformedPoolFile;
                report.reason = "header zero, other blocks not".into();
                return finish_report(report);
            }
        };
        report.header = Some(header.clone());
        report.records = self.records;
        if let Some(why) = self.block_failure {
            report.class = FileClass::MalformedJournal;
            report.reason = why;
            return finish_report(report);
        }
        if let Some((rule, summary)) = self.grammar_failure {
            report.class = FileClass::MalformedJournal;
            report.reason = format!("grammar {rule:?}");
            report.summary = Some(summary);
            return finish_report(report);
        }
        let summary = self
            .grammar
            .map(|grammar| grammar.summary())
            .unwrap_or_else(|| Grammar::new(Generation::new(header.generation), 0).summary());
        let facts = PrefixFacts {
            records: report.records.len() as u64,
            last_digest: report
                .records
                .last()
                .map(|record| record.digest)
                .unwrap_or([0; 32]),
            run_ended: summary.run_ended,
            run_started: summary.run_started,
            unsettled: summary.unsettled(),
        };
        let seal = match self.seal_block.as_deref() {
            Some(block) => parse_seal(block, &header, &facts),
            None => SealParse::Unreadable,
        };
        report.seal = Some(seal);
        report.class = match seal {
            SealParse::Malformed(why) => {
                report.reason = format!("seal: {why}");
                FileClass::MalformedJournal
            }
            SealParse::Sealed => FileClass::Sealed,
            SealParse::Unsealed | SealParse::Unreadable if summary.action_started => {
                FileClass::UnsealedAction
            }
            SealParse::Unsealed | SealParse::Unreadable => FileClass::UnsealedNoAction,
        };
        report.summary = Some(summary);
        finish_report(report)
    }
}

fn finish_report(mut report: FileReport) -> FileReport {
    let class = report.class;
    if let Some(summary) = &report.summary {
        report.recorded_unsettled = summary
            .unsettled_actions
            .iter()
            .map(|seq| Unsettled::Action(*seq))
            .chain(
                summary
                    .unsettled_incidents
                    .iter()
                    .map(|seq| Unsettled::Incident(*seq)),
            )
            .collect();
    }
    report.evidence_complete = class == FileClass::Sealed;
    report.native_work_possible = class.malformed()
        || class == FileClass::SizeInvalid
        || report
            .summary
            .as_ref()
            .is_some_and(|summary| summary.action_started);
    report.refused = refused(class) || class == FileClass::SizeInvalid;
    report.notes.clear();
    if class == FileClass::UnsealedAction {
        report.notes.push(NOTE_LATE);
        report.notes.push(NOTE_AFTER_CLOSE);
    }
    if class == FileClass::AbandonedClaim {
        report.notes.push(NOTE_ABANDONED);
    }
    report
}

/// Classify one pool file's complete bytes (a convenience over the
/// streaming classifier; a size other than the pool file size is
/// SizeInvalid before any block is examined).
pub fn classify_bytes(data: &[u8], root_id: [u8; 16], c_pool: u32, index: u32) -> FileReport {
    let mut classifier = FileClassifier::new(root_id, c_pool, index);
    let expected = format::pool_file_size(c_pool);
    if expected != Some(data.len() as u64) {
        return classifier.finish();
    }
    for block in data.chunks(BLOCK) {
        if classifier.feed(block).is_err() {
            break;
        }
    }
    classifier.finish()
}

// ---------------------------------------------------------------------------
// Pool-level and archive checks; incidents and history (sections 11.3, 13.5)
// ---------------------------------------------------------------------------

/// One incident the store computed: its facts and the outcome the core
/// receives for it. Plain data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Incident {
    pub facts: IncidentFacts,
    pub outcome: PriorOutcome,
}

/// One archive entry as startup reads it: its parsed name, blocks 0 and 1,
/// and its size. The rest is not re-read (design section 11.3).
#[derive(Debug, Clone)]
pub struct ArchiveEntry {
    pub name: String,
    pub parsed: ArchiveName,
    pub blocks: Vec<u8>,
    pub size: u64,
}

/// The store-level facts the pool-level checks need from `PROVISION`.
#[derive(Debug, Clone, Copy)]
pub struct PoolFacts {
    pub root_id: [u8; 16],
    pub c_pool: u32,
    pub retired_through: u64,
}

/// Everything the pool-level checks established, over the complete set.
#[derive(Debug, Clone, Default)]
pub struct PoolLevel {
    pub incidents: BTreeMap<[u8; 32], Incident>,
    pub history: BTreeSet<[u8; 32]>,
    /// Pool files archived and pending recycling.
    pub pending_recycle: BTreeSet<u32>,
    pub max_claim: u64,
    pub generations: BTreeSet<[u8; 16]>,
    pub applied: BTreeSet<[u8; 32]>,
}

/// Why the pool-level checks refused the store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PoolRefusal {
    Invalid(String),
    Capacity(String),
}

/// Whether a disposition restates exactly the incident the store computed.
pub fn disposition_matches(disposition: Option<&Disposition>, incident: &Incident) -> bool {
    disposition.is_some_and(|d| d.facts == incident.facts)
}

/// The pool-level and archive checks (design section 11.3), the incidents
/// and their split into current and history (section 13.5).
pub fn pool_level(
    facts: PoolFacts,
    files: &[FileReport],
    archive: &[ArchiveEntry],
    dispositions: &BTreeMap<[u8; 32], Disposition>,
    incident_limit: usize,
) -> Result<PoolLevel, PoolRefusal> {
    let invalid = |why: String| Err(PoolRefusal::Invalid(why));
    let root_id = facts.root_id;
    let retired = facts.retired_through;
    let mut level = PoolLevel::default();
    let mut claims: BTreeMap<u64, u32> = BTreeMap::new();
    let mut generations: BTreeMap<[u8; 16], u32> = BTreeMap::new();
    let mut archived_j: BTreeSet<(u64, [u8; 16], [u8; 32])> = BTreeSet::new();
    let mut archived_p: BTreeSet<(u32, [u8; 32])> = BTreeSet::new();
    let pool_size = format::pool_file_size(facts.c_pool);
    for entry in archive {
        match &entry.parsed {
            ArchiveName::Journal {
                claim,
                generation,
                content,
                class,
            } => {
                let header = match entry
                    .blocks
                    .get(0..BLOCK)
                    .map(|block| parse_header(block, &root_id, None, None))
                {
                    Some(HeaderParse::Valid(header))
                        if header.claim == *claim && header.generation == *generation =>
                    {
                        header
                    }
                    _ => return invalid(format!("archive {} header", entry.name)),
                };
                if format::pool_file_size(header.c_pool) != Some(entry.size) {
                    return invalid(format!("archive {} size", entry.name));
                }
                level
                    .applied
                    .extend(header.applied.iter().map(|(binding, _)| *binding));
                if *claim <= retired {
                    continue;
                }
                if *class == ArchiveClass::Sealed {
                    let seal = entry.blocks.get(BLOCK..2 * BLOCK).unwrap_or(&[]);
                    let checksum_valid = seal.len() == BLOCK
                        && seal[0..4] == format::SEAL_MAGIC
                        && format::sha256(&seal[0..128]) == seal[128..160];
                    if !checksum_valid
                        || seal[8..24] != *generation
                        || seal[24..32] != claim.to_be_bytes()
                        || seal[32..64] != header.digest
                    {
                        return invalid(format!("archive {} seal", entry.name));
                    }
                }
                if matches!(class, ArchiveClass::Unresolved | ArchiveClass::Malformed) {
                    let class = if *class == ArchiveClass::Unresolved {
                        IncidentClass::Unresolved
                    } else {
                        IncidentClass::Malformed
                    };
                    let binding = bind_journal(&root_id, *claim, generation, content, class);
                    if !dispositions.contains_key(&binding) {
                        return invalid(format!("archive {} inconsistent", entry.name));
                    }
                    level.history.insert(binding);
                }
                *claims.entry(*claim).or_default() += 1;
                *generations.entry(*generation).or_default() += 1;
                archived_j.insert((*claim, *generation, *content));
            }
            ArchiveName::PoolFile { index, content } => {
                if pool_size != Some(entry.size) {
                    return invalid(format!("archive {} size", entry.name));
                }
                let binding = bind_pool(&root_id, *index, content);
                if !dispositions.contains_key(&binding) {
                    return invalid(format!("archive {} inconsistent", entry.name));
                }
                level.history.insert(binding);
                archived_p.insert((*index, *content));
            }
        }
    }
    for report in files {
        let index = report.index;
        if report.class == FileClass::SizeInvalid {
            return invalid(format!("pool {index} size"));
        }
        let Some(header) = &report.header else {
            if report.class == FileClass::MalformedPoolFile {
                if archived_p.contains(&(index, report.content)) {
                    level.pending_recycle.insert(index);
                    continue;
                }
                let incident = Incident {
                    facts: IncidentFacts {
                        kind: IncidentKind::PoolFile,
                        claim: None,
                        generation: None,
                        pool_index: Some(index),
                        content: Some(report.content),
                        class: IncidentClass::Malformed,
                        recorded_unsettled: 0,
                    },
                    outcome: PriorOutcome::Malformed,
                };
                level
                    .incidents
                    .insert(bind_pool(&root_id, index, &report.content), incident);
            }
            continue;
        };
        level
            .applied
            .extend(header.applied.iter().map(|(binding, _)| *binding));
        if header.claim <= retired {
            return invalid(format!("pool {index} claim retired"));
        }
        if archived_j.contains(&(header.claim, header.generation, report.content)) {
            level.pending_recycle.insert(index);
            continue;
        }
        *claims.entry(header.claim).or_default() += 1;
        *generations.entry(header.generation).or_default() += 1;
        if matches!(
            report.class,
            FileClass::UnsealedAction | FileClass::MalformedJournal
        ) {
            let class = if report.class == FileClass::UnsealedAction {
                IncidentClass::Unresolved
            } else {
                IncidentClass::Malformed
            };
            let unsettled = report.recorded_unsettled.len() as u64;
            let outcome = match class {
                IncidentClass::Unresolved => PriorOutcome::Unresolved {
                    outstanding: u32::try_from(unsettled).unwrap_or(u32::MAX),
                },
                IncidentClass::Malformed => PriorOutcome::Malformed,
            };
            let binding = bind_journal(
                &root_id,
                header.claim,
                &header.generation,
                &report.content,
                class,
            );
            level.incidents.insert(
                binding,
                Incident {
                    facts: IncidentFacts {
                        kind: IncidentKind::Journal,
                        claim: Some(header.claim),
                        generation: Some(header.generation),
                        pool_index: None,
                        content: Some(report.content),
                        class,
                        recorded_unsettled: unsettled,
                    },
                    outcome,
                },
            );
        }
    }
    if let Some((claim, _)) = claims.iter().find(|(_, count)| **count > 1) {
        return invalid(format!("duplicate claim {claim}"));
    }
    if generations.values().any(|count| *count > 1) {
        return invalid("duplicate generation".into());
    }
    let top = claims
        .keys()
        .copied()
        .chain(std::iter::once(retired))
        .max()
        .unwrap_or(retired);
    level.max_claim = top;
    level.generations = generations.keys().copied().collect();
    let live = claims.keys().filter(|claim| **claim > retired).count() as u64;
    let gap_count = (top - retired).saturating_sub(live);
    let allowance = (incident_limit as u64).saturating_add(level.applied.len() as u64);
    if gap_count > allowance {
        return Err(PoolRefusal::Capacity(format!("{gap_count} claim gaps")));
    }
    if gap_count > 0 {
        for claim in retired + 1..=top {
            if !claims.contains_key(&claim) {
                level.incidents.insert(
                    bind_gap(&root_id, claim),
                    Incident {
                        facts: IncidentFacts {
                            kind: IncidentKind::ClaimGap,
                            claim: Some(claim),
                            generation: None,
                            pool_index: None,
                            content: None,
                            class: IncidentClass::Malformed,
                            recorded_unsettled: 0,
                        },
                        outcome: PriorOutcome::Malformed,
                    },
                );
            }
        }
    }
    let applied_history: Vec<[u8; 32]> = level
        .incidents
        .iter()
        .filter(|(binding, incident)| {
            level.applied.contains(*binding)
                && disposition_matches(dispositions.get(*binding), incident)
        })
        .map(|(binding, _)| *binding)
        .collect();
    level.history.extend(applied_history);
    Ok(level)
}

/// The current incidents (not history), in binding order.
pub fn current_incidents(level: &PoolLevel) -> Vec<([u8; 32], Incident)> {
    level
        .incidents
        .iter()
        .filter(|(binding, _)| !level.history.contains(*binding))
        .map(|(binding, incident)| (*binding, incident.clone()))
        .collect()
}

/// The authorization decision over the complete set (design section 10.2
/// step 8 and 13.10): more current incidents than the limit refuse as
/// Capacity; otherwise each current incident is dispositioned (its
/// disposition restates it exactly) or blocking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
    pub current: Vec<[u8; 32]>,
    pub dispositioned: Vec<[u8; 32]>,
    pub blocking: Vec<[u8; 32]>,
}

pub fn decide(
    level: &PoolLevel,
    dispositions: &BTreeMap<[u8; 32], Disposition>,
    incident_limit: usize,
) -> Result<Decision, PoolRefusal> {
    let current = current_incidents(level);
    if current.len() > incident_limit {
        return Err(PoolRefusal::Capacity(format!(
            "{} current incidents",
            current.len()
        )));
    }
    let mut decision = Decision {
        current: Vec::new(),
        dispositioned: Vec::new(),
        blocking: Vec::new(),
    };
    for (binding, incident) in current {
        decision.current.push(binding);
        if disposition_matches(dispositions.get(&binding), &incident) {
            decision.dispositioned.push(binding);
        } else {
            decision.blocking.push(binding);
        }
    }
    Ok(decision)
}

/// A bounded report: at most 64 incidents in detail (those without a claim
/// first, then in claim order, then by pool index and binding), with exact
/// totals and an explicit partial flag. Never used for a decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedReport {
    pub detail: Vec<([u8; 32], Incident)>,
    pub total: usize,
    pub current: usize,
    pub partial: bool,
}

pub fn bounded_report(level: &PoolLevel) -> BoundedReport {
    let mut everything: Vec<(&[u8; 32], &Incident)> = level.incidents.iter().collect();
    everything.sort_by_key(|(binding, incident)| {
        (
            incident
                .facts
                .claim
                .map_or(0, |claim| u128::from(claim) + 1),
            incident
                .facts
                .pool_index
                .map_or(0, |index| u64::from(index) + 1),
            **binding,
        )
    });
    BoundedReport {
        detail: everything
            .iter()
            .take(REPORT_LIMIT)
            .map(|(binding, incident)| (**binding, (*incident).clone()))
            .collect(),
        total: everything.len(),
        current: everything
            .iter()
            .filter(|(binding, _)| !level.history.contains(*binding))
            .count(),
        partial: everything.len() > REPORT_LIMIT,
    }
}
