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

use crate::registry::ProjectionId;

/// The owner's approved facts, from the admission's snapshot.
#[derive(Debug)]
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
#[derive(Debug)]
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

/// `untrusted-text-v1`: one untrusted text per request.
#[derive(Debug)]
pub struct UntrustedTextV1 {
    source: NoteSource,
    text: String,
}

impl UntrustedTextV1 {
    pub fn new(source: NoteSource, text: String) -> Self {
        Self { source, text }
    }
}

/// A complete projection, built by the backend.
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

    /// Every admitted string, for the profile and the inventory check.
    pub(crate) fn strings(&self) -> Vec<&str> {
        // STUB (oracle commit).
        Vec::new()
    }

    /// Only an untrusted text may hold newline and tab.
    pub(crate) fn allows_line_breaks(&self) -> bool {
        matches!(self, Self::UntrustedText(_))
    }

    /// The bytes of the visible text: the title and the blocks, or the
    /// untrusted text.
    pub(crate) fn source_text_bytes(&self) -> usize {
        // STUB (oracle commit).
        0
    }

    /// The canonical JSON with `td-escape-v1` applied.
    pub(crate) fn encode(&self) -> String {
        // STUB (oracle commit).
        let _ = (&ProjectionId::PageTextV1, self.allows_line_breaks());
        String::new()
    }
}

/// `td-chars-v1`: S1a's Latin profile; newline and tab only in notes.
pub(crate) fn in_profile(c: char, line_breaks: bool) -> bool {
    // STUB (oracle commit).
    let _ = (c, line_breaks);
    true
}

/// `td-frame-v1`.
pub(crate) fn frame(encoded: &str) -> String {
    // STUB (oracle commit).
    let _ = encoded;
    String::new()
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

    pub const fn len(&self) -> usize {
        self.spellings.len()
    }

    pub const fn is_empty(&self) -> bool {
        self.spellings.is_empty()
    }

    /// Whether any spelling occurs in `text`.
    pub fn occurs_in(&self, text: &str) -> bool {
        // STUB (oracle commit).
        let _ = text;
        false
    }
}
