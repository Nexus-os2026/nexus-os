//! S-O10: the facts check FC-1 to FC-4 and the facts' construction (design
//! v2 §5, frozen). Positives pass; each page negative gives exactly its
//! fact code; each construction negative is refused with exactly its code.
//! The inert label is unchanged in every case: a missing fact never changes
//! it, and a facts pass adds only the separate facts line.
//!
//! Also: the facts hash follows the declared encoding, `ApprovedFacts`'
//! `Debug` shows only that hash, and `ApprovedFacts` has no `Display` and no
//! `Serialize` (a compile-time guard).

use sha2::{Digest, Sha256};

use super::facts_fixtures::*;
use super::*;
use FactCode as F;

const FACTS_LINE_3: &str = "facts present as given: name, hours (3 lines), phone";

/// `page` passes with `facts`, and the facts line is `line`.
fn positive(label: &str, page: &str, facts: &ApprovedFacts, line: &str) {
    let v = PageVerdict::evaluate(page.as_bytes(), Some(facts));
    assert_eq!(v.outcome, PageOutcome::Passed, "{label}");
    assert!(v.passed(), "{label}");
    assert_eq!(v.label(), LABEL_PASSED, "{label}");
    assert_eq!(v.facts_line().as_deref(), Some(line), "{label}");
    let parsed = check(page.as_bytes()).unwrap_or_else(|r| panic!("{label}: {r}"));
    assert_eq!(check_facts(&parsed, facts), Ok(()), "{label}");
}

/// `page` passes the inert check alone, and with `facts` lacks exactly
/// `code`: the label is still the pass label, and no facts line is added.
fn negative(label: &str, page: &str, facts: &ApprovedFacts, code: FactCode) {
    let bare = PageVerdict::evaluate(page.as_bytes(), None);
    assert_eq!(bare.outcome, PageOutcome::Passed, "{label}: inert part");
    let v = PageVerdict::evaluate(page.as_bytes(), Some(facts));
    assert_eq!(v.outcome, PageOutcome::FactMissing(code), "{label}");
    assert!(!v.passed(), "{label}");
    assert_eq!(v.label(), LABEL_PASSED, "{label}: the inert label");
    assert_eq!(v.facts_line(), None, "{label}");
    let parsed = check(page.as_bytes()).unwrap_or_else(|r| panic!("{label}: {r}"));
    assert_eq!(check_facts(&parsed, facts), Err(code), "{label}");
}

#[test]
fn s_o10_facts_positives_pass_and_only_add_the_facts_line() {
    let reference = reference_facts();
    positive("reference facts", REFERENCE, &reference, FACTS_LINE_3);

    let bare = PageVerdict::evaluate(REFERENCE.as_bytes(), None);
    assert_eq!(bare.outcome, PageOutcome::Passed);
    assert_eq!(bare.label(), LABEL_PASSED);
    assert_eq!(bare.facts_line(), None, "no facts, no facts line");

    let colon = facts(NAME, &["Monday 07:00-17:00"], PHONE);
    positive(
        "Monday 07:00-17:00",
        &edit("<li>Sunday: closed</li>", "<li>Monday 07:00-17:00</li>"),
        &colon,
        "facts present as given: name, hours (1 lines), phone",
    );

    positive(
        "the phone split across spans inside one leaf",
        &edit(
            "<p>Phone: <strong>01632 960123</strong></p>",
            "<p><strong>01632</strong> 960123</p>",
        ),
        &reference,
        FACTS_LINE_3,
    );
    positive(
        "the phone split across nested spans inside one leaf",
        &edit(
            "<p>Phone: <strong>01632 960123</strong></p>",
            "<p><strong>01632</strong> <em><small>960123</small></em></p>",
        ),
        &reference,
        FACTS_LINE_3,
    );
    positive(
        "an en-dash spelling of the approved number elsewhere",
        &with_paragraph(&london_page(), "Or call 020–7946–0123"),
        &london_facts(),
        FACTS_LINE_3,
    );
}

