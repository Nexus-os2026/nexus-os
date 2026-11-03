//! S1a-C oracles (design v2 §8, with the Architect's S-O13 split; joint
//! note O1): the inert page check bound to the coding run. Pre-registered
//! in the first commit of the S1a-C PR, red on its skeleton, and never
//! edited after a result is seen.
//!
//! | ID    | Oracle                                                                   |
//! |-------|--------------------------------------------------------------------------|
//! | S-O11 | check, then review, gives `InertPage(result.binding_hash())`; the ledger |
//! |       | holds digests only. Refused: staged bytes tampered after approval; a     |
//! |       | check after an approval (`ApprovalMismatch`); a sandbox result with a    |
//! |       | check, in either order; two changed files; Stop engaged; a ledger        |
//! |       | failure (`Spent`, nothing usable); a result for another candidate        |
//! | S-O12 | the golden hashes of `NoResult` and `Result(h)` are unchanged; the tag-2 |
//! |       | `InertPage(h)` is pinned and differs from `Result(h)`                    |
//! | S-O13 | changed or unreadable staged bytes: `CandidateUnavailable`; an altered   |
//! |       | stored verdict: `Stale`. Each leaves `Spent` and apply refused           |
//! | S-O14 | privacy canaries in the page, title, facts and path are absent from the  |
//! |       | ledger's main, `-wal` and `-shm` files, its verified records, and every  |
//! |       | error's, result's and facts' `Debug` or `Display`                        |
//! | S-O19 | unchanged facts succeed: check, review, approve, recheck, apply          |
//! |       | publishes; with facts and with none                                      |
//! | S-O20 | changed, removed or added facts: `FactsChanged`, then `Spent`; apply is  |
//! |       | refused, and so are a later recheck with the right facts and a later     |
//! |       | check                                                                    |
//! | S-O21 | a second check (passing or failing) revokes an approved pass; a refused  |
//! |       | or unrecorded first check cannot be retried; an unrecorded recheck       |
//! |       | success is `Spent`                                                       |
//!
//! `Spent` means: no `latest_inert_page`; `review` is `CandidateChanged`; an
//! approval cannot be requested; an earlier approval's apply is refused with
//! nothing written; every later recheck is `Spent` and every later check is
//! `InvalidState`.

use super::p0b_ledger_privacy::{leaks, path_digest, Canary};
use super::p1_apply::{env, verified, verified_with, Confirm, Env};
use super::*;
use crate::emergency_stop::StopAuthority;
use crate::inert_page::{
    policy_hash, ApprovedFacts, FactCode, FactsMarker, PageOutcome, PageVerdict, RefusalCode,
    LABEL_PASSED,
};
use sha2::{Digest as _, Sha256};
use std::os::fd::OwnedFd;

const REFERENCE: &str = include_str!("../inert_page/fixtures/reference.html");
const PAGE: &str = "src/index.html";
const NAME: &str = "Rosie's Bakery";
const HOURS: [&str; 3] = [
    "Monday to Friday: 07:00 – 17:00",
    "Saturday: 08:00 – 14:00",
    "Sunday: closed",
];
const PHONE: &str = "01632 960123";

fn facts_of(name: &str, hours: &[&str], phone: &str) -> ApprovedFacts {
    ApprovedFacts::new(name, hours, phone)
        .unwrap_or_else(|code| panic!("valid facts are refused: {code}"))
}

/// The reference page's facts.
fn reference_facts() -> ApprovedFacts {
    facts_of(NAME, &HOURS, PHONE)
}

/// Facts the reference page lacks (a fourth hours line, index 3).
fn missing_facts() -> ApprovedFacts {
    facts_of(
        NAME,
        &[HOURS[0], HOURS[1], HOURS[2], "Holidays: closed"],
        PHONE,
    )
}

/// A verified run whose one change creates `PAGE` with the reference page.
fn page_run(e: &Env) -> (CodingRun, WorkspaceBinding) {
    verified(e, vec![create(PAGE, REFERENCE)])
}

fn approve(e: &Env, run: &mut CodingRun) -> OwnerApproval {
    run.request_approval(&e.info.name, &Confirm::yes()).unwrap()
}

fn apply_with(
    e: &Env,
    run: &mut CodingRun,
    binding: WorkspaceBinding,
    approval: OwnerApproval,
) -> Result<ApplyReport, ApplyError> {
    let grant = e.projects.grant_for_apply(e.info.id, binding).unwrap();
    run.apply(approval, grant, &parent(&e.f))
}

fn published(e: &Env) -> Option<String> {
    std::fs::read_to_string(e.f.project.join(PAGE)).ok()
}

/// The payloads of `run`'s verified records of `kind`, in order.
fn records(e: &Env, run: &CodingRun, kind: &str) -> Vec<Value> {
    e.f.ledger
        .verify_run(run.id().ledger_key())
        .unwrap()
        .into_iter()
        .filter(|record| record.event_kind == kind)
        .map(|record| serde_json::from_str(&record.payload).unwrap())
        .collect()
}

