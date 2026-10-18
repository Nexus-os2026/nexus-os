//! The workspace's production source, as the N5 guards read it.
//!
//! - Every `.rs` file of the workspace (build output, dependencies, VCS
//!   metadata and `docs/` excluded).
//! - Test code is excluded: test files (`tests/`, `*_tests.rs`,
//!   `*/tests.rs`, `benches/`, `examples/`, `build.rs`), every module file
//!   declared under `#[cfg(test)]` (and everything below it), and every
//!   `#[cfg(test)]` item inside a production file.
//! - Two views of a production file: `code` (comments and the contents of
//!   string and char literals blanked) and `text` (comments blanked,
//!   literals kept). Both keep every byte offset and line.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

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

pub fn relative(path: &Path) -> String {
    path.strip_prefix(workspace_root())
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
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

/// `text` with comments blanked and, when `literals` is false, the contents
/// of string and char literals blanked too. Newlines are kept.
pub fn mask(text: &str, literals: bool) -> String {
    let b = text.as_bytes();
    let mut out = b.to_vec();
    let blank = |out: &mut Vec<u8>, from: usize, to: usize| {
        for byte in out.iter_mut().take(to).skip(from) {
            if *byte != b'\n' {
                *byte = b' ';
            }
        }
    };
    let ident = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
    let mut i = 0;
    while i < b.len() {
        if b[i..].starts_with(b"//") {
            let end = b[i..]
                .iter()
                .position(|&c| c == b'\n')
                .map_or(b.len(), |p| i + p);
            blank(&mut out, i, end);
            i = end;
        } else if b[i..].starts_with(b"/*") {
            let (mut depth, mut j) = (1, i + 2);
            while j < b.len() && depth > 0 {
                if b[j..].starts_with(b"/*") {
                    depth += 1;
                    j += 2;
                } else if b[j..].starts_with(b"*/") {
                    depth -= 1;
                    j += 2;
                } else {
                    j += 1;
                }
            }
            blank(&mut out, i, j);
            i = j;
        } else if (b[i] == b'r' || (b[i] == b'b' && b.get(i + 1) == Some(&b'r')))
            && (i == 0 || !ident(b[i - 1]))
            && {
                let k = if b[i] == b'b' { i + 2 } else { i + 1 };
                let mut h = k;
                while b.get(h) == Some(&b'#') {
                    h += 1;
                }
                b.get(h) == Some(&b'"')
            }
        {
            let k = if b[i] == b'b' { i + 2 } else { i + 1 };
            let mut h = k;
            while b[h] == b'#' {
                h += 1;
            }
            let hashes = h - k;
            let body = h + 1;
            let mut j = body;
            let end = loop {
                if j >= b.len() {
                    break b.len();
                }
                if b[j] == b'"'
                    && b[j + 1..]
                        .iter()
                        .take(hashes)
                        .filter(|&&c| c == b'#')
                        .count()
                        == hashes
                    && b.len() >= j + 1 + hashes
                {
                    break j;
                }
                j += 1;
            };
            if !literals {
                blank(&mut out, body, end);
            }
            i = (end + 1 + hashes).min(b.len());
        } else if b[i] == b'"' {
            let mut j = i + 1;
            while j < b.len() && b[j] != b'"' {
                j += if b[j] == b'\\' { 2 } else { 1 };
            }
            if !literals {
                blank(&mut out, i + 1, j.min(b.len()));
            }
            i = j + 1;
        } else if b[i] == b'\'' {
            // A char literal ('x', '\n', '\u{..}'); a lifetime is left alone.
            if b.get(i + 1) == Some(&b'\\') {
                if let Some(p) = b[i + 2..].iter().take(10).position(|&c| c == b'\'') {
                    if !literals {
                        blank(&mut out, i + 1, i + 2 + p);
                    }
                    i = i + 3 + p;
                    continue;
                }
            } else if b.get(i + 2) == Some(&b'\'') {
                if !literals {
                    blank(&mut out, i + 1, i + 2);
                }
                i += 3;
                continue;
            } else if let Some(len) = text[i + 1..].chars().next().map(char::len_utf8) {
                if len > 1 && b.get(i + 1 + len) == Some(&b'\'') {
                    if !literals {
                        blank(&mut out, i + 1, i + 1 + len);
                    }
                    i += 2 + len;
                    continue;
                }
            }
            i += 1;
        } else {
            i += 1;
        }
    }
    String::from_utf8(out).unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned())
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

/// Skips whitespace and outer attributes from `i`.
fn skip_attributes(code: &str, mut i: usize) -> usize {
    let b = code.as_bytes();
    loop {
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if b[i..].starts_with(b"#[") {
            i = close_of(code, i + 1);
            continue;
        }
        return i;
    }
}

/// Whether the attribute starting at `at` (`#[cfg(...)]`) gates on `test`.
fn cfg_test_attribute(code: &str, at: usize) -> Option<usize> {
    let end = close_of(code, at + 1);
    let attr: String = code[at..end].split_whitespace().collect();
    let gated = attr.starts_with("#[cfg(test)")
        || attr.starts_with("#[cfg(all(test")
        || attr.starts_with("#[cfg(all(any(test");
    gated.then_some(end)
}

/// Byte ranges `(start, end)` of a file.
pub type Ranges = Vec<(usize, usize)>;

/// `mod NAME;` declarations, each with its `#[path]` if any.
pub type ModuleDecls = Vec<(String, Option<String>)>;

/// The byte ranges of the `#[cfg(test)]` items of `code` (attribute
/// included), and the names of the `#[cfg(test)] mod NAME;` declarations
/// with their `#[path]`, if any.
pub fn cfg_test_items(code: &str, text: &str) -> (Ranges, ModuleDecls) {
    let mut ranges = Vec::new();
    let mut modules = Vec::new();
    let mut from = 0;
    while let Some(p) = code[from..].find("#[cfg(") {
        let at = from + p;
        from = at + 1;
        let Some(attr_end) = cfg_test_attribute(code, at) else {
            continue;
        };
        let item = skip_attributes(code, attr_end);
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
                    let path = text[at..item].find("#[path").and_then(|q| {
                        let s = &text[at + q..item];
                        let a = s.find('"')? + 1;
                        let z = s[a..].find('"')? + a;
                        Some(s[a..z].to_string())
                    });
                    modules.push((name.trim_end_matches(';').to_string(), path));
                }
            }
        }
        ranges.push((at, end));
        from = end;
    }
    (ranges, modules)
}

