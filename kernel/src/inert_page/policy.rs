//! The declared policy (v1): every rule of the inert page check, in one
//! constant, [`POLICY_V1`], and its hash.
//!
//! The validator reads every parameter from `POLICY_V1`. The `*_rule`,
//! `cm:`, `fc*:` and other identifier strings declare behaviour that is
//! written as code; the frozen oracles (S-O2, S-O3, S-O6 here) pin it, so a
//! change of behaviour without a new identifier turns them red. The facts
//! constants and identifiers are declared here already and are used by the
//! facts check (S1a-B).
//!
//! [`policy_hash`] is SHA-256 over `put("nexus.inert_page.policy.v1")` and,
//! for every field in declaration order, `put(field name)`, a kind byte and
//! the value, where `put(b)` is `b`'s length as a u64 (big-endian) then `b`:
//! - 1, a number: u64;
//! - 2, text: `put(text)`;
//! - 3, a list of text (tag, class and code names included): u64 count, then
//!   `put` each;
//! - 4, character ranges: u64 count, then each inclusive bound as a u32;
//! - 5, characters: u64 count, then each as a u32;
//! - 6, pairs: u64 count, then `put` each half (an entity's character as
//!   UTF-8; a code as its name);
//! - 7, the stylesheet: the 32-byte SHA-256 of its bytes.
//!
//! The policy hash names the declared policy, not the implementation.

use sha2::{Digest, Sha256};

use super::tree::{Class, Tag};
use super::RefusalCode;

/// Mother's fixed stylesheet, carried inline in every page.
pub const MOTHER_CSS: &str = include_str!("mother-page-v1.css");

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

