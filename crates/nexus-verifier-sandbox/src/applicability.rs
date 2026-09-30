//! Applicability of `rust.cargo-test.offline.v1`, decided by the backend.
//!
//! The profile applies only to a single dependency-free Rust library
//! package. The decision is made here, from the candidate's own files, by a
//! parser that accepts exactly a small documented grammar; anything outside
//! it is not applicable. No model decides applicability.
//!
//! Accepted `Cargo.toml` (a TOML subset: bare keys, basic and literal
//! strings, integers, booleans, arrays of those, `#` comments):
//!
//! - `[package]` (required) with `name` and `version` (required) and only
//!   `edition` (2015, 2018, 2021 or 2024), `rust-version`, `authors`,
//!   `description`, `license`, `readme`, `repository`, `homepage`,
//!   `documentation`, `keywords`, `categories` and `publish`;
//! - `[lib]` (optional) with only `name`, `path` (a normalized relative
//!   `.rs` path of the candidate), `test`, `doctest`, `bench` and `doc`;
//! - `[dependencies]` (optional) with no entry.
//!
//! Anything else is refused, among it: a workspace, any dependency
//! (registry, git or path, normal, dev, build or target-specific), patches,
//! features, profiles, explicit targets, `build`, `links`, proc macros and
//! crate types, and every other key or table. The candidate must have its
//! library source, no `build.rs`, no `.cargo/config` or
//! `.cargo/config.toml` anywhere, and a current `Cargo.lock`: lock format 3
//! or 4 and exactly the package itself, with no source, checksum or
//! dependency.

use std::collections::{BTreeMap, BTreeSet};

/// Why the profile does not apply. Value-free.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotApplicable {
    /// No `Cargo.toml` at the candidate root.
    NoManifest,
    /// The manifest is outside the accepted grammar.
    UnsupportedManifest,
    Workspace,
    Dependencies,
    DevDependencies,
    BuildDependencies,
    TargetSpecific,
    /// `[patch]` or `[replace]`.
    Patch,
    /// A table or key the profile does not accept.
    UnsupportedSetting,
    /// `build.rs`, or a `build` or `links` key.
    BuildScript,
    /// `.cargo/config` or `.cargo/config.toml`.
    CargoConfig,
    /// A proc macro or another crate type.
    NotALibrary,
    MissingLibrary,
    MissingLockFile,
    /// The lock file is outside its grammar or does not describe exactly
    /// the package.
    StaleLockFile,
    /// A candidate path is not UTF-8.
    UnsupportedPath,
}

/// What an applicable candidate is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustPackage {
    pub name: String,
    pub version: String,
}

/// A parsed value of the subset.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Value {
    String(String),
    Integer(i64),
    Bool(bool),
    Array(Vec<Value>),
}

/// One `[table]` or `[[table]]`, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Table {
    path: Vec<String>,
    array: bool,
    entries: BTreeMap<String, Value>,
}

/// A document: top-level entries (path empty) and then tables.
fn parse(text: &str) -> Option<Vec<Table>> {
    let mut tables = vec![Table {
        path: Vec::new(),
        array: false,
        entries: BTreeMap::new(),
    }];
    let mut lines = text.lines().peekable();
    while let Some(raw) = lines.next() {
        let line = strip_comment(raw)?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(header) = line.strip_prefix("[[") {
            let path = key_path(header.strip_suffix("]]")?)?;
            if tables.iter().any(|t| t.path == path && !t.array) {
                return None;
            }
            tables.push(Table {
                path,
                array: true,
                entries: BTreeMap::new(),
            });
            continue;
        }
        if let Some(header) = line.strip_prefix('[') {
            let path = key_path(header.strip_suffix(']')?)?;
            if tables.iter().any(|t| t.path == path) {
                return None;
            }
            tables.push(Table {
                path,
                array: false,
                entries: BTreeMap::new(),
            });
            continue;
        }
        let (key, rest) = line.split_once('=')?;
        let key = bare_key(key.trim())?;
        let mut value_text = rest.trim().to_owned();
        // An array may continue over several lines until it closes.
        while value_text.starts_with('[') && !array_closed(&value_text)? {
            let next = strip_comment(lines.next()?)?;
            value_text.push(' ');
            value_text.push_str(next.trim());
        }
        let (value, rest) = parse_value(&value_text)?;
        if !rest.trim().is_empty() {
            return None;
        }
        let table = tables.last_mut()?;
        if table.entries.insert(key, value).is_some() {
            return None;
        }
    }
    Some(tables)
}

