//! The workspace's production source, as the N5 guards read it.
//!
//! - Every `.rs` file of the workspace (build output, dependencies, VCS
//!   metadata and `docs/` excluded).
//! - Whole-file exclusions are the INTERSECTION of the fe5328af exclusion
//!   predicate and the new proof of exclusively test-only reachability. Every
//!   independent Cargo production root stays scanned. Coverage can only grow
//!   from fe5328af; shared, uncertain or unresolved reachability stays scanned.
//! - Inside a scanned file, only items proved to require `test` are blanked.
//!   Production-enabled cfg predicates never hide their items.
//! - Two views of a production file: `code` (comments and the contents of
//!   string and char literals blanked) and `text` (comments blanked,
//!   literals kept). Both keep every byte offset and line.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

#[cfg(test)]
#[path = "source_cfg_tests.rs"]
mod cfg_tests;

#[cfg(test)]
#[path = "source_literals_tests.rs"]
mod source_literals_tests;

#[cfg(test)]
#[path = "source_reachability_tests.rs"]
mod source_reachability_tests;

// Preserve the complete historical scanner, including unused view helpers.
#[allow(dead_code)]
#[path = "source_fe5328af.rs"]
mod fe5328af;

pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

/// One production file: its path relative to the workspace root and its
/// two views.
pub struct ProdFile {
    pub rel: String,
    pub code: String,
    pub text: String,
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        let Ok(ft) = entry.file_type() else { continue };
        if ft.is_dir() {
            if name.starts_with("target")
                || matches!(name.as_str(), "node_modules" | ".git" | "dist")
            {
                continue;
            }
            walk(&path, out);
        } else if name.ends_with(".rs") {
            out.push(path);
        }
    }
}

/// A test file by its path alone.
pub fn is_test_path(rel: &str) -> bool {
    rel.starts_with("tests/")
        || rel.contains("/tests/")
        || rel.contains("/benches/")
        || rel.contains("/examples/")
        || rel.ends_with("_tests.rs")
        || rel.ends_with("/tests.rs")
        || rel == "build.rs"
        || rel.ends_with("/build.rs")
}

/// A classified literal's content and end offsets. Prefixes and delimiters
/// remain visible, while only the content is masked in the code view.
struct Literal {
    body: usize,
    body_end: usize,
    end: usize,
}

fn string_literal(text: &str, at: usize, prefix: &str, quote: usize, hashes: usize) -> Literal {
    let b = text.as_bytes();
    let body = quote + 1;
    let mut end = body;
    let raw = matches!(prefix, "r" | "br" | "cr");
    while end < b.len() {
        if b[end] == b'"'
            && (!raw
                || b.get(end + 1..end + 1 + hashes)
                    .is_some_and(|s| s.iter().all(|&c| c == b'#')))
        {
            return Literal {
                body,
                body_end: end,
                end: end + 1 + hashes,
            };
        }
        if !raw && b[end] == b'\\' {
            assert!(
                end + 1 < b.len(),
                "source scan: unterminated string escape at byte {at}"
            );
            end += 2;
        } else {
            end += 1;
        }
    }
    panic!("source scan: unterminated string literal at byte {at}");
}

/// `None` is a lifetime; a recognized character literal must close exactly.
fn character_literal(text: &str, at: usize, quote: usize, byte: bool) -> Option<Literal> {
    let b = text.as_bytes();
    let body = quote + 1;
    let first = *b
        .get(body)
        .unwrap_or_else(|| panic!("source scan: incomplete character at byte {at}"));
    let mut end = body;
    if first == b'\\' {
        end += 1;
        match b.get(end) {
            Some(b'u') if !byte => {
                end += 1;
                assert_eq!(
                    b.get(end),
                    Some(&b'{'),
                    "source scan: unclassified Unicode character escape at byte {at}"
                );
                end += 1;
                let digits = end;
                while b
                    .get(end)
                    .is_some_and(|c| c.is_ascii_hexdigit() || *c == b'_')
                {
                    end += 1;
                }
                assert!(
                    end > digits && b.get(end) == Some(&b'}'),
                    "source scan: unterminated Unicode character escape at byte {at}"
                );
                end += 1;
            }
            Some(b'x') => {
                end += 1;
                assert!(
                    b.get(end..end + 2)
                        .is_some_and(|s| s.iter().all(u8::is_ascii_hexdigit)),
                    "source scan: unclassified hex character escape at byte {at}"
                );
                end += 2;
            }
            Some(b'n' | b'r' | b't' | b'\\' | b'0' | b'\'' | b'"') => end += 1,
            _ => panic!("source scan: unclassified character escape at byte {at}"),
        }
    } else {
        let c = text[body..].chars().next().expect("character content");
        end += c.len_utf8();
        if b.get(end) != Some(&b'\'') && !byte && (c.is_alphanumeric() || c == '_') {
            // Identifier-shaped tokens beginning with an apostrophe are
            // lifetimes. A later apostrophe in the same identifier is an
            // invalid character literal, not permission to skip the token.
            let tail = text[end..]
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .map(char::len_utf8)
                .sum::<usize>();
            assert_ne!(
                b.get(end + tail),
                Some(&b'\''),
                "source scan: unclassified character literal at byte {at}"
            );
            return None;
        }
    }
    assert_eq!(
        b.get(end),
        Some(&b'\''),
        "source scan: unterminated character literal at byte {at}"
    );
    Some(Literal {
        body,
        body_end: end,
        end: end + 1,
    })
}

