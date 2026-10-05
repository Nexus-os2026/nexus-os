//! XA-R4-FINAL: structural Rust source analysis shared by the trust guards
//! (the kernel resource limiter's process construction and termination,
//! the governed coding path's closed world, and Wasmtime's escape and async
//! rules).
//!
//! It is not a Rust parser and not a name resolver with type information.
//! It is a lexer that drops comments and keeps every literal whole, the
//! bindings a file's `use` trees (aliases, nested groups, `self`, globs,
//! `as _`), `extern crate` items and `type` aliases create, and every path
//! occurrence resolved through those bindings. Resolution fails closed for a
//! guard: a binding anywhere in a file applies to the whole file, a name
//! bound more than once resolves to every candidate, a name a glob may have
//! brought in also resolves under the glob's module, and only test-only items
//! (`#[test]`, `#[cfg(test)]`, `#[cfg(all(test, …))]`) are left out. A path
//! is attributed to the innermost function whose signature or body holds
//! it, qualified by its `impl` or `trait` type.
//!
//! The file is named `tests.rs` so the production-source scanners never read
//! its tables and fixtures as production code.

/// One token. Comments are dropped; every literal stays whole, so neither a
/// comment marker inside a literal nor a quote inside a comment can shift the
/// lexer out of step with the code that follows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Tok {
    /// An identifier or keyword; a raw identifier `r#x` is `x`.
    Ident(String),
    /// The text between a string literal's quotes (plain, byte, C or raw,
    /// any number of hashes), escapes left as written.
    Str(String),
    /// A number literal as written.
    Num(String),
    /// A character literal, lifetime or label: never part of a path.
    Atom,
    /// `::`, `->`, `=>`, or one other punctuation character.
    Punct(String),
}

#[derive(Debug, Clone)]
pub(crate) struct Token {
    pub(crate) tok: Tok,
    pub(crate) line: usize,
}

impl Token {
    /// Whether this is the identifier, punctuation or number `text`.
    pub(crate) fn is(&self, text: &str) -> bool {
        match &self.tok {
            Tok::Ident(s) | Tok::Punct(s) | Tok::Num(s) => s == text,
            _ => false,
        }
    }

    fn ident(&self) -> Option<&str> {
        match &self.tok {
            Tok::Ident(s) => Some(s),
            _ => None,
        }
    }
}

fn peek(c: &[char], k: usize) -> char {
    c.get(k).copied().unwrap_or('\0')
}

fn ident_start(ch: char) -> bool {
    ch == '_' || ch.is_alphabetic()
}

fn ident_char(ch: char) -> bool {
    ch == '_' || ch.is_alphanumeric()
}

/// The end of the (nested) block comment that starts at `i`.
fn block_comment_end(c: &[char], mut i: usize) -> usize {
    let mut depth = 0usize;
    while i < c.len() {
        if c[i] == '/' && peek(c, i + 1) == '*' {
            depth += 1;
            i += 2;
        } else if c[i] == '*' && peek(c, i + 1) == '/' {
            depth -= 1;
            i += 2;
            if depth == 0 {
                return i;
            }
        } else {
            i += 1;
        }
    }
    i
}

/// The string literal starting at `i`, if one does: `"…"`, `b"…"` or `c"…"`
/// with escapes, or raw `r"…"`, `r#"…"#` (any number of hashes), `br…` or
/// `cr…`, which end only at a quote followed by as many hashes. Its end and
/// the text between its quotes; an unterminated literal runs to the end.
fn string_literal(c: &[char], i: usize) -> Option<(usize, String)> {
    let mut p = i;
    if matches!(c[p], 'b' | 'c') && matches!(peek(c, p + 1), '"' | 'r') {
        p += 1;
    }
    if c[p] == 'r' {
        let hashes = c[p + 1..].iter().take_while(|&&ch| ch == '#').count();
        let open = p + 1 + hashes;
        if peek(c, open) != '"' {
            return None;
        }
        let mut k = open + 1;
        while k < c.len() {
            if c[k] == '"' && (1..=hashes).all(|h| peek(c, k + h) == '#') {
                return Some((k + 1 + hashes, c[open + 1..k].iter().collect()));
            }
            k += 1;
        }
        return Some((c.len(), c[open + 1..].iter().collect()));
    }
    if c[p] != '"' {
        return None;
    }
    let mut k = p + 1;
    while k < c.len() {
        match c[k] {
            '\\' => k += 2,
            '"' => return Some((k + 1, c[p + 1..k].iter().collect())),
            _ => k += 1,
        }
    }
    Some((c.len(), c[p + 1..].iter().collect()))
}

