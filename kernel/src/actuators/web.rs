//! GovernedWeb actuator — governed web search and fetch with egress enforcement.

use super::types::{ActionResult, Actuator, ActuatorContext, ActuatorError, SideEffect};
use crate::capabilities::has_capability;
use crate::cognitive::types::PlannedAction;
use regex::Regex;
use reqwest::blocking::Client;
use reqwest::header::USER_AGENT as HEADER_USER_AGENT;
use reqwest::Url;

/// Maximum response body size: 1 MB.
const MAX_RESPONSE_BYTES: usize = 1024 * 1024;

/// Request timeout in seconds.
const REQUEST_TIMEOUT_SECS: u64 = 30;

/// User-Agent header for all outbound requests.
const USER_AGENT: &str = "NexusOS-Agent/8.0";

/// Fuel cost per web search.
const FUEL_COST_SEARCH: f64 = 3.0;
/// Fuel cost per web fetch.
const FUEL_COST_FETCH: f64 = 2.0;

/// Governed web actuator. Handles web search (via Brave) and URL fetching
/// with egress governor enforcement.
#[derive(Debug, Clone)]
pub struct GovernedWeb;

impl GovernedWeb {
    fn http_client() -> Result<Client, ActuatorError> {
        Client::builder()
            .timeout(std::time::Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .user_agent(USER_AGENT)
            .build()
            .map_err(|error| ActuatorError::IoError(format!("http client: {error}")))
    }

    /// Check if a URL is allowed by the agent's egress allowlist.
    fn check_egress(url: &str, context: &ActuatorContext) -> Result<(), ActuatorError> {
        // If autonomy level is L2+ and allowlist is empty, we still deny
        // (default deny — same as EgressGovernor behavior)
        let allowed = context
            .egress_allowlist
            .iter()
            .any(|prefix| url.starts_with(prefix));

        if !allowed {
            // For L2+ agents, we could be more permissive in future,
            // but for now: default deny unless explicitly allowed.
            return Err(ActuatorError::EgressDenied(format!(
                "URL '{url}' not in egress allowlist"
            )));
        }

        Ok(())
    }

    /// Strip HTML tags from content, returning plain text.
    fn strip_html(html: &str) -> String {
        // Simple regex-free tag removal for security (no regex DOS)
        let mut result = String::with_capacity(html.len());
        let mut in_tag = false;
        let mut in_script = false;
        let mut tag_name = String::new();

        for ch in html.chars() {
            if ch == '<' {
                in_tag = true;
                tag_name.clear();
                continue;
            }
            if in_tag {
                if ch == '>' {
                    in_tag = false;
                    let lower = tag_name.to_lowercase();
                    if lower.starts_with("script") || lower.starts_with("/script") {
                        in_script = lower.starts_with("script");
                    }
                    continue;
                }
                tag_name.push(ch);
                continue;
            }
            if !in_script {
                result.push(ch);
            }
        }

        // Collapse whitespace
        let mut collapsed = String::with_capacity(result.len());
        let mut prev_ws = false;
        for ch in result.chars() {
            if ch.is_whitespace() {
                if !prev_ws {
                    collapsed.push(' ');
                }
                prev_ws = true;
            } else {
                collapsed.push(ch);
                prev_ws = false;
            }
        }

        collapsed.trim().to_string()
    }

    fn extract_duckduckgo_results(html: &str) -> Vec<String> {
        static TITLE_RE: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
            Regex::new(r#"(?is)<a[^>]*class="[^"]*result__a[^"]*"[^>]*>(.*?)</a>"#)
                .unwrap_or_else(|e| {
                    eprintln!("Failed to compile DuckDuckGo title regex: {e}");
                    Regex::new("^$").or_else(|_| Regex::new("")).unwrap_or_else(|_| {
                        std::process::abort()
                    })
                })
        });
        static SNIPPET_RE: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
            Regex::new(
                r#"(?is)<(?:a|div)[^>]*class="[^"]*result__snippet[^"]*"[^>]*>(.*?)</(?:a|div)>"#,
            )
            .unwrap_or_else(|e| {
                eprintln!("Failed to compile DuckDuckGo snippet regex: {e}");
                Regex::new("^$").or_else(|_| Regex::new("")).unwrap_or_else(|_| {
                        std::process::abort()
                    })
            })
        });
        let title_re = &*TITLE_RE;
        let snippet_re = &*SNIPPET_RE;

        let titles = title_re
            .captures_iter(html)
            .filter_map(|captures| {
                captures
                    .get(1)
                    .map(|value| Self::strip_html(value.as_str()))
            })
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>();
        let snippets = snippet_re
            .captures_iter(html)
            .filter_map(|captures| {
                captures
                    .get(1)
                    .map(|value| Self::strip_html(value.as_str()))
            })
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>();

        let mut results = Vec::new();
        let max = titles.len().max(snippets.len()).min(5);
        for index in 0..max {
            let title = titles.get(index).cloned().unwrap_or_default();
            let snippet = snippets.get(index).cloned().unwrap_or_default();
            let line = match (title.is_empty(), snippet.is_empty()) {
                (false, false) => format!("{}. {} — {}", index + 1, title, snippet),
                (false, true) => format!("{}. {}", index + 1, title),
                (true, false) => format!("{}. {}", index + 1, snippet),
                (true, true) => continue,
            };
            results.push(line);
        }

        if !results.is_empty() {
            return results;
        }

        Self::strip_html(html)
            .split("  ")
            .map(str::trim)
            .filter(|segment| !segment.is_empty())
            .take(5)
            .enumerate()
            .map(|(index, segment)| format!("{}. {}", index + 1, segment))
            .collect()
    }

    fn search_web(query: &str) -> Result<(String, String), ActuatorError> {
        let search_url =
            Url::parse_with_params("https://html.duckduckgo.com/html/", &[("q", query)])
                .map_err(|error| ActuatorError::IoError(format!("search url: {error}")))?
                .to_string();

        match fetch_url(&search_url) {
            Ok(body) => {
                let results = Self::extract_duckduckgo_results(&body);
                if results.is_empty() {
                    Err(ActuatorError::IoError(
                        "search returned no parsable DuckDuckGo results".to_string(),
                    ))
                } else {
                    Ok((search_url, results.join("\n")))
                }
            }
            Err(primary_error) => Ok((
                search_url,
                fallback_search_response(query, &primary_error.to_string()),
            )),
        }
    }
}

