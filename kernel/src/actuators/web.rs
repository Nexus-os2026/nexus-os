//! GovernedWeb actuator — web fetch with egress enforcement, and web search
//! only through a backend the host injects.
//!
//! Defines a `WebSearchBackend` trait so the host can inject a governed
//! implementation without a circular dependency (the broker crate depends on
//! this kernel). M0-FIX F3: there is no built-in search. The broker has no
//! governed web-search capability, so without an injected backend a
//! `WebSearch` step is refused ([`WEB_SEARCH_CLOSED`]) before any process or
//! connection. The built-in `CurlWebBackend` serves agent fetches only.

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

/// The refusal of a web search with no injected backend (M0-FIX F3).
pub const WEB_SEARCH_CLOSED: &str = "web search is closed: no governed search backend is \
     configured; the broker has no governed web-search capability and the built-in curl \
     search was removed (M0-FIX F3)";

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

// ── CurlWebBackend (built-in fetch) ─────────────────────────────────────────

/// Built-in backend for agent fetches (curl subprocess). It never searches:
/// its `search` returns the closed refusal without starting any process.
pub struct CurlWebBackend;

impl WebSearchBackend for CurlWebBackend {
    fn search(&self, _query: &str) -> Result<Vec<WebSearchResult>, String> {
        Err(WEB_SEARCH_CLOSED.to_string())
    }

    fn fetch_content(&self, url: &str) -> Result<String, String> {
        // P0-002C5C: a fetch follows no redirect. The allowlist admitted the
        // requested URL, not wherever it redirects; the agent may fetch the
        // target itself, which is checked in turn.
        let body = curl_get(url).map_err(|e| e.to_string())?;
        Ok(strip_html(&body))
    }
}

// ── GovernedWeb actuator ────────────────────────────────────────────────────

/// Governed web actuator. Handles URL fetching with egress enforcement, and
/// web search only through an injected `WebSearchBackend`.
pub struct GovernedWeb {
    backend: Arc<dyn WebSearchBackend>,
    /// Whether `backend` was injected by the host and so may serve search.
    /// The built-in backend never does (M0-FIX F3).
    injected: bool,
}

impl GovernedWeb {
    /// Create with the built-in fetch backend: web search is closed.
    pub fn new() -> Self {
        Self {
            backend: Arc::new(CurlWebBackend),
            injected: false,
        }
    }

    /// Create with a backend the host injects (a governed implementation, or
    /// a hermetic one in tests); it serves both search and fetch.
    pub fn with_backend(backend: Arc<dyn WebSearchBackend>) -> Self {
        Self {
            backend,
            injected: true,
        }
    }

