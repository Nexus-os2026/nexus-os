//! M3-O16 (design M3 §5.6, pre-registered): the S1a facts API added in
//! M3-A. `check_facts` equals `facts_report(..).first_error()` and an
//! independent restatement of S1a's frozen order (FC-1 heading, FC-1 title,
//! FC-2 by index, FC-3, FC-4) on S-O10's pages and on 2,000 seeded pages;
//! every check is evaluated after an earlier failure, every hours line
//! included; the accessors borrow the approved values; `ApprovedFacts`'
//! `Debug` stays redacted and the report's `Debug` shows booleans only; the
//! policy hash is unchanged (S-O1 stays green beside it).

use super::facts_fixtures::*;
use super::*;
use FactCode as F;

/// `policy_hash()` at S1a-B (`4be8956f`), before M3-A: the facts API is
/// not policy, so it must not change.
const POLICY_HASH_AT_S1A_B: &str =
    "cca57a6d9204010ea957b51eb3def9e7c118f5d8922a21be54bfb62e85f5093e";

/// Every element text of `page` with its tag: each leaf's, then each of its
/// spans' (`Leaf::text`, the text S-O7 pins against html5ever).
fn element_texts(page: &Page) -> Vec<(Tag, String)> {
    fn spans(l: &Leaf, out: &mut Vec<(Tag, String)>) {
        for item in &l.inlines {
            let (tag, s) = match item {
                Inline::Strong(s) => (Tag::Strong, s),
                Inline::Em(s) => (Tag::Em, s),
                Inline::Small(s) => (Tag::Small, s),
                Inline::Text(_) | Inline::Br => continue,
            };
            out.push((tag, s.text()));
            spans(s, out);
        }
    }
    fn leaf(tag: Tag, l: &Leaf, out: &mut Vec<(Tag, String)>) {
        out.push((tag, l.text()));
        spans(l, out);
    }
    fn blocks(bs: &[Block], out: &mut Vec<(Tag, String)>) {
        for b in bs {
            match b {
                Block::H1(l) => leaf(Tag::H1, l, out),
                Block::H2(l) => leaf(Tag::H2, l, out),
                Block::H3(l) => leaf(Tag::H3, l, out),
                Block::P(l) => leaf(Tag::P, l, out),
                Block::Address(l) => leaf(Tag::Address, l, out),
                Block::Ul(list) | Block::Ol(list) => {
                    for li in &list.items {
                        leaf(Tag::Li, li, out);
                    }
                }
            }
        }
    }
    let mut out = Vec::new();
    if let Some(h) = page.header() {
        blocks(&h.blocks, &mut out);
    }
    for item in &page.main().items {
        match item {
            MainItem::Section(s) => blocks(&s.blocks, &mut out),
            MainItem::Block(b) => blocks(std::slice::from_ref(b), &mut out),
        }
    }
    if let Some(f) = page.footer() {
        blocks(&f.blocks, &mut out);
    }
    out
}

/// S1a design v2 §5's FC-4 tokens, restated: the digits of each maximal
/// run of ASCII digits and separators that holds at least the threshold
/// of digits.
fn phone_like(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut digits = String::new();
    for c in text.chars().chain(std::iter::once('x')) {
        if c.is_ascii_digit() {
            digits.push(c);
        } else if !POLICY_V1.fc4_separators.contains(&c) {
            if digits.len() >= POLICY_V1.fc4_threshold {
                found.push(digits.clone());
            }
            digits.clear();
        }
    }
    found
}

