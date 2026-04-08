//! GovernedWeb actuator — governed web search and fetch with egress enforcement.
//!
//! Defines a `WebSearchBackend` trait so the app layer can inject a governed
//! implementation (Brave API, scraper-based reader) without creating a circular
//! dependency.  A built-in `CurlWebBackend` serves as the default when no
//! external backend is injected.

use super::types::{ActionResult, Actuator, ActuatorContext, ActuatorError, SideEffect};
use crate::capabilities::has_capability;
use crate::cognitive::types::PlannedAction;
use regex::Regex;
use std::process::{Command, Stdio};
use std::sync::Arc;

/// Maximum response body size: 1 MB.
const MAX_RESPONSE_BYTES: usize = 1024 * 1024;

/// Request timeout in seconds.
const REQUEST_TIMEOUT_SECS: u32 = 30;

/// User-Agent header for all outbound requests.
const USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64; rv:120.0) Gecko/20100101 Firefox/120.0";

/// Fuel cost per web search.
const FUEL_COST_SEARCH: f64 = 3.0;
/// Fuel cost per web fetch.
const FUEL_COST_FETCH: f64 = 2.0;

// ── WebSearchBackend trait ──────────────────────────────────────────────────

/// A structured search result.
#[derive(Debug, Clone)]
pub struct WebSearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
    pub relevance_score: f64,
}

/// Backend trait for web search and content fetching.
/// Implement this at the app layer to inject governed connectors (Brave,
/// scraper-based reader, etc.) without circular dependencies.
pub trait WebSearchBackend: Send + Sync {
    /// Search the web for `query` and return structured results.
    fn search(&self, query: &str) -> Result<Vec<WebSearchResult>, String>;

    /// Fetch a URL and return clean readable text (HTML stripped).
    fn fetch_content(&self, url: &str) -> Result<String, String>;
}

// ── CurlWebBackend (built-in default) ───────────────────────────────────────

/// Default backend that uses curl subprocesses. Works everywhere, no deps.
pub struct CurlWebBackend;

/// SearXNG instance URL. Configurable via SEARXNG_URL env var.
/// Default: http://localhost:8080 (local Docker instance).
fn searxng_url() -> Option<String> {
    let url = std::env::var("SEARXNG_URL").unwrap_or_else(|_| "http://localhost:8080".to_string());
    // Quick probe: if SearXNG isn't running, don't waste time on it
    if let Ok(output) = Command::new("curl")
        .args(["-sS", "--max-time", "2", &format!("{url}/healthz")])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
    {
        if output.status.success() {
            return Some(url);
        }
    }
    None
}

impl WebSearchBackend for CurlWebBackend {
    fn search(&self, query: &str) -> Result<Vec<WebSearchResult>, String> {
        let encoded = urlencoded(query);

        // 1. Try SearXNG (local, private, structured JSON, best results)
        if let Some(base) = searxng_url() {
            let url =
                format!("{base}/search?q={encoded}&format=json&categories=general&language=en");
            if let Ok(body) = curl_get(&url) {
                let results = parse_searxng_results(&body);
                if !results.is_empty() {
                    return Ok(results);
                }
            }
        }

        // 2. Fallback: DuckDuckGo HTML (public, may be rate-limited)
        let ddg_url = format!("https://html.duckduckgo.com/html/?q={encoded}");
        if let Ok(body) = curl_get(&ddg_url) {
            let results = parse_duckduckgo_results(&body);
            if !results.is_empty() {
                return Ok(results);
            }
        }

        // 3. Fallback: HackerNews RSS (good for tech/AI queries)
        let hn_url = format!("https://hnrss.org/newest?q={encoded}&count=10");
        if let Ok(body) = curl_get(&hn_url) {
            let results = parse_rss_results(&body);
            if !results.is_empty() {
                return Ok(results);
            }
        }

        Err(format!(
            "Web search failed for \"{query}\": all sources returned no results. \
             Try a different query or use ShellCommand with curl directly."
        ))
    }

    fn fetch_content(&self, url: &str) -> Result<String, String> {
        let body = curl_get(url).map_err(|e| e.to_string())?;
        Ok(strip_html(&body))
    }
}

