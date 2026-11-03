//! S1a-C oracles added after Codex review 1 of PR #63 (`test(oracle-add)`,
//! red at `aca0796f`, never edited after a result is seen). The original
//! S1a-C oracles in `s1a_c_inert_slot_tests.rs` are unchanged.
//!
//! | ID     | Oracle                                                                |
//! |--------|-----------------------------------------------------------------------|
//! | S-O11b | the recheck honours the owner's emergency stop exactly as the check   |
//! |        | does: after a pass, Stop (in force, or engaged and already resumed)   |
//! |        | makes the recheck `EmergencyStopped`; the slot stays `Spent`, no      |
//! |        | recheck is recorded as passed, `review_binding` is `CandidateChanged` |
//! |        | and the earlier approval's apply writes nothing                       |
//! | S-O11c | a page over S1a's input cap (65,536 bytes) is refused as `too_large`  |
//! |        | after reading at most 65,537 bytes of it; the record names the whole  |
//! |        | file and the validator's verdict on its bytes                         |

use super::p0b_ledger_privacy::path_digest;
use super::p1_apply::{env, verified, Confirm, Env};
use super::*;
use crate::coding_run::verifier::inert_read_bytes_for_test;
use crate::emergency_stop::StopAuthority;
use crate::inert_page::{
    check as inert_check, policy_hash, ApprovedFacts, PageVerdict, RefusalCode, POLICY_V1,
};
use sha2::{Digest as _, Sha256};

const REFERENCE: &str = include_str!("../inert_page/fixtures/reference.html");
const PAGE: &str = "src/index.html";

fn reference_facts() -> ApprovedFacts {
    ApprovedFacts::new(
        "Rosie's Bakery",
        &[
            "Monday to Friday: 07:00 – 17:00",
            "Saturday: 08:00 – 14:00",
            "Sunday: closed",
        ],
        "01632 960123",
    )
    .unwrap()
}

fn records(e: &Env, run: &CodingRun, kind: &str) -> Vec<Value> {
    e.f.ledger
        .verify_run(run.id().ledger_key())
        .unwrap()
        .into_iter()
        .filter(|record| record.event_kind == kind)
        .map(|record| serde_json::from_str(&record.payload).unwrap())
        .collect()
}

fn none_passed(e: &Env, run: &CodingRun) -> bool {
    records(e, run, "page.inert_recheck")
        .iter()
        .all(|p| p["outcome"] != "passed")
}

// ── S-O11b: the recheck honours Stop ───────────────────────────────────────

#[test]
fn s_o11b_the_emergency_stop_refuses_the_recheck_and_spends_the_pass() {
    for resumed in [false, true] {
        let e = env();
        let stop = StopAuthority::new();
        let controller = stop.claim_controller().unwrap();
        let (mut run, binding) =
            with_test_stop_authority(stop.clone(), || verified(&e, vec![create(PAGE, REFERENCE)]));
        let facts = reference_facts();
        run.check_inert_page(PAGE, Some(facts.clone())).unwrap();
        let approval = run.request_approval(&e.info.name, &Confirm::yes()).unwrap();
        let verification = run.verification().unwrap().clone();
        stop.engage();
        if resumed {
            assert!(controller.lift_if(stop.stop_number()));
        }
        assert_eq!(
            run.recheck_inert_page(Some(&facts)),
            Err(InertPageError::EmergencyStopped),
            "resumed: {resumed}"
        );
        assert_eq!(run.latest_inert_page(), None, "resumed: {resumed}");
        assert!(none_passed(&e, &run), "resumed: {resumed}");
        assert_eq!(
            run.review_binding(&verification).unwrap_err(),
            RunError::CandidateChanged,
            "resumed: {resumed}"
        );
        assert_eq!(
            run.state(),
            RunState::Revoked(RevocationReason::EmergencyStop),
            "resumed: {resumed}"
        );
        // It never comes back: not by a later recheck, and not by apply.
        assert!(run.recheck_inert_page(Some(&facts)).is_err());
        assert_eq!(run.latest_inert_page(), None);
        assert!(none_passed(&e, &run), "resumed: {resumed}");
        let before = digest(&e.f.project);
        let grant = e.projects.grant_for_apply(e.info.id, binding);
        if let Ok(grant) = grant {
            assert!(run.apply(approval, grant, &parent(&e.f)).is_err());
        }
        assert_eq!(digest(&e.f.project), before, "resumed: {resumed}");
        assert!(!e.f.project.join(PAGE).exists(), "resumed: {resumed}");
    }
}

// ── S-O11c: a bounded read of an oversized page ────────────────────────────

#[test]
fn s_o11c_an_oversized_page_is_refused_as_too_large_after_a_bounded_read() {
    let cap = POLICY_V1.max_page_bytes;
    for size in [8 * 1024 * 1024, cap + 2, cap + 1] {
        let e = env();
        let page = "a".repeat(size - 1) + "\n";
        let (mut run, _) = verified(&e, vec![create(PAGE, &page)]);
        let verification = run.verification().unwrap().clone();
        let facts = reference_facts();
        let _ = inert_read_bytes_for_test();
        assert_eq!(
            run.check_inert_page(PAGE, Some(facts.clone())).unwrap_err(),
            InertPageError::Refused(RefusalCode::TooLarge),
            "{size}"
        );
        let read = inert_read_bytes_for_test();
        assert!(read <= cap as u64 + 1, "{size} bytes: {read} were read");
        // The record names the whole file and the validator's verdict on it.
        let expected = InertPageResult {
            run_id: run.id(),
            candidate_manifest_hash: verification.candidate_manifest_hash,
            structural_binding_hash: verification.binding_hash(),
            path_sha256: hex::decode(path_digest(PAGE)).unwrap().try_into().unwrap(),
            page: PageVerdict::evaluate(page.as_bytes(), Some(&facts)),
        };
        assert_eq!(
            records(&e, &run, "page.inert_check"),
            vec![json!({
                "result": hex::encode(expected.binding_hash()),
                "candidate_manifest": verification.candidate_manifest_hash.to_hex(),
                "page_sha256": hex::encode(Sha256::digest(page.as_bytes())),
                "path_sha256": path_digest(PAGE),
                "page_bytes": size,
                "path_bytes": PAGE.len(),
                "offset": inert_check(page.as_bytes()).unwrap_err().offset,
                "policy": hex::encode(policy_hash()),
                "facts": hex::encode(facts.hash()),
                "outcome": "too_large",
            })],
            "{size}"
        );
        assert_eq!(run.latest_inert_page(), None);
        assert_eq!(run.review().unwrap_err(), RunError::CandidateChanged);
    }
    // A page within the cap is read in full, once, and passes.
    let e = env();
    let (mut run, _) = verified(&e, vec![create(PAGE, REFERENCE)]);
    let _ = inert_read_bytes_for_test();
    run.check_inert_page(PAGE, None).unwrap();
    assert_eq!(inert_read_bytes_for_test(), REFERENCE.len() as u64);
}