/// The end of the character literal (`'x'`, `b'x'`, `'\''`, `'\u{22}'`) or
/// lifetime or label (`'a`) starting at `i`, if one does.
fn char_or_lifetime(c: &[char], i: usize) -> Option<usize> {
    let q = if c[i] == 'b' && peek(c, i + 1) == '\'' {
        i + 1
    } else {
        i
    };
    if c[q] != '\'' {
        return None;
    }
    if peek(c, q + 1) == '\\' {
        let mut k = q + 3;
        while k < c.len() && c[k] != '\'' && c[k] != '\n' {
            k += 1;
        }
        return Some((k + 1).min(c.len()));
    }
    if peek(c, q + 2) == '\'' && !matches!(peek(c, q + 1), '\'' | '\n') {
        return Some(q + 3);
    }
    if q != i {
        return None;
    }
    let mut k = q + 1;
    while k < c.len() && ident_char(c[k]) {
        k += 1;
    }
    Some(k)
}

/// The identifier (or raw identifier `r#x`, as `x`) starting at `i`.
fn identifier(c: &[char], i: usize) -> Option<(usize, String)> {
    let start = if c[i] == 'r' && peek(c, i + 1) == '#' && ident_start(peek(c, i + 2)) {
        i + 2
    } else {
        i
    };
    if !ident_start(c[start]) {
        return None;
    }
    let mut k = start + 1;
    while k < c.len() && ident_char(c[k]) {
        k += 1;
    }
    Some((k, c[start..k].iter().collect()))
}

/// Rust source as tokens, comments dropped and literals whole.
pub(crate) fn lex(src: &str) -> Vec<Token> {
    let c: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    let (mut i, mut line) = (0, 1);
    while i < c.len() {
        let (start, first_line) = (i, line);
        let tok = if c[i].is_whitespace() {
            i += 1;
            None
        } else if c[i] == '/' && peek(&c, i + 1) == '/' {
            while i < c.len() && c[i] != '\n' {
                i += 1;
            }
            None
        } else if c[i] == '/' && peek(&c, i + 1) == '*' {
            i = block_comment_end(&c, i);
            None
        } else if let Some((end, text)) = string_literal(&c, i) {
            i = end;
            Some(Tok::Str(text))
        } else if let Some(end) = char_or_lifetime(&c, i) {
            i = end;
            Some(Tok::Atom)
        } else if c[i].is_ascii_digit() {
            while i < c.len()
                && (ident_char(c[i]) || (c[i] == '.' && peek(&c, i + 1).is_ascii_digit()))
            {
                i += 1;
            }
            Some(Tok::Num(c[start..i].iter().collect()))
        } else if let Some((end, name)) = identifier(&c, i) {
            i = end;
            Some(Tok::Ident(name))
        } else {
            let two: String = c[i..(i + 2).min(c.len())].iter().collect();
            if matches!(two.as_str(), "::" | "->" | "=>") {
                i += 2;
                Some(Tok::Punct(two))
            } else {
                i += 1;
                Some(Tok::Punct(c[start].to_string()))
            }
        };
        line += c[start..i].iter().filter(|&&ch| ch == '\n').count();
        if let Some(tok) = tok {
            out.push(Token {
                tok,
                line: first_line,
            });
        }
    }
    out
}

/// Keywords that never start a path (`crate`, `self`, `super` and `Self`
/// do).
const KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "dyn", "else", "enum", "extern", "false",
    "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref",
    "return", "static", "struct", "trait", "true", "type", "unsafe", "use", "where", "while",
];

/// The index of the bracket closing the one at `open` (`(`, `[` or `{`;
/// the others nested inside are counted too), or the end.
fn matching(t: &[Token], open: usize) -> usize {
    let mut depth = 0usize;
    for (k, token) in t.iter().enumerate().skip(open) {
        if token.is("(") || token.is("[") || token.is("{") {
            depth += 1;
        } else if token.is(")") || token.is("]") || token.is("}") {
            depth -= 1;
            if depth == 0 {
                return k;
            }
        }
    }
    t.len()
}

/// The index after the generic arguments opened by the `<` at `open`.
fn after_angles(t: &[Token], open: usize) -> usize {
    let mut depth = 0usize;
    for (k, token) in t.iter().enumerate().skip(open) {
        if token.is("<") {
            depth += 1;
        } else if token.is(">") {
            depth -= 1;
            if depth == 0 {
                return k + 1;
            }
        } else if token.is(";") || token.is("{") {
            return k;
        }
    }
    t.len()
}

/// Top-level comma-separated parts of `t`.
fn split_args(t: &[Token]) -> Vec<&[Token]> {
    let (mut parts, mut depth, mut from) = (Vec::new(), 0usize, 0);
    for (k, token) in t.iter().enumerate() {
        if token.is("(") || token.is("[") || token.is("{") {
            depth += 1;
        } else if token.is(")") || token.is("]") || token.is("}") {
            depth = depth.saturating_sub(1);
        } else if token.is(",") && depth == 0 {
            parts.push(&t[from..k]);
            from = k + 1;
        }
    }
    if from < t.len() {
        parts.push(&t[from..]);
    }
    parts
}