/// FC-1 to FC-4 restated from S1a design v2 §5, each check on its own.
fn independent_report(page: &Page, name: &str, hours: &[&str], phone: &str) -> FactsReport {
    let texts = element_texts(page);
    let leaves = || {
        texts
            .iter()
            .filter(|(t, _)| !matches!(t, Tag::Strong | Tag::Em | Tag::Small))
    };
    let h1: Vec<&String> = leaves()
        .filter(|(t, _)| *t == Tag::H1)
        .map(|(_, s)| s)
        .collect();
    let digits: String = phone.chars().filter(char::is_ascii_digit).collect();
    FactsReport {
        name_heading: h1.len() == 1 && h1[0] == name,
        name_title: page.title() == name,
        hours: hours
            .iter()
            .map(|line| leaves().any(|(t, s)| matches!(t, Tag::Li | Tag::P) && s == line))
            .collect(),
        phone: texts.iter().any(|(_, s)| s == phone),
        no_foreign_number: std::iter::once(page.title())
            .chain(leaves().map(|(_, s)| s.as_str()))
            .all(|t| phone_like(t).iter().all(|d| *d == digits)),
    }
}

/// S1a's frozen order over a report, restated.
fn frozen_first_error(r: &FactsReport) -> Result<(), FactCode> {
    if !r.name_heading {
        return Err(F::NameNotHeading);
    }
    if !r.name_title {
        return Err(F::NameNotTitle);
    }
    if let Some(i) = r.hours.iter().position(|ok| !ok) {
        return Err(F::HoursLineMissing { index: i as u8 });
    }
    if !r.phone {
        return Err(F::PhoneMissing);
    }
    if !r.no_foreign_number {
        return Err(F::ForeignNumber);
    }
    Ok(())
}

/// The restatement's code for `page`, which must also be `check_facts`'.
fn restated(
    label: &str,
    page: &str,
    name: &str,
    hours: &[&str],
    phone: &str,
) -> Result<(), FactCode> {
    let parsed = check(page.as_bytes()).unwrap_or_else(|r| panic!("{label}: refused: {r}"));
    let code = frozen_first_error(&independent_report(&parsed, name, hours, phone));
    let f = facts(name, hours, phone);
    assert_eq!(check_facts(&parsed, &f), code, "{label}: check_facts");
    code
}

/// The report equals the restated one on `page`, and its first error is
/// the restatement's code and `check_facts`'. Returns the code.
fn agree(label: &str, page: &str, name: &str, hours: &[&str], phone: &str) -> Result<(), FactCode> {
    let code = restated(label, page, name, hours, phone);
    let parsed = check(page.as_bytes()).unwrap();
    let report = facts_report(&parsed, &facts(name, hours, phone));
    assert_eq!(
        report,
        independent_report(&parsed, name, hours, phone),
        "{label}: the report"
    );
    assert_eq!(report.first_error(), code, "{label}: first_error");
    code
}

#[test]
fn m3_o16_accessors_borrow_the_approved_values_and_debug_stays_redacted() {
    let f = reference_facts();
    assert_eq!(f.name(), NAME);
    assert_eq!(
        f.hours().iter().map(String::as_str).collect::<Vec<_>>(),
        HOURS
    );
    assert_eq!(f.phone(), PHONE);
    let london = london_facts();
    assert_eq!(london.phone(), LONDON_PHONE);
    // Borrowed: each result's lifetime is the facts' own, and repeated
    // calls give the same storage.
    let _: for<'a> fn(&'a ApprovedFacts) -> &'a str = ApprovedFacts::name;
    let _: for<'a> fn(&'a ApprovedFacts) -> &'a [String] = ApprovedFacts::hours;
    let _: for<'a> fn(&'a ApprovedFacts) -> &'a str = ApprovedFacts::phone;
    assert!(std::ptr::eq(f.name(), f.name()));
    assert!(std::ptr::eq(f.hours(), f.hours()));
    assert!(std::ptr::eq(f.phone(), f.phone()));
    assert_eq!(f.hours().len(), 3);
    assert_eq!(
        format!("{f:?}"),
        format!("ApprovedFacts {{ hash: {:?} }}", hex::encode(f.hash()))
    );
}

