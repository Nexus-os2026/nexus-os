//! The display-only record view (§7, §9.1): per-mille values rounded
//! half-even from ppm. Nothing reads it for gating, and there is no way
//! from it back to a [`Decision`]. (T1b oracle commit: interface stub.)

use crate::decode::{Attempt, Decision, Top};
use crate::ident::{QuestionId, SetId};
use crate::registry::{AnswerMode, Polarity, Reading};

/// One `td.answer`'s values.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnswerView {
    question: QuestionId,
    polarity: Option<Polarity>,
    reading: Reading,
    permille: u16,
    top: Option<Top>,
    top_permille: Option<u16>,
    vector_sha256: Option<[u8; 32]>,
}

impl AnswerView {
    pub fn question(&self) -> QuestionId {
        self.question
    }
    /// A Noul's or Score's polarity; a Choice has per-option dispositions.
    pub fn polarity(&self) -> Option<Polarity> {
        self.polarity
    }
    pub fn reading(&self) -> Reading {
        self.reading
    }
    /// The probability that was read: a Noul's, a Score's cumulative one,
    /// a Choice's top.
    pub fn permille(&self) -> u16 {
        self.permille
    }
    pub fn top(&self) -> Option<Top> {
        self.top
    }
    pub fn top_permille(&self) -> Option<u16> {
        self.top_permille
    }
    pub fn vector_sha256(&self) -> Option<[u8; 32]> {
        self.vector_sha256
    }
}

/// A decision's displayable values.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordView {
    set: SetId,
    mode: AnswerMode,
    attempt: Attempt,
    renormalized: bool,
    answers: Vec<AnswerView>,
}

impl RecordView {
    pub fn set(&self) -> SetId {
        self.set
    }
    pub fn mode(&self) -> AnswerMode {
        self.mode
    }
    pub fn attempts(&self) -> u8 {
        self.attempt.count()
    }
    pub fn renormalized(&self) -> bool {
        self.renormalized
    }
    pub fn answers(&self) -> &[AnswerView] {
        &self.answers
    }
}

/// Round-half-even from ppm to per-mille.
pub(crate) fn permille(_ppm: u32) -> u16 {
    0
}

impl Decision {
    /// The display-only view of this decision.
    pub fn record_view(&self) -> RecordView {
        RecordView {
            set: self.set().id(),
            mode: self.mode(),
            attempt: self.attempt(),
            renormalized: self.renormalized(),
            answers: Vec::new(),
        }
    }
}
