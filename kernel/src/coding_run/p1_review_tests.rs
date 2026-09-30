//! Phase One P1-05 tests: the backend-computed, bounded owner review.
//! `p1_d_nc_*` are negative controls.

use super::*;
use crate::coding_run::review::unified_for_test;

fn verified(f: &Fixture, edits: Vec<CandidateEdit>) -> CodingRun {
    let mut run = staged_run(f);
    for edit in edits {
        run.edit(edit).unwrap();
    }
    assert!(run.verify_structural().unwrap().passed());
    run
}

fn diff_text(change: &FileChange) -> &str {
    match &change.diff {
        TextDiff::Unified { text, .. } => text,
        other => panic!("no diff: {other:?}"),
    }
}

// ── Positive behaviour ──────────────────────────────────────────────────────

#[test]
fn p1_d_01_review_lists_exact_changes_with_hashes_and_diffs() {
    let f = fixture();
    let before = digest(&f.project);
    let mut run = verified(
        &f,
        vec![
            replace("src/lib.rs", "pub fn answer() -> u32 { 42 }\n"),
            create("src/new.rs", "pub fn added() {}\n"),
        ],
    );
    let verification = run.verification().unwrap().clone();
    let review = run.review().unwrap();
    assert_eq!(review.binding.run_id, run.id());
    assert_eq!(
        review.binding.base_manifest_hash,
        verification.base_manifest_hash
    );
    assert_eq!(
        review.binding.candidate_manifest_hash,
        verification.candidate_manifest_hash
    );
    assert_eq!(review.binding.profile_hash, verification.profile_hash);
    assert_eq!(review.changes.len(), 2);
    assert_eq!(review.count(ChangeKind::Replace), 1);
    assert_eq!(review.count(ChangeKind::Create), 1);
    assert_eq!(review.count(ChangeKind::Delete), 0);

    let lib = &review.changes[0];
    assert_eq!(lib.path, rel("src/lib.rs"));
    assert_eq!(lib.kind, ChangeKind::Replace);
    assert_eq!(
        lib.old,
        Some(ManifestEntry::of(b"pub fn answer() -> u32 { 41 }\n"))
    );
    assert_eq!(
        lib.new,
        Some(ManifestEntry::of(b"pub fn answer() -> u32 { 42 }\n"))
    );
    assert_eq!(
        diff_text(lib),
        "--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1,1 +1,1 @@\n\
         -pub fn answer() -> u32 { 41 }\n+pub fn answer() -> u32 { 42 }\n"
    );
    let new = &review.changes[1];
    assert_eq!(new.kind, ChangeKind::Create);
    assert_eq!(new.old, None);
    assert_eq!(new.new.as_ref().unwrap().size, 18);
    assert_eq!(
        diff_text(new),
        "--- a/src/new.rs\n+++ b/src/new.rs\n@@ -0,0 +1,1 @@\n+pub fn added() {}\n"
    );

    let records = f.ledger.verify_run(run.id().ledger_key()).unwrap();
    let last = records.last().unwrap();
    assert_eq!(last.event_kind, "review.computed");
    assert!(last.payload.contains(&hex::encode(review.binding.hash())));
    // The review is data: the run, its authority and the project are
    // unchanged.
    assert_eq!(run.state(), RunState::StructurallyVerified);
    assert_eq!(digest(&f.project), before);
}

#[test]
fn p1_d_02_review_is_deterministic_and_the_binding_covers_every_field() {
    let f = fixture();
    let mut run = verified(
        &f,
        vec![replace("src/util.rs", "pub fn helper() {}\n// x\n")],
    );
    let first = run.review().unwrap();
    assert_eq!(run.review().unwrap(), first);
    let base = first.binding;
    let other_hash = ManifestEntry::of(b"other").sha256;
    let mut variants = vec![];
    let mut v = base;
    v.run_id = RunId::generate();
    variants.push(v);
    let other_manifest = {
        let mut m = Manifest::default();
        m.insert(rel("x"), ManifestEntry::of(b"x"));
        m.hash()
    };
    let mut v = base;
    v.base_manifest_hash = other_manifest;
    variants.push(v);
    let mut v = base;
    v.candidate_manifest_hash = other_manifest;
    variants.push(v);
    let mut v = base;
    v.profile_hash = other_hash;
    variants.push(v);
    for variant in variants {
        assert_ne!(variant.hash(), base.hash());
    }
}

#[test]
fn p1_d_03_unified_diffs_are_exact_and_bounded() {
    let old = "a\nb\nc\nd\ne\nf\ng\nh\ni\nj\nk\nl\nm\n";
    let new = "a\nB\nc\nd\ne\nf\ng\nh\ni\nj\nk\nL\nm\nn";
    let (text, truncated) = unified_for_test("x", old, new, 1 << 20);
    assert!(!truncated);
    assert_eq!(
        text,
        "--- a/x\n+++ b/x\n\
         @@ -1,5 +1,5 @@\n a\n-b\n+B\n c\n d\n e\n\
         @@ -9,5 +9,6 @@\n i\n j\n k\n-l\n+L\n m\n+n\n\\ No newline at end of file\n"
    );
    let (cut, truncated) = unified_for_test("x", old, new, 40);
    assert!(truncated);
    assert!(cut.len() <= 40);
    assert!(text.starts_with(&cut));
    // Identical content renders no hunk.
    assert_eq!(
        unified_for_test("x", old, old, 1 << 20).0,
        "--- a/x\n+++ b/x\n"
    );
}

// ── Negative controls ───────────────────────────────────────────────────────

