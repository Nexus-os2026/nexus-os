//! Oracle-add for Codex review 1 of PR #74 (MEDIUM), pre-registered before
//! the repair. The candidate the kernel verifies, reviews and publishes must
//! be the input the Owner is shown, for plain text as for a page. A same-UID
//! edit of the staged candidate between its creation and its structural
//! verification (crate-private hook `files.staged`) is refused at prepare:
//! no commitment, nothing published, the staging discarded. A plain
//! create's receipt names exactly the published bytes. A child of the
//! S1a-D+B oracles, for their fixture.

use super::*;

/// Overwrite the staged candidate `name` with `with`, once, at
/// `files.staged`: after the kernel staged it, before it is verified.
fn tamper(f: &Fx, name: &'static str, with: String) {
    let parent = f.state().join("coding-runs");
    f.gov.files().hooks().arm(at::FILE_STAGED, move || {
        let staged = find(&parent, name).expect("the staged candidate");
        std::fs::write(staged, with).expect("a same-UID edit");
    });
}

/// Refused at prepare as `target_changed`, with no commitment, no staging
/// left, nothing in the workspace and the staging's discard recorded.
fn refused_before_commitment(f: &Fx, refused: AuthorityError, what: &str) {
    assert_eq!(refused, AuthorityError::TargetChanged, "{what}");
    assert!(
        f.gov
            .authority()
            .commitments()
            .views_of_run(f.run)
            .is_empty(),
        "{what}: no commitment"
    );
    assert!(f.staged().is_empty(), "{what}: staging discarded");
    assert!(f.entries().is_empty(), "{what}: nothing published");
    assert!(
        f.events().iter().any(|e| e == "staging.discarded"),
        "{what}"
    );
}

/// The MEDIUM: plain text edited after staging, before verification, is
/// refused before any commitment exists.
#[test]
fn s1a_d_r1_a_a_tampered_text_candidate_is_refused_before_commitment() {
    let (f, _) = fx(None);
    tamper(
        &f,
        "notes.txt",
        "Fresh bread daily, and a forged line.\n".into(),
    );
    let refused = f
        .propose("notes.txt", "Fresh bread daily.\n")
        .expect_err("the staged bytes are not the input shown");
    refused_before_commitment(&f, refused, "text");
}

/// Control: a page edited the same way (still inert, still showing every
/// fact) is refused too.
#[test]
fn s1a_d_r1_b_a_tampered_page_candidate_is_refused_before_commitment() {
    let edited = REFERENCE.replace("baked every morning.", "baked every evening.");
    assert_ne!(edited, REFERENCE);
    let (f, _) = fx(Some(reference()));
    tamper(&f, PAGE, edited);
    let refused = f
        .propose(PAGE, REFERENCE)
        .expect_err("the staged bytes are not the page shown");
    refused_before_commitment(&f, refused, "page");
}

/// A plain create's receipt is the published file's own identity: its
/// SHA-256 and size equal the bytes at the name.
#[test]
fn s1a_d_r1_c_a_plain_receipt_names_the_published_bytes() {
    let (f, _) = fx(None);
    let view = f.ready("notes.txt", "Fresh bread daily.\n");
    f.execute(&view).expect("published");
    let published = std::fs::read(f.ws.join("notes.txt")).unwrap();
    let receipt = f.door.file_receipt(view.id).expect("a receipt");
    assert_eq!(receipt.sha256, sha256(&published));
    assert_eq!(receipt.bytes, published.len() as u64);
    assert_eq!(receipt.policy_hash, None);
}
