//! M3-A oracles (`m3-design-approved.md` §9.3, pre-registered in the first
//! commit and not edited after a result): M3-O1 to M3-O9 for Mother's
//! deterministic page Verifier, over fixed fixtures only. No assembler,
//! broker or model is involved. M3-O6's 2,000-page differential is in
//! `m3a_s_o4.rs`; its hand-written expected texts are in
//! `m3a_reader_fixtures.rs`, which the HTML5 oracle host also checks.

use super::m3a_reader_fixtures::{fixture_page, FIXTURES};
use super::m3a_s_o4::s1a_lines;
use crate::lane::Verdict;
use crate::verifier::*;
use nexus_governed_control::inert_page::{
    check, check_facts, policy_hash, ApprovedFacts, FactCode, FactsReport, RefusalCode,
};
use sha2::{Digest, Sha256};

/// The hand-written bakery page (design M3 §4.3's layout, no assembler).
const BAKERY: &str = include_str!("m3a_bakery.html");
const BAKERY_SHA256: &str = "411303c80093b710266b4188b267acc3ad0e87fa5ea6626d0f8682e550afde73";
/// S1a's reference page, whose head the reader fixtures reuse.
const REFERENCE: &str = include_str!("../../../kernel/src/inert_page/fixtures/reference.html");

/// The F-BAKERY facts (design M3 §9.5).
const NAME: &str = "Rosa's Bakery";
const HOURS: [&str; 2] = ["Tuesday to Sunday, 7am to 3pm", "Closed on Mondays"];
const PHONE: &str = "555-0142";

use CheckId::{V0, V1, V2, V3, V4, V5};
use CheckResult::{Fail, NotRun, Pass};

fn bakery_facts() -> ApprovedFacts {
    ApprovedFacts::new(NAME, &HOURS, PHONE).expect("F-BAKERY is admissible")
}

/// The receipt the broker would give for exactly `bytes`.
fn receipt_of(bytes: &[u8]) -> Receipt {
    Receipt {
        sha256: Sha256::digest(bytes).into(),
        bytes: bytes.len() as u64,
        policy_hash: policy_hash(),
    }
}

/// `page`, published as is, with the bakery facts.
fn verify(page: &str) -> VerifierReport {
    let bytes = page.as_bytes();
    verify_page(bytes, &bakery_facts(), Some(&receipt_of(bytes)))
}

/// `base` with its one occurrence of `from` replaced by `to`.
fn edit_in(base: &str, from: &str, to: &str) -> String {
    assert_eq!(base.matches(from).count(), 1, "anchor {from:?}");
    base.replacen(from, to, 1)
}

/// The bakery page with one edit.
fn edit(from: &str, to: &str) -> String {
    edit_in(BAKERY, from, to)
}

/// The bakery page with one more paragraph at the end of the contact
/// section.
fn with_paragraph(text: &str) -> String {
    edit(
        "<p>Phone: <strong>555-0142</strong></p>\n",
        &format!("<p>Phone: <strong>555-0142</strong></p>\n<p>{text}</p>\n"),
    )
}

fn facts_results(name: FactResult, hours: FactResult, phone: FactResult) -> FactResults {
    FactResults { name, hours, phone }
}

const ALL_PASS: FactResults = FactResults {
    name: FactResult::Pass,
    hours: FactResult::Pass,
    phone: FactResult::Pass,
};

const ALL_NOT_RUN: FactResults = FactResults {
    name: FactResult::NotRun,
    hours: FactResult::NotRun,
    phone: FactResult::NotRun,
};

/// The reader's leaf lines as (tag, text).
fn lines(text: &VisibleText) -> Vec<(&'static str, &str)> {
    text.lines
        .iter()
        .map(|l| (l.tag.name(), l.text.as_str()))
        .collect()
}

/// The gates before `at` passed, `at` failed with `code` (shown as
/// `name`), and nothing after it ran: no fact has a result.
fn gate_failed(label: &str, r: &VerifierReport, at: CheckId, code: CheckCode, name: &str) {
    let at = CheckId::ALL.iter().position(|id| *id == at).unwrap();
    for (k, (id, result)) in r.checks().into_iter().enumerate() {
        let expected = match k.cmp(&at) {
            std::cmp::Ordering::Less => Pass,
            std::cmp::Ordering::Equal => Fail(code),
            std::cmp::Ordering::Greater => NotRun,
        };
        assert_eq!(result, expected, "{label}: {}", id.as_str());
        assert_eq!(r.check(id), expected, "{label}: {}", id.as_str());
    }
    assert_eq!(code.to_string(), name, "{label}");
    assert_eq!(r.first_failure(), Some(code), "{label}");
    assert!(!r.passed(), "{label}");
    assert_eq!(
        r.verdict(),
        Verdict::Fail {
            reason: name.to_string()
        },
        "{label}"
    );
    assert_eq!(r.facts(), ALL_NOT_RUN, "{label}");
    assert_eq!(r.facts_report(), None, "{label}");
    assert_eq!(r.visible_report(), None, "{label}");
}

