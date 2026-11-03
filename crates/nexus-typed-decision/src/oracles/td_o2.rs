//! TD-O2: the two artifacts agree (§6). Schema `properties` keys =
//! `required` = the shape keys = the registry ids = the decoder's keys, in
//! order; every registry text is verbatim on its question's line and absent
//! from the schema; the schema passes A1's prose check; text and label
//! changes change the right artifacts and `set_hash`; generated test
//! registries parse back to the same structure from both artifacts.

use super::support::{
    labels, mixed_set, options, parse, question, test_set, with_question, Rng, J,
};
use crate::registry::Question;
use crate::{
    render_schema, render_shape, render_system, set_hash, validate, AnswerMode, Kind,
    OptionDisposition, Polarity, QuestionSet, RenderError, REGISTRY, SEN_1,
};

/// A1's `ResponseSchema::new` rules, restated (`request.rs:43`): one
/// object; none of the five prose keys, in any case, at any depth; no
/// duplicate key at any depth; at most 16 KiB.
fn a1_prose_check(schema: &str) -> Result<(), String> {
    const PROSE_KEYS: [&str; 5] = ["description", "title", "examples", "default", "$comment"];
    fn walk(v: &J) -> Result<(), String> {
        match v {
            J::Obj(kv) => {
                for (i, (k, x)) in kv.iter().enumerate() {
                    if PROSE_KEYS.contains(&k.to_ascii_lowercase().as_str()) {
                        return Err(format!("prose key {k}"));
                    }
                    if kv[..i].iter().any(|(seen, _)| seen == k) {
                        return Err(format!("duplicate key {k}"));
                    }
                    walk(x)?;
                }
                Ok(())
            }
            J::Arr(xs) => xs.iter().try_for_each(walk),
            _ => Ok(()),
        }
    }
    serde_json::from_str::<serde_json::Value>(schema).map_err(|e| format!("not JSON: {e}"))?;
    let v = parse(schema);
    if !matches!(v, J::Obj(_)) {
        return Err("not an object".into());
    }
    if schema.len() > 16 * 1024 {
        return Err("over 16 KiB".into());
    }
    walk(&v)
}

fn strs(v: &J) -> Vec<String> {
    match v {
        J::Arr(xs) => xs
            .iter()
            .map(|x| match x {
                J::Str(s) => s.clone(),
                other => panic!("not a string: {other:?}"),
            })
            .collect(),
        other => panic!("not an array: {other:?}"),
    }
}

fn ids(set: &QuestionSet) -> Vec<String> {
    set.keys().map(|k| k.as_str().to_string()).collect()
}

fn choice_labels(q: &Question) -> Option<Vec<String>> {
    match q.kind() {
        Kind::Noul { .. } => None,
        Kind::Choice { options } => Some(
            options
                .iter()
                .map(|(l, _)| l.as_str().to_string())
                .collect(),
        ),
        Kind::Score { levels, .. } => Some(levels.iter().map(|l| l.as_str().to_string()).collect()),
    }
}

/// The structure the registry implies for the schema (§6).
fn expected_schema_value(q: &Question, mode: AnswerMode) -> J {
    let s = |x: &str| J::Str(x.to_string());
    let number = J::Obj(vec![("type".into(), s("number"))]);
    match (choice_labels(q), mode) {
        (None, AnswerMode::Probabilities) => number,
        (None, AnswerMode::Discrete) => J::Obj(vec![(
            "enum".into(),
            J::Arr(vec![J::Num("0".into()), J::Num("1".into())]),
        )]),
        (Some(ls), AnswerMode::Probabilities) => J::Obj(vec![
            ("type".into(), s("object")),
            (
                "properties".into(),
                J::Obj(ls.iter().map(|l| (l.clone(), number.clone())).collect()),
            ),
            ("required".into(), J::Arr(ls.iter().map(|l| s(l)).collect())),
            ("additionalProperties".into(), J::Bool(false)),
        ]),
        (Some(ls), AnswerMode::Discrete) => J::Obj(vec![(
            "enum".into(),
            J::Arr(ls.iter().map(|l| s(l)).collect()),
        )]),
    }
}

/// The structure the registry implies for the shape (`P` read as `0`).
fn expected_shape(set: &QuestionSet) -> J {
    J::Obj(
        set.questions()
            .iter()
            .map(|q| {
                let v = match choice_labels(q) {
                    None => J::Num("0".into()),
                    Some(ls) => J::Obj(ls.into_iter().map(|l| (l, J::Num("0".into()))).collect()),
                };
                (q.id().as_str().to_string(), v)
            })
            .collect(),
    )
}