/// `text` with comments blanked and, when `literals` is false, the contents
/// of every Rust string/character literal blanked too. Newlines are kept.
/// Unknown prefixes and unterminated tokens abort the scan: they must never
/// turn later production code into silently masked literal content.
pub fn mask(text: &str, literals: bool) -> String {
    let b = text.as_bytes();
    let mut out = b.to_vec();
    let blank = |out: &mut Vec<u8>, from: usize, to: usize| {
        for byte in &mut out[from..to] {
            if *byte != b'\n' {
                *byte = b' ';
            }
        }
    };
    let mut i = 0;
    while i < b.len() {
        if b[i..].starts_with(b"//") {
            let end = b[i..]
                .iter()
                .position(|&c| c == b'\n')
                .map_or(b.len(), |p| i + p);
            blank(&mut out, i, end);
            i = end;
            continue;
        }
        if b[i..].starts_with(b"/*") {
            let (mut depth, mut end) = (1, i + 2);
            while end < b.len() && depth > 0 {
                if b[end..].starts_with(b"/*") {
                    depth += 1;
                    end += 2;
                } else if b[end..].starts_with(b"*/") {
                    depth -= 1;
                    end += 2;
                } else {
                    end += 1;
                }
            }
            assert_eq!(
                depth, 0,
                "source scan: unterminated block comment at byte {i}"
            );
            blank(&mut out, i, end);
            i = end;
            continue;
        }
        let literal;
        if b[i] == b'"' {
            literal = Some(string_literal(text, i, "", i, 0));
        } else if b[i] == b'\'' {
            literal = character_literal(text, i, i, false);
        } else {
            let c = text[i..].chars().next().expect("source character");
            if c.is_alphabetic() || c == '_' {
                let len = text[i..]
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .map(char::len_utf8)
                    .sum::<usize>();
                let end = i + len;
                let prefix = &text[i..end];
                let mut quote = end;
                while b.get(quote) == Some(&b'#') {
                    quote += 1;
                }
                let hashes = quote - end;
                if b.get(quote) == Some(&b'"') {
                    assert!(
                        matches!(prefix, "r" | "br" | "cr")
                            || (hashes == 0 && matches!(prefix, "b" | "c")),
                        "source scan: unclassified string prefix at byte {i}"
                    );
                    literal = Some(string_literal(text, i, prefix, quote, hashes));
                } else if b.get(end) == Some(&b'\'') {
                    assert_eq!(
                        prefix, "b",
                        "source scan: unclassified character prefix at byte {i}"
                    );
                    literal = character_literal(text, i, end, true);
                } else {
                    i = end;
                    continue;
                }
            } else {
                i += c.len_utf8();
                continue;
            }
        }
        if let Some(literal) = literal {
            if !literals {
                blank(&mut out, literal.body, literal.body_end);
            }
            i = literal.end;
        } else {
            i += 1; // A lifetime's apostrophe.
        }
    }
    String::from_utf8(out).expect("source masking preserves UTF-8")
}

/// The end (exclusive) of the bracket group opening at `open` in `code`.
pub fn close_of(code: &str, open: usize) -> usize {
    let b = code.as_bytes();
    let mut depth = 0i64;
    for (k, &c) in b.iter().enumerate().skip(open) {
        match c {
            b'{' | b'(' | b'[' => depth += 1,
            b'}' | b')' | b']' => {
                depth -= 1;
                if depth == 0 {
                    return k + 1;
                }
            }
            _ => {}
        }
    }
    b.len()
}

/// The `[` of an outer attribute; Rust permits whitespace after `#`.
fn outer_attribute_open(code: &str, at: usize) -> Option<usize> {
    let b = code.as_bytes();
    if b.get(at) != Some(&b'#') {
        return None;
    }
    let mut open = at + 1;
    while b.get(open).is_some_and(u8::is_ascii_whitespace) {
        open += 1;
    }
    (b.get(open) == Some(&b'[')).then_some(open)
}

/// Skips whitespace and outer attributes from `i`.
fn skip_attributes(code: &str, mut i: usize) -> usize {
    let b = code.as_bytes();
    loop {
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if let Some(open) = outer_attribute_open(code, i) {
            i = close_of(code, open);
            continue;
        }
        return i;
    }
}

