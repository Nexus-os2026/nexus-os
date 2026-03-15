//! GovernedWeb actuator — governed web search and fetch with egress enforcement.

use super::types::{ActionResult, Actuator, ActuatorContext, ActuatorError, SideEffect};
use crate::capabilities::has_capability;
use crate::cognitive::types::PlannedAction;

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

                // Web search would route through BraveSearchConnector.
                // Since the connector lives in connectors/web/ (not a kernel dep),
                // we return a structured placeholder that the runtime bridge fills in.
                // The actuator validates governance; the bridge provides the HTTP call.
                Ok(ActionResult {
                    success: true,
                    output: format!(
                        "{{\"query\":\"{}\",\"status\":\"search_dispatched\",\"note\":\"route through BraveSearchConnector\"}}",
                        query.replace('"', "\\\"")
                    ),
                    fuel_cost: FUEL_COST_SEARCH,
                    side_effects: vec![SideEffect::HttpRequest {
                        url: format!("brave-search://?q={query}"),
                    }],
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
    // Use a subprocess curl for simplicity (kernel has no reqwest dep).
    // This avoids adding a heavy HTTP client to the kernel crate.
    let output = std::process::Command::new("curl")
        .args([
            "-sS",
            "--max-time",
            &REQUEST_TIMEOUT_SECS.to_string(),
            "--max-filesize",
            &MAX_RESPONSE_BYTES.to_string(),
            "-A",
            USER_AGENT,
            "-L", // follow redirects
            url,
        ])
        .output()
        .map_err(|e| ActuatorError::IoError(format!("curl spawn: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ActuatorError::IoError(format!(
            "fetch failed (exit {}): {stderr}",
            output.status
        )));
    }

    let body = String::from_utf8_lossy(&output.stdout);
    if body.len() > MAX_RESPONSE_BYTES {
        Ok(body[..MAX_RESPONSE_BYTES].to_string())
    } else {
        Ok(body.to_string())
    }
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
            working_dir: std::path::PathBuf::from("/tmp"),
            autonomy_level: AutonomyLevel::L2,
            capabilities: caps,
            fuel_remaining: 1000.0,
            egress_allowlist: vec!["https://example.com".into()],
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
        assert!(result.output.contains("search_dispatched"));
        assert_eq!(result.side_effects.len(), 1);
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
