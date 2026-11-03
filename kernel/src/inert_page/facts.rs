//! The approved facts (for M3) and the facts check, FC-1 to FC-4 (design v2
//! §5, frozen). Every parameter is read from `POLICY_V1`.
//!
//! **Element text** is the decoded text of an element's descendants, with
//! spans joined and `br` read as U+0020 (`POLICY_V1.flatten_text`). A fact
//! is compared with it byte for byte (`POLICY_V1.matching`): no case folding
//! and no substrings. A span's text is a range of its leaf's text, so every
//! element text comes from one pass over each leaf.
//!
//! - FC-1: exactly one `h1`, whose text is the name; the decoded title is
//!   the name.
//! - FC-2: each hours line is the whole text of some `li` or `p`.
//! - FC-3: the phone is the whole text of some leaf or span; inline spans
//!   inside one leaf count, separate blocks do not.
//! - FC-4: every phone-like token of every leaf's text and of the title has
//!   exactly the approved phone's digits.
//!
//! [`facts_report`] evaluates every check, even after an earlier one
//! failed (M3); [`check_facts`] is its first error, in the order above. The
//! accessors lend the approved values read-only.
//!
//! Not claimed (FC-4 is a conservative heuristic, and its false positives
//! fail closed): a number split across separate blocks or written in words
//! is missed; numbers joined only by separators read as one token, so a
//! price row or a slashed date is refused as foreign; a second,
//! contradictory set of hours is not detected; phone extensions and
//! international equivalence are not supported. The facts never change the
//! inert label.

use sha2::{Digest, Sha256};

use super::policy::POLICY_V1;
use super::tree::{Block, Inline, Leaf, MainItem, Page, Tag};
use super::work;

/// Why approved facts could not be built, or which fact a page lacks. A
/// code (and, for an hours line, its 0-based index) only, never content.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FactCode {
    /// A value outside its length, character, spacing or needle rules, or
    /// an hours line count outside its range.
    FactsInvalid,
    /// The phone is not in the frozen phone format.
    PhoneFormat,
    /// The name or an hours line holds a phone-like token.
    FactPhoneLike,
    NameNotHeading,
    NameNotTitle,
    HoursLineMissing {
        index: u8,
    },
    PhoneMissing,
    ForeignNumber,
}

impl FactCode {
    /// Every code's name, in position order.
    pub const CODES: [&'static str; 8] = [
        "facts_invalid",
        "phone_format",
        "fact_phone_like",
        "name_not_heading",
        "name_not_title",
        "hours_line_missing",
        "phone_missing",
        "foreign_number",
    ];

    /// The numeric code: this code's index in [`FactCode::CODES`].
    pub const fn position(self) -> u16 {
        match self {
            FactCode::FactsInvalid => 0,
            FactCode::PhoneFormat => 1,
            FactCode::FactPhoneLike => 2,
            FactCode::NameNotHeading => 3,
            FactCode::NameNotTitle => 4,
            FactCode::HoursLineMissing { .. } => 5,
            FactCode::PhoneMissing => 6,
            FactCode::ForeignNumber => 7,
        }
    }

    /// The code's stable name.
    pub const fn as_str(self) -> &'static str {
        Self::CODES[self.position() as usize]
    }
}

impl std::fmt::Display for FactCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The facts the Owner approved for a page: immutable once built, compared
/// by value. It is held in memory only: its `Debug` shows only
/// [`ApprovedFacts::hash`], and it has no `Display` and no `Serialize`.
#[derive(Clone, PartialEq, Eq)]
pub struct ApprovedFacts {
    name: String,
    hours: Vec<String>,
    phone: String,
    hash: [u8; 32],
}

const FACTS_DOMAIN: &[u8] = b"nexus.inert_page.facts.v1";