/// Checks the schema's structure against the registry, and the shape's
/// when the mode has one.
fn check_artifacts(set: &QuestionSet) {
    let schema_text = render_schema(set);
    assert_eq!(a1_prose_check(&schema_text), Ok(()), "{schema_text}");
    let schema = parse(&schema_text);
    assert_eq!(
        schema.keys(),
        ["type", "properties", "required", "additionalProperties"],
        "{schema_text}"
    );
    assert_eq!(schema.get("type"), &J::Str("object".into()));
    assert_eq!(schema.get("additionalProperties"), &J::Bool(false));
    assert_eq!(
        schema.get("properties").keys(),
        ids(set),
        "properties keys = registry ids"
    );
    assert_eq!(
        strs(schema.get("required")),
        ids(set),
        "required = registry ids"
    );
    for q in set.questions() {
        assert_eq!(
            schema.get("properties").get(q.id().as_str()),
            &expected_schema_value(q, set.mode()),
            "{schema_text}"
        );
    }
    match set.mode() {
        AnswerMode::Probabilities => {
            let shape = render_shape(set).expect("a probabilities set has a shape");
            assert_eq!(
                parse(&shape.replace('P', "0")),
                expected_shape(set),
                "{shape}"
            );
            assert_eq!(
                parse(&shape.replace('P', "0")).keys(),
                ids(set),
                "shape keys = registry ids"
            );
        }
        AnswerMode::Discrete => assert_eq!(render_shape(set), Err(RenderError::NoRegisteredText)),
    }
}

#[test]
fn td_o2_schema_shape_registry_and_decoder_keys_agree() {
    for set in REGISTRY {
        check_artifacts(set);
        let system = render_system(set).unwrap();
        let shape = render_shape(set).unwrap();
        assert_eq!(
            system.matches(shape.as_str()).count(),
            1,
            "the shape appears once in the system text"
        );
        // The question lines appear in registry order.
        let at: Vec<usize> = set
            .questions()
            .iter()
            .map(|q| {
                system
                    .find(&format!("\n- {} (yes/no): ", q.id().as_str()))
                    .expect("a line per question")
            })
            .collect();
        assert!(
            at.windows(2).all(|w| w[0] < w[1]),
            "lines in registry order"
        );
    }
}

#[test]
fn td_o2_every_text_is_verbatim_on_its_line_and_absent_from_the_schema() {
    for set in REGISTRY {
        let system = render_system(set).unwrap();
        let schema = render_schema(set);
        assert!(!schema.contains(' '), "the schema holds no prose: {schema}");
        for q in set.questions() {
            let head = format!("- {} (yes/no): ", q.id().as_str());
            let lines: Vec<&str> = system.lines().filter(|l| l.starts_with(&head)).collect();
            assert_eq!(lines.len(), 1, "one line for {}", q.id().as_str());
            let line = lines[0];
            assert!(line.contains(q.instructions()), "{line}");
            assert!(line.contains(&format!(" Yes if {}.", q.if_yes())), "{line}");
            assert!(line.contains(&format!(" No if {}.", q.if_no())), "{line}");
            for text in [q.instructions(), q.if_yes(), q.if_no()] {
                assert!(!schema.contains(text), "{text} in the schema");
            }
        }
    }
}

