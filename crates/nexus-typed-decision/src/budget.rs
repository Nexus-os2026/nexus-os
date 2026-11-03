//! Preflight (§5.3 steps 1–6) and the exact byte budget (§4.4, §4.5).

use std::fmt;

use crate::ident::QuestionId;
use crate::projection::{frame, in_profile, Inventory, Projection};
use crate::registry::{QuestionSet, Reading};
use crate::render::{correction_sentence, render_schema, render_system, DecodeCode};

/// A1's bound on `system.len() + user.len()`.
pub const MAX_CONTENT_BYTES: usize = 3_936;

/// Coverage indicators (§4.1), computed for every preflight that reaches
/// the bound.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Coverage {
    pub(crate) source_text_bytes: usize,
    pub(crate) sent_text_bytes: usize,
    pub(crate) omitted_text_bytes: usize,
    pub(crate) user_bytes: usize,
    pub(crate) needed_bytes: usize,
    pub(crate) complete: bool,
}

impl Coverage {
    pub const fn source_text_bytes(&self) -> usize {
        self.source_text_bytes
    }
    pub const fn sent_text_bytes(&self) -> usize {
        self.sent_text_bytes
    }
    pub const fn omitted_text_bytes(&self) -> usize {
        self.omitted_text_bytes
    }
    /// The encoded, framed user message's bytes.
    pub const fn user_bytes(&self) -> usize {
        self.user_bytes
    }
    /// The attempt-2 system text with the longest correction, plus the
    /// user bytes: the one figure checked against the bound.
    pub const fn needed_bytes(&self) -> usize {
        self.needed_bytes
    }
    pub const fn complete(&self) -> bool {
        self.complete
    }
}

/// A request that passed preflight: both attempts fit the bound. `Debug`
/// shows sizes only.
pub struct Prepared {
    set: &'static QuestionSet,
    system: String,
    user: String,
    schema: String,
    coverage: Coverage,
}

impl Prepared {
    pub fn set(&self) -> &'static QuestionSet {
        self.set
    }
    /// The attempt-1 system text.
    pub fn system(&self) -> &str {
        &self.system
    }
    /// The attempt-2 system text for a decoder code.
    pub fn correction_system(&self, code: DecodeCode) -> String {
        attempt2(&self.system, code)
    }
    /// The user message, identical for both attempts.
    pub fn user(&self) -> &str {
        &self.user
    }
    /// The `format` schema text.
    pub fn schema(&self) -> &str {
        &self.schema
    }
    pub fn coverage(&self) -> &Coverage {
        &self.coverage
    }
    pub fn attempt1_bytes(&self) -> usize {
        self.system.len() + self.user.len()
    }
    /// The largest attempt-2 request (the longest correction).
    pub fn attempt2_max_bytes(&self) -> usize {
        self.coverage.needed_bytes
    }
}

impl fmt::Debug for Prepared {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Prepared")
            .field("set", &self.set.id().as_str())
            .field("system_bytes", &self.system.len())
            .field("user_bytes", &self.user.len())
            .field("schema_bytes", &self.schema.len())
            .field("coverage", &self.coverage)
            .finish()
    }
}

fn attempt2(system: &str, code: DecodeCode) -> String {
    format!("{system}\n\n{}", correction_sentence(code))
}

/// A typed preflight refusal: nothing is built, so nothing can be sent
/// or charged.
#[derive(Debug, thiserror::Error)]
pub enum Refusal {
    #[error("td_projection_mismatch")]
    ProjectionMismatch,
    #[error("td_unrenderable")]
    Unrenderable,
    #[error("td_char_not_allowed")]
    CharNotAllowed,
    #[error("td_unsafe_delimiter")]
    UnsafeDelimiter,
    /// The complete projection does not fit: every question reads
    /// Unsettled, set in code (§4.5, OD-1).
    #[error("td_coverage_insufficient")]
    CoverageInsufficient {
        coverage: Coverage,
        readings: Vec<(QuestionId, Reading)>,
    },
    #[error("td_request_too_large")]
    RequestTooLarge,
}

impl Refusal {
    /// The preflight code, as in `refused:<code>`.
    pub const fn code(&self) -> &'static str {
        match self {
            Self::ProjectionMismatch => "projection_mismatch",
            Self::Unrenderable => "unrenderable",
            Self::CharNotAllowed => "char_not_allowed",
            Self::UnsafeDelimiter => "unsafe_delimiter",
            Self::CoverageInsufficient { .. } => "coverage_insufficient",
            Self::RequestTooLarge => "request_too_large",
        }
    }

    /// The record's `outcome`.
    pub fn outcome(&self) -> String {
        format!("refused:{}", self.code())
    }
}

/// Preflight for one set and projection, with no charge and no
/// connection.
///
/// §5.3, in order: (1) the `td-chars-v1` profile on every admitted string;
/// (2) the inventory on every admitted string, before encoding; (3) the
/// `td-escape-v1` encoding; (4) the `td-frame-v1` frame; (5) the inventory
/// on the complete system texts of both attempts and the user bytes; (6)
/// the bound, for attempt 2 with the longest correction (§4.5): if the
/// complete projection does not fit, every question reads Unsettled.
pub fn prepare(
    set: &'static QuestionSet,
    projection: &Projection,
    inventory: &Inventory,
) -> Result<Prepared, Refusal> {
    if set.projection() != projection.id() {
        return Err(Refusal::ProjectionMismatch);
    }
    let system = render_system(set).map_err(|_| Refusal::Unrenderable)?;
    let corrections: Vec<String> = DecodeCode::ALL
        .iter()
        .map(|code| attempt2(&system, *code))
        .collect();
    let strings = projection.strings();
    let line_breaks = projection.allows_line_breaks();
    if !strings
        .iter()
        .all(|s| s.chars().all(|c| in_profile(c, line_breaks)))
    {
        return Err(Refusal::CharNotAllowed);
    }
    if strings.iter().any(|s| inventory.occurs_in(s)) {
        return Err(Refusal::UnsafeDelimiter);
    }
    let user = frame(&projection.encode());
    if inventory.occurs_in(&user)
        || inventory.occurs_in(&system)
        || corrections.iter().any(|c| inventory.occurs_in(c))
    {
        return Err(Refusal::UnsafeDelimiter);
    }
    // Attempt 2 with the longest correction; fail closed if none exists.
    let longest = corrections
        .iter()
        .map(String::len)
        .max()
        .unwrap_or(usize::MAX);
    let source = projection.source_text_bytes();
    let needed = longest.saturating_add(user.len());
    if needed > MAX_CONTENT_BYTES {
        let coverage = Coverage {
            source_text_bytes: source,
            sent_text_bytes: 0,
            omitted_text_bytes: source,
            user_bytes: user.len(),
            needed_bytes: needed,
            complete: false,
        };
        let readings = set.keys().map(|id| (id, Reading::Unsettled)).collect();
        return Err(Refusal::CoverageInsufficient { coverage, readings });
    }
    // The second guard (§4.5): unreachable once coverage is checked.
    let fits = |system: &str| system.len() + user.len() <= MAX_CONTENT_BYTES;
    if !fits(&system) || !corrections.iter().all(|c| fits(c)) {
        return Err(Refusal::RequestTooLarge);
    }
    let coverage = Coverage {
        source_text_bytes: source,
        sent_text_bytes: source,
        omitted_text_bytes: 0,
        user_bytes: user.len(),
        needed_bytes: needed,
        complete: true,
    };
    Ok(Prepared {
        set,
        system,
        user,
        schema: render_schema(set),
        coverage,
    })
}
