//! `set_hash` and `registry_hash` (§6): SHA-256 over an exhaustive,
//! no-`..` destructuring, each field length-prefixed (S1a §4.7's method).

use crate::registry::{QuestionSet, REGISTRY};
use crate::render::{Texts, TEXTS_V1};

/// The versioned rule identifiers every set hash covers.
#[derive(Clone, Copy, Debug)]
pub(crate) struct RuleIds {
    pub(crate) prompt: &'static str,
    pub(crate) decode: &'static str,
    pub(crate) normalize: &'static str,
    pub(crate) read: &'static str,
    pub(crate) escape: &'static str,
    pub(crate) frame: &'static str,
    pub(crate) chars: &'static str,
}

pub(crate) const RULES_V1: RuleIds = RuleIds {
    prompt: "td-prompt-v1",
    decode: "td-decode-v1",
    normalize: "td-normalize-v1",
    read: "td-read-v1",
    escape: "td-escape-v1",
    frame: "td-frame-v1",
    chars: "td-chars-v1",
};

/// The hash of one registered set.
pub fn set_hash(set: &QuestionSet) -> [u8; 32] {
    set_hash_with(set, &TEXTS_V1, &RULES_V1)
}

pub(crate) fn set_hash_with(set: &QuestionSet, texts: &Texts, rules: &RuleIds) -> [u8; 32] {
    // STUB (oracle commit).
    let _ = (set, texts, rules);
    [0; 32]
}

/// The hash of the whole registry.
pub fn registry_hash() -> [u8; 32] {
    registry_hash_of(&REGISTRY, &TEXTS_V1, &RULES_V1)
}

pub(crate) fn registry_hash_of(sets: &[&QuestionSet], texts: &Texts, rules: &RuleIds) -> [u8; 32] {
    // STUB (oracle commit).
    let _ = (sets, texts, rules);
    [0; 32]
}
