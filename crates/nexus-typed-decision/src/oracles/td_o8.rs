//! TD-O8: readings (§7, `td-read-v1`) and the polarity mapping (§2.1).
//! Policy validation (`no_below = yes_above` refused, `yes_above > 10^6`
//! refused, a Choice below 500,000 refused); the thresholds are strict:
//! 0.7 and 0.3 are Unsettled, 0.700001 is Yes, 0.299999 is No; a Choice
//! selects only a unique top above `yes_above`; a Score reads its
//! cumulative probability; risk and positive polarity map as registered.

use super::support::{choice, noul, question, test_set, Rng};
use super::support_b::{choice3, dec, decide, mixed, mixed_reply, noul_with, obj, score3};
use crate::{
    read, validate, verdict, AnswerMode, Label, OptionDisposition, Polarity, Ppm, QuestionSet,
    ReadPolicy, Reading, RegistryError, Verdict, INJ_1, REGISTRY, SEN_1, VER_1,
};
use Reading::{No, Unsettled, Yes};

fn read_one(set: &'static QuestionSet, reply: &str) -> Reading {
    let d = decide(set, reply).expect("decodes");
    let reading = d.readings()[0].1;
    // `read` on the decoded answer agrees with the decision's reading.
    assert_eq!(read(&set.questions()[0], &d.answers()[0].1), reading);
    reading
}

fn noul_reading(set: &'static QuestionSet, value: &str) -> Reading {
    let key = set.questions()[0].id().as_str();
    read_one(set, &obj(&[(key, value.to_string())]))
}

#[test]
fn td_o8_noul_thresholds_are_strict() {
    let table = [
        ("0.7", Unsettled),
        ("0.700001", Yes),
        ("0.3", Unsettled),
        ("0.299999", No),
        ("0.300001", Unsettled),
        ("0.699999", Unsettled),
        ("0.5", Unsettled),
        ("0", No),
        ("1", Yes),
        ("0.000001", No),
        ("0.999999", Yes),
    ];
    for set in [&INJ_1, &SEN_1, &VER_1] {
        for (value, expected) in table {
            let values: Vec<String> = set.keys().map(|_| value.to_string()).collect();
            let keys: Vec<&str> = set.keys().map(|k| k.as_str()).collect();
            let members: Vec<(&str, String)> = keys.into_iter().zip(values).collect();
            let d = decide(set, &obj(&members)).expect("decodes");
            for (i, (id, reading)) in d.readings().iter().enumerate() {
                assert_eq!(*id, set.questions()[i].id());
                assert_eq!(*reading, expected, "{} = {value}", id.as_str());
            }
        }
    }
    // The extreme valid policies: equality with either threshold is never
    // a Yes or a No.
    assert_eq!(noul_reading(noul_with(0, 1_000_000), "0"), Unsettled);
    assert_eq!(noul_reading(noul_with(0, 1_000_000), "1"), Unsettled);
    assert_eq!(noul_reading(noul_with(0, 1), "0.000001"), Unsettled);
    assert_eq!(noul_reading(noul_with(0, 1), "0.000002"), Yes);
    assert_eq!(noul_reading(noul_with(999_999, 1_000_000), "0.999998"), No);
}

