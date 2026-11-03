//! TD-O9: the lane tables of §2.2 to §2.4 as pure functions, exhaustive over
//! every deterministic verdict, every reading and every failure. Never more
//! permissive than the deterministic verdict; `NeedsOwner` never eligible;
//! the Verifier never passes when D fails. The Sentinel's "token
//! invalidated before settlement" row is the slot's (TD-O12): that
//! assessment's result can never be installed.

use super::support::{mixed_set, note, TEST_INVENTORY};
use super::support_b::{all_failures, decide, mixed_reply, reading_combos, with_readings};
use crate::{
    compose, notes_screen, prepare, sentinel, verifier, Advice, Check, Decision, Disposition,
    Failure, PreflightCode, QuestionId, Reading, Reason, SentinelResult, TransportCode, INJ_1,
    SEN_1, VER_1,
};
use Reading::{No, Unsettled, Yes};

const CHECKS: [Check; 3] = [Check::Pass, Check::Fail, Check::Error];

/// The design's polarity rule, restated: risk clears only on No, positive
/// only on Yes.
fn clears(positive: bool, reading: Reading) -> bool {
    if positive {
        reading == Yes
    } else {
        reading == No
    }
}

fn qid(set: &crate::QuestionSet, i: usize) -> QuestionId {
    set.questions()[i].id()
}

/// An unrelated decision: required advice of another set is missing.
fn wrong_set() -> Decision {
    with_readings(&INJ_1, &[No])
}

