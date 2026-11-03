//! TD-O4: the byte budget (§4.4). Every §4.4 golden and Appendix A literal
//! (Codex re-check 3's binding table); 1,000 seeded formula comparisons;
//! exactly at the bound → built, one byte over → coverage insufficient
//! (every reading Unsettled, nothing built, so nothing to charge); the
//! longest correction is the one used.

use super::appendix_a::*;
use super::support::{
    min_page, note, note_formula, page, page_formula, ref_page, Rng, REF_HOURS, REF_NAME,
    REF_PHONE, TEST_INVENTORY,
};
use crate::{
    correction_sentence, prepare, render_schema, render_system, DecodeCode, Element, Facts,
    PageTextV1, Projection, QuestionSet, Reading, Refusal, INJ_1, MAX_CONTENT_BYTES, SEN_1, VER_1,
};

fn system(desc: &str, lines: &[&str], shape: &str) -> String {
    format!("{PREAMBLE}\n\n{RULE}\n\n{desc}\n\nQuestions:\n{}\n\n{SHAPE_INTRO}\n{shape}\n{SHAPE_ENDING}", lines.join("\n"))
}

#[test]
fn td_o4_appendix_a_texts_are_golden() {
    let sizes = |xs: &[&str]| xs.iter().map(|x| x.len()).collect::<Vec<_>>();
    assert_eq!(
        sizes(&[PREAMBLE, RULE, DESC_PAGE, DESC_UNTRUSTED]),
        [267, 234, 290, 132]
    );
    assert_eq!(sizes(LINES_SEN_1), [303, 315, 284, 254]);
    assert_eq!(sizes(LINES_VER_1), [210, 241, 175]);
    assert_eq!(sizes(LINES_INJ_1), [337]);
    assert_eq!(
        sizes(&[
            SHAPE_INTRO,
            SHAPE_SEN_1,
            SHAPE_VER_1,
            SHAPE_INJ_1,
            SHAPE_ENDING
        ]),
        [86, 79, 48, 18, 72]
    );
    for (set, desc, lines, shape, block, shape_block) in [
        (&SEN_1, DESC_PAGE, LINES_SEN_1, SHAPE_SEN_1, 1170, 239),
        (&VER_1, DESC_PAGE, LINES_VER_1, SHAPE_VER_1, 639, 208),
        (&INJ_1, DESC_UNTRUSTED, LINES_INJ_1, SHAPE_INJ_1, 348, 178),
    ] {
        assert_eq!(format!("Questions:\n{}", lines.join("\n")).len(), block);
        assert_eq!(
            format!("{SHAPE_INTRO}\n{shape}\n{SHAPE_ENDING}").len(),
            shape_block
        );
        assert_eq!(
            render_system(set).unwrap(),
            system(desc, lines, shape),
            "{}",
            set.id().as_str()
        );
    }
    assert_eq!((CORRECTION_PREFIX.len(), CORRECTION_SUFFIX.len()), (45, 85));
    for (code, (name, phrase, n)) in DecodeCode::ALL.iter().zip(PHRASES) {
        assert_eq!(
            (code.code(), code.phrase(), code.phrase().len()),
            (name, phrase, n)
        );
        assert_eq!(
            correction_sentence(*code),
            format!("{CORRECTION_PREFIX}{phrase}{CORRECTION_SUFFIX}")
        );
    }
}