#[test]
fn td_o8_policy_validation() {
    let with_policy = |q: crate::Question, no_below: u32, yes_above: u32| {
        let mut q = q;
        q.read = ReadPolicy {
            no_below: Ppm(no_below),
            yes_above: Ppm(yes_above),
        };
        validate(test_set("p_1", AnswerMode::Probabilities, vec![q]))
    };
    let risk = || question("q", noul(Polarity::Risk));
    let pick = || {
        question(
            "q",
            choice(&[
                ("a", OptionDisposition::Clear),
                ("b", OptionDisposition::Restrict),
            ]),
        )
    };
    let bad = Err(RegistryError::BadPolicy);
    assert_eq!(
        with_policy(risk(), 700_000, 700_000),
        bad,
        "no_below = yes_above"
    );
    assert_eq!(with_policy(risk(), 0, 0), bad);
    assert_eq!(with_policy(risk(), 0, 1_000_001), bad, "yes_above > 10^6");
    assert_eq!(with_policy(risk(), 700_000, 300_000), bad);
    assert_eq!(
        with_policy(pick(), 0, 499_999),
        bad,
        "a Choice below 500,000"
    );
    assert_eq!(with_policy(pick(), 0, 500_000), Ok(()));
    assert_eq!(with_policy(risk(), 0, 499_999), Ok(()));
    for set in REGISTRY {
        assert_eq!(validate(set), Ok(()));
    }
    // A validated Choice policy with yes_above at its floor reads strictly.
    let reply = |a: u32, b: u32| {
        obj(&[(
            "pick",
            obj(&[("a", dec(a)), ("b", dec(b)), ("c", "0".into())]),
        )])
    };
    let label_a = Reading::Selected(Label::new("a"));
    assert_eq!(
        read_one(choice3(500_000), &reply(500_001, 499_999)),
        label_a
    );
    assert_eq!(
        read_one(choice3(500_000), &reply(500_000, 500_000)),
        Unsettled
    );
}

#[test]
fn td_o8_choice_and_score_readings() {
    let fits = Reading::Selected(Label::new("fits"));
    let unclear = Reading::Selected(Label::new("unclear"));
    let choice = |v: [u32; 2]| {
        decide(mixed(), &mixed_reply(0, v, [0, 0, 1_000_000]))
            .expect("decodes")
            .readings()[1]
            .1
    };
    assert_eq!(choice([700_001, 299_999]), fits);
    assert_eq!(choice([299_999, 700_001]), unclear);
    assert_eq!(choice([700_000, 300_000]), Unsettled, "equal to yes_above");
    assert_eq!(choice([500_000, 500_000]), Unsettled, "a tie");
    assert_eq!(choice([1_000_000, 0]), fits);
    // A Score reads P(level >= at_least) as a Noul: `score_q` is at least `high`.
    let score = |v: [u32; 3]| {
        decide(mixed(), &mixed_reply(0, [0, 1_000_000], v))
            .expect("decodes")
            .readings()[2]
            .1
    };
    assert_eq!(score([0, 299_999, 700_001]), Yes);
    assert_eq!(score([0, 300_000, 700_000]), Unsettled);
    assert_eq!(score([300_000, 400_000, 300_000]), Unsettled);
    assert_eq!(score([400_001, 300_000, 299_999]), No);
    assert_eq!(
        score([0, 1_000_000, 0]),
        No,
        "only the levels from at_least count"
    );
    // At least `mid`: the cumulative probability of `mid` and `hi`.
    let lvl = |v: [u32; 3]| {
        read_one(
            score3(1),
            &obj(&[(
                "lvl",
                obj(&[("lo", dec(v[0])), ("mid", dec(v[1])), ("hi", dec(v[2]))]),
            )]),
        )
    };
    assert_eq!(lvl([299_999, 0, 700_001]), Yes);
    assert_eq!(lvl([299_999, 700_001, 0]), Yes);
    assert_eq!(lvl([300_000, 400_000, 300_000]), Unsettled);
    assert_eq!(lvl([700_001, 0, 299_999]), No);
    assert_eq!(lvl([700_000, 150_000, 150_000]), Unsettled);
}

