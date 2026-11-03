//! M3-A: Mother's deterministic page Verifier (`m3-design-approved.md` §5),
//! over values only. It reads its arguments and nothing else: no file,
//! environment, process, clock, lock or network. No model is called, and
//! nothing is judged by a model. The lane that calls it, its input and its
//! journal records are M3-D's.
//!
//! The checks run in a fixed order. V0 to V2 gate the rest; V3 to V5 then
//! always run in full, so the per-fact results are complete.
//!
//! | ID | Check | Fail code |
//! |---|---|---|
//! | V0 | the one File effect was approved and its receipt is journaled | `no_publication` |
//! | V1 | the candidate's SHA-256 and size equal the receipt's | `published_mismatch` |
//! | V2 | S1a's `check` accepts the candidate, and Mother's policy hash equals the receipt's | `page_not_inert:<s1a code>`, `policy_changed` |
//! | V3 | S1a's facts report: FC-1 heading and title, FC-2 per hours line, FC-3, FC-4 | `name_not_heading`, `name_not_title`, `hours_line_missing:<i>`, `phone_missing`, `foreign_number` |
//! | V4 | the second reader: the name is the only `h1` line and the title; each hours line is a whole `li` or `p` line; the phone is in some line with no ASCII digit directly before or after it | `name_not_visible`, `hours_line_not_visible:<i>`, `phone_not_visible`, `reader_unsupported` |
//! | V5 | [`email_scan`]: no email-shaped token in any line or the title | `email_not_supported` |
//!
//! An hours index `<i>` counts from 0, as S1a's does. The outcome is a pass
//! only if all six checks pass; otherwise it is the first failing code in
//! table order.
//!
//! **The second reader's independence is limited, and stated.**
//! [`read_visible_text`] is a separate implementation of text extraction
//! within S1a's accepted canonical language, and no more. It runs only
//! after V2 has accepted the bytes, and relies on that structural
//! acceptance and on the argument that Mother's fixed CSS never hides text
//! (an argument from the CSS, not a rendering). It does not check
//! inertness, rendering or FC-4. It catches a disagreement between two
//! extractions (title decoding, entities, span joining, the space between
//! spans, `br`); it does not catch a misreading of the grammar that both
//! share. Syntax it does not know fails closed (`reader_unsupported`): it
//! is never deleted as decoration.
//!
//! **Not checked:** how the page looks in a browser (375 px and 1366 px
//! stay OPEN); whether its other sentences are true; a number written in
//! words or split across blocks; an address that is not email-shaped.
//! Every report holds codes and booleans only, never page or facts text.

use crate::lane::Verdict;
use nexus_governed_control::inert_page::{
    check, facts_report, policy_hash, ApprovedFacts, Block, FactCode, FactsReport, Leaf, MainItem,
    Page, RefusalCode,
};
use sha2::{Digest, Sha256};
use std::fmt;

/// The broker's receipt for the mission's one publication, as Mother
/// journaled it (M3-D fills it): digests and a size, never content.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Receipt {
    /// The SHA-256 of the published bytes.
    pub sha256: [u8; 32],
    /// Their size in bytes.
    pub bytes: u64,
    /// The policy hash of the kernel's inert verdict on them.
    pub policy_hash: [u8; 32],
}

/// The six checks, in their fixed order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CheckId {
    V0,
    V1,
    V2,
    V3,
    V4,
    V5,
}

impl CheckId {
    /// Every check, in order.
    pub const ALL: [CheckId; 6] = [
        CheckId::V0,
        CheckId::V1,
        CheckId::V2,
        CheckId::V3,
        CheckId::V4,
        CheckId::V5,
    ];

    /// The check's stable name.
    pub const fn as_str(self) -> &'static str {
        match self {
            CheckId::V0 => "v0",
            CheckId::V1 => "v1",
            CheckId::V2 => "v2",
            CheckId::V3 => "v3",
            CheckId::V4 => "v4",
            CheckId::V5 => "v5",
        }
    }
}

