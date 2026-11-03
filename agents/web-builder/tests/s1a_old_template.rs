//! S-O8 (design v2 §8): the old `local_business` template (sha256 pinned to
//! the design's F11) is refused by the inert page check, and its lexical
//! findings include `remote_reference`, `script_element`, `comment` and
//! `element_not_allowed`. The same holds after every `{{…}}` placeholder is
//! filled with the bakery's facts, checked with those facts: the inert check
//! refuses it before any fact is looked at.

use nexus_kernel::inert_page::*;
use sha2::{Digest, Sha256};
use web_builder_agent::templates::get_template;

const OLD_SHA256: &str = "5f29b79a0f6f7df3d1831de3d5343c29ab4e8544b3808191e765d291a8b8387f";
const OLD_BYTES: usize = 38_533;

const NAME: &str = "Rosie's Bakery";
const HOURS: [&str; 3] = [
    "Monday to Friday: 07:00 – 17:00",
    "Saturday: 08:00 – 14:00",
    "Sunday: closed",
];
const PHONE: &str = "01632 960123";

/// The bakery's value for a placeholder.
fn bakery(key: &str) -> &'static str {
    match key {
        "BUSINESS_NAME" | "FOOTER_BUSINESS_NAME" => NAME,
        "PHONE" | "HOURS_PHONE" => PHONE,
        "WEEKDAY_HOURS" => HOURS[0],
        "SATURDAY_HOURS" => HOURS[1],
        "SUNDAY_HOURS" => HOURS[2],
        "ADDRESS" | "HOURS_ADDRESS" => "12 Mill Lane, Exampleton",
        "TAGLINE" => "Fresh bread and cakes, baked every morning.",
        _ => "Fresh bread",
    }
}

/// `html` with every `{{KEY}}` replaced by the bakery's value.
fn filled(html: &str) -> String {
    let mut out = String::new();
    let mut rest = html;
    while let Some(start) = rest.find("{{") {
        let end = start + rest[start..].find("}}").expect("a closed placeholder");
        out.push_str(&rest[..start]);
        out.push_str(bakery(&rest[start + 2..end]));
        rest = &rest[end + 2..];
    }
    out.push_str(rest);
    out
}

#[test]
fn s_o8_the_old_template_is_refused_as_shipped_and_filled() {
    let shipped = get_template("local_business")
        .expect("the local_business template")
        .html;
    assert_eq!(hex::encode(Sha256::digest(shipped)), OLD_SHA256);
    assert_eq!(shipped.len(), OLD_BYTES);
    let bakery_page = filled(shipped);
    assert!(!bakery_page.contains("{{") && !bakery_page.contains("}}"));
    let facts = ApprovedFacts::new(NAME, &HOURS, PHONE).expect("the bakery's facts");

    for (label, page, facts) in [
        ("as shipped", shipped.to_string(), None),
        ("filled with the bakery's facts", bakery_page, Some(&facts)),
    ] {
        let refusal = check(page.as_bytes()).expect_err(label);
        let v = PageVerdict::evaluate(page.as_bytes(), facts);
        assert_eq!(v.outcome, PageOutcome::Refused(refusal.code), "{label}");
        assert!(!v.passed(), "{label}");
        assert_eq!(v.label(), POLICY_V1.label_refused_prefix, "{label}");
        assert_ne!(v.label(), LABEL_PASSED, "{label}");
        assert_eq!(v.facts_line(), None, "{label}");
        assert_eq!(v.facts, FactsMarker::of(facts), "{label}");
        let findings = lexical_findings(page.as_bytes());
        for code in [
            RefusalCode::RemoteReference,
            RefusalCode::ScriptElement,
            RefusalCode::Comment,
            RefusalCode::ElementNotAllowed,
        ] {
            assert!(
                findings.contains(code),
                "{label}: {} not among {findings:?}",
                code.as_str()
            );
        }
    }
}
