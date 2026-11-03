//! The strict reader (§7, `td-decode-v1`), exact fixed-point numbers, the
//! normalization `td-normalize-v1` and the decision they produce.
//!
//! A small hand-written reader for exactly this language: no serde map, so
//! no rule depends on `deny_unknown_fields`. It runs in two passes, each in
//! document order. The first reads the shape (JSON whitespace, exactly one
//! object, JSON whitespace, end) and the depth, keeping only raw source
//! slices; as it reads each object, every key is checked against the keys
//! already read in that object before it is inserted (`duplicate_key`, at
//! every depth). The second matches the members to the registry: each key
//! must be a literal in the registry grammar and registered; then its
//! value; a missing key is found when its object closes. Keys are compared
//! as raw text, and a key with an escape is never registered, so no
//! duplicate can hide behind an escape. No floating point is used anywhere.

use std::cmp::Reverse;
use std::fmt;

use crate::ident::{is_ident, Label, QuestionId};
use crate::read::read;
use crate::registry::{AnswerMode, Kind, Ppm, Question, QuestionSet, Reading};
use crate::render::DecodeCode::{self, *};

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
        let max = self.max_ppm();
        let mut at_max = self.entries.iter().filter(|(_, p)| p.get() == max);
        match (at_max.next(), at_max.next()) {
            (Some((label, _)), None) => Top::Label(*label),
            _ => Top::Tie,
        }
    }
    /// The largest probability.
    pub(crate) fn max_ppm(&self) -> u32 {
        self.entries.iter().map(|(_, p)| p.get()).max().unwrap_or(0)
    }
    /// The probability of the levels from index `from` on.
    pub(crate) fn sum_from(&self, from: usize) -> u32 {
        self.entries.iter().skip(from).map(|(_, p)| p.get()).sum()
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
        match self {
            Self::Score(d) => Some(
                (0u64..)
                    .zip(&d.entries)
                    .map(|(i, (_, p))| i * u64::from(p.get()))
                    .sum(),
            ),
            Self::Noul(_) | Self::Choice(_) => None,
        }
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
    set: &'static QuestionSet,
    reply: &str,
    binding: [u8; 32],
    attempt: Attempt,
) -> Result<Decision, ReplyError> {
    if reply.len() > MAX_REPLY_BYTES {
        return Err(ReplyError::TooLarge);
    }
    let answers = read_reply(set, reply).map_err(ReplyError::Decode)?;
    let readings = set
        .questions()
        .iter()
        .zip(&answers)
        .map(|(q, (id, answer))| (*id, read(q, answer)))
        .collect();
    let renormalized = answers.iter().any(
        |(_, answer)| matches!(answer, Answer::Choice(d) | Answer::Score(d) if d.renormalized),
    );
    Ok(Decision {
        set,
        binding,
        attempt,
        renormalized,
        answers,
        readings,
    })
}

fn read_reply(
    set: &'static QuestionSet,
    reply: &str,
) -> Result<Vec<(QuestionId, Answer)>, DecodeCode> {
    let nodes = parse(reply)?;
    let questions = set.questions();
    let names: Vec<&str> = questions.iter().map(|q| q.id().as_str()).collect();
    let answers = members(&nodes, &names, |i, node| {
        answer(&questions[i], set.mode(), node)
    })?;
    Ok(questions.iter().map(Question::id).zip(answers).collect())
}