/// Boolean cfg syntax. Other options are independent atoms: ignoring target
/// relationships over-approximates production and cannot hide a live item.
#[derive(Clone)]
enum Cfg {
    Atom(String),
    All(Vec<Cfg>),
    Any(Vec<Cfg>),
    Not(Box<Cfg>),
}

impl Cfg {
    fn atoms(&self, out: &mut BTreeSet<String>) {
        match self {
            Self::Atom(name) if name != "test" => {
                out.insert(name.clone());
            }
            Self::All(children) | Self::Any(children) => {
                for child in children {
                    child.atoms(out);
                }
            }
            Self::Not(child) => child.atoms(out),
            Self::Atom(_) => {}
        }
    }

    fn eval(&self, values: &BTreeMap<&str, bool>) -> bool {
        match self {
            Self::Atom(name) => name != "test" && values[name.as_str()],
            Self::All(children) => children.iter().all(|child| child.eval(values)),
            Self::Any(children) => children.iter().any(|child| child.eval(values)),
            Self::Not(child) => !child.eval(values),
        }
    }

    fn requires_test(&self) -> bool {
        let mut atoms = BTreeSet::new();
        self.atoms(&mut atoms);
        // Bound the proof cost. Beyond the bound, uncertainty is production.
        if atoms.len() > 16 {
            return false;
        }
        let names: Vec<_> = atoms.iter().map(String::as_str).collect();
        for valuation in 0..(1usize << names.len()) {
            let values = names
                .iter()
                .enumerate()
                .map(|(i, &name)| (name, valuation & (1 << i) != 0))
                .collect();
            if self.eval(&values) {
                return false;
            }
        }
        true
    }
}

struct CfgParser<'a> {
    src: &'a str,
    at: usize,
}

impl<'a> CfgParser<'a> {
    fn ws(&mut self) {
        while self
            .src
            .as_bytes()
            .get(self.at)
            .is_some_and(u8::is_ascii_whitespace)
        {
            self.at += 1;
        }
    }

    fn eat(&mut self, token: &str) -> bool {
        self.ws();
        if self.src[self.at..].starts_with(token) {
            self.at += token.len();
            true
        } else {
            false
        }
    }

    fn ident(&mut self) -> Option<String> {
        self.ws();
        let start = self.at;
        if self.src[self.at..].starts_with("r#") {
            self.at += 2;
        }
        let name = self.at;
        let first = self.src[self.at..].chars().next()?;
        if !(first.is_alphabetic() || first == '_') {
            self.at = start;
            return None;
        }
        self.at += self.src[self.at..]
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .map(char::len_utf8)
            .sum::<usize>();
        Some(self.src[name..self.at].to_string())
    }

    fn string(&mut self) -> Option<&'a str> {
        self.ws();
        let start = self.at;
        let b = self.src.as_bytes();
        let (prefix, mut quote) = if b.get(start) == Some(&b'"') {
            ("", start)
        } else if b.get(start) == Some(&b'r') {
            ("r", start + 1)
        } else {
            return None;
        };
        let marks = quote;
        while b.get(quote) == Some(&b'#') {
            quote += 1;
        }
        if b.get(quote) != Some(&b'"') {
            return None;
        }
        self.at = string_literal(self.src, start, prefix, quote, quote - marks).end;
        Some(&self.src[start..self.at])
    }

    fn predicate(&mut self, depth: usize) -> Option<Cfg> {
        if depth > 64 {
            return None;
        }
        let name = self.ident()?;
        if self.eat("(") {
            if !matches!(name.as_str(), "all" | "any" | "not") {
                return None;
            }
            let mut children = Vec::new();
            if !self.eat(")") {
                loop {
                    children.push(self.predicate(depth + 1)?);
                    if self.eat(")") {
                        break;
                    }
                    if !self.eat(",") {
                        return None;
                    }
                    if self.eat(")") {
                        break;
                    }
                }
            }
            return match name.as_str() {
                "all" => Some(Cfg::All(children)),
                "any" => Some(Cfg::Any(children)),
                "not" if children.len() == 1 => Some(Cfg::Not(Box::new(children.remove(0)))),
                _ => None,
            };
        }
        if self.eat("=") {
            return Some(Cfg::Atom(format!("{name}={}", self.string()?)));
        }
        Some(Cfg::Atom(name))
    }

    /// Skip an ordinary non-gating attribute's token tree, already checked
    /// by literal masking. A mismatch stays uncertain rather than excluding.
    fn group(&mut self) -> Option<()> {
        self.ws();
        let code = mask(&self.src[self.at..], false);
        let mut stack = Vec::new();
        for (offset, byte) in code.bytes().enumerate() {
            match byte {
                b'(' => stack.push(b')'),
                b'[' => stack.push(b']'),
                b'{' => stack.push(b'}'),
                b')' | b']' | b'}' => {
                    if stack.pop()? != byte {
                        return None;
                    }
                    if stack.is_empty() {
                        self.at += offset + 1;
                        return Some(());
                    }
                }
                _ => {}
            }
            if stack.len() > 64 {
                return None;
            }
        }
        None
    }

    fn attribute(&mut self, depth: usize) -> Option<Cfg> {
        if depth > 64 {
            return None;
        }
        match self.ident()?.as_str() {
            "cfg" => {
                if !self.eat("(") {
                    return None;
                }
                let predicate = self.predicate(depth + 1)?;
                if !self.eat(")") {
                    return None;
                }
                Some(predicate)
            }
            "cfg_attr" => {
                if !self.eat("(") {
                    return None;
                }
                let condition = self.predicate(depth + 1)?;
                if !self.eat(",") {
                    return None;
                }
                let mut effects = vec![self.attribute(depth + 1)?];
                while !self.eat(")") {
                    if !self.eat(",") {
                        return None;
                    }
                    if self.eat(")") {
                        break;
                    }
                    effects.push(self.attribute(depth + 1)?);
                }
                // If the condition holds, all nested cfg effects apply.
                Some(Cfg::Any(vec![
                    Cfg::Not(Box::new(condition)),
                    Cfg::All(effects),
                ]))
            }
            _ => {
                self.ws();
                if self.src.as_bytes().get(self.at) == Some(&b'(') {
                    self.group()?;
                } else if self.eat("=") {
                    self.string()?;
                }
                Some(Cfg::All(Vec::new()))
            }
        }
    }
}

