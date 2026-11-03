// The document escaping in this file follows system-one-adapter-python
// v0.2.1 (commit e1d4cc93, https://github.com/typesafe-ai/system-one-adapter-python),
// ported to Rust and changed for Mother (literal six-character escapes
// inside string literals, td-escape-v1). No adapter code is vendored.
//
// MIT License
//
// Copyright (c) 2026 TypeSafe AI
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

//! Projections (§4.1), the character profile `td-chars-v1`, the inventory
//! matcher, the encoding `td-escape-v1` and the frame `td-frame-v1` (§5.3).

use std::fmt;

use crate::registry::ProjectionId;

/// The owner's approved facts, from the admission's snapshot. `Debug`
/// shows sizes only.
pub struct Facts {
    name: String,
    hours: Vec<String>,
    phone: String,
}

impl Facts {
    pub fn new(name: String, hours: Vec<String>, phone: String) -> Self {
        Self { name, hours, phone }
    }
}

/// A leaf block of S1a's typed tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Element {
    H1,
    H2,
    H3,
    P,
    Li,
    Address,
}

impl Element {
    pub const fn code(self) -> &'static str {
        match self {
            Self::H1 => "h1",
            Self::H2 => "h2",
            Self::H3 => "h3",
            Self::P => "p",
            Self::Li => "li",
            Self::Address => "address",
        }
    }
}

/// `page-text-v1`: the facts, the title and every leaf block in order.
/// `Debug` shows sizes only.
pub struct PageTextV1 {
    facts: Facts,
    title: String,
    text: Vec<(Element, String)>,
}

impl PageTextV1 {
    pub fn new(facts: Facts, title: String, text: Vec<(Element, String)>) -> Self {
        Self { facts, title, text }
    }
}

/// Where an untrusted text came from (a closed code).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoteSource {
    OwnerNote,
}

impl NoteSource {
    pub const fn code(self) -> &'static str {
        match self {
            Self::OwnerNote => "owner_note",
        }
    }
}

/// `untrusted-text-v1`: one untrusted text per request. `Debug` shows
/// sizes only.
pub struct UntrustedTextV1 {
    source: NoteSource,
    text: String,
}

impl UntrustedTextV1 {
    pub fn new(source: NoteSource, text: String) -> Self {
        Self { source, text }
    }
}

impl fmt::Debug for Facts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Facts")
            .field("name_bytes", &self.name.len())
            .field("hours_lines", &self.hours.len())
            .field(
                "hours_bytes",
                &self.hours.iter().map(String::len).sum::<usize>(),
            )
            .field("phone_bytes", &self.phone.len())
            .finish()
    }
}

impl fmt::Debug for PageTextV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PageTextV1")
            .field("facts", &self.facts)
            .field("title_bytes", &self.title.len())
            .field("blocks", &self.text.len())
            .field(
                "text_bytes",
                &self.text.iter().map(|(_, t)| t.len()).sum::<usize>(),
            )
            .finish()
    }
}

impl fmt::Debug for UntrustedTextV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UntrustedTextV1")
            .field("source", &self.source.code())
            .field("text_bytes", &self.text.len())
            .finish()
    }
}

/// A complete projection, built by the backend. Never truncated.
#[derive(Debug)]
pub enum Projection {
    PageText(PageTextV1),
    UntrustedText(UntrustedTextV1),
}

impl Projection {
    pub const fn id(&self) -> ProjectionId {
        match self {
            Self::PageText(_) => ProjectionId::PageTextV1,
            Self::UntrustedText(_) => ProjectionId::UntrustedTextV1,
        }
    }

    /// Every admitted string, for the profile and the inventory check. The
    /// destructuring is exhaustive, so a new string field must be listed.
    pub(crate) fn strings(&self) -> Vec<&str> {
        match self {
            Self::PageText(PageTextV1 {
                facts: Facts { name, hours, phone },
                title,
                text,
            }) => {
                let mut all = vec![name.as_str(), phone.as_str(), title.as_str()];
                all.extend(hours.iter().map(String::as_str));
                all.extend(text.iter().map(|(_, t)| t.as_str()));
                all
            }
            Self::UntrustedText(UntrustedTextV1 { source: _, text }) => vec![text.as_str()],
        }
    }

