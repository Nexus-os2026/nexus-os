//! TD-O6: the strict reader's boundary table (§7, `td-decode-v1`). A
//! duplicate key at depth 1 and at depth 2, refused before insertion;
//! unknown and missing keys; fences, prose before or after, a BOM; depth 3;
//! every number form of §7; the discrete forms; TD's own cap, 8,192 bytes
//! accepted and 8,193 refused, before the reader runs.

use super::support::{assert_no_canary, C, CL};
use super::support_b::{dec, decide, mixed, mixed_discrete, mixed_reply, obj};
use crate::{
    decode, Answer, Attempt, DecodeCode, Distribution, QuestionSet, ReplyError, INJ_1,
    MAX_REPLY_BYTES, SEN_1,
};
use DecodeCode::*;

const BS: char = '\\';

fn refused(set: &'static QuestionSet, reply: &str, expected: DecodeCode) {
    assert_eq!(
        decide(set, reply).map(|_| ()),
        Err(ReplyError::Decode(expected)),
        "{reply:?}"
    );
}

fn inj(value: &str) -> String {
    format!("{{\"instructs_ai\":{value}}}")
}

fn noul_ppm(answer: &Answer) -> u32 {
    match answer {
        Answer::Noul(p) => p.get(),
        other => panic!("not a Noul: {other:?}"),
    }
}

fn ppms(dist: &Distribution) -> Vec<u32> {
    dist.entries().iter().map(|(_, p)| p.get()).collect()
}

fn vector(answer: &Answer) -> Vec<u32> {
    match answer {
        Answer::Choice(d) | Answer::Score(d) => ppms(d),
        other => panic!("not a vector: {other:?}"),
    }
}

/// `choice_q` with `fits` written as `fits` and the other two questions valid.
fn mixed_with_fits(fits: &str, unclear: &str) -> String {
    format!(
        "{{\"noul_q\":0.1,\"choice_q\":{{\"fits\":{fits},\"unclear\":{unclear}}},\"score_q\":{{\"low\":0,\"mid\":0,\"high\":1}}}}"
    )
}

#[test]
fn td_o6_well_formed_replies_decode() {
    let d = decide(&INJ_1, &inj("0.25")).expect("decodes");
    assert_eq!(noul_ppm(&d.answers()[0].1), 250_000);
    assert_eq!(d.answers()[0].0.as_str(), "instructs_ai");
    // JSON whitespace around and inside, keys in any order; answers come
    // back in registry order.
    let reply = " \n\t{ \"facts_consistent\" : 1 ,\r\n\"addressed_to_ai\":0, \"other_contact\":0.5,\"hours_conflict\":0.000001 }\n ";
    let d = decide(&SEN_1, reply).expect("decodes");
    let got: Vec<(&str, u32)> = d
        .answers()
        .iter()
        .map(|(id, a)| (id.as_str(), noul_ppm(a)))
        .collect();
    assert_eq!(
        got,
        [
            ("hours_conflict", 1),
            ("other_contact", 500_000),
            ("addressed_to_ai", 0),
            ("facts_consistent", 1_000_000)
        ]
    );
    // Vectors at depth 2, labels in any order.
    let reply = "{\"score_q\":{\"high\":0.5,\"low\":0.2,\"mid\":0.3},\"choice_q\":{\"unclear\":0.7,\"fits\":0.3},\"noul_q\":0.1}";
    let d = decide(mixed(), reply).expect("decodes");
    let keys: Vec<&str> = d.answers().iter().map(|(id, _)| id.as_str()).collect();
    assert_eq!(keys, ["noul_q", "choice_q", "score_q"]);
    assert!(matches!(d.answers()[1].1, Answer::Choice(_)));
    assert!(matches!(d.answers()[2].1, Answer::Score(_)));
    assert_eq!(vector(&d.answers()[1].1), [300_000, 700_000]);
    assert_eq!(vector(&d.answers()[2].1), [200_000, 300_000, 500_000]);
    let labels: Vec<&str> = match &d.answers()[2].1 {
        Answer::Score(dist) => dist.entries().iter().map(|(l, _)| l.as_str()).collect(),
        _ => unreachable!(),
    };
    assert_eq!(labels, ["low", "mid", "high"]);
    // The engine's binding and attempt are carried, not read from the reply.
    let d = decode(&INJ_1, &inj("1"), [7; 32], Attempt::Second).expect("decodes");
    assert_eq!((d.binding(), d.attempt()), (&[7; 32], Attempt::Second));
}

