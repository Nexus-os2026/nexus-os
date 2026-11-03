//! Preflight (§5.3 steps 1–6) and the exact byte budget (§4.4, §4.5).

use crate::ident::QuestionId;
use crate::projection::{Inventory, Projection};
use crate::registry::{QuestionSet, Reading};
use crate::render::DecodeCode;

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

/// A request that passed preflight: both attempts fit the bound.
#[derive(Debug)] // STUB (oracle commit): not yet redacted.
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
        // STUB (oracle commit).
        let _ = code;
        String::new()
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
pub fn prepare(
    set: &'static QuestionSet,
    projection: &Projection,
    inventory: &Inventory,
) -> Result<Prepared, Refusal> {
    // STUB (oracle commit).
    let _ = (set, projection, inventory);
    Err(Refusal::Unrenderable)
}