#[test]
fn s_o10_facts_page_negatives_give_exactly_their_code() {
    let reference = reference_facts();
    let london = london_facts();
    let h1 = "<h1>Rosie&#39;s Bakery</h1>";
    for (label, page, code) in [
        (
            "the name only in a p",
            edit(h1, "<p>Rosie&#39;s Bakery</p>"),
            F::NameNotHeading,
        ),
        (
            "two h1",
            edit("<h2>Contact</h2>", "<h1>Rosie&#39;s Bakery</h1>"),
            F::NameNotHeading,
        ),
        (
            "the h1 differs in case",
            edit(h1, "<h1>Rosie&#39;s bakery</h1>"),
            F::NameNotHeading,
        ),
        (
            "title differs from the name",
            edit(
                "<title>Rosie&#39;s Bakery</title>",
                "<title>Rosie&#39;s Bakes</title>",
            ),
            F::NameNotTitle,
        ),
        (
            "hours line 2 missing",
            edit("<li>Saturday: 08:00 – 14:00</li>\n", ""),
            F::HoursLineMissing { index: 1 },
        ),
        (
            "an hours line only as a substring",
            edit(
                "<li>Monday to Friday: 07:00 – 17:00</li>",
                "<li>Open Monday to Friday: 07:00 – 17:00</li>",
            ),
            F::HoursLineMissing { index: 0 },
        ),
        (
            "an hours line only in a heading",
            edit(
                "<li>Sunday: closed</li>\n</ul>\n",
                "</ul>\n<h3>Sunday: closed</h3>\n",
            ),
            F::HoursLineMissing { index: 2 },
        ),
        (
            "a phone superstring",
            edit(PHONE_ELEMENT, "<strong>01632 9601234</strong>"),
            F::PhoneMissing,
        ),
        (
            "the phone split across separate blocks",
            edit(
                "<p>Phone: <strong>01632 960123</strong></p>",
                "<ul>\n<li>01632</li>\n<li>960123</li>\n</ul>",
            ),
            F::PhoneMissing,
        ),
        (
            "a price row",
            with_paragraph(REFERENCE, "1.50 2.50 3.50 4.50"),
            F::ForeignNumber,
        ),
        (
            "a slashed date",
            with_paragraph(REFERENCE, "Baked on 3/11/2026"),
            F::ForeignNumber,
        ),
        (
            "a foreign number across a br",
            edit(
                "<address>12 Mill Lane<br>Exampleton</address>",
                "<address>01632<br>960999</address>",
            ),
            F::ForeignNumber,
        ),
        (
            "a foreign number across spans",
            with_paragraph(REFERENCE, "<strong>01632</strong> 960999"),
            F::ForeignNumber,
        ),
    ] {
        negative(label, &page, &reference, code);
    }
    for (label, number) in [
        ("en dashes", "020–7946–0999"),
        ("brackets and a dot", "(020) 7946.0999"),
        ("an international prefix", "+44 20 7946 0999"),
    ] {
        negative(
            label,
            &with_paragraph(&london_page(), &format!("Fax {number}")),
            &london,
            F::ForeignNumber,
        );
    }
}

#[test]
fn s_o10_facts_construction_refuses_with_exactly_its_code() {
    let ok = |name: &str, hours: &[&str], phone: &str| {
        assert!(
            ApprovedFacts::new(name, hours, phone).is_ok(),
            "{name:?} {hours:?} {phone:?}"
        );
    };
    for phone in [
        "(020) 7946 0123",
        "+44 20 7946 0123",
        "+44 (0)20 7946 0123",
        "020-7946-0123",
        "020.7946.0123",
        "02079460123",
    ] {
        ok(NAME, &HOURS, phone);
    }

    let refused = |label: &str, name: &str, hours: &[&str], phone: &str, code: FactCode| {
        assert_eq!(ApprovedFacts::new(name, hours, phone), Err(code), "{label}");
    };
    refused(
        "Monday 0700-1700",
        NAME,
        &["Monday 0700-1700"],
        PHONE,
        F::FactPhoneLike,
    );
    refused(
        "a phone-like name",
        "Rosie 01632 960123",
        &HOURS,
        PHONE,
        F::FactPhoneLike,
    );
    refused(
        "an en-dash number in an hours line",
        NAME,
        &["Monday 0700–1700"],
        PHONE,
        F::FactPhoneLike,
    );
    for (label, phone) in [
        ("ext. 45", "020 7946 0123 ext. 45"),
        ("x45", "020 7946 0123 x45"),
        ("#45", "020 7946 0123 #45"),
        ("en dashes", "020–7946–0123"),
        ("6 digits", "016 329"),
        ("16 digits", "0123 4567 8901 2345"),
        ("a plus inside", "020+7946 0123"),
        ("two pluses", "++44 20 7946 0123"),
        ("ends with a bracket", "(020) 7946 0123)"),
        ("starts with a bracket close", ")020 7946 0123"),
        ("three non-digits in a row", "020 - 7946 0123"),
        ("a slash", "020/7946/0123"),
    ] {
        refused(label, NAME, &HOURS, phone, F::PhoneFormat);
    }
    let eight: Vec<String> = (1..=8).map(|k| format!("Day {k}: closed")).collect();
    let eight: Vec<&str> = eight.iter().map(String::as_str).collect();
    for (label, name, hours, phone) in [
        (
            "bidi in the name",
            "Rosie\u{202E}s Bakery",
            &HOURS[..],
            PHONE,
        ),
        (
            "bidi in an hours line",
            NAME,
            &["Sunday\u{2066}: closed"][..],
            PHONE,
        ),
        ("8 hours lines", NAME, &eight[..], PHONE),
        ("no hours line", NAME, &[][..], PHONE),
        (
            "a needle in an hours line",
            NAME,
            &["Orders: https://x"][..],
            PHONE,
        ),
        (
            "a needle in the name",
            "Rosie DATA: Bakery",
            &HOURS[..],
            PHONE,
        ),
        ("an empty name", "", &HOURS[..], PHONE),
        ("an empty hours line", NAME, &[""][..], PHONE),
        ("a leading space", " Rosie", &HOURS[..], PHONE),
        ("a trailing space", NAME, &["Sunday: closed "][..], PHONE),
        ("a double space", "Rosie's  Bakery", &HOURS[..], PHONE),
        ("a newline", NAME, &["Sunday:\nclosed"][..], PHONE),
        ("a tab", "Rosie's\tBakery", &HOURS[..], PHONE),
        ("Cyrillic", "Rosie's Bаkery", &HOURS[..], PHONE),
        ("U+00A0", "Rosie's\u{A0}Bakery", &HOURS[..], PHONE),
        (
            "a leading space in the phone",
            NAME,
            &HOURS[..],
            " 01632 960123",
        ),
        (
            "a double space in the phone",
            NAME,
            &HOURS[..],
            "01632  960123",
        ),
    ] {
        refused(label, name, hours, phone, F::FactsInvalid);
    }
}

