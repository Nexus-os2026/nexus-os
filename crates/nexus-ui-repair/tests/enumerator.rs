//! Fixture-based enumerator tests. See v1.1 §4 and §6.5.

use std::collections::HashSet;
use std::path::PathBuf;

use nexus_ui_repair::specialists::destructive_policy::ElementKind;
use nexus_ui_repair::specialists::enumerator::Enumerator;

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/teams_page_snapshot.html")
}

#[test]
fn enumerates_all_interactive_elements_from_fixture() {
    let enumerator = Enumerator::new();
    let elements = enumerator
        .enumerate_fixture(&fixture_path())
        .expect("enumerate fixture");
    assert!(
        elements.len() >= 7,
        "expected at least 7 elements, got {}",
        elements.len()
    );
}

#[test]
fn tags_destructive_elements_correctly() {
    let enumerator = Enumerator::new();
    let elements = enumerator
        .enumerate_fixture(&fixture_path())
        .expect("enumerate fixture");

    let delete = elements
        .iter()
        .find(|e| e.id == "delete-team-btn")
        .expect("delete-team-btn present");
    assert_eq!(delete.kind, ElementKind::Destructive);

    let reset = elements
        .iter()
        .find(|e| e.id == "reset-all-btn")
        .expect("reset-all-btn present");
    assert_eq!(reset.kind, ElementKind::Destructive);
}

#[test]
fn does_not_tag_non_destructive_buttons() {
    let enumerator = Enumerator::new();
    let elements = enumerator
        .enumerate_fixture(&fixture_path())
        .expect("enumerate fixture");

    let edit = elements
        .iter()
        .find(|e| e.id == "edit-team-btn")
        .expect("edit-team-btn present");
    assert_eq!(edit.kind, ElementKind::Button);

    let save = elements
        .iter()
        .find(|e| e.id == "save-btn")
        .expect("save-btn present");
    assert_eq!(save.kind, ElementKind::Button);
}

#[test]
fn fingerprints_are_stable_and_unique() {
    let enumerator = Enumerator::new();
    let e1 = enumerator
        .enumerate_fixture(&fixture_path())
        .expect("enumerate fixture 1");
    let e2 = enumerator
        .enumerate_fixture(&fixture_path())
        .expect("enumerate fixture 2");

    // Stable: same input → same fingerprints, same order.
    assert_eq!(e1.len(), e2.len());
    for (a, b) in e1.iter().zip(e2.iter()) {
        assert_eq!(a.fingerprint, b.fingerprint);
        assert_eq!(a.id, b.id);
    }

    // Unique within a page.
    let mut seen = HashSet::new();
    for el in &e1 {
        assert!(
            seen.insert(el.fingerprint.clone()),
            "duplicate fingerprint for element {:?}",
            el
        );
    }
}