#[test]
fn td_o4_system_schema_and_maximum_user_goldens() {
    let goldens: [(&QuestionSet, usize, usize, usize, usize, &str); 3] = [
        (&SEN_1, 2208, 2390, 286, 1546, SCHEMA_SEN_1),
        (&VER_1, 1646, 1828, 210, 2108, SCHEMA_VER_1),
        (&INJ_1, 1167, 1349, 122, 2587, SCHEMA_INJ_1),
    ];
    let longest = DecodeCode::ALL
        .iter()
        .map(|c| 2 + correction_sentence(*c).len())
        .max()
        .unwrap();
    assert_eq!(longest, 182, "2 + 45 + 50 + 85");
    assert_eq!(DecodeCode::BadSum.phrase().len(), 50);
    for (set, attempt1, attempt2, schema, max_user, schema_text) in goldens {
        let id = set.id().as_str();
        let first = render_system(set).unwrap();
        assert_eq!(first.len(), attempt1, "{id} attempt 1");
        let second = DecodeCode::ALL
            .iter()
            .map(|c| crate::render_correction_system(set, *c).unwrap().len())
            .max()
            .unwrap();
        assert_eq!(second, attempt2, "{id} attempt 2, max");
        assert_eq!(second, first.len() + longest);
        assert_eq!(render_schema(set).len(), schema, "{id} schema");
        assert_eq!(render_schema(set), schema_text, "{id} schema text");
        assert_eq!(MAX_CONTENT_BYTES - attempt2, max_user, "{id} max user");
    }
    assert_eq!(MAX_CONTENT_BYTES, 3_936);
}

/// The text array's serialized size and content size in a built user.
fn text_array(user: &str) -> (usize, usize) {
    let json = &user["<document>\n".len()..user.len() - "\n</document>".len()];
    let at = json.find("\"text\":[").expect("text array") + "\"text\":".len();
    let serialized = json.len() - 1 - at;
    (serialized, serialized - 2)
}

fn built(set: &'static QuestionSet, p: &Projection) -> (usize, usize, usize) {
    let prepared = prepare(set, p, &TEST_INVENTORY).expect("built");
    assert!(prepared.coverage().complete());
    assert_eq!(prepared.coverage().omitted_text_bytes(), 0);
    assert_eq!(
        prepared.coverage().sent_text_bytes(),
        prepared.coverage().source_text_bytes()
    );
    let longest = DecodeCode::ALL
        .iter()
        .map(|c| prepared.correction_system(*c).len())
        .max()
        .unwrap();
    assert_eq!(
        longest,
        prepared.system().len() + 182,
        "the longest correction is the one used"
    );
    assert_eq!(
        prepared.attempt2_max_bytes(),
        longest + prepared.user().len()
    );
    assert_eq!(
        prepared.coverage().needed_bytes(),
        prepared.attempt2_max_bytes()
    );
    assert_eq!(prepared.coverage().user_bytes(), prepared.user().len());
    (
        prepared.user().len(),
        prepared.attempt1_bytes(),
        prepared.attempt2_max_bytes(),
    )
}

#[test]
fn td_o4_skeletons_reference_and_minimum_fixtures() {
    let empty_page = page("", &[], "", "", &[]);
    assert_eq!(built(&SEN_1, &empty_page).0, 131, "page skeleton");
    assert_eq!(built(&INJ_1, &note("")).0, 105, "note skeleton");
    // The S1a reference page.
    let reference = ref_page();
    assert_eq!(
        prepare(&SEN_1, &reference, &TEST_INVENTORY).unwrap().user(),
        REF_USER
    );
    assert_eq!(built(&SEN_1, &reference), (579, 2787, 2969));
    assert_eq!(built(&VER_1, &reference), (579, 2225, 2407));
    assert_eq!(MAX_CONTENT_BYTES - 2969, 967);
    assert_eq!(MAX_CONTENT_BYTES - 2407, 1529);
    assert_eq!(text_array(REF_USER), (330, 328));
    // The frozen minimum fixture.
    let minimum = min_page();
    assert_eq!(
        prepare(&VER_1, &minimum, &TEST_INVENTORY).unwrap().user(),
        MIN_USER
    );
    assert_eq!(built(&VER_1, &minimum), (179, 1825, 2007));
    assert_eq!(built(&SEN_1, &minimum), (179, 2387, 2569));
    assert_eq!(text_array(MIN_USER), (38, 36));
    // A one-byte note.
    assert_eq!(built(&INJ_1, &note("a")), (106, 1273, 1455));
}

fn max_facts() -> (String, Vec<String>, String) {
    let mut phone = String::from("+(1");
    for i in 0..14 {
        phone.push_str(if i % 2 == 0 { ") 1" } else { " (1" });
    }
    assert_eq!(phone.len(), 45);
    ("N".repeat(80), vec!["H".repeat(120); 7], phone)
}