/// Policy v1 (design v2 §4 and §5).
pub const POLICY_V1: PolicyParams = PolicyParams {
    version: 1,
    max_page_bytes: 65_536,
    control_ranges: &[(0x00, 0x1F), (0x7F, 0x9F)],
    invisible_ranges: &[
        (0xAD, 0xAD),
        (0x34F, 0x34F),
        (0x61C, 0x61C),
        (0x115F, 0x1160),
        (0x180E, 0x180E),
        (0x200B, 0x200F),
        (0x2028, 0x202E),
        (0x2060, 0x206F),
        (0x3164, 0x3164),
        (0xFE00, 0xFE0F),
        (0xFEFF, 0xFEFF),
        (0xFFA0, 0xFFA0),
        (0xFFF9, 0xFFFB),
        (0xE0000, 0xE007F),
        (0xE0100, 0xE01EF),
    ],
    profile_ranges: &[
        (0x20, 0x7E),
        (0xA1, 0xAC),
        (0xAE, 0xFF),
        (0x100, 0x17F),
        (0x2012, 0x2014),
        (0x2018, 0x201F),
        (0x2022, 0x2022),
        (0x2026, 0x2026),
        (0x20AC, 0x20AC),
        (0x2122, 0x2122),
    ],
    controls_rule: "controls:v1:U+000A=separator-only;U+000D=carriage_return;\
        U+FEFF-at-0=byte_order_mark;control_ranges=control_char;\
        invisible_ranges=invisible_char;outside-profile_ranges=char_not_allowed",
    lexical_priority: "lexical-priority:v1:tag(script_tag)>tag(style_tag-after-first)>\
        tag(not-allowed)>tag(case)>attr(event_prefix*)>attr(style_attribute)>\
        attr(url_attributes)>attr(not-allowed)>declarations(in-order;<!-not-at-0)>\
        entities>needles(in-order);report=first-code-in-refusal_codes-at-its-first-offset",
    lexical_scan: "lexical-scan:v1:s1=whole-input,unless-too_large-alone;\
        tags=html-data-state-from-every-lt-letter-or-lt-slash-letter;\
        tag-and-dangerous-attribute-names=ascii-case-insensitive;allowed-attributes=exact;\
        declarations,entities,needles=every-offset;needles=ascii-case-insensitive;\
        entities=exact",
    script_tag: "script",
    style_tag: "style",
    head_tags: &["html", "head", "body", "meta", "title", "style"],
    body_tags: &[
        Tag::Header,
        Tag::Main,
        Tag::Footer,
        Tag::Section,
        Tag::H1,
        Tag::H2,
        Tag::H3,
        Tag::P,
        Tag::Address,
        Tag::Ul,
        Tag::Ol,
        Tag::Li,
        Tag::Strong,
        Tag::Em,
        Tag::Small,
        Tag::Br,
    ],
    event_prefix: "on",
    style_attribute: "style",
    url_attributes: &[
        "href",
        "src",
        "srcset",
        "action",
        "formaction",
        "poster",
        "background",
        "cite",
        "data",
        "ping",
        "manifest",
        "xlink:href",
    ],
    prologue_attributes: &["lang", "charset", "name", "content"],
    body_attributes: ["class", "id"],
    declarations: &[
        ("<!--", RefusalCode::Comment),
        ("<!", RefusalCode::MarkupDeclaration),
        ("<?", RefusalCode::ProcessingInstruction),
    ],
    entities: &[
        ('&', "&amp;"),
        ('<', "&lt;"),
        ('>', "&gt;"),
        ('"', "&quot;"),
        ('\'', "&#39;"),
    ],
    needles: &[
        ("://", RefusalCode::RemoteReference),
        ("url(", RefusalCode::CssUrl),
        ("@import", RefusalCode::CssImport),
        ("javascript:", RefusalCode::JavascriptScheme),
        ("data:", RefusalCode::DataScheme),
    ],
    head_segments: [
        "<!doctype html>\n<html lang=\"",
        "\">\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<title>",
        "</title>\n<style>\n",
        "</style>\n</head>\n<body>\n",
    ],
    epilogue: "</body>\n</html>\n",
    css: MOTHER_CSS,
    lang_rule: "lang:v1:[a-z]{2}(-[A-Z]{2})?",
    content_model: "cm:v1:body=header?,main,footer?;main=(section|block)+;header=block+;\
        footer=block+;section=block+;block=h1|h2|h3|p|address|ul|ol;ul=li+;ol=li+;\
        leaf=h1|h2|h3|p|address|li;leaf=inline+;span=strong|em|small;span=inline+;\
        inline=text|br|span;title=text;attrs=class?,id?;br=void,no-attrs",
    classes: &[
        Class::Lead,
        Class::Muted,
        Class::Center,
        Class::Note,
        Class::Plain,
    ],
    id_rule: "id:v1:[a-z][a-z0-9-]*;chars<=max_id_chars;unique-per-document",
    max_depth: 8,
    max_elements: 400,
    max_word_chars: 40,
    max_title_chars: 80,
    max_id_chars: 32,
    caps_rule: "caps:v1:bytes<=max_page_bytes;depth<=max_depth(body-children=1,br-counts);\
        elements-in-body<=max_elements(br-counts);word-chars<=max_word_chars;\
        title-chars=1..max_title_chars(decoded);id-chars<=max_id_chars;\
        chars=unicode-scalar-values",
    refusal_order: "refusal-order:v1:s1>s2>s3>s4;s1,s2=refusal_codes-order;\
        s3=first-in-document-order;s4=first-differing-byte",
    refusal_codes: &RefusalCode::CODES,
    flatten_text: "flatten-text:v1:spans-joined;br=U+0020",
    flatten_words: "flatten-words:v1:spans-joined;br=break;split=U+0020",
    spacing: "spacing:v1:leaf-flattened-text,title;no-U+000A;no-leading-space;\
        no-trailing-space;no-double-space;no-space-next-to-br;no-br-first;no-br-last;\
        no-br-br",
    canonical: "canonical:v1:no-indent;block-lf;class-then-id;entities=5;\
        s3-superset=attr-order,raw-quotes,missing-lf",
    matching: "match:v1:decoded-element-text-byte-exact",
    fc1: "fc1:v1:one-h1;h1=name;title=name",
    fc2: "fc2:v1:each-line=text(li|p)",
    fc3: "fc3:v1:text(any-element,leaf-included)=phone",
    fc4: "fc4:v1:tokens(leaf,title);digits=ascii;sep=fc4_separators;\
        phone-like>=fc4_threshold;eq=phone-digits",
    facts_construction: "facts-construction:v1:\
        value-chars=facts_value_min_chars..facts_value_max_chars;\
        name-chars<=facts_name_max_chars;hours-lines=facts_hours_min_lines..facts_hours_max_lines;\
        s1-characters;spacing:v1;no-needle;phone=phone-shape:v1;no-phone-like-token(name,hours)",
    facts_name_max_chars: 80,
    facts_value_min_chars: 1,
    facts_value_max_chars: 120,
    facts_hours_min_lines: 1,
    facts_hours_max_lines: 7,
    phone_min_digits: 7,
    phone_max_digits: 15,
    phone_chars: &[
        '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', ' ', '-', '.', '(', ')', '+',
    ],
    phone_shape: "phone-shape:v1:chars=phone_chars;plus=single-leading;start=+|(|digit;\
        end=digit;non-digit-run<=phone_max_non_digit_run;\
        digits=phone_min_digits..phone_max_digits;digits=ascii",
    phone_max_non_digit_run: 2,
    fc4_threshold: 7,
    fc4_separators: &[
        ' ', '-', '.', '(', ')', '/', '+', '\u{2012}', '\u{2013}', '\u{2014}',
    ],
    label_passed: "inert page check: passed (not rendered)",
    label_refused_prefix: "inert page check: refused",
    facts_line_template: "facts present as given: name, hours ({n} lines), phone",
};