    /// Only an untrusted text may hold newline and tab.
    pub(crate) fn allows_line_breaks(&self) -> bool {
        matches!(self, Self::UntrustedText(_))
    }

    /// The bytes of the visible text: the title and the blocks, or the
    /// untrusted text.
    pub(crate) fn source_text_bytes(&self) -> usize {
        match self {
            Self::PageText(p) => p.title.len() + p.text.iter().map(|(_, t)| t.len()).sum::<usize>(),
            Self::UntrustedText(u) => u.text.len(),
        }
    }

    /// The canonical JSON (§4.1: compact, keys in the order shown) with
    /// `td-escape-v1` applied inside every string literal.
    pub(crate) fn encode(&self) -> String {
        let mut out = String::from(r#"{"projection":"#);
        put_str(&mut out, self.id().id());
        match self {
            Self::PageText(p) => {
                out.push_str(r#","complete":true,"facts":{"name":"#);
                put_str(&mut out, &p.facts.name);
                out.push_str(r#","hours":["#);
                for (i, line) in p.facts.hours.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    put_str(&mut out, line);
                }
                out.push_str(r#"],"phone":"#);
                put_str(&mut out, &p.facts.phone);
                out.push_str(r#"},"title":"#);
                put_str(&mut out, &p.title);
                out.push_str(r#","text":["#);
                for (i, (element, text)) in p.text.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    out.push('[');
                    put_str(&mut out, element.code());
                    out.push(',');
                    put_str(&mut out, text);
                    out.push(']');
                }
                out.push_str("]}");
            }
            Self::UntrustedText(u) => {
                out.push_str(r#","source":"#);
                put_str(&mut out, u.source.code());
                out.push_str(r#","complete":true,"text":"#);
                put_str(&mut out, &u.text);
                out.push('}');
            }
        }
        out
    }
}

const BACKSLASH: char = '\\';

/// One JSON string literal: serde_json's escaping, then `td-escape-v1`:
/// every `<` and `>` becomes the six ASCII characters backslash, `u`,
/// `0`, `0`, `3`, and `c` or `e`. They cannot occur outside strings in
/// this JSON, so the replacement is total and decoding is unchanged.
fn put_str(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' | '\\' => {
                out.push(BACKSLASH);
                out.push(c);
            }
            '<' => {
                out.push(BACKSLASH);
                out.push_str("u003c");
            }
            '>' => {
                out.push(BACKSLASH);
                out.push_str("u003e");
            }
            c if u32::from(c) < 0x20 => {
                out.push(BACKSLASH);
                match u32::from(c) {
                    0x08 => out.push('b'),
                    0x09 => out.push('t'),
                    0x0a => out.push('n'),
                    0x0c => out.push('f'),
                    0x0d => out.push('r'),
                    n => out.push_str(&format!("u{n:04x}")),
                }
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// `td-chars-v1`: S1a's Latin profile; newline and tab only in notes.
pub(crate) fn in_profile(c: char, line_breaks: bool) -> bool {
    matches!(
        u32::from(c),
        0x20..=0x7e
            | 0xa1..=0xac
            | 0xae..=0xff
            | 0x100..=0x17f
            | 0x2012..=0x2014
            | 0x2018..=0x201f
            | 0x2022
            | 0x2026
            | 0x20ac
            | 0x2122
    ) || (line_breaks && matches!(c, '\n' | '\t'))
}

/// `td-frame-v1`.
pub(crate) fn frame(encoded: &str) -> String {
    format!("<document>\n{encoded}\n</document>")
}

/// A renderer's sensitive spellings, matched as exact, case-sensitive
/// substrings. T2a supplies gemma4's; T1a's tests use a test inventory.
#[derive(Clone, Copy, Debug)]
pub struct Inventory {
    spellings: &'static [&'static str],
}

impl Inventory {
    /// An inventory of code-owned spellings: at least one, none empty.
    pub const fn new(spellings: &'static [&'static str]) -> Self {
        assert!(!spellings.is_empty(), "empty inventory");
        let mut i = 0;
        while i < spellings.len() {
            assert!(!spellings[i].is_empty(), "empty spelling");
            i += 1;
        }
        Self { spellings }
    }

    /// Whether any spelling occurs in `text`.
    pub fn occurs_in(&self, text: &str) -> bool {
        self.spellings
            .iter()
            .any(|spelling| text.contains(spelling))
    }
}