#[test]
fn td_o9_the_sentinel_table() {
    let polarity_positive = [false, false, false, true];
    let decided: Vec<(Vec<Reading>, Decision)> = reading_combos(4)
        .into_iter()
        .map(|r| {
            let d = with_readings(&SEN_1, &r);
            (r, d)
        })
        .collect();
    assert_eq!(decided.len(), 81);
    let other = wrong_set();
    let mut clear_seen = 0;
    for s1a in CHECKS {
        let mut cases: Vec<(Option<Advice<'_>>, SentinelResult)> = Vec::new();
        cases.push((None, SentinelResult::Halt(Reason::Td(Failure::Missing))));
        cases.push((
            Some(Advice::Decided(&other)),
            SentinelResult::Halt(Reason::Td(Failure::Missing)),
        ));
        for failure in all_failures() {
            cases.push((
                Some(Advice::Failed(failure)),
                SentinelResult::Halt(Reason::Td(failure)),
            ));
        }
        for (readings, d) in &decided {
            let first = (0..4).find(|i| !clears(polarity_positive[*i], readings[*i]));
            let expected = match first {
                None => SentinelResult::Clear,
                Some(i) => SentinelResult::Halt(Reason::Question(qid(&SEN_1, i))),
            };
            cases.push((Some(Advice::Decided(d)), expected));
        }
        for (advice, expected) in cases {
            // S1a first: after a refusal there is no judge call, and S1a's
            // result is returned unchanged whatever advice is passed.
            let expected = if s1a == Check::Pass {
                expected
            } else {
                SentinelResult::S1aRefusal(s1a)
            };
            let got = sentinel(s1a, advice);
            assert_eq!(got, expected, "S1a {s1a:?}, advice {advice:?}");
            assert_eq!(got.eligible(), got == SentinelResult::Clear);
            if got.eligible() {
                assert_eq!(s1a, Check::Pass, "never more permissive than S1a");
                clear_seen += 1;
            }
        }
    }
    assert_eq!(clear_seen, 1, "exactly one combination clears");
    // The reason codes.
    let halt = |advice| match sentinel(Check::Pass, Some(advice)) {
        SentinelResult::Halt(reason) => reason.code(),
        other => panic!("{other:?}"),
    };
    let codes = [
        (
            Failure::Refused(PreflightCode::CharNotAllowed),
            "td_refused:char_not_allowed",
        ),
        (
            Failure::Refused(PreflightCode::RequestTooLarge),
            "td_refused:request_too_large",
        ),
        (Failure::CoverageInsufficient, "td_coverage_insufficient"),
        (
            Failure::Transport(TransportCode::Timeout),
            "td_transport:timeout",
        ),
        (
            Failure::Transport(TransportCode::OverBudget),
            "td_transport:over_budget",
        ),
        (Failure::ReplyTooLarge, "td_reply_too_large"),
        (Failure::Malformed, "td_malformed"),
        (Failure::Stopped, "td_stopped"),
        (Failure::Cancelled, "td_cancelled"),
        (Failure::Timeout, "td_timeout"),
        (Failure::Late, "td_late"),
        (Failure::Stale, "td_stale"),
        (Failure::Unrecorded, "td_unrecorded"),
        (Failure::Missing, "td_missing"),
    ];
    for (failure, code) in codes {
        assert_eq!(halt(Advice::Failed(failure)), code);
    }
    let facts_bad = with_readings(&SEN_1, &[No, No, No, No]);
    assert_eq!(
        halt(Advice::Decided(&facts_bad)),
        "facts_consistent",
        "positive polarity"
    );
    let two = with_readings(&SEN_1, &[No, Yes, Unsettled, Yes]);
    assert_eq!(
        halt(Advice::Decided(&two)),
        "other_contact",
        "the first non-clear question"
    );
    assert_eq!(
        sentinel(Check::Pass, None),
        SentinelResult::Halt(Reason::Td(Failure::Missing)),
        "required but absent"
    );
}

/// What A is for each fact: a reading, or the failure that stands for it.
type PerFact = [Result<Reading, Failure>; 3];

/// §2.3's per-fact table, restated.
fn fact(d: Check, a: Result<Reading, Failure>) -> Disposition {
    match (d, a) {
        (Check::Error, _) => Disposition::Restrict(Reason::CheckError),
        (Check::Fail, Ok(Yes)) => Disposition::NeedsOwner(Reason::FormatVariant),
        (Check::Fail, _) => Disposition::Restrict(Reason::Deterministic),
        (Check::Pass, Ok(Yes)) => Disposition::Clear,
        (Check::Pass, Ok(No)) => Disposition::Restrict(Reason::AdviceDisagrees),
        (Check::Pass, Ok(_)) => Disposition::Restrict(Reason::AdviceUnsettled),
        (Check::Pass, Err(failure)) => Disposition::Restrict(Reason::AdviceUnavailable(failure)),
    }
}

#[test]
fn td_o9_the_verifier_table() {
    let decided: Vec<(Vec<Reading>, Decision)> = reading_combos(3)
        .into_iter()
        .map(|r| {
            let d = with_readings(&VER_1, &r);
            (r, d)
        })
        .collect();
    let other = wrong_set();
    // Per fact, what A is for each advice.
    let mut advices: Vec<(Option<Advice<'_>>, PerFact)> = vec![
        (None, [Err(Failure::Missing); 3]),
        (Some(Advice::Decided(&other)), [Err(Failure::Missing); 3]),
    ];
    for failure in all_failures() {
        // Insufficient coverage reads every question Unsettled (§4.5).
        let a = if failure == Failure::CoverageInsufficient {
            Ok(Unsettled)
        } else {
            Err(failure)
        };
        advices.push((Some(Advice::Failed(failure)), [a; 3]));
    }
    for (r, d) in &decided {
        advices.push((Some(Advice::Decided(d)), [Ok(r[0]), Ok(r[1]), Ok(r[2])]));
    }
    let mut passes = 0;
    let mut needs_owner = 0;
    for d0 in CHECKS {
        for d1 in CHECKS {
            for d2 in CHECKS {
                let d = [d0, d1, d2];
                for (advice, a) in &advices {
                    let got = verifier(d, *advice);
                    let facts = [fact(d0, a[0]), fact(d1, a[1]), fact(d2, a[2])];
                    assert_eq!(got.facts(), facts, "D {d:?}, advice {advice:?}");
                    let worst = facts.iter().map(|f| f.rank()).max().unwrap();
                    let overall = *facts.iter().find(|f| f.rank() == worst).unwrap();
                    assert_eq!(got.overall(), overall, "D {d:?}, advice {advice:?}");
                    assert_eq!(got.eligible(), overall == Disposition::Clear);
                    if got.eligible() {
                        assert_eq!(d, [Check::Pass; 3], "never a pass when D fails");
                        passes += 1;
                    }
                    if matches!(overall, Disposition::NeedsOwner(_)) {
                        assert!(!got.eligible(), "NeedsOwner is never eligible");
                        needs_owner += 1;
                    }
                }
            }
        }
    }
    assert_eq!(passes, 1, "only D pass and A Yes for every fact");
    assert!(needs_owner > 0);
    // Codes, and both views kept: D's failure stays recorded with a format variant.
    let yes = with_readings(&VER_1, &[Yes, Yes, Yes]);
    let got = verifier(
        [Check::Pass, Check::Fail, Check::Pass],
        Some(Advice::Decided(&yes)),
    );
    assert_eq!(got.overall().reason().unwrap().code(), "format_variant");
    let failed = verifier([Check::Pass; 3], Some(Advice::Failed(Failure::Stale)));
    assert_eq!(
        failed.overall().reason().unwrap().code(),
        "advice_unavailable:stale"
    );
    let transport = Some(Advice::Failed(Failure::Transport(TransportCode::Late)));
    assert_eq!(
        verifier([Check::Pass; 3], transport).facts()[2]
            .reason()
            .unwrap()
            .code(),
        "advice_unavailable:transport:late"
    );
    let codes = [
        Reason::AdviceDisagrees,
        Reason::AdviceUnsettled,
        Reason::Deterministic,
        Reason::CheckError,
    ]
    .map(|r| r.code());
    assert_eq!(
        codes,
        [
            "advice_disagrees",
            "advice_unsettled",
            "deterministic",
            "check_error"
        ]
    );
}

#[test]
fn td_o9_the_notes_screen_table() {
    let needs = |r: Reason| Disposition::NeedsOwner(r);
    let other = with_readings(&SEN_1, &[No, No, No, Yes]);
    let mut cases: Vec<(Option<Advice<'_>>, Disposition)> = vec![
        (None, needs(Reason::NotesUnavailable(Failure::Missing))),
        (
            Some(Advice::Decided(&other)),
            needs(Reason::NotesUnavailable(Failure::Missing)),
        ),
    ];
    for failure in all_failures() {
        let expected = if failure == Failure::CoverageInsufficient {
            needs(Reason::NotesUnsettled)
        } else {
            needs(Reason::NotesUnavailable(failure))
        };
        cases.push((Some(Advice::Failed(failure)), expected));
    }
    let decided: Vec<(Reading, Decision)> = [Yes, No, Unsettled]
        .map(|r| (r, with_readings(&INJ_1, &[r])))
        .into();
    for (r, d) in &decided {
        let expected = match r {
            No => Disposition::Clear,
            Yes => needs(Reason::NotesInstructsAi),
            _ => needs(Reason::NotesUnsettled),
        };
        cases.push((Some(Advice::Decided(d)), expected));
    }
    for (advice, expected) in cases {
        let got = notes_screen(advice);
        assert_eq!(got, expected, "{advice:?}");
        assert_eq!(got.eligible(), got == Disposition::Clear);
        assert!(
            !matches!(got, Disposition::Restrict(_)),
            "never a silent refusal"
        );
    }
    let codes = [
        Reason::NotesInstructsAi,
        Reason::NotesUnsettled,
        Reason::NotesUnavailable(Failure::Refused(PreflightCode::CharNotAllowed)),
        Reason::NotesUnavailable(Failure::Stale),
    ]
    .map(|r| r.code());
    assert_eq!(
        codes,
        [
            "notes_screen:instructs_ai",
            "notes_screen:unsettled",
            "notes_screen:unavailable:refused:char_not_allowed",
            "notes_screen:unavailable:stale"
        ]
    );
}

#[test]
fn td_o9_the_generic_composition_rule() {
    let set = mixed_set();
    let (n, c, s) = (qid(set, 0), qid(set, 1), qid(set, 2));
    // noul_q is risk, choice_q's `unclear` needs the Owner, score_q is
    // positive (at least `high`).
    let decide_mixed = |noul: u32, choice: [u32; 2], score: [u32; 3]| {
        decide(set, &mixed_reply(noul, choice, score)).expect("decodes")
    };
    let table = [
        (
            100_000,
            [900_000, 100_000],
            [0, 0, 1_000_000],
            Disposition::Clear,
        ),
        (
            900_000,
            [900_000, 100_000],
            [0, 0, 1_000_000],
            Disposition::Restrict(Reason::Question(n)),
        ),
        (
            100_000,
            [100_000, 900_000],
            [0, 0, 1_000_000],
            Disposition::NeedsOwner(Reason::Question(c)),
        ),
        (
            100_000,
            [500_000, 500_000],
            [0, 0, 1_000_000],
            Disposition::Restrict(Reason::Question(c)),
        ),
        (
            100_000,
            [900_000, 100_000],
            [0, 500_000, 500_000],
            Disposition::Restrict(Reason::Question(s)),
        ),
        // The most restrictive verdict, with the first non-clear question.
        (
            100_000,
            [100_000, 900_000],
            [0, 500_000, 500_000],
            Disposition::Restrict(Reason::Question(c)),
        ),
        (
            500_000,
            [100_000, 900_000],
            [0, 0, 1_000_000],
            Disposition::Restrict(Reason::Question(n)),
        ),
    ];
    for (noul, choice, score, expected) in table {
        let d = decide_mixed(noul, choice, score);
        assert_eq!(compose(&d), expected, "{noul} {choice:?} {score:?}");
    }
    // Eligibility: only Clear, whatever the reason.
    assert!(Disposition::Clear.eligible());
    let reasons = [
        Reason::NotesUnsettled,
        Reason::FormatVariant,
        Reason::Question(n),
        Reason::Td(Failure::Stale),
    ];
    for reason in reasons {
        assert!(
            !Disposition::NeedsOwner(reason).eligible(),
            "NeedsOwner is never eligible"
        );
        assert!(!Disposition::Restrict(reason).eligible());
        assert!(!SentinelResult::Halt(reason).eligible());
    }
    assert!(!SentinelResult::S1aRefusal(Check::Fail).eligible());
    assert!(SentinelResult::Clear.eligible());
}

#[test]
fn td_o9_preflight_refusals_and_transport_codes_map_exactly() {
    // T1a's refusals become the lane failures.
    let over = note(&"x".repeat(3_000));
    let refused = |p: &crate::Projection, set: &'static crate::QuestionSet| {
        prepare(set, p, &TEST_INVENTORY).expect_err("refused")
    };
    let coverage = refused(&over, &INJ_1);
    assert_eq!(
        Failure::from_refusal(&coverage),
        Failure::CoverageInsufficient
    );
    let mismatch = refused(&note("a"), &SEN_1);
    assert_eq!(
        Failure::from_refusal(&mismatch),
        Failure::Refused(PreflightCode::ProjectionMismatch)
    );
    let delimiter = refused(&note("a <bos> b"), &INJ_1);
    assert_eq!(
        Failure::from_refusal(&delimiter),
        Failure::Refused(PreflightCode::UnsafeDelimiter)
    );
    let nbsp = format!("a{}b", char::from_u32(0xa0).unwrap());
    let chars = refused(&note(&nbsp), &INJ_1);
    assert_eq!(
        Failure::from_refusal(&chars),
        Failure::Refused(PreflightCode::CharNotAllowed)
    );
    // A preflight failure's code is its refusal's outcome.
    for refusal in [&mismatch, &delimiter, &chars] {
        assert_eq!(Failure::from_refusal(refusal).code(), refusal.outcome());
    }
    assert_eq!(
        sentinel(
            Check::Pass,
            Some(Advice::Failed(Failure::from_refusal(&coverage)))
        ),
        SentinelResult::Halt(Reason::Td(Failure::CoverageInsufficient))
    );
    assert_eq!(
        PreflightCode::ALL.map(|c| c.code()),
        [
            "projection_mismatch",
            "unrenderable",
            "char_not_allowed",
            "unsafe_delimiter",
            "request_too_large"
        ]
    );
    // A1's codes, in A1's order (ModelError::CODES at bdedc7fb).
    let a1 = [
        "not_local",
        "invalid_address",
        "invalid_model_name",
        "cloud_name",
        "cloud_enabled",
        "not_installed",
        "digest_mismatch",
        "runtime_mismatch",
        "remote",
        "config_mismatch",
        "unavailable",
        "timeout",
        "request_too_large",
        "response_too_large",
        "malformed",
        "truncated",
        "tool_calls",
        "thinking_leaked",
        "empty_content",
        "over_budget",
        "stopped",
        "cancelled",
        "late",
    ];
    assert_eq!(TransportCode::ALL.map(|c| c.code()), a1);
}
