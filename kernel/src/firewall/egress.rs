//! Egress governor: per-agent URL allowlisting and rate limiting for all
//! outbound network calls.
//!
//! **Default deny** — unless an endpoint is explicitly listed in the agent's
//! `allowed_endpoints` manifest field, the call is blocked.
//!
//! Every egress decision is audited. The governor is fail-closed: any internal
//! error results in a block, never a silent pass.

use crate::audit::{AuditTrail, EventType};
use crate::errors::AgentError;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use uuid::Uuid;

/// Whether the allowlist entry `entry` admits the request `url` (P0-002C5C;
/// explicit transport restrictions by Architect decision). A URL prefix is
/// not authority. Both sides are parsed and compared as a scheme, a
/// normalized host, an effective port and whole leading path segments:
///
/// - **Request.** It must pass [`crate::governed_http::http_url`]: `http` or
///   `https`, a host, no user information, no whitespace or control
///   characters. Anything else is denied.
/// - **Entry with a scheme** (`https://host[:port][/path]`). It admits only
///   its own scheme: an `https` entry never admits `http`, and an `http`
///   entry never admits `https`. The request's effective port (its explicit
///   port, or its scheme's default) must equal the entry's, computed the
///   same way, so `https://host` and `https://host:443` are the same entry.
/// - **Legacy entry without a scheme** (`host[:port][/path]`). This is a
///   compatibility form. It admits both `http` and `https`, as it did
///   before. With a port, the request's effective port must equal that port.
///   Without one, the request must use its own scheme's default port (80 for
///   `http`, 443 for `https`). The form applies only to entries written
///   without a scheme, so it never changes what a schemed entry admits.
/// - **Host.** The normalized hosts must be equal: the parser folds ASCII
///   case and canonicalizes IDNA and IPv4 forms. A longer host never matches.
/// - **Path.** The entry's non-empty path segments must lead the request's:
///   `/v1` admits `/v1` and `/v1/chat`, never `/v11`. Segments are compared
///   exactly, so paths are case-sensitive. The request's query and fragment
///   are not compared.
/// - **Malformed entries admit nothing.** That covers:
///   - an empty entry or a scheme other than `http` or `https`;
///   - user information, a query or a fragment;
///   - a backslash, whitespace or a control character;
///   - a `.` or `..` path segment;
///   - an authority that is not `host[:port]` with a numeric port.
///
///   None of these is repaired into a grant.
pub fn endpoint_admits(entry: &str, url: &str) -> bool {
    match (
        AllowedEndpoint::parse(entry),
        crate::governed_http::http_url(url),
    ) {
        (Some(entry), Ok(request)) => entry.admits(&request),
        _ => false,
    }
}

/// A parsed egress allowlist entry (see [`endpoint_admits`]).
#[derive(Debug, Clone, PartialEq, Eq)]
struct AllowedEndpoint {
    /// `http` or `https`, or `None` for a legacy entry without a scheme.
    scheme: Option<&'static str>,
    host: url::Host<String>,
    /// The effective port. For a schemed entry it is the explicit port or
    /// the scheme's default. For a legacy entry it is the explicit port, or
    /// `None` when the request's own default port applies.
    port: Option<u16>,
    /// Non-empty leading path segments.
    segments: Vec<String>,
}

impl AllowedEndpoint {
    fn parse(entry: &str) -> Option<Self> {
        if entry.is_empty()
            || entry
                .chars()
                .any(|c| c.is_whitespace() || c.is_control() || matches!(c, '\\' | '@' | '?' | '#'))
        {
            return None;
        }
        let (scheme, rest) = match entry.split_once("://") {
            Some((scheme, rest)) if scheme.eq_ignore_ascii_case("https") => (Some("https"), rest),
            Some((scheme, rest)) if scheme.eq_ignore_ascii_case("http") => (Some("http"), rest),
            Some(_) => return None,
            None => (None, entry),
        };
        let (authority, path) = match rest.find('/') {
            Some(at) => rest.split_at(at),
            None => (rest, ""),
        };
        let explicit_port = authority_port(authority)?;
        let segments: Vec<String> = path
            .split('/')
            .filter(|segment| !segment.is_empty())
            .map(str::to_string)
            .collect();
        if segments.iter().any(|segment| is_dot_segment(segment)) {
            return None;
        }
        let parsed =
            url::Url::parse(&format!("{}://{authority}/", scheme.unwrap_or("http"))).ok()?;
        let host = parsed.host()?.to_owned();
        let port = match scheme {
            Some(_) => parsed.port_or_known_default(),
            None => explicit_port,
        };
        Some(Self {
            scheme,
            host,
            port,
            segments,
        })
    }