impl ApprovedFacts {
    /// Builds the facts under `POLICY_V1.facts_construction`, in its order:
    /// every value's length, characters, spacing and needles, the name's
    /// length and the number of hours lines (`facts_invalid`); then the
    /// phone's format (`phone_format`); then no phone-like token in the name
    /// or an hours line (`fact_phone_like`), so an approved fact can never
    /// trip FC-4.
    pub fn new(name: &str, hours: &[&str], phone: &str) -> Result<Self, FactCode> {
        let p = &POLICY_V1;
        let values = || std::iter::once(name).chain(hours.iter().copied());
        if !values().chain([phone]).all(value_ok)
            || name.chars().count() > p.facts_name_max_chars
            || !(p.facts_hours_min_lines..=p.facts_hours_max_lines).contains(&hours.len())
        {
            return Err(FactCode::FactsInvalid);
        }
        if !phone_shape_ok(phone) {
            return Err(FactCode::PhoneFormat);
        }
        if values().any(|v| !phone_like_tokens(v).is_empty()) {
            return Err(FactCode::FactPhoneLike);
        }
        let mut h = Sha256::new();
        put(&mut h, FACTS_DOMAIN);
        put(&mut h, name.as_bytes());
        // At most `facts_hours_max_lines` lines, so the count fits.
        h.update((hours.len() as u32).to_be_bytes());
        for line in hours {
            put(&mut h, line.as_bytes());
        }
        put(&mut h, phone.as_bytes());
        Ok(ApprovedFacts {
            name: name.to_owned(),
            hours: hours.iter().map(|l| (*l).to_owned()).collect(),
            phone: phone.to_owned(),
            hash: h.finalize().into(),
        })
    }

    /// `SHA-256(put("nexus.inert_page.facts.v1") ‖ put(name) ‖ u32 n ‖
    /// put(line)* ‖ put(phone))`, where `put(b)` is `b`'s length as a u64
    /// (big-endian) then `b`, and `n` is big-endian.
    pub fn hash(&self) -> [u8; 32] {
        self.hash
    }

    /// The number of approved hours lines.
    pub(super) fn hours_lines(&self) -> usize {
        self.hours.len()
    }

    /// The approved name, borrowed (M3 §5.6): read-only, so the facts stay
    /// as they were approved.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The approved hours lines, in order, borrowed (M3 §5.6).
    pub fn hours(&self) -> &[String] {
        &self.hours
    }

    /// The approved phone, borrowed (M3 §5.6).
    pub fn phone(&self) -> &str {
        &self.phone
    }
}

impl std::fmt::Debug for ApprovedFacts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApprovedFacts")
            .field("hash", &hex::encode(self.hash))
            .finish()
    }
}

fn put(h: &mut Sha256, b: &[u8]) {
    h.update((b.len() as u64).to_be_bytes());
    h.update(b);
}

fn in_ranges(ranges: &[(u32, u32)], c: char) -> bool {
    let v = u32::from(c);
    ranges.iter().any(|&(lo, hi)| (lo..=hi).contains(&v))
}

/// One value: its length in characters, S1's character rules (no control,
/// so no line feed; nothing invisible; only the profile), the spacing rules
/// (no leading, trailing or double space) and no needle.
fn value_ok(v: &str) -> bool {
    let p = &POLICY_V1;
    let chars = v.chars().count();
    let lower = v.to_ascii_lowercase();
    (p.facts_value_min_chars..=p.facts_value_max_chars).contains(&chars)
        && v.chars().all(|c| {
            !in_ranges(p.control_ranges, c)
                && !in_ranges(p.invisible_ranges, c)
                && in_ranges(p.profile_ranges, c)
        })
        && !v.starts_with(' ')
        && !v.ends_with(' ')
        && !v.contains("  ")
        && !p.needles.iter().any(|(n, _)| lower.contains(n))
}