#[test]
fn td_o8_at_most_one_label_is_ever_selected_and_a_raw_tie_never_is() {
    let mut rng = Rng(0x7d08);
    for _ in 0..3_000 {
        let yes_above = 500_000 + rng.below(500_001) as u32;
        let a = rng.below(1_000_001) as u32;
        let b = rng.below(1_000_001 - a as usize) as u32;
        let tie = rng.below(4) == 0;
        let (a, b, c) = if tie {
            (a / 2, a / 2, 1_000_000 - 2 * (a / 2))
        } else {
            (a, b, 1_000_000 - a - b)
        };
        let reply = obj(&[("pick", obj(&[("a", dec(a)), ("b", dec(b)), ("c", dec(c))]))]);
        let d = decide(choice3(yes_above), &reply).expect("decodes");
        let above = [a, b, c].iter().filter(|p| **p > yes_above).count();
        assert!(above <= 1);
        let expected = match () {
            _ if a > yes_above => Reading::Selected(Label::new("a")),
            _ if b > yes_above => Reading::Selected(Label::new("b")),
            _ if c > yes_above => Reading::Selected(Label::new("c")),
            _ => Unsettled,
        };
        assert_eq!(
            d.readings()[0].1,
            expected,
            "({a}, {b}, {c}) over {yes_above}"
        );
    }
    // A three-way raw tie, renormalized, is never selected under any valid
    // Choice policy, although the remainder makes the first label the top.
    for yes_above in [500_000, 600_000, 700_000, 999_999] {
        for s in [316_667, 333_333, 350_000] {
            let reply = obj(&[("pick", obj(&[("a", dec(s)), ("b", dec(s)), ("c", dec(s))]))]);
            assert_eq!(read_one(choice3(yes_above), &reply), Unsettled, "{s} x 3");
        }
    }
}

#[test]
fn td_o8_the_polarity_mapping() {
    let restrict = Verdict::Restrict;
    let clear = Verdict::Clear;
    let risk = question("r", noul(Polarity::Risk));
    let positive = question("p", noul(Polarity::Positive));
    for (q, yes, no) in [(&risk, restrict, clear), (&positive, clear, restrict)] {
        assert_eq!(verdict(q, Yes), yes);
        assert_eq!(verdict(q, No), no);
        assert_eq!(
            verdict(q, Unsettled),
            restrict,
            "Unsettled always restricts"
        );
        assert_eq!(
            verdict(q, Reading::Selected(Label::new("x"))),
            restrict,
            "not a Noul reading"
        );
    }
    // Every registered question, by its registered polarity.
    let expected = [
        ("hours_conflict", restrict, clear),
        ("other_contact", restrict, clear),
        ("addressed_to_ai", restrict, clear),
        ("facts_consistent", clear, restrict),
        ("name_shown", clear, restrict),
        ("hours_shown", clear, restrict),
        ("phone_shown", clear, restrict),
        ("instructs_ai", restrict, clear),
    ];
    let all: Vec<_> = REGISTRY.iter().flat_map(|s| s.questions()).collect();
    for (q, (id, yes, no)) in all.iter().zip(expected) {
        assert_eq!(q.id().as_str(), id);
        assert_eq!(
            (verdict(q, Yes), verdict(q, No), verdict(q, Unsettled)),
            (yes, no, restrict),
            "{id}"
        );
    }
    // A Choice: the selected option's registered disposition; anything
    // else restricts. A Score maps like a Noul.
    let set = mixed();
    let (pick, level) = (&set.questions()[1], &set.questions()[2]);
    assert_eq!(verdict(pick, Reading::Selected(Label::new("fits"))), clear);
    assert_eq!(
        verdict(pick, Reading::Selected(Label::new("unclear"))),
        Verdict::NeedsOwner
    );
    assert_eq!(
        verdict(pick, Reading::Selected(Label::new("other"))),
        restrict,
        "unregistered"
    );
    for r in [Yes, No, Unsettled] {
        assert_eq!(verdict(pick, r), restrict);
    }
    assert_eq!(
        (
            verdict(level, Yes),
            verdict(level, No),
            verdict(level, Unsettled)
        ),
        (clear, restrict, restrict)
    );
    // Reading codes.
    let codes = [Yes, No, Unsettled, Reading::Selected(Label::new("fits"))].map(|r| r.code());
    assert_eq!(codes, ["yes", "no", "unsettled", "selected"]);
}