/// An outer attribute excludes source only after a bounded proof that it is
/// false for every assignment with `test=false`. Unknown syntax is production.
fn cfg_test_attribute(code: &str, text: &str, at: usize) -> Option<usize> {
    let open = outer_attribute_open(code, at)?;
    let end = close_of(code, open);
    let mut parser = CfgParser {
        src: &text[open + 1..end.checked_sub(1)?],
        at: 0,
    };
    let predicate = parser.attribute(0)?;
    parser.ws();
    (parser.at == parser.src.len() && predicate.requires_test()).then_some(end)
}

/// Byte ranges `(start, end)` of a file.
pub type Ranges = Vec<(usize, usize)>;

/// `mod NAME;` declarations, each with its `#[path]` if any.
pub type ModuleDecls = Vec<(String, Option<String>)>;

/// The byte ranges of items proved to require `test` (attribute included),
/// and the corresponding `mod NAME;` declarations with any `#[path]`.
/// Ordinary cfg atoms are free booleans, and unparseable attributes remain
/// production. `cfg_attr(C, A)` imposes `!C || A`, including nested effects.
pub fn cfg_test_items(code: &str, text: &str) -> (Ranges, ModuleDecls) {
    let mut ranges = Vec::new();
    let mut modules = Vec::new();
    let mut from = 0;
    while let Some(p) = code[from..].find('#') {
        let at = from + p;
        from = at + 1;
        let item = skip_attributes(code, at);
        if item == at {
            continue;
        }
        // Start at the FIRST attribute, so path/cfg order cannot change the
        // module's identity. Any independent test requirement is sufficient.
        let mut attr = at;
        let mut test_only = false;
        while let Some(open) = outer_attribute_open(code, attr) {
            test_only |= cfg_test_attribute(code, text, attr).is_some();
            attr = close_of(code, open);
            while code
                .as_bytes()
                .get(attr)
                .is_some_and(u8::is_ascii_whitespace)
            {
                attr += 1;
            }
        }
        if !test_only {
            from = item;
            continue;
        }
        // The gated item, field, match arm or expression: up to its block's
        // close (and a following comma), or a `;` or `,` at depth 0, or the
        // close of the enclosing group (exclusive).
        let b = code.as_bytes();
        let mut j = item;
        let mut depth = 0i64;
        let mut terminator = None;
        let end = loop {
            if j >= b.len() {
                break b.len();
            }
            match b[j] {
                b'(' | b'[' => depth += 1,
                b'{' if depth == 0 => {
                    let e = close_of(code, j);
                    let next = code[e..].trim_start();
                    break if next.starts_with(',') {
                        code.len() - next.len() + 1
                    } else {
                        e
                    };
                }
                b'{' => depth += 1,
                b')' | b']' | b'}' if depth == 0 => break j,
                b')' | b']' | b'}' => depth -= 1,
                b';' | b',' if depth == 0 => {
                    terminator = Some(b[j]);
                    break j + 1;
                }
                _ => {}
            }
            j += 1;
        };
        let j = j.min(b.len());
        let head: String = code[item..j]
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        if terminator == Some(b';') {
            let words: Vec<&str> = head.split(' ').collect();
            if let Some(m) = words.iter().position(|w| *w == "mod") {
                if let Some(name) = words.get(m + 1) {
                    let paths = module_paths(code, text, at, item);
                    let path = (!paths.uncertain && !paths.unresolved)
                        .then(|| paths.explicit.first().cloned())
                        .flatten();
                    modules.push((name.trim_end_matches(';').to_string(), path));
                }
            }
        }
        ranges.push((at, end));
        from = end;
    }
    (ranges, modules)
}

