use crate::WebAgentContext;
use nexus_connectors_core::rate_limit::{RateLimitDecision, RateLimiter};
use nexus_kernel::audit::{AuditTrail, EventType};
use nexus_kernel::errors::AgentError;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

const DEFAULT_MAX_CONTENT_CHARS: usize = 50_000;
const TRUNCATION_MARKER: &str = "[truncated]";

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
    mock_pages: HashMap<String, String>,
    pub audit_trail: AuditTrail,
    rate_limiter: RateLimiter,
}

impl WebReaderConnector {
    pub fn new(max_content_chars: Option<usize>) -> Self {
        let limiter = RateLimiter::new();
        limiter.configure("web.read", 30, 60);

        Self {
            max_content_chars: max_content_chars.unwrap_or(DEFAULT_MAX_CONTENT_CHARS),
            mock_pages: HashMap::new(),
            audit_trail: AuditTrail::new(),
            rate_limiter: limiter,
        }
    }

    pub fn add_mock_page(&mut self, url: &str, html: &str) {
        self.mock_pages.insert(url.to_string(), html.to_string());
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

        let html = self.mock_pages.get(url).cloned().ok_or_else(|| {
            AgentError::SupervisorError(format!("no mock page configured for '{url}'"))
        })?;

        let title = extract_title(html.as_str());
        let mut text = extract_readable_text(html.as_str());
        text = strip_personal_data(text.as_str());
        text = enforce_size_limit(text.as_str(), self.max_content_chars);

        let word_count = text.split_whitespace().count();
        let extracted_at = current_unix_timestamp();

        let clean = CleanContent {
            title,
            text,
            word_count,
            source_url: url.to_string(),
            extracted_at,
        };

        let _ = self.audit_trail.append_event(
            agent.agent_id,
            EventType::ToolCall,
            json!({
                "event": "web_read_extract",
                "source_url": url,
                "word_count": clean.word_count,
                "char_count": clean.text.chars().count(),
                "fuel_cost": fuel_cost
            }),
        );

        Ok(clean)
    }
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
    let mut text = html.to_string();

    for tag in ["script", "style", "nav", "aside", "footer"] {
        text = remove_tag_blocks(text.as_str(), tag);
    }

    if let Ok(ad_block) = Regex::new(
        r#"(?is)<(?:div|section)[^>]*(?:class|id)\s*=\s*["'][^"']*(?:ad|advert|promo)[^"']*["'][^>]*>.*?</(?:div|section)>"#,
    ) {
        text = ad_block.replace_all(&text, " ").into_owned();
    }

    if let Ok(tag_re) = Regex::new(r"(?is)<[^>]+>") {
        text = tag_re.replace_all(&text, " ").into_owned();
    }

    sanitize_whitespace(text.as_str())
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
    use uuid::Uuid;

    fn capability_set(values: &[&str]) -> HashSet<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn test_extract_clean_text() {
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
    <article>
      Rust makes systems programming safer.
    </article>
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
    fn test_personal_data_stripped() {
        let url = "https://example.com/pii";
        let html = r#"
<html>
  <head><title>PII</title></head>
  <body>
    <article>
      Contact email: john@test.com and phone +1 415-555-1111.
    </article>
  </body>
</html>
"#;

        let mut connector = WebReaderConnector::new(None);
        connector.add_mock_page(url, html);
        let mut context = WebAgentContext::new(Uuid::new_v4(), capability_set(&["web.read"]), 500);

        let result = connector.fetch_and_extract(&mut context, url);
        assert!(result.is_ok());

        if let Ok(content) = result {
            assert!(!content.text.contains("john@test.com"));
            assert!(!content.text.contains("415-555-1111"));
            assert!(content.text.contains("[redacted-email]"));
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
            assert_eq!(content.text.chars().count(), 50_000);
            assert!(content.text.ends_with("[truncated]"));
        }
    }
}