#[test]
fn m3_o16_every_check_is_evaluated_after_an_earlier_failure() {
    let page = edit("<h1>Rosie&#39;s Bakery</h1>", "<p>Rosie&#39;s Bakery</p>");
    let page = edit_in(
        &page,
        "<title>Rosie&#39;s Bakery</title>",
        "<title>Rosie&#39;s Bakes</title>",
    );
    let page = edit_in(&page, "<li>Monday to Friday: 07:00 – 17:00</li>\n", "");
    let page = edit_in(&page, "<li>Sunday: closed</li>\n", "");
    let page = edit_in(&page, PHONE_ELEMENT, "<strong>01632 960999</strong>");
    let parsed = check(page.as_bytes()).expect("the page is inert");
    assert_eq!(
        check_facts(&parsed, &reference_facts()),
        Err(F::NameNotHeading)
    );
    let report = facts_report(&parsed, &reference_facts());
    assert_eq!(
        report,
        FactsReport {
            name_heading: false,
            name_title: false,
            hours: vec![false, true, false],
            phone: false,
            no_foreign_number: false,
        }
    );
    assert_eq!(report.first_error(), Err(F::NameNotHeading));

    // Only the last hours line missing, after two that pass.
    let page = edit("<li>Sunday: closed</li>\n", "");
    let parsed = check(page.as_bytes()).expect("the page is inert");
    let report = facts_report(&parsed, &reference_facts());
    assert_eq!(report.hours, vec![true, true, false]);
    assert!(report.name_heading && report.name_title && report.phone && report.no_foreign_number);
    assert_eq!(report.first_error(), Err(F::HoursLineMissing { index: 2 }));

    let parsed = check(REFERENCE.as_bytes()).expect("the reference page");
    let report = facts_report(&parsed, &reference_facts());
    assert_eq!(
        report,
        FactsReport {
            name_heading: true,
            name_title: true,
            hours: vec![true; 3],
            phone: true,
            no_foreign_number: true,
        }
    );
    assert_eq!(report.first_error(), Ok(()));
}

#[test]
fn m3_o16_check_facts_is_the_report_s_first_error_on_s_o10() {
    let h1 = "<h1>Rosie&#39;s Bakery</h1>";
    let phone_p = "<p>Phone: <strong>01632 960123</strong></p>";
    let cases: Vec<(&str, String, Result<(), FactCode>)> = vec![
        ("reference", REFERENCE.to_string(), Ok(())),
        (
            "Monday 07:00-17:00",
            edit("<li>Sunday: closed</li>", "<li>Monday 07:00-17:00</li>"),
            Err(F::HoursLineMissing { index: 2 }),
        ),
        (
            "the phone split across spans inside one leaf",
            edit(phone_p, "<p><strong>01632</strong> 960123</p>"),
            Ok(()),
        ),
        (
            "the name only in a p",
            edit(h1, "<p>Rosie&#39;s Bakery</p>"),
            Err(F::NameNotHeading),
        ),
        (
            "two h1",
            edit("<h2>Contact</h2>", h1),
            Err(F::NameNotHeading),
        ),
        (
            "title differs from the name",
            edit(
                "<title>Rosie&#39;s Bakery</title>",
                "<title>Rosie&#39;s Bakes</title>",
            ),
            Err(F::NameNotTitle),
        ),
        (
            "hours line 2 missing",
            edit("<li>Saturday: 08:00 – 14:00</li>\n", ""),
            Err(F::HoursLineMissing { index: 1 }),
        ),
        (
            "an hours line only as a substring",
            edit(
                "<li>Monday to Friday: 07:00 – 17:00</li>",
                "<li>Open Monday to Friday: 07:00 – 17:00</li>",
            ),
            Err(F::HoursLineMissing { index: 0 }),
        ),
        (
            "a phone superstring",
            edit(PHONE_ELEMENT, "<strong>01632 9601234</strong>"),
            Err(F::PhoneMissing),
        ),
        (
            "the phone split across separate blocks",
            edit(phone_p, "<ul>\n<li>01632</li>\n<li>960123</li>\n</ul>"),
            Err(F::PhoneMissing),
        ),
        (
            "a price row",
            with_paragraph(REFERENCE, "1.50 2.50 3.50 4.50"),
            Err(F::ForeignNumber),
        ),
        (
            "a slashed date",
            with_paragraph(REFERENCE, "Baked on 3/11/2026"),
            Err(F::ForeignNumber),
        ),
        (
            "a foreign number across a br",
            edit(
                "<address>12 Mill Lane<br>Exampleton</address>",
                "<address>01632<br>960999</address>",
            ),
            Err(F::ForeignNumber),
        ),
    ];
    let colon = ["Monday 07:00-17:00"];
    let colon_page = edit("<li>Sunday: closed</li>", "<li>Monday 07:00-17:00</li>");
    let london: Vec<(&str, String, Result<(), FactCode>)> = [
        (
            "an en-dash spelling of the approved number",
            "Or call 020–7946–0123",
            Ok(()),
        ),
        ("en dashes", "Fax 020–7946–0999", Err(F::ForeignNumber)),
        (
            "brackets and a dot",
            "Fax (020) 7946.0999",
            Err(F::ForeignNumber),
        ),
        (
            "an international prefix",
            "Fax +44 20 7946 0999",
            Err(F::ForeignNumber),
        ),
    ]
    .into_iter()
    .map(|(label, number, code)| (label, with_paragraph(&london_page(), number), code))
    .collect();
    // The restatement and S1a's check_facts give each case's code...
    for (label, page, code) in &cases {
        assert_eq!(restated(label, page, NAME, &HOURS, PHONE), *code, "{label}");
    }
    assert_eq!(restated("colon", &colon_page, NAME, &colon, PHONE), Ok(()));
    for (label, page, code) in &london {
        assert_eq!(
            restated(label, page, NAME, &HOURS, LONDON_PHONE),
            *code,
            "{label}"
        );
    }
    // ...and so does the report, which equals the restated report.
    for (label, page, code) in &cases {
        assert_eq!(agree(label, page, NAME, &HOURS, PHONE), *code, "{label}");
    }
    assert_eq!(agree("colon", &colon_page, NAME, &colon, PHONE), Ok(()));
    for (label, page, code) in &london {
        assert_eq!(
            agree(label, page, NAME, &HOURS, LONDON_PHONE),
            *code,
            "{label}"
        );
    }
}

