use crate::WebAgentContext;
use nexus_connectors_core::rate_limit::{RateLimitDecision, RateLimiter};
use nexus_kernel::audit::{AuditTrail, EventType};
use nexus_kernel::config::load_config;
use nexus_kernel::errors::AgentError;
use nexus_kernel::firewall::{ContentOrigin, SemanticBoundary};
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashSet;
use std::process::{Command, Stdio};
use url::Url;

const BRAVE_SEARCH_ENDPOINT: &str = "https://api.search.brave.com/res/v1/web/search";
const DUCKDUCKGO_HTML_ENDPOINT: &str = "https://html.duckduckgo.com/html/";

/// Timeout for HTTP requests in seconds.
const REQUEST_TIMEOUT_SECS: u32 = 15;

/// User-Agent header for fallback DuckDuckGo requests.
const USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64; rv:120.0) Gecko/20100101 Firefox/120.0";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
    pub relevance_score: f32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BraveSearchRequest {
    pub endpoint: String,
    pub headers: Vec<(String, String)>,
    pub query: Vec<(String, String)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FallbackProvider {
    None,
    Bing,
    SerpApi,
}

pub struct WebSearchConnector {
    fallback_provider: FallbackProvider,
    brave_api_key: Option<String>,
    pub audit_trail: AuditTrail,
    rate_limiter: RateLimiter,
}

impl WebSearchConnector {
    pub fn new(fallback_provider: FallbackProvider) -> Self {
        Self::with_brave_api_key(fallback_provider, None)
    }

    pub fn with_brave_api_key(
        fallback_provider: FallbackProvider,
        brave_api_key: Option<String>,
    ) -> Self {
        let limiter = RateLimiter::new();
        limiter.configure("web.search.brave", 1, 1);

        Self {
            fallback_provider,
            brave_api_key,
            audit_trail: AuditTrail::new(),
            rate_limiter: limiter,
        }
    }

    pub fn build_brave_request(
        &self,
        query: &str,
        max_results: usize,
    ) -> Result<BraveSearchRequest, AgentError> {
        let api_key = self.resolve_brave_api_key()?;
        Ok(BraveSearchRequest {
            endpoint: BRAVE_SEARCH_ENDPOINT.to_string(),
            headers: vec![
                ("X-Subscription-Token".to_string(), api_key),
                ("Accept".to_string(), "application/json".to_string()),
            ],
            query: vec![
                ("q".to_string(), query.to_string()),
                ("count".to_string(), max_results.to_string()),
            ],
        })
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

        match self.rate_limiter.check("web.search.brave") {
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

        let request = self.build_brave_request(normalized.as_str(), max_results)?;
        let primary = self.execute_brave_search(&request);
        let mut results = match primary {
            Ok(results) => results,
            Err(error) => self.try_fallback(normalized.as_str(), max_results, error)?,
        };

        if results.len() > max_results {
            results.truncate(max_results);
        }

        let boundary = SemanticBoundary::new();
        for result in &mut results {
            result.snippet =
                boundary.sanitize_data(result.snippet.as_str(), ContentOrigin::SearchResult);
        }

        self.audit_trail.append_event(
            agent.agent_id,
            EventType::ToolCall,
            json!({
                "event": "web_search_query",
                "query": normalized,
                "max_results": max_results,
                "returned": results.len(),
                "fuel_cost": fuel_cost,
                "provider": if self.fallback_provider == FallbackProvider::None { "brave" } else { "brave+fallback" }
            }),
        )?;

        Ok(results)
    }

    fn execute_brave_search(
        &self,
        request: &BraveSearchRequest,
    ) -> Result<Vec<SearchResult>, AgentError> {
        // Build URL with query params
        let mut url = request.endpoint.clone();
        if !request.query.is_empty() {
            url.push('?');
            let params: Vec<String> = request
                .query
                .iter()
                .map(|(k, v)| format!("{}={}", urlencoded(k), urlencoded(v)))
                .collect();
            url.push_str(&params.join("&"));
        }

        let timeout_str = REQUEST_TIMEOUT_SECS.to_string();
        let mut cmd = Command::new("curl");
        cmd.args(["-sS", "-L", "--max-time", &timeout_str]);
        for (name, value) in &request.headers {
            cmd.arg("-H").arg(format!("{name}: {value}"));
        }
        cmd.arg(&url)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let output = cmd.output().map_err(|error| {
            AgentError::SupervisorError(format!("curl execution failed: {error}"))
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AgentError::SupervisorError(format!(
                "brave request failed: curl exit {}: {}",
                output.status.code().unwrap_or(-1),
                stderr.trim()
            )));
        }

        let body = String::from_utf8_lossy(&output.stdout);
        let payload: BraveSearchResponse = serde_json::from_str(&body).map_err(|error| {
            AgentError::SupervisorError(format!("brave response parse failed: {error}"))
        })?;

        let mut rank = 1.0_f32;
        let mut results = Vec::new();
        if let Some(web) = payload.web {
            for item in web.results {
                let snippet = item.description.unwrap_or_default();
                results.push(SearchResult {
                    title: item.title,
                    url: item.url,
                    snippet,
                    relevance_score: rank,
                });
                rank = (rank - 0.05).max(0.1);
            }
        }

        Ok(results)
    }

    fn try_fallback(
        &self,
        normalized: &str,
        max_results: usize,
        primary_error: AgentError,
    ) -> Result<Vec<SearchResult>, AgentError> {
        match self.execute_duckduckgo_search(normalized, max_results) {
            Ok(results) if !results.is_empty() => Ok(results),
            Ok(_) => Err(primary_error),
            Err(fallback_error) => Err(AgentError::SupervisorError(format!(
                "primary search failed: {primary_error}; fallback search failed: {fallback_error}"
            ))),
        }
    }

    fn execute_duckduckgo_search(
        &self,
        query: &str,
        max_results: usize,
    ) -> Result<Vec<SearchResult>, AgentError> {
        let url = format!("{}?q={}", DUCKDUCKGO_HTML_ENDPOINT, urlencoded(query));
        let body = curl_get(&url)?;
        Ok(parse_duckduckgo_results(&body, max_results))
    }

    fn resolve_brave_api_key(&self) -> Result<String, AgentError> {
        if let Some(key) = self.brave_api_key.as_deref() {
            let trimmed = key.trim();
            if !trimmed.is_empty() {
                return Ok(trimmed.to_string());
            }
        }

        if let Ok(env_key) = std::env::var("BRAVE_API_KEY") {
            let trimmed = env_key.trim();
            if !trimmed.is_empty() {
                return Ok(trimmed.to_string());
            }
        }

        let config = load_config()?;
        let trimmed = config.search.brave_api_key.trim();
        if trimmed.is_empty() {
            return Err(AgentError::SupervisorError(
                "Brave API key is missing. Configure search.brave_api_key in ~/.nexus/config.toml"
                    .to_string(),
            ));
        }
        Ok(trimmed.to_string())
    }
}

/// Perform a GET request via curl subprocess. Safe in async tokio contexts.
fn curl_get(url: &str) -> Result<String, AgentError> {
    let timeout_str = REQUEST_TIMEOUT_SECS.to_string();
    let output = Command::new("curl")
        .args([
            "-sS",
            "-L",
            "--max-time",
            &timeout_str,
            "-A",
            USER_AGENT,
            url,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| AgentError::SupervisorError(format!("curl execution failed: {error}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("timed out") || stderr.contains("Timeout") {
            return Err(AgentError::SupervisorError("Timeout".to_string()));
        }
        return Err(AgentError::SupervisorError(format!(
            "web request failed: curl exit {}: {}",
            output.status.code().unwrap_or(-1),
            stderr.trim()
        )));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Minimal percent-encoding for URL query parameters.
fn urlencoded(input: &str) -> String {
    let mut encoded = String::with_capacity(input.len() * 3);
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            b' ' => encoded.push('+'),
            _ => {
                encoded.push('%');
                encoded.push_str(&format!("{byte:02X}"));
            }
        }
    }
    encoded
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

#[derive(Debug, Deserialize)]
struct BraveSearchResponse {
    web: Option<BraveWebSection>,
}

#[derive(Debug, Deserialize)]
struct BraveWebSection {
    results: Vec<BraveWebResult>,
}

#[derive(Debug, Deserialize)]
struct BraveWebResult {
    title: String,
    url: String,
    description: Option<String>,
}

fn parse_duckduckgo_results(body: &str, max_results: usize) -> Vec<SearchResult> {
    let document = Html::parse_document(body);
    let Ok(result_selector) = Selector::parse(".result") else {
        return Vec::new();
    };
    let Ok(title_selector) = Selector::parse("a.result__a") else {
        return Vec::new();
    };
    let Ok(snippet_selector) = Selector::parse(".result__snippet") else {
        return Vec::new();
    };

    let mut results = Vec::new();
    for (index, result) in document.select(&result_selector).enumerate() {
        if results.len() >= max_results {
            break;
        }

        let Some(title_link) = result.select(&title_selector).next() else {
            continue;
        };
        let title = title_link.text().collect::<String>().trim().to_string();
        if title.is_empty() {
            continue;
        }

        let raw_url = title_link.value().attr("href").unwrap_or_default();
        let snippet = result
            .select(&snippet_selector)
            .next()
            .map(|value| value.text().collect::<String>().trim().to_string())
            .unwrap_or_default();

        results.push(SearchResult {
            title,
            url: resolve_duckduckgo_url(raw_url),
            snippet,
            relevance_score: (1.0 - (index as f32 * 0.05)).max(0.1),
        });
    }

    results
}

fn resolve_duckduckgo_url(href: &str) -> String {
    if href.starts_with("http://") || href.starts_with("https://") {
        return href.to_string();
    }
    if href.starts_with("//") {
        return format!("https:{href}");
    }

    let Some(base) = Url::parse("https://duckduckgo.com").ok() else {
        return href.to_string();
    };
    let Ok(joined) = base.join(href) else {
        return href.to_string();
    };
    if let Some((_, value)) = joined.query_pairs().find(|(key, _)| key == "uddg") {
        return value.into_owned();
    }

    joined.to_string()
}

#[cfg(test)]
mod tests {
    use super::{normalize_query, FallbackProvider, WebSearchConnector, BRAVE_SEARCH_ENDPOINT};
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
        let mut connector = WebSearchConnector::with_brave_api_key(
            FallbackProvider::Bing,
            Some("test-key".to_string()),
        );
        let mut context = WebAgentContext::new(Uuid::new_v4(), capability_set(&["web.read"]), 100);

        let result = connector.query(&mut context, "rust ecosystem", 5);
        assert_eq!(
            result,
            Err(AgentError::CapabilityDenied("web.search".to_string()))
        );
    }

    #[test]
    fn test_brave_search_request_format() {
        let connector = WebSearchConnector::with_brave_api_key(
            FallbackProvider::None,
            Some("brave-key-123".to_string()),
        );
        let request = connector.build_brave_request("rust agents", 7);
        assert!(request.is_ok());

        if let Ok(req) = request {
            assert_eq!(req.endpoint, BRAVE_SEARCH_ENDPOINT);
            assert!(req.headers.contains(&(
                "X-Subscription-Token".to_string(),
                "brave-key-123".to_string()
            )));
            assert!(req
                .query
                .contains(&("q".to_string(), "rust agents".to_string())));
            assert!(req.query.contains(&("count".to_string(), "7".to_string())));
        }
    }
}