/// Whether a `cfg` predicate holds only in test builds: `test`, an `all`
/// with such a member, or an `any` whose members all are. `not(…)`, features
/// and platforms count as production (fail closed).
fn requires_test(p: &[Token]) -> bool {
    match p {
        [only] => only.is("test"),
        [head, open, args @ .., close]
            if (head.is("all") || head.is("any")) && open.is("(") && close.is(")") =>
        {
            let args = split_args(args);
            if head.is("all") {
                args.iter().any(|arg| requires_test(arg))
            } else {
                !args.is_empty() && args.iter().all(|arg| requires_test(arg))
            }
        }
        _ => false,
    }
}

/// Whether the attribute body `a` (between `#[` and `]`) marks a test-only
/// item: a test attribute (`test`, `tokio::test`, …) or a `cfg` that holds
/// only in test builds.
fn test_attribute(a: &[Token]) -> bool {
    if let [cfg, open, predicate @ .., close] = a {
        if cfg.is("cfg") && open.is("(") && close.is(")") {
            return requires_test(predicate);
        }
    }
    let path_end = a
        .iter()
        .position(|token| !(token.ident().is_some() || token.is("::")))
        .unwrap_or(a.len());
    path_end > 0 && a[path_end - 1].is("test") && (path_end == a.len() || a[path_end].is("("))
}

/// Keywords after which an identifier is defined, not used.
const DEFINERS: &[&str] = &[
    "fn",
    "struct",
    "enum",
    "union",
    "mod",
    "trait",
    "type",
    "const",
    "static",
    "let",
    "mut",
    "ref",
    "macro_rules",
];

const ITEM_KEYWORDS: &[&str] = &[
    "fn",
    "impl",
    "mod",
    "struct",
    "enum",
    "union",
    "trait",
    "type",
    "use",
    "const",
    "static",
    "extern",
    "unsafe",
    "async",
    "pub",
    "macro_rules",
];

/// The index after the item, field, arm or statement starting at `start`
/// (following attributes included): the `;`, or the block, that ends it;
/// for anything but an item also a `,` or an unmatched closer.
fn item_end(t: &[Token], mut start: usize) -> usize {
    while start < t.len() && t[start].is("#") && t.get(start + 1).is_some_and(|x| x.is("[")) {
        start = matching(t, start + 1) + 1;
    }
    let item = t
        .get(start)
        .is_some_and(|x| ITEM_KEYWORDS.iter().any(|keyword| x.is(keyword)));
    let mut k = start;
    while k < t.len() {
        let token = &t[k];
        if token.is("(") || token.is("[") {
            k = matching(t, k) + 1;
            continue;
        }
        if token.is("{") {
            return (matching(t, k) + 1).min(t.len());
        }
        if token.is(";") {
            return k + 1;
        }
        if !item && (token.is(",") || token.is(")") || token.is("]") || token.is("}")) {
            return k;
        }
        k += 1;
    }
    t.len()
}

/// Per token: whether it belongs to a test-only item.
fn test_mask(t: &[Token]) -> Vec<bool> {
    let mut mask = vec![false; t.len()];
    let mut k = 0;
    while k < t.len() {
        if !t[k].is("#") {
            k += 1;
            continue;
        }
        let inner = t.get(k + 1).is_some_and(|x| x.is("!"));
        let open = if inner { k + 2 } else { k + 1 };
        if !t.get(open).is_some_and(|x| x.is("[")) {
            k += 1;
            continue;
        }
        let close = matching(t, open);
        if test_attribute(&t[open + 1..close.min(t.len())]) {
            let (from, to) = if inner {
                // The enclosing module or block, or the whole file.
                let mut depth = 0usize;
                let mut from = 0;
                for j in (0..k).rev() {
                    if t[j].is("}") {
                        depth += 1;
                    } else if t[j].is("{") {
                        if depth == 0 {
                            from = j;
                            break;
                        }
                        depth -= 1;
                    }
                }
                let to = if from == 0 && !t[0].is("{") {
                    t.len()
                } else {
                    matching(t, from) + 1
                };
                (from, to.min(t.len()))
            } else {
                (k, item_end(t, close + 1))
            };
            mask[from..to].iter_mut().for_each(|m| *m = true);
        }
        k = close + 1;
    }
    mask
}

/// The type an `impl` header (the tokens between `impl` and `{`) is for.
fn impl_type(h: &[Token]) -> String {
    let mut k = 0;
    if h.first().is_some_and(|x| x.is("<")) {
        k = after_angles(h, 0);
    }
    let mut depth = 0usize;
    let mut from = k;
    let mut to = h.len();
    for (j, token) in h.iter().enumerate().skip(k) {
        if token.is("<") {
            depth += 1;
        } else if token.is(">") {
            depth = depth.saturating_sub(1);
        } else if depth == 0 && token.is("for") {
            from = j + 1;
        } else if depth == 0 && token.is("where") {
            to = j;
            break;
        }
    }
    let mut depth = 0usize;
    let mut name = None;
    for token in &h[from.min(to)..to] {
        if token.is("<") {
            depth += 1;
        } else if token.is(">") {
            depth = depth.saturating_sub(1);
        } else if depth == 0 {
            if let Some(ident) = token.ident() {
                name = Some(ident.to_string());
            }
        }
    }
    name.unwrap_or_else(|| "impl".to_string())
}

