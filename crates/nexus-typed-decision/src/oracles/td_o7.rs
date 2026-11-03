//! TD-O7: normalization (§7, `td-normalize-v1`). An exact sum is kept; a
//! sum within 50,000 ppm of one million is renormalized in integers by
//! largest remainder, ties to the earlier registry label; 50,001 away, and
//! a zero sum, are `bad_sum`, with no uniform fallback; `top` is computed
//! in code and equal largest values are a `Tie`; a Score's expectation is
//! computed in code and never read.

use super::support::Rng;
use super::support_b::{choice3, dec, decide, mixed, mixed_reply, obj, score3};
use crate::{Answer, Decision, DecodeCode, Distribution, ReplyError, Top};

fn dist(answer: &Answer) -> &Distribution {
    match answer {
        Answer::Choice(d) | Answer::Score(d) => d,
        other => panic!("not a vector: {other:?}"),
    }
}

fn ppms(answer: &Answer) -> Vec<u32> {
    dist(answer)
        .entries()
        .iter()
        .map(|(_, p)| p.get())
        .collect()
}

fn mixed_ok(choice: [u32; 2], score: [u32; 3]) -> Decision {
    decide(mixed(), &mixed_reply(100_000, choice, score)).expect("decodes")
}

fn mixed_err(choice: [u32; 2], score: [u32; 3]) -> Result<(), ReplyError> {
    decide(mixed(), &mixed_reply(100_000, choice, score)).map(|_| ())
}

const BAD_SUM: Result<(), ReplyError> = Err(ReplyError::Decode(DecodeCode::BadSum));

#[test]
fn td_o7_exact_sums_are_kept() {
    for (choice, score) in [
        ([300_000, 700_000], [200_000, 300_000, 500_000]),
        ([1_000_000, 0], [0, 0, 1_000_000]),
        ([0, 1_000_000], [1_000_000, 0, 0]),
        ([999_999, 1], [1, 999_998, 1]),
    ] {
        let d = mixed_ok(choice, score);
        assert_eq!(ppms(&d.answers()[1].1), choice);
        assert_eq!(ppms(&d.answers()[2].1), score);
        assert!(!dist(&d.answers()[1].1).renormalized());
        assert!(!dist(&d.answers()[2].1).renormalized());
        assert!(!d.renormalized());
    }
}

#[test]
fn td_o7_the_tolerance_is_50000_either_side() {
    let exact = [500_000, 500_000];
    // (score values, sum, normalized or bad_sum)
    let table: [([u32; 3], u32, Option<[u32; 3]>); 6] = [
        (
            [300_000, 300_000, 350_000],
            950_000,
            Some([315_790, 315_789, 368_421]),
        ),
        (
            [300_000, 300_000, 350_001],
            950_001,
            Some([315_789, 315_789, 368_422]),
        ),
        ([300_000, 300_000, 349_999], 949_999, None),
        (
            [350_000, 350_000, 350_000],
            1_050_000,
            Some([333_334, 333_333, 333_333]),
        ),
        (
            [350_000, 350_000, 349_999],
            1_049_999,
            Some([333_334, 333_333, 333_333]),
        ),
        ([350_001, 350_000, 350_000], 1_050_001, None),
    ];
    for (score, sum, expected) in table {
        assert_eq!(score.iter().sum::<u32>(), sum);
        match expected {
            Some(normalized) => {
                let d = mixed_ok(exact, score);
                assert_eq!(ppms(&d.answers()[2].1), normalized, "sum {sum}");
                assert!(dist(&d.answers()[2].1).renormalized(), "sum {sum}");
                assert!(!dist(&d.answers()[1].1).renormalized());
                assert!(d.renormalized(), "the decision records it");
            }
            None => assert_eq!(mixed_err(exact, score), BAD_SUM, "sum {sum}"),
        }
    }
    let score = [0, 0, 1_000_000];
    let d = mixed_ok([475_000, 475_000], score);
    assert_eq!(ppms(&d.answers()[1].1), [500_000, 500_000]);
    assert!(mixed_ok([525_000, 525_000], score).renormalized());
    assert_eq!(mixed_err([475_000, 474_999], score), BAD_SUM);
    assert_eq!(mixed_err([525_001, 525_000], score), BAD_SUM);
}