/// The line without a trailing comment, respecting strings.
fn strip_comment(line: &str) -> Option<&str> {
    let mut in_basic = false;
    let mut in_literal = false;
    let mut escaped = false;
    for (at, c) in line.char_indices() {
        if in_basic {
            match (escaped, c) {
                (true, _) => escaped = false,
                (false, '\\') => escaped = true,
                (false, '"') => in_basic = false,
                _ => {}
            }
        } else if in_literal {
            if c == '\'' {
                in_literal = false;
            }
        } else {
            match c {
                '"' => in_basic = true,
                '\'' => in_literal = true,
                '#' => return Some(&line[..at]),
                _ => {}
            }
        }
    }
    // A string never spans lines in this subset.
    (!in_basic && !in_literal).then_some(line)
}

fn array_closed(text: &str) -> Option<bool> {
    let mut depth = 0i32;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '[' => depth += 1,
            ']' => depth -= 1,
            '"' => loop {
                match chars.next()? {
                    '\\' => {
                        chars.next()?;
                    }
                    '"' => break,
                    _ => {}
                }
            },
            '\'' => loop {
                if chars.next()? == '\'' {
                    break;
                }
            },
            _ => {}
        }
        if depth == 0 {
            return Some(true);
        }
    }
    Some(false)
}

fn bare_key(text: &str) -> Option<String> {
    (!text.is_empty()
        && text
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-'))
    .then(|| text.to_owned())
}

/// A table header: dot-separated bare keys.
fn key_path(text: &str) -> Option<Vec<String>> {
    text.trim()
        .split('.')
        .map(|part| bare_key(part.trim()))
        .collect()
}

/// Parse one value; return it and the unparsed rest.
fn parse_value(text: &str) -> Option<(Value, &str)> {
    let text = text.trim_start();
    if let Some(rest) = text.strip_prefix('"') {
        if rest.starts_with("\"\"") {
            return None; // multi-line strings are outside the subset
        }
        let mut out = String::new();
        let mut chars = rest.char_indices();
        while let Some((at, c)) = chars.next() {
            match c {
                '"' => return Some((Value::String(out), &rest[at + 1..])),
                '\\' => match chars.next()?.1 {
                    '"' => out.push('"'),
                    '\\' => out.push('\\'),
                    'n' => out.push('\n'),
                    't' => out.push('\t'),
                    'u' => {
                        let hex: String = (0..4)
                            .map(|_| chars.next().map(|(_, c)| c))
                            .collect::<Option<_>>()?;
                        out.push(char::from_u32(u32::from_str_radix(&hex, 16).ok()?)?);
                    }
                    _ => return None,
                },
                c if c.is_control() && c != '\t' => return None,
                c => out.push(c),
            }
        }
        return None;
    }
    if let Some(rest) = text.strip_prefix('\'') {
        if rest.starts_with("''") {
            return None;
        }
        let end = rest.find('\'')?;
        let value = &rest[..end];
        if value.chars().any(|c| c.is_control() && c != '\t') {
            return None;
        }
        return Some((Value::String(value.to_owned()), &rest[end + 1..]));
    }
    if let Some(mut rest) = text.strip_prefix('[') {
        let mut items = Vec::new();
        loop {
            rest = rest.trim_start();
            if let Some(after) = rest.strip_prefix(']') {
                return Some((Value::Array(items), after));
            }
            let (item, after) = parse_value(rest)?;
            items.push(item);
            rest = after.trim_start();
            if let Some(after) = rest.strip_prefix(',') {
                rest = after;
            } else if !rest.starts_with(']') {
                return None;
            }
        }
    }
    for (word, value) in [("true", true), ("false", false)] {
        if let Some(rest) = text.strip_prefix(word) {
            if !rest.starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_') {
                return Some((Value::Bool(value), rest));
            }
        }
    }
    let digits = text
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(text.len());
    if digits > 0 && digits <= 9 && !(digits > 1 && text.starts_with('0')) {
        let rest = &text[digits..];
        if !rest.starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_' || c == '.') {
            return Some((Value::Integer(text[..digits].parse().ok()?), rest));
        }
    }
    None
}