#[test]
fn td_o6_exactly_one_object_and_nothing_else() {
    let good = inj("0.25");
    assert!(decide(&INJ_1, &good).is_ok(), "control");
    let bom = char::from_u32(0xfeff).unwrap();
    let nbsp = char::from_u32(0xa0).unwrap();
    let form_feed = char::from_u32(0x0c).unwrap();
    let unknown_then_prose = format!("{} trailing", obj(&[("zzz", "0".into())]));
    let cases: Vec<String> = vec![
        format!("```json\n{good}\n```"),
        format!("```\n{good}\n```"),
        format!("{good}\n```"),
        format!("```json\n{good}"),
        format!("Here is the answer: {good}"),
        format!("{good} Hope this helps."),
        format!("{good}."),
        format!("{bom}{good}"),
        format!("{good}{bom}"),
        format!("{nbsp}{good}"),
        format!("{form_feed}{good}"),
        format!("{good}{good}"),
        format!("{good},{good}"),
        format!("[{good}]"),
        format!("// note\n{good}"),
        format!("{good} /* note */"),
        format!("# {good}"),
        // The shape is checked before any key: prose after an unknown key.
        unknown_then_prose,
        String::new(),
        "  \n ".into(),
        "null".into(),
        "0.25".into(),
        "\"instructs_ai\"".into(),
        "{'instructs_ai':0.25}".into(),
        "{instructs_ai:0.25}".into(),
        "{\"instructs_ai\":0.25,}".into(),
        "{,\"instructs_ai\":0.25}".into(),
        "{\"instructs_ai\" 0.25}".into(),
        "{\"instructs_ai\"::0.25}".into(),
        "{\"instructs_ai\":0.25".into(),
        "{\"instructs_ai\":}".into(),
        "{\"instructs_ai\":0.25 /* x */}".into(),
        "{\"instructs_ai\":0.25}}".into(),
        "{\"instr\nucts_ai\":0.25}".into(),
        format!("{{\"instructs_ai\":\"{BS}x\"}}"),
        format!("{{\"instructs_ai\":\"{BS}u12\"}}"),
        "{\"instructs_ai\":\"open}".into(),
    ];
    for reply in cases {
        refused(&INJ_1, &reply, NotOneObject);
    }
}

#[test]
fn td_o6_duplicate_keys_are_refused_before_insertion_at_both_depths() {
    // Depth 1. A first-wins and a last-wins reader would disagree here.
    refused(
        &INJ_1,
        "{\"instructs_ai\":0.9,\"instructs_ai\":0.1}",
        DuplicateKey,
    );
    refused(
        &INJ_1,
        "{\"instructs_ai\":0.1,\"instructs_ai\":0.1}",
        DuplicateKey,
    );
    // The key is checked before its value.
    refused(
        &INJ_1,
        "{\"instructs_ai\":0.1,\"instructs_ai\":2}",
        DuplicateKey,
    );
    let sen = "{\"hours_conflict\":0.1,\"other_contact\":0.1,\"addressed_to_ai\":0.1,\"facts_consistent\":0.9,\"other_contact\":0.1}";
    refused(&SEN_1, sen, DuplicateKey);
    // Depth 2. A map that inserted first would keep one "fits" and accept a
    // sum of exactly one; keeping the first or the last would also differ.
    refused(
        mixed(),
        &mixed_with_fits("0.5,\"unclear\":0.5,\"fits\"", "0.5"),
        DuplicateKey,
    );
    let low_twice = "{\"noul_q\":0.1,\"choice_q\":{\"fits\":1,\"unclear\":0},\"score_q\":{\"low\":1,\"low\":0,\"mid\":0,\"high\":0}}";
    refused(mixed(), low_twice, DuplicateKey);
    // A label may repeat a top-level key's name: keys are per object.
    let reply = mixed_reply(100_000, [300_000, 700_000], [0, 0, 1_000_000]);
    assert!(decide(mixed(), &reply).is_ok(), "control");
}