/// The run is spent: it can never be reviewed, approved or applied again,
/// and its check can never be redone.
fn assert_spent(
    e: &Env,
    run: &mut CodingRun,
    binding: WorkspaceBinding,
    approval: Option<OwnerApproval>,
    what: &str,
) {
    assert_eq!(run.latest_inert_page(), None, "{what}: a result survived");
    assert_eq!(
        run.review().unwrap_err(),
        RunError::CandidateChanged,
        "{what}: review"
    );
    let before = digest(&e.f.project);
    if let Some(approval) = approval {
        assert_eq!(
            apply_with(e, run, binding, approval).unwrap_err(),
            ApplyError::Refused(Refusal::CandidateChanged),
            "{what}: the earlier approval's apply"
        );
    }
    assert_eq!(
        run.request_approval(&e.info.name, &Confirm::yes())
            .unwrap_err(),
        ApplyError::Refused(Refusal::CandidateChanged),
        "{what}: a new approval"
    );
    assert_eq!(digest(&e.f.project), before, "{what}: the project changed");
    assert_eq!(published(e), None, "{what}: the page was published");
    for expected in [Some(reference_facts()), None] {
        assert_eq!(
            run.recheck_inert_page(expected.as_ref()),
            Err(InertPageError::Spent),
            "{what}: a later recheck"
        );
        assert_eq!(
            run.check_inert_page(PAGE, expected).unwrap_err(),
            InertPageError::InvalidState,
            "{what}: a later check"
        );
    }
    assert_eq!(run.latest_inert_page(), None, "{what}: a result came back");
    assert!(
        matches!(
            run.review(),
            Err(RunError::CandidateChanged | RunError::CandidateUnavailable)
        ),
        "{what}: a later review"
    );
}

/// Delegates to the real ledger, but refuses the first `failures` appends
/// of one event kind.
struct FailKind {
    inner: Arc<CodingRunLedger>,
    kind: &'static str,
    left: AtomicUsize,
    refused: AtomicUsize,
}

impl FailKind {
    fn new(inner: &Arc<CodingRunLedger>, kind: &'static str, failures: usize) -> Arc<Self> {
        Arc::new(Self {
            inner: Arc::clone(inner),
            kind,
            left: AtomicUsize::new(failures),
            refused: AtomicUsize::new(0),
        })
    }

    fn refused(&self) -> usize {
        self.refused.load(Ordering::SeqCst)
    }
}

impl LedgerStore for FailKind {
    fn append(&self, event: NewLedgerEvent<'_>) -> Result<LedgerRecord, LedgerFailure> {
        if event.event_kind == self.kind
            && self
                .left
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1))
                .is_ok()
        {
            self.refused.fetch_add(1, Ordering::SeqCst);
            return Err(LedgerFailure::Unavailable);
        }
        LedgerStore::append(self.inner.as_ref(), event)
    }

    fn verified_records(&self, run: Uuid) -> Result<Vec<LedgerRecord>, LedgerFailure> {
        self.inner.verified_records(run)
    }
}

// ── A sandbox verification, for the mutual exclusion ───────────────────────

struct Launch;

impl VerifierLaunchConfirmer for Launch {
    fn confirm_launch(&self, _request: &VerifierLaunchRequest) -> bool {
        true
    }
}

fn inputs() -> VerifierInputs {
    VerifierInputs {
        profile_hash: [1; 32],
        toolchain_digest: [2; 32],
        toolchain_generation: 1,
        sandbox_policy_hash: [4; 32],
        resource_policy_hash: [5; 32],
    }
}

fn input_dir(e: &Env) -> OwnedFd {
    let path =
        e.f.staging_parent
            .parent()
            .unwrap()
            .join(format!("s1a-c-input-{}", Uuid::new_v4()));
    std::fs::create_dir(&path).unwrap();
    OwnedFd::from(std::fs::File::open(&path).unwrap())
}

/// A finalized, passing sandbox verification of `run`'s candidate.
fn sandbox_result(e: &Env, run: &mut CodingRun) -> VerificationResult {
    run.prepare_verification(inputs()).unwrap();
    run.materialize_verification_input(input_dir(e)).unwrap();
    let facts = VerifierLaunchFacts {
        profile_display: "Rust library tests (offline)".to_string(),
        wall_timeout_secs: 600,
        memory_max_bytes: 4 << 30,
        cpus: 4,
        processes: 256,
    };
    let approval = run.request_verification_approval(&facts, &Launch).unwrap();
    let generation = run.begin_verification(approval, inputs()).unwrap();
    run.verification_running(generation).unwrap();
    run.verification_finalizing(generation).unwrap();
    let stream = StreamSummary {
        bytes: 1,
        sha256: [9; 32],
        truncated: false,
    };
    run.finish_verification(
        generation,
        VerifierOutcome {
            exit: VerifierExit::Passed,
            duration_ms: 1,
            stdout: stream,
            stderr: stream,
            cleanup: VerifierCleanup::Confirmed,
        },
    )
    .unwrap()
}

/// The result binding, encoded independently: SHA-256 over
/// `put("nexus.coding_run.inert_page.v1")`, the run's UUID bytes, the
/// candidate manifest hash, the structural binding hash, the path digest and
/// the page verdict's binding hash.
fn independent_result_hash(result: &InertPageResult) -> [u8; 32] {
    let domain = b"nexus.coding_run.inert_page.v1";
    let mut h = Sha256::new();
    h.update((domain.len() as u64).to_be_bytes());
    h.update(domain);
    h.update(result.run_id.ledger_key().as_bytes());
    h.update(hex::decode(result.candidate_manifest_hash.to_hex()).unwrap());
    h.update(result.structural_binding_hash);
    h.update(result.path_sha256);
    h.update(result.page.binding_hash());
    h.finalize().into()
}