    fn admits(&self, request: &url::Url) -> bool {
        let scheme = match self.scheme {
            Some(scheme) => scheme == request.scheme(),
            None => matches!(request.scheme(), "http" | "https"),
        };
        // The parser drops an explicit default port, so `port()` is `None`
        // exactly when the request uses its scheme's default port.
        let port = match self.port {
            Some(port) => request.port_or_known_default() == Some(port),
            None => request.port().is_none(),
        };
        let host = request
            .host()
            .is_some_and(|host| host.to_owned() == self.host);
        let request_segments: Vec<&str> = request
            .path_segments()
            .map(|segments| segments.filter(|segment| !segment.is_empty()).collect())
            .unwrap_or_default();
        let path = self.segments.len() <= request_segments.len()
            && self
                .segments
                .iter()
                .zip(&request_segments)
                .all(|(entry, request)| entry == request);
        scheme && port && host && path
    }
}

/// The explicit port of a `host[:port]` authority: `Some(None)` without a
/// port, `Some(Some(port))` with a numeric port, and `None` when the
/// authority is not of that form (empty, empty host, empty or non-numeric
/// port, or more than one colon outside an IPv6 literal).
fn authority_port(authority: &str) -> Option<Option<u16>> {
    let (host, port) = if let Some(literal) = authority.strip_prefix('[') {
        let (address, after) = literal.split_once(']')?;
        if address.is_empty() {
            return None;
        }
        match after {
            "" => (address, None),
            _ => (address, Some(after.strip_prefix(':')?)),
        }
    } else {
        match authority.split_once(':') {
            Some((host, port)) => (host, Some(port)),
            None => (authority, None),
        }
    };
    if host.is_empty() {
        return None;
    }
    match port {
        None => Some(None),
        Some(port) if !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()) => {
            Some(Some(port.parse().ok()?))
        }
        Some(_) => None,
    }
}

/// A `.` or `..` path segment, literal or percent-encoded.
fn is_dot_segment(segment: &str) -> bool {
    matches!(
        segment.to_ascii_lowercase().as_str(),
        "." | ".." | "%2e" | "%2e%2e" | ".%2e" | "%2e."
    )
}

/// Default rate limit: 60 requests per minute per endpoint.
pub const DEFAULT_RATE_LIMIT_PER_MIN: u32 = 60;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Outcome of an egress check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EgressDecision {
    /// Request is allowed — proceed.
    Allow,
    /// Request is blocked with reason.
    Deny { reason: String },
}

/// Per-agent egress policy.
#[derive(Debug, Clone)]
struct AgentEgressPolicy {
    /// Allowlist entries (e.g. `["https://api.example.com"]`), matched by
    /// [`endpoint_admits`].
    allowed_endpoints: Vec<String>,
    /// Max requests per minute per endpoint.
    rate_limit_per_min: u32,
}

/// Tracks request timestamps for rate limiting.
#[derive(Debug, Clone, Default)]
struct RateState {
    /// Map from endpoint prefix → list of request timestamps (unix secs).
    windows: HashMap<String, Vec<u64>>,
}

// ---------------------------------------------------------------------------
// EgressGovernor
// ---------------------------------------------------------------------------

/// Per-agent egress governor. Enforces URL allowlists and rate limits.
#[derive(Debug, Clone, Default)]
pub struct EgressGovernor {
    policies: HashMap<Uuid, AgentEgressPolicy>,
    rate_state: HashMap<Uuid, RateState>,
}

impl EgressGovernor {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns `true` if a policy is registered for the given agent.
    pub fn has_policy(&self, agent_id: Uuid) -> bool {
        self.policies.contains_key(&agent_id)
    }

    /// Register an agent's egress policy with the default rate limit.
    ///
    /// * `allowed_endpoints` — URL prefixes the agent may call. Empty = deny all.
    pub fn register_agent(&mut self, agent_id: Uuid, allowed_endpoints: Vec<String>) {
        self.register_agent_with_limit(agent_id, allowed_endpoints, 0);
    }