#[test]
fn td_o6_unknown_and_missing_keys() {
    let escaped_key = format!("{{\"instructs{BS}u005fai\":0.1}}");
    let full = mixed_reply(0, [0, 1_000_000], [0, 0, 1_000_000]);
    let with_expected = format!("{},\"expected\":1}}", &full[..full.len() - 1]);
    let unknown: Vec<(&'static QuestionSet, String)> = vec![
        (&INJ_1, "{\"instructs_ai\":0.1,\"extra\":0}".into()),
        (&INJ_1, "{\"extra\":0,\"instructs_ai\":0.1}".into()),
        (&INJ_1, "{\"Instructs_ai\":0.1}".into()),
        (&INJ_1, "{\"instructs_ai \":0.1}".into()),
        (&INJ_1, "{\"\":0.1}".into()),
        // An escape is never decoded, so it cannot spell a registered key.
        (&INJ_1, escaped_key),
        // Another set's key.
        (&INJ_1, "{\"hours_conflict\":0.1}".into()),
        // `top` and `expected` are computed in code, never read.
        (mixed(), mixed_with_fits("0.5", "0.5,\"top\":\"fits\"")),
        (mixed(), with_expected),
        // Depth 2.
        (mixed(), mixed_with_fits("0.5,\"maybe\":0", "0.5")),
        (
            mixed(),
            format!(
                "{{\"noul_q\":0.1,\"choice_q\":{{\"f{BS}u0069ts\":0.5,\"unclear\":0.5}},\"score_q\":{{\"low\":0,\"mid\":0,\"high\":1}}}}"
            ),
        ),
    ];
    for (set, reply) in &unknown {
        refused(set, reply, UnknownKey);
    }
    let missing: Vec<(&'static QuestionSet, String)> = vec![
        (&INJ_1, "{}".into()),
        (
            &SEN_1,
            "{\"hours_conflict\":0.1,\"other_contact\":0.1,\"addressed_to_ai\":0.1}".into(),
        ),
        (
            mixed(),
            "{\"noul_q\":0.1,\"choice_q\":{\"fits\":1,\"unclear\":0}}".into(),
        ),
        (
            mixed(),
            mixed_with_fits("1", "0").replace(",\"unclear\":0", ""),
        ),
        (
            mixed(),
            "{\"noul_q\":0.1,\"choice_q\":{\"fits\":1,\"unclear\":0},\"score_q\":{}}".into(),
        ),
    ];
    for (set, reply) in &missing {
        refused(set, reply, MissingKey);
    }
    // The first failure in document order wins; a missing key is found when
    // its object closes.
    refused(&INJ_1, "{\"instructs_ai\":2,\"extra\":0}", OutOfRange);
    refused(&INJ_1, "{\"extra\":0,\"instructs_ai\":2}", UnknownKey);
    assert!(decide(&INJ_1, "{\"instructs_ai\":0.1}").is_ok(), "control");
}

#[test]
fn td_o6_nesting_deeper_than_two_is_too_deep() {
    assert!(
        decide(mixed(), &mixed_with_fits("1", "0")).is_ok(),
        "control: depth 2"
    );
    refused(&INJ_1, "{\"instructs_ai\":{\"a\":0.5}}", WrongType);
    refused(&INJ_1, "{\"instructs_ai\":[0.5]}", WrongType);
    refused(&INJ_1, "{\"instructs_ai\":{\"a\":{\"b\":0.5}}}", TooDeep);
    refused(&INJ_1, "{\"instructs_ai\":[[0.5]]}", TooDeep);
    refused(&INJ_1, "{\"instructs_ai\":{\"a\":[0.5]}}", TooDeep);
    refused(mixed(), &mixed_with_fits("{\"x\":1}", "0"), TooDeep);
    refused(mixed(), &mixed_with_fits("[1]", "0"), TooDeep);
    // Found when the third level opens: before the rest is read, and
    // without recursion on hostile nesting.
    refused(&INJ_1, "{\"a\":{\"b\":{", TooDeep);
    let deep_objects = format!(
        "{{\"instructs_ai\":{}0{}}}",
        "{\"a\":".repeat(1_300),
        "}".repeat(1_300)
    );
    assert!(deep_objects.len() <= MAX_REPLY_BYTES);
    refused(&INJ_1, &deep_objects, TooDeep);
    let deep_arrays = format!(
        "{{\"instructs_ai\":{}0{}}}",
        "[".repeat(4_000),
        "]".repeat(4_000)
    );
    assert!(deep_arrays.len() <= MAX_REPLY_BYTES);
    refused(&INJ_1, &deep_arrays, TooDeep);
}