/// The hash of [`POLICY_V1`].
pub fn policy_hash() -> [u8; 32] {
    hash_params(&POLICY_V1)
}

const POLICY_DOMAIN: &[u8] = b"nexus.inert_page.policy.v1";

/// The hash of a policy table. Every field is destructured (no `..`), so a
/// field that is not hashed does not compile.
pub(super) fn hash_params(p: &PolicyParams) -> [u8; 32] {
    let PolicyParams {
        version,
        max_page_bytes,
        control_ranges,
        invisible_ranges,
        profile_ranges,
        controls_rule,
        lexical_priority,
        lexical_scan,
        script_tag,
        style_tag,
        head_tags,
        body_tags,
        event_prefix,
        style_attribute,
        url_attributes,
        prologue_attributes,
        body_attributes,
        declarations,
        entities,
        needles,
        head_segments,
        epilogue,
        css,
        lang_rule,
        content_model,
        classes,
        id_rule,
        max_depth,
        max_elements,
        max_word_chars,
        max_title_chars,
        max_id_chars,
        caps_rule,
        refusal_order,
        refusal_codes,
        flatten_text,
        flatten_words,
        spacing,
        canonical,
        matching,
        fc1,
        fc2,
        fc3,
        fc4,
        facts_construction,
        facts_name_max_chars,
        facts_value_min_chars,
        facts_value_max_chars,
        facts_hours_min_lines,
        facts_hours_max_lines,
        phone_min_digits,
        phone_max_digits,
        phone_chars,
        phone_shape,
        phone_max_non_digit_run,
        fc4_threshold,
        fc4_separators,
        label_passed,
        label_refused_prefix,
        facts_line_template,
    } = *p;
    let mut e = Enc(Sha256::new());
    e.put(POLICY_DOMAIN);
    e.num("version", version as usize);
    e.num("max_page_bytes", max_page_bytes);
    e.ranges("control_ranges", control_ranges);
    e.ranges("invisible_ranges", invisible_ranges);
    e.ranges("profile_ranges", profile_ranges);
    e.text("controls_rule", controls_rule);
    e.text("lexical_priority", lexical_priority);
    e.text("lexical_scan", lexical_scan);
    e.text("script_tag", script_tag);
    e.text("style_tag", style_tag);
    e.texts("head_tags", head_tags);
    let tags: Vec<&str> = body_tags.iter().map(|t| t.name()).collect();
    e.texts("body_tags", &tags);
    e.text("event_prefix", event_prefix);
    e.text("style_attribute", style_attribute);
    e.texts("url_attributes", url_attributes);
    e.texts("prologue_attributes", prologue_attributes);
    e.texts("body_attributes", &body_attributes);
    e.pairs("declarations", &coded(declarations));
    let ents: Vec<(String, &str)> = entities.iter().map(|(c, s)| (c.to_string(), *s)).collect();
    e.pairs("entities", &ents);
    e.pairs("needles", &coded(needles));
    e.texts("head_segments", &head_segments);
    e.text("epilogue", epilogue);
    e.digest("css", &Sha256::digest(css.as_bytes()));
    e.text("lang_rule", lang_rule);
    e.text("content_model", content_model);
    let class_names: Vec<&str> = classes.iter().map(|c| c.name()).collect();
    e.texts("classes", &class_names);
    e.text("id_rule", id_rule);
    e.num("max_depth", max_depth);
    e.num("max_elements", max_elements);
    e.num("max_word_chars", max_word_chars);
    e.num("max_title_chars", max_title_chars);
    e.num("max_id_chars", max_id_chars);
    e.text("caps_rule", caps_rule);
    e.text("refusal_order", refusal_order);
    let codes: Vec<&str> = refusal_codes.iter().map(|c| c.as_str()).collect();
    e.texts("refusal_codes", &codes);
    e.text("flatten_text", flatten_text);
    e.text("flatten_words", flatten_words);
    e.text("spacing", spacing);
    e.text("canonical", canonical);
    e.text("matching", matching);
    e.text("fc1", fc1);
    e.text("fc2", fc2);
    e.text("fc3", fc3);
    e.text("fc4", fc4);
    e.text("facts_construction", facts_construction);
    e.num("facts_name_max_chars", facts_name_max_chars);
    e.num("facts_value_min_chars", facts_value_min_chars);
    e.num("facts_value_max_chars", facts_value_max_chars);
    e.num("facts_hours_min_lines", facts_hours_min_lines);
    e.num("facts_hours_max_lines", facts_hours_max_lines);
    e.num("phone_min_digits", phone_min_digits);
    e.num("phone_max_digits", phone_max_digits);
    e.chars("phone_chars", phone_chars);
    e.text("phone_shape", phone_shape);
    e.num("phone_max_non_digit_run", phone_max_non_digit_run);
    e.num("fc4_threshold", fc4_threshold);
    e.chars("fc4_separators", fc4_separators);
    e.text("label_passed", label_passed);
    e.text("label_refused_prefix", label_refused_prefix);
    e.text("facts_line_template", facts_line_template);
    e.0.finalize().into()
}