/// A path as written: `abs` for a leading `::`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Written {
    pub(crate) abs: bool,
    pub(crate) segs: Vec<String>,
}

impl std::fmt::Display for Written {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}{}",
            if self.abs { "::" } else { "" },
            self.segs.join("::")
        )
    }
}

/// What declared a binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Declaration {
    Use,
    ExternCrate,
    TypeAlias,
}

/// A name a declaration binds (`*` for a glob, `_` for `as _`).
#[derive(Debug, Clone)]
pub(crate) struct Binding {
    pub(crate) name: String,
    pub(crate) target: Written,
    pub(crate) declaration: Declaration,
    /// Declared `pub` or `pub(…)`: visible outside its file.
    pub(crate) public: bool,
    pub(crate) test: bool,
    pub(crate) line: usize,
    module: Vec<String>,
}

/// One path occurrence and everything it may resolve to.
#[derive(Debug, Clone)]
pub(crate) struct Occurrence {
    pub(crate) written: Written,
    /// Every candidate resolution (fail closed), each from its root
    /// (`crate`, an external crate, or an unresolved local name).
    pub(crate) resolved: Vec<Vec<String>>,
    pub(crate) line: usize,
    /// The innermost function holding it, qualified by its `impl` or `trait`
    /// type; `None` at module level.
    pub(crate) item: Option<String>,
    /// The declaration it is part of, if any (`use`, `extern crate`, or the
    /// target of a `type` alias).
    pub(crate) declaration: Option<Declaration>,
    pub(crate) public: bool,
    pub(crate) test: bool,
    /// Inside an attribute (`#[…]`, `#![…]`).
    pub(crate) attribute: bool,
    /// The index of its first token.
    pub(crate) token: usize,
}

/// One analysed source file.
#[derive(Debug)]
pub(crate) struct Analysis {
    pub(crate) tokens: Vec<Token>,
    /// Per token: inside a test-only item.
    pub(crate) test: Vec<bool>,
    /// The modules this file declares (`mod name;` or `mod name { … }`).
    pub(crate) child_modules: Vec<String>,
    /// Per token: the innermost function holding it (see [`Occurrence`]).
    pub(crate) item: Vec<Option<String>>,
    module: Vec<Vec<String>>,
    pub(crate) bindings: Vec<Binding>,
    pub(crate) occurrences: Vec<Occurrence>,
}

/// Per token: the innermost function holding it and its module path.
fn items(t: &[Token], root: &[String]) -> (Vec<Option<String>>, Vec<Vec<String>>) {
    enum Frame {
        Fn(String),
        Type(String),
        Mod(String),
        Block,
    }
    let mut stack: Vec<Frame> = Vec::new();
    let mut item = vec![None; t.len()];
    let mut module = vec![Vec::new(); t.len()];
    let (mut pending_fn, mut pending_impl, mut pending_mod, mut pending_trait) = (
        None::<String>,
        None::<usize>,
        None::<String>,
        None::<String>,
    );
    for k in 0..t.len() {
        let mut path = root.to_vec();
        path.extend(stack.iter().filter_map(|frame| match frame {
            Frame::Mod(name) => Some(name.clone()),
            _ => None,
        }));
        module[k] = path;
        let next_ident = t.get(k + 1).and_then(Token::ident).map(str::to_string);
        if let Some(name) = next_ident.clone().filter(|_| t[k].is("fn")) {
            let owner = stack.iter().rev().find_map(|frame| match frame {
                Frame::Type(ty) => Some(Some(ty.clone())),
                Frame::Fn(_) | Frame::Mod(_) => Some(None),
                Frame::Block => None,
            });
            pending_fn = Some(match owner.flatten() {
                Some(ty) => format!("{ty}::{name}"),
                None => name,
            });
        } else if t[k].is("impl") && pending_fn.is_none() {
            pending_impl = Some(k);
        } else if t[k].is("mod") && t.get(k + 2).is_some_and(|x| x.is("{")) {
            pending_mod = next_ident;
        } else if t[k].is("trait") && next_ident.is_some() && pending_fn.is_none() {
            pending_trait = next_ident;
        } else if t[k].is("{") {
            let frame = if let Some(name) = pending_fn.take() {
                Frame::Fn(name)
            } else if let Some(start) = pending_impl {
                Frame::Type(impl_type(&t[start + 1..k]))
            } else if let Some(name) = pending_mod.take() {
                Frame::Mod(name)
            } else if let Some(name) = pending_trait.take() {
                Frame::Type(name)
            } else {
                Frame::Block
            };
            pending_impl = None;
            pending_mod = None;
            pending_trait = None;
            stack.push(frame);
        } else if t[k].is("}") {
            stack.pop();
        } else if t[k].is(";") {
            pending_fn = None;
            pending_impl = None;
            pending_mod = None;
            pending_trait = None;
        }
        item[k] = pending_fn.clone().or_else(|| {
            stack.iter().rev().find_map(|frame| match frame {
                Frame::Fn(name) => Some(name.clone()),
                _ => None,
            })
        });
    }
    (item, module)
}

