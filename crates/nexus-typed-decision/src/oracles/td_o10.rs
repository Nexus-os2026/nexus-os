//! TD-O10: records (§7, §9.1). `RecordView` gives per-mille values rounded
//! half-even from ppm, for display only; there is no path from a record to
//! a `Decision` (the compile-fail doctests in `lib.rs` and the source guard
//! below), and nothing that gates names a record.

use sha2::{Digest, Sha256};

use super::support_b::{decide, mixed, mixed_discrete, mixed_reply};
use crate::record::permille;
use crate::{decode, AnswerMode, Attempt, Label, Polarity, Reading, Top, INJ_1};

fn put(h: &mut Sha256, bytes: &[u8]) {
    h.update((bytes.len() as u64).to_be_bytes());
    h.update(bytes);
}

/// `vector_sha256`: SHA-256 over the domain, the count, and each label and
/// its ppm (u32, big-endian), each length-prefixed.
fn vector_digest(entries: &[(&str, u32)]) -> [u8; 32] {
    let mut h = Sha256::new();
    put(&mut h, b"nexus.typed_decision.vector.v1");
    put(&mut h, &(entries.len() as u64).to_be_bytes());
    for (label, ppm) in entries {
        put(&mut h, label.as_bytes());
        put(&mut h, &ppm.to_be_bytes());
    }
    h.finalize().into()
}

#[test]
fn td_o10_per_mille_is_rounded_half_even() {
    let table = [
        (0, 0),
        (1, 0),
        (499, 0),
        (500, 0),
        (501, 1),
        (1_499, 1),
        (1_500, 2),
        (2_500, 2),
        (3_500, 4),
        (250_500, 250),
        (251_500, 252),
        (299_999, 300),
        (699_500, 700),
        (700_500, 700),
        (701_500, 702),
        (998_500, 998),
        (999_499, 999),
        (999_500, 1_000),
        (1_000_000, 1_000),
    ];
    for (ppm, expected) in table {
        assert_eq!(permille(ppm), expected, "{ppm}");
    }
    for ppm in 0..=1_000_000u32 {
        let pm = u32::from(permille(ppm));
        let off = (pm * 1_000).abs_diff(ppm);
        assert!(off < 500 || (off == 500 && pm % 2 == 0), "{ppm} -> {pm}");
    }
    // Through a decision. The rounded value is for display only: 0.7 and
    // 0.700001 both show 700, but they read differently.
    let view = |value: &str| {
        let d = decide(&INJ_1, &format!("{{\"instructs_ai\":{value}}}")).expect("decodes");
        let v = d.record_view();
        (v.answers()[0].permille(), v.answers()[0].reading())
    };
    for (value, expected) in [
        ("0.0005", 0),
        ("0.0015", 2),
        ("0.0025", 2),
        ("0.0035", 4),
        ("0.9985", 998),
        ("0.9995", 1_000),
        ("0.123456", 123),
    ] {
        assert_eq!(view(value).0, expected, "{value}");
    }
    assert_eq!(view("0.7"), (700, Reading::Unsettled));
    assert_eq!(view("0.700001"), (700, Reading::Yes));
    assert_eq!(view("0.2995"), (300, Reading::No));
}

