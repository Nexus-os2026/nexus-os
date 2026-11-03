//! S-O1: the reference page is accepted; the label, the stylesheet and the
//! whole declared policy are pinned, the policy hash being recomputed from a
//! literal restatement with an independent encoder.
//! S-O1b: changing any one `POLICY_V1` field changes the policy hash.

use sha2::{Digest, Sha256};

use super::super::policy::hash_params;
use super::*;

const CSS_SHA256: &str = "fbed26aba654ef2e959416042141ce5a015dc790f4db8f3de8e839492037b8fe";
const REFERENCE_SHA256: &str = "c81d83a00b03fd73fee367cbab3ea162df46c034936db2ca0070a0c3c45ca64b";

const CODE_NAMES: [&str; 44] = [
    "too_large",
    "not_utf8",
    "byte_order_mark",
    "carriage_return",
    "no_final_newline",
    "control_char",
    "invisible_char",
    "char_not_allowed",
    "script_element",
    "style_element",
    "element_not_allowed",
    "tag_case",
    "event_handler",
    "style_attribute",
    "url_attribute",
    "attribute_not_allowed",
    "comment",
    "markup_declaration",
    "processing_instruction",
    "entity_not_allowed",
    "remote_reference",
    "css_url",
    "css_import",
    "javascript_scheme",
    "data_scheme",
    "prologue_mismatch",
    "lang_invalid",
    "title_invalid",
    "css_mismatch",
    "content_model",
    "tag_unclosed",
    "tag_misnested",
    "void_syntax",
    "attribute_syntax",
    "class_not_allowed",
    "id_invalid",
    "id_duplicate",
    "empty_element",
    "text_spacing",
    "depth_exceeded",
    "too_many_elements",
    "word_too_long",
    "trailing_content",
    "not_canonical",
];

const BODY_TAG_NAMES: [&str; 16] = [
    "header", "main", "footer", "section", "h1", "h2", "h3", "p", "address", "ul", "ol", "li",
    "strong", "em", "small", "br",
];

const CLASS_NAMES: [&str; 5] = ["lead", "muted", "center", "note", "plain"];

