//! Web crawling ethics engine — robots.txt parsing, rate limiting, and conduct enforcement.
//!
//! Ensures agents respect website boundaries by parsing robots.txt directives,
//! enforcing per-domain rate limits, and tracking conduct violations.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;
use uuid::Uuid;

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// ── Errors ──────────────────────────────────────────────────────────────

/// Errors from web conduct enforcement.
#[derive(Debug, Error)]
pub enum ConductViolation {
    #[error("rate limit exceeded for domain '{domain}': {requests_per_second:.1} req/s > {limit:.1} req/s")]
    RateLimitExceeded {
        domain: String,
        requests_per_second: f64,
        limit: f64,
    },
    #[error("domain '{0}' is blocked")]
    BlockedDomain(String),
    #[error("URL '{0}' is disallowed by robots.txt")]
    DisallowedByRobots(String),
}

// ── Types ───────────────────────────────────────────────────────────────

/// A parsed robots.txt rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RobotsRule {
    pub user_agent: String,
    pub allows: Vec<String>,
    pub disallows: Vec<String>,
    pub crawl_delay: Option<f64>,
}

/// Parsed robots.txt file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RobotsTxt {
    pub rules: Vec<RobotsRule>,
}

/// A recorded conduct violation event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViolationRecord {
    pub id: String,
    pub domain: String,
    pub violation_type: String,
    pub details: String,
    pub timestamp: u64,
}

/// Per-domain rate limit tracking.
#[derive(Debug, Clone)]
struct DomainRateState {
    request_timestamps: Vec<u64>,
    limit_rps: f64,
}

// ── Engine ──────────────────────────────────────────────────────────────

/// Web conduct engine enforcing ethical crawling behavior.
pub struct WebConductEngine {
    /// Parsed robots.txt per domain.
    robots: HashMap<String, RobotsTxt>,
    /// Rate limit state per domain.
    rate_limits: HashMap<String, DomainRateState>,
    /// Default requests per second for domains without explicit limits.
    default_rps: f64,
    /// Blocked domain set.
    blocked_domains: Vec<String>,
    /// Violation history.
    violations: Vec<ViolationRecord>,
}

impl Default for WebConductEngine {
    fn default() -> Self {
        Self::new(1.0)
    }
}

impl WebConductEngine {
    /// Create a new engine with the given default requests-per-second limit.
    pub fn new(default_rps: f64) -> Self {
        Self {
            robots: HashMap::new(),
            rate_limits: HashMap::new(),
            default_rps,
            blocked_domains: Vec::new(),
            violations: Vec::new(),
        }
    }

    /// Parse a robots.txt file content into structured rules.
    pub fn parse_robots_txt(content: &str) -> RobotsTxt {
        let mut rules: Vec<RobotsRule> = Vec::new();
        let mut current_ua: Option<String> = None;
        let mut allows: Vec<String> = Vec::new();
        let mut disallows: Vec<String> = Vec::new();
        let mut crawl_delay: Option<f64> = None;

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let parts: Vec<&str> = line.splitn(2, ':').collect();
            if parts.len() != 2 {
                continue;
            }

            let directive = parts[0].trim().to_lowercase();
            let value = parts[1].trim().to_string();

            match directive.as_str() {
                "user-agent" => {
                    // Save previous rule group if any.
                    if let Some(ua) = current_ua.take() {
                        rules.push(RobotsRule {
                            user_agent: ua,
                            allows: std::mem::take(&mut allows),
                            disallows: std::mem::take(&mut disallows),
                            crawl_delay: crawl_delay.take(),
                        });
                    }
                    current_ua = Some(value);
                }
                "allow" => allows.push(value),
                "disallow" => disallows.push(value),
                "crawl-delay" => {
                    crawl_delay = value.parse::<f64>().ok();
                }
                _ => {}
            }
        }

        // Save last rule group.
        if let Some(ua) = current_ua {
            rules.push(RobotsRule {
                user_agent: ua,
                allows,
                disallows,
                crawl_delay,
            });
        }