// ---------------------------------------------------------------- M3-O1

#[test]
fn m3_o1_a_hand_written_bakery_page_passes_with_all_three_facts() {
    assert_eq!(hex::encode(Sha256::digest(BAKERY)), BAKERY_SHA256);
    assert!(check(BAKERY.as_bytes()).is_ok(), "the page is canonical");
    let report = verify(BAKERY);
    for (id, result) in report.checks() {
        assert_eq!(result, Pass, "{}", id.as_str());
    }
    assert!(report.passed());
    assert_eq!(report.first_failure(), None);
    assert_eq!(report.verdict(), Verdict::Pass);
    assert_eq!(report.facts(), ALL_PASS);
    assert_eq!(
        report.facts_report(),
        Some(&FactsReport {
            name_heading: true,
            name_title: true,
            hours: vec![true, true],
            phone: true,
            no_foreign_number: true,
        })
    );
    assert_eq!(
        report.visible_report(),
        Some(&VisibleReport {
            supported: true,
            name: true,
            hours: vec![true, true],
            phone: true,
        })
    );
    assert_eq!(
        report.to_string(),
        "v0 pass, v1 pass, v2 pass, v3 pass, v4 pass, v5 pass; \
         name pass, hours pass, phone pass"
    );

    // The reader's text of it, written by hand.
    let text = read_visible_text(BAKERY.as_bytes()).expect("the reader reads it");
    assert_eq!(text.title, NAME);
    assert_eq!(
        lines(&text),
        vec![
            ("h1", "Rosa's Bakery"),
            (
                "p",
                "Fresh bread and cakes, baked every morning in our small kitchen."
            ),
            ("h2", "Our bread"),
            (
                "p",
                "We bake sourdough, rye and seeded loaves before the doors open."
            ),
            ("li", "Sourdough"),
            ("li", "Rye"),
            ("li", "Seeded loaf"),
            ("h2", "Cakes"),
            (
                "p",
                "Lemon drizzle and carrot cake are on the counter every day."
            ),
            ("h2", "Opening hours"),
            ("li", "Tuesday to Sunday, 7am to 3pm"),
            ("li", "Closed on Mondays"),
            ("h2", "Contact"),
            ("p", "Phone: 555-0142"),
            ("p", "Thank you for buying local."),
        ]
    );
}

// ---------------------------------------------------------------- M3-O2

#[test]
fn m3_o2_no_receipt_or_a_different_receipt_fails_before_any_page_check() {
    // The same size and other bytes: the lengths agree, the digests do not.
    let changed = edit("buying local.", "buying lokal.");
    assert_eq!(changed.len(), BAKERY.len());
    assert!(check(changed.as_bytes()).is_ok());

    let facts = bakery_facts();
    let bytes = BAKERY.as_bytes();
    let good = receipt_of(bytes);
    assert!(verify_page(bytes, &facts, Some(&good)).passed(), "control");

    let r = verify_page(bytes, &facts, None);
    gate_failed(
        "no receipt",
        &r,
        V0,
        CheckCode::NoPublication,
        "no_publication",
    );

    for k in [0, 17, 31] {
        let mut other = good;
        other.sha256[k] ^= 0x01;
        let r = verify_page(bytes, &facts, Some(&other));
        let label = format!("SHA-256 byte {k} differs");
        gate_failed(
            &label,
            &r,
            V1,
            CheckCode::PublishedMismatch,
            "published_mismatch",
        );
    }
    for size in [good.bytes + 1, good.bytes - 1, 0, u64::MAX] {
        let other = Receipt {
            bytes: size,
            ..good
        };
        let r = verify_page(bytes, &facts, Some(&other));
        let label = format!("size {size}");
        gate_failed(
            &label,
            &r,
            V1,
            CheckCode::PublishedMismatch,
            "published_mismatch",
        );
    }

    let r = verify_page(changed.as_bytes(), &facts, Some(&good));
    gate_failed(
        "one byte changed",
        &r,
        V1,
        CheckCode::PublishedMismatch,
        "published_mismatch",
    );
}

// ---------------------------------------------------------------- M3-O3