/// One seeded page: the reference page's parts, each kept or broken.
fn seeded_page(rng: &mut Rng) -> String {
    let h1 = *rng.pick(&[
        "<h1>Rosie&#39;s Bakery</h1>\n",
        "<h1>Rosie&#39;s Bakery</h1>\n",
        "<h1>Rosie&#39;s Bakery</h1>\n",
        "<p>Rosie&#39;s Bakery</p>\n",
        "<h1>Rosie&#39;s bakery</h1>\n",
        "",
    ]);
    let title = *rng.pick(&[
        "Rosie&#39;s Bakery",
        "Rosie&#39;s Bakery",
        "Rosie&#39;s Bakery",
        "Rosie&#39;s Bakes",
    ]);
    let second_h1 = *rng.pick(&[
        "",
        "",
        "",
        "",
        "<h1>Rosie&#39;s Bakery</h1>\n",
        "<h1>Bread</h1>\n",
    ]);
    let mut list = String::new();
    let mut outside = String::new();
    for line in HOURS {
        match rng.below(8) {
            0 => {}
            1 => list.push_str(&format!("<li>Open {line}</li>\n")),
            2 => outside.push_str(&format!("<h3>{line}</h3>\n")),
            3 => outside.push_str(&format!("<p>{line}</p>\n")),
            _ => list.push_str(&format!("<li>{line}</li>\n")),
        }
    }
    let phone = *rng.pick(&[
        "<p>Phone: <strong>01632 960123</strong></p>\n",
        "<p>Phone: <strong>01632 960123</strong></p>\n",
        "<p><strong>01632</strong> 960123</p>\n",
        "<p>Phone: <strong>01632 9601234</strong></p>\n",
        "<ul>\n<li>01632</li>\n<li>960123</li>\n</ul>\n",
        "<p>Phone: ask in the shop</p>\n",
    ]);
    let extra = *rng.pick(&[
        "",
        "",
        "<p>Fax 01632 960999</p>\n",
        "<p>Or call 01632–960123</p>\n",
        "<p>1.50 2.50 3.50 4.50</p>\n",
        "<address>01632<br>960999</address>\n",
    ]);
    let head = &REFERENCE[..REFERENCE.find("<title>").unwrap()];
    let style =
        &REFERENCE[REFERENCE.find("</title>").unwrap()..REFERENCE.find("<header>").unwrap()];
    format!(
        "{head}<title>{title}</title>{}<header>\n{h1}<p class=\"lead\">Fresh bread.</p>\n</header>\n\
         <main>\n{second_h1}<section id=\"hours\">\n<h2>Opening hours</h2>\n<ul class=\"plain\">\n{list}\
         <li>Bank holidays: closed</li>\n</ul>\n{outside}</section>\n<section id=\"contact\">\n{phone}{extra}\
         </section>\n</main>\n</body>\n</html>\n",
        &style["</title>".len()..]
    )
}