/// Whether the declaration keyword at `k` is preceded by `pub` or
/// `pub(…)`.
fn public_at(t: &[Token], k: usize) -> bool {
    if k == 0 {
        return false;
    }
    if t[k - 1].is("pub") {
        return true;
    }
    if t[k - 1].is(")") {
        let mut depth = 0usize;
        for j in (0..k).rev() {
            if t[j].is(")") {
                depth += 1;
            } else if t[j].is("(") {
                depth -= 1;
                if depth == 0 {
                    return j > 0 && t[j - 1].is("pub");
                }
            }
        }
    }
    false
}

/// Parses the use tree at `k` under `prefix`, appending its leaves (path,
/// alias, glob, first token); returns the index after it.
fn use_tree(
    t: &[Token],
    mut k: usize,
    prefix: &Written,
    leaves: &mut Vec<(Written, Option<String>, bool, usize)>,
) -> usize {
    let first = k;
    let mut path = prefix.clone();
    if path.segs.is_empty() && !path.abs && t.get(k).is_some_and(|x| x.is("::")) {
        path.abs = true;
        k += 1;
    }
    loop {
        match t.get(k) {
            Some(token) if token.ident().is_some_and(|s| s != "as") => {
                path.segs.push(token.ident().unwrap().to_string());
                k += 1;
            }
            Some(token) if token.is("*") => {
                leaves.push((path, None, true, first));
                return k + 1;
            }
            Some(token) if token.is("{") => {
                k += 1;
                while t.get(k).is_some_and(|x| !x.is("}")) {
                    let next = use_tree(t, k, &path, leaves);
                    k = if next > k { next } else { k + 1 };
                    if t.get(k).is_some_and(|x| x.is(",")) {
                        k += 1;
                    }
                }
                return k + 1;
            }
            _ => break,
        }
        if t.get(k).is_some_and(|x| x.is("::")) {
            k += 1;
        } else {
            break;
        }
    }
    let mut alias = None;
    if t.get(k).is_some_and(|x| x.is("as")) {
        alias = t.get(k + 1).and_then(Token::ident).map(str::to_string);
        k += 2;
    }
    if path.segs.len() > prefix.segs.len() || alias.is_some() {
        leaves.push((path, alias, false, first));
    }
    k
}

/// The leading path of `t` (optional `::`, segments, turbofish skipped), if
/// it starts with one.
fn leading_path(t: &[Token]) -> Option<Written> {
    let mut path = Written {
        abs: false,
        segs: Vec::new(),
    };
    let mut k = 0;
    if t.first().is_some_and(|x| x.is("::")) {
        path.abs = true;
        k = 1;
    }
    let first = t.get(k)?.ident()?;
    if KEYWORDS.contains(&first) {
        return None;
    }
    path.segs.push(first.to_string());
    k += 1;
    while t.get(k).is_some_and(|x| x.is("::")) {
        match t.get(k + 1) {
            Some(token) if token.ident().is_some() => {
                path.segs.push(token.ident().unwrap().to_string());
                k += 2;
            }
            Some(token) if token.is("<") => k = after_angles(t, k + 1),
            _ => break,
        }
    }
    Some(path)
}