#[test]
fn m3_o3_s1a_s_o2_plants_are_not_inert_and_nothing_after_runs() {
    use RefusalCode as C;
    let invisible = format!("Seeded{}loaf", char::from_u32(0x200B).unwrap());
    let cyrillic = format!("Seeded l{}af", char::from_u32(0x043E).unwrap());
    let nbsp = format!("Seeded{}loaf", char::from_u32(0x00A0).unwrap());
    // (from, to, S1a's code): each a minimal edit of the bakery page.
    let edits = [
        (
            "<p>Phone: ",
            "<p>Phone: <script>x</script>",
            C::ScriptElement,
        ),
        ("<p>Phone: ", "<p onclick=\"x\">Phone: ", C::EventHandler),
        ("<p>Phone: ", "<p style=\"x\">Phone: ", C::StyleAttribute),
        ("<p>Phone: ", "<p href=\"x\">Phone: ", C::UrlAttribute),
        (
            "<strong>555-0142</strong>",
            "<a>555-0142</a>",
            C::ElementNotAllowed,
        ),
        ("<p>Phone: ", "<div>x</div><p>Phone: ", C::ElementNotAllowed),
        ("<main>\n", "<main>\n<!-- x -->\n", C::Comment),
        ("Seeded loaf", "Seeded&nbsp;loaf", C::EntityNotAllowed),
        ("buying local.", "buying at https://x", C::RemoteReference),
        ("buying local.", "javascript:x", C::JavascriptScheme),
        ("buying local.", "DATA:x", C::DataScheme),
        ("<main>\n", "<main>\r\n", C::CarriageReturn),
        ("Seeded loaf", &invisible, C::InvisibleChar),
        ("Seeded loaf", &cyrillic, C::CharNotAllowed),
        ("Seeded loaf", &nbsp, C::CharNotAllowed),
        ("<!doctype html>", "<!DOCTYPE html>", C::PrologueMismatch),
        (
            "*{box-sizing:border-box}",
            "*{box-sizing:content-box}",
            C::CssMismatch,
        ),
        (
            "<ul class=\"plain\">",
            "<ul id=\"x\" class=\"plain\">",
            C::NotCanonical,
        ),
        (
            "<h1>Rosa&#39;s Bakery</h1>",
            "<h1>Rosa's Bakery</h1>",
            C::NotCanonical,
        ),
    ];
    let mut plants: Vec<(String, String, C)> = edits
        .iter()
        .map(|(from, to, code)| (to.to_string(), edit(from, to), *code))
        .collect();
    let trimmed = BAKERY.trim_end().to_string();
    plants.push(("no final newline".into(), trimmed, C::NoFinalNewline));
    for (label, page, code) in &plants {
        let refused = check(page.as_bytes()).err().map(|r| r.code);
        assert_eq!(refused, Some(*code), "{label}: S1a");
    }
    let facts = bakery_facts();
    for (label, page, code) in &plants {
        let bytes = page.as_bytes();
        let r = verify_page(bytes, &facts, Some(&receipt_of(bytes)));
        let name = format!("page_not_inert:{}", code.as_str());
        gate_failed(label, &r, V2, CheckCode::PageNotInert(*code), &name);
    }

    // The policy the receipt names is not Mother's.
    let mut other = receipt_of(BAKERY.as_bytes());
    other.policy_hash[0] ^= 0x01;
    let r = verify_page(BAKERY.as_bytes(), &facts, Some(&other));
    gate_failed(
        "another policy",
        &r,
        V2,
        CheckCode::PolicyChanged,
        "policy_changed",
    );
}

// ---------------------------------------------------------------- M3-O4

