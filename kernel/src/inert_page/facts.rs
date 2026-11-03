//! The approved facts (for M3) and the facts check, FC-1 to FC-4 (design v2
//! §5). SKELETON: it refuses every fact set and every page.

use super::tree::{Leaf, Page};

/// Why approved facts could not be built, or which fact a page lacks. A
/// code (and, for an hours line, its 0-based index) only, never content.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FactCode {
    FactsInvalid,
    PhoneFormat,
    FactPhoneLike,
    NameNotHeading,
    NameNotTitle,
    HoursLineMissing { index: u8 },
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

/// The facts the Owner approved for a page.
#[derive(Clone, PartialEq, Eq)]
#[allow(dead_code)] // skeleton
pub struct ApprovedFacts {
    name: String,
    hours: Vec<String>,
    phone: String,
    hash: [u8; 32],
}

impl ApprovedFacts {
    pub fn new(_name: &str, _hours: &[&str], _phone: &str) -> Result<Self, FactCode> {
        Err(FactCode::FactsInvalid)
    }

    pub fn hash(&self) -> [u8; 32] {
        self.hash
    }

    #[allow(dead_code)] // skeleton
    pub(super) fn hours_lines(&self) -> usize {
        self.hours.len()
    }
}

impl std::fmt::Debug for ApprovedFacts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApprovedFacts")
            .field("hash", &hex::encode(self.hash))
            .finish()
    }
}

pub fn check_facts(_page: &Page, _facts: &ApprovedFacts) -> Result<(), FactCode> {
    Err(FactCode::PhoneMissing)
}

impl Leaf {
    pub fn text(&self) -> String {
        String::new()
    }
}
