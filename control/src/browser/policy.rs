use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DomainPolicy {
    pub allowed_domains: Vec<String>,
}

impl DomainPolicy {
    pub fn new(allowed_domains: Vec<String>) -> Self {
        let mut policy = Self { allowed_domains };
        policy.normalize();
        policy
    }

    pub fn is_allowed_url(&self, url: &str) -> bool {
        let Some(host) = extract_host(url) else {
            return false;
        };

        self.allowed_domains
            .iter()
            .any(|pattern| domain_matches(pattern.as_str(), host.as_str()))
    }

    fn normalize(&mut self) {
        self.allowed_domains = self
            .allowed_domains
            .iter()
            .map(|domain| domain.trim().to_lowercase())
            .filter(|domain| !domain.is_empty())
            .collect::<Vec<_>>();
        self.allowed_domains.sort();
        self.allowed_domains.dedup();
    }
}

pub fn extract_host(url: &str) -> Option<String> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return None;
    }

    let without_scheme = match trimmed.find("://") {
        Some(index) => &trimmed[(index + 3)..],
        None => trimmed,
    };

    let host_port = without_scheme
        .split('/')
        .next()
        .unwrap_or_default()
        .split('?')
        .next()
        .unwrap_or_default()
        .split('#')
        .next()
        .unwrap_or_default();

    let host = host_port
        .split('@')
        .next_back()
        .unwrap_or_default()
        .split(':')
        .next()
        .unwrap_or_default()
        .trim()
        .to_lowercase();

    if host.is_empty() {
        None
    } else {
        Some(host)
    }
}

pub fn domain_matches(pattern: &str, host: &str) -> bool {
    let normalized_pattern = pattern.trim().to_lowercase();
    let normalized_host = host.trim().to_lowercase();

    if normalized_pattern.is_empty() || normalized_host.is_empty() {
        return false;
    }

    if let Some(base_domain) = normalized_pattern.strip_prefix("*.") {
        return normalized_host == base_domain
            || normalized_host.ends_with(format!(".{base_domain}").as_str());
    }

    normalized_host == normalized_pattern
}

#[cfg(test)]
mod tests {
    use super::{domain_matches, extract_host, DomainPolicy};

    #[test]
    fn test_domain_policy_matching() {
        let policy = DomainPolicy::new(vec!["*.github.com".to_string(), "example.com".to_string()]);
        assert!(policy.is_allowed_url("https://github.com/nex-lang"));
        assert!(policy.is_allowed_url("https://docs.github.com/en"));
        assert!(policy.is_allowed_url("https://example.com/home"));
        assert!(!policy.is_allowed_url("https://evil.com"));
    }

    #[test]
    fn test_domain_match_helper() {
        assert!(domain_matches("*.github.com", "github.com"));
        assert!(domain_matches("*.github.com", "api.github.com"));
        assert!(!domain_matches("*.github.com", "evilgithub.com"));
    }

    #[test]
    fn test_extract_host() {
        assert_eq!(
            extract_host("https://github.com/nex-lang"),
            Some("github.com".to_string())
        );
        assert_eq!(
            extract_host("http://user:pass@example.com:8080/path"),
            Some("example.com".to_string())
        );
    }
}
