//! S1 (bytes and characters) and S2 (lexical findings) over the whole input.
//!
//! S2 sees tags the way an HTML tokenizer in its data state does, from every
//! `<` followed by a letter (or `</` and a letter): the tag name, then the
//! attribute names, through the closing `>`. Declarations, entities and
//! needles are looked for at every offset, inside tags or not. Every rule
//! reads its parameters from `POLICY_V1`.

use super::policy::POLICY_V1;
use super::{LexicalFindings, RefusalCode as C};

pub(super) fn scan(input: &[u8]) -> LexicalFindings {
    let mut f = LexicalFindings::default();
    if input.len() > POLICY_V1.max_page_bytes {
        f.note(C::TooLarge, POLICY_V1.max_page_bytes);
        return f;
    }
    characters(input, &mut f);
    if input.last() != Some(&b'\n') {
        f.note(C::NoFinalNewline, input.len());
    }
    tags(input, &mut f);
    for at in 0..input.len() {
        anywhere(input, at, &mut f);
    }
    f
}

fn in_ranges(ranges: &[(u32, u32)], c: char) -> bool {
    let v = u32::from(c);
    ranges.iter().any(|&(lo, hi)| (lo..=hi).contains(&v))
}

/// S1: every character is the separator, or in the profile and neither a
/// control nor invisible; anything that is not UTF-8 is `not_utf8`.
fn characters(input: &[u8], f: &mut LexicalFindings) {
    let p = &POLICY_V1;
    let mut base = 0;
    for chunk in input.utf8_chunks() {
        for (k, c) in chunk.valid().char_indices() {
            let at = base + k;
            let code = match c {
                '\n' => continue,
                '\r' => C::CarriageReturn,
                '\u{FEFF}' if at == 0 => C::ByteOrderMark,
                _ if in_ranges(p.control_ranges, c) => C::ControlChar,
                _ if in_ranges(p.invisible_ranges, c) => C::InvisibleChar,
                _ if !in_ranges(p.profile_ranges, c) => C::CharNotAllowed,
                _ => continue,
            };
            f.note(code, at);
        }
        base += chunk.valid().len();
        if !chunk.invalid().is_empty() {
            f.note(C::NotUtf8, base);
            base += chunk.invalid().len();
        }
    }
}

/// HTML's tag whitespace (CR included: browsers read it as a line feed).
fn is_space(b: u8) -> bool {
    matches!(b, b'\t' | b'\n' | 0x0C | b'\r' | b' ')
}

fn allowed_tag(lower: &[u8]) -> bool {
    POLICY_V1
        .body_tags
        .iter()
        .map(|t| t.name())
        .chain(POLICY_V1.head_tags.iter().copied())
        .any(|name| name.as_bytes() == lower)
}

/// S2, tags and attributes.
fn tags(s: &[u8], f: &mut LexicalFindings) {
    let p = &POLICY_V1;
    let mut styles = 0usize;
    let mut i = 0;
    while i < s.len() {
        if s[i] != b'<' {
            i += 1;
            continue;
        }
        let (end_tag, name_at) = match s.get(i + 1) {
            Some(c) if c.is_ascii_alphabetic() => (false, i + 1),
            Some(b'/') if s.get(i + 2).is_some_and(u8::is_ascii_alphabetic) => (true, i + 2),
            _ => {
                i += 1;
                continue;
            }
        };
        let mut j = name_at;
        while j < s.len() && !is_space(s[j]) && s[j] != b'/' && s[j] != b'>' {
            j += 1;
        }
        let raw = &s[name_at..j];
        let lower = raw.to_ascii_lowercase();
        if lower == p.script_tag.as_bytes() {
            f.note(C::ScriptElement, i);
        } else if !allowed_tag(&lower) {
            f.note(C::ElementNotAllowed, i);
        } else {
            if !end_tag && lower == p.style_tag.as_bytes() {
                styles += 1;
                if styles > 1 {
                    f.note(C::StyleElement, i);
                }
            }
            if raw != lower.as_slice() {
                f.note(C::TagCase, i);
            }
        }
        i = attributes(s, j, f);
    }
}

/// The attributes of a tag whose name ends at `j`, through its `>` (HTML's
/// attribute states); returns the offset after the tag.
fn attributes(s: &[u8], mut j: usize, f: &mut LexicalFindings) -> usize {
    let n = s.len();
    loop {
        while j < n && (is_space(s[j]) || s[j] == b'/') {
            j += 1;
        }
        if j >= n {
            return n;
        }
        if s[j] == b'>' {
            return j + 1;
        }
        let at = j;
        // The first character belongs to the name even if it is `=`.
        j += 1;
        while j < n && !is_space(s[j]) && !matches!(s[j], b'/' | b'>' | b'=') {
            j += 1;
        }
        attribute_name(&s[at..j], at, f);
        while j < n && is_space(s[j]) {
            j += 1;
        }
        if j < n && s[j] == b'=' {
            j += 1;
            while j < n && is_space(s[j]) {
                j += 1;
            }
            match s.get(j) {
                Some(&quote @ (b'"' | b'\'')) => {
                    j += 1;
                    while j < n && s[j] != quote {
                        j += 1;
                    }
                    j = (j + 1).min(n);
                }
                Some(b'>') => return j + 1,
                Some(_) => {
                    while j < n && !is_space(s[j]) && s[j] != b'>' {
                        j += 1;
                    }
                }
                None => return n,
            }
        }
    }
}

fn attribute_name(raw: &[u8], at: usize, f: &mut LexicalFindings) {
    let p = &POLICY_V1;
    let lower = raw.to_ascii_lowercase();
    if lower.starts_with(p.event_prefix.as_bytes()) {
        f.note(C::EventHandler, at);
    } else if lower == p.style_attribute.as_bytes() {
        f.note(C::StyleAttribute, at);
    } else if p.url_attributes.iter().any(|u| lower == u.as_bytes()) {
        f.note(C::UrlAttribute, at);
    } else if !p
        .body_attributes
        .iter()
        .chain(p.prologue_attributes)
        .any(|a| raw == a.as_bytes())
    {
        f.note(C::AttributeNotAllowed, at);
    }
}

/// S2 at one offset: declarations, entities and needles.
fn anywhere(s: &[u8], at: usize, f: &mut LexicalFindings) {
    let p = &POLICY_V1;
    let rest = &s[at..];
    if let Some(&(_, code)) = p
        .declarations
        .iter()
        .find(|(marker, _)| rest.starts_with(marker.as_bytes()))
    {
        // The doctype is the one declaration allowed, at offset 0.
        if !(code == C::MarkupDeclaration && at == 0) {
            f.note(code, at);
        }
    }
    if rest[0] == b'&'
        && !p
            .entities
            .iter()
            .any(|(_, e)| rest.starts_with(e.as_bytes()))
    {
        f.note(C::EntityNotAllowed, at);
    }
    for &(needle, code) in p.needles {
        if rest.len() >= needle.len()
            && rest[..needle.len()].eq_ignore_ascii_case(needle.as_bytes())
        {
            f.note(code, at);
        }
    }
}
