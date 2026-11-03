//! The strict reader (§7, `td-decode-v1`), exact fixed-point numbers, the
//! normalization `td-normalize-v1` and the decision they produce.
//! (T1b oracle commit: interface stub.)

use std::fmt;

use crate::ident::{Label, QuestionId};
use crate::registry::{AnswerMode, Ppm, QuestionSet, Reading};
use crate::render::DecodeCode;

/// TD's own cap on the reply content, checked before the reader (§8).
pub const MAX_REPLY_BYTES: usize = 8_192;

/// Why a reply was not decoded. Codes only: no reply byte is kept.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ReplyError {
    /// Over 8,192 bytes: terminal, never corrected.
    #[error("td_reply_too_large")]
    TooLarge,
    /// A typed decode failure; its code selects the correction phrase.
    #[error("td_decode:{}", .0.code())]
    Decode(DecodeCode),
}

/// Which attempt produced the reply.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Attempt {
    First,
    Second,
}

impl Attempt {
    pub const fn count(self) -> u8 {
        match self {
            Self::First => 1,
            Self::Second => 2,
        }
    }
}

/// The label with the largest ppm, or a tie.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Top {
    Label(Label),
    Tie,
}

impl Top {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Label(label) => label.as_str(),
            Self::Tie => "tie",
        }
    }
}

/// A Choice's or Score's vector: exactly the registered labels, in
/// registry order, summing to exactly 1,000,000.
#[derive(Debug, PartialEq, Eq)]
pub struct Distribution {
    entries: Vec<(Label, Ppm)>,
    renormalized: bool,
}

impl Distribution {
    pub fn entries(&self) -> &[(Label, Ppm)] {
        &self.entries
    }
    /// Whether the reply's sum was within tolerance but not exact.
    pub fn renormalized(&self) -> bool {
        self.renormalized
    }
    /// Computed in code, never read from the reply.
    pub fn top(&self) -> Top {
        Top::Tie
    }
    /// The probability of the levels from index `from` on.
    pub(crate) fn sum_from(&self, _from: usize) -> u32 {
        0
    }
}

/// One decoded answer, in exact ppm.
#[derive(Debug, PartialEq, Eq)]
pub enum Answer {
    Noul(Ppm),
    Choice(Distribution),
    Score(Distribution),
}

impl Answer {
    /// A Score's expectation `sum(i * p_i)` over its 0-based level
    /// indices, in level-ppm units; for explanation only, never read.
    pub fn expected(&self) -> Option<u64> {
        None
    }
}

/// A decoded, read reply. No `Clone`, no `Serialize`, redacted `Debug`;
/// only [`decode`] makes one.
pub struct Decision {
    set: &'static QuestionSet,
    binding: [u8; 32],
    attempt: Attempt,
    renormalized: bool,
    answers: Vec<(QuestionId, Answer)>,
    readings: Vec<(QuestionId, Reading)>,
}

impl Decision {
    pub fn set(&self) -> &'static QuestionSet {
        self.set
    }
    /// The binding the assessment was prepared under.
    pub fn binding(&self) -> &[u8; 32] {
        &self.binding
    }
    pub fn attempt(&self) -> Attempt {
        self.attempt
    }
    pub fn mode(&self) -> AnswerMode {
        self.set.mode()
    }
    /// Whether any vector was renormalized.
    pub fn renormalized(&self) -> bool {
        self.renormalized
    }
    /// The answers, in registry order.
    pub fn answers(&self) -> &[(QuestionId, Answer)] {
        &self.answers
    }
    /// The readings, in registry order.
    pub fn readings(&self) -> &[(QuestionId, Reading)] {
        &self.readings
    }
}

impl fmt::Debug for Decision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Decision")
            .field("set", &self.set.id().as_str())
            .field("attempts", &self.attempt.count())
            .field("renormalized", &self.renormalized)
            .field("answers", &self.answers.len())
            .finish_non_exhaustive()
    }
}

/// Decodes one reply for `set`: TD's 8,192-byte cap first, then the strict
/// reader, then the readings.
pub fn decode(
    _set: &'static QuestionSet,
    _reply: &str,
    _binding: [u8; 32],
    _attempt: Attempt,
) -> Result<Decision, ReplyError> {
    Err(ReplyError::Decode(DecodeCode::NotOneObject))
}