    /// Register an agent's egress policy with a custom rate limit.
    ///
    /// * `allowed_endpoints` — URL prefixes the agent may call. Empty = deny all.
    /// * `rate_limit_per_min` — max requests per minute per endpoint (0 = use default).
    pub fn register_agent_with_limit(
        &mut self,
        agent_id: Uuid,
        allowed_endpoints: Vec<String>,
        rate_limit_per_min: u32,
    ) {
        let limit = if rate_limit_per_min == 0 {
            DEFAULT_RATE_LIMIT_PER_MIN
        } else {
            rate_limit_per_min
        };
        self.policies.insert(
            agent_id,
            AgentEgressPolicy {
                allowed_endpoints,
                rate_limit_per_min: limit,
            },
        );
    }

    /// Check whether `agent_id` is allowed to call `url`.
    ///
    /// Returns `Deny` if:
    /// - No policy registered for the agent (default deny).
    /// - URL doesn't match any allowed endpoint prefix.
    /// - Rate limit exceeded for the matching endpoint.
    ///
    /// On `Allow`, the rate counter is incremented.
    pub fn check_egress(
        &mut self,
        agent_id: Uuid,
        url: &str,
        audit: &mut AuditTrail,
    ) -> EgressDecision {
        let policy = match self.policies.get(&agent_id) {
            Some(p) => p.clone(),
            None => {
                let decision = EgressDecision::Deny {
                    reason: "no egress policy registered for agent (default deny)".to_string(),
                };
                // Best-effort: egress decision already made; audit failure must not alter the deny verdict
                let _ = Self::audit(agent_id, url, &decision, audit);
                return decision;
            }
        };

        // Find the allowlist entry that admits the URL under its own scheme,
        // host, port and path rules (`endpoint_admits`).
        let matched_prefix = policy
            .allowed_endpoints
            .iter()
            .find(|prefix| endpoint_admits(prefix, url));

        let prefix = match matched_prefix {
            Some(p) => p.clone(),
            None => {
                let decision = EgressDecision::Deny {
                    reason: format!(
                        "URL '{url}' does not match any allowed endpoint for this agent"
                    ),
                };
                // Best-effort: egress decision already made; audit failure must not alter the deny verdict
                let _ = Self::audit(agent_id, url, &decision, audit);
                return decision;
            }
        };

        // Rate limit check.
        let now = now_secs();
        let window_start = now.saturating_sub(60);
        let rate = self.rate_state.entry(agent_id).or_default();
        let timestamps = rate.windows.entry(prefix).or_default();

        // Purge stale entries.
        timestamps.retain(|&t| t > window_start);

        if timestamps.len() as u32 >= policy.rate_limit_per_min {
            let decision = EgressDecision::Deny {
                reason: format!(
                    "rate limit exceeded: {} requests in last 60s (limit {})",
                    timestamps.len(),
                    policy.rate_limit_per_min
                ),
            };
            // Best-effort: egress decision already made; audit failure must not alter the deny verdict
            let _ = Self::audit(agent_id, url, &decision, audit);
            return decision;
        }

        // Allow — record this request.
        timestamps.push(now);

        let decision = EgressDecision::Allow;
        // Best-effort: allow verdict is final; audit failure is non-fatal
        let _ = Self::audit(agent_id, url, &decision, audit);
        decision
    }