// ── S-O11: the run binding ──────────────────────────────────────────────────

#[test]
fn s_o11_check_then_review_binds_the_inert_page_marker_with_digests_only() {
    let e = env();
    let (mut run, _) = page_run(&e);
    let facts = reference_facts();
    let verification = run.verification().unwrap().clone();
    let result = run
        .check_inert_page(PAGE, Some(facts.clone()))
        .expect("the reference page and its facts pass");
    assert_eq!(result.run_id, run.id());
    assert_eq!(
        result.candidate_manifest_hash,
        verification.candidate_manifest_hash
    );
    assert_eq!(result.structural_binding_hash, verification.binding_hash());
    assert_eq!(hex::encode(result.path_sha256), path_digest(PAGE));
    assert_eq!(
        result.page,
        PageVerdict::evaluate(REFERENCE.as_bytes(), Some(&facts))
    );
    assert!(result.page.passed());
    assert_eq!(result.page.label(), LABEL_PASSED);
    assert_eq!(result.binding_hash(), independent_result_hash(&result));
    assert_eq!(run.latest_inert_page(), Some(&result));

    let review = run.review().unwrap();
    assert_eq!(
        review.binding.verification,
        VerifierMarker::InertPage(result.binding_hash())
    );
    assert_eq!(review.verification, None);

    // The ledger: digests, counts and codes only.
    assert_eq!(
        records(&e, &run, "page.inert_check"),
        vec![json!({
            "result": hex::encode(result.binding_hash()),
            "candidate_manifest": verification.candidate_manifest_hash.to_hex(),
            "page_sha256": hex::encode(Sha256::digest(REFERENCE.as_bytes())),
            "path_sha256": path_digest(PAGE),
            "page_bytes": REFERENCE.len(),
            "path_bytes": PAGE.len(),
            "policy": hex::encode(policy_hash()),
            "facts": hex::encode(facts.hash()),
            "outcome": "passed",
        })]
    );
    let reviews = records(&e, &run, "review.computed");
    let last = reviews.last().unwrap();
    assert_eq!(last["binding"], hex::encode(review.binding.hash()));
    assert_eq!(last["inert_page"], hex::encode(result.binding_hash()));
    assert!(
        last.get("verification").is_some_and(Value::is_null),
        "{last}"
    );
    for record in e.f.ledger.verify_run(run.id().ledger_key()).unwrap() {
        for text in ["Rosie", "Bakery", "Mill Lane", "Exampleton", "index"] {
            assert!(
                !record.payload.contains(text),
                "{} carries {text:?}",
                record.event_kind
            );
        }
    }
}

#[test]
fn s_o11_staged_bytes_tampered_after_approval_are_never_published() {
    let e = env();
    let (mut run, binding) = page_run(&e);
    run.check_inert_page(PAGE, None).unwrap();
    let approval = approve(&e, &mut run);
    std::fs::write(
        run.staging_path_for_test().unwrap().join(PAGE),
        REFERENCE.replace("Fresh bread", "Stale bread"),
    )
    .unwrap();
    let before = digest(&e.f.project);
    assert_eq!(
        apply_with(&e, &mut run, binding, approval).unwrap_err(),
        ApplyError::Refused(Refusal::CandidateChanged)
    );
    assert_eq!(digest(&e.f.project), before);
    assert_eq!(published(&e), None);
}

#[test]
fn s_o11_a_check_after_an_approval_is_an_approval_mismatch() {
    let e = env();
    let (mut run, binding) = page_run(&e);
    let approval = approve(&e, &mut run);
    assert_eq!(approval.binding().verification, VerifierMarker::NoResult);
    run.check_inert_page(PAGE, None).unwrap();
    let before = digest(&e.f.project);
    assert_eq!(
        apply_with(&e, &mut run, binding, approval).unwrap_err(),
        ApplyError::Refused(Refusal::ApprovalMismatch)
    );
    assert_eq!(digest(&e.f.project), before);
    assert_eq!(published(&e), None);
}

#[test]
fn s_o11_a_sandbox_result_and_an_inert_check_exclude_each_other() {
    let e = env();
    // A sandbox result first: the check is refused, and spends the slot.
    let (mut run, binding) = page_run(&e);
    let sandbox = sandbox_result(&e, &mut run);
    assert_eq!(
        run.review().unwrap().binding.verification,
        VerifierMarker::Result(sandbox.binding_hash())
    );
    assert_eq!(
        run.check_inert_page(PAGE, None).unwrap_err(),
        InertPageError::InvalidState
    );
    assert_spent(
        &e,
        &mut run,
        binding,
        None,
        "a check after a sandbox result",
    );

    // A check first: no sandbox verification can be prepared.
    let (mut run, _) = page_run(&e);
    let checked = run.check_inert_page(PAGE, None).unwrap();
    assert_eq!(
        run.prepare_verification(inputs()).unwrap_err(),
        VerificationError::InvalidState
    );
    assert_eq!(run.verification_phase(), VerificationPhase::Idle);
    assert_eq!(run.latest_verification(), None);
    assert_eq!(
        run.review().unwrap().binding.verification,
        VerifierMarker::InertPage(checked.binding_hash())
    );

    // A check while a launch is prepared: refused, and the slot is spent.
    let (mut run, binding) = page_run(&e);
    run.prepare_verification(inputs()).unwrap();
    assert_eq!(
        run.check_inert_page(PAGE, None).unwrap_err(),
        InertPageError::InvalidState
    );
    run.abandon_verification().unwrap();
    assert_spent(&e, &mut run, binding, None, "a check during a launch");
}