#[test]
fn p1_d_nc_01_only_a_verified_candidate_has_a_review() {
    let f = fixture();
    let mut run = staged_run(&f);
    assert!(matches!(run.review(), Err(RunError::InvalidState { .. })));
    run.edit(replace("src/lib.rs", "x\n")).unwrap();
    assert!(matches!(run.review(), Err(RunError::InvalidState { .. })));

    let mut rejected = staged_run(&f);
    rejected.edit(replace("src/lib.rs", "y\n")).unwrap();
    std::fs::write(
        rejected
            .staging_path_for_test()
            .unwrap()
            .join("docs/guide.md"),
        "tampered\n",
    )
    .unwrap();
    assert!(!rejected.verify_structural().unwrap().passed());
    assert!(matches!(
        rejected.review(),
        Err(RunError::InvalidState { .. })
    ));

    let mut discarded = verified(&f, vec![replace("src/lib.rs", "z\n")]);
    discarded.discard_staging().unwrap();
    assert_eq!(discarded.review(), Err(RunError::CandidateUnavailable));
    assert!(discarded.verification().is_none());
}

#[test]
fn p1_d_nc_02_staging_changed_after_verification_withdraws_the_candidate() {
    let f = fixture();
    for tamper in [
        |s: &Path| std::fs::write(s.join("src/lib.rs"), "pub fn evil() {}\n").unwrap(),
        |s: &Path| std::fs::write(s.join("src/extra.rs"), "pub fn extra() {}\n").unwrap(),
        |s: &Path| std::fs::write(s.join("tests/check.rs"), "// off\n").unwrap(),
        |s: &Path| std::fs::remove_file(s.join("src/util.rs")).unwrap(),
    ] {
        let mut run = verified(
            &f,
            vec![replace("src/lib.rs", "pub fn answer() -> u32 { 42 }\n")],
        );
        tamper(&run.staging_path_for_test().unwrap());
        assert_eq!(run.review(), Err(RunError::CandidateChanged));
        assert!(
            run.verification().is_none(),
            "the verification is withdrawn"
        );
        assert!(run.review().is_err(), "and never comes back");
        assert!(kinds(&f, &run).contains(&"review.candidate_withdrawn".to_string()));
    }
}

#[test]
fn p1_d_nc_03_rendering_is_bounded_for_model_controlled_candidates() {
    let f = fixture();
    let big = "x".repeat(MAX_DIFF_FILE_BYTES + 1);
    let chunk: String = (0..1500)
        .map(|i| format!("line {i:06} of generated text\n"))
        .collect();
    assert!(chunk.len() > MAX_DIFF_BYTES_PER_FILE / 2);
    let mut edits = vec![create("src/big.rs", &big)];
    for i in 0..14 {
        edits.push(create(&format!("src/gen{i:02}.rs"), &chunk));
    }
    let mut run = verified(&f, edits);
    let review = run.review().unwrap();
    assert_eq!(review.changes.len(), 15);
    let big_change = review
        .changes
        .iter()
        .find(|c| c.path == rel("src/big.rs"))
        .unwrap();
    assert_eq!(big_change.diff, TextDiff::Omitted(DiffOmitted::TooLarge));
    let mut total = 0;
    let mut exhausted = 0;
    for change in &review.changes {
        match &change.diff {
            TextDiff::Unified { text, .. } => {
                assert!(text.len() <= MAX_DIFF_BYTES_PER_FILE);
                total += text.len();
            }
            TextDiff::Omitted(DiffOmitted::BudgetExhausted) => exhausted += 1,
            TextDiff::Omitted(_) => {}
        }
    }
    assert!(total <= MAX_DIFF_BYTES_TOTAL);
    assert!(exhausted > 0, "the total budget was reached");
    // Hashes and sizes are still reported for every change.
    assert!(review.changes.iter().all(|c| c.new.is_some()));
}

#[test]
fn p1_d_nc_04_diff_like_candidate_content_stays_inside_its_file() {
    let f = fixture();
    let hostile = "--- a/src/other.rs\n+++ b/src/other.rs\n@@ -1 +1 @@\n+approved: true\n";
    let mut run = verified(&f, vec![replace("src/util.rs", hostile)]);
    let review = run.review().unwrap();
    assert_eq!(review.changes.len(), 1);
    let text = diff_text(&review.changes[0]);
    // Every candidate line is rendered as an added line of this file.
    for line in hostile.lines() {
        assert!(text.contains(&format!("+{line}\n")));
    }
    assert_eq!(text.matches("\n--- a/").count(), 0);
}

#[test]
fn p1_d_nc_05_an_unrecordable_review_is_not_returned() {
    let f = fixture();
    // 0–5 create/grant/snapshot; 6–7 edit; 8 verify.structural; 9 review.
    let store = FailingStore::new(&f.ledger, 9);
    let mut run = new_run_with(&f, store, default_scopes());
    run.grant(&parent(&f)).unwrap();
    run.snapshot().unwrap();
    run.edit(replace("src/lib.rs", "x\n")).unwrap();
    assert!(run.verify_structural().unwrap().passed());
    assert!(matches!(run.review(), Err(RunError::Ledger(_))));
    // Nothing was withdrawn: a later review can still be recorded.
    assert!(run.review().is_ok());
}

#[test]
fn p1_d_nc_06_tampered_base_bytes_before_the_first_edit_end_the_run() {
    let f = fixture();
    let mut run = staged_run(&f);
    let staging = run.staging_path_for_test().unwrap();
    std::fs::write(staging.join("src/lib.rs"), "pub fn swapped() {}\n").unwrap();
    assert_eq!(
        run.edit(replace("src/lib.rs", "pub fn answer() -> u32 { 42 }\n")),
        Err(RunError::StagingRedirect)
    );
    assert_eq!(
        run.state(),
        RunState::Failed(FailureReason::StagingRedirect)
    );
}
