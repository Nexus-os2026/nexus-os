//! Shared support for the T1b oracles: replies written as exact decimals,
//! decided fixtures, bindings and the complete failure list.

use super::support::{choice, mixed_set, noul, question, score, test_set};
use crate::{
    decode, AnswerMode, Attempt, Decision, Failure, OptionDisposition, Polarity, PreflightCode,
    QuestionSet, Reading, ReplyError, TransportCode,
};

pub const B0: [u8; 32] = [0xb0; 32];
pub const B1: [u8; 32] = [0xb1; 32];

/// The exact decimal for a ppm value: `0`, `1`, or `0.` and at most six
/// digits.
pub fn dec(ppm: u32) -> String {
    match ppm {
        0 => "0".into(),
        1_000_000 => "1".into(),
        p => {
            assert!(p < 1_000_000);
            format!("0.{p:06}").trim_end_matches('0').to_string()
        }
    }
}

/// `{"k":v,...}`, compact, in the given order.
pub fn obj(members: &[(&str, String)]) -> String {
    let inner: Vec<String> = members
        .iter()
        .map(|(k, v)| format!("\"{k}\":{v}"))
        .collect();
    format!("{{{}}}", inner.join(","))
}

/// A Noul set's reply: one value text per question, in registry order.
pub fn noul_reply(set: &QuestionSet, values: &[&str]) -> String {
    let keys: Vec<&str> = set.keys().map(|k| k.as_str()).collect();
    assert_eq!(keys.len(), values.len());
    let members: Vec<(&str, String)> = keys
        .iter()
        .zip(values)
        .map(|(k, v)| (*k, v.to_string()))
        .collect();
    obj(&members)
}

pub fn decide(set: &'static QuestionSet, reply: &str) -> Result<Decision, ReplyError> {
    decode(set, reply, B0, Attempt::First)
}

/// Under the v1 policy (0.3, 0.7): Yes = 0.9, No = 0.1, Unsettled = 0.5.
pub fn value_for(reading: Reading) -> &'static str {
    match reading {
        Reading::Yes => "0.9",
        Reading::No => "0.1",
        Reading::Unsettled => "0.5",
        Reading::Selected(_) => panic!("a Noul has no Selected reading"),
    }
}

/// A decided Noul set whose readings are exactly `readings`.
pub fn with_readings(set: &'static QuestionSet, readings: &[Reading]) -> Decision {
    let values: Vec<&str> = readings.iter().map(|r| value_for(*r)).collect();
    let decision = decide(set, &noul_reply(set, &values)).expect("decodes");
    let got: Vec<Reading> = decision.readings().iter().map(|(_, r)| *r).collect();
    assert_eq!(got, readings, "fixture readings");
    decision
}

/// Every combination of `n` Noul readings.
pub fn reading_combos(n: usize) -> Vec<Vec<Reading>> {
    let mut all = vec![Vec::new()];
    for _ in 0..n {
        all = all
            .into_iter()
            .flat_map(|prefix| {
                [Reading::Yes, Reading::No, Reading::Unsettled].map(|r| {
                    let mut next = prefix.clone();
                    next.push(r);
                    next
                })
            })
            .collect();
    }
    all
}

/// Every failure a lane can receive. The match fails to compile if a
/// variant is added without being listed here.
pub fn all_failures() -> Vec<Failure> {
    fn listed(failure: Failure) {
        match failure {
            Failure::Refused(_)
            | Failure::CoverageInsufficient
            | Failure::Transport(_)
            | Failure::ReplyTooLarge
            | Failure::Malformed
            | Failure::Stopped
            | Failure::Cancelled
            | Failure::Timeout
            | Failure::Late
            | Failure::Stale
            | Failure::Unrecorded
            | Failure::Missing => {}
        }
    }
    let mut all: Vec<Failure> = PreflightCode::ALL.map(Failure::Refused).to_vec();
    all.push(Failure::CoverageInsufficient);
    all.extend(TransportCode::ALL.map(Failure::Transport));
    all.extend([
        Failure::ReplyTooLarge,
        Failure::Malformed,
        Failure::Stopped,
        Failure::Cancelled,
        Failure::Timeout,
        Failure::Late,
        Failure::Stale,
        Failure::Unrecorded,
        Failure::Missing,
    ]);
    all.iter().copied().for_each(listed);
    assert_eq!(all.len(), 5 + 1 + 23 + 9);
    all
}

/// T1a's mixed set: `noul_q` (risk), `choice_q` (`fits` clear, `unclear`
/// needs the Owner), `score_q` (`low`, `mid`, `high`; at least `high`;
/// positive); policy (0.3, 0.7).
pub fn mixed() -> &'static QuestionSet {
    mixed_set()
}

/// The same questions in discrete mode.
pub fn mixed_discrete() -> &'static QuestionSet {
    test_set(
        "dis_1",
        AnswerMode::Discrete,
        mixed_set().questions().to_vec(),
    )
}

/// A reply for the mixed set from exact ppm values.
pub fn mixed_reply(noul_q: u32, choice_q: [u32; 2], score_q: [u32; 3]) -> String {
    let choice = obj(&[("fits", dec(choice_q[0])), ("unclear", dec(choice_q[1]))]);
    let score = obj(&[
        ("low", dec(score_q[0])),
        ("mid", dec(score_q[1])),
        ("high", dec(score_q[2])),
    ]);
    obj(&[
        ("noul_q", dec(noul_q)),
        ("choice_q", choice),
        ("score_q", score),
    ])
}

/// A one-question set with a three-label Choice (all clear) and `yes_above`
/// as given.
pub fn choice3(yes_above: u32) -> &'static QuestionSet {
    use OptionDisposition::Clear;
    let mut q = question("pick", choice(&[("a", Clear), ("b", Clear), ("c", Clear)]));
    q.read = crate::ReadPolicy::new(300_000.min(yes_above - 1), yes_above);
    test_set("ch3_1", AnswerMode::Probabilities, vec![q])
}

/// A one-question Score set (`lo`, `mid`, `hi`) with the given `at_least`.
pub fn score3(at_least: u8) -> &'static QuestionSet {
    let q = question(
        "lvl",
        score(&["lo", "mid", "hi"], at_least, Polarity::Positive),
    );
    test_set("sc3_1", AnswerMode::Probabilities, vec![q])
}

/// A one-question Noul set with a custom policy.
pub fn noul_with(no_below: u32, yes_above: u32) -> &'static QuestionSet {
    let mut q = question("q", noul(Polarity::Risk));
    q.read = crate::ReadPolicy::new(no_below, yes_above);
    test_set("nl_1", AnswerMode::Probabilities, vec![q])
}