// ── GovernedWeb actuator ────────────────────────────────────────────────────

/// Governed web actuator. Handles web search and URL fetching with egress
/// enforcement. Uses an injected `WebSearchBackend` for the actual HTTP work.
pub struct GovernedWeb {
    backend: Arc<dyn WebSearchBackend>,
}

impl GovernedWeb {
    /// Create with the default curl backend.
    pub fn new() -> Self {
        Self {
            backend: Arc::new(CurlWebBackend),
        }
    }

    /// Create with a custom backend (e.g. governed connectors from the app layer).
    pub fn with_backend(backend: Arc<dyn WebSearchBackend>) -> Self {
        Self { backend }
    }

    /// Check if a URL is allowed by the agent's egress allowlist.
    fn check_egress(url: &str, context: &ActuatorContext) -> Result<(), ActuatorError> {
        let allowed = context
            .egress_allowlist
            .iter()
            .any(|prefix| url.starts_with(prefix));

        if !allowed {
            return Err(ActuatorError::EgressDenied(format!(
                "URL '{url}' not in egress allowlist"
            )));
        }

        Ok(())
    }
}

impl Default for GovernedWeb {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for GovernedWeb {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GovernedWeb").finish()
    }
}

impl Clone for GovernedWeb {
    fn clone(&self) -> Self {
        Self {
            backend: Arc::clone(&self.backend),
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

                let results = self.backend.search(query).map_err(ActuatorError::IoError)?;

                let output = if results.is_empty() {
                    format!("No results found for \"{query}\".")
                } else {
                    let mut lines = vec![format!("Search results for \"{query}\":\n")];
                    for (i, r) in results.iter().enumerate() {
                        lines.push(format!(
                            "{}. {}\n   {}\n   {}",
                            i + 1,
                            r.title,
                            r.url,
                            r.snippet
                        ));
                    }
                    lines.join("\n")
                };

                Ok(ActionResult {
                    success: true,
                    output,
                    fuel_cost: FUEL_COST_SEARCH,
                    side_effects: vec![SideEffect::HttpRequest {
                        url: format!("search:{query}"),
                    }],
                })
            }

            PlannedAction::WebFetch { url } => {
                if !has_capability(context.capabilities.iter().map(String::as_str), "web.read") {
                    return Err(ActuatorError::CapabilityDenied("web.read".into()));
                }

                Self::check_egress(url, context)?;

                let text = self
                    .backend
                    .fetch_content(url)
                    .map_err(ActuatorError::IoError)?;

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

// ── Shared helpers ──────────────────────────────────────────────────────────

/// Perform a blocking HTTP GET via curl subprocess.
/// Safe to call from async context — spawns a child process.
fn curl_get(url: &str) -> Result<String, ActuatorError> {
    let timeout_str = REQUEST_TIMEOUT_SECS.to_string();
    let output = Command::new("curl")
        .args([
            "-sS",
            "-L",
            "--max-time",
            &timeout_str,
            "--max-filesize",
            &MAX_RESPONSE_BYTES.to_string(),
            "-A",
            USER_AGENT,
            url,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| {
            ActuatorError::IoError(format!(
                "curl execution failed (is curl installed?): {error}"
            ))
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ActuatorError::IoError(format!(
            "curl exit {}: {}",
            output.status.code().unwrap_or(-1),
            stderr.trim()
        )));
    }

    let body = String::from_utf8_lossy(&output.stdout).to_string();
    if body.len() > MAX_RESPONSE_BYTES {
        Ok(body[..MAX_RESPONSE_BYTES].to_string())
    } else {
        Ok(body)
    }
}

/// Convert HTML to clean readable content for LLM consumption.
///
/// Strategy:
/// 1. Try `readability` to extract article content (strips nav/ads/footer)
/// 2. Convert the clean HTML to Markdown via `htmd` (LLMs understand Markdown)
/// 3. If both fail, fall back to basic tag stripping
fn strip_html(html: &str) -> String {
    // Pre-process: remove script/style/nav/footer tags that all parsers struggle with
    let cleaned = remove_dangerous_tags(html);

    // For large pages (>2KB), use readability to extract the article first,
    // then convert to markdown. Readability is too aggressive on small fragments.
    if cleaned.len() > 2048 {
        if let Ok(url) = url::Url::parse("https://example.com") {
            let mut input = std::io::Cursor::new(cleaned.as_bytes());
            if let Ok(product) = readability::extractor::extract(&mut input, &url) {
                if let Ok(md) = htmd::convert(&product.content) {
                    let trimmed = md.trim().to_string();
                    if !trimmed.is_empty() {
                        return trimmed;
                    }
                }
                if !product.text.trim().is_empty() {
                    return product.text.trim().to_string();
                }
            }
        }
    }

    // For small pages or if readability failed, convert directly to markdown
    if let Ok(md) = htmd::convert(&cleaned) {
        let trimmed = md.trim().to_string();
        if !trimmed.is_empty() {
            return trimmed;
        }
    }

    // Final fallback: basic tag stripping
    strip_html_basic(&cleaned)
}

/// Remove script, style, and other dangerous tags before any parsing.
fn remove_dangerous_tags(html: &str) -> String {
    let mut result = html.to_string();
    for tag in &["script", "style", "noscript", "svg", "iframe"] {
        // Rust regex doesn't support backreferences, so we build one regex per tag
        let pattern = format!(r"(?is)<{tag}[^>]*>.*?</{tag}>");
        if let Ok(re) = Regex::new(&pattern) {
            result = re.replace_all(&result, "").to_string();
        }
    }
    result
}

/// Fast fallback: strip HTML tags character-by-character.
fn strip_html_basic(html: &str) -> String {
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

/// Parse SearXNG JSON response into structured results.
fn parse_searxng_results(json_body: &str) -> Vec<WebSearchResult> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(json_body) else {
        return Vec::new();
    };
    let Some(results) = value.get("results").and_then(|v| v.as_array()) else {
        return Vec::new();
    };

    results
        .iter()
        .take(10)
        .enumerate()
        .filter_map(|(i, item)| {
            let title = item.get("title")?.as_str()?.trim().to_string();
            if title.is_empty() {
                return None;
            }
            let url = item
                .get("url")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let snippet = item
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            Some(WebSearchResult {
                title,
                url,
                snippet,
                relevance_score: 1.0 - (i as f64 * 0.05),
            })
        })
        .collect()
}

/// Parse DuckDuckGo HTML search results into structured results.
fn parse_duckduckgo_results(html: &str) -> Vec<WebSearchResult> {
    static TITLE_RE: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
        Regex::new(r#"(?is)<a[^>]*class="[^"]*result__a[^"]*"[^>]*>(.*?)</a>"#)
            .unwrap_or_else(|_| Regex::new("^$").unwrap_or_else(|_| std::process::abort()))
    });
    static SNIPPET_RE: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
        Regex::new(
            r#"(?is)<(?:a|div)[^>]*class="[^"]*result__snippet[^"]*"[^>]*>(.*?)</(?:a|div)>"#,
        )
        .unwrap_or_else(|_| Regex::new("^$").unwrap_or_else(|_| std::process::abort()))
    });
    static URL_RE: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
        Regex::new(r#"(?i)<a[^>]*class="[^"]*result__a[^"]*"[^>]*href="([^"]*)"[^>]*>"#)
            .unwrap_or_else(|_| Regex::new("^$").unwrap_or_else(|_| std::process::abort()))
    });