#[test]
fn m3_o4_s1a_s_o10_page_negatives_fail_their_fact() {
    use FactResult::{Fail as F, Pass as P};
    let h1 = "<h1>Rosa&#39;s Bakery</h1>";
    let phone_p = "<p>Phone: <strong>555-0142</strong></p>";
    let missing = |c: FactCode| Fail(CheckCode::FactMissing(c));
    let cases: Vec<(&str, String, &str, CheckResult, CheckResult, FactResults)> = vec![
        (
            "the name only in a p",
            edit(h1, "<p>Rosa&#39;s Bakery</p>"),
            "name_not_heading",
            missing(FactCode::NameNotHeading),
            Fail(CheckCode::NameNotVisible),
            facts_results(F, P, P),
        ),
        (
            "two h1",
            edit("<h2>Contact</h2>", h1),
            "name_not_heading",
            missing(FactCode::NameNotHeading),
            Fail(CheckCode::NameNotVisible),
            facts_results(F, P, P),
        ),
        (
            "the h1 differs in case",
            edit(h1, "<h1>Rosa&#39;s bakery</h1>"),
            "name_not_heading",
            missing(FactCode::NameNotHeading),
            Fail(CheckCode::NameNotVisible),
            facts_results(F, P, P),
        ),
        (
            "the title differs from the name",
            edit(
                "<title>Rosa&#39;s Bakery</title>",
                "<title>Rosa&#39;s Bakes</title>",
            ),
            "name_not_title",
            missing(FactCode::NameNotTitle),
            Fail(CheckCode::NameNotVisible),
            facts_results(F, P, P),
        ),
        (
            "hours line 2 missing",
            edit("<li>Closed on Mondays</li>\n", ""),
            "hours_line_missing:1",
            missing(FactCode::HoursLineMissing { index: 1 }),
            Fail(CheckCode::HoursLineNotVisible(1)),
            facts_results(P, F, P),
        ),
        (
            "an hours line only as a substring",
            edit(
                "<li>Tuesday to Sunday, 7am to 3pm</li>",
                "<li>Open Tuesday to Sunday, 7am to 3pm</li>",
            ),
            "hours_line_missing:0",
            missing(FactCode::HoursLineMissing { index: 0 }),
            Fail(CheckCode::HoursLineNotVisible(0)),
            facts_results(P, F, P),
        ),
        (
            "an hours line only in a heading",
            edit(
                "<li>Closed on Mondays</li>\n</ul>\n",
                "</ul>\n<h3>Closed on Mondays</h3>\n",
            ),
            "hours_line_missing:1",
            missing(FactCode::HoursLineMissing { index: 1 }),
            Fail(CheckCode::HoursLineNotVisible(1)),
            facts_results(P, F, P),
        ),
        (
            "a phone superstring",
            edit("<strong>555-0142</strong>", "<strong>555-01423</strong>"),
            "phone_missing",
            missing(FactCode::PhoneMissing),
            Fail(CheckCode::PhoneNotVisible),
            facts_results(P, P, F),
        ),
        (
            "the phone split across separate blocks",
            edit(phone_p, "<ul>\n<li>555</li>\n<li>-0142</li>\n</ul>"),
            "phone_missing",
            missing(FactCode::PhoneMissing),
            Fail(CheckCode::PhoneNotVisible),
            facts_results(P, P, F),
        ),
        (
            "a second phone number",
            with_paragraph("Or call 555-0199"),
            "foreign_number",
            missing(FactCode::ForeignNumber),
            Pass,
            facts_results(P, P, F),
        ),
        (
            "a foreign number across spans",
            with_paragraph("<strong>555</strong>-0199"),
            "foreign_number",
            missing(FactCode::ForeignNumber),
            Pass,
            facts_results(P, P, F),
        ),
        (
            "a price row",
            with_paragraph("1.50 2.50 3.50 4.50"),
            "foreign_number",
            missing(FactCode::ForeignNumber),
            Pass,
            facts_results(P, P, F),
        ),
        (
            "a slashed date",
            with_paragraph("Baked on 3/11/2026"),
            "foreign_number",
            missing(FactCode::ForeignNumber),
            Pass,
            facts_results(P, P, F),
        ),
    ];
    let positives = [
        (
            "spans inside one leaf",
            edit(phone_p, "<p><strong>555</strong>-0142</p>"),
        ),
        ("an en-dash spelling", with_paragraph("Or call 555–0142")),
    ];
    let f = bakery_facts();
    for (label, page, _, v3, _, _) in &cases {
        let parsed = check(page.as_bytes()).unwrap_or_else(|r| panic!("{label}: {r}"));
        let expected = match v3 {
            Fail(CheckCode::FactMissing(code)) => Err(*code),
            _ => Ok(()),
        };
        assert_eq!(check_facts(&parsed, &f), expected, "{label}: S1a");
    }
    for (label, page) in &positives {
        let parsed = check(page.as_bytes()).unwrap_or_else(|r| panic!("{label}: {r}"));
        assert_eq!(check_facts(&parsed, &f), Ok(()), "{label}: S1a");
    }

    for (label, page, name, v3, v4, facts) in &cases {
        let r = verify(page);
        for id in [V0, V1, V2, V5] {
            assert_eq!(r.check(id), Pass, "{label}: {}", id.as_str());
        }
        assert_eq!(r.check(V3), *v3, "{label}: V3");
        assert_eq!(r.check(V4), *v4, "{label}: V4");
        assert_eq!(r.facts(), *facts, "{label}: the facts");
        let code = r
            .first_failure()
            .unwrap_or_else(|| panic!("{label}: no failure"));
        assert_eq!(code.to_string(), *name, "{label}");
        assert_eq!(
            r.verdict(),
            Verdict::Fail {
                reason: name.to_string()
            },
            "{label}"
        );
    }

    // Positives: the phone split across spans inside one leaf, and an
    // en-dash spelling of the approved number elsewhere.
    for (label, page) in &positives {
        let r = verify(page);
        assert!(r.passed(), "{label}: {r}");
        assert_eq!(r.facts(), ALL_PASS, "{label}");
    }
}