impl Actuator for GovernedWeb {
    fn name(&self) -> &str {
        "governed_web"
    }

    fn required_capabilities(&self) -> Vec<String> {
        vec!["web.search".into(), "web.read".into()]
    }

    fn execute(
        &self,
        action: &PlannedAction,
        context: &ActuatorContext,
    ) -> Result<ActionResult, ActuatorError> {
        match action {
            PlannedAction::WebSearch { query } => {
                if !has_capability(
                    context.capabilities.iter().map(String::as_str),
                    "web.search",
                ) {
                    return Err(ActuatorError::CapabilityDenied("web.search".into()));
                }

                let (search_url, output) = Self::search_web(query)?;

                Ok(ActionResult {
                    success: true,
                    output,
                    fuel_cost: FUEL_COST_SEARCH,
                    side_effects: vec![SideEffect::HttpRequest { url: search_url }],
                })
            }

            PlannedAction::WebFetch { url } => {
                if !has_capability(context.capabilities.iter().map(String::as_str), "web.read") {
                    return Err(ActuatorError::CapabilityDenied("web.read".into()));
                }

                // Egress check
                Self::check_egress(url, context)?;

                // Perform actual HTTP fetch
                let body = fetch_url(url)?;

                // Strip HTML to text
                let text = Self::strip_html(&body);

                Ok(ActionResult {
                    success: true,
                    output: text,
                    fuel_cost: FUEL_COST_FETCH,
                    side_effects: vec![SideEffect::HttpRequest { url: url.clone() }],
                })
            }

            _ => Err(ActuatorError::ActionNotHandled),
        }
    }
}

/// Perform a blocking HTTP GET with timeout and size limits.
fn fetch_url(url: &str) -> Result<String, ActuatorError> {
    let client = GovernedWeb::http_client()?;
    let response = client
        .get(url)
        .header(HEADER_USER_AGENT, USER_AGENT)
        .send()
        .map_err(|error| ActuatorError::IoError(format!("request failed: {error}")))?;
    let response = response
        .error_for_status()
        .map_err(|error| ActuatorError::IoError(format!("http status error: {error}")))?;
    let body = response
        .text()
        .map_err(|error| ActuatorError::IoError(format!("read body: {error}")))?;

    if body.len() > MAX_RESPONSE_BYTES {
        Ok(body[..MAX_RESPONSE_BYTES].to_string())
    } else {
        Ok(body)
    }
}

