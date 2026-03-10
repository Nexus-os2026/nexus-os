use crate::WebAgentContext;
use nexus_connectors_core::rate_limit::{RateLimitDecision, RateLimiter};
use nexus_kernel::audit::{AuditTrail, EventType};
use nexus_kernel::errors::AgentError;
use nexus_kernel::firewall::{ContentOrigin, SemanticBoundary};
use regex::Regex;
use reqwest::blocking::Client;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use url::Url;

const DEFAULT_MAX_CONTENT_CHARS: usize = 50_000;
const DEFAULT_TIMEOUT_SECONDS: u64 = 10;
const TRUNCATION_MARKER: &str = "[truncated]";
const ROBOTS_BLOCKED_ERROR: &str = "RobotsTxtBlocked";
const TIMEOUT_ERROR: &str = "Timeout";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CleanContent {
    pub title: String,
    pub text: String,
    pub word_count: usize,
    pub source_url: String,
    pub extracted_at: u64,
}

pub struct WebReaderConnector {
    max_content_chars: usize,
    timeout: Duration,
    mock_pages: HashMap<String, String>,
    mock_robots: HashMap<String, String>,
    mock_delays: HashMap<String, Duration>,
    pub audit_trail: AuditTrail,
    rate_limiter: RateLimiter,
    http_client: Client,
}

impl WebReaderConnector {
    pub fn new(max_content_chars: Option<usize>) -> Self {
        Self::with_timeout_seconds(max_content_chars, DEFAULT_TIMEOUT_SECONDS)
    }

    pub fn with_timeout_seconds(max_content_chars: Option<usize>, timeout_seconds: u64) -> Self {
        let limiter = RateLimiter::new();
        limiter.configure("web.read", 30, 60);
        let timeout = Duration::from_secs(timeout_seconds);

        let client = Client::builder()
            .timeout(timeout)
            .build()
            .unwrap_or_else(|_| Client::new());

        Self {
            max_content_chars: max_content_chars.unwrap_or(DEFAULT_MAX_CONTENT_CHARS),
            timeout,
            mock_pages: HashMap::new(),
            mock_robots: HashMap::new(),
            mock_delays: HashMap::new(),
            audit_trail: AuditTrail::new(),
            rate_limiter: limiter,
            http_client: client,
        }
    }

    pub fn add_mock_page(&mut self, url: &str, html: &str) {
        self.mock_pages.insert(url.to_string(), html.to_string());
    }

    pub fn add_mock_robots_txt(&mut self, base_url: &str, robots_txt: &str) {
        self.mock_robots.insert(
            base_url.trim_end_matches('/').to_string(),
            robots_txt.to_string(),
        );
    }

    pub fn add_mock_delay(&mut self, url: &str, delay: Duration) {
        self.mock_delays.insert(url.to_string(), delay);
    }

    pub fn fetch_and_extract(
        &mut self,
        agent: &mut WebAgentContext,
        url: &str,
    ) -> Result<CleanContent, AgentError> {
        if !agent.has_capability("web.read") {
            return Err(AgentError::CapabilityDenied("web.read".to_string()));
        }

        let fuel_cost = 25_u64;
        if !agent.consume_fuel(fuel_cost) {
            return Err(AgentError::FuelExhausted);
        }

        match self.rate_limiter.check("web.read") {
            RateLimitDecision::Allowed => {}
            RateLimitDecision::RateLimited { retry_after_ms } => {
                return Err(AgentError::SupervisorError(format!(
                    "web.read rate limited, retry after {retry_after_ms} ms"
                )));
            }
        }

        self.ensure_robots_allowed(url)?;
        let html = self.fetch_page_html(url)?;

        let title = extract_title(html.as_str());
        let mut text = extract_readable_text(html.as_str());
        text = strip_personal_data(text.as_str());
        text = enforce_size_limit(text.as_str(), self.max_content_chars);

        let word_count = text.split_whitespace().count();
        let extracted_at = current_unix_timestamp();

        let boundary = SemanticBoundary::new();
        text = boundary.sanitize_data(text.as_str(), ContentOrigin::WebContent);

        let clean = CleanContent {
            title,
            text,
            word_count,
            source_url: url.to_string(),
            extracted_at,
        };

        self.audit_trail.append_event(
            agent.agent_id,
            EventType::ToolCall,
            json!({
                "event": "web_read_extract",
                "source_url": url,
                "word_count": clean.word_count,
                "char_count": clean.text.chars().count(),
                "fuel_cost": fuel_cost
            }),
        )?;

        Ok(clean)
    }

