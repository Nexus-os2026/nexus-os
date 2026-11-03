//! TD-O11: the correction text (§8). One fixed phrase per decoder code;
//! the sentence is the fixed prefix, the phrase and the fixed suffix; it
//! never carries a byte of the rejected reply (canary).

use std::collections::BTreeSet;

use super::appendix_a::{CORRECTION_PREFIX, CORRECTION_SUFFIX, PHRASES};
use super::support::{assert_no_canary, note, ref_page, C, CL, TEST_INVENTORY};
use crate::{
    correction_sentence, prepare, render_correction_system, render_system, DecodeCode, REGISTRY,
};

#[test]
fn td_o11_one_fixed_phrase_per_decoder_code() {
    assert_eq!(DecodeCode::ALL.len(), 9);
    let codes: BTreeSet<&str> = DecodeCode::ALL.iter().map(|c| c.code()).collect();
    let phrases: BTreeSet<&str> = DecodeCode::ALL.iter().map(|c| c.phrase()).collect();
    assert_eq!(
        (codes.len(), phrases.len()),
        (9, 9),
        "distinct codes and phrases"
    );
    for (code, (name, phrase, bytes)) in DecodeCode::ALL.iter().zip(PHRASES) {
        assert_eq!(code.code(), name);
        assert_eq!(code.phrase(), phrase, "{name}");
        assert_eq!(code.phrase().len(), bytes, "{name}");
    }
    assert!(CORRECTION_PREFIX.ends_with("because ") && !CORRECTION_PREFIX.ends_with("  "));
    assert!(CORRECTION_SUFFIX.starts_with(". "));
}

#[test]
fn td_o11_the_sentence_is_prefix_phrase_suffix_after_attempt_one() {
    for code in DecodeCode::ALL {
        let sentence = correction_sentence(code);
        assert_eq!(
            sentence,
            format!("{CORRECTION_PREFIX}{}{CORRECTION_SUFFIX}", code.phrase())
        );
        assert_eq!(sentence.len(), 45 + code.phrase().len() + 85);
        for set in REGISTRY {
            let first = render_system(set).unwrap();
            assert_eq!(
                render_correction_system(set, code).unwrap(),
                format!("{first}\n\n{sentence}")
            );
        }
    }
    for (set, p) in [
        (REGISTRY[0], ref_page()),
        (REGISTRY[1], ref_page()),
        (REGISTRY[2], note("a note")),
    ] {
        let prepared = prepare(set, &p, &TEST_INVENTORY).unwrap();
        for code in DecodeCode::ALL {
            let second = prepared.correction_system(code);
            assert_eq!(
                second,
                format!("{}\n\n{}", prepared.system(), correction_sentence(code))
            );
        }
    }
}

#[test]
fn td_o11_the_correction_never_carries_reply_bytes() {
    // A rejected attempt-1 reply, as the model might send it. The correction
    // API takes only the decoder's code, so none of it can reach attempt 2.
    let rejected = format!("```json\n{{\"instructs_ai\": \"{C}\", \"{CL}\": 0.5}}\n```");
    assert!(rejected.contains(C) && rejected.contains(CL));
    let prepared = prepare(REGISTRY[2], &note("a note"), &TEST_INVENTORY).unwrap();
    for code in DecodeCode::ALL {
        let second = prepared.correction_system(code);
        assert_no_canary("attempt 2 system", &second);
        assert!(!second.contains("```") && !second.contains("0.5"));
        assert!(second.starts_with(prepared.system()));
        let tail = &second[prepared.system().len()..];
        assert_eq!(
            tail,
            format!("\n\n{}", correction_sentence(code)),
            "only the fixed sentence is added"
        );
    }
    assert_no_canary("attempt 2 user", prepared.user());
}
