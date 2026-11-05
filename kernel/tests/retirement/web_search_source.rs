//! F3 source predicate preserved verbatim from m0_f3_web_search_guard.rs.
//! Kept through K11; the single fetch-only curl builder is not a search exception.
/// The production part of `web.rs`: everything before its test module.
fn production(source: &str) -> String {
    let source = source.replace("\r\n", "\n");
    source
        .split("#[cfg(test)]\nmod tests")
        .next()
        .unwrap()
        .to_string()
}

/// The span of the top-level `fn name(` item: up to its closing brace at
/// column 0.
fn top_level_fn(source: &str, name: &str) -> Option<std::ops::Range<usize>> {
    let start = source.find(&format!("\nfn {name}("))?;
    let end = start + source[start..].find("\n}\n")? + 3;
    Some(start..end)
}

/// The spans of every method `fn name(` with a body inside an impl block:
/// up to its closing brace at four spaces. A trait declaration
/// (`fn name(..) -> T;`) has no body and no span: its span would run on to
/// the next method's closing brace and hide a call there.
fn methods(source: &str, name: &str) -> Vec<std::ops::Range<usize>> {
    let needle = format!("\n    fn {name}(");
    let mut spans = Vec::new();
    let mut from = 0;
    while let Some(at) = source[from..].find(&needle) {
        let start = from + at;
        from = start + needle.len();
        let line_end = source[start + 1..]
            .find('\n')
            .map_or(source.len(), |i| start + 1 + i);
        if !source[start..line_end].trim_end().ends_with('{') {
            continue;
        }
        let Some(len) = source[start..].find("\n    }\n") else {
            break;
        };
        spans.push(start..start + len + 7);
    }
    spans
}

/// Every reason the production source still lets search reach a process.
pub fn search_process_violations(source: &str) -> Vec<String> {
    let prod = production(source);
    let mut violations = Vec::new();
    for banned in [
        "duckduckgo",
        "hnrss",
        "SEARXNG_URL",
        "searxng",
        "with curl",
        "ShellCommand",
    ] {
        if prod
            .to_ascii_lowercase()
            .contains(&banned.to_ascii_lowercase())
        {
            violations.push(format!("names {banned}"));
        }
    }
    // One process builder only: curl_get's, for WebFetch.
    let spawns: Vec<usize> = prod.match_indices("Command::new").map(|(i, _)| i).collect();
    let curl_get = top_level_fn(&prod, "curl_get");
    match (&curl_get, spawns.as_slice()) {
        (Some(span), [at]) if span.contains(at) => {}
        _ => violations.push(format!(
            "{} process builders, expected one inside curl_get",
            spawns.len()
        )),
    }
    // curl_get is called only from a fetch_content method.
    let fetches = methods(&prod, "fetch_content");
    for (at, _) in prod.match_indices("curl_get(") {
        let is_definition = prod[..at].ends_with("fn ");
        if !is_definition && !fetches.iter().any(|span| span.contains(&at)) {
            violations.push(format!(
                "curl_get called outside fetch_content at byte {at}"
            ));
        }
    }
    violations
}
