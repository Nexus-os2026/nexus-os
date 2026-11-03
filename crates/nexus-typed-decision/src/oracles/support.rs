//! Shared test support for the T1a oracles: the test inventory, fixtures,
//! the §4.4 weight formula, a seeded generator, an order-keeping JSON
//! reader and the P0 canary forms.

use crate::ident::{Label, QuestionId, SetId};
use crate::registry::{
    AnswerMode, Kind, Lane, OptionDisposition, ProjectionId, Question, QuestionSet, ReadPolicy,
};
use crate::{Element, Facts, Inventory, NoteSource, PageTextV1, Projection, UntrustedTextV1};

/// A test inventory (T1a only; T2a pins gemma4's 24 spellings).
pub const TEST_INVENTORY: Inventory = Inventory::new(&[
    "<|turn>",
    "<turn|>",
    "<bos>",
    "<|\"|>",
    "<eos>",
    "<|channel>",
]);

/// The literal six-character escapes, built from a backslash character so
/// that no tool can turn them back into `<` and `>`.
pub fn esc_lt() -> String {
    format!("{}u003c", '\\')
}
pub fn esc_gt() -> String {
    format!("{}u003e", '\\')
}

/// §4.4: `w('<') = w('>') = 6`; `w('"') = w('\') = 2`; newline and tab 2;
/// otherwise the UTF-8 length.
pub fn enc(s: &str) -> usize {
    s.chars()
        .map(|c| match c {
            '<' | '>' => 6,
            '"' | '\\' | '\n' | '\t' => 2,
            c => c.len_utf8(),
        })
        .sum()
}

/// §4.4's `page-text-v1` formula.
pub fn page_formula(
    name: &str,
    hours: &[String],
    phone: &str,
    title: &str,
    text: &[(Element, String)],
) -> usize {
    131 + enc(name)
        + enc(phone)
        + enc(title)
        + hours.iter().map(|h| enc(h) + 2).sum::<usize>()
        + hours.len().saturating_sub(1)
        + text
            .iter()
            .map(|(e, t)| e.code().len() + 7 + enc(t))
            .sum::<usize>()
        + text.len().saturating_sub(1)
}

/// §4.4's `untrusted-text-v1` formula.
pub fn note_formula(text: &str) -> usize {
    105 + enc(text)
}

pub fn page(
    name: &str,
    hours: &[&str],
    phone: &str,
    title: &str,
    text: &[(Element, &str)],
) -> Projection {
    Projection::PageText(PageTextV1::new(
        Facts::new(
            name.into(),
            hours.iter().map(|h| h.to_string()).collect(),
            phone.into(),
        ),
        title.into(),
        text.iter().map(|(e, t)| (*e, t.to_string())).collect(),
    ))
}

pub fn note(text: &str) -> Projection {
    Projection::UntrustedText(UntrustedTextV1::new(NoteSource::OwnerNote, text.into()))
}

pub const REF_NAME: &str = "Rosie's Bakery";
pub const REF_HOURS: [&str; 3] = [
    "Monday to Friday: 07:00 \u{2013} 17:00",
    "Saturday: 08:00 \u{2013} 14:00",
    "Sunday: closed",
];
pub const REF_PHONE: &str = "01632 960123";
pub const REF_TEXT: [(Element, &str); 10] = [
    (Element::H1, "Rosie's Bakery"),
    (Element::P, "Fresh bread and cakes, baked every morning."),
    (Element::H2, "Opening hours"),
    (Element::Li, "Monday to Friday: 07:00 \u{2013} 17:00"),
    (Element::Li, "Saturday: 08:00 \u{2013} 14:00"),
    (Element::Li, "Sunday: closed"),
    (Element::H2, "Contact"),
    (Element::P, "Phone: 01632 960123"),
    (Element::Address, "12 Mill Lane Exampleton"),
    (Element::P, "Hours may change on public holidays."),
];

/// The S1a reference page with its facts (`budget.py`).
pub fn ref_page() -> Projection {
    page(REF_NAME, &REF_HOURS, REF_PHONE, REF_NAME, &REF_TEXT)
}

/// The frozen minimum fixture (TD-O4, R2-5).
pub fn min_page() -> Projection {
    page(
        "a",
        &["a"],
        "1234567",
        "a",
        &[
            (Element::H1, "a"),
            (Element::P, "a"),
            (Element::P, "1234567"),
        ],
    )
}