/// Why a check failed: a closed code, whose only detail is an hours index
/// or an S1a code.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CheckCode {
    /// V0: no approved publication with a receipt.
    NoPublication,
    /// V1: the candidate is not the published file.
    PublishedMismatch,
    /// V2: S1a refused the candidate with this code.
    PageNotInert(RefusalCode),
    /// V2: Mother's policy hash differs from the receipt's.
    PolicyChanged,
    /// V3: S1a's first missing fact (only its page codes occur here).
    FactMissing(FactCode),
    /// V4: the name is not the only `h1` line and the title.
    NameNotVisible,
    /// V4: this hours line (from 0) is no whole `li` or `p` line.
    HoursLineNotVisible(u8),
    /// V4: no line shows the phone unglued from other digits.
    PhoneNotVisible,
    /// V4: the reader met syntax it does not know.
    ReaderUnsupported,
    /// V5: an email-shaped token is in a line or the title.
    EmailNotSupported,
}

impl fmt::Display for CheckCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CheckCode::NoPublication => f.write_str("no_publication"),
            CheckCode::PublishedMismatch => f.write_str("published_mismatch"),
            CheckCode::PageNotInert(code) => write!(f, "page_not_inert:{}", code.as_str()),
            CheckCode::PolicyChanged => f.write_str("policy_changed"),
            CheckCode::FactMissing(FactCode::HoursLineMissing { index }) => {
                write!(f, "hours_line_missing:{index}")
            }
            CheckCode::FactMissing(code) => f.write_str(code.as_str()),
            CheckCode::NameNotVisible => f.write_str("name_not_visible"),
            CheckCode::HoursLineNotVisible(index) => write!(f, "hours_line_not_visible:{index}"),
            CheckCode::PhoneNotVisible => f.write_str("phone_not_visible"),
            CheckCode::ReaderUnsupported => f.write_str("reader_unsupported"),
            CheckCode::EmailNotSupported => f.write_str("email_not_supported"),
        }
    }
}

/// One check's result.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CheckResult {
    Pass,
    Fail(CheckCode),
    /// An earlier gate (V0, V1 or V2) failed.
    NotRun,
}

impl CheckResult {
    fn of(code: Option<CheckCode>) -> CheckResult {
        code.map_or(CheckResult::Pass, CheckResult::Fail)
    }
}

impl fmt::Display for CheckResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CheckResult::Pass => f.write_str("pass"),
            CheckResult::Fail(code) => write!(f, "fail:{code}"),
            CheckResult::NotRun => f.write_str("not_run"),
        }
    }
}

/// One fact's result: the deterministic "D" interface for typed
/// decisions T4.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FactResult {
    /// Its V3 part and its V4 part both passed.
    Pass,
    /// One of them failed.
    Fail,
    /// V0, V1 or V2 failed, so neither ran (`not_run`).
    NotRun,
}

impl FactResult {
    /// The result's stable name.
    pub const fn as_str(self) -> &'static str {
        match self {
            FactResult::Pass => "pass",
            FactResult::Fail => "fail",
            FactResult::NotRun => "not_run",
        }
    }

    fn of(passed: bool) -> FactResult {
        if passed {
            FactResult::Pass
        } else {
            FactResult::Fail
        }
    }
}

/// The per-fact results. The phone's V3 part is FC-3 and FC-4 together, so
/// a foreign phone-like token fails the phone.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FactResults {
    pub name: FactResult,
    pub hours: FactResult,
    pub phone: FactResult,
}

/// V4's findings for each fact: booleans only.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VisibleReport {
    /// The reader read the page; if not, every other field is false.
    pub supported: bool,
    /// The name is the only `h1` line and the title.
    pub name: bool,
    /// Each hours line is a whole `li` or `p` line.
    pub hours: Vec<bool>,
    /// Some line shows the phone with no ASCII digit directly before or
    /// after it.
    pub phone: bool,
}