    let titles: Vec<String> = TITLE_RE
        .captures_iter(html)
        .filter_map(|c| c.get(1).map(|m| strip_html(m.as_str())))
        .filter(|t| !t.is_empty())
        .collect();
    let snippets: Vec<String> = SNIPPET_RE
        .captures_iter(html)
        .filter_map(|c| c.get(1).map(|m| strip_html(m.as_str())))
        .filter(|s| !s.is_empty())
        .collect();
    let urls: Vec<String> = URL_RE
        .captures_iter(html)
        .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
        .collect();

    let max = titles.len().min(10);
    let mut results = Vec::new();
    for i in 0..max {
        let title = titles.get(i).cloned().unwrap_or_default();
        let snippet = snippets.get(i).cloned().unwrap_or_default();
        let url = urls.get(i).cloned().unwrap_or_default();
        if title.is_empty() {
            continue;
        }
        results.push(WebSearchResult {
            title,
            url,
            snippet,
            relevance_score: 1.0 - (i as f64 * 0.05),
        });
    }

    results
}

/// Parse RSS/XML feed into structured results (for HackerNews, Reddit, etc.)
fn parse_rss_results(xml: &str) -> Vec<WebSearchResult> {
    static ITEM_RE: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
        Regex::new(r"(?is)<item>(.*?)</item>")
            .unwrap_or_else(|_| Regex::new("^$").unwrap_or_else(|_| std::process::abort()))
    });
    static TITLE_RE: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
        Regex::new(r"(?is)<title>(.*?)</title>")
            .unwrap_or_else(|_| Regex::new("^$").unwrap_or_else(|_| std::process::abort()))
    });
    static LINK_RE: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
        Regex::new(r"(?is)<link>(.*?)</link>")
            .unwrap_or_else(|_| Regex::new("^$").unwrap_or_else(|_| std::process::abort()))
    });
    static DESC_RE: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
        Regex::new(r"(?is)<description>(.*?)</description>")
            .unwrap_or_else(|_| Regex::new("^$").unwrap_or_else(|_| std::process::abort()))
    });

    let mut results = Vec::new();
    for (i, cap) in ITEM_RE.captures_iter(xml).take(10).enumerate() {
        let item = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        let title = TITLE_RE
            .captures(item)
            .and_then(|c| c.get(1))
            .map(|m| strip_html(m.as_str()))
            .unwrap_or_default();
        let link = LINK_RE
            .captures(item)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().trim().to_string())
            .unwrap_or_default();
        let desc = DESC_RE
            .captures(item)
            .and_then(|c| c.get(1))
            .map(|m| strip_html(m.as_str()))
            .unwrap_or_default();

        if title.is_empty() {
            continue;
        }
        results.push(WebSearchResult {
            title,
            url: link,
            snippet: if desc.len() > 200 {
                format!("{}...", &desc[..200])
            } else {
                desc
            },
            relevance_score: 1.0 - (i as f64 * 0.08),
        });
    }

    results
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
        let web = GovernedWeb::new();

        let action = PlannedAction::WebFetch {
            url: "https://evil.com/data".into(),
        };
        let err = web.execute(&action, &ctx).unwrap_err();
        assert!(matches!(err, ActuatorError::EgressDenied(_)));
    }

    #[test]
    fn egress_check_function() {
        let ctx = make_context();
        assert!(GovernedWeb::check_egress("https://example.com/page", &ctx).is_ok());
        assert!(GovernedWeb::check_egress("https://other.com/page", &ctx).is_err());
    }

    #[test]
    fn search_dispatches() {
        let ctx = make_context();
        let web = GovernedWeb::new();

        let action = PlannedAction::WebSearch {
            query: "rust programming".into(),
        };
        match web.execute(&action, &ctx) {
            Ok(result) => {
                assert!(result.success);
                assert!(!result.output.is_empty());
                assert_eq!(result.side_effects.len(), 1);
            }
            Err(ActuatorError::IoError(_)) => {
                // Network unavailable or search provider blocked — acceptable in CI
            }
            Err(other) => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn strip_html_basic() {
        let html = "<html><body><h1>Title</h1><p>Hello <b>world</b></p></body></html>";
        let text = strip_html(html);
        // htmd converts to markdown: "# Title\n\nHello **world**"
        // or readability extracts article text: "Title Hello world"
        // Either way, the semantic content must be present
        let lower = text.to_lowercase();
        assert!(lower.contains("title"), "Missing 'title' in: {text}");
        assert!(lower.contains("hello"), "Missing 'hello' in: {text}");
        assert!(lower.contains("world"), "Missing 'world' in: {text}");
        assert!(!text.contains("<h1>"), "Raw HTML leaked: {text}");
    }

    #[test]
    fn strip_html_removes_scripts() {
        let html = "<p>before</p><script>alert('xss')</script><p>after</p>";
        let text = strip_html(html);
        assert!(text.contains("before"), "Missing 'before' in: {text}");
        assert!(text.contains("after"), "Missing 'after' in: {text}");
        assert!(!text.contains("alert"), "Script content leaked: {text}");
    }

    #[test]
    fn capability_denied_search() {
        let mut ctx = make_context();
        ctx.capabilities.remove("web.search");
        let web = GovernedWeb::new();
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
        let web = GovernedWeb::new();
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
        let web = GovernedWeb::new();
        let action = PlannedAction::WebFetch {
            url: "https://example.com/page".into(),
        };
        let err = web.execute(&action, &ctx).unwrap_err();
        assert!(matches!(err, ActuatorError::EgressDenied(_)));
    }

    #[test]
    fn urlencoded_basic() {
        assert_eq!(urlencoded("hello world"), "hello+world");
        assert_eq!(urlencoded("a&b=c"), "a%26b%3Dc");
    }

    #[test]
    fn custom_backend_is_used() {
        struct MockBackend;
        impl WebSearchBackend for MockBackend {
            fn search(&self, _query: &str) -> Result<Vec<WebSearchResult>, String> {
                Ok(vec![WebSearchResult {
                    title: "Mock Result".into(),
                    url: "https://mock.test".into(),
                    snippet: "This is a mock".into(),
                    relevance_score: 1.0,
                }])
            }
            fn fetch_content(&self, _url: &str) -> Result<String, String> {
                Ok("Mock content".into())
            }
        }

        let web = GovernedWeb::with_backend(Arc::new(MockBackend));
        let ctx = make_context();
        let action = PlannedAction::WebSearch {
            query: "anything".into(),
        };
        let result = web.execute(&action, &ctx).unwrap();
        assert!(result.output.contains("Mock Result"));
    }

    #[test]
    fn parse_searxng_json() {
        let json = r#"{
            "results": [
                {"title": "Rust Language", "url": "https://rust-lang.org", "content": "A language empowering everyone."},
                {"title": "Rust Book", "url": "https://doc.rust-lang.org/book/", "content": "The official book."}
            ]
        }"#;
        let results = parse_searxng_results(json);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, "Rust Language");
        assert_eq!(results[0].url, "https://rust-lang.org");
        assert!(results[0].snippet.contains("empowering"));
    }

    #[test]
    fn readability_extracts_article() {
        let html = r#"
<html>
<head><title>AI in 2026</title></head>
<body>
  <nav>Home | About | Contact</nav>
  <div class="ads">Buy stuff!</div>
  <article>
    <h1>The State of AI in 2026</h1>
    <p>Artificial intelligence has made remarkable progress in 2026, with
    autonomous agents becoming mainstream tools for software development,
    research, and business automation.</p>
    <p>The key breakthrough was governed autonomy, where agents operate
    freely within safety boundaries defined by humans.</p>
  </article>
  <footer>Copyright 2026 TechCo</footer>
  <script>trackUser();</script>
</body>
</html>"#;

        let result = strip_html(html);
        // Should contain the article content
        assert!(
            result.contains("autonomous agents") || result.contains("Artificial intelligence"),
            "Expected article content, got: {}",
            &result[..result.len().min(200)]
        );
        // Should NOT contain nav/ads/scripts
        assert!(!result.contains("trackUser"), "Script content leaked");
    }

    #[test]
    fn parse_rss_extracts_items() {
        let rss = r#"
        <rss><channel>
          <item>
            <title>AI News Today</title>
            <link>https://example.com/ai</link>
            <description>Latest AI developments</description>
          </item>
          <item>
            <title>Rust Update</title>
            <link>https://example.com/rust</link>
            <description>Rust 2026 edition</description>
          </item>
        </channel></rss>"#;

        let results = parse_rss_results(rss);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, "AI News Today");
        assert_eq!(results[1].url, "https://example.com/rust");
    }
}