fn is_crate_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name.starts_with(|c: char| c.is_ascii_alphabetic())
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

fn is_version(version: &str) -> bool {
    let core = version.split(['-', '+']).next().unwrap_or("");
    let parts: Vec<&str> = core.split('.').collect();
    version.len() <= 64
        && parts.len() == 3
        && parts.iter().all(|part| {
            !part.is_empty()
                && part.bytes().all(|b| b.is_ascii_digit())
                && !(part.len() > 1 && part.starts_with('0'))
        })
        && version
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'+'))
}

/// A normalized relative path of the candidate: no empty, `.` or `..`
/// component, no leading `/`.
fn is_relative_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && path
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}

fn string(value: &Value) -> Option<&str> {
    match value {
        Value::String(text) => Some(text),
        _ => None,
    }
}

fn strings(value: &Value) -> bool {
    matches!(value, Value::Array(items) if items.iter().all(|item| string(item).is_some()))
}

/// The first table header's leading key, read textually (quotes allowed),
/// and whether a `[dependencies]` table has any entry: precise reasons even
/// for manifests the subset cannot parse.
fn prescan(text: &str) -> Option<NotApplicable> {
    use NotApplicable::*;
    let mut in_dependencies = false;
    for raw in text.lines() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') {
            let head = line
                .trim_start_matches('[')
                .split(['.', ']'])
                .next()
                .unwrap_or("")
                .trim()
                .trim_matches(['"', '\'']);
            in_dependencies = head == "dependencies";
            match head {
                "workspace" => return Some(Workspace),
                "dev-dependencies" | "dev_dependencies" => return Some(DevDependencies),
                "build-dependencies" | "build_dependencies" => return Some(BuildDependencies),
                "target" => return Some(TargetSpecific),
                "patch" | "replace" => return Some(Patch),
                _ => {}
            }
        } else if in_dependencies {
            return Some(Dependencies);
        }
    }
    None
}

/// Decide whether `rust.cargo-test.offline.v1` applies to a candidate, from
/// its file paths (`/`-separated, relative) and a reader of its files.
pub fn check(
    files: &BTreeSet<String>,
    read: &dyn Fn(&str) -> Option<Vec<u8>>,
) -> Result<RustPackage, NotApplicable> {
    use NotApplicable::*;
    for path in files {
        let parts: Vec<&str> = path.split('/').collect();
        if parts
            .windows(2)
            .any(|pair| pair[0] == ".cargo" && matches!(pair[1], "config" | "config.toml"))
        {
            return Err(CargoConfig);
        }
    }
    if files.contains("build.rs") {
        return Err(BuildScript);
    }
    if !files.contains("Cargo.toml") {
        return Err(NoManifest);
    }
    let manifest = read("Cargo.toml").ok_or(NoManifest)?;
    let manifest = String::from_utf8(manifest).map_err(|_| UnsupportedManifest)?;
    if let Some(reason) = prescan(&manifest) {
        return Err(reason);
    }
    let tables = parse(&manifest).ok_or(UnsupportedManifest)?;
    let mut package = None;
    let mut library_path = "src/lib.rs".to_owned();
    for table in &tables {
        let head = table.path.first().map(String::as_str);
        match (head, table.path.len(), table.array) {
            (None, _, _) => {
                if !table.entries.is_empty() {
                    return Err(UnsupportedSetting);
                }
            }
            (Some("workspace"), _, _) => return Err(Workspace),
            (Some("dependencies"), 1, false) => {
                if !table.entries.is_empty() {
                    return Err(Dependencies);
                }
            }
            (Some("dependencies"), _, _) => return Err(Dependencies),
            (Some("dev-dependencies" | "dev_dependencies"), _, _) => return Err(DevDependencies),
            (Some("build-dependencies" | "build_dependencies"), _, _) => {
                return Err(BuildDependencies)
            }
            (Some("target"), _, _) => return Err(TargetSpecific),
            (Some("patch" | "replace"), _, _) => return Err(Patch),
            (Some("package"), 1, false) => package = Some(check_package(&table.entries)?),
            (Some("lib"), 1, false) => {
                if let Some(path) = check_lib(&table.entries)? {
                    library_path = path;
                }
            }
            _ => return Err(UnsupportedSetting),
        }
    }
    let package = package.ok_or(UnsupportedManifest)?;
    if !files.contains(&library_path) {
        return Err(MissingLibrary);
    }
    if !files.contains("Cargo.lock") {
        return Err(MissingLockFile);
    }
    let lock = read("Cargo.lock").ok_or(MissingLockFile)?;
    let lock = String::from_utf8(lock).map_err(|_| StaleLockFile)?;
    check_lock(&lock, &package)?;
    Ok(package)
}