#[test]
fn s_o11_only_a_candidate_of_exactly_that_one_file_is_checked() {
    let e = env();
    for (what, edits) in [
        (
            "two created files",
            vec![create(PAGE, REFERENCE), create("src/notes.txt", "notes\n")],
        ),
        (
            "a page beside a replaced file",
            vec![
                create(PAGE, REFERENCE),
                replace("src/lib.rs", "pub fn answer() -> u32 { 42 }\n"),
            ],
        ),
    ] {
        let (mut run, binding) = verified(&e, edits);
        assert_eq!(
            run.check_inert_page(PAGE, None).unwrap_err(),
            InertPageError::NotSingleFile,
            "{what}"
        );
        assert_spent(&e, &mut run, binding, None, what);
    }
    // The one changed file is not the path named.
    let (mut run, binding) = page_run(&e);
    assert_eq!(
        run.check_inert_page("src/lib.rs", None).unwrap_err(),
        InertPageError::NotSingleFile
    );
    assert_spent(&e, &mut run, binding, None, "another path");
}

#[test]
fn s_o11_the_emergency_stop_refuses_the_check() {
    let e = env();
    let stop = StopAuthority::new();
    let (mut run, _) = with_test_stop_authority(stop.clone(), || page_run(&e));
    stop.engage();
    assert_eq!(
        run.check_inert_page(PAGE, Some(reference_facts()))
            .unwrap_err(),
        InertPageError::EmergencyStopped
    );
    assert_eq!(
        run.state(),
        RunState::Revoked(RevocationReason::EmergencyStop)
    );
    assert_eq!(run.latest_inert_page(), None);
    assert!(records(&e, &run, "page.inert_check").is_empty());
    assert!(run.review().is_err());
    assert_eq!(published(&e), None);
}

#[test]
fn s_o11_an_unrecorded_check_leaves_nothing_usable() {
    let e = env();
    let store = FailKind::new(&e.f.ledger, "page.inert_check", 1);
    let (mut run, binding) = verified_with(
        &e,
        Arc::clone(&store) as Arc<dyn LedgerStore>,
        vec![create(PAGE, REFERENCE)],
    );
    assert_eq!(
        run.check_inert_page(PAGE, Some(reference_facts()))
            .unwrap_err(),
        InertPageError::Unrecorded
    );
    assert_eq!(store.refused(), 1, "the check was not recorded first");
    assert!(records(&e, &run, "page.inert_check").is_empty());
    assert_spent(&e, &mut run, binding, None, "an unrecorded check");
}

#[test]
fn s_o11_a_result_for_another_candidate_is_never_bound() {
    let e = env();
    let (mut other, _) = verified(
        &e,
        vec![create(
            PAGE,
            &REFERENCE.replace("Fresh bread", "Warm bread"),
        )],
    );
    let foreign = other.check_inert_page(PAGE, None).unwrap();
    let (mut run, binding) = page_run(&e);
    let own = run.check_inert_page(PAGE, None).unwrap();
    assert_ne!(own.candidate_manifest_hash, foreign.candidate_manifest_hash);
    let approval = approve(&e, &mut run);
    run.alter_inert_result_for_test(|stored| *stored = foreign.clone());
    assert_eq!(run.review().unwrap_err(), RunError::CandidateChanged);
    let before = digest(&e.f.project);
    assert_eq!(
        apply_with(&e, &mut run, binding, approval).unwrap_err(),
        ApplyError::Refused(Refusal::CandidateChanged)
    );
    assert_eq!(digest(&e.f.project), before);
    assert_eq!(run.recheck_inert_page(None), Err(InertPageError::Stale));
    assert_spent(&e, &mut run, binding, None, "a foreign result");
}

// ── S-O12: the marker encodings ────────────────────────────────────────────