#[test]
fn m3_o16_check_facts_is_the_report_s_first_error_on_2000_seeded_pages() {
    let mut rng = Rng::new(0x5EED_0316);
    let pages: Vec<String> = (0..2_000).map(|_| seeded_page(&mut rng)).collect();
    // The corpus, judged by the restatement alone: every outcome occurs,
    // and most pages fail more than one check, so the order is exercised.
    let mut codes = std::collections::BTreeMap::<u16, usize>::new();
    let mut several = 0;
    let mut restated_codes = Vec::new();
    for (k, page) in pages.iter().enumerate() {
        let code = restated(&format!("seeded page {k}"), page, NAME, &HOURS, PHONE);
        restated_codes.push(code);
        let parsed = check(page.as_bytes()).unwrap();
        let r = independent_report(&parsed, NAME, &HOURS, PHONE);
        *codes
            .entry(code.map_or_else(|c| c.position(), |()| 99))
            .or_default() += 1;
        let failing = [
            !r.name_heading,
            !r.name_title,
            !r.phone,
            !r.no_foreign_number,
        ]
        .iter()
        .filter(|x| **x)
        .count()
            + r.hours.iter().filter(|ok| !**ok).count();
        if failing >= 2 {
            several += 1;
        }
    }
    for code in [
        F::NameNotHeading,
        F::NameNotTitle,
        F::HoursLineMissing { index: 0 },
        F::PhoneMissing,
        F::ForeignNumber,
    ] {
        let n = codes.get(&code.position()).copied().unwrap_or(0);
        assert!(n >= 20, "{code}: {codes:?}");
    }
    assert!(
        codes.get(&99).copied().unwrap_or(0) >= 20,
        "passes: {codes:?}"
    );
    assert!(several >= 1_000, "{several} pages fail several checks");

    for (k, (page, code)) in pages.iter().zip(&restated_codes).enumerate() {
        let label = format!("seeded page {k}");
        assert_eq!(agree(&label, page, NAME, &HOURS, PHONE), *code, "{label}");
    }
}

#[test]
fn m3_o16_the_report_s_debug_shows_booleans_only() {
    let canary = "NXCANARY-r3p0rtc4n4ry";
    let page = with_paragraph(REFERENCE, canary);
    let parsed = check(page.as_bytes()).expect("the page is inert");
    let report = facts_report(&parsed, &reference_facts());
    let shown = format!("{report:?}");
    assert_eq!(
        shown,
        "FactsReport { name_heading: true, name_title: true, hours: [true, true, true], \
         phone: true, no_foreign_number: true }"
    );
    for secret in [canary, "Rosie", "Monday", "01632", "960123"] {
        assert!(!shown.contains(secret), "{shown}");
    }
}

#[test]
fn m3_o16_the_policy_hash_is_unchanged() {
    assert_eq!(hex::encode(policy_hash()), POLICY_HASH_AT_S1A_B);
}