#[test]
fn td_o6_every_number_form() {
    let table: Vec<(&str, Result<u32, DecodeCode>)> = vec![
        ("0", Ok(0)),
        ("1", Ok(1_000_000)),
        ("0.5", Ok(500_000)),
        ("0.25", Ok(250_000)),
        ("0.000001", Ok(1)),
        ("0.999999", Ok(999_999)),
        ("0.10", Ok(100_000)),
        ("0.100000", Ok(100_000)),
        ("0.0", Ok(0)),
        ("0.000000", Ok(0)),
        ("1.0", Ok(1_000_000)),
        ("1.000000", Ok(1_000_000)),
        ("0.0000001", Err(BadNumber)),
        ("1.0000001", Err(BadNumber)),
        ("0.1234567", Err(BadNumber)),
        ("1.0000000", Err(BadNumber)),
        ("-0", Err(BadNumber)),
        ("-0.5", Err(BadNumber)),
        ("+0.5", Err(BadNumber)),
        ("-1", Err(BadNumber)),
        ("1e-1", Err(BadNumber)),
        ("5E-1", Err(BadNumber)),
        ("0.5e0", Err(BadNumber)),
        ("1e0", Err(BadNumber)),
        (".5", Err(BadNumber)),
        ("00.5", Err(BadNumber)),
        ("01", Err(BadNumber)),
        ("00", Err(BadNumber)),
        ("0.", Err(BadNumber)),
        ("1.", Err(BadNumber)),
        ("0..5", Err(BadNumber)),
        ("0.5.0", Err(BadNumber)),
        ("NaN", Err(BadNumber)),
        ("nan", Err(BadNumber)),
        ("Infinity", Err(BadNumber)),
        ("-Infinity", Err(BadNumber)),
        ("0x1", Err(BadNumber)),
        ("0_5", Err(BadNumber)),
        ("\"0.5\"", Err(BadNumber)),
        ("\"\"", Err(BadNumber)),
        ("1.5", Err(OutOfRange)),
        ("1.000001", Err(OutOfRange)),
        ("1.1", Err(OutOfRange)),
        ("2", Err(OutOfRange)),
        ("10", Err(OutOfRange)),
        ("123456789012345678901234567890", Err(OutOfRange)),
        ("true", Err(WrongType)),
        ("false", Err(WrongType)),
        ("null", Err(WrongType)),
        ("[0.5]", Err(WrongType)),
        ("{}", Err(WrongType)),
    ];
    for (text, expected) in &table {
        // Depth 1.
        let got = decide(&INJ_1, &inj(text)).map(|d| noul_ppm(&d.answers()[0].1));
        assert_eq!(
            got,
            expected.map_err(ReplyError::Decode),
            "{text} at depth 1"
        );
        // Depth 2, as a Choice probability ("{}" and arrays are deeper there).
        if text.starts_with(['[', '{']) {
            continue;
        }
        let other = match expected {
            Ok(p) => dec(1_000_000 - p),
            Err(_) => "0.5".into(),
        };
        let got =
            decide(mixed(), &mixed_with_fits(text, &other)).map(|d| vector(&d.answers()[1].1));
        let want = expected.map(|p| vec![p, 1_000_000 - p]);
        assert_eq!(got, want.map_err(ReplyError::Decode), "{text} at depth 2");
    }
    // Every value is exactly the written decimal: no rounding, no float.
    for p in (0..=1_000_000u32)
        .step_by(7)
        .chain([999_999, 1_000_000, 700_000, 700_001])
    {
        let got = decide(&INJ_1, &inj(&dec(p))).map(|d| noul_ppm(&d.answers()[0].1));
        assert_eq!(got, Ok(p), "{}", dec(p));
        let padded = format!("{p:07}");
        let six = format!("{}.{}", &padded[..1], &padded[1..]);
        if p < 1_000_000 {
            assert_eq!(
                decide(&INJ_1, &inj(&six)).map(|d| noul_ppm(&d.answers()[0].1)),
                Ok(p),
                "{six}"
            );
        }
    }
}