impl Analysis {
    /// Analyse `src`, a file whose module path is `module` (from `crate`,
    /// e.g. `["crate", "coding_run", "apply"]`).
    pub(crate) fn new(src: &str, module: &[&str]) -> Self {
        let tokens = lex(src);
        let test = test_mask(&tokens);
        let mut attribute = vec![false; tokens.len()];
        for k in 0..tokens.len() {
            if tokens[k].is("#") {
                let open = if tokens.get(k + 1).is_some_and(|x| x.is("!")) {
                    k + 2
                } else {
                    k + 1
                };
                if tokens.get(open).is_some_and(|x| x.is("[")) {
                    let close = matching(&tokens, open).min(tokens.len() - 1);
                    attribute[k..=close].iter_mut().for_each(|a| *a = true);
                }
            }
        }
        let child_modules = tokens
            .windows(3)
            .filter(|w| w[0].is("mod") && (w[2].is(";") || w[2].is("{")))
            .filter_map(|w| w[1].ident().map(str::to_string))
            .collect();
        let root: Vec<String> = module.iter().map(|s| s.to_string()).collect();
        let (item, module) = items(&tokens, &root);
        let t = &tokens;
        let mut bindings = Vec::new();
        let mut occurrences = Vec::new();
        let mut declared = vec![false; t.len()];
        let mut k = 0;
        while k < t.len() {
            let raw_keyword = k > 0 && (t[k - 1].is("::") || t[k - 1].is("."));
            if t[k].is("use") && !raw_keyword {
                let public = public_at(t, k);
                let mut leaves = Vec::new();
                let end = use_tree(
                    t,
                    k + 1,
                    &Written {
                        abs: false,
                        segs: Vec::new(),
                    },
                    &mut leaves,
                );
                let end = t[end.min(t.len())..]
                    .iter()
                    .position(|x| x.is(";"))
                    .map_or(t.len(), |p| end + p + 1);
                for (mut path, alias, glob, first) in leaves {
                    if path.segs.last().is_some_and(|s| s == "self") {
                        path.segs.pop();
                    }
                    occurrences.push(Occurrence {
                        written: path.clone(),
                        resolved: Vec::new(),
                        line: t[first.min(t.len() - 1)].line,
                        item: item[k].clone(),
                        declaration: Some(Declaration::Use),
                        public,
                        test: test[k],
                        attribute: attribute[k],
                        token: first,
                    });
                    let name = if glob {
                        Some("*".to_string())
                    } else {
                        alias.or_else(|| path.segs.last().cloned())
                    };
                    if let Some(name) = name {
                        bindings.push(Binding {
                            name,
                            target: path,
                            declaration: Declaration::Use,
                            public,
                            test: test[k],
                            line: t[k].line,
                            module: module[k].clone(),
                        });
                    }
                }
                declared[k..end].iter_mut().for_each(|d| *d = true);
                k = end;
                continue;
            }
            if t[k].is("extern") && t.get(k + 1).is_some_and(|x| x.is("crate")) && !raw_keyword {
                if let Some(name) = t.get(k + 2).and_then(Token::ident) {
                    let target = Written {
                        abs: true,
                        segs: vec![name.to_string()],
                    };
                    let alias = t
                        .get(k + 3)
                        .filter(|x| x.is("as"))
                        .and_then(|_| t.get(k + 4))
                        .and_then(Token::ident)
                        .unwrap_or(name)
                        .to_string();
                    occurrences.push(Occurrence {
                        written: target.clone(),
                        resolved: Vec::new(),
                        line: t[k].line,
                        item: item[k].clone(),
                        declaration: Some(Declaration::ExternCrate),
                        public: public_at(t, k),
                        test: test[k],
                        attribute: attribute[k],
                        token: k + 2,
                    });
                    bindings.push(Binding {
                        name: alias,
                        target,
                        declaration: Declaration::ExternCrate,
                        public: public_at(t, k),
                        test: test[k],
                        line: t[k].line,
                        module: module[k].clone(),
                    });
                }
                let end = t[k..]
                    .iter()
                    .position(|x| x.is(";"))
                    .map_or(t.len(), |p| k + p + 1);
                declared[k..end].iter_mut().for_each(|d| *d = true);
                k = end;
                continue;
            }
            if t[k].is("type") && !raw_keyword {
                if let Some(name) = t.get(k + 1).and_then(Token::ident) {
                    let semi = t[k..]
                        .iter()
                        .position(|x| x.is(";"))
                        .map_or(t.len(), |p| k + p);
                    let mut eq = k + 2;
                    if t.get(eq).is_some_and(|x| x.is("<")) {
                        eq = after_angles(t, eq);
                    }
                    if eq < semi && t[eq].is("=") {
                        if let Some(target) = leading_path(&t[eq + 1..semi]) {
                            bindings.push(Binding {
                                name: name.to_string(),
                                target,
                                declaration: Declaration::TypeAlias,
                                public: public_at(t, k),
                                test: test[k],
                                line: t[k].line,
                                module: module[k].clone(),
                            });
                        }
                    }
                }
            }
            k += 1;
        }
        // Every other path occurrence. A segment after `::` or `.` (a
        // method or field) never starts one.
        let mut k = 0;
        while k < t.len() {
            let after_path_or_dot = k > 0 && (t[k - 1].is("::") || t[k - 1].is("."));
            // `crate`, `self` and `super` in `pub(crate)`, `pub(in …)` are
            // visibility, not paths.
            let visibility = k >= 2
                && ((t[k - 1].is("(") && t[k - 2].is("pub"))
                    || (t[k - 1].is("in") && t[k - 2].is("(") && k >= 3 && t[k - 3].is("pub")));
            // A name being defined (`fn f`, `let x`, `struct S`, …) is not a
            // path, nor is `_`, nor the value `self` (`self.x`) or a lone
            // `crate`/`super`.
            let definition = k > 0 && DEFINERS.iter().any(|definer| t[k - 1].is(definer));
            let lone = matches!(&t[k].tok, Tok::Ident(s) if matches!(s.as_str(), "_" | "self" | "super" | "crate"))
                && !t.get(k + 1).is_some_and(|x| x.is("::"));
            let starts = !declared[k]
                && !visibility
                && !definition
                && !lone
                && match &t[k].tok {
                    Tok::Ident(s) => !KEYWORDS.contains(&s.as_str()) && !after_path_or_dot,
                    Tok::Punct(p) => {
                        p == "::"
                            && t.get(k + 1).is_some_and(|x| x.ident().is_some())
                            && !(k > 0
                                && (t[k - 1].ident().is_some()
                                    || t[k - 1].is(">")
                                    || t[k - 1].is(")")
                                    || t[k - 1].is("]")))
                    }
                    _ => false,
                };
            if starts {
                if let Some(written) = leading_path(&t[k..]) {
                    let alias_target = k >= 2 && t[k - 1].is("=") && {
                        let mut j = k - 1;
                        while j > 0 && !t[j].is("type") && !t[j].is(";") && !t[j].is("{") {
                            j -= 1;
                        }
                        t[j].is("type")
                    };
                    occurrences.push(Occurrence {
                        written,
                        resolved: Vec::new(),
                        line: t[k].line,
                        item: item[k].clone(),
                        declaration: alias_target.then_some(Declaration::TypeAlias),
                        public: false,
                        test: test[k],
                        attribute: attribute[k],
                        token: k,
                    });
                }
            }
            k += 1;
        }
        let mut analysis = Self {
            tokens,
            test,
            child_modules,
            item,
            module,
            bindings,
            occurrences,
        };
        let resolved: Vec<Vec<Vec<String>>> = analysis
            .occurrences
            .iter()
            .map(|o| {
                let module = &analysis.module[o.token.min(analysis.module.len() - 1)];
                analysis.resolve(&o.written, module, o.declaration.is_none(), 0)
            })
            .collect();
        for (occurrence, resolved) in analysis.occurrences.iter_mut().zip(resolved) {
            occurrence.resolved = resolved;
        }
        analysis
    }