#[test]
fn td_o7_a_zero_sum_is_bad_sum_with_no_uniform_fallback() {
    assert_eq!(mixed_err([0, 0], [0, 0, 1_000_000]), BAD_SUM);
    assert_eq!(mixed_err([0, 1_000_000], [0, 0, 0]), BAD_SUM);
    assert_eq!(
        mixed_err([1_000_000, 1_000_000], [0, 0, 1_000_000]),
        BAD_SUM
    );
    let three = |v: [u32; 3]| {
        let reply = obj(&[(
            "pick",
            obj(&[("a", dec(v[0])), ("b", dec(v[1])), ("c", dec(v[2]))]),
        )]);
        decide(choice3(700_000), &reply).map(|d| ppms(&d.answers()[0].1))
    };
    assert_eq!(
        three([0, 0, 0]),
        Err(ReplyError::Decode(DecodeCode::BadSum))
    );
    assert_eq!(
        three([0, 0, 1_000_000]),
        Ok(vec![0, 0, 1_000_000]),
        "control"
    );
}

#[test]
fn td_o7_largest_remainder_ties_go_to_the_earlier_label() {
    let goldens: [([u32; 3], [u32; 3]); 4] = [
        // Three equal remainders, one ppm short: the first label gets it.
        ([333_333, 333_333, 333_333], [333_334, 333_333, 333_333]),
        // Equal remainders on the second and third: the second gets it.
        ([500_000, 249_999, 249_999], [500_001, 250_000, 249_999]),
        // The largest remainders win whatever their order.
        ([100_000, 200_001, 700_000], [100_000, 200_001, 699_999]),
        ([300_000, 300_000, 350_000], [315_790, 315_789, 368_421]),
    ];
    for (score, normalized) in goldens {
        assert_eq!(
            ppms(&mixed_ok([0, 1_000_000], score).answers()[2].1),
            normalized
        );
    }
    let d = mixed_ok([333_333, 666_666], [0, 0, 1_000_000]);
    assert_eq!(ppms(&d.answers()[1].1), [333_333, 666_667]);
    let d = mixed_ok([500_001, 500_000], [0, 0, 1_000_000]);
    assert_eq!(ppms(&d.answers()[1].1), [500_000, 500_000]);
    // Seeded vectors: the result sums to exactly one million, each value is
    // its floor or one more, and the extra ppm go to the largest remainders,
    // ties to the earlier label. These properties determine the result.
    let mut rng = Rng(0x7d07);
    let mut renormalized = 0;
    for _ in 0..2_000 {
        let target = 950_000 + rng.below(100_001) as u64;
        let k = 2 + rng.below(2);
        let mut v = vec![0u64; k];
        let mut left = target;
        for slot in v.iter_mut().take(k - 1) {
            let x = (rng.below(left.min(1_000_000) as usize + 1) as u64).min(left);
            *slot = x;
            left -= x;
        }
        v[k - 1] = left.min(1_000_000);
        let sum: u64 = v.iter().sum();
        let reply = if k == 2 {
            mixed_reply(0, [v[0] as u32, v[1] as u32], [0, 0, 1_000_000])
        } else {
            obj(&[(
                "lvl",
                obj(&[
                    ("lo", dec(v[0] as u32)),
                    ("mid", dec(v[1] as u32)),
                    ("hi", dec(v[2] as u32)),
                ]),
            )])
        };
        let result = if k == 2 {
            decide(mixed(), &reply).map(|d| ppms(&d.answers()[1].1))
        } else {
            decide(score3(1), &reply).map(|d| ppms(&d.answers()[0].1))
        };
        if sum.abs_diff(1_000_000) > 50_000 || sum == 0 {
            assert_eq!(result, Err(ReplyError::Decode(DecodeCode::BadSum)), "{v:?}");
            continue;
        }
        let n = result.unwrap_or_else(|e| panic!("{v:?}: {e:?}"));
        assert_eq!(
            n.iter().map(|x| u64::from(*x)).sum::<u64>(),
            1_000_000,
            "{v:?}"
        );
        if sum == 1_000_000 {
            assert_eq!(n.iter().map(|x| u64::from(*x)).collect::<Vec<_>>(), v);
            continue;
        }
        renormalized += 1;
        let floor: Vec<u64> = v.iter().map(|x| x * 1_000_000 / sum).collect();
        let rem: Vec<u64> = v.iter().map(|x| x * 1_000_000 % sum).collect();
        let plus: Vec<bool> = (0..k)
            .map(|i| {
                let got = u64::from(n[i]);
                assert!(got == floor[i] || got == floor[i] + 1, "{v:?} -> {n:?}");
                got == floor[i] + 1
            })
            .collect();
        for i in 0..k {
            for j in 0..k {
                if plus[i] && !plus[j] {
                    assert!(
                        rem[i] > rem[j] || (rem[i] == rem[j] && i < j),
                        "{v:?} -> {n:?}"
                    );
                }
            }
        }
    }
    assert!(
        renormalized > 1_000,
        "the property was exercised: {renormalized}"
    );
}

