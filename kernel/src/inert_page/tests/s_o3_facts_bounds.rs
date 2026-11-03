//! S-O3 (S1a-B part): the facts boundary pairs, accepted at the limit and
//! refused one past it: 7 hours lines; a phone of 7 and of 15 digits. Also
//! the name (80 characters) and value (120 characters) caps. At each limit
//! the facts are built and a page carrying them passes with them.

use super::facts_fixtures::*;
use super::*;

/// `page` passes the inert check and the facts check with `facts`.
fn passes_with(label: &str, page: &str, facts: &ApprovedFacts) {
    let v = PageVerdict::evaluate(page.as_bytes(), Some(facts));
    assert_eq!(v.outcome, PageOutcome::Passed, "{label}");
    assert!(v.passed(), "{label}");
}

#[test]
fn s_o3_facts_boundaries_accept_at_the_limit_and_refuse_one_past() {
    // 7 hours lines; 8 are refused.
    let lines: Vec<String> = (1..=8).map(|k| format!("Day {k}: closed")).collect();
    let seven: Vec<&str> = lines[..7].iter().map(String::as_str).collect();
    let eight: Vec<&str> = lines.iter().map(String::as_str).collect();
    let at = facts(NAME, &seven, PHONE);
    let items: String = seven.iter().map(|l| format!("<li>{l}</li>\n")).collect();
    let page = edit(
        "<li>Sunday: closed</li>\n",
        &format!("<li>Sunday: closed</li>\n{items}"),
    );
    passes_with("7 hours lines", &page, &at);
    let v = PageVerdict::evaluate(page.as_bytes(), Some(&at));
    assert_eq!(
        v.facts_line().as_deref(),
        Some("facts present as given: name, hours (7 lines), phone")
    );
    assert_eq!(
        ApprovedFacts::new(NAME, &eight, PHONE),
        Err(FactCode::FactsInvalid),
        "8 hours lines"
    );
    assert_eq!(
        ApprovedFacts::new(NAME, &[], PHONE),
        Err(FactCode::FactsInvalid),
        "no hours line"
    );

    // A phone of 7 and of 15 digits; 6 and 16 are refused.
    for (label, phone, past) in [
        ("7 digits", "123 4567", "123 456"),
        ("15 digits", "+44 1234 5678 90123", "+44 1234 5678 901234"),
    ] {
        let at = facts(NAME, &HOURS, phone);
        let page = edit(PHONE_ELEMENT, &format!("<strong>{phone}</strong>"));
        passes_with(label, &page, &at);
        assert_eq!(
            ApprovedFacts::new(NAME, &HOURS, past),
            Err(FactCode::PhoneFormat),
            "{label}: one past"
        );
    }

    // The name: 80 characters (and so the title); 81 are refused.
    let name = |n: usize| format!("{} {}", "R".repeat(n - 41), "B".repeat(40));
    let long = name(80);
    let at = facts(&long, &HOURS, PHONE);
    let page = edit_in(
        &edit(
            "<title>Rosie&#39;s Bakery</title>",
            &format!("<title>{long}</title>"),
        ),
        "<h1>Rosie&#39;s Bakery</h1>",
        &format!("<h1>{long}</h1>"),
    );
    passes_with("an 80-character name", &page, &at);
    assert_eq!(
        ApprovedFacts::new(&name(81), &HOURS, PHONE),
        Err(FactCode::FactsInvalid),
        "an 81-character name"
    );

    // A value: 120 characters; 121 are refused (characters, not bytes).
    let line = |n: usize| format!("{} {}", "é".repeat(40), "a".repeat(n - 41));
    assert!(ApprovedFacts::new(NAME, &[&line(120)], PHONE).is_ok());
    assert_eq!(
        ApprovedFacts::new(NAME, &[&line(121)], PHONE),
        Err(FactCode::FactsInvalid),
        "a 121-character hours line"
    );
}