/// The frozen phone format (`POLICY_V1.phone_shape`): only `phone_chars`; a
/// `+` only as the first character; it starts with `+`, `(` or a digit and
/// ends with a digit; at most `phone_max_non_digit_run` non-digits in a row;
/// `phone_min_digits` to `phone_max_digits` ASCII digits. Letters are not
/// allowed, so an extension is refused.
fn phone_shape_ok(phone: &str) -> bool {
    let p = &POLICY_V1;
    let mut run = 0;
    let mut digits = 0;
    for (k, c) in phone.chars().enumerate() {
        if !p.phone_chars.contains(&c) || (c == '+' && k > 0) {
            return false;
        }
        if c.is_ascii_digit() {
            digits += 1;
            run = 0;
        } else {
            run += 1;
            if run > p.phone_max_non_digit_run {
                return false;
            }
        }
    }
    matches!(phone.chars().next(), Some(c) if c == '+' || c == '(' || c.is_ascii_digit())
        && phone.chars().last().is_some_and(|c| c.is_ascii_digit())
        && (p.phone_min_digits..=p.phone_max_digits).contains(&digits)
}

/// FC-4's tokenizer (`POLICY_V1.fc4`). A token is a maximal substring that
/// begins and ends with an ASCII digit and holds only ASCII digits and
/// `fc4_separators`; its digits are the token without its separators. Each
/// token with at least `fc4_threshold` digits is phone-like; the result is
/// the digits of each, in order.
fn phone_like_tokens(text: &str) -> Vec<String> {
    let p = &POLICY_V1;
    let mut found = Vec::new();
    let mut digits = String::new();
    let mut flush = |digits: &mut String| {
        if digits.len() >= p.fc4_threshold {
            found.push(std::mem::take(digits));
        } else {
            digits.clear();
        }
    };
    for c in text.chars() {
        work::add(1);
        if c.is_ascii_digit() {
            digits.push(c);
        } else if !p.fc4_separators.contains(&c) {
            flush(&mut digits);
        }
    }
    flush(&mut digits);
    found
}

/// Byte-exact equality of two texts (`POLICY_V1.matching`), counted.
fn same(a: &str, b: &str) -> bool {
    work::add(1);
    if a.len() != b.len() {
        return false;
    }
    work::add(a.len());
    a == b
}

/// A leaf's element text, and the byte range of each of its spans' texts
/// in it, in document order. It does not recurse, so a deep tree cannot
/// exhaust the stack.
fn flatten(leaf: &Leaf, spans: &mut Vec<(usize, usize)>) -> String {
    let mut text = String::new();
    let mut stack: Vec<(std::slice::Iter<'_, Inline>, Option<usize>)> =
        vec![(leaf.inlines.iter(), None)];
    while let Some(top) = stack.last_mut() {
        let Some(item) = top.0.next() else {
            if let Some((_, Some(k))) = stack.pop() {
                spans[k].1 = text.len();
            }
            continue;
        };
        work::add(1);
        match item {
            Inline::Text(t) => {
                work::add(t.len());
                text.push_str(t);
            }
            Inline::Br => text.push(' '),
            Inline::Strong(s) | Inline::Em(s) | Inline::Small(s) => {
                spans.push((text.len(), text.len()));
                stack.push((s.inlines.iter(), Some(spans.len() - 1)));
            }
        }
    }
    text
}

impl Leaf {
    /// The element text of this leaf or span: decoded, spans joined, `br`
    /// read as U+0020.
    pub fn text(&self) -> String {
        flatten(self, &mut Vec::new())
    }
}

/// One leaf of the page: its tag, its element text, and its spans' ranges.
struct LeafText {
    tag: Tag,
    text: String,
    spans: Vec<(usize, usize)>,
}

/// Every leaf of `page`, in document order.
fn leaf_texts(page: &Page) -> Vec<LeafText> {
    let mut out = Vec::new();
    let mut push = |tag: Tag, leaf: &Leaf| {
        let mut spans = Vec::new();
        let text = flatten(leaf, &mut spans);
        out.push(LeafText { tag, text, spans });
    };
    let mut blocks = |blocks: &[Block]| {
        for b in blocks {
            work::add(1);
            match b {
                Block::H1(l) => push(Tag::H1, l),
                Block::H2(l) => push(Tag::H2, l),
                Block::H3(l) => push(Tag::H3, l),
                Block::P(l) => push(Tag::P, l),
                Block::Address(l) => push(Tag::Address, l),
                Block::Ul(list) | Block::Ol(list) => {
                    for li in &list.items {
                        push(Tag::Li, li);
                    }
                }
            }
        }
    };
    if let Some(header) = page.header() {
        blocks(&header.blocks);
    }
    for item in &page.main().items {
        match item {
            MainItem::Section(s) => blocks(&s.blocks),
            MainItem::Block(b) => blocks(std::slice::from_ref(b)),
        }
    }
    if let Some(footer) = page.footer() {
        blocks(&footer.blocks);
    }
    out
}

/// The result of every facts check on one page (M3 §5.6): FC-1's heading
/// and title, FC-2 for each hours line in order, FC-3 and FC-4, each
/// evaluated even after an earlier one failed. Booleans only, so its
/// `Debug` shows no content.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FactsReport {
    /// FC-1: exactly one `h1`, whose text is the name.
    pub name_heading: bool,
    /// FC-1: the decoded title is the name.
    pub name_title: bool,
    /// FC-2: whether each hours line is the text of some `li` or `p`.
    pub hours: Vec<bool>,
    /// FC-3: the phone is the text of some leaf or span.
    pub phone: bool,
    /// FC-4: no phone-like token has other digits than the phone's.
    pub no_foreign_number: bool,
}