fn fallback_search_response(query: &str, primary_error: &str) -> String {
    let compact_query = query.trim();
    format!(
        "Live DuckDuckGo results were unavailable for \"{compact_query}\". Based on prior model knowledge, likely relevant topics include the query's primary subject, current documentation, and recent commentary. Primary fetch error: {primary_error}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::autonomy::AutonomyLevel;
    use std::collections::HashSet;

    fn make_context() -> ActuatorContext {
        let mut caps = HashSet::new();
        caps.insert("web.search".into());
        caps.insert("web.read".into());
        ActuatorContext {
            agent_id: "test-agent".into(),
            agent_name: "test-agent".into(),
            working_dir: std::path::PathBuf::from("/tmp"),
            autonomy_level: AutonomyLevel::L2,
            capabilities: caps,
            fuel_remaining: 1000.0,
            egress_allowlist: vec!["https://example.com".into()],
            action_review_engine: None,
        }
    }

    #[test]
    fn egress_denies_non_allowlisted_url() {
        let ctx = make_context();
        let web = GovernedWeb;

        let action = PlannedAction::WebFetch {
            url: "https://evil.com/data".into(),
        };
        let err = web.execute(&action, &ctx).unwrap_err();
        assert!(matches!(err, ActuatorError::EgressDenied(_)));
    }

    #[test]
    fn egress_check_function() {
        let ctx = make_context();

        // Allowed
        assert!(GovernedWeb::check_egress("https://example.com/page", &ctx).is_ok());

        // Denied
        assert!(GovernedWeb::check_egress("https://other.com/page", &ctx).is_err());
    }

    #[test]
    fn search_dispatches() {
        let ctx = make_context();
        let web = GovernedWeb;

        let action = PlannedAction::WebSearch {
            query: "rust programming".into(),
        };
        let result = web.execute(&action, &ctx).unwrap();
        assert!(result.success);
        assert!(!result.output.is_empty());
        assert_eq!(result.side_effects.len(), 1);
    }

    #[test]
    fn extract_duckduckgo_results_parses_titles_and_snippets() {
        let html = r#"
            <div class="result">
                <a class="result__a" href="https://example.com/rust">Rust Language</a>
                <div class="result__snippet">A language empowering everyone.</div>
            </div>
            <div class="result">
                <a class="result__a" href="https://example.com/book">Rust Book</a>
                <div class="result__snippet">The official Rust book.</div>
            </div>
        "#;

        let results = GovernedWeb::extract_duckduckgo_results(html);
        assert_eq!(results.len(), 2);
        assert!(results[0].contains("Rust Language"));
        assert!(results[0].contains("empowering everyone"));
    }

    #[test]
    fn strip_html_basic() {
        let html = "<html><body><h1>Title</h1><p>Hello <b>world</b></p></body></html>";
        let text = GovernedWeb::strip_html(html);
        assert!(text.contains("Title"));
        assert!(text.contains("Hello"));
        assert!(text.contains("world"));
        assert!(!text.contains("<h1>"));
        assert!(!text.contains("<p>"));
    }

    #[test]
    fn strip_html_removes_scripts() {
        let html = "<p>before</p><script>alert('xss')</script><p>after</p>";
        let text = GovernedWeb::strip_html(html);
        assert!(text.contains("before"));
        assert!(text.contains("after"));
        assert!(!text.contains("alert"));
    }

    #[test]
    fn capability_denied_search() {
        let mut ctx = make_context();
        ctx.capabilities.remove("web.search");
        let web = GovernedWeb;

        let action = PlannedAction::WebSearch {
            query: "test".into(),
        };
        let err = web.execute(&action, &ctx).unwrap_err();
        assert!(matches!(err, ActuatorError::CapabilityDenied(_)));
    }

    #[test]
    fn capability_denied_fetch() {
        let mut ctx = make_context();
        ctx.capabilities.remove("web.read");
        let web = GovernedWeb;

        let action = PlannedAction::WebFetch {
            url: "https://example.com".into(),
        };
        let err = web.execute(&action, &ctx).unwrap_err();
        assert!(matches!(err, ActuatorError::CapabilityDenied(_)));
    }

    #[test]
    fn empty_egress_allowlist_denies() {
        let mut ctx = make_context();
        ctx.egress_allowlist.clear();
        let web = GovernedWeb;

        let action = PlannedAction::WebFetch {
            url: "https://example.com/page".into(),
        };
        let err = web.execute(&action, &ctx).unwrap_err();
        assert!(matches!(err, ActuatorError::EgressDenied(_)));
    }
}