/// splitmix64: a fixed-seed generator for the seeded projections.
pub struct Rng(pub u64);

impl Rng {
    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
    pub fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    pub fn pick<T: Copy>(&mut self, xs: &[T]) -> T {
        xs[self.below(xs.len())]
    }
}

pub fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

/// A test question with valid texts and the v1 read policy.
pub fn question(id: &str, kind: Kind) -> Question {
    Question {
        id: QuestionId::new(leak(id.into())),
        kind,
        instructions: "Is it so?",
        if_yes: "it is",
        if_no: "it is not",
        read: ReadPolicy::new(300_000, 700_000),
    }
}

/// A test set (the crate-private constructor used by property tests).
pub fn test_set(id: &str, mode: AnswerMode, questions: Vec<Question>) -> &'static QuestionSet {
    Box::leak(Box::new(QuestionSet {
        id: SetId::new(leak(id.into())),
        version: 1,
        lane: Lane::Sentinel,
        projection: ProjectionId::PageTextV1,
        mode,
        questions: Box::leak(questions.into_boxed_slice()),
    }))
}

pub fn labels(names: &[&str]) -> &'static [Label] {
    Box::leak(
        names
            .iter()
            .map(|n| Label::new(leak((*n).into())))
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    )
}

pub fn options(names: &[(&str, OptionDisposition)]) -> &'static [(Label, OptionDisposition)] {
    Box::leak(
        names
            .iter()
            .map(|(n, d)| (Label::new(leak((*n).into())), *d))
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    )
}

pub fn noul(polarity: crate::Polarity) -> Kind {
    Kind::Noul { polarity }
}

pub fn choice(names: &[(&str, OptionDisposition)]) -> Kind {
    Kind::Choice {
        options: options(names),
    }
}

pub fn score(levels: &[&str], at_least: u8, polarity: crate::Polarity) -> Kind {
    Kind::Score {
        levels: labels(levels),
        at_least,
        polarity,
    }
}

/// A copy of `base` changed by `f`.
pub fn edit(base: &QuestionSet, f: impl FnOnce(&mut QuestionSet)) -> QuestionSet {
    let mut set = QuestionSet { ..*base };
    f(&mut set);
    set
}

/// `base` with question `i` changed by `f`.
pub fn with_question(base: &QuestionSet, i: usize, f: impl Fn(&mut Question)) -> QuestionSet {
    let mut qs = base.questions.to_vec();
    f(&mut qs[i]);
    QuestionSet {
        questions: Box::leak(qs.into_boxed_slice()),
        ..*base
    }
}

/// A probabilities-mode set with one question of each kind.
pub fn mixed_set() -> &'static QuestionSet {
    use crate::Polarity::{Positive, Risk};
    use OptionDisposition::{Clear, NeedsOwner};
    let questions = vec![
        question("noul_q", noul(Risk)),
        question(
            "choice_q",
            choice(&[("fits", Clear), ("unclear", NeedsOwner)]),
        ),
        question("score_q", score(&["low", "mid", "high"], 2, Positive)),
    ];
    test_set("mix_1", AnswerMode::Probabilities, questions)
}

/// A JSON value that keeps key order (serde_json's map does not).
#[derive(Clone, Debug, PartialEq)]
pub enum J {
    Obj(Vec<(String, J)>),
    Arr(Vec<J>),
    Str(String),
    Num(String),
    Bool(bool),
    Null,
}

impl J {
    pub fn keys(&self) -> Vec<String> {
        match self {
            J::Obj(kv) => kv.iter().map(|(k, _)| k.clone()).collect(),
            other => panic!("not an object: {other:?}"),
        }
    }
    pub fn get(&self, key: &str) -> &J {
        match self {
            J::Obj(kv) => {
                &kv.iter()
                    .find(|(k, _)| k == key)
                    .unwrap_or_else(|| panic!("no key {key}"))
                    .1
            }
            other => panic!("not an object: {other:?}"),
        }
    }
}

/// Reads one JSON value with no string escapes (schemas and shapes only).
pub fn parse(text: &str) -> J {
    let mut p = Reader {
        b: text.as_bytes(),
        i: 0,
    };
    let v = p.value();
    p.ws();
    assert_eq!(p.i, p.b.len(), "trailing bytes after the value in {text}");
    v
}

struct Reader<'a> {
    b: &'a [u8],
    i: usize,
}

