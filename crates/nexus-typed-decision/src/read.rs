//! Readings (`td-read-v1`) and the per-question polarity verdict (§2.1).
//! Equality with either threshold is Unsettled. Rounded values are never
//! read: a reading comes from the exact ppm of a decoded answer.

use crate::decode::{Answer, Top};
use crate::registry::{Kind, OptionDisposition, Polarity, Question, Reading};

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

/// Reads one decoded answer under its question's policy: a Noul's
/// probability, or a Score's cumulative probability of the levels from
/// `at_least`, is Yes above `yes_above` and No below `no_below`; a Choice
/// selects its top only if there is no tie and the top is above
/// `yes_above`. Anything else is Unsettled.
pub fn read(question: &Question, answer: &Answer) -> Reading {
    let policy = question.read();
    let noul = |p: u32| {
        if p > policy.yes_above().get() {
            Reading::Yes
        } else if p < policy.no_below().get() {
            Reading::No
        } else {
            Reading::Unsettled
        }
    };
    match (question.kind(), answer) {
        (Kind::Noul { .. }, Answer::Noul(p)) => noul(p.get()),
        (Kind::Score { at_least, .. }, Answer::Score(d)) => noul(d.sum_from(usize::from(at_least))),
        (Kind::Choice { .. }, Answer::Choice(d)) => match d.top() {
            Top::Label(label) if d.max_ppm() > policy.yes_above().get() => Reading::Selected(label),
            _ => Reading::Unsettled,
        },
        // Not an answer the reader produces for this question.
        _ => Reading::Unsettled,
    }
}

/// The verdict of one reading under its question's polarity. Risk: only No
/// clears. Positive: only Yes clears. A Choice: the selected option's
/// registered disposition. Everything else restricts.
pub fn verdict(question: &Question, reading: Reading) -> Verdict {
    match question.kind() {
        Kind::Noul { polarity } | Kind::Score { polarity, .. } => match (polarity, reading) {
            (Polarity::Risk, Reading::No) | (Polarity::Positive, Reading::Yes) => Verdict::Clear,
            _ => Verdict::Restrict,
        },
        Kind::Choice { options } => match reading {
            Reading::Selected(label) => options
                .iter()
                .find(|(option, _)| *option == label)
                .map_or(Verdict::Restrict, |(_, disposition)| *disposition),
            _ => Verdict::Restrict,
        },
    }
}