    /// Every path `written` (in module `module`) may resolve to. A name a
    /// glob may have brought in also resolves under the glob's module
    /// (`globs`: for a path in code, not for a declaration's own path or a
    /// binding's target).
    fn resolve(
        &self,
        written: &Written,
        module: &[String],
        globs: bool,
        depth: usize,
    ) -> Vec<Vec<String>> {
        let segs = &written.segs;
        if segs.is_empty() {
            return Vec::new();
        }
        if written.abs {
            return vec![segs.clone()];
        }
        let join = |head: &[String], tail: &[String]| {
            let mut path = head.to_vec();
            path.extend_from_slice(tail);
            path
        };
        match segs[0].as_str() {
            "crate" => vec![segs.clone()],
            "self" => vec![join(module, &segs[1..])],
            "super" => {
                let ups = segs.iter().take_while(|s| *s == "super").count();
                let keep = module.len().saturating_sub(ups).max(1);
                vec![join(&module[..keep], &segs[ups..])]
            }
            first => {
                // An explicit binding shadows a glob and the name itself.
                let mut out = Vec::new();
                if depth < 8 {
                    for binding in self.bindings.iter().filter(|b| b.name == first) {
                        for target in
                            self.resolve(&binding.target, &binding.module, false, depth + 1)
                        {
                            out.push(join(&target, &segs[1..]));
                        }
                    }
                }
                if out.is_empty() && self.child_modules.iter().any(|m| m == first) {
                    // A module this file declares.
                    out.push(join(module, segs));
                } else if out.is_empty() {
                    out.push(segs.clone());
                    if globs && depth < 8 {
                        for glob in self.bindings.iter().filter(|b| b.name == "*") {
                            for target in self.resolve(&glob.target, &glob.module, false, depth + 1)
                            {
                                out.push(join(&target, segs));
                            }
                        }
                    }
                }
                out.sort();
                out.dedup();
                out
            }
        }
    }

    /// Whether `name` is bound by a declaration in this file (glob imports
    /// aside).
    pub(crate) fn binds(&self, name: &str) -> bool {
        self.bindings.iter().any(|b| b.name == name)
    }

    /// Production occurrences (outside test-only items).
    pub(crate) fn production(&self) -> impl Iterator<Item = &Occurrence> {
        self.occurrences.iter().filter(|o| !o.test)
    }

    /// The production tokens of the function `qualified` (`Type::name` or
    /// `name`), signature and body, in order.
    pub(crate) fn function(&self, qualified: &str) -> Vec<&Token> {
        self.tokens
            .iter()
            .zip(&self.item)
            .zip(&self.test)
            .filter(|((_, item), test)| !**test && item.as_deref() == Some(qualified))
            .map(|((token, _), _)| token)
            .collect()
    }

    /// The production string literals, each with its function.
    pub(crate) fn literals(&self) -> Vec<(&str, Option<&str>, usize)> {
        self.tokens
            .iter()
            .zip(&self.item)
            .zip(&self.test)
            .filter(|(_, test)| !**test)
            .filter_map(|((token, item), _)| match &token.tok {
                Tok::Str(text) => Some((text.as_str(), item.as_deref(), token.line)),
                _ => None,
            })
            .collect()
    }
}