impl VisibleReport {
    /// The first failing V4 code: an unread page first, then the facts in
    /// table order.
    fn first_error(&self) -> Option<CheckCode> {
        if !self.supported {
            return Some(CheckCode::ReaderUnsupported);
        }
        if !self.name {
            return Some(CheckCode::NameNotVisible);
        }
        if let Some(i) = self.hours.iter().position(|ok| !ok) {
            let index = u8::try_from(i).unwrap_or(u8::MAX);
            return Some(CheckCode::HoursLineNotVisible(index));
        }
        (!self.phone).then_some(CheckCode::PhoneNotVisible)
    }
}

/// The Verifier's report: each check's result, and the V3 and V4 findings
/// behind the per-fact results. Codes and booleans only.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerifierReport {
    checks: [CheckResult; 6],
    facts_report: Option<FactsReport>,
    visible: Option<VisibleReport>,
}

impl VerifierReport {
    /// The result of `id`.
    pub fn check(&self, id: CheckId) -> CheckResult {
        self.checks[id as usize]
    }

    /// Every check's result, in order.
    pub fn checks(&self) -> [(CheckId, CheckResult); 6] {
        CheckId::ALL.map(|id| (id, self.check(id)))
    }

    /// V3's findings, if it ran.
    pub fn facts_report(&self) -> Option<&FactsReport> {
        self.facts_report.as_ref()
    }

    /// V4's findings, if it ran.
    pub fn visible_report(&self) -> Option<&VisibleReport> {
        self.visible.as_ref()
    }

    /// The per-fact results: each passes only if its V3 part and its V4
    /// part both passed.
    pub fn facts(&self) -> FactResults {
        let (Some(v3), Some(v4)) = (&self.facts_report, &self.visible) else {
            return FactResults {
                name: FactResult::NotRun,
                hours: FactResult::NotRun,
                phone: FactResult::NotRun,
            };
        };
        let all = |lines: &[bool]| lines.iter().all(|ok| *ok);
        FactResults {
            name: FactResult::of(v3.name_heading && v3.name_title && v4.name),
            hours: FactResult::of(all(&v3.hours) && all(&v4.hours)),
            phone: FactResult::of(v3.phone && v3.no_foreign_number && v4.phone),
        }
    }

    /// The first failing code in table order, if any.
    pub fn first_failure(&self) -> Option<CheckCode> {
        self.checks.iter().find_map(|c| match c {
            CheckResult::Fail(code) => Some(*code),
            CheckResult::Pass | CheckResult::NotRun => None,
        })
    }

    /// True only if all six checks passed.
    pub fn passed(&self) -> bool {
        self.checks.iter().all(|c| *c == CheckResult::Pass)
    }

    /// The lane's verdict: `Pass` only if all six checks passed; otherwise
    /// `Fail` with the first failing code as its reason.
    pub fn verdict(&self) -> Verdict {
        if self.passed() {
            return Verdict::Pass;
        }
        let reason = self
            .first_failure()
            .map_or_else(|| "not_run".to_string(), |code| code.to_string());
        Verdict::Fail { reason }
    }
}

impl fmt::Display for VerifierReport {
    /// `v0 pass, …, v5 fail:<code>; name pass, hours pass, phone fail`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (k, (id, result)) in self.checks().iter().enumerate() {
            let sep = if k == 0 { "" } else { ", " };
            write!(f, "{sep}{} {result}", id.as_str())?;
        }
        let facts = self.facts();
        write!(
            f,
            "; name {}, hours {}, phone {}",
            facts.name.as_str(),
            facts.hours.as_str(),
            facts.phone.as_str()
        )
    }
}

/// Checks `candidate` against `receipt` and `facts`: V0 to V5 (see the
/// module documentation).
pub fn verify_page(
    candidate: &[u8],
    facts: &ApprovedFacts,
    receipt: Option<&Receipt>,
) -> VerifierReport {
    run(candidate, facts, receipt, Hooks::NONE)
}

/// [`verify_page`] with M3-O5's test seams.
#[cfg(test)]
pub(crate) fn verify_page_hooked(
    candidate: &[u8],
    facts: &ApprovedFacts,
    receipt: Option<&Receipt>,
    hooks: Hooks,
) -> VerifierReport {
    run(candidate, facts, receipt, hooks)
}

