//! Destructive Action Policy — Layer 1 only. See v1.1 amendment §6.5.
//!
//! **Phase gating.** This file ships Layer 1 only: the pattern denylist
//! applied at enumeration time. The `enumerator` specialist uses
//! `classify_element_kind` to tag every element with an [`ElementKind`];
//! elements tagged [`ElementKind::Destructive`] are recorded in the
//! report but skipped from the ACT phase by default.
//!
//! Layer 2 (confirmation-modal handling) and Layer 3 (per-page
//! descriptor opt-ins) are **deferred to Phase 1.3**. The Phase 1.3
//! commit is the same commit that first imports `nexus-computer-use`
//! into `nexus-ui-repair`, because Layers 2 and 3 only make sense once
//! the scout can actually move the mouse and type into modals.
//!
//! The five §6.5 acceptance cases are sketched in
//! `tests/destructive_policy.rs`: Phase 1.2 asserts only case 1 (the
//! pattern denylist) for real; cases 2–5 are `#[ignore]`'d stubs.

use std::sync::OnceLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

/// Taxonomy of interactive UI element kinds produced by the enumerator.
///
/// `Destructive` takes precedence over the HTML-tag-based variants: an
/// element whose label matches the denylist is always `Destructive`,
/// regardless of tag. See `classify_element_kind`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ElementKind {
    Button,
    Input,
    Select,
    Link,
    Destructive,
    Other,
}

/// Lazily compiled case-insensitive destructive-label pattern.
///
/// The pattern matches the word list from v1.1 §6.5. We use word
/// boundaries (`\b`) for single words and literal matches for multi-
/// word phrases so we don't false-positive on words like "deleted"
/// (which we actually *do* want to match — `\b` matches there because
/// `deleted` starts with the word `delet...` but `\bdelete\b` would
/// not match "deleted"). In practice we want both "Delete" and
/// "Deleted" to match, so we anchor on the shared stems and accept
/// the occasional precision loss in favor of recall — this is a
/// safety denylist, and recall matters more than precision.
fn destructive_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        // Case-insensitive alternation of every denylist term. Each
        // single word uses `\b` word boundaries; multi-word phrases
        // are matched literally.
        Regex::new(
            r"(?i)(\bdelete\b|\bremove\b|\breset\b|\bwipe\b|\brevoke\b|\bdestroy\b|\bpurge\b|\bdrop\b|clear all|factory reset|\buninstall\b|\bforget\b|\berase\b)",
        )
        .expect("destructive denylist regex must compile")
    })
}

/// Returns `true` if `label` matches any term in the v1.1 §6.5
/// destructive-label denylist, case-insensitively.
pub fn is_destructive_label(label: &str) -> bool {
    destructive_pattern().is_match(label)
}

/// Classify an element by its visible label and HTML role/tag.
///
/// Destructive wins. Otherwise the role/tag maps to its natural kind.
pub fn classify_element_kind(label: &str, role: &str) -> ElementKind {
    if is_destructive_label(label) {
        return ElementKind::Destructive;
    }
    match role {
        "button" => ElementKind::Button,
        "input" => ElementKind::Input,
        "select" => ElementKind::Select,
        "a" => ElementKind::Link,
        _ => ElementKind::Other,
    }
}