fn one_block(name: &str, hours: &[String], phone: &str, title: &str, content: usize) -> Projection {
    // One `["p", t]` block: content = 1 + 7 + enc(t).
    let t = "x".repeat(content - 8);
    Projection::PageText(PageTextV1::new(
        Facts::new(name.into(), hours.to_vec(), phone.into()),
        title.into(),
        vec![(Element::P, t)],
    ))
}

fn assert_coverage_refusal(set: &'static QuestionSet, p: &Projection, user: usize, source: usize) {
    let refusal = prepare(set, p, &TEST_INVENTORY).expect_err("over the bound");
    assert_eq!(refusal.code(), "coverage_insufficient");
    assert_eq!(refusal.outcome(), "refused:coverage_insufficient");
    assert_eq!(refusal.to_string(), "td_coverage_insufficient");
    let Refusal::CoverageInsufficient { coverage, readings } = refusal else {
        panic!("not a coverage refusal")
    };
    let unsettled: Vec<_> = set.keys().map(|k| (k, Reading::Unsettled)).collect();
    assert_eq!(readings, unsettled, "every reading Unsettled, set in code");
    assert!(!coverage.complete());
    assert_eq!(
        (coverage.sent_text_bytes(), coverage.omitted_text_bytes()),
        (0, source)
    );
    assert_eq!(coverage.source_text_bytes(), source);
    assert_eq!(coverage.user_bytes(), user);
    assert_eq!(coverage.needed_bytes(), MAX_CONTENT_BYTES + 1);
}

#[test]
fn td_o4_exactly_at_the_bound_is_built_and_one_byte_over_is_refused() {
    let hours: Vec<String> = REF_HOURS.iter().map(|h| h.to_string()).collect();
    // The reference facts and title: content allowances 1,295 and 1,857.
    for (set, content, serialized) in [(&SEN_1, 1295, 1297), (&VER_1, 1857, 1859)] {
        let at = one_block(REF_NAME, &hours, REF_PHONE, REF_NAME, content);
        let (user, _, total) = built(set, &at);
        assert_eq!(total, MAX_CONTENT_BYTES);
        assert_eq!(
            text_array(prepare(set, &at, &TEST_INVENTORY).unwrap().user()),
            (serialized, content)
        );
        let over = one_block(REF_NAME, &hours, REF_PHONE, REF_NAME, content + 1);
        assert_coverage_refusal(set, &over, user + 1, REF_NAME.len() + content + 1 - 8);
    }
    // MTD-5: attempt 1 alone would fit, but the longest correction does not.
    let over = one_block(REF_NAME, &hours, REF_PHONE, REF_NAME, 1296);
    let refusal = prepare(&SEN_1, &over, &TEST_INVENTORY).expect_err("attempt 2 does not fit");
    let Refusal::CoverageInsufficient { coverage, .. } = refusal else {
        panic!("not a coverage refusal")
    };
    assert_eq!(coverage.user_bytes(), 1547);
    assert!(render_system(&SEN_1).unwrap().len() + coverage.user_bytes() < MAX_CONTENT_BYTES);
    // Facts at their ASCII construction maxima and an 80-character title.
    let (name, hours, phone) = max_facts();
    let title = "T".repeat(80);
    let skeleton = one_block(&name, &hours, &phone, &title, 8);
    let user = prepare(&VER_1, &skeleton, &TEST_INVENTORY)
        .unwrap()
        .user()
        .to_string();
    let json = &user[11..user.len() - 12];
    let facts = &json[json.find("\"facts\":").unwrap() + 8..json.find(",\"title\":").unwrap()];
    assert_eq!(facts.len(), 1018, "facts JSON");
    assert_eq!(user.len() - text_array(&user).1, 1196, "fixed user part");
    for (set, content, serialized) in [(&SEN_1, 350, 352), (&VER_1, 912, 914)] {
        let at = one_block(&name, &hours, &phone, &title, content);
        assert_eq!(built(set, &at).2, MAX_CONTENT_BYTES);
        assert_eq!(
            text_array(prepare(set, &at, &TEST_INVENTORY).unwrap().user()),
            (serialized, content)
        );
        let over = one_block(&name, &hours, &phone, &title, content + 1);
        assert_coverage_refusal(set, &over, 1196 + content + 1, 80 + content + 1 - 8);
    }
    // Notes: enc(text) <= 2,482 under inj_1, ASCII or escape-heavy.
    assert_eq!(built(&INJ_1, &note(&"n".repeat(2482))).2, MAX_CONTENT_BYTES);
    assert_coverage_refusal(&INJ_1, &note(&"n".repeat(2483)), 2588, 2483);
    let heavy = format!("{}nnnn", "<".repeat(413));
    assert_eq!(built(&INJ_1, &note(&heavy)).2, MAX_CONTENT_BYTES);
    assert_coverage_refusal(&INJ_1, &note(&format!("{heavy}n")), 2588, 418);
}