#[test]
fn td_o6_discrete_forms() {
    let set = mixed_discrete();
    let d = decide(
        set,
        "{\"noul_q\":1,\"choice_q\":\"fits\",\"score_q\":\"high\"}",
    )
    .expect("decodes");
    assert_eq!(noul_ppm(&d.answers()[0].1), 1_000_000);
    assert_eq!(vector(&d.answers()[1].1), [1_000_000, 0]);
    assert_eq!(vector(&d.answers()[2].1), [0, 0, 1_000_000]);
    assert_eq!(d.mode(), crate::AnswerMode::Discrete);
    let d = decide(
        set,
        "{\"noul_q\":0,\"choice_q\":\"unclear\",\"score_q\":\"low\"}",
    )
    .expect("decodes");
    assert_eq!(noul_ppm(&d.answers()[0].1), 0);
    assert_eq!(vector(&d.answers()[1].1), [0, 1_000_000]);
    assert_eq!(vector(&d.answers()[2].1), [1_000_000, 0, 0]);
    let reply =
        |n: &str, c: &str, s: &str| format!("{{\"noul_q\":{n},\"choice_q\":{c},\"score_q\":{s}}}");
    for n in [
        "0.5", "1.0", "0.0", "\"1\"", "true", "null", "-0", "01", "2", "{}", "[1]",
    ] {
        refused(set, &reply(n, "\"fits\"", "\"low\""), WrongType);
    }
    let escaped = format!("\"f{BS}u0069ts\"");
    for c in [
        "\"maybe\"",
        "\"Fits\"",
        "\"fits \"",
        "\"\"",
        "1",
        "null",
        "[\"fits\"]",
        "{\"fits\":1,\"unclear\":0}",
        escaped.as_str(),
    ] {
        refused(set, &reply("1", c, "\"low\""), WrongType);
    }
    for s in [
        "\"fits\"",
        "\"HIGH\"",
        "2",
        "{\"low\":1,\"mid\":0,\"high\":0}",
    ] {
        refused(set, &reply("1", "\"fits\"", s), WrongType);
    }
    // Probabilities mode does not accept the discrete forms.
    refused(mixed(), &reply("1", "\"fits\"", "\"low\""), WrongType);
}

#[test]
fn td_o6_the_cap_is_8192_bytes_and_comes_first() {
    assert_eq!(MAX_REPLY_BYTES, 8_192);
    let good = inj("0.25");
    let at = format!("{good}{}", " ".repeat(MAX_REPLY_BYTES - good.len()));
    assert_eq!(at.len(), 8_192);
    for attempt in [Attempt::First, Attempt::Second] {
        assert!(decode(&INJ_1, &at, [0; 32], attempt).is_ok(), "8,192 bytes");
        let over = format!("{at} ");
        assert_eq!(over.len(), 8_193);
        assert_eq!(
            decode(&INJ_1, &over, [0; 32], attempt).map(|_| ()),
            Err(ReplyError::TooLarge),
            "8,193 bytes"
        );
    }
    // Before the reader: a malformed reply over the cap is too large, and
    // one at the cap gets its decoder code.
    let fence = |n: usize| format!("```{}", " ".repeat(n - 3));
    assert_eq!(
        decide(&INJ_1, &fence(8_192)).map(|_| ()),
        Err(ReplyError::Decode(NotOneObject))
    );
    assert_eq!(
        decide(&INJ_1, &fence(8_193)).map(|_| ()),
        Err(ReplyError::TooLarge)
    );
    // Bytes, not characters.
    let e_acute = char::from_u32(0xe9).unwrap().to_string();
    assert_eq!(e_acute.repeat(4_096).len(), 8_192);
    assert_eq!(
        decide(&INJ_1, &e_acute.repeat(4_096)).map(|_| ()),
        Err(ReplyError::Decode(NotOneObject))
    );
    assert_eq!(
        decide(&INJ_1, &e_acute.repeat(4_097)).map(|_| ()),
        Err(ReplyError::TooLarge)
    );
}

#[test]
fn td_o6_errors_carry_codes_only() {
    let cases = [
        (
            format!("{{\"instructs_ai\":\"{C}\"}}"),
            ReplyError::Decode(BadNumber),
        ),
        (format!("{{\"{CL}\":0.1}}"), ReplyError::Decode(UnknownKey)),
        (format!("```{C}```"), ReplyError::Decode(NotOneObject)),
        (
            format!("{{\"instructs_ai\":{CL}}}"),
            ReplyError::Decode(BadNumber),
        ),
        (format!("{}{C}", " ".repeat(8_193)), ReplyError::TooLarge),
    ];
    for (reply, expected) in cases {
        let error = decide(&INJ_1, &reply).map(|_| ()).unwrap_err();
        assert_eq!(error, expected, "{reply:.40}");
        let display = error.to_string();
        match error {
            ReplyError::TooLarge => assert_eq!(display, "td_reply_too_large"),
            ReplyError::Decode(code) => assert_eq!(display, format!("td_decode:{}", code.code())),
        }
        assert_no_canary("ReplyError Display", &display);
        assert_no_canary("ReplyError Debug", &format!("{error:?}"));
    }
    // A decided reply's Debug holds no answer value.
    let d = decide(&INJ_1, &inj("0.123457")).expect("decodes");
    let debug = format!("{d:?} {d:#?}");
    assert!(
        !debug.contains("123457") && !debug.contains("0.12"),
        "{debug}"
    );
}
