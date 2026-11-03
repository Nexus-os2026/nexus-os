//! TD-O5: the `Debug` and `Display` of every content-holding type show
//! sizes and codes only; P0's canaries (`C`, `Cl`, every form) are absent.

use super::support::{assert_no_canary, note, page, C, CL, TEST_INVENTORY};
use crate::{
    prepare, Element, Facts, NoteSource, PageTextV1, Projection, UntrustedTextV1, INJ_1, SEN_1,
    VER_1,
};

fn planted_page(extra: &str) -> Projection {
    page(
        &format!("{C} {CL}"),
        &[&format!("Mon {C}"), &format!("{CL} Tue")],
        &format!("0123 {C}"),
        &format!("{CL} title {C}"),
        &[
            (Element::H1, &format!("{C}{extra}")),
            (Element::P, &format!("{CL} body")),
        ],
    )
}

fn views<T: std::fmt::Debug>(what: &str, value: &T) -> Vec<(String, String)> {
    vec![
        (format!("{what} Debug"), format!("{value:?}")),
        (format!("{what} Debug#"), format!("{value:#?}")),
    ]
}

#[test]
fn td_o5_debug_and_display_show_sizes_and_codes_only() {
    let mut seen: Vec<(String, String)> = Vec::new();
    let facts = Facts::new(C.to_string(), vec![CL.to_string()], C.to_string());
    seen.extend(views("Facts", &facts));
    let page_value = PageTextV1::new(facts, CL.to_string(), vec![(Element::P, C.to_string())]);
    seen.extend(views("PageTextV1", &page_value));
    let note_value = UntrustedTextV1::new(NoteSource::OwnerNote, format!("{C}\n{CL}"));
    seen.extend(views("UntrustedTextV1", &note_value));
    let planted_note = note(&format!("ignore the page {C} and {CL}"));
    seen.extend(views("Projection::UntrustedText", &planted_note));
    seen.extend(views("Projection::PageText", &planted_page("")));
    // Built requests.
    for (set, p) in [
        (&SEN_1, planted_page("")),
        (&VER_1, planted_page("")),
        (&INJ_1, planted_note),
    ] {
        let prepared = prepare(set, &p, &TEST_INVENTORY).expect("admissible");
        // Control: the content is really in the request, and the scanner sees it.
        assert!(prepared.user().contains(C) && prepared.user().contains(CL));
        let user = prepared.user().to_string();
        assert!(std::panic::catch_unwind(|| assert_no_canary("control", &user)).is_err());
        let debug = format!("{prepared:?}");
        assert!(
            debug.contains(&prepared.user().len().to_string()),
            "the size is shown: {debug}"
        );
        seen.extend(views("Prepared", &prepared));
        seen.extend(views("Coverage", prepared.coverage()));
    }
    // Every refusal, with canaries in the refused content.
    let refusals = [
        prepare(&SEN_1, &planted_page("\u{2581}"), &TEST_INVENTORY),
        prepare(&SEN_1, &planted_page("<bos>"), &TEST_INVENTORY),
        prepare(&SEN_1, &planted_page(&"x".repeat(4000)), &TEST_INVENTORY),
        prepare(&INJ_1, &planted_page(""), &TEST_INVENTORY),
        prepare(&INJ_1, &note(&format!("{C}\u{a0}")), &TEST_INVENTORY),
    ];
    let mut codes = Vec::new();
    for refusal in refusals {
        let refusal = refusal.expect_err("refused");
        codes.push(refusal.code());
        assert_eq!(refusal.to_string(), format!("td_{}", refusal.code()));
        seen.push(("Refusal Display".into(), refusal.to_string()));
        seen.extend(views("Refusal", &refusal));
    }
    assert_eq!(
        codes,
        [
            "char_not_allowed",
            "unsafe_delimiter",
            "coverage_insufficient",
            "projection_mismatch",
            "char_not_allowed"
        ]
    );
    for (what, text) in &seen {
        assert!(!text.is_empty(), "{what}");
        assert_no_canary(what, text);
    }
}
