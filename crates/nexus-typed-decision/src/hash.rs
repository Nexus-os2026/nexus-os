//! `set_hash` and `registry_hash` (§6): SHA-256 over an exhaustive,
//! no-`..` destructuring, each field length-prefixed (S1a §4.7's method).

use sha2::{Digest, Sha256};

use crate::registry::{Kind, Question, QuestionSet, ReadPolicy, REGISTRY};
use crate::render::{render_schema, render_system_with, DecodeCode, Texts, TEXTS_V1};

const SET_DOMAIN: &[u8] = b"nexus.typed_decision.set.v1";
const REGISTRY_DOMAIN: &[u8] = b"nexus.typed_decision.registry.v1";

/// One length-prefixed field.
fn put(h: &mut Sha256, bytes: &[u8]) {
    h.update((bytes.len() as u64).to_be_bytes());
    h.update(bytes);
}

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
    let QuestionSet {
        id,
        version,
        lane,
        projection,
        mode,
        questions,
    } = set;
    let mut h = Sha256::new();
    put(&mut h, SET_DOMAIN);
    put(&mut h, id.as_str().as_bytes());
    put(&mut h, &version.to_be_bytes());
    put(&mut h, lane.code().as_bytes());
    put(&mut h, projection.id().as_bytes());
    put(&mut h, &projection.version().to_be_bytes());
    put(&mut h, mode.code().as_bytes());
    put(&mut h, &(questions.len() as u64).to_be_bytes());
    for question in *questions {
        put_question(&mut h, question);
    }
    // Both generated artifacts. A set with no registered system text hashes
    // a distinct marker in its place.
    match render_system_with(set, texts) {
        Ok(system) => put(&mut h, system.as_bytes()),
        Err(e) => put(&mut h, e.to_string().as_bytes()),
    }
    put(&mut h, render_schema(set).as_bytes());
    put_texts(&mut h, texts);
    put_rules(&mut h, rules);
    h.finalize().into()
}

fn put_question(h: &mut Sha256, question: &Question) {
    let Question {
        id,
        kind,
        instructions,
        if_yes,
        if_no,
        read,
    } = question;
    put(h, id.as_str().as_bytes());
    match kind {
        Kind::Noul { polarity } => {
            put(h, b"noul");
            put(h, polarity.code().as_bytes());
        }
        Kind::Choice { options } => {
            put(h, b"choice");
            put(h, &(options.len() as u64).to_be_bytes());
            for (label, disposition) in *options {
                put(h, label.as_str().as_bytes());
                put(h, disposition.code().as_bytes());
            }
        }
        Kind::Score {
            levels,
            at_least,
            polarity,
        } => {
            put(h, b"score");
            put(h, &(levels.len() as u64).to_be_bytes());
            for level in *levels {
                put(h, level.as_str().as_bytes());
            }
            put(h, &[*at_least]);
            put(h, polarity.code().as_bytes());
        }
    }
    put(h, instructions.as_bytes());
    put(h, if_yes.as_bytes());
    put(h, if_no.as_bytes());
    let ReadPolicy {
        no_below,
        yes_above,
    } = read;
    put(h, &no_below.get().to_be_bytes());
    put(h, &yes_above.get().to_be_bytes());
}

fn put_texts(h: &mut Sha256, texts: &Texts) {
    let Texts {
        preamble,
        rule_noul_probabilities,
        describe_page_text_v1,
        describe_untrusted_text_v1,
        shape_intro,
        shape_ending,
        correction_prefix,
        correction_suffix,
        phrases,
    } = texts;
    for text in [
        preamble,
        rule_noul_probabilities,
        describe_page_text_v1,
        describe_untrusted_text_v1,
        shape_intro,
        shape_ending,
        correction_prefix,
        correction_suffix,
    ] {
        put(h, text.as_bytes());
    }
    for (code, phrase) in DecodeCode::ALL.iter().zip(phrases) {
        put(h, code.code().as_bytes());
        put(h, phrase.as_bytes());
    }
}

fn put_rules(h: &mut Sha256, rules: &RuleIds) {
    let RuleIds {
        prompt,
        decode,
        normalize,
        read,
        escape,
        frame,
        chars,
    } = rules;
    for rule in [prompt, decode, normalize, read, escape, frame, chars] {
        put(h, rule.as_bytes());
    }
}

/// The hash of the whole registry.
pub fn registry_hash() -> [u8; 32] {
    registry_hash_of(&REGISTRY, &TEXTS_V1, &RULES_V1)
}

pub(crate) fn registry_hash_of(sets: &[&QuestionSet], texts: &Texts, rules: &RuleIds) -> [u8; 32] {
    let mut h = Sha256::new();
    put(&mut h, REGISTRY_DOMAIN);
    put(&mut h, &(sets.len() as u64).to_be_bytes());
    for set in sets {
        put(&mut h, set.id.as_str().as_bytes());
        put(&mut h, &set_hash_with(set, texts, rules));
    }
    put_rules(&mut h, rules);
    h.finalize().into()
}
