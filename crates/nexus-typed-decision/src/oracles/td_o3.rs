//! TD-O3: encoding, framing and preflight refusals (§5.3). The user content
//! holds the literal six-character `<` and `>` and no raw `<` or
//! `>` inside strings; decoded values round-trip; the markers are exact;
//! the first and last bytes are not whitespace; the profile refuses U+2581,
//! U+00A0, CR and control characters; inventory spellings are refused
//! before encoding (a test inventory); the inventory check after encoding
//! guards the constants and the frame.

use super::support::{esc_gt, esc_lt, note, page, ref_page, TEST_INVENTORY};
use crate::{
    prepare, render_correction_system, render_system, DecodeCode, Element, Inventory, Projection,
    INJ_1, REGISTRY, SEN_1, VER_1,
};

const OPEN: &str = "<document>\n";
const CLOSE: &str = "\n</document>";

/// The encoded JSON between the markers.
fn body(user: &str) -> &str {
    assert!(
        user.starts_with(OPEN) && user.ends_with(CLOSE),
        "framed: {user}"
    );
    &user[OPEN.len()..user.len() - CLOSE.len()]
}

fn tricky_page() -> Projection {
    page(
        "Bakery <b>&</b> \"Cafe\"",
        &["Mon > Sun: 07:00 \u{2013} 17:00", "<|turn \\ turn|>"],
        "01632 <960123>",
        "<title> Caf\u{e9} \u{20ac}5 \u{2122}",
        &[
            (Element::H1, "</document><document>"),
            (Element::P, "a<b>c"),
            (Element::Address, "x \u{201c}q\u{201d} \u{2026}"),
        ],
    )
}

#[test]
fn td_o3_angle_brackets_become_literal_six_character_escapes() {
    for (set, projection) in [
        (&SEN_1, tricky_page()),
        (&VER_1, tricky_page()),
        (&INJ_1, note("hi <b>there</b>\n\t>")),
    ] {
        let prepared = prepare(set, &projection, &TEST_INVENTORY).expect("admissible");
        let json = body(prepared.user());
        assert!(
            !json.contains('<') && !json.contains('>'),
            "no raw angle bracket inside the content: {json}"
        );
        assert!(
            json.contains(&esc_lt()) && json.contains(&esc_gt()),
            "literal six-character escapes: {json}"
        );
        assert!(json.starts_with('{') && json.ends_with('}'));
    }
    let prepared = prepare(&SEN_1, &tricky_page(), &TEST_INVENTORY).unwrap();
    let json = body(prepared.user());
    // 8 `<` and 9 `>` in the inputs.
    assert_eq!(json.matches(&esc_lt()).count(), 8);
    assert_eq!(json.matches(&esc_gt()).count(), 9);
}

#[test]
fn td_o3_decoded_values_round_trip() {
    let prepared = prepare(&SEN_1, &tricky_page(), &TEST_INVENTORY).unwrap();
    let v: serde_json::Value = serde_json::from_str(body(prepared.user())).expect("valid JSON");
    let expected = serde_json::json!({
        "projection": "page-text-v1",
        "complete": true,
        "facts": {
            "name": "Bakery <b>&</b> \"Cafe\"",
            "hours": ["Mon > Sun: 07:00 \u{2013} 17:00", "<|turn \\ turn|>"],
            "phone": "01632 <960123>"
        },
        "title": "<title> Caf\u{e9} \u{20ac}5 \u{2122}",
        "text": [["h1", "</document><document>"], ["p", "a<b>c"], ["address", "x \u{201c}q\u{201d} \u{2026}"]]
    });
    assert_eq!(v, expected);
    let text = "line one\n\tline <two> \"q\" \\ end";
    let prepared = prepare(&INJ_1, &note(text), &TEST_INVENTORY).unwrap();
    let json = body(prepared.user());
    let v: serde_json::Value = serde_json::from_str(json).unwrap();
    assert_eq!(
        v,
        serde_json::json!({"projection": "untrusted-text-v1", "source": "owner_note", "complete": true, "text": text})
    );
    // Newline and tab appear as their two-byte JSON escapes.
    assert!(!json.contains('\n') && !json.contains('\t'));
    assert!(json.contains(&format!("one{}n{}tline", '\\', '\\')));
    // Keys in the order shown (§4.1).
    let keys_at: Vec<usize> = [
        "\"projection\":",
        "\"complete\":",
        "\"facts\":",
        "\"title\":",
        "\"text\":",
    ]
    .iter()
    .map(|k| {
        body(
            prepare(&SEN_1, &ref_page(), &TEST_INVENTORY)
                .unwrap()
                .user(),
        )
        .find(k)
        .unwrap()
    })
    .collect();
    assert!(keys_at.windows(2).all(|w| w[0] < w[1]));
}