#[derive(Default)]
struct ModulePaths {
    explicit: Vec<String>,
    conditional: Vec<String>,
    uncertain: bool,
    unresolved: bool,
}

/// Decode only unambiguous path literals. Escapes, macros and unfamiliar
/// syntax cannot grant permission to exclude any file.
fn path_value(parser: &mut CfgParser<'_>) -> Option<String> {
    if !parser.eat("=") {
        return None;
    }
    let value = parser.string()?;
    let quote = value.find('"')?;
    let marks = value[..quote].bytes().filter(|&c| c == b'#').count();
    let body = &value[quote + 1..value.len() - 1 - marks];
    (!body.contains('\\')).then(|| body.to_string())
}

fn module_paths(code: &str, text: &str, mut at: usize, end: usize) -> ModulePaths {
    let mut out = ModulePaths::default();
    while at < end {
        let Some(open) = outer_attribute_open(code, at) else {
            break;
        };
        let close = close_of(code, open);
        let body = &text[open + 1..close - 1];
        let mut parser = CfgParser { src: body, at: 0 };
        match parser.ident().as_deref() {
            Some("path") => {
                if let Some(path) = path_value(&mut parser) {
                    out.explicit.push(path);
                    parser.ws();
                    out.unresolved |= parser.at != body.len();
                } else {
                    out.unresolved = true;
                }
            }
            Some("cfg_attr") => {
                // Inspect nested effects in the code view, never words inside
                // literals. Any path-changing cfg_attr is uncertain even if
                // another attribute independently proves cfg(test).
                let masked = &code[open + 1..close - 1];
                let mut tokens = CfgParser {
                    src: masked,
                    at: parser.at,
                };
                while tokens.at < masked.len() {
                    let start = tokens.at;
                    if let Some(name) = tokens.ident() {
                        if name == "path" && tokens.eat("=") {
                            out.uncertain = true;
                            let mut value = CfgParser {
                                src: body,
                                at: start,
                            };
                            value.ident();
                            if let Some(path) = path_value(&mut value) {
                                out.conditional.push(path);
                            } else {
                                out.unresolved = true;
                            }
                        }
                    } else {
                        tokens.at += masked[tokens.at..].chars().next().unwrap().len_utf8();
                    }
                }
            }
            _ => {}
        }
        at = close;
        while code.as_bytes().get(at).is_some_and(u8::is_ascii_whitespace) {
            at += 1;
        }
    }
    out.uncertain |= out.explicit.len() > 1;
    out
}

/// A module head at a token boundary, including restricted visibility and raw
/// identifiers. Return the name and the offset of its `;` or opening brace.
fn module_head(code: &str, at: usize) -> Option<(String, usize)> {
    let mut parser = CfgParser { src: code, at };
    let mut keyword = parser.ident()?;
    if keyword == "pub" {
        parser.ws();
        if code.as_bytes().get(parser.at) == Some(&b'(') {
            // This is already the code view. Re-masking it would interpret
            // blanked character contents as malformed character literals.
            parser.at = close_of(code, parser.at);
        }
        keyword = parser.ident()?;
    }
    if keyword == "unsafe" {
        keyword = parser.ident()?;
    }
    if keyword != "mod" {
        return None;
    }
    let name = parser.ident()?;
    parser.ws();
    matches!(code.as_bytes().get(parser.at), Some(b';' | b'{')).then_some((name, parser.at))
}

/// Normalize parent components even for a missing inline directory, then use
/// canonical file identity so two spellings/symlinks cannot split ownership.
fn file_identity(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            std::path::Component::CurDir => {}
            other => normalized.push(other),
        }
    }
    path.canonicalize()
        .unwrap_or_else(|_| normalized.canonicalize().unwrap_or(normalized))
}

fn default_module_dir(file: &Path) -> PathBuf {
    let parent = file.parent().expect("source parent");
    if matches!(
        file.file_name().and_then(|n| n.to_str()),
        Some("mod.rs" | "lib.rs" | "main.rs")
    ) {
        parent.to_path_buf()
    } else {
        parent.join(file.file_stem().expect("source stem"))
    }
}