/// M3-O5's two test seams: V3's or V4's findings forced to pass. Only the
/// test-build [`verify_page_hooked`] can set them; [`verify_page`] never
/// does.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Hooks {
    pub(crate) v3_pass: bool,
    pub(crate) v4_pass: bool,
}

impl Hooks {
    pub(crate) const NONE: Hooks = Hooks {
        v3_pass: false,
        v4_pass: false,
    };
}

/// V0 to V2 in order, each a gate; the page S1a accepted, or the report of
/// the first failing gate.
fn gates(candidate: &[u8], receipt: Option<&Receipt>) -> Result<Page, VerifierReport> {
    let failed = |k: usize, code: CheckCode| {
        let mut checks = [CheckResult::NotRun; 6];
        checks[..k].fill(CheckResult::Pass);
        checks[k] = CheckResult::Fail(code);
        VerifierReport {
            checks,
            facts_report: None,
            visible: None,
        }
    };
    let Some(receipt) = receipt else {
        return Err(failed(0, CheckCode::NoPublication));
    };
    let sha256: [u8; 32] = Sha256::digest(candidate).into();
    let bytes = u64::try_from(candidate.len()).unwrap_or(u64::MAX);
    if sha256 != receipt.sha256 || bytes != receipt.bytes {
        return Err(failed(1, CheckCode::PublishedMismatch));
    }
    let page = check(candidate).map_err(|r| failed(2, CheckCode::PageNotInert(r.code)))?;
    if policy_hash() != receipt.policy_hash {
        return Err(failed(2, CheckCode::PolicyChanged));
    }
    Ok(page)
}

fn run(
    candidate: &[u8],
    facts: &ApprovedFacts,
    receipt: Option<&Receipt>,
    hooks: Hooks,
) -> VerifierReport {
    let page = match gates(candidate, receipt) {
        Ok(page) => page,
        Err(report) => return report,
    };
    let lines = facts.hours().len();

    // V3: S1a's facts report, every check evaluated.
    let v3 = if hooks.v3_pass {
        FactsReport {
            name_heading: true,
            name_title: true,
            hours: vec![true; lines],
            phone: true,
            no_foreign_number: true,
        }
    } else {
        facts_report(&page, facts)
    };

    // V4: the second reader, over the bytes themselves.
    let v4 = if hooks.v4_pass {
        VisibleReport {
            supported: true,
            name: true,
            hours: vec![true; lines],
            phone: true,
        }
    } else {
        match read_visible_text(candidate) {
            Ok(text) => visible_facts(&text, facts),
            Err(ReaderUnsupported) => VisibleReport {
                supported: false,
                name: false,
                hours: vec![false; lines],
                phone: false,
            },
        }
    };

    // V5: no email-shaped token.
    let v5 = email_scan(&page).map(|EmailShaped| CheckCode::EmailNotSupported);

    let pass = CheckResult::Pass;
    VerifierReport {
        checks: [
            pass,
            pass,
            pass,
            CheckResult::of(v3.first_error().err().map(CheckCode::FactMissing)),
            CheckResult::of(v4.first_error()),
            CheckResult::of(v5),
        ],
        facts_report: Some(v3),
        visible: Some(v4),
    }
}

// ------------------------------------------------------------ the reader

/// The leaf tags of S1a's grammar: the lines the reader takes text from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LeafTag {
    H1,
    H2,
    H3,
    P,
    Li,
    Address,
}

impl LeafTag {
    const ALL: [LeafTag; 6] = [
        LeafTag::H1,
        LeafTag::H2,
        LeafTag::H3,
        LeafTag::P,
        LeafTag::Li,
        LeafTag::Address,
    ];

    /// The tag's name.
    pub const fn name(self) -> &'static str {
        match self {
            LeafTag::H1 => "h1",
            LeafTag::H2 => "h2",
            LeafTag::H3 => "h3",
            LeafTag::P => "p",
            LeafTag::Li => "li",
            LeafTag::Address => "address",
        }
    }
}