// ── The vocabulary the guards share ─────────────────────────────────────

/// Whether `path` is `module::name…` for one of `names`.
fn under(path: &[String], module: &str, names: &[&str]) -> bool {
    let depth = module.split("::").count();
    path.len() > depth && starts_with(path, module) && names.contains(&path[depth].as_str())
}

/// Whether `path` names one of `names` in a `libc` module (`libc::…` or a
/// re-export such as `nix::libc::…`).
fn libc_item(path: &[String], names: &[&str]) -> bool {
    path.windows(2)
        .any(|pair| pair[0] == "libc" && names.contains(&pair[1].as_str()))
}

/// Whether `path` is in a Windows API crate and names one of `names`.
fn windows_item(path: &[String], names: &[&str]) -> bool {
    path.first()
        .is_some_and(|root| root == "windows_sys" || root == "windows")
        && path.iter().any(|segment| names.contains(&segment.as_str()))
}

/// Whether a resolved path constructs or replaces a process: std's
/// `Command` and the `CommandExt` traits (`exec`, `pre_exec`), process
/// crates, `fork`, `exec*`, `posix_spawn*`, `system` or `popen` (nix or
/// libc), and the Windows process-creation and shell-execute calls.
pub(crate) fn constructs_process(path: &[String]) -> bool {
    under(path, "std::process", &["Command"])
        || under(path, "std::os::unix::process", &["CommandExt"])
        || under(path, "std::os::windows::process", &["CommandExt"])
        || [
            "tokio::process",
            "async_process",
            "async_std::process",
            "duct",
            "subprocess",
            "xshell",
            "nix::spawn",
        ]
        .iter()
        .any(|prefix| starts_with(path, prefix))
        || under(
            path,
            "nix::unistd",
            &[
                "fork", "execv", "execve", "execvp", "execvpe", "execveat", "fexecve",
            ],
        )
        || libc_item(
            path,
            &[
                "fork",
                "vfork",
                "clone",
                "clone3",
                "execv",
                "execve",
                "execvp",
                "execvpe",
                "execveat",
                "fexecve",
                "execl",
                "execle",
                "execlp",
                "posix_spawn",
                "posix_spawnp",
                "system",
                "popen",
            ],
        )
        || windows_item(
            path,
            &[
                "CreateProcessW",
                "CreateProcessA",
                "CreateProcessAsUserW",
                "CreateProcessAsUserA",
                "CreateProcessWithLogonW",
                "CreateProcessWithTokenW",
                "ShellExecuteW",
                "ShellExecuteA",
                "ShellExecuteExW",
                "ShellExecuteExA",
                "WinExec",
            ],
        )
}

/// Whether a resolved path signals, opens or ends a process or a process
/// group by an identifier.
pub(crate) fn ends_process(path: &[String]) -> bool {
    under(path, "nix::sys::signal", &["kill", "killpg"])
        || libc_item(
            path,
            &[
                "kill",
                "killpg",
                "tgkill",
                "tkill",
                "pidfd_send_signal",
                "sigqueue",
            ],
        )
        || windows_item(
            path,
            &[
                "TerminateProcess",
                "TerminateJobObject",
                "OpenProcess",
                "DebugActiveProcess",
                "NtTerminateProcess",
            ],
        )
}

/// Whether a resolved path opens a network connection or socket: std's
/// sockets, Unix sockets, async and HTTP client crates, and the libc socket
/// calls. (`std::net`'s address types are not sockets.)
pub(crate) fn opens_network(path: &[String]) -> bool {
    under(path, "std::net", &["TcpStream", "TcpListener", "UdpSocket"])
        || [
            "std::os::unix::net",
            "tokio::net",
            "async_std::net",
            "socket2",
            "mio",
            "hyper",
            "reqwest",
            "ureq",
            "curl",
            "isahc",
            "surf",
            "attohttpc",
        ]
        .iter()
        .any(|prefix| starts_with(path, prefix))
        || libc_item(
            path,
            &[
                "socket",
                "socketpair",
                "connect",
                "bind",
                "listen",
                "accept",
                "accept4",
                "sendto",
                "sendmsg",
                "recvfrom",
                "recvmsg",
            ],
        )
}

/// Whether `path` starts with the segments of `prefix` (`"a::b"`).
pub(crate) fn starts_with(path: &[String], prefix: &str) -> bool {
    let prefix: Vec<&str> = prefix.split("::").collect();
    path.len() >= prefix.len() && path.iter().zip(&prefix).all(|(a, b)| a == b)
}

/// Whether `tokens` contains the token sequence `pattern` (texts of
/// identifiers, punctuation and numbers, separated by spaces).
pub(crate) fn contains_sequence(tokens: &[&Token], pattern: &str) -> bool {
    let pattern: Vec<&str> = pattern.split_whitespace().collect();
    tokens.windows(pattern.len()).any(|window| {
        window
            .iter()
            .zip(&pattern)
            .all(|(token, text)| token.is(text))
    })
}