#[test]
fn td_o4_the_formula_equals_the_built_length_on_1000_seeded_projections() {
    const PAGE_CHARS: &[char] = &[
        'a', 'Z', '0', ' ', '.', ':', '<', '>', '"', '\\', '\u{e9}', '\u{2013}', '\u{20ac}',
        '\u{2122}', '\u{2019}',
    ];
    const NOTE_CHARS: &[char] = &[
        'a', ' ', '<', '>', '"', '\\', '\n', '\t', '\u{e9}', '\u{2013}', '\u{20ac}',
    ];
    let mut rng = Rng(0x7d01_0004);
    let text = |rng: &mut Rng, chars: &[char], max: usize| -> String {
        let n = rng.below(max + 1);
        (0..n).map(|_| rng.pick(chars)).collect()
    };
    let elements = [
        Element::H1,
        Element::H2,
        Element::H3,
        Element::P,
        Element::Li,
        Element::Address,
    ];
    let (mut built_n, mut refused_n) = ([0usize; 3], [0usize; 3]);
    for n in 0..1000 {
        let (projection, formula, sets): (Projection, usize, Vec<(usize, &'static QuestionSet)>) =
            if n % 2 == 0 {
                let name = text(&mut rng, PAGE_CHARS, 40);
                let hours: Vec<String> = (0..rng.below(8))
                    .map(|_| text(&mut rng, PAGE_CHARS, 30))
                    .collect();
                let phone = text(&mut rng, PAGE_CHARS, 20);
                let title = text(&mut rng, PAGE_CHARS, 40);
                let blocks: Vec<(Element, String)> = (0..rng.below(16))
                    .map(|_| (rng.pick(&elements), text(&mut rng, PAGE_CHARS, 120)))
                    .collect();
                let formula = page_formula(&name, &hours, &phone, &title, &blocks);
                let p = Projection::PageText(PageTextV1::new(
                    Facts::new(name, hours, phone),
                    title,
                    blocks,
                ));
                (p, formula, vec![(0, &SEN_1), (1, &VER_1)])
            } else {
                let body = text(&mut rng, NOTE_CHARS, 3000);
                (note(&body), note_formula(&body), vec![(2, &INJ_1)])
            };
        for (k, set) in sets {
            let limit = MAX_CONTENT_BYTES - (render_system(set).unwrap().len() + 182);
            match prepare(set, &projection, &TEST_INVENTORY) {
                Ok(prepared) => {
                    assert_eq!(prepared.user().len(), formula, "projection {n}");
                    assert!(formula <= limit, "projection {n}");
                    built_n[k] += 1;
                }
                Err(Refusal::CoverageInsufficient { coverage, .. }) => {
                    assert_eq!(coverage.user_bytes(), formula, "projection {n}");
                    assert!(formula > limit, "projection {n}");
                    refused_n[k] += 1;
                }
                Err(other) => panic!("projection {n}: unexpected {other}"),
            }
        }
    }
    for k in 0..3 {
        assert!(
            built_n[k] >= 50 && refused_n[k] >= 50,
            "both sides exercised: {built_n:?} {refused_n:?}"
        );
    }
}