/// One leaf line's visible text. Its `Debug` shows the tag and a length
/// only.
#[derive(Clone, PartialEq, Eq)]
pub struct VisibleLine {
    pub tag: LeafTag,
    pub text: String,
}

impl fmt::Debug for VisibleLine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VisibleLine")
            .field("tag", &self.tag)
            .field("chars", &self.text.chars().count())
            .finish()
    }
}

/// The page's visible text as the second reader extracted it: the title,
/// then every leaf line in order. Its `Debug` shows lengths only.
#[derive(Clone, PartialEq, Eq)]
pub struct VisibleText {
    pub title: String,
    pub lines: Vec<VisibleLine>,
}

impl fmt::Debug for VisibleText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VisibleText")
            .field("title_chars", &self.title.chars().count())
            .field("lines", &self.lines)
            .finish()
    }
}

/// The reader met syntax it does not know (`reader_unsupported`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReaderUnsupported;

/// The container tags: a line holding exactly one open or close of one of
/// these carries no text.
const CONTAINERS: [&str; 6] = ["header", "main", "section", "ul", "ol", "footer"];
/// The spans allowed inside a leaf line, beside `br`.
const SPANS: [&str; 3] = ["strong", "em", "small"];
/// The five entities, each decoded to its character.
const ENTITIES: [(&str, char); 5] = [
    ("&amp;", '&'),
    ("&lt;", '<'),
    ("&gt;", '>'),
    ("&quot;", '"'),
    ("&#39;", '\''),
];

/// The second reader (design M3 §5.3; see the module documentation for
/// its limited independence):
/// 1. the title is the text between the first `<title>` and the next
///    `</title>`;
/// 2. the body is the text between `<body>` and `</body>`, each followed
///    by a line feed, split at line feeds; each line is exactly a
///    container open or close, or a leaf line `<t …>…</t>` for a leaf tag
///    `t`;
/// 3. inside a leaf line the only tags are `strong`, `em` and `small`
///    (each with at most a `class` and an `id`) and a bare `<br>`;
/// 4. each `<br>` becomes one space, the span tags are removed, and the
///    five entities are decoded in one left-to-right pass.
///
/// Anything else is [`ReaderUnsupported`].
pub fn read_visible_text(candidate: &[u8]) -> Result<VisibleText, ReaderUnsupported> {
    let text = std::str::from_utf8(candidate).map_err(|_| ReaderUnsupported)?;
    let title = between(text, "<title>", "</title>").ok_or(ReaderUnsupported)?;
    let title = decode(title)?;
    let body = between(text, "<body>\n", "</body>\n").ok_or(ReaderUnsupported)?;
    let mut lines = Vec::new();
    if !body.is_empty() {
        let body = body.strip_suffix('\n').ok_or(ReaderUnsupported)?;
        for line in body.split('\n') {
            if let Some(leaf) = read_line(line)? {
                lines.push(leaf);
            }
        }
    }
    Ok(VisibleText { title, lines })
}

/// The text between the first `open` and the next `close` after it.
fn between<'a>(text: &'a str, open: &str, close: &str) -> Option<&'a str> {
    let start = text.find(open)? + open.len();
    let len = text[start..].find(close)?;
    Some(&text[start..start + len])
}

/// One tag at the start of a text.
enum TagAt<'a> {
    /// `<name …>`, and whether it had attributes.
    Open(&'a str, bool),
    /// `</name>`.
    Close(&'a str),
}

/// A tag name: lower-case ASCII letters and digits.
fn is_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
}