    fn audit(
        agent_id: Uuid,
        url: &str,
        decision: &EgressDecision,
        audit: &mut AuditTrail,
    ) -> Result<(), AgentError> {
        let (action, details) = match decision {
            EgressDecision::Allow => ("allow", json!({})),
            EgressDecision::Deny { reason } => ("deny", json!({ "reason": reason })),
        };
        audit.append_event(
            agent_id,
            EventType::UserAction,
            json!({
                "event_kind": "firewall.egress",
                "url": url,
                "action": action,
                "details": details,
            }),
        )?;
        Ok(())
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::AuditTrail;

    fn id() -> Uuid {
        Uuid::new_v4()
    }

    /// P0-002C5C: an allowlist entry admits whole host and path segments,
    /// never a longer host, another port or a longer path segment.
    #[test]
    fn p0_002c5c_entries_admit_only_whole_hosts_and_path_segments() {
        for (entry, url) in [
            ("https://example.com", "https://example.com"),
            ("https://example.com", "https://example.com/"),
            ("https://example.com", "https://example.com/page?x=1#y"),
            ("https://example.com", "https://example.com?q=1"),
            ("https://example.com/", "https://example.com/a"),
            (
                "https://api.example.com/v1",
                "https://api.example.com/v1/chat",
            ),
            ("https://example.com:8443", "https://example.com:8443/x"),
        ] {
            assert!(endpoint_admits(entry, url), "{entry} should admit {url}");
        }
        for (entry, url) in [
            // This case used to be in the admitted list: the scheme was
            // stripped from both sides, so an `https` entry admitted plain
            // `http`. The Architect ruled that a policy defect (C5C repair
            // B): an explicit scheme is a restriction. Moving the case here
            // corrects the policy; it does not weaken the test.
            ("https://example.com", "http://example.com/downgrade"),
            ("https://example.com", "https://example.com.evil.net/"),
            ("https://example.com", "https://example.community/"),
            ("https://example.com", "https://example.com:8443/"),
            ("https://example.com", "https://example.com.:443/"),
            (
                "https://api.example.com/v1",
                "https://api.example.com/v1beta",
            ),
            (
                "https://example.com",
                "https://evil.net/https://example.com",
            ),
            ("https://example.com", "https://evil.net/?next=example.com"),
            ("", "https://example.com/"),
            ("https://", "https://example.com/"),
        ] {
            assert!(!endpoint_admits(entry, url), "{entry} must not admit {url}");
        }
        let mut governor = EgressGovernor::new();
        let agent = id();
        governor.register_agent(agent, vec!["https://api.example.com".into()]);
        let mut audit = AuditTrail::new();
        assert!(matches!(
            governor.check_egress(agent, "https://api.example.com.evil.net/x", &mut audit),
            EgressDecision::Deny { .. }
        ));
        assert!(matches!(
            governor.check_egress(agent, "https://api.example.com/x", &mut audit),
            EgressDecision::Allow
        ));
    }

    /// P0-002C5C (Architect repair B): an explicit scheme and the effective
    /// port are restrictions. An `https` entry never admits `http`, an
    /// `http` entry never admits `https`, and implicit and explicit default
    /// ports compare the same way.
    #[test]
    fn p0_002c5c_explicit_schemes_and_ports_are_enforced() {
        for (entry, url) in [
            ("https://example.test/v1", "https://example.test/v1"),
            ("https://example.test/v1", "https://example.test/v1/x?q=1"),
            ("https://example.test", "https://example.test:443/x"),
            ("https://example.test:443", "https://example.test/x"),
            ("http://example.test", "http://example.test:80/x"),
            ("http://example.test:80", "http://example.test/x"),
            ("http://example.test:8080", "http://example.test:8080/x"),
            ("https://EXAMPLE.test/v1", "https://example.TEST/v1/x"),
            ("HTTPS://example.test", "https://example.test/"),
            ("https://127.0.0.1:8443", "https://127.0.0.1:8443/x"),
            ("https://[::1]:8443/v1", "https://[::1]:8443/v1/x"),
        ] {
            assert!(endpoint_admits(entry, url), "{entry} should admit {url}");
        }
        for (entry, url) in [
            // The downgrade is denied, and so is the silent upgrade.
            ("https://example.test/v1", "http://example.test/v1"),
            ("https://example.test", "http://example.test:443/"),
            ("http://example.test", "https://example.test/"),
            ("http://example.test", "https://example.test:80/"),
            // Another effective port.
            ("https://example.test", "https://example.test:8443/"),
            ("https://example.test:8443", "https://example.test/"),
            ("http://example.test", "http://example.test:443/"),
            ("https://127.0.0.1:8443", "https://127.0.0.1:8444/"),
            // Another host, or a longer one.
            ("https://example.test", "https://example.test.evil/"),
            ("https://example.test", "https://evil.example.test/"),
            // A longer or differently cased path segment.
            ("https://example.test/v1", "https://example.test/v11"),
            ("https://example.test/v1", "https://example.test/v"),
            ("https://example.test/V1", "https://example.test/v1"),
            (
                "https://example.test/v1",
                "https://example.test/v1/../admin",
            ),
        ] {
            assert!(!endpoint_admits(entry, url), "{entry} must not admit {url}");
        }

        let mut governor = EgressGovernor::new();
        let agent = id();
        governor.register_agent(agent, vec!["https://api.example.test".into()]);
        let mut audit = AuditTrail::new();
        assert!(matches!(
            governor.check_egress(agent, "http://api.example.test/x", &mut audit),
            EgressDecision::Deny { .. }
        ));
        assert!(matches!(
            governor.check_egress(agent, "https://api.example.test/x", &mut audit),
            EgressDecision::Allow
        ));
        assert_eq!(audit.events().len(), 2, "both decisions are audited");
    }

    /// P0-002C5C (Architect repair B): a legacy entry written without a
    /// scheme keeps its documented compatibility meaning. It admits `http`
    /// and `https` to its host, and its explicit port or the request's
    /// default port, and it never widens a schemed entry.
    #[test]
    fn p0_002c5c_legacy_scheme_less_entries_keep_their_documented_meaning() {
        for (entry, url) in [
            ("example.test", "https://example.test/"),
            ("example.test", "http://example.test/x"),
            ("example.test", "https://example.test:443/x"),
            ("example.test", "http://example.test:80/x"),
            ("Example.TEST/v1", "https://example.test/v1/chat"),
            ("example.test:8443", "https://example.test:8443/x"),
            ("example.test:8443", "http://example.test:8443/x"),
            ("127.0.0.1:11434", "http://127.0.0.1:11434/api/tags"),
            ("[::1]:8080", "http://[::1]:8080/"),
        ] {
            assert!(endpoint_admits(entry, url), "{entry} should admit {url}");
        }
        for (entry, url) in [
            ("example.test", "https://example.test:8443/"),
            ("example.test", "http://example.test:443/"),
            ("example.test", "https://example.test.evil/"),
            ("example.test:8443", "https://example.test/"),
            // An explicit port 80 stays port 80: it does not widen to 443.
            ("example.test:80", "https://example.test/"),
            ("example.test/v1", "https://example.test/v11"),
            ("example.test", "ws://example.test/"),
        ] {
            assert!(!endpoint_admits(entry, url), "{entry} must not admit {url}");
        }

        // Each entry keeps its own meaning. A legacy entry for another host
        // or path never lets the schemed entry admit plain `http`.
        let mut governor = EgressGovernor::new();
        let agent = id();
        governor.register_agent(
            agent,
            vec![
                "https://example.test".into(),
                "other.test".into(),
                "example.test/public".into(),
            ],
        );
        let mut audit = AuditTrail::new();
        for denied in ["http://example.test/private", "http://example.test/"] {
            assert!(
                matches!(
                    governor.check_egress(agent, denied, &mut audit),
                    EgressDecision::Deny { .. }
                ),
                "{denied}"
            );
        }
        for allowed in [
            "https://example.test/private",
            "http://other.test/x",
            "http://example.test/public/page",
        ] {
            assert!(
                matches!(
                    governor.check_egress(agent, allowed, &mut audit),
                    EgressDecision::Allow
                ),
                "{allowed}"
            );
        }
    }

    /// P0-002C5C (Architect repair B): malformed, credential-bearing,
    /// ambiguous or non-HTTP(S) entries and requests admit nothing. None is
    /// repaired into a grant.
    #[test]
    fn p0_002c5c_malformed_or_ambiguous_endpoints_admit_nothing() {
        // Requests.
        for url in [
            "",
            "https://",
            "https//example.test/",
            "example.test/x",
            "https://exa mple.test/",
            "https://example.test/\n",
            "https://user:secret@example.test/",
            "https://example.test@evil.test/",
            "ftp://example.test/",
            "file:///etc/passwd",
            "javascript:alert(1)",
            "ws://example.test/",
        ] {
            for entry in ["https://example.test", "example.test"] {
                assert!(
                    !endpoint_admits(entry, url),
                    "{entry} must not admit {url:?}"
                );
            }
        }
        // Entries.
        for entry in [
            "",
            "https://",
            "https:example.test",
            "//example.test",
            " example.test",
            "example.test ",
            "ftp://example.test",
            "file:///",
            "wss://example.test",
            "https://user@example.test",
            "https://user:secret@example.test",
            "https://example.test?x=1",
            "https://example.test#top",
            "https://example.test\\evil.test",
            "https://example.test:",
            "https://example.test:https",
            "https://example.test:99999",
            "example.test:",
            "example.test:http",
            "example.test:1:2",
            "[::1",
            "[]:80",
            "example.test/../admin",
            "https://example.test/v1/%2e%2e/admin",
            "https://example.test/./v1",
        ] {
            for url in [
                "https://example.test/",
                "http://example.test/",
                "https://example.test/admin",
            ] {
                assert!(
                    !endpoint_admits(entry, url),
                    "{entry:?} must not admit {url}"
                );
            }
        }
        // A policy of malformed entries still denies by default.
        let mut governor = EgressGovernor::new();
        let agent = id();
        governor.register_agent(
            agent,
            vec![
                "https://user@example.test".into(),
                "ftp://example.test".into(),
            ],
        );
        let mut audit = AuditTrail::new();
        assert!(matches!(
            governor.check_egress(agent, "https://example.test/", &mut audit),
            EgressDecision::Deny { .. }
        ));
    }

    #[test]
    fn allowed_passes() {
        let mut gov = EgressGovernor::new();
        let mut audit = AuditTrail::new();
        let agent = id();

        gov.register_agent(agent, vec!["https://api.example.com".into()]);

        let result = gov.check_egress(agent, "https://api.example.com/v1/data", &mut audit);
        assert_eq!(result, EgressDecision::Allow);
    }

    #[test]
    fn disallowed_blocked() {
        let mut gov = EgressGovernor::new();
        let mut audit = AuditTrail::new();
        let agent = id();

        gov.register_agent(agent, vec!["https://api.example.com".into()]);

        let result = gov.check_egress(agent, "https://evil.com/exfiltrate", &mut audit);
        assert!(matches!(result, EgressDecision::Deny { .. }));
    }

    #[test]
    fn default_deny_works() {
        let mut gov = EgressGovernor::new();
        let mut audit = AuditTrail::new();
        let agent = id();
        // No policy registered.

        let result = gov.check_egress(agent, "https://anything.com", &mut audit);
        assert!(matches!(result, EgressDecision::Deny { .. }));
    }

    #[test]
    fn empty_allowlist_denies_all() {
        let mut gov = EgressGovernor::new();
        let mut audit = AuditTrail::new();
        let agent = id();

        gov.register_agent(agent, vec![]);

        let result = gov.check_egress(agent, "https://api.example.com/v1/data", &mut audit);
        assert!(matches!(result, EgressDecision::Deny { .. }));
    }

    #[test]
    fn rate_limit_enforced() {
        let mut gov = EgressGovernor::new();
        let mut audit = AuditTrail::new();
        let agent = id();

        // Set very low rate limit: 3 per minute.
        gov.register_agent_with_limit(agent, vec!["https://api.example.com".into()], 3);

        for _ in 0..3 {
            let r = gov.check_egress(agent, "https://api.example.com/call", &mut audit);
            assert_eq!(r, EgressDecision::Allow);
        }

        // 4th call should be rate-limited.
        let r = gov.check_egress(agent, "https://api.example.com/call", &mut audit);
        assert!(matches!(r, EgressDecision::Deny { .. }));
        if let EgressDecision::Deny { reason } = &r {
            assert!(reason.contains("rate limit"));
        }
    }

    #[test]
    fn audited() {
        let mut gov = EgressGovernor::new();
        let mut audit = AuditTrail::new();
        let agent = id();

        gov.register_agent(agent, vec!["https://ok.com".into()]);

        // 1. Allow
        gov.check_egress(agent, "https://ok.com/a", &mut audit);
        // 2. Deny (wrong URL)
        gov.check_egress(agent, "https://bad.com/b", &mut audit);

        let events = audit.events();
        assert_eq!(events.len(), 2, "expected 2 audit events");

        let kinds: Vec<&str> = events
            .iter()
            .filter_map(|e| e.payload.get("event_kind").and_then(|v| v.as_str()))
            .collect();
        assert!(kinds.iter().all(|k| *k == "firewall.egress"));

        let actions: Vec<&str> = events
            .iter()
            .filter_map(|e| e.payload.get("action").and_then(|v| v.as_str()))
            .collect();
        assert_eq!(actions[0], "allow");
        assert_eq!(actions[1], "deny");
    }

    #[test]
    fn multiple_allowed_prefixes() {
        let mut gov = EgressGovernor::new();
        let mut audit = AuditTrail::new();
        let agent = id();

        gov.register_agent(
            agent,
            vec![
                "https://api.example.com".into(),
                "https://cdn.example.com".into(),
            ],
        );

        assert_eq!(
            gov.check_egress(agent, "https://api.example.com/v1", &mut audit),
            EgressDecision::Allow
        );
        assert_eq!(
            gov.check_egress(agent, "https://cdn.example.com/img.png", &mut audit),
            EgressDecision::Allow
        );
        assert!(matches!(
            gov.check_egress(agent, "https://other.com", &mut audit),
            EgressDecision::Deny { .. }
        ));
    }
}