#[test]
fn s_o12_the_marker_encodings_are_pinned() {
    let run_id = RunId::parse("7f3a2c8e-0000-4000-8000-000000000001").unwrap();
    let empty = Manifest::default().hash();
    let binding = |verification| ReviewBinding {
        run_id,
        base_manifest_hash: empty,
        candidate_manifest_hash: empty,
        profile_hash: [7; 32],
        verification,
    };
    let h = [0x11; 32];
    // Computed outside Rust from the documented encodings.
    assert_eq!(
        empty.to_hex(),
        "e084e24f13c152ef86f2c7568923949a77072fec6d48c52b6bcb5ceb850fcd22"
    );
    assert_eq!(
        hex::encode(binding(VerifierMarker::NoResult).hash()),
        "0df6fa18fd6b9c55275fa8420d22fb31e29aeb3b85db2e09149fe912e4060865"
    );
    assert_eq!(
        hex::encode(binding(VerifierMarker::Result(h)).hash()),
        "37609e7e9b36293edafa15704602f29ab9a808aef5058a387d89304274ed151a"
    );
    assert_eq!(
        hex::encode(binding(VerifierMarker::InertPage(h)).hash()),
        "733293b2035ff0eea0f5e0b1fc76112917afebad07e01855f98fdeffbccbb7d9"
    );
    // The same, from an independent encoder: tag 0; tag 1 then the hash;
    // tag 2 then the hash.
    let independent = |marker: &[u8]| -> [u8; 32] {
        let domain = b"nexus.coding_run.review_binding.v1";
        let mut s = Sha256::new();
        s.update((domain.len() as u64).to_be_bytes());
        s.update(domain);
        s.update(run_id.ledger_key().as_bytes());
        s.update(hex::decode(empty.to_hex()).unwrap());
        s.update(hex::decode(empty.to_hex()).unwrap());
        s.update([7; 32]);
        s.update(marker);
        s.finalize().into()
    };
    let tagged = |tag: u8| [&[tag][..], &h[..]].concat();
    assert_eq!(binding(VerifierMarker::NoResult).hash(), independent(&[0]));
    assert_eq!(
        binding(VerifierMarker::Result(h)).hash(),
        independent(&tagged(1))
    );
    assert_eq!(
        binding(VerifierMarker::InertPage(h)).hash(),
        independent(&tagged(2))
    );
    assert_ne!(
        binding(VerifierMarker::InertPage(h)).hash(),
        binding(VerifierMarker::Result(h)).hash()
    );
    assert_ne!(
        binding(VerifierMarker::InertPage(h)).hash(),
        binding(VerifierMarker::InertPage([0x12; 32])).hash()
    );
}

// ── S-O13: the recheck re-reads and re-evaluates ───────────────────────────

#[test]
fn s_o13_an_altered_stored_verdict_is_stale_and_spends_the_run() {
    type Alter = fn(&mut PageVerdict);
    let alterations: [(&str, Alter); 4] = [
        ("another policy hash", |v| v.policy_hash[0] ^= 1),
        ("another candidate sha", |v| v.candidate_sha256[31] ^= 1),
        ("another facts marker", |v| v.facts = FactsMarker::NoFacts),
        ("another outcome", |v| {
            v.outcome = PageOutcome::Refused(RefusalCode::NotCanonical)
        }),
    ];
    for (what, alter) in alterations {
        let e = env();
        let (mut run, binding) = page_run(&e);
        let facts = reference_facts();
        run.check_inert_page(PAGE, Some(facts.clone())).unwrap();
        let approval = approve(&e, &mut run);
        run.alter_inert_result_for_test(|stored| alter(&mut stored.page));
        assert_eq!(
            run.recheck_inert_page(Some(&facts)),
            Err(InertPageError::Stale),
            "{what}"
        );
        assert_eq!(
            records(&e, &run, "page.inert_recheck")
                .first()
                .map(|p| p["outcome"].clone()),
            Some(json!("stale")),
            "{what}"
        );
        assert_spent(&e, &mut run, binding, Some(approval), what);
    }
}

#[test]
fn s_o13_changed_or_unreadable_staged_bytes_are_candidate_unavailable() {
    type Tamper = fn(&Path);
    let tamperings: [(&str, Tamper); 4] = [
        ("changed bytes", |staged| {
            std::fs::write(staged, REFERENCE.replace("Fresh bread", "Stale bread")).unwrap()
        }),
        ("truncated bytes", |staged| {
            std::fs::write(staged, &REFERENCE.as_bytes()[..REFERENCE.len() / 2]).unwrap()
        }),
        ("a removed file", |staged| {
            std::fs::remove_file(staged).unwrap()
        }),
        ("a symlink to the same bytes", |staged| {
            // Outside the run's staging tree: the staging parent.
            let outside = staged
                .ancestors()
                .nth(3)
                .unwrap()
                .join(format!("s1a-c-{}.html", Uuid::new_v4()));
            std::fs::rename(staged, &outside).unwrap();
            std::os::unix::fs::symlink(&outside, staged).unwrap();
        }),
    ];
    for (what, tamper) in tamperings {
        let e = env();
        let (mut run, binding) = page_run(&e);
        run.check_inert_page(PAGE, None).unwrap();
        let approval = approve(&e, &mut run);
        tamper(&run.staging_path_for_test().unwrap().join(PAGE));
        assert_eq!(
            run.recheck_inert_page(None),
            Err(InertPageError::CandidateUnavailable),
            "{what}"
        );
        assert_spent(&e, &mut run, binding, Some(approval), what);
    }
}

// ── S-O14: privacy ──────────────────────────────────────────────────────────

/// P0 canaries whose hex parts hold no run of seven digits, so no planted
/// spelling is ever a phone-like token (FC-4).
fn page_canary() -> Canary {
    let phone_like = |s: &str| {
        s.as_bytes()
            .windows(7)
            .any(|w| w.iter().all(u8::is_ascii_digit))
    };
    loop {
        let canary = Canary::new();
        if !phone_like(&canary.c) && !phone_like(&canary.cl) {
            return canary;
        }
    }
}