#[test]
fn td_o3_the_frame_is_exact_and_nothing_trims() {
    for (set, projection) in [
        (&SEN_1, tricky_page()),
        (&INJ_1, note(" leading and trailing \n")),
    ] {
        let prepared = prepare(set, &projection, &TEST_INVENTORY).unwrap();
        let user = prepared.user();
        assert!(user.starts_with(OPEN) && user.ends_with(CLOSE));
        assert_eq!(
            user.matches("<document>").count(),
            1,
            "content cannot add a marker"
        );
        assert_eq!(
            user.matches("</document>").count(),
            1,
            "content cannot close the frame"
        );
        assert_eq!(user.as_bytes()[0], b'<');
        assert_eq!(*user.as_bytes().last().unwrap(), b'>');
    }
    for set in REGISTRY {
        let mut texts = vec![render_system(set).unwrap()];
        texts.extend(
            DecodeCode::ALL
                .iter()
                .map(|c| render_correction_system(set, *c).unwrap()),
        );
        for system in texts {
            assert_eq!(system.trim(), system, "no leading or trailing whitespace");
            assert!(system.is_ascii() && !system.contains('<') && !system.contains('>'));
            assert!(!system.is_empty());
        }
    }
}

#[test]
fn td_o3_the_profile_refuses_characters_outside_td_chars_v1() {
    let bad = [
        '\u{2581}',
        '\u{a0}',
        '\r',
        '\u{7}',
        '\u{1b}',
        '\u{0}',
        '\u{7f}',
        '\u{ad}',
        '\u{200b}',
        '\u{feff}',
        '\u{2028}',
        '\u{85}',
        '\u{1f600}',
        '\u{3000}',
    ];
    let at = |c: char, slot: usize| -> Projection {
        let s = format!("ok{c}ok");
        let mut fields = [
            "Name".to_string(),
            "Mon 9-5".into(),
            "0123 456789".into(),
            "Title".into(),
            "Body".into(),
        ];
        fields[slot] = s;
        let [name, hours, phone, title, text] = fields;
        page(&name, &[&hours], &phone, &title, &[(Element::P, &text)])
    };
    for c in bad.iter().copied().chain(['\n', '\t']) {
        for slot in 0..5 {
            let refusal = prepare(&SEN_1, &at(c, slot), &TEST_INVENTORY).err();
            assert_eq!(
                refusal.map(|r| r.code()),
                Some("char_not_allowed"),
                "U+{:04X} in slot {slot}",
                c as u32
            );
        }
    }
    for c in bad {
        let refusal = prepare(&INJ_1, &note(&format!("note {c}")), &TEST_INVENTORY).err();
        assert_eq!(
            refusal.map(|r| r.code()),
            Some("char_not_allowed"),
            "U+{:04X} in a note",
            c as u32
        );
    }
    // The profile's edges are admitted; notes also admit newline and tab.
    let edges: String = [
        '\u{20}', '\u{7e}', '\u{a1}', '\u{ac}', '\u{ae}', '\u{ff}', '\u{100}', '\u{17f}',
        '\u{2012}', '\u{2014}', '\u{2018}', '\u{201f}', '\u{2022}', '\u{2026}', '\u{20ac}',
        '\u{2122}',
    ]
    .iter()
    .collect();
    for slot in 0..5 {
        assert!(prepare(&SEN_1, &at('x', slot), &TEST_INVENTORY).is_ok());
    }
    assert!(prepare(
        &SEN_1,
        &page(&edges, &[&edges], &edges, &edges, &[(Element::Li, &edges)]),
        &TEST_INVENTORY
    )
    .is_ok());
    assert!(prepare(&INJ_1, &note(&format!("{edges}\n\t")), &TEST_INVENTORY).is_ok());
    // Just outside each range.
    for c in [
        '\u{1f}', '\u{a0}', '\u{ad}', '\u{180}', '\u{2011}', '\u{2015}', '\u{2017}', '\u{2020}',
        '\u{2021}', '\u{2023}', '\u{2025}', '\u{2027}', '\u{20ab}', '\u{20ad}', '\u{2121}',
        '\u{2123}',
    ] {
        assert!(
            prepare(&INJ_1, &note(&format!("x{c}")), &TEST_INVENTORY).is_err(),
            "U+{:04X}",
            c as u32
        );
    }
}

