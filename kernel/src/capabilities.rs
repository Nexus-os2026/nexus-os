pub fn canonical_capability(capability: &str) -> &str {
    match capability {
        "shell.execute" => "process.exec",
        "web.fetch" => "web.read",
        "api.call" => "mcp.call",
        _ => capability,
    }
}

pub fn capability_matches(granted: &str, required: &str) -> bool {
    canonical_capability(granted) == canonical_capability(required)
}

/// Capabilities that are always allowed regardless of manifest.
/// `llm.query` is the agent's ability to reason — without it no agent can function.
const IMPLICIT_CAPABILITIES: &[&str] = &["llm.query"];

pub fn has_capability<'a, I>(capabilities: I, required: &str) -> bool
where
    I: IntoIterator<Item = &'a str>,
{
    let canonical = canonical_capability(required);
    if IMPLICIT_CAPABILITIES.contains(&canonical) {
        return true;
    }
    capabilities
        .into_iter()
        .any(|capability| capability_matches(capability, required))
}

#[cfg(test)]
mod tests {
    use super::{canonical_capability, capability_matches, has_capability};

    #[test]
    fn canonicalizes_legacy_aliases() {
        assert_eq!(canonical_capability("shell.execute"), "process.exec");
        assert_eq!(canonical_capability("web.fetch"), "web.read");
        assert_eq!(canonical_capability("api.call"), "mcp.call");
    }

    #[test]
    fn matches_canonical_and_legacy_capabilities() {
        assert!(capability_matches("process.exec", "shell.execute"));
        assert!(capability_matches("web.fetch", "web.read"));
        assert!(capability_matches("mcp.call", "api.call"));
    }

    #[test]
    fn finds_required_capability_in_mixed_sets() {
        let caps = ["fs.read", "web.fetch", "api.call"];
        assert!(has_capability(caps.iter().copied(), "web.read"));
        assert!(has_capability(caps.iter().copied(), "mcp.call"));
        assert!(!has_capability(caps.iter().copied(), "process.exec"));
    }

    #[test]
    fn llm_query_is_implicitly_allowed() {
        // llm.query is the agent's ability to reason — always allowed even with
        // an empty capability set.
        let empty: [&str; 0] = [];
        assert!(has_capability(empty.iter().copied(), "llm.query"));

        let basic = ["fs.read", "web.search"];
        assert!(has_capability(basic.iter().copied(), "llm.query"));
    }
}