/// Every member of one object, in registry order: each key a literal in
/// the registry grammar (an escape is never decoded) and registered, never
/// filled twice (the first pass already refused a repeated key); at the
/// end, none missing.
fn members<T>(
    nodes: &[(&str, Node<'_>)],
    names: &[&str],
    mut value: impl FnMut(usize, &Node<'_>) -> Result<T, DecodeCode>,
) -> Result<Vec<T>, DecodeCode> {
    let mut slots: Vec<Option<T>> = names.iter().map(|_| None).collect();
    for (key, node) in nodes {
        if key.contains('\\') || !is_ident(key) {
            return Err(UnknownKey);
        }
        let i = names.iter().position(|n| n == key).ok_or(UnknownKey)?;
        if slots[i].is_some() {
            return Err(DuplicateKey);
        }
        slots[i] = Some(value(i, node)?);
    }
    slots
        .into_iter()
        .collect::<Option<Vec<T>>>()
        .ok_or(MissingKey)
}

fn answer(question: &Question, mode: AnswerMode, node: &Node<'_>) -> Result<Answer, DecodeCode> {
    if let Kind::Noul { .. } = question.kind() {
        return match (mode, node) {
            (AnswerMode::Probabilities, node) => probability(node).map(Answer::Noul),
            (AnswerMode::Discrete, Node::Word("0")) => Ok(Answer::Noul(Ppm(0))),
            (AnswerMode::Discrete, Node::Word("1")) => Ok(Answer::Noul(Ppm(Ppm::MAX))),
            (AnswerMode::Discrete, _) => Err(WrongType),
        };
    }
    let labels = question.kind().labels();
    let names: Vec<&str> = labels.iter().map(|l| l.as_str()).collect();
    let values = match (mode, node) {
        (AnswerMode::Probabilities, Node::Object(nodes)) => {
            members(nodes, &names, |_, n| probability(n))?
        }
        (AnswerMode::Discrete, Node::Str(raw)) => {
            let chosen = names.iter().position(|n| n == raw).ok_or(WrongType)?;
            (0..names.len())
                .map(|i| Ppm(if i == chosen { Ppm::MAX } else { 0 }))
                .collect()
        }
        _ => return Err(WrongType),
    };
    let dist = normalize(&labels, &values)?;
    Ok(match question.kind() {
        Kind::Choice { .. } => Answer::Choice(dist),
        _ => Answer::Score(dist),
    })
}

/// A probability in probabilities mode.
fn probability(node: &Node<'_>) -> Result<Ppm, DecodeCode> {
    match node {
        Node::Word("true" | "false" | "null") | Node::Object(_) | Node::Array => Err(WrongType),
        Node::Word(word) => fixed_point(word),
        Node::Str(_) => Err(BadNumber),
    }
}

/// `0`, `1`, `0.` followed by 1-6 digits, or `1.` followed by 1-6 zeros,
/// converted exactly. Any other plain decimal is out of range; anything
/// else (a sign, an exponent, `.5`, `00.5`, seven fraction digits, `NaN`)
/// is not a plain decimal. No value exists until the form is checked.
fn fixed_point(word: &str) -> Result<Ppm, DecodeCode> {
    let (int, frac) = word.split_once('.').unwrap_or((word, "0"));
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    let canonical = int == "0" || !int.starts_with('0');
    if !(digits(int) && canonical && digits(frac) && frac.len() <= 6) {
        return Err(BadNumber);
    }
    match int {
        "0" => Ok(Ppm(frac
            .bytes()
            .zip([100_000, 10_000, 1_000, 100, 10, 1])
            .map(|(b, scale)| u32::from(b - b'0') * scale)
            .sum())),
        "1" if frac.bytes().all(|b| b == b'0') => Ok(Ppm(Ppm::MAX)),
        _ => Err(OutOfRange),
    }
}

/// `td-normalize-v1`: an exact sum is kept; within 50,000 ppm of one
/// million, each value becomes `floor(p * 10^6 / S)` and the remainder goes
/// by largest remainder, ties to the earlier registry label; otherwise, or
/// a zero sum, `bad_sum`. There is no uniform fallback.
fn normalize(labels: &[Label], values: &[Ppm]) -> Result<Distribution, DecodeCode> {
    const ONE: u64 = 1_000_000;
    let sum: u64 = values.iter().map(|p| u64::from(p.get())).sum();
    let renormalized = sum != ONE;
    let ppm: Vec<u64> = if renormalized {
        if sum == 0 || sum.abs_diff(ONE) > 50_000 {
            return Err(BadSum);
        }
        let scaled: Vec<u64> = values.iter().map(|p| u64::from(p.get()) * ONE).collect();
        let mut out: Vec<u64> = scaled.iter().map(|x| x / sum).collect();
        let short = ONE - out.iter().sum::<u64>();
        let mut order: Vec<usize> = (0..out.len()).collect();
        order.sort_by_key(|&i| (Reverse(scaled[i] % sum), i));
        for &i in order.iter().take(short as usize) {
            out[i] += 1;
        }
        out
    } else {
        values.iter().map(|p| u64::from(p.get())).collect()
    };
    let entries = labels
        .iter()
        .zip(ppm)
        .map(|(label, p)| (*label, Ppm(p.min(ONE) as u32)))
        .collect();
    Ok(Distribution {
        entries,
        renormalized,
    })
}

/// A syntactically valid value of this language. Strings and words keep
/// only their raw source slice; nothing is decoded.
enum Node<'a> {
    Object(Vec<(&'a str, Node<'a>)>),
    Str(&'a str),
    Word(&'a str),
    Array,
}

/// Objects and arrays nest at most this deep; the top object is depth 1.
const MAX_DEPTH: usize = 2;

/// The first pass: exactly one object between JSON whitespace, and the
/// depth. A third level is refused as it opens, so the recursion is bounded.
fn parse(text: &str) -> Result<Vec<(&str, Node<'_>)>, DecodeCode> {
    let mut p = Parser { text, at: 0 };
    p.skip_ws();
    if p.peek() != Some(b'{') {
        return Err(NotOneObject);
    }
    let members = p.object(1)?;
    p.skip_ws();
    if p.at != text.len() {
        return Err(NotOneObject);
    }
    Ok(members)
}

struct Parser<'a> {
    text: &'a str,
    at: usize,
}

fn is_word(c: u8) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, b'+' | b'-' | b'.' | b'_')
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<u8> {
        self.text.as_bytes().get(self.at).copied()
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.at += 1;
        }
    }

    /// A value inside a container at `depth`.
    fn value(&mut self, depth: usize) -> Result<Node<'a>, DecodeCode> {
        self.skip_ws();
        match self.peek() {
            Some(b'{' | b'[') if depth >= MAX_DEPTH => Err(TooDeep),
            Some(b'{') => self.object(depth + 1).map(Node::Object),
            Some(b'[') => self.array(depth + 1).map(|()| Node::Array),
            Some(b'"') => self.string().map(Node::Str),
            Some(c) if is_word(c) => {
                let start = self.at;
                while self.peek().is_some_and(is_word) {
                    self.at += 1;
                }
                Ok(Node::Word(&self.text[start..self.at]))
            }
            _ => Err(NotOneObject),
        }
    }

    /// At `{`: its members, keys as raw slices.
    fn object(&mut self, depth: usize) -> Result<Vec<(&'a str, Node<'a>)>, DecodeCode> {
        self.at += 1;
        let mut members = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.at += 1;
            return Ok(members);
        }
        loop {
            self.skip_ws();
            if self.peek() != Some(b'"') {
                return Err(NotOneObject);
            }
            let key = self.string()?;
            if members.iter().any(|(seen, _)| *seen == key) {
                return Err(DuplicateKey);
            }
            self.skip_ws();
            if self.peek() != Some(b':') {
                return Err(NotOneObject);
            }
            self.at += 1;
            members.push((key, self.value(depth)?));
            if self.close(b'}')? {
                return Ok(members);
            }
        }
    }

    /// At `[`: checked, never kept (arrays are not used).
    fn array(&mut self, depth: usize) -> Result<(), DecodeCode> {
        self.at += 1;
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.at += 1;
            return Ok(());
        }
        loop {
            self.value(depth)?;
            if self.close(b']')? {
                return Ok(());
            }
        }
    }

    /// After a member or element: `,` continues, `end` closes.
    fn close(&mut self, end: u8) -> Result<bool, DecodeCode> {
        self.skip_ws();
        match self.peek() {
            Some(b',') => {
                self.at += 1;
                Ok(false)
            }
            Some(c) if c == end => {
                self.at += 1;
                Ok(true)
            }
            _ => Err(NotOneObject),
        }
    }

    /// At `"`: a valid JSON string; its raw content between the quotes.
    fn string(&mut self) -> Result<&'a str, DecodeCode> {
        self.at += 1;
        let start = self.at;
        loop {
            match self.peek() {
                None => return Err(NotOneObject),
                Some(b'"') => {
                    self.at += 1;
                    return Ok(&self.text[start..self.at - 1]);
                }
                Some(b'\\') => {
                    let bytes = self.text.as_bytes();
                    match bytes.get(self.at + 1) {
                        Some(b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't') => {
                            self.at += 2;
                        }
                        Some(b'u')
                            if bytes
                                .get(self.at + 2..self.at + 6)
                                .is_some_and(|h| h.iter().all(u8::is_ascii_hexdigit)) =>
                        {
                            self.at += 6;
                        }
                        _ => return Err(NotOneObject),
                    }
                }
                Some(c) if c < 0x20 => return Err(NotOneObject),
                Some(_) => self.at += 1,
            }
        }
    }
}