// ---------------------------------------------------------------- M3-O5

#[test]
fn m3_o5_two_keys_each_refuses_what_the_other_was_made_to_pass() {
    let facts = bakery_facts();
    let hooked = |page: &str, v3_pass: bool, v4_pass: bool| {
        let bytes = page.as_bytes();
        let hooks = Hooks { v3_pass, v4_pass };
        verify_page_hooked(bytes, &facts, Some(&receipt_of(bytes)), hooks)
    };

    // The phone absent: both keys refuse, and with V3 made to pass, V4
    // still refuses.
    let absent = edit(
        "<p>Phone: <strong>555-0142</strong></p>",
        "<p>Phone: ask in the shop</p>",
    );
    let r = hooked(&absent, false, false);
    assert_eq!(
        r.check(V3),
        Fail(CheckCode::FactMissing(FactCode::PhoneMissing))
    );
    assert_eq!(r.check(V4), Fail(CheckCode::PhoneNotVisible));
    let r = hooked(&absent, true, false);
    assert_eq!(r.check(V3), Pass);
    assert_eq!(r.check(V4), Fail(CheckCode::PhoneNotVisible));
    assert_eq!(r.first_failure(), Some(CheckCode::PhoneNotVisible));
    assert_eq!(r.facts().phone, FactResult::Fail);
    assert!(!r.passed());
    assert_eq!(
        r.verdict(),
        Verdict::Fail {
            reason: "phone_not_visible".into()
        }
    );

    // FC-4 failing: with V4 made to pass, V3 still refuses.
    let foreign = with_paragraph("Or call 555-0199");
    let r = hooked(&foreign, false, true);
    assert_eq!(r.check(V4), Pass);
    assert_eq!(
        r.check(V3),
        Fail(CheckCode::FactMissing(FactCode::ForeignNumber))
    );
    assert_eq!(
        r.first_failure(),
        Some(CheckCode::FactMissing(FactCode::ForeignNumber))
    );
    assert_eq!(r.facts().phone, FactResult::Fail);
    assert!(!r.passed());

    // Controls: the seams really force their check, and only both together
    // let the absent phone through.
    let r = hooked(&absent, true, true);
    assert!(r.passed(), "{r}");
    assert_eq!(hooked(BAKERY, false, false), verify(BAKERY));
}

// ---------------------------------------------------------------- M3-O6

#[test]
fn m3_o6_the_reader_gives_the_hand_written_texts() {
    assert_eq!(FIXTURES.len(), 8);
    // S1a accepts each page, and its own extraction agrees with the
    // hand-written texts.
    for f in FIXTURES {
        let page = fixture_page(REFERENCE, f);
        let parsed = check(page.as_bytes()).unwrap_or_else(|r| panic!("{}: {r}", f.label));
        assert_eq!(parsed.title(), f.title_text, "{}: S1a title", f.label);
        let s1a = s1a_lines(&parsed);
        let s1a: Vec<(&str, &str)> = s1a.iter().map(|(t, s)| (*t, s.as_str())).collect();
        assert_eq!(s1a, f.lines.to_vec(), "{}: S1a", f.label);
    }
    for f in FIXTURES {
        let page = fixture_page(REFERENCE, f);
        let text = read_visible_text(page.as_bytes())
            .unwrap_or_else(|_| panic!("{}: reader_unsupported", f.label));
        assert_eq!(text.title, f.title_text, "{}: title", f.label);
        assert_eq!(lines(&text), f.lines.to_vec(), "{}", f.label);
    }
}