    fn fetch_page_html(&self, url: &str) -> Result<String, AgentError> {
        if let Some(delay) = self.mock_delays.get(url) {
            if *delay > self.timeout {
                return Err(AgentError::SupervisorError(TIMEOUT_ERROR.to_string()));
            }
            std::thread::sleep(*delay);
        }

        if let Some(mock) = self.mock_pages.get(url) {
            return Ok(mock.clone());
        }

        let response = self
            .http_client
            .get(url)
            .send()
            .map_err(map_request_error)?;
        if !response.status().is_success() {
            return Err(AgentError::SupervisorError(format!(
                "web fetch failed with status {}",
                response.status()
            )));
        }
        response.text().map_err(|error| {
            AgentError::SupervisorError(format!("failed to read page body: {error}"))
        })
    }

    fn ensure_robots_allowed(&self, target_url: &str) -> Result<(), AgentError> {
        let parsed = Url::parse(target_url).map_err(|error| {
            AgentError::SupervisorError(format!("invalid URL '{target_url}': {error}"))
        })?;
        let Some(host) = parsed.host_str() else {
            return Err(AgentError::SupervisorError("URL host missing".to_string()));
        };
        let base = format!("{}://{}", parsed.scheme(), host);
        let robots_url = format!("{base}/robots.txt");

        let robots_text = if let Some(mock) = self.mock_robots.get(base.as_str()) {
            Some(mock.clone())
        } else {
            match self.http_client.get(robots_url).send() {
                Ok(response) if response.status().is_success() => response.text().ok(),
                Ok(_) => None,
                Err(error) => {
                    if error.is_timeout() {
                        return Err(AgentError::SupervisorError(TIMEOUT_ERROR.to_string()));
                    }
                    None
                }
            }
        };

        if let Some(robots) = robots_text {
            let path = parsed.path();
            if robots_disallow_path(robots.as_str(), path) {
                return Err(AgentError::SupervisorError(
                    ROBOTS_BLOCKED_ERROR.to_string(),
                ));
            }
        }
        Ok(())
    }
}

fn map_request_error(error: reqwest::Error) -> AgentError {
    if error.is_timeout() {
        return AgentError::SupervisorError(TIMEOUT_ERROR.to_string());
    }
    AgentError::SupervisorError(format!("web request failed: {error}"))
}

fn robots_disallow_path(robots_txt: &str, path: &str) -> bool {
    let mut applies = false;
    for raw_line in robots_txt.lines() {
        let line = raw_line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let lower = line.to_ascii_lowercase();
        if let Some(value) = lower.strip_prefix("user-agent:") {
            applies = value.trim() == "*";
            continue;
        }
        if !applies {
            continue;
        }
        if let Some(value) = line.strip_prefix("Disallow:") {
            let rule = value.trim();
            if rule.is_empty() {
                continue;
            }
            if rule == "/" || path.starts_with(rule) {
                return true;
            }
        }
    }
    false
}

fn extract_title(html: &str) -> String {
    let lower = html.to_lowercase();
    let start = lower.find("<title>");
    let end = lower.find("</title>");

    match (start, end) {
        (Some(start_index), Some(end_index)) if end_index > start_index + 7 => {
            let raw = &html[start_index + 7..end_index];
            sanitize_whitespace(raw)
        }
        _ => "Untitled".to_string(),
    }
}

fn extract_readable_text(html: &str) -> String {
    let mut cleaned = html.to_string();
    for tag in ["script", "style", "nav", "header", "footer"] {
        cleaned = remove_tag_blocks(cleaned.as_str(), tag);
    }

    if let Ok(ad_block) = Regex::new(
        r#"(?is)<(?:div|section|aside)[^>]*(?:class|id)\s*=\s*["'][^"']*(?:ad|advert|promo)[^"']*["'][^>]*>.*?</(?:div|section|aside)>"#,
    ) {
        cleaned = ad_block.replace_all(&cleaned, " ").into_owned();
    }

    let doc = Html::parse_document(cleaned.as_str());
    let selectors = [
        Selector::parse("article").ok(),
        Selector::parse("main").ok(),
        Selector::parse("p").ok(),
    ];

    let mut pieces = Vec::new();
    for selector in selectors.into_iter().flatten() {
        for node in doc.select(&selector) {
            let text = node.text().collect::<Vec<_>>().join(" ");
            let normalized = sanitize_whitespace(text.as_str());
            if !normalized.is_empty() {
                pieces.push(normalized);
            }
        }
    }

    if pieces.is_empty() {
        if let Ok(body_selector) = Selector::parse("body") {
            for node in doc.select(&body_selector) {
                let text = node.text().collect::<Vec<_>>().join(" ");
                let normalized = sanitize_whitespace(text.as_str());
                if !normalized.is_empty() {
                    pieces.push(normalized);
                }
            }
        }
    }

    sanitize_whitespace(pieces.join(" ").as_str())
}

fn remove_tag_blocks(input: &str, tag: &str) -> String {
    let pattern = format!(r"(?is)<{tag}[^>]*>.*?</{tag}>");
    if let Ok(regex) = Regex::new(pattern.as_str()) {
        regex.replace_all(input, " ").into_owned()
    } else {
        input.to_string()
    }
}

