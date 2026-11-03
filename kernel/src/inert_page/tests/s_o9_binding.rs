//! S-O9: the verdict is bound to the candidate's SHA-256, the policy hash,
//! the facts marker and the outcome. Changing any one of them changes
//! `binding_hash`, which follows the documented encoding (restated here
//! independently), and `is_for` is false for a one-byte change, another
//! policy or other facts.
//!
//! S-O13, the verdict part (Architect note after Codex round 2). The run
//! part, `recheck_inert_page` and its codes, is S1a-C's: changed or
//! unreadable staged bytes give `CandidateUnavailable` there (the F8 read
//! refuses a manifest mismatch), and an altered stored verdict gives
//! `Stale`; both leave the run `Spent`. Here: a fresh evaluation of
//! unchanged bytes and facts equals the stored verdict; changed bytes can
//! never satisfy that equality or `is_for`; and a stored verdict altered in
//! any one bound field is unequal to the fresh evaluation.

use std::collections::BTreeSet;

use sha2::{Digest, Sha256};

use super::facts_fixtures::*;
use super::*;

/// The documented binding encoding, restated: SHA-256 over
/// `put("nexus.inert_page.verdict.v1")`, the candidate SHA-256, the policy
/// hash, the facts tag (0, or 1 then the facts hash), then the outcome as a
/// u16 (0 passed; 0x0100 + refusal position; 0x0200 + fact position) and a
/// u32 detail (an hours line's index, else 0), big-endian.
fn independent_binding(v: &PageVerdict) -> [u8; 32] {
    let mut h = Sha256::new();
    let domain = b"nexus.inert_page.verdict.v1";
    h.update((domain.len() as u64).to_be_bytes());
    h.update(domain);
    h.update(v.candidate_sha256);
    h.update(v.policy_hash);
    match v.facts {
        FactsMarker::NoFacts => h.update([0u8]),
        FactsMarker::Facts(f) => {
            h.update([1u8]);
            h.update(f);
        }
    }
    let (code, detail): (u16, u32) = match v.outcome {
        PageOutcome::Passed => (0, 0),
        PageOutcome::Refused(c) => (0x0100 + c.position(), 0),
        PageOutcome::FactMissing(f) => (
            0x0200 + f.position(),
            match f {
                FactCode::HoursLineMissing { index } => u32::from(index),
                _ => 0,
            },
        ),
    };
    h.update(code.to_be_bytes());
    h.update(detail.to_be_bytes());
    h.finalize().into()
}

fn every_fact_code() -> Vec<FactCode> {
    let mut codes = vec![
        FactCode::FactsInvalid,
        FactCode::PhoneFormat,
        FactCode::FactPhoneLike,
        FactCode::NameNotHeading,
        FactCode::NameNotTitle,
        FactCode::PhoneMissing,
        FactCode::ForeignNumber,
    ];
    for index in [0, 1, 6, 255] {
        codes.push(FactCode::HoursLineMissing { index });
    }
    codes
}

fn flip(mut b: [u8; 32], at: usize) -> [u8; 32] {
    b[at] ^= 1;
    b
}

/// One-byte changes of `base`: a byte changed, inserted or removed.
fn one_byte_changes(base: &[u8]) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    for at in (0..base.len()).step_by(97).chain([base.len() - 1]) {
        let mut changed = base.to_vec();
        changed[at] ^= 0x20;
        out.push(changed);
        let mut inserted = base.to_vec();
        inserted.insert(at, b'x');
        out.push(inserted);
        let mut removed = base.to_vec();
        removed.remove(at);
        out.push(removed);
    }
    out
}