#[test]
fn m3_o6_unsupported_tags_attributes_lines_and_entities_fail_closed() {
    assert!(read_visible_text(BAKERY.as_bytes()).is_ok(), "control");
    let title = "<title>Rosa&#39;s Bakery</title>";
    let strong = "<strong>555-0142</strong>";
    let muted = "<p class=\"muted\">";
    let cases: &[(&str, &str, &str)] = &[
        // Tags inside a leaf: never deleted as decoration.
        ("<span> in a leaf", strong, "<span>555-0142</span>"),
        ("<a> in a leaf", strong, "<a>555-0142</a>"),
        ("<b> in a leaf", strong, "<b>555-0142</b>"),
        ("<div> in a leaf", "<p>Phone: ", "<p>Phone: <div>x</div>"),
        ("a leaf tag in a leaf", "<p>Phone: ", "<p>Phone: <p>x</p>"),
        ("<br/>", "Thank you for", "Thank you<br/>for"),
        ("<br />", "Thank you for", "Thank you<br />for"),
        ("</br>", "Thank you for", "Thank you</br>for"),
        ("a comment in a leaf", "Thank you", "Thank<!-- x --> you"),
        // Attributes.
        ("title=", muted, "<p class=\"muted\" title=\"x\">"),
        ("style=", muted, "<p style=\"x\">"),
        ("a bare attribute", muted, "<p hidden>"),
        ("an unquoted value", muted, "<p class=muted>"),
        ("an empty value", muted, "<p class=\"\">"),
        (
            "a repeated attribute",
            muted,
            "<p class=\"muted\" class=\"lead\">",
        ),
        ("an upper-case value", muted, "<p class=\"Muted\">"),
        (
            "an attribute on a span",
            strong,
            "<strong hidden>555-0142</strong>",
        ),
        (
            "<br> with a class",
            "Thank you for",
            "Thank you<br class=\"x\">for",
        ),
        (
            "data- on a container",
            "<section id=\"hours\">",
            "<section data-x=\"1\">",
        ),
        (
            "single quotes",
            "<section id=\"hours\">",
            "<section id='hours'>",
        ),
        (
            "an attribute on a close",
            "</footer>",
            "</footer class=\"x\">",
        ),
        ("an upper-case tag", "<h2>Cakes</h2>", "<H2>Cakes</H2>"),
        // Lines.
        ("an empty line", "<main>\n", "<main>\n\n"),
        ("a text line", "<main>\n", "<main>\nHello\n"),
        ("an unknown container", "<main>\n", "<main>\n<div>\n"),
        ("an unknown leaf", "<h2>Cakes</h2>", "<h4>Cakes</h4>"),
        ("a leaf without its close", "<h2>Cakes</h2>", "<h2>Cakes"),
        (
            "a leaf closed by another tag",
            "<h2>Cakes</h2>",
            "<h2>Cakes</h3>",
        ),
        (
            "two leaves on one line",
            "<h2>Cakes</h2>\n",
            "<h2>Cakes</h2><h2>Pies</h2>\n",
        ),
        ("text after a container", "<main>\n", "<main>x\n"),
        ("a br line", "<main>\n", "<main>\n<br>\n"),
        ("an indented line", "<h2>Cakes</h2>", " <h2>Cakes</h2>"),
        // Entities and raw markup characters.
        ("&nbsp;", "Seeded loaf", "Seeded&nbsp;loaf"),
        ("&#106;", "Seeded loaf", "Seeded &#106; loaf"),
        ("&#x6A;", "Seeded loaf", "Seeded &#x6A; loaf"),
        ("a bare &", "Seeded loaf", "Seeded & loaf"),
        ("&AMP;", "Seeded loaf", "Seeded &AMP; loaf"),
        (
            "&amp without its semicolon",
            "Seeded loaf",
            "Seeded &amp loaf",
        ),
        ("a raw <", "Seeded loaf", "Seeded < loaf"),
        ("a raw >", "Seeded loaf", "Seeded > loaf"),
        (
            "&nbsp; in the title",
            title,
            "<title>Rosa&nbsp;s Bakery</title>",
        ),
        (
            "a tag in the title",
            title,
            "<title>Rosa <em>s</em></title>",
        ),
        // The document's frame.
        ("no title end", title, "<title>Rosa&#39;s Bakery"),
        ("no title", "<title>Rosa&#39;s Bakery</title>\n", ""),
        ("no body start", "<body>\n", "<body class=\"x\">\n"),
        ("no body end", "</body>\n", "</body>"),
        (
            "no line feed before the body end",
            "</footer>\n</body>",
            "</footer></body>",
        ),
    ];
    for (label, from, to) in cases {
        let page = edit(from, to);
        assert_eq!(
            read_visible_text(page.as_bytes()),
            Err(ReaderUnsupported),
            "{label}"
        );
    }
    let mut not_utf8 = BAKERY.as_bytes().to_vec();
    not_utf8[BAKERY.find("Seeded").unwrap()] = 0xFF;
    assert_eq!(
        read_visible_text(&not_utf8),
        Err(ReaderUnsupported),
        "not UTF-8"
    );
    assert_eq!(
        CheckCode::ReaderUnsupported.to_string(),
        "reader_unsupported"
    );
}