/// The tag at the start of `s`, and the rest of `s` after it. Attributes
/// are ` class="…"` and ` id="…"`, each at most once, with values of
/// lower-case ASCII letters, digits and `-`.
fn tag_at(s: &str) -> Result<(TagAt<'_>, &str), ReaderUnsupported> {
    let s = s.strip_prefix('<').ok_or(ReaderUnsupported)?;
    if let Some(rest) = s.strip_prefix('/') {
        let end = rest.find('>').ok_or(ReaderUnsupported)?;
        let name = &rest[..end];
        if !is_name(name) {
            return Err(ReaderUnsupported);
        }
        return Ok((TagAt::Close(name), &rest[end + 1..]));
    }
    let end = s.find([' ', '>']).ok_or(ReaderUnsupported)?;
    let name = &s[..end];
    if !is_name(name) {
        return Err(ReaderUnsupported);
    }
    let mut rest = &s[end..];
    let mut seen = [false; 2];
    loop {
        if let Some(after) = rest.strip_prefix('>') {
            return Ok((TagAt::Open(name, seen != [false; 2]), after));
        }
        let attribute = rest.strip_prefix(' ').ok_or(ReaderUnsupported)?;
        let (k, value) = if let Some(v) = attribute.strip_prefix("class=\"") {
            (0, v)
        } else if let Some(v) = attribute.strip_prefix("id=\"") {
            (1, v)
        } else {
            return Err(ReaderUnsupported);
        };
        let end = value.find('"').ok_or(ReaderUnsupported)?;
        let ok = value[..end]
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
        if seen[k] || end == 0 || !ok {
            return Err(ReaderUnsupported);
        }
        seen[k] = true;
        rest = &value[end + 1..];
    }
}

/// One body line: `None` for a container line, the visible text of a
/// leaf line.
fn read_line(line: &str) -> Result<Option<VisibleLine>, ReaderUnsupported> {
    let (tag, rest) = tag_at(line)?;
    let name = match tag {
        TagAt::Open(name, _) | TagAt::Close(name) if CONTAINERS.contains(&name) => {
            return if rest.is_empty() {
                Ok(None)
            } else {
                Err(ReaderUnsupported)
            };
        }
        TagAt::Open(name, _) => name,
        TagAt::Close(_) => return Err(ReaderUnsupported),
    };
    let tag = LeafTag::ALL
        .into_iter()
        .find(|t| t.name() == name)
        .ok_or(ReaderUnsupported)?;
    let content = rest
        .strip_suffix(">")
        .and_then(|r| r.strip_suffix(name))
        .and_then(|r| r.strip_suffix("</"))
        .ok_or(ReaderUnsupported)?;
    let text = inline_text(content)?;
    Ok(Some(VisibleLine { tag, text }))
}

/// A leaf line's content: the span tags removed, each `<br>` one space,
/// the text decoded.
fn inline_text(content: &str) -> Result<String, ReaderUnsupported> {
    let mut out = String::new();
    let mut rest = content;
    while !rest.is_empty() {
        if rest.starts_with('<') {
            let (tag, after) = tag_at(rest)?;
            match tag {
                TagAt::Open("br", false) => out.push(' '),
                TagAt::Open(name, _) | TagAt::Close(name) if SPANS.contains(&name) => {}
                TagAt::Open(..) | TagAt::Close(_) => return Err(ReaderUnsupported),
            }
            rest = after;
        } else {
            let end = rest.find('<').unwrap_or(rest.len());
            out.push_str(&decode(&rest[..end])?);
            rest = &rest[end..];
        }
    }
    Ok(out)
}

/// Text with the five entities decoded in one left-to-right pass. Any other
/// `&`, and any `<` or `>`, is unsupported.
fn decode(text: &str) -> Result<String, ReaderUnsupported> {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(k) = rest.find(['&', '<', '>']) {
        out.push_str(&rest[..k]);
        let tail = &rest[k..];
        let (entity, c) = ENTITIES
            .iter()
            .find(|(entity, _)| tail.starts_with(entity))
            .ok_or(ReaderUnsupported)?;
        out.push(*c);
        rest = &tail[entity.len()..];
    }
    out.push_str(rest);
    Ok(out)
}