#[test]
fn td_o10_the_view_shows_every_answer() {
    let d = decide(
        mixed(),
        &mixed_reply(250_000, [700_001, 299_999], [100_000, 199_999, 700_001]),
    )
    .expect("decodes");
    let v = d.record_view();
    assert_eq!(
        (v.set().as_str(), v.mode(), v.attempts(), v.renormalized()),
        ("mix_1", AnswerMode::Probabilities, 1, false)
    );
    let a = v.answers();
    let ids: Vec<&str> = a.iter().map(|x| x.question().as_str()).collect();
    assert_eq!(ids, ["noul_q", "choice_q", "score_q"]);
    // A Noul: its probability, no vector.
    assert_eq!(
        (a[0].polarity(), a[0].reading(), a[0].permille()),
        (Some(Polarity::Risk), Reading::No, 250)
    );
    assert_eq!(
        (a[0].top(), a[0].top_permille(), a[0].vector_sha256()),
        (None, None, None)
    );
    // A Choice: its top, read and shown; per-option dispositions, so no polarity.
    let fits = Label::new("fits");
    assert_eq!(
        (a[1].polarity(), a[1].reading(), a[1].permille()),
        (None, Reading::Selected(fits), 700)
    );
    assert_eq!(
        (a[1].top(), a[1].top_permille()),
        (Some(Top::Label(fits)), Some(700))
    );
    assert_eq!(
        a[1].vector_sha256(),
        Some(vector_digest(&[("fits", 700_001), ("unclear", 299_999)]))
    );
    // A Score: its cumulative probability is the one read and shown.
    assert_eq!(
        (a[2].polarity(), a[2].reading(), a[2].permille()),
        (Some(Polarity::Positive), Reading::Yes, 700)
    );
    assert_eq!(
        (a[2].top(), a[2].top_permille()),
        (Some(Top::Label(Label::new("high"))), Some(700))
    );
    assert_eq!(
        a[2].vector_sha256(),
        Some(vector_digest(&[
            ("low", 100_000),
            ("mid", 199_999),
            ("high", 700_001)
        ]))
    );
    // A tie, a renormalized vector (its normalized values are hashed), a
    // second attempt and the discrete mode.
    let d = decode(
        mixed(),
        &mixed_reply(0, [500_000, 500_000], [333_333, 333_333, 333_333]),
        [0; 32],
        Attempt::Second,
    )
    .expect("decodes");
    let v = d.record_view();
    assert_eq!((v.attempts(), v.renormalized()), (2, true));
    assert_eq!(
        (
            v.answers()[1].top(),
            v.answers()[1].top_permille(),
            v.answers()[1].permille()
        ),
        (Some(Top::Tie), Some(500), 500)
    );
    assert_eq!(v.answers()[1].reading(), Reading::Unsettled);
    assert_eq!(
        v.answers()[2].vector_sha256(),
        Some(vector_digest(&[
            ("low", 333_334),
            ("mid", 333_333),
            ("high", 333_333)
        ]))
    );
    assert_eq!(v.answers()[2].permille(), 333);
    let d = decide(
        mixed_discrete(),
        "{\"noul_q\":0,\"choice_q\":\"fits\",\"score_q\":\"high\"}",
    )
    .expect("decodes");
    let v = d.record_view();
    assert_eq!(v.mode(), AnswerMode::Discrete);
    assert_eq!(
        v.answers().iter().map(|x| x.permille()).collect::<Vec<_>>(),
        [0, 1_000, 1_000]
    );
}

#[test]
fn td_o10_no_record_to_decision_api() {
    // Control: a view exists for a decision.
    let d = decide(&INJ_1, "{\"instructs_ai\":0.5}").expect("decodes");
    assert_eq!(d.record_view().answers().len(), 1);
    // Every production file, as compile-time text (no I/O).
    let files = [
        ("budget.rs", include_str!("../budget.rs")),
        ("decode.rs", include_str!("../decode.rs")),
        ("hash.rs", include_str!("../hash.rs")),
        ("ident.rs", include_str!("../ident.rs")),
        ("lane.rs", include_str!("../lane.rs")),
        ("lib.rs", include_str!("../lib.rs")),
        ("projection.rs", include_str!("../projection.rs")),
        ("read.rs", include_str!("../read.rs")),
        ("record.rs", include_str!("../record.rs")),
        ("registry.rs", include_str!("../registry.rs")),
        ("render.rs", include_str!("../render.rs")),
        ("slot.rs", include_str!("../slot.rs")),
    ];
    for (name, text) in files {
        // Only the record module (and its re-export) names the view, so no
        // lane, slot, reader or reading takes one.
        let names_view = text.contains("RecordView") || text.contains("AnswerView");
        assert_eq!(names_view, matches!(name, "record.rs" | "lib.rs"), "{name}");
        // Only the reader returns a decision (lib.rs holds only doctests).
        if !matches!(name, "decode.rs" | "lib.rs") {
            assert!(
                !text.contains("-> Decision") && !text.contains("<Decision"),
                "{name}"
            );
        }
    }
    let reader = include_str!("../decode.rs");
    assert_eq!(
        reader.matches("-> Result<Decision, ReplyError>").count(),
        1,
        "decode alone"
    );
    assert!(!reader.contains("-> Decision"));
    // The decision's fields are private to the reader's module.
    let start =
        reader.find("pub struct Decision {").expect("defined") + "pub struct Decision {".len();
    let fields = &reader[start..start + reader[start..].find('}').unwrap()];
    assert!(!fields.contains("pub"), "private fields: {fields}");
}