#[test]
fn td_o7_top_is_computed_in_code_and_ties_are_tie() {
    let fits = || crate::Label::new("fits");
    let top = |choice: [u32; 2], score: [u32; 3]| {
        let d = mixed_ok(choice, score);
        (dist(&d.answers()[1].1).top(), dist(&d.answers()[2].1).top())
    };
    let label = |s: &'static str| Top::Label(crate::Label::new(s));
    assert_eq!(
        top([500_000, 500_000], [400_000, 400_000, 200_000]),
        (Top::Tie, Top::Tie)
    );
    assert_eq!(
        top([500_001, 499_999], [200_000, 400_000, 400_000]),
        (Top::Label(fits()), Top::Tie)
    );
    assert_eq!(
        top([499_999, 500_001], [400_000, 200_000, 400_000]),
        (label("unclear"), Top::Tie)
    );
    assert_eq!(
        top([1_000_000, 0], [100_000, 450_000, 450_000]),
        (label("fits"), Top::Tie)
    );
    assert_eq!(
        top([0, 1_000_000], [0, 0, 1_000_000]),
        (label("unclear"), label("high"))
    );
    assert_eq!(top([0, 1_000_000], [0, 600_000, 400_000]).1, label("mid"));
    // On the normalized vector: an unequal reply can normalize to a tie, and
    // the earlier label's remainder can make it the unique top (a Choice
    // never selects such a top: TD-O8).
    assert_eq!(top([500_001, 500_000], [0, 0, 1_000_000]).0, Top::Tie);
    assert_eq!(
        top([0, 1_000_000], [333_333, 333_333, 333_333]).1,
        label("low")
    );
    assert_eq!(Top::Tie.code(), "tie");
    assert_eq!(label("mid").code(), "mid");
}

#[test]
fn td_o7_a_score_expectation_is_computed_in_code() {
    let expected = |score: [u32; 3]| mixed_ok([0, 1_000_000], score).answers()[2].1.expected();
    assert_eq!(expected([200_000, 300_000, 500_000]), Some(1_300_000));
    assert_eq!(expected([1_000_000, 0, 0]), Some(0));
    assert_eq!(expected([0, 1_000_000, 0]), Some(1_000_000));
    assert_eq!(expected([0, 0, 1_000_000]), Some(2_000_000));
    // From the normalized vector: (333,334, 333,333, 333,333).
    assert_eq!(expected([333_333, 333_333, 333_333]), Some(999_999));
    let d = mixed_ok([0, 1_000_000], [0, 0, 1_000_000]);
    assert_eq!(d.answers()[0].1.expected(), None, "a Noul has none");
    assert_eq!(d.answers()[1].1.expected(), None, "a Choice has none");
    // Never read: the same cumulative probability with different
    // expectations gives the same reading.
    let a = mixed_ok([0, 1_000_000], [200_000, 0, 800_000]);
    let b = mixed_ok([0, 1_000_000], [0, 200_000, 800_000]);
    assert_ne!(a.answers()[2].1.expected(), b.answers()[2].1.expected());
    assert_eq!(a.readings()[2], b.readings()[2]);
    assert_eq!(a.readings()[2].1, crate::Reading::Yes);
}