fn strip_personal_data(input: &str) -> String {
    let mut output = input.to_string();

    if let Ok(email_re) = Regex::new(r"(?i)\b[a-z0-9._%+-]+@[a-z0-9.-]+\.[a-z]{2,}\b") {
        output = email_re
            .replace_all(&output, "[redacted-email]")
            .into_owned();
    }

    if let Ok(phone_re) = Regex::new(r"\b(?:\+?\d[\d\s\-\(\)]{8,}\d)\b") {
        output = phone_re
            .replace_all(&output, "[redacted-phone]")
            .into_owned();
    }

    sanitize_whitespace(output.as_str())
}

fn enforce_size_limit(input: &str, max_chars: usize) -> String {
    let chars = input.chars().collect::<Vec<_>>();
    if chars.len() <= max_chars {
        return input.to_string();
    }

    let marker_chars = TRUNCATION_MARKER.chars().count();
    if max_chars <= marker_chars {
        return TRUNCATION_MARKER
            .chars()
            .take(max_chars)
            .collect::<String>();
    }

    let keep = max_chars - marker_chars;
    let mut truncated = chars.into_iter().take(keep).collect::<String>();
    truncated.push_str(TRUNCATION_MARKER);
    truncated
}

fn sanitize_whitespace(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn current_unix_timestamp() -> u64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_secs(),
        Err(_) => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::WebReaderConnector;
    use crate::WebAgentContext;
    use std::collections::HashSet;
    use std::time::Duration;
    use uuid::Uuid;

    fn capability_set(values: &[&str]) -> HashSet<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn test_html_extraction() {
        let url = "https://example.com/article";
        let html = r#"
<html>
  <head>
    <title>Rust Article</title>
    <script>stealCookies();</script>
  </head>
  <body>
    <nav>Home | About</nav>
    <div class="ad">Buy now!</div>
    <main>
      <article>
        <p>Rust makes systems programming safer.</p>
      </article>
    </main>
    <footer>Copyright text</footer>
  </body>
</html>
"#;

        let mut connector = WebReaderConnector::new(None);
        connector.add_mock_page(url, html);
        let mut context = WebAgentContext::new(Uuid::new_v4(), capability_set(&["web.read"]), 500);

        let result = connector.fetch_and_extract(&mut context, url);
        assert!(result.is_ok());

        if let Ok(content) = result {
            assert!(content
                .text
                .contains("Rust makes systems programming safer."));
            assert!(!content.text.contains("Home | About"));
            assert!(!content.text.contains("Buy now!"));
            assert!(!content.text.contains("stealCookies"));
        }
    }

    #[test]
    fn test_robots_txt_respected() {
        let mut connector = WebReaderConnector::new(None);
        connector.add_mock_page(
            "https://example.com/private/page",
            "<html><body><article>hidden</article></body></html>",
        );
        connector.add_mock_robots_txt("https://example.com", "User-agent: *\nDisallow: /private");
        let mut context = WebAgentContext::new(Uuid::new_v4(), capability_set(&["web.read"]), 500);

        let result = connector.fetch_and_extract(&mut context, "https://example.com/private/page");
        assert!(result.is_err());
        if let Err(error) = result {
            assert!(error.to_string().contains("RobotsTxtBlocked"));
        }
    }

    #[test]
    fn test_timeout_enforcement() {
        let url = "https://example.com/slow";
        let mut connector = WebReaderConnector::with_timeout_seconds(None, 10);
        connector.add_mock_page(url, "<html><body><article>slow</article></body></html>");
        connector.add_mock_delay(url, Duration::from_secs(15));
        let mut context = WebAgentContext::new(Uuid::new_v4(), capability_set(&["web.read"]), 500);

        let result = connector.fetch_and_extract(&mut context, url);
        assert!(result.is_err());
        if let Err(error) = result {
            assert!(error.to_string().contains("Timeout"));
        }
    }

    #[test]
    fn test_content_size_limit() {
        let url = "https://example.com/large";
        let large_body = "a".repeat(100_000);
        let html = format!("<html><head><title>Large</title></head><body><article>{large_body}</article></body></html>");

        let mut connector = WebReaderConnector::new(Some(50_000));
        connector.add_mock_page(url, html.as_str());
        let mut context = WebAgentContext::new(Uuid::new_v4(), capability_set(&["web.read"]), 500);

        let result = connector.fetch_and_extract(&mut context, url);
        assert!(result.is_ok());

        if let Ok(content) = result {
            // Content is wrapped with semantic boundary delimiters after truncation.
            assert!(content.text.contains("[truncated]"));
            assert!(content
                .text
                .contains("---BEGIN EXTERNAL DATA (WebContent)---"));
            assert!(content.text.contains("---END EXTERNAL DATA---"));
        }
    }
}