/// Independent entry points cannot inherit test-only status from module
/// aliases. Keep each root's actual load directory as well as physical module
/// contexts. Unreadable/malformed Cargo metadata denies ALL file exclusions.
fn cargo_production_roots(
    root: &Path,
    raw: &BTreeMap<PathBuf, String>,
) -> Option<BTreeMap<PathBuf, BTreeSet<PathBuf>>> {
    let known: BTreeSet<_> = raw.keys().map(|p| file_identity(p)).collect();
    let mut roots: BTreeMap<PathBuf, BTreeSet<PathBuf>> = BTreeMap::new();
    let mut manifests = BTreeSet::new();
    for file in raw.keys() {
        // Preserve the previous conservative rule for standalone source trees
        // too; Cargo metadata may be absent in an isolated guard fixture.
        let rel = file
            .strip_prefix(root)
            .ok()?
            .to_string_lossy()
            .replace('\\', "/");
        if !is_test_path(&rel) && matches!(file.file_name()?.to_str(), Some("lib.rs" | "main.rs")) {
            roots
                .entry(file_identity(file))
                .or_default()
                .insert(file.parent()?.to_path_buf());
        }
        for dir in file.ancestors().skip(1).take_while(|p| p.starts_with(root)) {
            let manifest = dir.join("Cargo.toml");
            if manifest.try_exists().ok()? {
                manifests.insert(manifest);
            }
        }
    }
    for manifest in manifests {
        let data: toml::Value = toml::from_str(&std::fs::read_to_string(&manifest).ok()?).ok()?;
        let Some(package) = data.get("package") else {
            continue;
        };
        let package = package.as_table()?;
        let dir = manifest.parent()?;
        let mut paths = vec![dir.join("src/lib.rs"), dir.join("src/main.rs")];
        // Keep conventional bin candidates conservatively even if autodiscovery
        // is disabled: a named [[bin]] without a path can still select them.
        for file in raw.keys() {
            if let Ok(rel) = file.strip_prefix(dir.join("src/bin")) {
                if rel.components().count() == 1
                    || (rel.components().count() == 2
                        && rel.file_name()?.to_str() == Some("main.rs"))
                {
                    paths.push(file.clone());
                }
            }
        }
        if let Some(lib) = data.get("lib") {
            if let Some(path) = lib.as_table()?.get("path") {
                paths.push(dir.join(path.as_str()?));
            }
        }
        if let Some(bins) = data.get("bin") {
            for bin in bins.as_array()? {
                if let Some(path) = bin.as_table()?.get("path") {
                    paths.push(dir.join(path.as_str()?));
                }
            }
        }
        // The old scanner never scanned conventional build.rs. Custom build
        // paths it could scan are independent production roots too.
        if let Some(toml::Value::String(path)) = package.get("build") {
            paths.push(dir.join(path));
        }
        for path in paths {
            let identity = file_identity(&path);
            if known.contains(&identity) {
                roots
                    .entry(identity)
                    .or_default()
                    .insert(path.parent()?.to_path_buf());
            }
        }
    }
    Some(roots)
}

#[derive(Default)]
struct ModuleReach {
    targets: BTreeSet<PathBuf>,
    // Canonical identity plus the directory of the actual load spelling.
    path_loaded: BTreeSet<(PathBuf, PathBuf)>,
    test_only: bool,
    uncertain: bool,
}

struct ModuleScan<'a> {
    code: &'a str,
    text: &'a str,
    ranges: &'a Ranges,
    known: &'a BTreeSet<PathBuf>,
    declarations: BTreeMap<usize, ModuleReach>,
    unresolved: bool,
}

impl ModuleScan<'_> {
    /// Scan all token groups, preserving the directory only across inline
    /// module blocks. Function/macro blocks do not add module directories.
    fn visit(
        &mut self,
        mut at: usize,
        end: usize,
        default_dir: &Path,
        path_dir: &Path,
        uncertain: bool,
    ) {
        while at < end {
            if self.code.as_bytes()[at].is_ascii_whitespace() {
                at += 1;
                continue;
            }
            let start = at;
            let item = skip_attributes(self.code, at);
            if let Some((name, terminator)) = module_head(self.code, item) {
                let paths = module_paths(self.code, self.text, start, item);
                self.unresolved |= paths.unresolved;
                let uncertain = uncertain || paths.uncertain || paths.unresolved;
                let mut dirs: Vec<_> = paths
                    .explicit
                    .iter()
                    .chain(&paths.conditional)
                    .map(|p| path_dir.join(p))
                    .collect();
                if paths.explicit.is_empty() {
                    dirs.push(default_dir.join(&name));
                }
                if self.code.as_bytes()[terminator] == b'{' {
                    let close = close_of(self.code, terminator);
                    for dir in dirs {
                        self.visit(terminator + 1, close - 1, &dir, &dir, uncertain);
                    }
                    at = close;
                    continue;
                }
                let mut reach = ModuleReach {
                    test_only: self.ranges.iter().any(|&(a, z)| a <= item && item < z),
                    uncertain,
                    ..ModuleReach::default()
                };
                for path in paths.explicit.iter().chain(&paths.conditional) {
                    let target = path_dir.join(path);
                    let identity = file_identity(&target);
                    if self.known.contains(&identity) {
                        reach.uncertain |= !target.is_file();
                        reach.targets.insert(identity.clone());
                        reach
                            .path_loaded
                            .insert((identity, target.parent().unwrap().to_path_buf()));
                    }
                }
                if paths.explicit.is_empty() {
                    let candidates: Vec<_> = [
                        default_dir.join(format!("{name}.rs")),
                        default_dir.join(&name).join("mod.rs"),
                    ]
                    .into_iter()
                    .filter(|p| self.known.contains(&file_identity(p)))
                    .collect();
                    reach.uncertain |= candidates.len() > 1;
                    for target in candidates {
                        reach.uncertain |= !target.is_file();
                        reach.targets.insert(file_identity(&target));
                    }
                }
                // Multiple inline path contexts must agree before excluding.
                if let Some(previous) = self.declarations.get_mut(&item) {
                    previous.uncertain |= reach.uncertain || previous.targets != reach.targets;
                    previous.targets.extend(reach.targets);
                    previous.path_loaded.extend(reach.path_loaded);
                } else {
                    self.declarations.insert(item, reach);
                }
                at = terminator + 1;
            } else if item > at {
                at = item;
            } else {
                // Advance a complete identifier so `some_mod`/raw identifiers
                // cannot be mistaken for a new `mod` keyword mid-token.
                let mut token = CfgParser { src: self.code, at };
                if token.ident().is_some() {
                    at = token.at;
                } else {
                    at += self.code[at..].chars().next().unwrap().len_utf8();
                }
            }
        }
    }
}