/// Every `mod NAME;` declaration of `code` (any gating), with its
/// `#[path]`.
fn module_declarations(code: &str, text: &str) -> Vec<(String, Option<String>)> {
    let mut out = Vec::new();
    let b = code.as_bytes();
    let mut from = 0;
    while let Some(p) = code[from..].find("mod ") {
        let at = from + p;
        from = at + 4;
        if at > 0 && (b[at - 1].is_ascii_alphanumeric() || b[at - 1] == b'_') {
            continue;
        }
        let rest = &code[at + 4..];
        let name: String = rest
            .trim_start()
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        let after = rest.trim_start()[name.len()..].trim_start();
        if name.is_empty() || !after.starts_with(';') {
            continue;
        }
        let line_start = code[..at].rfind('\n').map_or(0, |n| n + 1);
        let window_start = code[..line_start].rfind(['}', ';']).map_or(0, |n| n + 1);
        let path = text[window_start..at].find("#[path").and_then(|q| {
            let s = &text[window_start + q..at];
            let a = s.find('"')? + 1;
            let z = s[a..].find('"')? + a;
            Some(s[a..z].to_string())
        });
        out.push((name, path));
    }
    out
}

/// The file a `mod NAME;` of `declaring` resolves to.
fn module_file(declaring: &Path, name: &str, path: Option<&str>) -> Option<PathBuf> {
    let dir = declaring.parent()?;
    if let Some(path) = path {
        return Some(dir.join(path));
    }
    let file = declaring.file_name()?.to_string_lossy();
    let base = if matches!(file.as_ref(), "mod.rs" | "lib.rs" | "main.rs") {
        dir.to_path_buf()
    } else {
        dir.join(declaring.file_stem()?)
    };
    let flat = base.join(format!("{name}.rs"));
    if flat.exists() {
        return Some(flat);
    }
    let nested = base.join(name).join("mod.rs");
    nested.exists().then_some(nested)
}

/// Every production file of the workspace.
pub fn production_files() -> Vec<ProdFile> {
    let root = workspace_root();
    let mut paths = Vec::new();
    walk(&root, &mut paths);
    paths.retain(|p| !relative(p).starts_with("docs/"));
    paths.sort();
    let mut raw: BTreeMap<PathBuf, String> = BTreeMap::new();
    for p in &paths {
        if let Ok(t) = std::fs::read_to_string(p) {
            raw.insert(p.clone(), t);
        }
    }
    // Test module files: by path, by a `#[cfg(test)] mod` declaration, and
    // every module declared by a test file, to a fixed point.
    let mut test: BTreeSet<PathBuf> = paths
        .iter()
        .filter(|p| is_test_path(&relative(p)))
        .cloned()
        .collect();
    let mut cfg_ranges: BTreeMap<PathBuf, Vec<(usize, usize)>> = BTreeMap::new();
    for (p, t) in &raw {
        let code = mask(t, false);
        let text = mask(t, true);
        let (ranges, modules) = cfg_test_items(&code, &text);
        for (name, path) in modules {
            if let Some(f) = module_file(p, &name, path.as_deref()) {
                test.insert(f);
            }
        }
        cfg_ranges.insert(p.clone(), ranges);
    }
    loop {
        let mut more = Vec::new();
        for p in &test {
            if let Some(t) = raw.get(p) {
                let code = mask(t, false);
                let text = mask(t, true);
                for (name, path) in module_declarations(&code, &text) {
                    if let Some(f) = module_file(p, &name, path.as_deref()) {
                        if !test.contains(&f) {
                            more.push(f);
                        }
                    }
                }
            }
        }
        if more.is_empty() {
            break;
        }
        test.extend(more);
    }
    let mut out = Vec::new();
    for (p, t) in &raw {
        if test.contains(p) {
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
            rel: relative(p),
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
