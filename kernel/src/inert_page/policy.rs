//! The declared policy (v1): every rule of the inert page check in one
//! constant, [`POLICY_V1`], and its hash. Skeleton: the values and the hash
//! land with the implementation.

use super::tree::{Class, Tag};
use super::RefusalCode;

/// Mother's fixed stylesheet, carried inline in every page.
pub const MOTHER_CSS: &str = "";

/// The one label of a pass.
pub const LABEL_PASSED: &str = POLICY_V1.label_passed;

/// Every declared rule of the inert page check.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PolicyParams {
    pub version: u32,
    pub max_page_bytes: usize,
    pub control_ranges: &'static [(u32, u32)],
    pub invisible_ranges: &'static [(u32, u32)],
    pub profile_ranges: &'static [(u32, u32)],
    pub controls_rule: &'static str,
    pub lexical_priority: &'static str,
    pub lexical_scan: &'static str,
    pub script_tag: &'static str,
    pub style_tag: &'static str,
    pub head_tags: &'static [&'static str],
    pub body_tags: &'static [Tag],
    pub event_prefix: &'static str,
    pub style_attribute: &'static str,
    pub url_attributes: &'static [&'static str],
    pub prologue_attributes: &'static [&'static str],
    pub body_attributes: [&'static str; 2],
    pub declarations: &'static [(&'static str, RefusalCode)],
    pub entities: &'static [(char, &'static str)],
    pub needles: &'static [(&'static str, RefusalCode)],
    pub head_segments: [&'static str; 4],
    pub epilogue: &'static str,
    pub css: &'static str,
    pub lang_rule: &'static str,
    pub content_model: &'static str,
    pub classes: &'static [Class],
    pub id_rule: &'static str,
    pub max_depth: usize,
    pub max_elements: usize,
    pub max_word_chars: usize,
    pub max_title_chars: usize,
    pub max_id_chars: usize,
    pub caps_rule: &'static str,
    pub refusal_order: &'static str,
    pub refusal_codes: &'static [RefusalCode],
    pub flatten_text: &'static str,
    pub flatten_words: &'static str,
    pub spacing: &'static str,
    pub canonical: &'static str,
    pub matching: &'static str,
    pub fc1: &'static str,
    pub fc2: &'static str,
    pub fc3: &'static str,
    pub fc4: &'static str,
    pub facts_construction: &'static str,
    pub facts_name_max_chars: usize,
    pub facts_value_min_chars: usize,
    pub facts_value_max_chars: usize,
    pub facts_hours_min_lines: usize,
    pub facts_hours_max_lines: usize,
    pub phone_min_digits: usize,
    pub phone_max_digits: usize,
    pub phone_chars: &'static [char],
    pub phone_shape: &'static str,
    pub phone_max_non_digit_run: usize,
    pub fc4_threshold: usize,
    pub fc4_separators: &'static [char],
    pub label_passed: &'static str,
    pub label_refused_prefix: &'static str,
    pub facts_line_template: &'static str,
}

/// Skeleton placeholder.
pub const POLICY_V1: PolicyParams = PolicyParams {
    version: 0,
    max_page_bytes: 0,
    control_ranges: &[],
    invisible_ranges: &[],
    profile_ranges: &[],
    controls_rule: "",
    lexical_priority: "",
    lexical_scan: "",
    script_tag: "",
    style_tag: "",
    head_tags: &[],
    body_tags: &[],
    event_prefix: "",
    style_attribute: "",
    url_attributes: &[],
    prologue_attributes: &[],
    body_attributes: ["", ""],
    declarations: &[],
    entities: &[],
    needles: &[],
    head_segments: ["", "", "", ""],
    epilogue: "",
    css: MOTHER_CSS,
    lang_rule: "",
    content_model: "",
    classes: &[],
    id_rule: "",
    max_depth: 0,
    max_elements: 0,
    max_word_chars: 0,
    max_title_chars: 0,
    max_id_chars: 0,
    caps_rule: "",
    refusal_order: "",
    refusal_codes: &[],
    flatten_text: "",
    flatten_words: "",
    spacing: "",
    canonical: "",
    matching: "",
    fc1: "",
    fc2: "",
    fc3: "",
    fc4: "",
    facts_construction: "",
    facts_name_max_chars: 0,
    facts_value_min_chars: 0,
    facts_value_max_chars: 0,
    facts_hours_min_lines: 0,
    facts_hours_max_lines: 0,
    phone_min_digits: 0,
    phone_max_digits: 0,
    phone_chars: &[],
    phone_shape: "",
    phone_max_non_digit_run: 0,
    fc4_threshold: 0,
    fc4_separators: &[],
    label_passed: "",
    label_refused_prefix: "",
    facts_line_template: "",
};

/// The hash of [`POLICY_V1`].
pub fn policy_hash() -> [u8; 32] {
    hash_params(&POLICY_V1)
}

/// The hash of a policy table.
pub(super) fn hash_params(p: &PolicyParams) -> [u8; 32] {
    let _ = p;
    [0; 32]
}
