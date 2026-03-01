use crate::WebAgentContext;
use nexus_connectors_core::rate_limit::{RateLimitDecision, RateLimiter};
use nexus_kernel::audit::{AuditTrail, EventType};
use nexus_kernel::errors::AgentError;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashSet;

const MIN_RELEVANCE_SCORE: f32 = 0.35;
const SPAM_DOMAINS: [&str; 4] = [
    "spam.example",
    "clickbait.invalid",
    "malware.test",
    "farm-content.local",
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
    pub relevance_score: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FallbackProvider {
    None,
    Bing,
    SerpApi,
}

pub struct WebSearchConnector {
    fallback_provider: FallbackProvider,
    brave_fail_mode: bool,
    pub audit_trail: AuditTrail,
    rate_limiter: RateLimiter,
}

impl WebSearchConnector {
    pub fn new(fallback_provider: FallbackProvider) -> Self {
        let limiter = RateLimiter::new();
        limiter.configure("web.search", 30, 60);

        Self {
            fallback_provider,
            brave_fail_mode: false,
            audit_trail: AuditTrail::new(),
            rate_limiter: limiter,
        }
    }

    pub fn set_brave_fail_mode(&mut self, enabled: bool) {
        self.brave_fail_mode = enabled;
    }

    pub fn query(
        &mut self,
        agent: &mut WebAgentContext,
        keywords: &str,
        max_results: usize,
    ) -> Result<Vec<SearchResult>, AgentError> {
        if !agent.has_capability("web.search") {
            return Err(AgentError::CapabilityDenied("web.search".to_string()));
        }

        let fuel_cost = (max_results as u64).max(1);
        if !agent.consume_fuel(fuel_cost) {
            return Err(AgentError::FuelExhausted);
        }

        match self.rate_limiter.check("web.search") {
            RateLimitDecision::Allowed => {}
            RateLimitDecision::RateLimited { retry_after_ms } => {
                return Err(AgentError::SupervisorError(format!(
                    "web.search rate limited, retry after {retry_after_ms} ms"
                )));
            }
        }

        let normalized = normalize_query(keywords);
        if normalized.is_empty() {
            return Err(AgentError::SupervisorError(
                "search query cannot be empty after normalization".to_string(),
            ));
        }

        let primary = if self.brave_fail_mode {
            Err(AgentError::SupervisorError(
                "Brave Search request failed".to_string(),
            ))
        } else {
            Ok(self.mock_provider_results("brave", normalized.as_str()))
        };

        let mut results = match primary {
            Ok(results) => results,
            Err(primary_error) => self.try_fallback(normalized.as_str(), primary_error)?,
        };

        results = self.apply_quality_filter(results);
        if results.len() > max_results {
            results.truncate(max_results);
        }

        let _ = self.audit_trail.append_event(
            agent.agent_id,
            EventType::ToolCall,
            json!({
                "event": "web_search_query",
                "query": normalized,
                "max_results": max_results,
                "returned": results.len(),
                "fuel_cost": fuel_cost
            }),
        );

        Ok(results)
    }

    fn try_fallback(
        &self,
        normalized: &str,
        primary_error: AgentError,
    ) -> Result<Vec<SearchResult>, AgentError> {
        match self.fallback_provider {
            FallbackProvider::None => Err(primary_error),
            FallbackProvider::Bing => Ok(self.mock_provider_results("bing", normalized)),
            FallbackProvider::SerpApi => Ok(self.mock_provider_results("serpapi", normalized)),
        }
    }

    fn mock_provider_results(&self, provider: &str, normalized: &str) -> Vec<SearchResult> {
        vec![
            SearchResult {
                title: format!("{provider} result: {normalized}"),
                url: "https://www.rust-lang.org/learn".to_string(),
                snippet: "Official Rust learning resources".to_string(),
                relevance_score: 0.95,
            },
            SearchResult {
                title: "Potentially spammy source".to_string(),
                url: "https://spam.example/cheap-clicks".to_string(),
                snippet: "Suspicious ad-heavy domain".to_string(),
                relevance_score: 0.92,
            },
            SearchResult {
                title: "Low relevance page".to_string(),
                url: "https://docs.example.org/unrelated".to_string(),
                snippet: "Mostly unrelated result".to_string(),
                relevance_score: 0.10,
            },
        ]
    }

    fn apply_quality_filter(&self, results: Vec<SearchResult>) -> Vec<SearchResult> {
        let spam: HashSet<&str> = SPAM_DOMAINS.iter().copied().collect();

        let mut filtered = Vec::new();
        for result in results {
            if result.relevance_score < MIN_RELEVANCE_SCORE {
                continue;
            }

            let domain = extract_domain(result.url.as_str());
            if spam.contains(domain.as_str()) {
                continue;
            }

            filtered.push(result);
        }

        filtered
    }
}

pub fn normalize_query(input: &str) -> String {
    let lowered = input.trim().to_lowercase();
    if lowered.is_empty() {
        return String::new();
    }

    let mut seen = HashSet::new();
    let mut normalized_words = Vec::new();
    for word in lowered.split_whitespace() {
        if seen.insert(word.to_string()) {
            normalized_words.push(word.to_string());
        }
    }

    normalized_words.join(" ")
}

fn extract_domain(url: &str) -> String {
    let no_scheme = if let Some(position) = url.find("://") {
        &url[position + 3..]
    } else {
        url
    };

    let host = no_scheme.split('/').next().unwrap_or_default();
    host.to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::{normalize_query, FallbackProvider, WebSearchConnector};
    use crate::WebAgentContext;
    use nexus_kernel::errors::AgentError;
    use std::collections::HashSet;
    use uuid::Uuid;

    fn capability_set(values: &[&str]) -> HashSet<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn test_search_query_normalization() {
        let normalized = normalize_query("  BEST  rust  BEST frameworks  ");
        assert_eq!(normalized, "best rust frameworks");
    }

    #[test]
    fn test_search_governed() {
        let mut connector = WebSearchConnector::new(FallbackProvider::Bing);
        let mut context = WebAgentContext::new(Uuid::new_v4(), capability_set(&["web.read"]), 100);

        let result = connector.query(&mut context, "rust ecosystem", 5);
        assert_eq!(
            result,
            Err(AgentError::CapabilityDenied("web.search".to_string()))
        );
    }
}
