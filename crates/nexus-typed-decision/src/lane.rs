//! The lane composition tables of §2.2 to §2.4, as pure functions. Advice
//! is never authority: only `Clear` is eligible, and no table is more
//! permissive than its deterministic verdict. (T1b oracle commit:
//! interface stub.)

use crate::budget::Refusal;
use crate::decode::Decision;
use crate::ident::QuestionId;

/// A1's `ModelError` codes, in A1's declaration order (`ModelError::CODES`
/// at `bdedc7fb`). T2b maps each `ModelError` onto exactly one of these.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TransportCode {
    NotLocal,
    InvalidAddress,
    InvalidModelName,
    CloudName,
    CloudEnabled,
    NotInstalled,
    DigestMismatch,
    RuntimeMismatch,
    Remote,
    ConfigMismatch,
    Unavailable,
    Timeout,
    RequestTooLarge,
    ResponseTooLarge,
    Malformed,
    Truncated,
    ToolCalls,
    ThinkingLeaked,
    EmptyContent,
    OverBudget,
    Stopped,
    Cancelled,
    Late,
}

impl TransportCode {
    pub const ALL: [Self; 23] = [
        Self::NotLocal,
        Self::InvalidAddress,
        Self::InvalidModelName,
        Self::CloudName,
        Self::CloudEnabled,
        Self::NotInstalled,
        Self::DigestMismatch,
        Self::RuntimeMismatch,
        Self::Remote,
        Self::ConfigMismatch,
        Self::Unavailable,
        Self::Timeout,
        Self::RequestTooLarge,
        Self::ResponseTooLarge,
        Self::Malformed,
        Self::Truncated,
        Self::ToolCalls,
        Self::ThinkingLeaked,
        Self::EmptyContent,
        Self::OverBudget,
        Self::Stopped,
        Self::Cancelled,
        Self::Late,
    ];

    pub const fn code(self) -> &'static str {
        match self {
            Self::NotLocal => "not_local",
            Self::InvalidAddress => "invalid_address",
            Self::InvalidModelName => "invalid_model_name",
            Self::CloudName => "cloud_name",
            Self::CloudEnabled => "cloud_enabled",
            Self::NotInstalled => "not_installed",
            Self::DigestMismatch => "digest_mismatch",
            Self::RuntimeMismatch => "runtime_mismatch",
            Self::Remote => "remote",
            Self::ConfigMismatch => "config_mismatch",
            Self::Unavailable => "unavailable",
            Self::Timeout => "timeout",
            Self::RequestTooLarge => "request_too_large",
            Self::ResponseTooLarge => "response_too_large",
            Self::Malformed => "malformed",
            Self::Truncated => "truncated",
            Self::ToolCalls => "tool_calls",
            Self::ThinkingLeaked => "thinking_leaked",
            Self::EmptyContent => "empty_content",
            Self::OverBudget => "over_budget",
            Self::Stopped => "stopped",
            Self::Cancelled => "cancelled",
            Self::Late => "late",
        }
    }
}

/// T1a's preflight refusal codes other than coverage (`Refusal::code`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PreflightCode {
    ProjectionMismatch,
    Unrenderable,
    CharNotAllowed,
    UnsafeDelimiter,
    RequestTooLarge,
}

impl PreflightCode {
    pub const ALL: [Self; 5] = [
        Self::ProjectionMismatch,
        Self::Unrenderable,
        Self::CharNotAllowed,
        Self::UnsafeDelimiter,
        Self::RequestTooLarge,
    ];

    pub const fn code(self) -> &'static str {
        match self {
            Self::ProjectionMismatch => "projection_mismatch",
            Self::Unrenderable => "unrenderable",
            Self::CharNotAllowed => "char_not_allowed",
            Self::UnsafeDelimiter => "unsafe_delimiter",
            Self::RequestTooLarge => "request_too_large",
        }
    }
}

/// Every way an assessment can fail to produce a decision (§2.1 rule 3).
/// Each one is restrictive for its lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Failure {
    /// A preflight refusal (§4.5, §5.3, §5.4).
    Refused(PreflightCode),
    /// The complete projection does not fit: every question reads
    /// Unsettled, set in code (§4.5, OD-1).
    CoverageInsufficient,
    /// An A1 error on either attempt; each is terminal.
    Transport(TransportCode),
    /// A reply over TD's 8,192-byte cap.
    ReplyTooLarge,
    /// A typed decode failure on attempt 2.
    Malformed,
    Stopped,
    Cancelled,
    Timeout,
    Late,
    /// The binding changed before settlement.
    Stale,
    /// The result could not be recorded.
    Unrecorded,
    /// Required advice absent at prepare or execute, or a decision of
    /// another set.
    Missing,
}

impl Failure {
    /// The failure's code inside a lane reason: `refused:<code>`,
    /// `coverage_insufficient`, `transport:<A1 code>`, `stale`, ...
    pub fn code(&self) -> String {
        match self {
            Self::Refused(code) => format!("refused:{}", code.code()),
            Self::CoverageInsufficient => "coverage_insufficient".into(),
            Self::Transport(code) => format!("transport:{}", code.code()),
            Self::ReplyTooLarge => "reply_too_large".into(),
            Self::Malformed => "malformed".into(),
            Self::Stopped => "stopped".into(),
            Self::Cancelled => "cancelled".into(),
            Self::Timeout => "timeout".into(),
            Self::Late => "late".into(),
            Self::Stale => "stale".into(),
            Self::Unrecorded => "unrecorded".into(),
            Self::Missing => "missing".into(),
        }
    }