#[test]
fn td_o3_inventory_spellings_are_refused_before_encoding() {
    let spellings = [
        "<|turn>",
        "<turn|>",
        "<bos>",
        "<|\"|>",
        "<eos>",
        "<|channel>",
    ];
    for s in spellings {
        for slot in 0..5 {
            let v = format!("a {s} b");
            let mut f = ["N", "H", "0123 456789", "T", "B"].map(String::from);
            f[slot] = v;
            let p = page(&f[0], &[&f[1]], &f[2], &f[3], &[(Element::P, &f[4])]);
            assert_eq!(
                prepare(&SEN_1, &p, &TEST_INVENTORY).err().map(|r| r.code()),
                Some("unsafe_delimiter"),
                "{s} in slot {slot}"
            );
        }
        let refusal = prepare(&INJ_1, &note(&format!("x{s}y")), &TEST_INVENTORY).err();
        assert_eq!(
            refusal.map(|r| r.code()),
            Some("unsafe_delimiter"),
            "{s} in a note"
        );
    }
    // Exact and case-sensitive: near misses are admitted, and escaped.
    for ok in ["<|turn", "turn|>", "<BOS>", "< bos>", "<|\" |>", "<|tur n>"] {
        let prepared = prepare(&INJ_1, &note(ok), &TEST_INVENTORY).expect(ok);
        assert!(!body(prepared.user()).contains('<'));
    }
    // The profile is checked first.
    let both = note("\u{2581}<bos>");
    assert_eq!(
        prepare(&INJ_1, &both, &TEST_INVENTORY)
            .err()
            .map(|r| r.code()),
        Some("char_not_allowed")
    );
}

#[test]
fn td_o3_the_inventory_check_after_encoding_guards_the_frame_and_constants() {
    const FRAME: Inventory = Inventory::new(&["</document>"]);
    const CONSTANT: Inventory = Inventory::new(&["Evaluate every question"]);
    const CORRECTION: Inventory = Inventory::new(&["sum to 1"]);
    let clean = || note("plain words");
    for inventory in [FRAME, CONSTANT] {
        for set in REGISTRY {
            let p = if set.projection() == INJ_1.projection() {
                clean()
            } else {
                ref_page()
            };
            assert_eq!(
                prepare(set, &p, &inventory).err().map(|r| r.code()),
                Some("unsafe_delimiter")
            );
        }
    }
    assert_eq!(
        prepare(&INJ_1, &clean(), &CORRECTION)
            .err()
            .map(|r| r.code()),
        Some("unsafe_delimiter")
    );
    assert!(prepare(&INJ_1, &clean(), &TEST_INVENTORY).is_ok());
}