struct Enc(Sha256);

impl Enc {
    fn put(&mut self, b: &[u8]) {
        self.0.update((b.len() as u64).to_be_bytes());
        self.0.update(b);
    }

    fn head(&mut self, name: &str, kind: u8, count: Option<usize>) {
        self.put(name.as_bytes());
        self.0.update([kind]);
        if let Some(n) = count {
            self.0.update((n as u64).to_be_bytes());
        }
    }

    fn num(&mut self, name: &str, v: usize) {
        self.head(name, 1, None);
        self.0.update((v as u64).to_be_bytes());
    }

    fn text(&mut self, name: &str, v: &str) {
        self.head(name, 2, None);
        self.put(v.as_bytes());
    }

    fn texts(&mut self, name: &str, v: &[&str]) {
        self.head(name, 3, Some(v.len()));
        for s in v {
            self.put(s.as_bytes());
        }
    }

    fn ranges(&mut self, name: &str, v: &[(u32, u32)]) {
        self.head(name, 4, Some(v.len()));
        for (lo, hi) in v {
            self.0.update(lo.to_be_bytes());
            self.0.update(hi.to_be_bytes());
        }
    }

    fn chars(&mut self, name: &str, v: &[char]) {
        self.head(name, 5, Some(v.len()));
        for c in v {
            self.0.update(u32::from(*c).to_be_bytes());
        }
    }

    fn pairs(&mut self, name: &str, v: &[(String, &str)]) {
        self.head(name, 6, Some(v.len()));
        for (a, b) in v {
            self.put(a.as_bytes());
            self.put(b.as_bytes());
        }
    }

    fn digest(&mut self, name: &str, v: &[u8]) {
        self.head(name, 7, None);
        self.0.update(v);
    }
}

fn coded(v: &[(&str, RefusalCode)]) -> Vec<(String, &'static str)> {
    v.iter().map(|(a, c)| (a.to_string(), c.as_str())).collect()
}