    /// Check if a URL is allowed by the agent's egress allowlist.
    fn check_egress(url: &str, context: &ActuatorContext) -> Result<(), ActuatorError> {
        // P0-002C5C (Architect repair B): `endpoint_admits` compares the
        // scheme, the normalized host, the effective port and whole path
        // segments; an `https` entry never admits `http`. The desktop's Phase
        // Zero executor refuses agent web fetches before this check (Final
        // Gate item B), so it governs only other hosts of this actuator.
        let allowed = context
            .egress_allowlist
            .iter()
            .any(|entry| crate::firewall::egress::endpoint_admits(entry, url));

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
            injected: self.injected,
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
                // M0-FIX F3: no injected backend, no search; nothing is
                // started and no connection is made.
                if !self.injected {
                    return Err(ActuatorError::Closed(WEB_SEARCH_CLOSED.to_string()));
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

                // P0-002C5B: only an http(s) URL with a host and no userinfo,
                // checked against the allowlist in the normalized spelling the
                // backend's curl request receives.
                let target = crate::governed_http::http_url(url)
                    .map_err(|error| ActuatorError::EgressDenied(error.to_string()))?;
                Self::check_egress(target.as_str(), context)?;

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

/// Marks where curl reports the redirect target of an unfollowed redirect.
const REDIRECT_MARKER: &str = "\n__NEXUS_REDIRECT__:";

/// Perform a blocking HTTP GET via curl subprocess for an agent's fetch.
/// Safe to call from async context — spawns a child process.
///
/// A redirect response is refused and names its target instead of being
/// followed. (The redirect-following variant served only the built-in
/// search, removed by M0-FIX F3.)
fn curl_get(url: &str) -> Result<String, ActuatorError> {
    // P0-002C5B: HTTP(S) only, including redirects, with the URL after `--`.
    let url = crate::governed_http::http_url(url)
        .map_err(|error| ActuatorError::IoError(error.to_string()))?;
    let timeout_str = REQUEST_TIMEOUT_SECS.to_string();
    let redirect_format = format!("{REDIRECT_MARKER}%{{redirect_url}}");
    let mut command = Command::new("curl");
    command.args(crate::governed_http::CURL_HTTP_ONLY);
    command.args([
        "-sS",
        "--max-time",
        &timeout_str,
        "--max-filesize",
        &MAX_RESPONSE_BYTES.to_string(),
        "-A",
        USER_AGENT,
    ]);
    command.args(["-w", &redirect_format]);
    let output = command
        .args(["--", url.as_str()])
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

    let mut body = String::from_utf8_lossy(&output.stdout).to_string();
    if let Some(at) = body.rfind(REDIRECT_MARKER) {
        let target = body[at + REDIRECT_MARKER.len()..].trim().to_string();
        body.truncate(at);
        if !target.is_empty() {
            return Err(ActuatorError::IoError(format!(
                "the page redirects to {target}; fetch that URL if the egress allowlist admits it"
            )));
        }
    }
    // P0-002C5C: cut on a character boundary, never inside one.
    body.truncate(body.floor_char_boundary(MAX_RESPONSE_BYTES));
    Ok(body)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::autonomy::AutonomyLevel;
    use std::collections::HashSet;

    /// Answer each connection on a loopback port with the next of `responses`.
    fn serve(responses: Vec<Vec<u8>>) -> (String, std::thread::JoinHandle<usize>) {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let handle = std::thread::spawn(move || {
            let count = responses.len();
            for response in responses {
                let (mut stream, _) = listener.accept().unwrap();
                let mut head = Vec::new();
                let mut byte = [0u8; 1];
                while !head.ends_with(b"\r\n\r\n") && matches!(stream.read(&mut byte), Ok(1)) {
                    head.push(byte[0]);
                }
                let _ = stream.write_all(&response);
            }
            count
        });
        (base, handle)
    }

    /// P0-002C5C: an agent's fetch follows no redirect: the allowlist
    /// admitted the requested URL, not its target, so the target is named
    /// and nothing is sent to it.
    #[test]
    fn p0_002c5c_agent_fetches_follow_no_redirect() {
        let target = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        target.set_nonblocking(true).unwrap();
        let target_url = format!("http://{}/exfiltrate", target.local_addr().unwrap());
        let (base, server) = serve(vec![format!(
            "HTTP/1.1 302 Found\r\nLocation: {target_url}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        )
        .into_bytes()]);
        let mut ctx = make_context();
        ctx.egress_allowlist = vec![base.clone()];
        let web = GovernedWeb::new();
        let action = PlannedAction::WebFetch {
            url: format!("{base}/start"),
        };
        let error = web.execute(&action, &ctx).unwrap_err().to_string();
        assert!(
            error.contains(&format!("redirects to {target_url}")),
            "{error}"
        );
        assert_eq!(server.join().unwrap(), 1);
        assert!(matches!(
            target.accept(),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock
        ));
        // A fetch outside the allowlist, such as the target, is refused.
        let refused = PlannedAction::WebFetch { url: target_url };
        assert!(matches!(
            web.execute(&refused, &ctx),
            Err(ActuatorError::EgressDenied(_))
        ));
    }

    /// P0-002C5C: remote text is cut on character boundaries. Invalid
    /// UTF-8 widens under lossy decoding (one byte to three), so the cut
    /// falls inside a character unless it is placed on a boundary.
    #[test]
    fn p0_002c5c_remote_text_is_cut_on_character_boundaries() {
        let mut response =
            b"HTTP/1.1 200 OK\r\nContent-Length: 400000\r\nConnection: close\r\n\r\n".to_vec();
        response.extend_from_slice(&[0xFF; 400_000]);
        let (base, server) = serve(vec![response]);
        let body = curl_get(&format!("{base}/")).unwrap();
        server.join().unwrap();
        assert!(body.len() <= MAX_RESPONSE_BYTES);
        assert!(body.chars().all(|c| c == '\u{FFFD}'));
    }

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
            hitl_approved: false,
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

    /// P0-002C5C (Architect repair B): an explicit `https` allowlist entry no
    /// longer admits plain `http`. This test used to assert the opposite
    /// (`http_matches_https_allowlist`). The Architect ruled that downgrade a
    /// policy defect, so the test now asserts the corrected policy. It has
    /// not been weakened.
    #[test]
    fn http_does_not_match_https_allowlist() {
        let ctx = make_context();
        assert!(
            GovernedWeb::check_egress("http://example.com/path", &ctx).is_err(),
            "http://host must not match an https://host allowlist entry"
        );
    }

    #[test]
    fn https_matches_https_allowlist() {
        let ctx = make_context();
        assert!(
            GovernedWeb::check_egress("https://example.com/path", &ctx).is_ok(),
            "https://host must still match https://host allowlist entry"
        );
    }

    #[test]
    fn different_host_still_blocked() {
        let ctx = make_context();
        assert!(
            GovernedWeb::check_egress("http://evil.com", &ctx).is_err(),
            "different host must stay blocked regardless of scheme"
        );
    }

    /// M0-FIX F3: without an injected backend a search is refused as
    /// closed, after the capability check, before any process or request.
    #[test]
    fn search_is_closed_without_an_injected_backend() {
        let web = GovernedWeb::new();
        let action = PlannedAction::WebSearch {
            query: "rust programming".into(),
        };
        match web.execute(&action, &make_context()) {
            Err(ActuatorError::Closed(reason)) => assert_eq!(reason, WEB_SEARCH_CLOSED),
            other => panic!("expected the closed refusal, got {other:?}"),
        }
        assert_eq!(
            CurlWebBackend.search("rust").unwrap_err(),
            WEB_SEARCH_CLOSED
        );
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
    fn p0_002c5b_fetches_refuse_non_http_and_userinfo_urls_before_curl() {
        let ctx = make_context();
        let web = GovernedWeb::new();
        for url in [
            "file:///etc/passwd",
            "file://example.com/etc/passwd",
            "-K/etc/passwd",
            "https://example.com@evil.com/x",
            "gopher://example.com/",
        ] {
            let action = PlannedAction::WebFetch { url: url.into() };
            let err = web.execute(&action, &ctx).unwrap_err();
            assert!(
                matches!(err, ActuatorError::EgressDenied(_)),
                "{url:?}: {err}"
            );
        }
        for url in [
            "file:///etc/passwd",
            "-K/etc/passwd",
            "https://a@example.com/",
        ] {
            assert_eq!(
                curl_get(url).unwrap_err().to_string(),
                "io error: URL must be an http or https URL with a host",
                "{url:?}"
            );
        }
    }

    /// Final Gate item B (Architect decision D6): no search service is
    /// guessed at a local port. M0-FIX F3 removed the built-in search (and
    /// its operator-configured instance) with the function this test also
    /// checked; the source half stays.
    #[test]
    fn p0_fg_no_search_service_address_is_guessed() {
        let source = include_str!("web.rs");
        assert_no_guessed_searxng_address(source);
        // As a CRLF checkout (Windows CI, core.autocrlf=true) reads it.
        assert_no_guessed_searxng_address(&source.replace("\r\n", "\n").replace('\n', "\r\n"));
    }

    /// The source side of the guard above, for either line ending: the
    /// source is read with LF line endings before the production cut.
    fn assert_no_guessed_searxng_address(source: &str) {
        let source = source.replace("\r\n", "\n");
        let production = source.split("#[cfg(test)]\nmod tests").next().unwrap();
        for guessed in [
            "localhost:8080",
            "127.0.0.1:8080",
            "unwrap_or_else(|_| \"http",
        ] {
            assert!(!production.contains(guessed), "{guessed}");
        }
    }
}
