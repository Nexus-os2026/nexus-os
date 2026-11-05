//! Narrow declaration reader for the preregistered oracle inputs. Comments,
//! literals and function/macro bodies cannot stand in for real declarations.
//! Attribute/scope equality preserves the registered cfg and ignore policy;
//! this is deliberately stricter than guessing whether arbitrary cfg is enabled.
use crate::source;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Contract {
    pub scopes: Vec<String>,
    pub attributes: Vec<String>,
    pub head: String,
}

#[derive(Clone)]
pub struct Declaration {
    pub name: String,
    pub kind: String,
    pub contract: Contract,
    pub span: (usize, usize),
}

fn attribute_text(text: &str) -> String {
    // The registered attributes use ordinary quoted strings. Do not normalize
    // an unregistered raw-string spelling; it cannot match their contract.
    if text.contains("r\"") || text.contains("r#") {
        return text.replace("\r\n", "\n");
    }
    let mut out = String::new();
    let mut quoted = false;
    let mut escaped = false;
    for c in text.chars() {
        if quoted || !c.is_whitespace() {
            out.push(c);
        }
        if c == '"' && !escaped {
            quoted = !quoted;
        }
        escaped = quoted && c == '\\' && !escaped;
    }
    out
}

pub fn declarations(raw: &str) -> Vec<Declaration> {
    fn scope(
        code: &str,
        text: &str,
        mut at: usize,
        end: usize,
        mut parents: Vec<String>,
        out: &mut Vec<Declaration>,
    ) {
        static HEAD: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
        let head = HEAD.get_or_init(|| regex::Regex::new(
            r"^(?:pub(?:\s*\([^)]*\))?\s+)?(?:(?:async|unsafe|const)\s+)*(fn|mod|struct|enum|type)\s+(?:r#)?(\w+)\b",
        ).unwrap());
        while at < end {
            while at < end && code.as_bytes()[at].is_ascii_whitespace() {
                at += 1;
            }
            let start = at;
            let mut attributes = Vec::new();
            while code.as_bytes().get(at) == Some(&b'#') {
                let inner = code.as_bytes().get(at + 1) == Some(&b'!');
                let rest = code[at + 1 + usize::from(inner)..].trim_start();
                if !rest.starts_with('[') {
                    break;
                }
                let open = code.len() - rest.len();
                let close = source::close_of(code, open);
                let attribute = attribute_text(&text[at..close]);
                if inner {
                    parents.push(attribute);
                } else {
                    attributes.push(attribute);
                }
                at = close;
                while at < end && code.as_bytes()[at].is_ascii_whitespace() {
                    at += 1;
                }
            }
            let item = at;
            while at < end && !matches!(code.as_bytes()[at], b'{' | b';') {
                if matches!(code.as_bytes()[at], b'(' | b'[') {
                    at = source::close_of(code, at);
                } else {
                    at += 1;
                }
            }
            if at >= end {
                break;
            }
            let body = (code.as_bytes()[at] == b'{').then_some(at);
            let close = body.map_or(at + 1, |open| source::close_of(code, open));
            let capture = head.captures(&code[item..at]);
            let implementation = code[item..at].starts_with("impl")
                && code[item + 4..at]
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_whitespace() || c == '<');
            if capture.is_some() || implementation {
                let contract = Contract {
                    scopes: parents.clone(),
                    attributes,
                    head: code[item..=at].split_whitespace().collect(),
                };
                let (kind, name) = capture.map_or_else(
                    || ("impl".into(), "<impl>".into()),
                    |c| (c[1].to_owned(), c[2].to_owned()),
                );
                if kind == "mod" || kind == "impl" {
                    if let Some(open) = body {
                        let mut parents = parents.clone();
                        parents.push(serde_json::to_string(&contract).unwrap());
                        scope(code, text, open + 1, close - 1, parents, out);
                    }
                }
                out.push(Declaration {
                    name,
                    kind,
                    contract,
                    span: (start, close),
                });
            }
            at = close;
        }
    }
    // Fixtures differ in one source at a time. Bound this exact-input cache so
    // exhaustive omission controls do not repeatedly parse unchanged subjects.
    type Cache = std::sync::Mutex<std::collections::BTreeMap<String, Vec<Declaration>>>;
    static CACHE: std::sync::OnceLock<Cache> = std::sync::OnceLock::new();
    let mut cache = CACHE.get_or_init(Cache::default).lock().unwrap();
    if let Some(declarations) = cache.get(raw) {
        return declarations.clone();
    }
    if cache.len() >= 64 {
        cache.clear();
    }
    let code = source::mask(raw, false);
    let text = source::mask(raw, true);
    let mut out = Vec::new();
    scope(&code, &text, 0, code.len(), Vec::new(), &mut out);
    cache.insert(raw.into(), out.clone());
    out
}

pub fn test_contract(raw: &str, name: &str) -> Option<Contract> {
    let mut matches = declarations(raw)
        .into_iter()
        .filter(|d| d.kind == "fn" && d.name == name);
    let declaration = matches.next()?;
    if matches.next().is_some()
        || !declaration
            .contract
            .attributes
            .iter()
            .any(|a| a == "#[test]" || a == "#[tokio::test]" || a.starts_with("#[tokio::test("))
    {
        return None;
    }
    Some(declaration.contract)
}

pub fn root_module(raw: &str, name: &str) -> Option<Contract> {
    let mut matches = declarations(raw)
        .into_iter()
        .filter(|d| d.kind == "mod" && d.name == name && d.contract.scopes.is_empty());
    let declaration = matches.next()?;
    matches.next().is_none().then_some(declaration.contract)
}

/// A live declaration's header, with no unregistered cfg restriction. This
/// does not accept the spelling inside a comment, literal or unused macro.
pub fn active_anchor(raw: &str, anchor: &str) -> bool {
    let anchor: String = anchor.split_whitespace().collect();
    let suffix = if anchor
        .chars()
        .last()
        .is_some_and(|c| c.is_alphanumeric() || c == '_')
    {
        r"\b"
    } else {
        ""
    };
    let header = regex::Regex::new(&format!("{}{suffix}", regex::escape(&anchor))).unwrap();
    declarations(raw).iter().any(|d| {
        header.is_match(&d.contract.head)
            && d.contract
                .attributes
                .iter()
                .all(|a| !a.starts_with("#[cfg"))
            && d.contract
                .scopes
                .iter()
                .all(|s| !s.contains("#[cfg") && !s.contains("#![cfg"))
    })
}