/// Every `POLICY_V1` field, restated as literals (design v2 §4.7, §5).
fn literal_policy() -> PolicyParams {
    use RefusalCode as C;
    PolicyParams {
        version: 1,
        max_page_bytes: 65_536,
        control_ranges: &[(0x00, 0x1F), (0x7F, 0x9F)],
        invisible_ranges: &[
            (0xAD, 0xAD), (0x34F, 0x34F), (0x61C, 0x61C), (0x115F, 0x1160), (0x180E, 0x180E),
            (0x200B, 0x200F), (0x2028, 0x202E), (0x2060, 0x206F), (0x3164, 0x3164),
            (0xFE00, 0xFE0F), (0xFEFF, 0xFEFF), (0xFFA0, 0xFFA0), (0xFFF9, 0xFFFB),
            (0xE0000, 0xE007F), (0xE0100, 0xE01EF),
        ],
        profile_ranges: &[
            (0x20, 0x7E), (0xA1, 0xAC), (0xAE, 0xFF), (0x100, 0x17F), (0x2012, 0x2014),
            (0x2018, 0x201F), (0x2022, 0x2022), (0x2026, 0x2026), (0x20AC, 0x20AC),
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
            Tag::Header, Tag::Main, Tag::Footer, Tag::Section, Tag::H1, Tag::H2, Tag::H3,
            Tag::P, Tag::Address, Tag::Ul, Tag::Ol, Tag::Li, Tag::Strong, Tag::Em, Tag::Small,
            Tag::Br,
        ],
        event_prefix: "on",
        style_attribute: "style",
        url_attributes: &[
            "href", "src", "srcset", "action", "formaction", "poster", "background", "cite",
            "data", "ping", "manifest", "xlink:href",
        ],
        prologue_attributes: &["lang", "charset", "name", "content"],
        body_attributes: ["class", "id"],
        declarations: &[
            ("<!--", C::Comment),
            ("<!", C::MarkupDeclaration),
            ("<?", C::ProcessingInstruction),
        ],
        entities: &[
            ('&', "&amp;"),
            ('<', "&lt;"),
            ('>', "&gt;"),
            ('"', "&quot;"),
            ('\'', "&#39;"),
        ],
        needles: &[
            ("://", C::RemoteReference),
            ("url(", C::CssUrl),
            ("@import", C::CssImport),
            ("javascript:", C::JavascriptScheme),
            ("data:", C::DataScheme),
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
        classes: &[Class::Lead, Class::Muted, Class::Center, Class::Note, Class::Plain],
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
        phone_chars: &['0', '1', '2', '3', '4', '5', '6', '7', '8', '9', ' ', '-', '.', '(', ')', '+'],
        phone_shape: "phone-shape:v1:chars=phone_chars;plus=single-leading;start=+|(|digit;\
            end=digit;non-digit-run<=phone_max_non_digit_run;\
            digits=phone_min_digits..phone_max_digits;digits=ascii",
        phone_max_non_digit_run: 2,
        fc4_threshold: 7,
        fc4_separators: &[' ', '-', '.', '(', ')', '/', '+', '\u{2012}', '\u{2013}', '\u{2014}'],
        label_passed: concat!("inert page check: ", "passed (not rendered)"),
        label_refused_prefix: "inert page check: refused",
        facts_line_template: "facts present as given: name, hours ({n} lines), phone",
    }
}

/// The documented encoding (policy.rs), restated: `put(b)` is a u64 BE
/// length then `b`; each field is `put(name)`, a kind byte, then its value.
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
            self.0.update((*c as u32).to_be_bytes());
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

/// An encoder independent of `policy.rs`, over every field (no `..`).
fn independent_hash(p: &PolicyParams) -> [u8; 32] {
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
    e.put(b"nexus.inert_page.policy.v1");
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

#[test]
fn s_o1_reference_label_css_and_policy_hash_are_pinned() {
    assert_eq!(REFERENCE.len(), 1_555);
    assert_eq!(REFERENCE.lines().count(), 54);
    assert_eq!(
        hex::encode(Sha256::digest(REFERENCE.as_bytes())),
        REFERENCE_SHA256
    );
    let page = check(REFERENCE.as_bytes()).expect("the reference page is accepted");
    assert_eq!(serialize(&page), REFERENCE.as_bytes());

    assert_eq!(
        LABEL_PASSED,
        concat!("inert page check: ", "passed (not rendered)")
    );

    assert_eq!(MOTHER_CSS.len(), 797);
    assert_eq!(MOTHER_CSS.lines().count(), 20);
    assert!(MOTHER_CSS.is_ascii());
    assert_eq!(
        hex::encode(Sha256::digest(MOTHER_CSS.as_bytes())),
        CSS_SHA256
    );
    assert_eq!(POLICY_V1.css, MOTHER_CSS);
    // The class tokens are exactly the stylesheet's class selectors.
    let mut selectors: Vec<&str> = MOTHER_CSS
        .match_indices('.')
        .map(|(i, _)| &MOTHER_CSS[i + 1..])
        .filter(|rest| rest.starts_with(|c: char| c.is_ascii_lowercase()))
        .map(|rest| &rest[..rest.find('{').unwrap()])
        .collect();
    selectors.sort_unstable();
    let mut classes = CLASS_NAMES.to_vec();
    classes.sort_unstable();
    assert_eq!(selectors, classes);

    // The closed code set and its numeric positions.
    assert_eq!(RefusalCode::CODES.len(), CODE_NAMES.len());
    for (i, code) in RefusalCode::CODES.iter().enumerate() {
        assert_eq!(code.as_str(), CODE_NAMES[i]);
        assert_eq!(usize::from(code.position()), i);
    }
    let names: Vec<&str> = POLICY_V1.body_tags.iter().map(|t| t.name()).collect();
    assert_eq!(names, BODY_TAG_NAMES);
    let names: Vec<&str> = POLICY_V1.classes.iter().map(|c| c.name()).collect();
    assert_eq!(names, CLASS_NAMES);

    let literal = literal_policy();
    assert_eq!(POLICY_V1, literal);
    assert_eq!(policy_hash(), independent_hash(&literal));
}

fn leak_str(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

fn leak<T>(v: Vec<T>) -> &'static [T] {
    Box::leak(v.into_boxed_slice())
}

/// One byte appended, and the last byte changed.
fn texts_of(s: &'static str) -> [&'static str; 2] {
    let mut changed = s.to_string();
    let last = changed.pop().unwrap();
    changed.push(if last == 'x' { 'y' } else { 'x' });
    [leak_str(format!("{s}x")), leak_str(changed)]
}

/// One element dropped, one appended, and the first replaced.
fn lists_of<T: Copy + PartialEq>(l: &'static [T], other: T) -> [&'static [T]; 3] {
    assert!(l[0] != other);
    let dropped = l[..l.len() - 1].to_vec();
    let mut appended = l.to_vec();
    appended.push(other);
    let mut replaced = l.to_vec();
    replaced[0] = other;
    [leak(dropped), leak(appended), leak(replaced)]
}

/// One bound moved by one, at each end of the list.
fn ranges_of(l: &'static [(u32, u32)]) -> [&'static [(u32, u32)]; 2] {
    let mut first = l.to_vec();
    first[0].1 += 1;
    let mut last = l.to_vec();
    last[l.len() - 1].0 += 1;
    [leak(first), leak(last)]
}

#[test]
fn s_o1b_every_policy_field_changes_the_hash() {
    use RefusalCode as C;
    let base = policy_hash();
    assert_eq!(base, hash_params(&POLICY_V1));
    let p = POLICY_V1;
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
    } = p;
    let mut variants: Vec<(&str, PolicyParams)> = Vec::new();
    macro_rules! vary {
        ($field:ident: $($value:expr),+ $(,)?) => {
            for value in [$($value),+] {
                variants.push((stringify!($field), PolicyParams { $field: value, ..p }));
            }
        };
    }
    macro_rules! vary_all {
        ($field:ident: $values:expr) => {
            for value in $values {
                variants.push((stringify!($field), PolicyParams { $field: value, ..p }));
            }
        };
    }
    vary!(version: version + 1, version - 1);
    vary!(max_page_bytes: max_page_bytes + 1, max_page_bytes - 1);
    vary_all!(control_ranges: ranges_of(control_ranges));
    vary_all!(invisible_ranges: ranges_of(invisible_ranges));
    vary_all!(profile_ranges: ranges_of(profile_ranges));
    vary_all!(controls_rule: texts_of(controls_rule));
    vary_all!(lexical_priority: texts_of(lexical_priority));
    vary_all!(lexical_scan: texts_of(lexical_scan));
    vary_all!(script_tag: texts_of(script_tag));
    vary_all!(style_tag: texts_of(style_tag));
    vary_all!(head_tags: lists_of(head_tags, "div"));
    vary_all!(body_tags: lists_of(body_tags, Tag::P));
    vary_all!(event_prefix: texts_of(event_prefix));
    vary_all!(style_attribute: texts_of(style_attribute));
    vary_all!(url_attributes: lists_of(url_attributes, "title"));
    vary_all!(prologue_attributes: lists_of(prologue_attributes, "title"));
    for (i, &attribute) in body_attributes.iter().enumerate() {
        for text in texts_of(attribute) {
            let mut changed = body_attributes;
            changed[i] = text;
            vary!(body_attributes: changed);
        }
    }
    vary_all!(declarations: lists_of(declarations, ("<!", C::Comment)));
    vary_all!(entities: lists_of(entities, ('&', "&AMP;")));
    vary_all!(needles: lists_of(needles, ("://", C::DataScheme)));
    for (i, &segment) in head_segments.iter().enumerate() {
        for text in texts_of(segment) {
            let mut changed = head_segments;
            changed[i] = text;
            vary!(head_segments: changed);
        }
    }
    vary_all!(epilogue: texts_of(epilogue));
    vary_all!(css: texts_of(css));
    vary_all!(lang_rule: texts_of(lang_rule));
    vary_all!(content_model: texts_of(content_model));
    vary_all!(classes: lists_of(classes, Class::Note));
    vary_all!(id_rule: texts_of(id_rule));
    vary!(max_depth: max_depth + 1, max_depth - 1);
    vary!(max_elements: max_elements + 1, max_elements - 1);
    vary!(max_word_chars: max_word_chars + 1, max_word_chars - 1);
    vary!(max_title_chars: max_title_chars + 1, max_title_chars - 1);
    vary!(max_id_chars: max_id_chars + 1, max_id_chars - 1);
    vary_all!(caps_rule: texts_of(caps_rule));
    vary_all!(refusal_order: texts_of(refusal_order));
    vary_all!(refusal_codes: lists_of(refusal_codes, C::NotCanonical));
    let mut swapped = refusal_codes.to_vec();
    swapped.swap(0, 1);
    vary!(refusal_codes: leak(swapped));
    vary_all!(flatten_text: texts_of(flatten_text));
    vary_all!(flatten_words: texts_of(flatten_words));
    vary_all!(spacing: texts_of(spacing));
    vary_all!(canonical: texts_of(canonical));
    vary_all!(matching: texts_of(matching));
    vary_all!(fc1: texts_of(fc1));
    vary_all!(fc2: texts_of(fc2));
    vary_all!(fc3: texts_of(fc3));
    vary_all!(fc4: texts_of(fc4));
    vary_all!(facts_construction: texts_of(facts_construction));
    vary!(facts_name_max_chars: facts_name_max_chars + 1, facts_name_max_chars - 1);
    vary!(facts_value_min_chars: facts_value_min_chars + 1, facts_value_min_chars - 1);
    vary!(facts_value_max_chars: facts_value_max_chars + 1, facts_value_max_chars - 1);
    vary!(facts_hours_min_lines: facts_hours_min_lines + 1, facts_hours_min_lines - 1);
    vary!(facts_hours_max_lines: facts_hours_max_lines + 1, facts_hours_max_lines - 1);
    vary!(phone_min_digits: phone_min_digits + 1, phone_min_digits - 1);
    vary!(phone_max_digits: phone_max_digits + 1, phone_max_digits - 1);
    vary_all!(phone_chars: lists_of(phone_chars, '#'));
    vary_all!(phone_shape: texts_of(phone_shape));
    vary!(phone_max_non_digit_run: phone_max_non_digit_run + 1, phone_max_non_digit_run - 1);
    vary!(fc4_threshold: fc4_threshold + 1, fc4_threshold - 1);
    vary_all!(fc4_separators: lists_of(fc4_separators, ':'));
    vary_all!(label_passed: texts_of(label_passed));
    vary_all!(label_refused_prefix: texts_of(label_refused_prefix));
    vary_all!(facts_line_template: texts_of(facts_line_template));

    for (field, changed) in &variants {
        assert_ne!(changed, &POLICY_V1, "{field}: the variant must differ");
        assert_ne!(hash_params(changed), base, "{field}: the hash must change");
    }
}