    /// The failure a T1a preflight refusal stands for.
    pub fn from_refusal(_refusal: &Refusal) -> Self {
        Self::Missing
    }
}

/// A lane result's reason, from §2's closed registry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Reason {
    /// A decided set's first non-clear question (`page_advice:<id>`).
    Question(QuestionId),
    /// Sentinel: `td_<failure code>`.
    Td(Failure),
    AdviceDisagrees,
    AdviceUnsettled,
    AdviceUnavailable(Failure),
    FormatVariant,
    Deterministic,
    CheckError,
    NotesInstructsAi,
    NotesUnsettled,
    NotesUnavailable(Failure),
}

impl Reason {
    pub fn code(&self) -> String {
        match self {
            Self::Question(id) => id.as_str().into(),
            Self::Td(failure) => format!("td_{}", failure.code()),
            Self::AdviceDisagrees => "advice_disagrees".into(),
            Self::AdviceUnsettled => "advice_unsettled".into(),
            Self::AdviceUnavailable(failure) => format!("advice_unavailable:{}", failure.code()),
            Self::FormatVariant => "format_variant".into(),
            Self::Deterministic => "deterministic".into(),
            Self::CheckError => "check_error".into(),
            Self::NotesInstructsAi => "notes_screen:instructs_ai".into(),
            Self::NotesUnsettled => "notes_screen:unsettled".into(),
            Self::NotesUnavailable(failure) => {
                format!("notes_screen:unavailable:{}", failure.code())
            }
        }
    }
}

/// `Clear < NeedsOwner < Restrict`. Only `Clear` is eligible to proceed;
/// `NeedsOwner` is shown as "needs your decision" and is never a pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Disposition {
    Clear,
    NeedsOwner(Reason),
    Restrict(Reason),
}

impl Disposition {
    pub const fn eligible(&self) -> bool {
        false
    }
    pub const fn rank(&self) -> u8 {
        match self {
            Self::Clear => 0,
            Self::NeedsOwner(_) => 1,
            Self::Restrict(_) => 2,
        }
    }
    pub const fn reason(&self) -> Option<Reason> {
        match self {
            Self::Clear => None,
            Self::NeedsOwner(reason) | Self::Restrict(reason) => Some(*reason),
        }
    }
}

/// A deterministic verdict: S1a's for the Sentinel, M3's per-fact visible
/// text check for the Verifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Check {
    Pass,
    Fail,
    Error,
}

/// What the engine produced for one assessment.
#[derive(Clone, Copy, Debug)]
pub enum Advice<'a> {
    Decided(&'a Decision),
    Failed(Failure),
}

/// §2.1 rule 2: Clear only if every question is clear; otherwise the most
/// restrictive verdict, with the first non-clear question as its reason.
pub fn compose(_decision: &Decision) -> Disposition {
    Disposition::Restrict(Reason::Td(Failure::Missing))
}

/// The Sentinel's result (§2.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SentinelResult {
    /// S1a refused, or erred: its refusal, unchanged; no judge call.
    S1aRefusal(Check),
    /// Every question clear: the candidate goes on to the unchanged M2d
    /// dialogue and the Owner's approval. Clear adds nothing.
    Clear,
    /// A Halt, which the Owner cannot override (OD-3).
    Halt(Reason),
}

impl SentinelResult {
    pub const fn eligible(&self) -> bool {
        false
    }
}

/// §2.2: `s1a` first; advice only after S1a passed; `None` is advice that
/// is required but absent.
pub fn sentinel(_s1a: Check, _advice: Option<Advice<'_>>) -> SentinelResult {
    SentinelResult::Halt(Reason::Td(Failure::Missing))
}

/// The Verifier's result (§2.3): every fact's result, and the most
/// restrictive of them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VerifierResult {
    facts: [Disposition; 3],
    overall: Disposition,
}

impl VerifierResult {
    /// Name, hours and phone, in `ver_1`'s order.
    pub const fn facts(&self) -> [Disposition; 3] {
        self.facts
    }
    pub const fn overall(&self) -> Disposition {
        self.overall
    }
    pub const fn eligible(&self) -> bool {
        false
    }
}

/// §2.3: D per fact (name, hours, phone) and the advice for the same facts.
pub fn verifier(_d: [Check; 3], _advice: Option<Advice<'_>>) -> VerifierResult {
    let fail = Disposition::Restrict(Reason::CheckError);
    VerifierResult {
        facts: [fail; 3],
        overall: fail,
    }
}

/// §2.4: the Owner's approved note, screened before the Builder lane reads
/// it. Never a silent refusal: anything not clear needs the Owner.
pub fn notes_screen(_advice: Option<Advice<'_>>) -> Disposition {
    Disposition::NeedsOwner(Reason::NotesUnavailable(Failure::Missing))
}
