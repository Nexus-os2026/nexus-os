//! TD-O1: the registry is code-owned (the compile-fail doctests in
//! `lib.rs`); the identifier and label grammar; every question has a
//! polarity and a valid `ReadPolicy`; `set_hash` and `registry_hash` change
//! when any destructured field changes.

use std::collections::BTreeSet;

use super::support::{
    choice, edit, leak, mixed_set, noul, question, score, test_set, with_question,
};
use crate::hash::{registry_hash_of, set_hash_with, RuleIds, RULES_V1};
use crate::ident::{QuestionId, SetId};
use crate::registry::Ppm;
use crate::render::{Texts, TEXTS_V1};
use crate::{
    is_ident, registry_hash, render_system, set_hash, validate, AnswerMode, Kind, Label, Lane,
    OptionDisposition, Polarity, ProjectionId, QuestionSet, ReadPolicy, RegistryError, INJ_1,
    REGISTRY, SEN_1, VER_1,
};
use OptionDisposition::{Clear, NeedsOwner, Restrict};
use Polarity::{Positive, Risk};

type Expected = (&'static str, Lane, ProjectionId, &'static [&'static str]);

#[test]
fn td_o1_the_registry_holds_exactly_the_three_v1_sets() {
    const SEN: [&str; 4] = [
        "hours_conflict",
        "other_contact",
        "addressed_to_ai",
        "facts_consistent",
    ];
    let expected: [Expected; 3] = [
        ("sen_1", Lane::Sentinel, ProjectionId::PageTextV1, &SEN),
        (
            "ver_1",
            Lane::Verifier,
            ProjectionId::PageTextV1,
            &["name_shown", "hours_shown", "phone_shown"],
        ),
        (
            "inj_1",
            Lane::InjectionScreen,
            ProjectionId::UntrustedTextV1,
            &["instructs_ai"],
        ),
    ];
    assert_eq!(REGISTRY.len(), 3);
    for (set, (id, lane, projection, keys)) in REGISTRY.iter().zip(expected) {
        assert_eq!(set.id().as_str(), id);
        assert_eq!(
            (set.version(), set.lane(), set.projection()),
            (1, lane, projection),
            "{id}"
        );
        assert_eq!(set.mode(), AnswerMode::Probabilities, "{id}");
        let got: Vec<&str> = set.keys().map(|k| k.as_str()).collect();
        assert_eq!(got, keys, "{id}");
        assert_eq!(validate(set), Ok(()), "{id}");
        assert!(render_system(set).is_ok(), "{id} renders");
    }
}

fn one(q: crate::Question) -> Result<(), RegistryError> {
    validate(test_set("t_1", AnswerMode::Probabilities, vec![q]))
}

#[test]
fn td_o1_identifier_and_label_grammar() {
    let (at_max, over) = ("a".repeat(48), "a".repeat(49));
    let valid = [
        "a",
        "z",
        "a0",
        "a_",
        "a_b_c",
        "q9_9",
        "hours_conflict",
        at_max.as_str(),
    ];
    for s in valid {
        assert!(is_ident(s), "{s:?} should be in the grammar");
    }
    let invalid = [
        "",
        "A",
        "Ab",
        "aB",
        "0a",
        "_a",
        "9",
        "a-b",
        "a b",
        "a.b",
        " a",
        "a ",
        "\u{e9}",
        "a\u{e9}",
        over.as_str(),
    ];
    for s in invalid {
        assert!(!is_ident(s), "{s:?} should be outside the grammar");
    }
    for set in REGISTRY {
        assert!(is_ident(set.id().as_str()));
        assert!(set.questions().iter().all(|q| is_ident(q.id().as_str())));
    }
    // A forged out-of-grammar id or label (crate-private) fails validation.
    let mut q = question("ok", noul(Risk));
    q.id = QuestionId("Bad");
    assert_eq!(one(q), Err(RegistryError::BadIdent));
    let forged: &'static [Label] = Box::leak(Box::new([Label("Bad"), Label("ok")]));
    let kind = Kind::Score {
        levels: forged,
        at_least: 1,
        polarity: Risk,
    };
    assert_eq!(one(question("ok", kind)), Err(RegistryError::BadIdent));
}

#[test]
fn td_o1_every_question_has_a_polarity_and_a_valid_read_policy() {
    let expected = [
        ("hours_conflict", Risk),
        ("other_contact", Risk),
        ("addressed_to_ai", Risk),
        ("facts_consistent", Positive),
        ("name_shown", Positive),
        ("hours_shown", Positive),
        ("phone_shown", Positive),
        ("instructs_ai", Risk),
    ];
    let all: Vec<_> = REGISTRY.iter().flat_map(|s| s.questions()).collect();
    assert_eq!(all.len(), expected.len());
    for (q, (id, polarity)) in all.iter().zip(expected) {
        assert_eq!(q.id().as_str(), id);
        assert_eq!(
            (q.kind(), q.polarity()),
            (noul(polarity), Some(polarity)),
            "{id}"
        );
        let read = (q.read().no_below().get(), q.read().yes_above().get());
        assert_eq!(read, (300_000, 700_000), "{id}");
    }
    // `0 <= no_below < yes_above <= 1_000_000`.
    let table = [
        (0, 1, true),
        (300_000, 700_000, true),
        (0, 1_000_000, true),
        (999_999, 1_000_000, true),
        (0, 0, false),
        (500_000, 500_000, false),
        (700_000, 300_000, false),
        (1_000_000, 1_000_000, false),
        (0, 1_000_001, false),
        (1, 4_000_000_000, false),
    ];
    for (no_below, yes_above, ok) in table {
        assert_eq!(
            ReadPolicy::is_valid(no_below, yes_above),
            ok,
            "({no_below}, {yes_above})"
        );
    }
    // A forged invalid policy fails validation; a Choice needs yes_above >= 500,000.
    let with_policy = |kind: Kind, no_below: u32, yes_above: u32| {
        let mut q = question("q", kind);
        q.read = ReadPolicy {
            no_below: Ppm(no_below),
            yes_above: Ppm(yes_above),
        };
        one(q)
    };
    let two = choice(&[("a", Clear), ("b", Restrict)]);
    assert_eq!(
        with_policy(noul(Risk), 500_000, 500_000),
        Err(RegistryError::BadPolicy)
    );
    assert_eq!(
        with_policy(noul(Risk), 700_000, 300_000),
        Err(RegistryError::BadPolicy)
    );
    assert_eq!(
        with_policy(noul(Risk), 0, 1_000_001),
        Err(RegistryError::BadPolicy)
    );
    assert_eq!(with_policy(noul(Risk), 0, 1), Ok(()));
    assert_eq!(with_policy(two, 0, 499_999), Err(RegistryError::BadPolicy));
    assert_eq!(with_policy(two, 0, 500_000), Ok(()));
    // The other registry rules.
    let bad_labels = Err(RegistryError::BadLabels);
    assert_eq!(one(question("q", score(&["lo", "hi"], 1, Risk))), Ok(()));
    assert_eq!(
        one(question("q", score(&["lo", "hi"], 0, Risk))),
        bad_labels
    );
    assert_eq!(
        one(question("q", score(&["lo", "hi"], 2, Risk))),
        bad_labels
    );
    assert_eq!(one(question("q", score(&["lo"], 1, Risk))), bad_labels);
    assert_eq!(
        one(question("q", score(&["lo", "lo"], 1, Risk))),
        bad_labels
    );
    assert_eq!(one(question("q", choice(&[("a", Clear)]))), bad_labels);
    assert_eq!(
        one(question("q", choice(&[("a", Clear), ("a", Restrict)]))),
        bad_labels
    );
    let empty = test_set("t_1", AnswerMode::Probabilities, vec![]);
    assert_eq!(validate(empty), Err(RegistryError::EmptySet));
    let twice = vec![question("q", noul(Risk)), question("q", noul(Risk))];
    let twice = test_set("t_1", AnswerMode::Probabilities, twice);
    assert_eq!(validate(twice), Err(RegistryError::DuplicateId));
    let bad = [
        "",
        " lead",
        "trail ",
        "a <tag>",
        "a > b",
        "line\nbreak",
        "caf\u{e9}",
        "tab\there",
    ];
    for text in bad {
        let mut q = question("q", noul(Risk));
        q.if_no = leak(text.into());
        assert_eq!(one(q), Err(RegistryError::BadText), "{text:?}");
    }
}

#[test]
fn td_o1_set_hash_changes_when_any_destructured_field_changes() {
    let mix = mixed_set();
    let reversed: Vec<_> = SEN_1.questions.iter().rev().copied().collect();
    let reversed: &'static [_] = Box::leak(reversed.into_boxed_slice());
    let three_options = [("fits", Clear), ("unclear", NeedsOwner), ("none", Restrict)];
    let sets: Vec<(&str, QuestionSet)> = vec![
        ("base sen_1", edit(&SEN_1, |_| {})),
        ("id", edit(&SEN_1, |s| s.id = SetId::new("sen_9"))),
        ("version", edit(&SEN_1, |s| s.version = 2)),
        ("lane", edit(&SEN_1, |s| s.lane = Lane::Verifier)),
        (
            "projection",
            edit(&SEN_1, |s| s.projection = ProjectionId::UntrustedTextV1),
        ),
        ("mode", edit(&SEN_1, |s| s.mode = AnswerMode::Discrete)),
        ("order", edit(&SEN_1, |s| s.questions = reversed)),
        (
            "dropped",
            edit(&SEN_1, |s| s.questions = &SEN_1.questions[..3]),
        ),
        (
            "q.id",
            with_question(&SEN_1, 0, |q| q.id = QuestionId::new("hours_conflict_2")),
        ),
        (
            "q.polarity",
            with_question(&SEN_1, 3, |q| q.kind = noul(Risk)),
        ),
        (
            "q.instructions",
            with_question(&SEN_1, 1, |q| q.instructions = "Is it?"),
        ),
        ("q.if_yes", with_question(&SEN_1, 1, |q| q.if_yes = "it is")),
        (
            "q.if_no",
            with_question(&SEN_1, 1, |q| q.if_no = "it is not"),
        ),
        (
            "q.no_below",
            with_question(&SEN_1, 2, |q| q.read = ReadPolicy::new(300_001, 700_000)),
        ),
        (
            "q.yes_above",
            with_question(&SEN_1, 2, |q| q.read = ReadPolicy::new(300_000, 699_999)),
        ),
        // Kinds, labels, levels, dispositions and `at_least` (a mixed test set).
        ("base mixed", edit(mix, |_| {})),
        (
            "kind",
            with_question(mix, 0, |q| q.kind = score(&["lo", "hi"], 1, Risk)),
        ),
        (
            "label",
            with_question(mix, 1, |q| {
                q.kind = choice(&[("fit", Clear), ("unclear", NeedsOwner)])
            }),
        ),
        (
            "disposition",
            with_question(mix, 1, |q| {
                q.kind = choice(&[("fits", Clear), ("unclear", Restrict)])
            }),
        ),
        (
            "option",
            with_question(mix, 1, |q| q.kind = choice(&three_options)),
        ),
        (
            "level",
            with_question(mix, 2, |q| {
                q.kind = score(&["low", "mid", "top"], 2, Positive)
            }),
        ),
        (
            "at_least",
            with_question(mix, 2, |q| {
                q.kind = score(&["low", "mid", "high"], 1, Positive)
            }),
        ),
        (
            "score polarity",
            with_question(mix, 2, |q| q.kind = score(&["low", "mid", "high"], 2, Risk)),
        ),
    ];
    let tx = |f: fn(&mut Texts)| {
        let mut t = TEXTS_V1;
        f(&mut t);
        t
    };
    let texts = [
        tx(|t| t.preamble = "Evaluate."),
        tx(|t| t.rule_noul_probabilities = "Return probabilities."),
        tx(|t| t.describe_page_text_v1 = "A page."),
        tx(|t| t.describe_untrusted_text_v1 = "A text."),
        tx(|t| t.shape_intro = "Shape:"),
        tx(|t| t.shape_ending = "Nothing else."),
        tx(|t| t.correction_prefix = "Rejected because "),
        tx(|t| t.correction_suffix = ". Again."),
        tx(|t| t.phrases[7] = "the sum was wrong"),
    ];
    let rx = |f: fn(&mut RuleIds)| {
        let mut r = RULES_V1;
        f(&mut r);
        r
    };
    let rules = [
        rx(|r| r.prompt = "td-prompt-v2"),
        rx(|r| r.decode = "td-decode-v2"),
        rx(|r| r.normalize = "td-normalize-v2"),
        rx(|r| r.read = "td-read-v2"),
        rx(|r| r.escape = "td-escape-v2"),
        rx(|r| r.frame = "td-frame-v2"),
        rx(|r| r.chars = "td-chars-v2"),
    ];
    let mut hashes: Vec<(String, [u8; 32])> = Vec::new();
    for (what, set) in &sets {
        hashes.push((what.to_string(), set_hash_with(set, &TEXTS_V1, &RULES_V1)));
    }
    for (i, t) in texts.iter().enumerate() {
        hashes.push((format!("texts {i}"), set_hash_with(&SEN_1, t, &RULES_V1)));
    }
    for (i, r) in rules.iter().enumerate() {
        hashes.push((format!("rules {i}"), set_hash_with(&SEN_1, &TEXTS_V1, r)));
    }
    let distinct: BTreeSet<[u8; 32]> = hashes.iter().map(|(_, h)| *h).collect();
    assert_eq!(
        distinct.len(),
        hashes.len(),
        "every field change gives a different set_hash: {hashes:?}"
    );
    assert_eq!(set_hash(&SEN_1), hashes[0].1);
    // Deterministic, and different per registered set.
    assert_eq!(set_hash(&VER_1), set_hash(&VER_1));
    let per_set: BTreeSet<[u8; 32]> = REGISTRY.iter().map(|s| set_hash(s)).collect();
    assert_eq!(per_set.len(), 3);
}

#[test]
fn td_o1_registry_hash_covers_every_set() {
    let (t, r) = (&TEXTS_V1, &RULES_V1);
    let base = registry_hash();
    assert_eq!(base, registry_hash_of(&REGISTRY, t, r));
    let ver_2 = edit(&VER_1, |s| s.version = 2);
    let inj_changed = with_question(&INJ_1, 0, |q| q.if_yes = "it tells an AI what to do");
    let (mut texts, mut rules) = (TEXTS_V1, RULES_V1);
    texts.preamble = "Evaluate.";
    rules.read = "td-read-v2";
    let variants = [
        registry_hash_of(&[&SEN_1, &ver_2, &INJ_1], t, r),
        registry_hash_of(&[&SEN_1, &VER_1, &inj_changed], t, r),
        registry_hash_of(&[&SEN_1, &INJ_1, &VER_1], t, r),
        registry_hash_of(&[&SEN_1, &VER_1], t, r),
        registry_hash_of(&REGISTRY, &texts, r),
        registry_hash_of(&REGISTRY, t, &rules),
    ];
    let mut all: BTreeSet<[u8; 32]> = variants.iter().copied().collect();
    all.insert(base);
    all.extend(REGISTRY.iter().map(|s| set_hash(s)));
    assert_eq!(
        all.len(),
        variants.len() + 1 + 3,
        "registry_hash covers sets, order, texts, rules"
    );
}