/// V4 over the reader's text: each fact's visibility.
pub fn visible_facts(text: &VisibleText, facts: &ApprovedFacts) -> VisibleReport {
    let mut h1 = text.lines.iter().filter(|l| l.tag == LeafTag::H1);
    let name = matches!(
        (h1.next(), h1.next()),
        (Some(only), None) if only.text == facts.name()
    ) && text.title == facts.name();
    let hours = facts
        .hours()
        .iter()
        .map(|line| {
            text.lines
                .iter()
                .any(|l| matches!(l.tag, LeafTag::Li | LeafTag::P) && l.text == *line)
        })
        .collect();
    let phone = text
        .lines
        .iter()
        .any(|l| shows_unglued(&l.text, facts.phone()));
    VisibleReport {
        supported: true,
        name,
        hours,
        phone,
    }
}

/// Whether `text` holds `needle` at some place with no ASCII digit
/// directly before or after it. Every place is tried, overlapping ones
/// included.
fn shows_unglued(text: &str, needle: &str) -> bool {
    let digit = |c: Option<char>| c.is_some_and(|c| c.is_ascii_digit());
    text.char_indices().any(|(at, _)| {
        text[at..].starts_with(needle)
            && !digit(text[..at].chars().next_back())
            && !digit(text[at + needle.len()..].chars().next())
    })
}

// ------------------------------------------------------------ the email scan

/// [`email_scan`] found an email-shaped token.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EmailShaped;

/// V5, shared with the page Builder (M3-C), which runs it before asking
/// for the file: an email-shaped token (`local@domain.tld`, scanned
/// without a regex) in the title or any leaf's text, spans joined.
///
/// Around each `@`: the local part is the run of letters, digits and
/// ``.!#$%&'*+/=?^_`{|}~-`` directly before it, and the domain the run of
/// letters, digits, `-` and `.` directly after it, without trailing `.`
/// or `-`. A token is email-shaped if the local part is not empty and the
/// domain has a non-empty part before its last `.` and, after it, at least
/// two letters. Not detected: an address split across blocks or by a
/// `br`, written with words (`at`, `dot`), or with a numeric or one-letter
/// last part.
pub fn email_scan(page: &Page) -> Option<EmailShaped> {
    let title = std::iter::once(page.title().to_string());
    title
        .chain(leaves(page).into_iter().map(Leaf::text))
        .any(|text| has_email(&text))
        .then_some(EmailShaped)
}

/// Every leaf of `page`, in document order.
fn leaves(page: &Page) -> Vec<&Leaf> {
    fn blocks<'a>(bs: &'a [Block], out: &mut Vec<&'a Leaf>) {
        for b in bs {
            match b {
                Block::H1(l) | Block::H2(l) | Block::H3(l) | Block::P(l) | Block::Address(l) => {
                    out.push(l)
                }
                Block::Ul(list) | Block::Ol(list) => out.extend(&list.items),
            }
        }
    }
    let mut out = Vec::new();
    if let Some(header) = page.header() {
        blocks(&header.blocks, &mut out);
    }
    for item in &page.main().items {
        match item {
            MainItem::Section(s) => blocks(&s.blocks, &mut out),
            MainItem::Block(b) => blocks(std::slice::from_ref(b), &mut out),
        }
    }
    if let Some(footer) = page.footer() {
        blocks(&footer.blocks, &mut out);
    }
    out
}

fn has_email(text: &str) -> bool {
    let local = |c: char| c.is_alphanumeric() || "!#$%&'*+-./=?^_`{|}~".contains(c);
    let domain = |c: char| c.is_alphanumeric() || c == '-' || c == '.';
    let chars: Vec<char> = text.chars().collect();
    chars.iter().enumerate().any(|(at, c)| {
        if *c != '@' || at == 0 || !local(chars[at - 1]) {
            return false;
        }
        let run: Vec<char> = chars[at + 1..]
            .iter()
            .copied()
            .take_while(|c| domain(*c))
            .collect();
        let mut name: &[char] = &run;
        while let [rest @ .., '.' | '-'] = name {
            name = rest;
        }
        let Some(dot) = name.iter().rposition(|c| *c == '.') else {
            return false;
        };
        let last = &name[dot + 1..];
        dot > 0 && last.len() >= 2 && last.iter().all(|c| c.is_alphabetic())
    })
}
