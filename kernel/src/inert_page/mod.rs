//! The inert page check (S1a): a pure validator for Mother's one fixed page
//! format. It never renders anything; a pass is always labelled exactly
//! [`LABEL_PASSED`].
//!
//! A candidate passes only if it is byte for byte the canonical form of the
//! small grammar declared in [`POLICY_V1`]: a fixed head that carries Mother's
//! own stylesheet ([`MOTHER_CSS`]) inline, then text in 16 tags. Four stages
//! run in order, and the first refusal is reported as a [`Refusal`] that holds
//! a closed code and numeric positions only, never content:
//! - S1, bytes and characters (size, UTF-8, controls, invisible characters,
//!   and a positive Latin profile);
//! - S2, lexical: the whole input is scanned and every finding is collected
//!   ([`lexical_findings`]);
//! - S3, a strict parse into the typed tree ([`Page`]);
//! - S4, canonical byte equality: `serialize(parse(input)) == input`.
//!
//! The module does no I/O and takes no clock or lock. Its time is linear in
//! the input, which is capped at `POLICY_V1.max_page_bytes`.
//!
//! Not claimed here: anything about how a browser lays the page out (the
//! 375 px and 1366 px views stay open), the truth of the prose, or the
//! approved facts (S1a-B).

#![forbid(unsafe_code)]

mod facts;
mod parse;
mod policy;
mod scan;
mod tree;
mod verdict;

#[cfg(test)]
mod tests;

pub use facts::{check_facts, ApprovedFacts, FactCode};
pub use policy::{policy_hash, PolicyParams, LABEL_PASSED, MOTHER_CSS, POLICY_V1};
pub use tree::{
    Attrs, Block, Class, Inline, Leaf, List, Main, MainItem, Page, Region, Section, Tag,
};
pub use verdict::{FactsMarker, PageOutcome, PageVerdict};

/// S-O15's deterministic work counter. In test builds every bounded step of
/// the check adds to a per-thread count; in other builds [`work::add`] is
/// empty and compiles away. It is a count, not a clock or a lock.
mod work {
    #[cfg(test)]
    thread_local! {
        static STEPS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    }

    /// Counts `n` steps.
    #[inline(always)]
    #[allow(dead_code)] // skeleton
    pub(super) fn add(n: usize) {
        #[cfg(test)]
        STEPS.with(|s| s.set(s.get().saturating_add(n as u64)));
        #[cfg(not(test))]
        let _ = n;
    }

    /// The steps counted on this thread since the last call, which resets
    /// the count.
    #[cfg(test)]
    pub(super) fn take() -> u64 {
        STEPS.with(|s| s.replace(0))
    }
}

macro_rules! refusal_codes {
    ($($variant:ident => $name:literal,)*) => {
        /// The closed set of refusal codes. Their order is the reporting
        /// priority (`POLICY_V1.refusal_codes`): S1, then S2 in its rule order,
        /// then S3, then S4. [`RefusalCode::position`] is the numeric code.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        #[repr(u16)]
        pub enum RefusalCode {
            $($variant,)*
        }

        impl RefusalCode {
            /// Every code, in reporting order.
            pub const CODES: [RefusalCode; 44] = [$(RefusalCode::$variant,)*];

            /// The code's stable name.
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(RefusalCode::$variant => $name,)*
                }
            }
        }
    };
}

refusal_codes! {
    TooLarge => "too_large",
    NotUtf8 => "not_utf8",
    ByteOrderMark => "byte_order_mark",
    CarriageReturn => "carriage_return",
    NoFinalNewline => "no_final_newline",
    ControlChar => "control_char",
    InvisibleChar => "invisible_char",
    CharNotAllowed => "char_not_allowed",
    ScriptElement => "script_element",
    StyleElement => "style_element",
    ElementNotAllowed => "element_not_allowed",
    TagCase => "tag_case",
    EventHandler => "event_handler",
    StyleAttribute => "style_attribute",
    UrlAttribute => "url_attribute",
    AttributeNotAllowed => "attribute_not_allowed",
    Comment => "comment",
    MarkupDeclaration => "markup_declaration",
    ProcessingInstruction => "processing_instruction",
    EntityNotAllowed => "entity_not_allowed",
    RemoteReference => "remote_reference",
    CssUrl => "css_url",
    CssImport => "css_import",
    JavascriptScheme => "javascript_scheme",
    DataScheme => "data_scheme",
    PrologueMismatch => "prologue_mismatch",
    LangInvalid => "lang_invalid",
    TitleInvalid => "title_invalid",
    CssMismatch => "css_mismatch",
    ContentModel => "content_model",
    TagUnclosed => "tag_unclosed",
    TagMisnested => "tag_misnested",
    VoidSyntax => "void_syntax",
    AttributeSyntax => "attribute_syntax",
    ClassNotAllowed => "class_not_allowed",
    IdInvalid => "id_invalid",
    IdDuplicate => "id_duplicate",
    EmptyElement => "empty_element",
    TextSpacing => "text_spacing",
    DepthExceeded => "depth_exceeded",
    TooManyElements => "too_many_elements",
    WordTooLong => "word_too_long",
    TrailingContent => "trailing_content",
    NotCanonical => "not_canonical",
}