        RobotsTxt { rules }
    }

    /// Register parsed robots.txt for a domain.
    pub fn set_robots(&mut self, domain: &str, robots: RobotsTxt) {
        self.robots.insert(domain.to_string(), robots);
    }

    /// Check if a URL path is allowed for a given user-agent based on robots.txt.
    pub fn is_allowed(&self, domain: &str, path: &str, user_agent: &str) -> bool {
        let robots = match self.robots.get(domain) {
            Some(r) => r,
            None => return true, // No robots.txt → allowed.
        };

        // Find matching rule: exact user-agent match or wildcard.
        let rule = robots
            .rules
            .iter()
            .find(|r| r.user_agent == user_agent)
            .or_else(|| robots.rules.iter().find(|r| r.user_agent == "*"));

        let rule = match rule {
            Some(r) => r,
            None => return true,
        };

        // Standard robots.txt: longest matching path wins. If equal length, allow wins.
        let mut best_allow: Option<usize> = None;
        let mut best_disallow: Option<usize> = None;

        for allow in &rule.allows {
            if path.starts_with(allow.as_str()) {
                let len = allow.len();
                if best_allow.is_none() || len > best_allow.unwrap() {
                    best_allow = Some(len);
                }
            }
        }
        for disallow in &rule.disallows {
            if !disallow.is_empty() && path.starts_with(disallow.as_str()) {
                let len = disallow.len();
                if best_disallow.is_none() || len > best_disallow.unwrap() {
                    best_disallow = Some(len);
                }
            }
        }

        match (best_allow, best_disallow) {
            (Some(a), Some(d)) => a >= d, // Equal length: allow wins.
            (_, Some(_)) => false,         // Only disallow matched.
            _ => true,                     // Only allow matched or neither.
        }
    }

    /// Set a custom rate limit for a domain (requests per second).
    pub fn set_rate_limit(&mut self, domain: &str, rps: f64) {
        let state = self
            .rate_limits
            .entry(domain.to_string())
            .or_insert_with(|| DomainRateState {
                request_timestamps: Vec::new(),
                limit_rps: self.default_rps,
            });
        state.limit_rps = rps;
    }

    /// Check and record a request against rate limits.
    pub fn check_rate_limit(&mut self, domain: &str) -> Result<(), ConductViolation> {
        if self.blocked_domains.contains(&domain.to_string()) {
            return Err(ConductViolation::BlockedDomain(domain.to_string()));
        }

        let now = now_secs();
        let state = self
            .rate_limits
            .entry(domain.to_string())
            .or_insert_with(|| DomainRateState {
                request_timestamps: Vec::new(),
                limit_rps: self.default_rps,
            });

        // Remove timestamps older than 1 second.
        state
            .request_timestamps
            .retain(|&t| now.saturating_sub(t) < 2);

        let recent_count = state
            .request_timestamps
            .iter()
            .filter(|&&t| now.saturating_sub(t) < 1)
            .count();

        let current_rps = recent_count as f64;
        if current_rps >= state.limit_rps {
            return Err(ConductViolation::RateLimitExceeded {
                domain: domain.to_string(),
                requests_per_second: current_rps,
                limit: state.limit_rps,
            });
        }

        state.request_timestamps.push(now);
        Ok(())
    }

    /// Add a domain to the blocked list.
    pub fn block_domain(&mut self, domain: &str) {
        if !self.blocked_domains.contains(&domain.to_string()) {
            self.blocked_domains.push(domain.to_string());
        }
    }

    /// Remove a domain from the blocked list.
    pub fn unblock_domain(&mut self, domain: &str) {
        self.blocked_domains.retain(|d| d != domain);
    }

    /// Check if a domain is blocked.
    pub fn is_blocked(&self, domain: &str) -> bool {
        self.blocked_domains.contains(&domain.to_string())
    }

    /// Record a conduct violation for audit purposes.
    pub fn record_violation(&mut self, domain: &str, violation_type: &str, details: &str) {
        self.violations.push(ViolationRecord {
            id: Uuid::new_v4().to_string(),
            domain: domain.to_string(),
            violation_type: violation_type.to_string(),
            details: details.to_string(),
            timestamp: now_secs(),
        });
    }

    /// Get all recorded violations.
    pub fn get_violations(&self) -> &[ViolationRecord] {
        &self.violations
    }

    /// Get violations for a specific domain.
    pub fn get_domain_violations(&self, domain: &str) -> Vec<&ViolationRecord> {
        self.violations
            .iter()
            .filter(|v| v.domain == domain)
            .collect()
    }

    /// Total violation count.
    pub fn violation_count(&self) -> usize {
        self.violations.len()
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_ROBOTS: &str = r#"
User-agent: *
Disallow: /admin/
Disallow: /private/
Allow: /admin/public/
Crawl-delay: 2

User-agent: NexusBot
Disallow: /secret/
Allow: /
"#;

    #[test]
    fn test_parse_robots_txt() {
        let parsed = WebConductEngine::parse_robots_txt(SAMPLE_ROBOTS);
        assert_eq!(parsed.rules.len(), 2);

        let wildcard = &parsed.rules[0];
        assert_eq!(wildcard.user_agent, "*");
        assert_eq!(wildcard.disallows.len(), 2);
        assert_eq!(wildcard.allows.len(), 1);
        assert_eq!(wildcard.crawl_delay, Some(2.0));

        let nexus = &parsed.rules[1];
        assert_eq!(nexus.user_agent, "NexusBot");
        assert_eq!(nexus.disallows, vec!["/secret/"]);
    }

    #[test]
    fn test_parse_empty_robots() {
        let parsed = WebConductEngine::parse_robots_txt("");
        assert!(parsed.rules.is_empty());
    }

    #[test]
    fn test_parse_comments_only() {
        let parsed = WebConductEngine::parse_robots_txt("# This is a comment\n# Another");
        assert!(parsed.rules.is_empty());
    }

    #[test]
    fn test_is_allowed_no_robots() {
        let engine = WebConductEngine::new(1.0);
        assert!(engine.is_allowed("example.com", "/anything", "MyBot"));
    }

    #[test]
    fn test_is_allowed_disallowed_path() {
        let mut engine = WebConductEngine::new(1.0);
        let robots = WebConductEngine::parse_robots_txt(SAMPLE_ROBOTS);
        engine.set_robots("example.com", robots);

        assert!(!engine.is_allowed("example.com", "/admin/settings", "SomeBot"));
        assert!(!engine.is_allowed("example.com", "/private/data", "SomeBot"));
    }

    #[test]
    fn test_is_allowed_allow_overrides() {
        let mut engine = WebConductEngine::new(1.0);
        let robots = WebConductEngine::parse_robots_txt(SAMPLE_ROBOTS);
        engine.set_robots("example.com", robots);

        // /admin/public/ is explicitly allowed.
        assert!(engine.is_allowed("example.com", "/admin/public/page", "SomeBot"));
    }

    #[test]
    fn test_is_allowed_specific_agent() {
        let mut engine = WebConductEngine::new(1.0);
        let robots = WebConductEngine::parse_robots_txt(SAMPLE_ROBOTS);
        engine.set_robots("example.com", robots);

        // NexusBot has its own rules: /secret/ disallowed, everything else allowed.
        assert!(!engine.is_allowed("example.com", "/secret/file", "NexusBot"));
        assert!(engine.is_allowed("example.com", "/admin/settings", "NexusBot"));
    }

    #[test]
    fn test_is_allowed_root_path() {
        let mut engine = WebConductEngine::new(1.0);
        let robots = WebConductEngine::parse_robots_txt(SAMPLE_ROBOTS);
        engine.set_robots("example.com", robots);

        assert!(engine.is_allowed("example.com", "/", "SomeBot"));
        assert!(engine.is_allowed("example.com", "/public/page", "SomeBot"));
    }

    #[test]
    fn test_block_unblock_domain() {
        let mut engine = WebConductEngine::new(1.0);
        assert!(!engine.is_blocked("evil.com"));

        engine.block_domain("evil.com");
        assert!(engine.is_blocked("evil.com"));

        // Duplicate add should be idempotent.
        engine.block_domain("evil.com");
        assert_eq!(engine.blocked_domains.len(), 1);

        engine.unblock_domain("evil.com");
        assert!(!engine.is_blocked("evil.com"));
    }

    #[test]
    fn test_rate_limit_blocked_domain() {
        let mut engine = WebConductEngine::new(10.0);
        engine.block_domain("blocked.com");

        let result = engine.check_rate_limit("blocked.com");
        assert!(result.is_err());
        match result.unwrap_err() {
            ConductViolation::BlockedDomain(d) => assert_eq!(d, "blocked.com"),
            other => panic!("expected BlockedDomain, got: {other}"),
        }
    }

    #[test]
    fn test_rate_limit_first_request_ok() {
        let mut engine = WebConductEngine::new(10.0);
        assert!(engine.check_rate_limit("example.com").is_ok());
    }

    #[test]
    fn test_record_violation() {
        let mut engine = WebConductEngine::new(1.0);
        engine.record_violation("example.com", "robots_violation", "Accessed /admin/");
        engine.record_violation("other.com", "rate_limit", "Exceeded 5 req/s");

        assert_eq!(engine.violation_count(), 2);
        assert_eq!(engine.get_domain_violations("example.com").len(), 1);
        assert_eq!(engine.get_domain_violations("other.com").len(), 1);
        assert_eq!(engine.get_domain_violations("ghost.com").len(), 0);
    }

    #[test]
    fn test_violation_record_fields() {
        let mut engine = WebConductEngine::new(1.0);
        engine.record_violation("test.com", "type1", "details1");

        let v = &engine.get_violations()[0];
        assert_eq!(v.domain, "test.com");
        assert_eq!(v.violation_type, "type1");
        assert_eq!(v.details, "details1");
        assert!(!v.id.is_empty());
        assert!(v.timestamp > 0);
    }

    #[test]
    fn test_set_custom_rate_limit() {
        let mut engine = WebConductEngine::new(1.0);
        engine.set_rate_limit("fast.com", 100.0);

        // Should succeed many times at high limit.
        for _ in 0..50 {
            assert!(engine.check_rate_limit("fast.com").is_ok());
        }
    }
}