#[test]
fn s_o9_the_binding_names_the_bytes_the_policy_the_facts_and_the_outcome() {
    let facts = reference_facts();
    let page = REFERENCE.as_bytes();
    let v = PageVerdict::evaluate(page, Some(&facts));
    let sha: [u8; 32] = Sha256::digest(page).into();
    assert_eq!(v.candidate_sha256, sha);
    assert_eq!(v.policy_hash, policy_hash());
    assert_eq!(v.facts, FactsMarker::Facts(facts.hash()));
    assert_eq!(v.outcome, PageOutcome::Passed);
    assert_eq!(FactsMarker::of(None), FactsMarker::NoFacts);
    assert_eq!(
        FactsMarker::of(Some(&facts)),
        FactsMarker::Facts(facts.hash())
    );
    assert_eq!(
        PageVerdict::evaluate(page, None).facts,
        FactsMarker::NoFacts
    );
    assert_eq!(v.binding_hash(), independent_binding(&v));

    // Changing any one bound field changes the binding hash.
    let mut variants = vec![v.clone()];
    for at in [0, 17, 31] {
        let mut x = v.clone();
        x.candidate_sha256 = flip(x.candidate_sha256, at);
        variants.push(x);
        let mut x = v.clone();
        x.policy_hash = flip(x.policy_hash, at);
        variants.push(x);
        let mut x = v.clone();
        x.facts = FactsMarker::Facts(flip(facts.hash(), at));
        variants.push(x);
    }
    let mut x = v.clone();
    x.facts = FactsMarker::NoFacts;
    variants.push(x);
    let other = facts_with_phone("01632 960124");
    let mut x = v.clone();
    x.facts = FactsMarker::of(Some(&other));
    variants.push(x);
    for code in RefusalCode::CODES {
        let mut x = v.clone();
        x.outcome = PageOutcome::Refused(code);
        variants.push(x);
    }
    for code in every_fact_code() {
        let mut x = v.clone();
        x.outcome = PageOutcome::FactMissing(code);
        variants.push(x);
    }
    let hashes: BTreeSet<[u8; 32]> = variants.iter().map(PageVerdict::binding_hash).collect();
    assert_eq!(hashes.len(), variants.len(), "every variant binds apart");
    for x in &variants {
        assert_eq!(x.binding_hash(), independent_binding(x), "{x:?}");
    }

    // `is_for` names exactly these bytes, this policy and these facts.
    let marker = FactsMarker::Facts(facts.hash());
    assert!(v.is_for(page, &policy_hash(), &marker));
    for changed in one_byte_changes(page) {
        assert!(!v.is_for(&changed, &policy_hash(), &marker));
    }
    for at in [0, 31] {
        assert!(!v.is_for(page, &flip(policy_hash(), at), &marker));
    }
    assert!(!v.is_for(page, &policy_hash(), &FactsMarker::NoFacts));
    assert!(!v.is_for(page, &policy_hash(), &FactsMarker::of(Some(&other))));
    let bare = PageVerdict::evaluate(page, None);
    assert!(bare.is_for(page, &policy_hash(), &FactsMarker::NoFacts));
    assert!(!bare.is_for(page, &policy_hash(), &marker));
}

fn facts_with_phone(phone: &str) -> ApprovedFacts {
    facts(NAME, &HOURS, phone)
}

#[test]
fn s_o13_verdict_part_unchanged_inputs_agree_and_any_change_disagrees() {
    let facts = reference_facts();
    let page = REFERENCE.as_bytes();
    let stored = PageVerdict::evaluate(page, Some(&facts));
    assert_eq!(stored.outcome, PageOutcome::Passed);

    // Unchanged bytes and facts: the fresh evaluation equals the stored one.
    assert_eq!(PageVerdict::evaluate(page, Some(&facts)), stored);
    assert_eq!(
        PageVerdict::evaluate(page, None),
        PageVerdict::evaluate(page, None)
    );
    let refused = edit("<p>Phone: ", "<p>Phone: <script>x</script>");
    assert_eq!(
        PageVerdict::evaluate(refused.as_bytes(), Some(&facts)),
        PageVerdict::evaluate(refused.as_bytes(), Some(&facts))
    );

    // Changed bytes never satisfy the recheck's equality, nor `is_for`.
    let marker = FactsMarker::Facts(facts.hash());
    for changed in one_byte_changes(page) {
        assert_ne!(PageVerdict::evaluate(&changed, Some(&facts)), stored);
        assert!(!stored.is_for(&changed, &policy_hash(), &marker));
    }

    // A stored verdict altered in any one bound field is unequal to the
    // fresh evaluation (S1a-C reports it as `Stale`).
    let fresh = PageVerdict::evaluate(page, Some(&facts));
    let mut altered = Vec::new();
    let mut x = stored.clone();
    x.policy_hash = flip(x.policy_hash, 0);
    altered.push(x);
    let mut x = stored.clone();
    x.candidate_sha256 = flip(x.candidate_sha256, 0);
    altered.push(x);
    let mut x = stored.clone();
    x.facts = FactsMarker::NoFacts;
    altered.push(x);
    let mut x = stored.clone();
    x.outcome = PageOutcome::FactMissing(FactCode::PhoneMissing);
    altered.push(x);
    for x in &altered {
        assert_ne!(&fresh, x, "{x:?}");
        assert_ne!(fresh.binding_hash(), x.binding_hash(), "{x:?}");
    }
}