struct Incoming {
    source: PathBuf,
    test_only: bool,
    uncertain: bool,
}

/// Least fixed point: a file becomes test-only only after EVERY incoming
/// declaration is proved test-only (locally or through its declaring file).
/// Cycles without a proof remain scanned. Path uncertainty overrides parents.
fn exclusively_test_files(
    root: &Path,
    raw: &BTreeMap<PathBuf, String>,
    ranges: &BTreeMap<PathBuf, Ranges>,
) -> BTreeSet<PathBuf> {
    let Some(production_roots) = cargo_production_roots(root, raw) else {
        return BTreeSet::new();
    };
    let known: BTreeSet<_> = raw.keys().map(|p| file_identity(p)).collect();
    let mut directories: BTreeMap<_, BTreeSet<_>> = BTreeMap::new();
    for file in raw.keys() {
        directories
            .entry(file_identity(file))
            .or_default()
            .insert(default_module_dir(file));
    }
    for (identity, dirs) in &production_roots {
        directories
            .entry(identity.clone())
            .or_default()
            .extend(dirs.iter().cloned());
    }
    let mut incoming: BTreeMap<PathBuf, Vec<Incoming>>;
    let mut unresolved;
    loop {
        incoming = BTreeMap::new();
        unresolved = false;
        let mut more = Vec::new();
        for (file, source) in raw {
            let identity = file_identity(file);
            let code = mask(source, false);
            let text = mask(source, true);
            let mut scan = ModuleScan {
                code: &code,
                text: &text,
                ranges: &ranges[file],
                known: &known,
                declarations: BTreeMap::new(),
                unresolved: false,
            };
            // A #[path]-loaded non-mod-rs file can use its containing directory
            // for default children. Keep both physical contexts conservatively;
            // disagreement is uncertain, rather than guessing one ownership.
            for dir in &directories[&identity] {
                scan.visit(0, code.len(), dir, file.parent().unwrap(), false);
            }
            unresolved |= scan.unresolved;
            for reach in scan.declarations.into_values() {
                more.extend(reach.path_loaded);
                for target in reach.targets {
                    incoming.entry(target).or_default().push(Incoming {
                        source: identity.clone(),
                        test_only: reach.test_only,
                        uncertain: reach.uncertain,
                    });
                }
            }
        }
        let mut changed = false;
        for (file, dir) in more {
            changed |= directories.entry(file).or_default().insert(dir);
        }
        if !changed {
            break;
        }
    }
    // A nonliteral path could reach ANY file. No exclusion is safe in that
    // scan; do not silently use default resolution for the failed attribute.
    if unresolved {
        return BTreeSet::new();
    }
    let mut test = BTreeSet::new();
    loop {
        let more: Vec<_> = raw
            .keys()
            .filter_map(|file| {
                let identity = file_identity(file);
                if test.contains(&identity) {
                    return None;
                }
                let rel = file
                    .strip_prefix(root)
                    .expect("workspace source")
                    .to_string_lossy()
                    .replace('\\', "/");
                // ANY independent production root overrides test aliases,
                // including explicit paths, auto bins and canonical symlinks.
                if production_roots.contains_key(&identity) {
                    return None;
                }
                let proven = match incoming.get(&identity) {
                    Some(edges) => edges.iter().all(|edge| {
                        !edge.uncertain && (edge.test_only || test.contains(&edge.source))
                    }),
                    None => is_test_path(&rel),
                };
                proven.then_some(identity)
            })
            .collect();
        if more.is_empty() {
            return test;
        }
        test.extend(more);
    }
}