// ---------------------------------------------------------------- M3-O7

#[test]
fn m3_o7_an_email_shaped_token_fails_and_an_at_sign_alone_passes() {
    let scanned = [
        ("info@rosasbakery.com", true),
        ("Write to INFO@ROSAS.CO.UK today", true),
        ("Mail rosa.baker+orders@bakery-shop.example.", true),
        ("<strong>info</strong>@rosasbakery.com", true),
        ("<em>orders@rosas</em>.bakery", true),
        ("Find us @ the market", false),
        ("rosa@home", false),
        ("a@b.c", false),
        ("@rosasbakery.com", false),
        ("info@.com", false),
        ("x@y.4u", false),
        ("Fresh bread @ 8 o&#39;clock", false),
    ];
    let in_title = edit(
        "<title>Rosa&#39;s Bakery</title>",
        "<title>orders@rosas.bakery</title>",
    );
    let pages: Vec<String> = scanned.iter().map(|(t, _)| with_paragraph(t)).collect();
    for page in pages.iter().chain([&in_title]) {
        assert!(check(page.as_bytes()).is_ok(), "{page}: S1a");
    }

    let r = verify(&with_paragraph("Write to info@rosasbakery.com"));
    for id in [V0, V1, V2, V3, V4] {
        assert_eq!(r.check(id), Pass, "{}", id.as_str());
    }
    assert_eq!(r.check(V5), Fail(CheckCode::EmailNotSupported));
    assert_eq!(r.first_failure(), Some(CheckCode::EmailNotSupported));
    assert_eq!(
        CheckCode::EmailNotSupported.to_string(),
        "email_not_supported"
    );
    assert_eq!(
        r.verdict(),
        Verdict::Fail {
            reason: "email_not_supported".into()
        }
    );
    assert_eq!(r.facts(), ALL_PASS, "an email is not a fact");

    let r = verify(&with_paragraph("Find us @ the market"));
    assert!(r.passed(), "{r}");

    let scan = |page: &str| {
        let parsed = check(page.as_bytes()).unwrap_or_else(|r| panic!("{page}: {r}"));
        email_scan(&parsed)
    };
    for ((text, found), page) in scanned.iter().zip(&pages) {
        let expected = found.then_some(EmailShaped);
        assert_eq!(scan(page), expected, "{text}");
    }
    assert_eq!(scan(BAKERY), None, "control");

    // In the title.
    assert_eq!(scan(&in_title), Some(EmailShaped));
    let r = verify(&in_title);
    assert_eq!(r.check(V5), Fail(CheckCode::EmailNotSupported));
    assert_eq!(
        r.first_failure(),
        Some(CheckCode::FactMissing(FactCode::NameNotTitle))
    );
}

// ---------------------------------------------------------------- M3-O8