impl RefusalCode {
    /// The numeric code: this code's index in [`RefusalCode::CODES`].
    pub const fn position(self) -> u16 {
        self as u16
    }
}

impl std::fmt::Display for RefusalCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Why a candidate was refused: a code and where, as numbers only. `offset`
/// is a byte offset; `line` and `column` count from 1, the column in
/// characters. For [`Page::new`] they point into the canonical serialization.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Refusal {
    pub code: RefusalCode,
    pub offset: u32,
    pub line: u32,
    pub column: u32,
}

impl Refusal {
    /// The refusal `code` at byte `offset` of `input`.
    pub(crate) fn at(code: RefusalCode, input: &[u8], offset: usize) -> Refusal {
        let before = &input[..offset.min(input.len())];
        let line_start = before
            .iter()
            .rposition(|&b| b == b'\n')
            .map_or(0, |k| k + 1);
        let line = 1 + before.iter().filter(|&&b| b == b'\n').count();
        let column = 1 + before[line_start..]
            .iter()
            .filter(|&&b| b & 0xC0 != 0x80)
            .count();
        let num = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
        Refusal {
            code,
            offset: num(offset),
            line: num(line),
            column: num(column),
        }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} at line {}, column {} (byte {})",
            self.code, self.line, self.column, self.offset
        )
    }
}

impl std::error::Error for Refusal {}

/// Every S1 and S2 finding of one input: which codes occur, and the first
/// byte offset of each. Numbers only.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct LexicalFindings {
    seen: u64,
    first: [u32; RefusalCode::CODES.len()],
}

impl Default for LexicalFindings {
    fn default() -> Self {
        LexicalFindings {
            seen: 0,
            first: [0; RefusalCode::CODES.len()],
        }
    }
}

impl LexicalFindings {
    pub(crate) fn note(&mut self, code: RefusalCode, offset: usize) {
        let i = usize::from(code.position());
        let offset = u32::try_from(offset).unwrap_or(u32::MAX);
        if self.seen & (1 << i) == 0 || offset < self.first[i] {
            self.first[i] = offset;
        }
        self.seen |= 1 << i;
    }

    pub fn contains(&self, code: RefusalCode) -> bool {
        self.seen & (1 << code.position()) != 0
    }

    pub fn first_offset(&self, code: RefusalCode) -> Option<u32> {
        self.contains(code)
            .then(|| self.first[usize::from(code.position())])
    }

    /// The codes found, in reporting order.
    pub fn codes(&self) -> impl Iterator<Item = RefusalCode> + '_ {
        POLICY_V1
            .refusal_codes
            .iter()
            .copied()
            .filter(|c| self.contains(*c))
    }

    pub fn is_empty(&self) -> bool {
        self.seen == 0
    }

    /// The finding that is reported: the first code in reporting order, at
    /// its first offset.
    pub fn first(&self) -> Option<(RefusalCode, u32)> {
        self.codes()
            .next()
            .map(|c| (c, self.first[usize::from(c.position())]))
    }
}

impl std::fmt::Debug for LexicalFindings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_map()
            .entries(
                self.codes()
                    .map(|c| (c.as_str(), self.first[usize::from(c.position())])),
            )
            .finish()
    }
}

/// S1 and S2 only: every finding of the whole input. An input over the size
/// cap reports `too_large` alone and is not scanned further.
pub fn lexical_findings(candidate: &[u8]) -> LexicalFindings {
    scan::scan(candidate)
}

/// The full check, S1 to S4. `Ok` holds the typed tree of an accepted page.
pub fn check(candidate: &[u8]) -> Result<Page, Refusal> {
    if let Some((code, offset)) = scan::scan(candidate).first() {
        return Err(Refusal::at(code, candidate, offset as usize));
    }
    // S1 passed, so the input is UTF-8.
    let text = std::str::from_utf8(candidate)
        .map_err(|e| Refusal::at(RefusalCode::NotUtf8, candidate, e.valid_up_to()))?;
    let page =
        parse::parse_document(text).map_err(|(code, at)| Refusal::at(code, candidate, at))?;
    let canonical = serialize(&page);
    if canonical != candidate {
        let same = canonical
            .iter()
            .zip(candidate)
            .take_while(|(a, b)| a == b)
            .count();
        return Err(Refusal::at(RefusalCode::NotCanonical, candidate, same));
    }
    Ok(page)
}

/// The canonical bytes of `page`.
pub fn serialize(page: &Page) -> Vec<u8> {
    tree::write_page(page, usize::MAX)
}
