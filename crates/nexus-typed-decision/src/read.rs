//! Readings (`td-read-v1`) and the per-question polarity verdict (§2.1).
//! (T1b oracle commit: interface stub.)

use crate::decode::Answer;
use crate::registry::{OptionDisposition, Question, Reading};

/// A question's verdict, on the Choice options' three-valued scale.
pub type Verdict = OptionDisposition;

impl Reading {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Yes => "yes",
            Self::No => "no",
            Self::Unsettled => "unsettled",
            Self::Selected(_) => "selected",
        }
    }
}

/// Reads one decoded answer under its question's policy.
pub fn read(_question: &Question, _answer: &Answer) -> Reading {
    Reading::Unsettled
}

/// The verdict of one reading under its question's polarity.
pub fn verdict(_question: &Question, _reading: Reading) -> Verdict {
    Verdict::Restrict
}