impl FactsReport {
    /// The first failing check in [`check_facts`]' order (FC-1 heading,
    /// FC-1 title, FC-2 by index, FC-3, FC-4), or `Ok` if every one passed.
    pub fn first_error(&self) -> Result<(), FactCode> {
        if !self.name_heading {
            return Err(FactCode::NameNotHeading);
        }
        if !self.name_title {
            return Err(FactCode::NameNotTitle);
        }
        if let Some(i) = self.hours.iter().position(|ok| !ok) {
            let index = u8::try_from(i).unwrap_or(u8::MAX);
            return Err(FactCode::HoursLineMissing { index });
        }
        if !self.phone {
            return Err(FactCode::PhoneMissing);
        }
        if !self.no_foreign_number {
            return Err(FactCode::ForeignNumber);
        }
        Ok(())
    }
}

/// Every facts check on `page`, each evaluated in full (M3 §5.6): an
/// earlier failure never skips a later check, and every hours line is
/// checked. [`check_facts`] is its first error.
pub fn facts_report(page: &Page, facts: &ApprovedFacts) -> FactsReport {
    let leaves = leaf_texts(page);

    // FC-1: exactly one h1, whose text is the name; the title is the name.
    let mut h1 = leaves.iter().filter(|l| l.tag == Tag::H1);
    let name_heading = matches!(
        (h1.next(), h1.next()),
        (Some(only), None) if same(&only.text, &facts.name)
    );
    let name_title = same(page.title(), &facts.name);

    // FC-2: each hours line is the text of some li or p.
    let hours = facts
        .hours
        .iter()
        .map(|line| {
            leaves
                .iter()
                .any(|l| matches!(l.tag, Tag::Li | Tag::P) && same(&l.text, line))
        })
        .collect();

    // FC-3: the phone is the text of some leaf or span.
    let phone = leaves.iter().any(|l| {
        same(&l.text, &facts.phone)
            || l.spans
                .iter()
                .any(|&(a, b)| same(&l.text[a..b], &facts.phone))
    });

    // FC-4: every phone-like token has the approved phone's digits.
    let digits: String = facts.phone.chars().filter(char::is_ascii_digit).collect();
    let no_foreign_number = std::iter::once(page.title())
        .chain(leaves.iter().map(|l| l.text.as_str()))
        .all(|text| phone_like_tokens(text).iter().all(|d| same(d, &digits)));

    FactsReport {
        name_heading,
        name_title,
        hours,
        phone,
        no_foreign_number,
    }
}

/// FC-1 to FC-4, in that order; the first missing fact is reported. It is
/// [`facts_report`]'s first error.
pub fn check_facts(page: &Page, facts: &ApprovedFacts) -> Result<(), FactCode> {
    facts_report(page, facts).first_error()
}
