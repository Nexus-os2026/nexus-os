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
use nexus_governed_control::inert_page::{ApprovedFacts, FactCode, FactsReport, Page, RefusalCode};
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
        ""
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
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Ok(())
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

impl fmt::Display for CheckResult {
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Ok(())
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
        ""
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
        let _ = id;
        CheckResult::NotRun
    }

    /// Every check's result, in order.
    pub fn checks(&self) -> [(CheckId, CheckResult); 6] {
        CheckId::ALL.map(|id| (id, CheckResult::NotRun))
    }

    /// V3's findings, if it ran.
    pub fn facts_report(&self) -> Option<&FactsReport> {
        None
    }

    /// V4's findings, if it ran.
    pub fn visible_report(&self) -> Option<&VisibleReport> {
        None
    }

    /// The per-fact results.
    pub fn facts(&self) -> FactResults {
        FactResults {
            name: FactResult::NotRun,
            hours: FactResult::NotRun,
            phone: FactResult::NotRun,
        }
    }

    /// The first failing code in table order, if any.
    pub fn first_failure(&self) -> Option<CheckCode> {
        None
    }

    /// True only if all six checks passed.
    pub fn passed(&self) -> bool {
        false
    }

    /// The lane's verdict: `Pass` only if all six checks passed; otherwise
    /// `Fail` with the first failing code as its reason.
    pub fn verdict(&self) -> Verdict {
        Verdict::Fail {
            reason: String::new(),
        }
    }
}

impl fmt::Display for VerifierReport {
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Ok(())
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

fn run(
    candidate: &[u8],
    facts: &ApprovedFacts,
    receipt: Option<&Receipt>,
    hooks: Hooks,
) -> VerifierReport {
    let _ = (candidate, facts, receipt, hooks.v3_pass, hooks.v4_pass);
    VerifierReport {
        checks: [
            CheckResult::Fail(CheckCode::NoPublication),
            CheckResult::NotRun,
            CheckResult::NotRun,
            CheckResult::NotRun,
            CheckResult::NotRun,
            CheckResult::NotRun,
        ],
        facts_report: None,
        visible: None,
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
    /// The tag's name.
    pub const fn name(self) -> &'static str {
        ""
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

/// The second reader (design M3 §5.3; see the module documentation for
/// its limited independence).
pub fn read_visible_text(candidate: &[u8]) -> Result<VisibleText, ReaderUnsupported> {
    let _ = candidate;
    Err(ReaderUnsupported)
}

/// V4 over the reader's text: each fact's visibility.
pub fn visible_facts(text: &VisibleText, facts: &ApprovedFacts) -> VisibleReport {
    let _ = (text, facts);
    VisibleReport {
        supported: false,
        name: false,
        hours: Vec::new(),
        phone: false,
    }
}

// ------------------------------------------------------------ the email scan

/// [`email_scan`] found an email-shaped token.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EmailShaped;

/// V5, shared with the page Builder (M3-C), which runs it before asking
/// for the file: an email-shaped token (`local@domain.tld`, scanned
/// without a regex) in the title or any leaf's text.
pub fn email_scan(page: &Page) -> Option<EmailShaped> {
    let _ = page;
    None
}