/// The reference page with `name` as its title and heading, `sunday` as its
/// last hours line, and one more paragraph of `text`.
fn page_with(name: &str, sunday: &str, text: &str) -> String {
    let page = REFERENCE
        .replace("Rosie&#39;s Bakery", name)
        .replace("<li>Sunday: closed</li>", &format!("<li>{sunday}</li>"))
        .replace("</address>\n", &format!("</address>\n<p>{text}</p>\n"));
    assert_eq!(page.matches(name).count(), 2);
    page
}

/// The ledger's main, `-wal` and `-shm` files, and each verified record of
/// `runs` as stored and decoded.
fn ledger_targets(e: &Env, runs: &[RunId]) -> Vec<(String, Vec<u8>)> {
    fn strings(value: &Value, out: &mut Vec<String>) {
        match value {
            Value::String(text) => out.push(text.clone()),
            Value::Array(items) => items.iter().for_each(|item| strings(item, out)),
            Value::Object(map) => {
                for (key, item) in map {
                    out.push(key.clone());
                    strings(item, out);
                }
            }
            _ => {}
        }
    }
    let db = e.f.staging_parent.parent().unwrap().join("ledger.db");
    let mut out = vec![("ledger.db".to_string(), std::fs::read(&db).unwrap())];
    for suffix in ["-wal", "-shm"] {
        match std::fs::read(format!("{}{suffix}", db.display())) {
            Ok(bytes) => out.push((format!("ledger.db{suffix}"), bytes)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => panic!("ledger.db{suffix}: {error}"),
        }
    }
    for run in runs {
        for record in e.f.ledger.verify_run(run.ledger_key()).unwrap() {
            let name = format!("record {} #{}", record.event_kind, record.seq);
            let mut decoded = Vec::new();
            strings(
                &serde_json::from_str::<Value>(&record.payload).unwrap(),
                &mut decoded,
            );
            out.push((format!("{name} decoded"), decoded.join("\n").into_bytes()));
            out.push((name, record.payload.into_bytes()));
        }
    }
    out
}

#[test]
fn s_o14_no_page_title_facts_or_path_canary_leaves_the_page() {
    let canary = page_canary();
    let (c, cl) = (canary.c.clone(), canary.cl.clone());
    let name = format!("Rosie {cl}");
    let sunday = format!("Sunday: {cl}");
    let spellings = format!(
        "{cl} {} {}",
        cl.to_ascii_uppercase(),
        cl.to_ascii_lowercase()
    );
    let page = page_with(&name, &sunday, &spellings);
    let page_path = format!("src/{cl}.html");
    let passing = facts_of(&name, &[HOURS[0], HOURS[1], &sunday], PHONE);
    let lacking = facts_of(
        &name,
        &[HOURS[0], HOURS[1], &sunday, &format!("Holidays: {cl}")],
        PHONE,
    );
    let long_name = format!("Rosie {c}");
    let refused_page = page_with(&long_name, &format!("Sunday: {cl}"), &c);
    let refused_path = format!("src/{c}.html");
    let refused_facts = facts_of(&long_name, &HOURS, PHONE);

    // The scan's own control: the planted pages carry both canaries.
    let control = leaks(
        &canary,
        &[
            ("page".to_string(), page.clone().into_bytes()),
            (
                "refused page".to_string(),
                refused_page.clone().into_bytes(),
            ),
        ],
    );
    for secret in ["C", "Cl"] {
        assert!(control.iter().any(|l| l.secret == secret), "{secret}");
    }

    let e = env();
    let mut runs = Vec::new();
    let mut texts: Vec<(String, String)> = Vec::new();
    let error = |texts: &mut Vec<(String, String)>, what: &str, error: InertPageError| {
        texts.push((format!("{what} error Debug"), format!("{error:?}")));
        texts.push((format!("{what} error Display"), format!("{error}")));
    };

    // An inert refusal, with facts.
    let (mut run, _) = verified(&e, vec![create(&refused_path, &refused_page)]);
    let refused = run.check_inert_page(&refused_path, Some(refused_facts.clone()));
    assert_eq!(
        refused.clone().unwrap_err(),
        InertPageError::Refused(RefusalCode::WordTooLong)
    );
    error(&mut texts, "refused", refused.unwrap_err());
    error(
        &mut texts,
        "repeat",
        run.check_inert_page(&refused_path, None).unwrap_err(),
    );
    runs.push(run.id());

    // A missing fact.
    let (mut run, _) = verified(&e, vec![create(&page_path, &page)]);
    let missing = run
        .check_inert_page(&page_path, Some(lacking.clone()))
        .unwrap_err();
    assert_eq!(
        missing,
        InertPageError::FactMissing(FactCode::HoursLineMissing { index: 3 })
    );
    error(&mut texts, "missing", missing);
    runs.push(run.id());

    // Facts changed at the recheck.
    let (mut run, _) = verified(&e, vec![create(&page_path, &page)]);
    let result = run
        .check_inert_page(&page_path, Some(passing.clone()))
        .unwrap();
    texts.push(("result Debug".to_string(), format!("{result:?}")));
    error(
        &mut texts,
        "facts changed",
        run.recheck_inert_page(Some(&lacking)).unwrap_err(),
    );
    error(
        &mut texts,
        "spent",
        run.recheck_inert_page(Some(&passing)).unwrap_err(),
    );
    runs.push(run.id());

    // An altered stored verdict.
    let (mut run, _) = verified(&e, vec![create(&page_path, &page)]);
    run.check_inert_page(&page_path, Some(passing.clone()))
        .unwrap();
    run.alter_inert_result_for_test(|stored| stored.page.policy_hash[0] ^= 1);
    error(
        &mut texts,
        "stale",
        run.recheck_inert_page(Some(&passing)).unwrap_err(),
    );
    runs.push(run.id());

    // Changed staged bytes.
    let (mut run, _) = verified(&e, vec![create(&page_path, &page)]);
    run.check_inert_page(&page_path, Some(passing.clone()))
        .unwrap();
    std::fs::write(
        run.staging_path_for_test().unwrap().join(&page_path),
        page.replace("Fresh bread", "Stale bread"),
    )
    .unwrap();
    error(
        &mut texts,
        "unavailable",
        run.recheck_inert_page(Some(&passing)).unwrap_err(),
    );
    runs.push(run.id());

    // The whole way: check, review, approval, recheck, apply.
    let (mut run, binding) = verified(&e, vec![create(&page_path, &page)]);
    let result = run
        .check_inert_page(&page_path, Some(passing.clone()))
        .unwrap();
    run.review().unwrap();
    let approval = approve(&e, &mut run);
    run.recheck_inert_page(Some(&passing)).unwrap();
    apply_with(&e, &mut run, binding, approval).unwrap();
    assert_eq!(
        std::fs::read_to_string(e.f.project.join(&page_path)).unwrap(),
        page
    );
    error(
        &mut texts,
        "after apply",
        run.check_inert_page(&page_path, Some(passing.clone()))
            .unwrap_err(),
    );
    texts.push(("result Debug".to_string(), format!("{result:?}")));
    texts.push(("run Debug".to_string(), format!("{run:?}")));
    runs.push(run.id());

    for (what, facts) in [
        ("passing", &passing),
        ("lacking", &lacking),
        ("refused", &refused_facts),
    ] {
        texts.push((format!("{what} facts Debug"), format!("{facts:?}")));
    }

    // The records exist, so the scan below is not vacuous.
    let all: Vec<LedgerRecord> = runs
        .iter()
        .flat_map(|run| e.f.ledger.verify_run(run.ledger_key()).unwrap())
        .collect();
    for (kind, at_least) in [
        ("page.inert_check", 7),
        ("page.inert_recheck", 5),
        ("review.computed", 2),
    ] {
        let n = all.iter().filter(|r| r.event_kind == kind).count();
        assert!(n >= at_least, "{n} {kind} records");
    }

    let mut targets = ledger_targets(&e, &runs);
    targets.extend(
        texts
            .into_iter()
            .map(|(what, text)| (what, text.into_bytes())),
    );
    let hits = leaks(&canary, &targets);
    assert!(
        hits.is_empty(),
        "{} canary hit(s); the first: {:#?}",
        hits.len(),
        &hits[..hits.len().min(12)]
    );
}

// ── S-O19: unchanged facts succeed ─────────────────────────────────────────

#[test]
fn s_o19_unchanged_facts_recheck_and_the_page_is_published() {
    for facts in [Some(reference_facts()), None] {
        let e = env();
        let (mut run, binding) = page_run(&e);
        let result = run.check_inert_page(PAGE, facts.clone()).unwrap();
        assert_eq!(result.page.facts, FactsMarker::of(facts.as_ref()));
        let review = run.review().unwrap();
        assert_eq!(
            review.binding.verification,
            VerifierMarker::InertPage(result.binding_hash())
        );
        let approval = approve(&e, &mut run);
        assert_eq!(approval.binding(), &review.binding);
        // The admission's facts again: equal by value, a different object.
        let again = facts.as_ref().map(|_| reference_facts());
        assert_eq!(run.recheck_inert_page(again.as_ref()), Ok(()));
        assert_eq!(run.latest_inert_page(), Some(&result));
        assert_eq!(
            records(&e, &run, "page.inert_recheck"),
            vec![json!({
                "result": hex::encode(result.binding_hash()),
                "outcome": "passed",
            })]
        );
        let report = apply_with(&e, &mut run, binding, approval).unwrap();
        assert_eq!(report.files, vec![(rel(PAGE), ChangeKind::Create)]);
        assert_eq!(published(&e).as_deref(), Some(REFERENCE));
    }
}

// ── S-O20: changed or removed facts are refused ────────────────────────────

#[test]
fn s_o20_changed_removed_or_added_facts_are_refused_for_good() {
    let x = reference_facts();
    let cases: Vec<(&str, Option<ApprovedFacts>, Option<ApprovedFacts>)> = vec![
        (
            "another name",
            Some(x.clone()),
            Some(facts_of("Rosie's Bakehouse", &HOURS, PHONE)),
        ),
        (
            "another hours line",
            Some(x.clone()),
            Some(facts_of(
                NAME,
                &[HOURS[0], "Saturday: 08:00 – 15:00", HOURS[2]],
                PHONE,
            )),
        ),
        (
            "fewer hours lines",
            Some(x.clone()),
            Some(facts_of(NAME, &HOURS[..2], PHONE)),
        ),
        ("more hours lines", Some(x.clone()), Some(missing_facts())),
        (
            "another phone",
            Some(x.clone()),
            Some(facts_of(NAME, &HOURS, "01632 960124")),
        ),
        ("facts removed", Some(x.clone()), None),
        ("facts added", None, Some(x.clone())),
    ];
    for (what, retained, expected) in cases {
        let e = env();
        let (mut run, binding) = page_run(&e);
        run.check_inert_page(PAGE, retained.clone()).unwrap();
        let approval = approve(&e, &mut run);
        assert_eq!(
            run.recheck_inert_page(expected.as_ref()),
            Err(InertPageError::FactsChanged),
            "{what}"
        );
        assert_eq!(
            run.recheck_inert_page(retained.as_ref()),
            Err(InertPageError::Spent),
            "{what}: the right facts after a failure"
        );
        assert_spent(&e, &mut run, binding, Some(approval), what);
    }
}

// ── S-O21: a check is single-use ────────────────────────────────────────────

#[test]
fn s_o21_a_second_check_revokes_an_approved_pass() {
    let seconds: [(&str, Option<ApprovedFacts>); 4] = [
        ("the same facts", Some(reference_facts())),
        (
            "other passing facts",
            Some(facts_of(NAME, &HOURS[..2], PHONE)),
        ),
        ("no facts", None),
        ("failing facts", Some(missing_facts())),
    ];
    for (what, second) in seconds {
        let e = env();
        let (mut run, binding) = page_run(&e);
        run.check_inert_page(PAGE, Some(reference_facts())).unwrap();
        run.review().unwrap();
        let approval = approve(&e, &mut run);
        assert_eq!(
            run.check_inert_page(PAGE, second).unwrap_err(),
            InertPageError::InvalidState,
            "{what}"
        );
        assert_eq!(
            records(&e, &run, "page.inert_check")
                .iter()
                .map(|p| p["outcome"].clone())
                .collect::<Vec<_>>(),
            vec![json!("passed"), json!("repeat_refused")],
            "{what}"
        );
        assert_spent(&e, &mut run, binding, Some(approval), what);
    }
}

#[test]
fn s_o21_a_refused_first_check_cannot_be_retried() {
    // A missing fact (FC-2), then a passing check.
    let e = env();
    let (mut run, binding) = page_run(&e);
    assert_eq!(
        run.check_inert_page(PAGE, Some(missing_facts()))
            .unwrap_err(),
        InertPageError::FactMissing(FactCode::HoursLineMissing { index: 3 })
    );
    let first = records(&e, &run, "page.inert_check");
    assert_eq!(first[0]["outcome"], "hours_line_missing");
    assert_eq!(first[0]["fact_index"], 3);
    assert_eq!(first[0]["facts"], hex::encode(missing_facts().hash()));
    assert_eq!(
        run.check_inert_page(PAGE, Some(reference_facts()))
            .unwrap_err(),
        InertPageError::InvalidState
    );
    assert_spent(&e, &mut run, binding, None, "a missing fact first");

    // An inert refusal, then a check of the same file.
    let bad = REFERENCE.replace("<!doctype html>", "<!DOCTYPE html>");
    let (mut run, binding) = verified(&e, vec![create(PAGE, &bad)]);
    assert_eq!(
        run.check_inert_page(PAGE, None).unwrap_err(),
        InertPageError::Refused(RefusalCode::PrologueMismatch)
    );
    let first = records(&e, &run, "page.inert_check");
    assert_eq!(first[0]["outcome"], "prologue_mismatch");
    assert_eq!(
        first[0]["offset"],
        crate::inert_page::check(bad.as_bytes()).unwrap_err().offset
    );
    assert!(first[0].get("facts").is_none_or(Value::is_null));
    assert_eq!(
        run.check_inert_page(PAGE, None).unwrap_err(),
        InertPageError::InvalidState
    );
    assert_spent(&e, &mut run, binding, None, "an inert refusal first");
}

#[test]
fn s_o21_an_unrecorded_check_or_recheck_is_spent() {
    // The first check cannot be recorded; the ledger then works again.
    let e = env();
    let store = FailKind::new(&e.f.ledger, "page.inert_check", 1);
    let (mut run, binding) = verified_with(
        &e,
        Arc::clone(&store) as Arc<dyn LedgerStore>,
        vec![create(PAGE, REFERENCE)],
    );
    assert_eq!(
        run.check_inert_page(PAGE, None).unwrap_err(),
        InertPageError::Unrecorded
    );
    assert_eq!(store.refused(), 1);
    assert_eq!(
        run.check_inert_page(PAGE, None).unwrap_err(),
        InertPageError::InvalidState
    );
    assert_spent(&e, &mut run, binding, None, "an unrecorded first check");

    // A recheck that succeeds but cannot be recorded.
    let store = FailKind::new(&e.f.ledger, "page.inert_recheck", 1);
    let (mut run, binding) = verified_with(
        &e,
        Arc::clone(&store) as Arc<dyn LedgerStore>,
        vec![create(PAGE, REFERENCE)],
    );
    run.check_inert_page(PAGE, None).unwrap();
    let approval = approve(&e, &mut run);
    assert_eq!(
        run.recheck_inert_page(None),
        Err(InertPageError::Unrecorded)
    );
    assert_eq!(store.refused(), 1);
    assert_spent(
        &e,
        &mut run,
        binding,
        Some(approval),
        "an unrecorded recheck",
    );
}
