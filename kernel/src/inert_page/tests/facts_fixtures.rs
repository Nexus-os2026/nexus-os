//! Shared S1a-B fixtures: the reference page's approved facts (design v2
//! §5) and a few edits of the reference page.

use super::*;

pub(super) const NAME: &str = "Rosie's Bakery";
pub(super) const HOURS: [&str; 3] = [
    "Monday to Friday: 07:00 – 17:00",
    "Saturday: 08:00 – 14:00",
    "Sunday: closed",
];
pub(super) const PHONE: &str = "01632 960123";
/// A London spelling of a phone, for the design's `020 …` examples.
pub(super) const LONDON_PHONE: &str = "020 7946 0123";

/// The reference page's phone element.
pub(super) const PHONE_ELEMENT: &str = "<strong>01632 960123</strong>";

pub(super) fn facts(name: &str, hours: &[&str], phone: &str) -> ApprovedFacts {
    ApprovedFacts::new(name, hours, phone)
        .unwrap_or_else(|code| panic!("valid facts are refused: {code}"))
}

/// The reference page's facts.
pub(super) fn reference_facts() -> ApprovedFacts {
    facts(NAME, &HOURS, PHONE)
}

/// The reference facts with the London phone.
pub(super) fn london_facts() -> ApprovedFacts {
    facts(NAME, &HOURS, LONDON_PHONE)
}

/// The reference page showing the London phone.
pub(super) fn london_page() -> String {
    edit(PHONE_ELEMENT, "<strong>020 7946 0123</strong>")
}

/// `base` with one more paragraph at the end of the contact section.
pub(super) fn with_paragraph(base: &str, text: &str) -> String {
    edit_in(
        base,
        "</address>\n</section>\n",
        &format!("</address>\n<p>{text}</p>\n</section>\n"),
    )
}