/// Every production file of the workspace.
pub fn production_files() -> Vec<ProdFile> {
    let root = workspace_root();
    production_files_in(&root)
}

/// The same whole-file scan for an isolated workspace fixture.
pub fn production_files_in(root: &Path) -> Vec<ProdFile> {
    let mut paths = Vec::new();
    walk(root, &mut paths);
    paths.retain(|p| {
        !p.strip_prefix(root)
            .expect("workspace path")
            .starts_with("docs")
    });
    paths.sort();
    let mut raw: BTreeMap<PathBuf, String> = BTreeMap::new();
    for p in &paths {
        if let Ok(t) = std::fs::read_to_string(p) {
            raw.insert(p.clone(), t);
        }
    }
    let mut cfg_ranges: BTreeMap<PathBuf, Vec<(usize, usize)>> = BTreeMap::new();
    for (p, t) in &raw {
        let code = mask(t, false);
        let text = mask(t, true);
        let (ranges, _) = cfg_test_items(&code, &text);
        cfg_ranges.insert(p.clone(), ranges);
    }
    // R-4: old "new proof alone excludes" -> new "OLD predicate AND NEW
    // proof". Never canonicalize the old predicate's path spelling: a legacy
    // scanned alias must remain scanned even when another alias was excluded.
    let legacy_test = fe5328af::test_files_in(root, &raw);
    let proved_test = exclusively_test_files(root, &raw, &cfg_ranges);
    let mut out = Vec::new();
    for (p, t) in &raw {
        if legacy_test.contains(p) && proved_test.contains(&file_identity(p)) {
            continue;
        }
        let mut code = mask(t, false).into_bytes();
        let mut text = mask(t, true).into_bytes();
        for &(a, z) in cfg_ranges.get(p).into_iter().flatten() {
            for k in a..z.min(code.len()) {
                if code[k] != b'\n' {
                    code[k] = b' ';
                    text[k] = b' ';
                }
            }
        }
        out.push(ProdFile {
            rel: p
                .strip_prefix(root)
                .expect("workspace path")
                .to_string_lossy()
                .replace('\\', "/"),
            code: String::from_utf8_lossy(&code).into_owned(),
            text: String::from_utf8_lossy(&text).into_owned(),
        });
    }
    out
}

/// The name of the function whose body contains byte offset `at` in
/// `code` (the innermost `fn NAME` whose braces enclose it), with its
/// `impl` type if any, as `Type::name`.
pub fn enclosing_fn(code: &str, at: usize) -> String {
    let b = code.as_bytes();
    let mut best: Option<(usize, String)> = None;
    let mut from = 0;
    while let Some(p) = code[from..].find("fn ") {
        let s = from + p;
        from = s + 3;
        if s > at {
            break;
        }
        if s > 0 && (b[s - 1].is_ascii_alphanumeric() || b[s - 1] == b'_') {
            continue;
        }
        let name: String = code[s + 3..]
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        if name.is_empty() {
            continue;
        }
        // The body: the first `{` after the signature at depth 0 of parens.
        let mut j = s + 3 + name.len();
        let mut depth = 0i64;
        while j < b.len() {
            match b[j] {
                b'(' | b'[' | b'<' => depth += 1,
                b')' | b']' | b'>' => depth -= 1,
                b';' if depth <= 0 => break,
                b'{' if depth <= 0 => break,
                _ => {}
            }
            j += 1;
        }
        if j >= b.len() || b[j] != b'{' {
            continue;
        }
        let end = close_of(code, j);
        if j < at && at < end {
            best = Some((s, name));
        }
    }
    let Some((s, name)) = best else {
        return String::from("<top level>");
    };
    // The innermost enclosing `impl` block.
    let mut owner = None;
    let mut from = 0;
    while let Some(p) = code[from..s].find("impl") {
        let i = from + p;
        from = i + 4;
        if i > 0 && (b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_') {
            continue;
        }
        if !matches!(b.get(i + 4), Some(b' ' | b'<' | b'\n')) {
            continue;
        }
        let Some(open) = code[i..].find('{').map(|o| i + o) else {
            continue;
        };
        if close_of(code, open) <= s {
            continue;
        }
        let head: String = code[i..open]
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        let ty = head
            .rsplit(" for ")
            .next()
            .unwrap_or(&head)
            .split_whitespace()
            .last()
            .unwrap_or("")
            .split('<')
            .next()
            .unwrap_or("")
            .to_string();
        owner = Some(ty);
    }
    match owner {
        Some(ty) if !ty.is_empty() => format!("{ty}::{name}"),
        _ => name,
    }
}