impl Reader<'_> {
    fn ws(&mut self) {
        while self.b.get(self.i).is_some_and(|c| b" \n\t\r".contains(c)) {
            self.i += 1;
        }
    }
    fn peek(&mut self) -> u8 {
        self.ws();
        *self.b.get(self.i).expect("unexpected end")
    }
    fn eat(&mut self, c: u8) {
        assert_eq!(self.peek(), c, "expected {}", c as char);
        self.i += 1;
    }
    fn value(&mut self) -> J {
        match self.peek() {
            b'{' => {
                self.i += 1;
                let mut kv = Vec::new();
                if self.peek() == b'}' {
                    self.i += 1;
                    return J::Obj(kv);
                }
                loop {
                    let k = self.string();
                    self.eat(b':');
                    kv.push((k, self.value()));
                    if self.peek() == b',' {
                        self.i += 1;
                    } else {
                        self.eat(b'}');
                        return J::Obj(kv);
                    }
                }
            }
            b'[' => {
                self.i += 1;
                let mut xs = Vec::new();
                if self.peek() == b']' {
                    self.i += 1;
                    return J::Arr(xs);
                }
                loop {
                    xs.push(self.value());
                    if self.peek() == b',' {
                        self.i += 1;
                    } else {
                        self.eat(b']');
                        return J::Arr(xs);
                    }
                }
            }
            b'"' => J::Str(self.string()),
            _ => {
                let start = self.i;
                while self
                    .b
                    .get(self.i)
                    .is_some_and(|c| c.is_ascii_alphanumeric() || b"-+.".contains(c))
                {
                    self.i += 1;
                }
                match &self.b[start..self.i] {
                    b"true" => J::Bool(true),
                    b"false" => J::Bool(false),
                    b"null" => J::Null,
                    n if !n.is_empty()
                        && n.iter().all(|c| c.is_ascii_digit() || b"-+.eE".contains(c)) =>
                    {
                        J::Num(String::from_utf8(n.to_vec()).unwrap())
                    }
                    n => panic!("not a JSON value: {}", String::from_utf8_lossy(n)),
                }
            }
        }
    }
    fn string(&mut self) -> String {
        self.eat(b'"');
        let start = self.i;
        while self.b[self.i] != b'"' {
            assert_ne!(self.b[self.i], b'\\', "escapes are not expected here");
            self.i += 1;
        }
        self.i += 1;
        String::from_utf8(self.b[start..self.i - 1].to_vec()).unwrap()
    }
}

/// P0 §5.4's canaries: `C` and `Cl`.
pub const C: &str = "NXCANARY-7f3a9c1e5b2d4f6a8c0e1b3d5f7a9c2e";
pub const CL: &str = "nxsecret_4e8a2c6f1b9d3e7a";

fn base64(bytes: &[u8], url: bool) -> String {
    let table: &[u8] = if url {
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_"
    } else {
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"
    };
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        for k in 0..4 {
            if k <= chunk.len() {
                out.push(table[((n >> (18 - 6 * k)) & 63) as usize] as char);
            } else if !url {
                out.push('=');
            }
        }
    }
    out
}

/// Every scanned form of a canary: raw, upper, lower, hex of its bytes,
/// base64 (std and url), percent-encoded, JSON `\u`-escaped, and four
/// fragments of at least 8 characters.
pub fn canary_forms(c: &str) -> Vec<String> {
    let bytes = c.as_bytes();
    let mut forms = vec![
        c.to_string(),
        c.to_uppercase(),
        c.to_lowercase(),
        bytes.iter().map(|b| format!("{b:02x}")).collect(),
        bytes.iter().map(|b| format!("{b:02X}")).collect(),
        base64(bytes, false).trim_end_matches('=').to_string(),
        base64(bytes, true),
        bytes.iter().map(|b| format!("%{b:02X}")).collect(),
        c.chars()
            .map(|ch| format!("{}u{:04x}", '\\', ch as u32))
            .collect(),
    ];
    let len = (c.len() / 4).max(8);
    for k in 0..4 {
        let start = (c.len() - len) * k / 3;
        forms.push(c[start..start + len].to_string());
    }
    forms
}

/// Panics if any form of either canary occurs in `text`.
pub fn assert_no_canary(what: &str, text: &str) {
    for c in [C, CL] {
        for form in canary_forms(c) {
            assert!(
                !text.contains(&form),
                "{what}: canary form {form:?} found in {text}"
            );
        }
    }
}