/// `text` without its comment lines.
fn code_of(text: &str) -> String {
    text.lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn m3_o8_the_verifier_is_deterministic_and_reads_only_its_arguments() {
    for page in [
        BAKERY.to_string(),
        with_paragraph("Or call 555-0199"),
        edit("<p>Phone: ", "<p>Phone: <script>x</script>"),
        edit("<li>Closed on Mondays</li>\n", ""),
    ] {
        let first = verify(&page);
        let copy = page.clone();
        assert_eq!(verify(&copy), first);
        assert_eq!(verify(&page).to_string(), first.to_string());
    }

    // The source names no model, file, environment, process, command,
    // clock, lock, thread, network, randomness or output.
    let source = code_of(include_str!("../verifier.rs"));
    assert!(
        source.contains("pub fn verify_page("),
        "the scan reads the verifier"
    );
    let needles: Vec<String> = [
        ("mod", "el"),
        ("Agent", "Door"),
        ("Effect", "Port"),
        ("std::", "fs"),
        ("std::", "io"),
        ("std::", "env"),
        ("env", "!("),
        ("std::", "process"),
        ("Com", "mand"),
        ("std::", "time"),
        ("Inst", "ant"),
        ("System", "Time"),
        ("clo", "ck"),
        ("std::", "net"),
        ("Tcp", ""),
        ("std::", "sync"),
        ("Mut", "ex"),
        ("Rw", "Lock"),
        ("Atom", "ic"),
        ("Ce", "ll"),
        ("static ", "mut"),
        ("thre", "ad"),
        ("ran", "dom"),
        ("get", "random"),
        ("uu", "id"),
        ("pri", "nt"),
        ("include", "_"),
        ("uns", "afe"),
    ]
    .iter()
    .map(|(a, b)| format!("{a}{b}"))
    .collect();
    let lower = source.to_ascii_lowercase();
    for needle in &needles {
        assert!(
            !lower.contains(&needle.to_ascii_lowercase()),
            "verifier.rs: {needle}"
        );
    }
}

// ---------------------------------------------------------------- M3-O9

#[test]
fn m3_o9_canaries_in_the_page_and_the_facts_never_reach_the_report() {
    let name = "Rosa NXCANARYn4me Bakery";
    let hours = ["NXCANARYh0urs Tuesday to Sunday", "Closed on Mondays"];
    let facts = ApprovedFacts::new(name, &hours, PHONE).expect("admissible");
    let page = BAKERY
        .replace("Rosa&#39;s Bakery", name)
        .replace("Tuesday to Sunday, 7am to 3pm", hours[0]);
    let page = edit_in(
        &page,
        "buying local.",
        "buying local. NXCANARYp4ge orders@NXCANARYm4il.example",
    );
    let canaries = [
        "NXCANARY", "nxcanary", "n4me", "h0urs", "p4ge", "m4il", "Rosa", "Tuesday", "555", "0142",
    ];
    let bytes = page.as_bytes();
    let good = receipt_of(bytes);
    let foreign = edit_in(&page, "buying local.", "call 555-0199.");
    let reports = [
        verify_page(bytes, &facts, Some(&good)),
        verify_page(bytes, &facts, None),
        verify_page(
            foreign.as_bytes(),
            &facts,
            Some(&receipt_of(foreign.as_bytes())),
        ),
        verify_page(
            BAKERY.as_bytes(),
            &facts,
            Some(&receipt_of(BAKERY.as_bytes())),
        ),
    ];
    assert_eq!(reports[0].check(V5), Fail(CheckCode::EmailNotSupported));
    for r in &reports {
        let mut shown = vec![
            format!("{r:?}"),
            r.to_string(),
            format!("{:?}", r.verdict()),
            format!("{:?}", r.facts()),
            format!("{:?}", r.first_failure()),
        ];
        shown.extend(r.first_failure().map(|c| c.to_string()));
        shown.extend(
            r.checks()
                .iter()
                .map(|(id, c)| format!("{} {c} {c:?}", id.as_str())),
        );
        for s in &shown {
            for canary in canaries {
                assert!(!s.contains(canary), "{canary} in {s}");
            }
        }
    }
    let text = read_visible_text(bytes).expect("the reader reads it");
    let shown = format!("{text:?}");
    for canary in canaries {
        assert!(!shown.contains(canary), "{canary} in {shown}");
    }

    // The codes are closed: every one, as shown.
    for (code, name) in [
        (CheckCode::NoPublication, "no_publication"),
        (CheckCode::PublishedMismatch, "published_mismatch"),
        (
            CheckCode::PageNotInert(RefusalCode::ScriptElement),
            "page_not_inert:script_element",
        ),
        (CheckCode::PolicyChanged, "policy_changed"),
        (
            CheckCode::FactMissing(FactCode::NameNotHeading),
            "name_not_heading",
        ),
        (
            CheckCode::FactMissing(FactCode::NameNotTitle),
            "name_not_title",
        ),
        (
            CheckCode::FactMissing(FactCode::HoursLineMissing { index: 6 }),
            "hours_line_missing:6",
        ),
        (
            CheckCode::FactMissing(FactCode::PhoneMissing),
            "phone_missing",
        ),
        (
            CheckCode::FactMissing(FactCode::ForeignNumber),
            "foreign_number",
        ),
        (CheckCode::NameNotVisible, "name_not_visible"),
        (
            CheckCode::HoursLineNotVisible(6),
            "hours_line_not_visible:6",
        ),
        (CheckCode::PhoneNotVisible, "phone_not_visible"),
        (CheckCode::ReaderUnsupported, "reader_unsupported"),
        (CheckCode::EmailNotSupported, "email_not_supported"),
    ] {
        assert_eq!(code.to_string(), name);
    }
    let ids: Vec<&str> = CheckId::ALL.iter().map(|id| id.as_str()).collect();
    assert_eq!(ids, ["v0", "v1", "v2", "v3", "v4", "v5"]);
    assert_eq!(
        [FactResult::Pass, FactResult::Fail, FactResult::NotRun].map(FactResult::as_str),
        ["pass", "fail", "not_run"]
    );
    assert_eq!(Pass.to_string(), "pass");
    assert_eq!(NotRun.to_string(), "not_run");
    assert_eq!(
        Fail(CheckCode::HoursLineNotVisible(2)).to_string(),
        "fail:hours_line_not_visible:2"
    );
}
