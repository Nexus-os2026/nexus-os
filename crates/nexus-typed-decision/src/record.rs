//! The display-only record view (§7, §9.1): per-mille values rounded
//! half-even from ppm. Nothing reads it for gating, and there is no way
//! from it back to a [`Decision`].

use sha2::{Digest, Sha256};

use crate::decode::{Answer, Attempt, Decision, Distribution, Top};
use crate::hash::put;
use crate::ident::{QuestionId, SetId};
use crate::registry::{AnswerMode, Kind, Polarity, Question, Reading};

const VECTOR_DOMAIN: &[u8] = b"nexus.typed_decision.vector.v1";

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
pub(crate) fn permille(ppm: u32) -> u16 {
    let (q, r) = (ppm / 1_000, ppm % 1_000);
    let up = r > 500 || (r == 500 && q % 2 == 1);
    u16::try_from(q + u32::from(up)).unwrap_or(u16::MAX)
}

/// SHA-256 over the domain, the count, and each label and its exact ppm
/// (u32, big-endian), each length-prefixed.
fn vector_sha256(d: &Distribution) -> [u8; 32] {
    let mut h = Sha256::new();
    put(&mut h, VECTOR_DOMAIN);
    put(&mut h, &(d.entries().len() as u64).to_be_bytes());
    for (label, ppm) in d.entries() {
        put(&mut h, label.as_str().as_bytes());
        put(&mut h, &ppm.get().to_be_bytes());
    }
    h.finalize().into()
}

fn view(question: &Question, answer: &Answer, reading: Reading) -> AnswerView {
    let (read_ppm, vector) = match answer {
        Answer::Noul(p) => (p.get(), None),
        Answer::Choice(d) => (d.max_ppm(), Some(d)),
        Answer::Score(d) => {
            let at_least = match question.kind() {
                Kind::Score { at_least, .. } => usize::from(at_least),
                Kind::Noul { .. } | Kind::Choice { .. } => 0,
            };
            (d.sum_from(at_least), Some(d))
        }
    };
    AnswerView {
        question: question.id(),
        polarity: question.polarity(),
        reading,
        permille: permille(read_ppm),
        top: vector.map(Distribution::top),
        top_permille: vector.map(|d| permille(d.max_ppm())),
        vector_sha256: vector.map(vector_sha256),
    }
}

impl Decision {
    /// The display-only view of this decision.
    pub fn record_view(&self) -> RecordView {
        RecordView {
            set: self.set().id(),
            mode: self.mode(),
            attempt: self.attempt(),
            renormalized: self.renormalized(),
            answers: self
                .set()
                .questions()
                .iter()
                .zip(self.answers())
                .zip(self.readings())
                .map(|((q, (_, answer)), (_, reading))| view(q, answer, *reading))
                .collect(),
        }
    }
}