/// The declared facts hash, restated: SHA-256 over `put` of the domain, the
/// name, a u32 line count, `put` of each line, and `put` of the phone, where
/// `put(b)` is `b`'s length as a u64 (big-endian) then `b`.
fn independent_facts_hash(name: &str, hours: &[&str], phone: &str) -> [u8; 32] {
    fn put(h: &mut Sha256, b: &[u8]) {
        h.update((b.len() as u64).to_be_bytes());
        h.update(b);
    }
    let mut h = Sha256::new();
    put(&mut h, b"nexus.inert_page.facts.v1");
    put(&mut h, name.as_bytes());
    h.update((hours.len() as u32).to_be_bytes());
    for line in hours {
        put(&mut h, line.as_bytes());
    }
    put(&mut h, phone.as_bytes());
    h.finalize().into()
}

#[test]
fn s_o10_facts_hash_is_declared_and_debug_shows_only_it() {
    let f = reference_facts();
    assert_eq!(f.hash(), independent_facts_hash(NAME, &HOURS, PHONE));
    assert_eq!(f, f.clone());
    assert_ne!(f, london_facts());
    assert_ne!(f.hash(), london_facts().hash());
    let shown = format!("{f:?}");
    assert_eq!(
        shown,
        format!("ApprovedFacts {{ hash: {:?} }}", hex::encode(f.hash()))
    );
    for secret in ["Rosie", "Bakery", "Monday", "07:00", "01632", "960123"] {
        assert!(!shown.contains(secret), "{shown}");
    }
    assert_eq!(FactCode::CODES.len(), 8);
    for (k, name) in FactCode::CODES.iter().enumerate() {
        assert_eq!(usize::from(every_code()[k].position()), k);
        assert_eq!(every_code()[k].as_str(), *name);
    }
}

fn every_code() -> [FactCode; 8] {
    [
        F::FactsInvalid,
        F::PhoneFormat,
        F::FactPhoneLike,
        F::NameNotHeading,
        F::NameNotTitle,
        F::HoursLineMissing { index: 3 },
        F::PhoneMissing,
        F::ForeignNumber,
    ]
}

/// `ApprovedFacts` implements neither `Display` nor `Serialize`: if it did,
/// two impls of this trait would apply and the item below would be
/// ambiguous, which does not compile.
trait AmbiguousIfImplemented<A> {
    fn item() {}
}
impl<T: ?Sized> AmbiguousIfImplemented<()> for T {}
struct ViaDisplay;
impl<T: ?Sized + std::fmt::Display> AmbiguousIfImplemented<ViaDisplay> for T {}
struct ViaSerialize;
impl<T: ?Sized + serde::Serialize> AmbiguousIfImplemented<ViaSerialize> for T {}
const _: fn() = || {
    <ApprovedFacts as AmbiguousIfImplemented<_>>::item();
};