#[test]
fn td_o2_schemas_pass_the_prose_check() {
    for set in REGISTRY {
        assert_eq!(a1_prose_check(&render_schema(set)), Ok(()));
    }
    for mode in [AnswerMode::Probabilities, AnswerMode::Discrete] {
        let mix = mixed_set();
        let set = QuestionSet { mode, ..*mix };
        assert_eq!(a1_prose_check(&render_schema(&set)), Ok(()));
    }
    // The restated check itself refuses each prose key, any case, any depth.
    assert!(a1_prose_check(r#"{"type":"object","Description":"x"}"#).is_err());
    assert!(a1_prose_check(
        r#"{"type":"object","properties":{"a":{"type":"number","$comment":"x"}}}"#
    )
    .is_err());
    assert!(a1_prose_check(r#"{"a":1,"a":2}"#).is_err());
    assert!(a1_prose_check(r#"["type"]"#).is_err());
}

#[test]
fn td_o2_text_and_label_changes_change_the_artifacts_and_set_hash() {
    let (system, schema, hash) = (
        render_system(&SEN_1).unwrap(),
        render_schema(&SEN_1),
        set_hash(&SEN_1),
    );
    for i in 0..SEN_1.questions.len() {
        for field in 0..3 {
            let changed = with_question(&SEN_1, i, |q| match field {
                0 => q.instructions = "Is it so?",
                1 => q.if_yes = "it is so",
                _ => q.if_no = "it is not so",
            });
            assert_ne!(
                render_system(&changed).unwrap(),
                system,
                "question {i} text {field}"
            );
            assert_eq!(
                render_schema(&changed),
                schema,
                "a text never reaches the schema"
            );
            assert_ne!(set_hash(&changed), hash, "question {i} text {field}");
        }
    }
    let mix = mixed_set();
    let (schema, shape, hash) = (
        render_schema(mix),
        render_shape(mix).unwrap(),
        set_hash(mix),
    );
    let relabelled = [
        with_question(mix, 1, |q| {
            q.kind = Kind::Choice {
                options: options(&[
                    ("fit", OptionDisposition::Clear),
                    ("unclear", OptionDisposition::NeedsOwner),
                ]),
            }
        }),
        with_question(mix, 2, |q| {
            q.kind = Kind::Score {
                levels: labels(&["low", "middle", "high"]),
                at_least: 2,
                polarity: Polarity::Positive,
            }
        }),
        with_question(mix, 0, |q| q.id = crate::ident::QuestionId::new("noul_r")),
    ];
    for changed in &relabelled {
        assert_ne!(render_schema(changed), schema);
        assert_ne!(render_shape(changed).unwrap(), shape);
        assert_ne!(set_hash(changed), hash);
    }
}

fn ident(rng: &mut Rng, taken: &mut Vec<String>) -> String {
    const FIRST: &[u8] = b"abcdefghijklmnopqrstuvwxyz";
    const REST: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789_";
    loop {
        let mut s = String::from(rng.pick(FIRST) as char);
        for _ in 0..rng.below(12) {
            s.push(rng.pick(REST) as char);
        }
        if !taken.contains(&s) {
            taken.push(s.clone());
            return s;
        }
    }
}

#[test]
fn td_o2_generated_registries_parse_back_to_the_same_structure() {
    let mut rng = Rng(0x7d01_0002);
    let (mut renderable, mut not_renderable) = (0, 0);
    for n in 0..300 {
        let mut taken = Vec::new();
        let questions: Vec<Question> = (0..1 + rng.below(6))
            .map(|_| {
                let id = ident(&mut rng, &mut taken);
                let polarity = rng.pick(&[Polarity::Risk, Polarity::Positive]);
                let kind = match rng.below(4) {
                    0 | 1 => Kind::Noul { polarity },
                    k => {
                        let mut names = Vec::new();
                        let count = 2 + rng.below(4);
                        let ls: Vec<String> =
                            (0..count).map(|_| ident(&mut rng, &mut names)).collect();
                        let ls: Vec<&str> = ls.iter().map(String::as_str).collect();
                        if k == 2 {
                            let ds = [
                                OptionDisposition::Clear,
                                OptionDisposition::NeedsOwner,
                                OptionDisposition::Restrict,
                            ];
                            let opts: Vec<(&str, OptionDisposition)> =
                                ls.iter().map(|l| (*l, rng.pick(&ds))).collect();
                            Kind::Choice {
                                options: options(&opts),
                            }
                        } else {
                            Kind::Score {
                                levels: labels(&ls),
                                at_least: 1 + rng.below(count - 1) as u8,
                                polarity,
                            }
                        }
                    }
                };
                question(&id, kind)
            })
            .collect();
        let mode = rng.pick(&[AnswerMode::Probabilities, AnswerMode::Discrete]);
        let set = test_set(&format!("gen_{n}"), mode, questions);
        assert_eq!(validate(set), Ok(()), "generated set {n}");
        check_artifacts(set);
        let noul_only = set
            .questions()
            .iter()
            .all(|q| matches!(q.kind(), Kind::Noul { .. }));
        match render_system(set) {
            Ok(system) => {
                assert!(noul_only && mode == AnswerMode::Probabilities, "set {n}");
                assert!(system.contains(&render_shape(set).unwrap()));
                renderable += 1;
            }
            Err(RenderError::NoRegisteredText) => {
                assert!(!noul_only || mode == AnswerMode::Discrete, "set {n}");
                not_renderable += 1;
            }
        }
    }
    assert!(
        renderable > 10 && not_renderable > 10,
        "{renderable} / {not_renderable}"
    );
}