fn check_package(entries: &BTreeMap<String, Value>) -> Result<RustPackage, NotApplicable> {
    use NotApplicable::*;
    let mut name = None;
    let mut version = None;
    for (key, value) in entries {
        let ok = match key.as_str() {
            "name" => {
                name = string(value)
                    .filter(|n| is_crate_name(n))
                    .map(str::to_owned);
                name.is_some()
            }
            "version" => {
                version = string(value).filter(|v| is_version(v)).map(str::to_owned);
                version.is_some()
            }
            "edition" => matches!(string(value), Some("2015" | "2018" | "2021" | "2024")),
            "rust-version" | "description" | "license" | "readme" | "repository" | "homepage"
            | "documentation" => string(value).is_some(),
            "authors" | "keywords" | "categories" => strings(value),
            "publish" => matches!(value, Value::Bool(_)),
            "build" | "links" => return Err(BuildScript),
            "workspace" => return Err(Workspace),
            _ => return Err(UnsupportedSetting),
        };
        if !ok {
            return Err(UnsupportedManifest);
        }
    }
    Ok(RustPackage {
        name: name.ok_or(UnsupportedManifest)?,
        version: version.ok_or(UnsupportedManifest)?,
    })
}

fn check_lib(entries: &BTreeMap<String, Value>) -> Result<Option<String>, NotApplicable> {
    use NotApplicable::*;
    let mut path = None;
    for (key, value) in entries {
        let ok = match key.as_str() {
            "name" => string(value).is_some_and(is_crate_name),
            "path" => {
                path = string(value)
                    .filter(|p| is_relative_path(p) && p.ends_with(".rs"))
                    .map(str::to_owned);
                path.is_some()
            }
            "test" | "doctest" | "bench" | "doc" => matches!(value, Value::Bool(_)),
            "proc-macro" | "proc_macro" | "crate-type" | "crate_type" => return Err(NotALibrary),
            _ => return Err(UnsupportedSetting),
        };
        if !ok {
            return Err(UnsupportedManifest);
        }
    }
    Ok(path)
}

fn check_lock(text: &str, package: &RustPackage) -> Result<(), NotApplicable> {
    let stale = NotApplicable::StaleLockFile;
    let tables = parse(text).ok_or(stale)?;
    let [top, entry] = tables.as_slice() else {
        return Err(stale);
    };
    let version_ok =
        top.entries.len() == 1 && matches!(top.entries.get("version"), Some(Value::Integer(3 | 4)));
    let package_ok = entry.array
        && entry.path == ["package"]
        && entry.entries.len() == 2
        && entry.entries.get("name").and_then(string) == Some(package.name.as_str())
        && entry.entries.get("version").and_then(string) == Some(package.version.as_str());
    if version_ok && package_ok {
        Ok(())
    } else {
        Err(stale)
    }
}

#[cfg(test)]
mod tests;
