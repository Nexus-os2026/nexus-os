use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Generic HTTP adapter — executes tool calls via curl subprocess.
pub struct HttpAdapter {
    pub timeout_secs: u64,
    pub max_response_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpRequest {
    pub url: String,
    pub method: String,
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
    pub timeout_secs: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpResponse {
    pub status_code: u16,
    pub body: String,
    pub duration_ms: u64,
    pub success: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("Tool not found: {0}")]
    NotFound(String),
    #[error("Tool not available: {0}")]
    NotAvailable(String),
    #[error("Governance denied: {0}")]
    GovernanceDenied(String),
    #[error("Insufficient balance: {0}")]
    InsufficientBalance(String),
    #[error("Rate limit exceeded: {0}")]
    RateLimited(String),
    #[error("Execution failed: {0}")]
    ExecutionFailed(String),
    #[error("Timeout")]
    Timeout,
    #[error("Invalid parameters: {0}")]
    InvalidParameters(String),
    #[error("URL blocked: {0}")]
    UrlBlocked(String),
}

impl HttpAdapter {
    pub fn new() -> Self {
        Self {
            timeout_secs: 30,
            max_response_bytes: 10 * 1024 * 1024,
        }
    }

    pub fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, ToolError> {
        check_request(request)?;
        let start = std::time::Instant::now();
        let timeout = request.timeout_secs.unwrap_or(self.timeout_secs);

        // P0-002C5B: no `~/.curlrc`, HTTP(S) only for the request and every
        // redirect, a literal body, and the URL after `--`.
        let mut args = vec![
            "-q".to_string(),
            "--globoff".to_string(),
            "--proto".to_string(),
            "=http,https".to_string(),
            "--proto-redir".to_string(),
            "=http,https".to_string(),
            "-sS".to_string(),
            "-w".to_string(),
            "\n__NX_T__:%{http_code}".to_string(),
            "-X".to_string(),
            request.method.clone(),
            "--max-time".to_string(),
            timeout.to_string(),
            "--max-filesize".to_string(),
            self.max_response_bytes.to_string(),
        ];

        for (key, value) in &request.headers {
            args.push("-H".to_string());
            args.push(format!("{key}: {value}"));
        }

        if let Some(ref body) = request.body {
            args.push("--data-raw".to_string());
            args.push(body.clone());
        }

        args.push("--".to_string());
        args.push(request.url.clone());

        let output = std::process::Command::new("curl")
            .args(&args)
            .output()
            .map_err(|e| ToolError::ExecutionFailed(format!("curl failed: {e}")))?;

        let duration_ms = start.elapsed().as_millis() as u64;
        let raw = String::from_utf8_lossy(&output.stdout).to_string();

        let marker = "__NX_T__:";
        let (body, status_str) = raw
            .rsplit_once(marker)
            .map(|(b, s)| (b.to_string(), s.trim().to_string()))
            .unwrap_or((raw.clone(), "0".into()));

        let status_code = status_str.parse::<u16>().unwrap_or(0);

        Ok(HttpResponse {
            status_code,
            body,
            duration_ms,
            success: (200..300).contains(&status_code),
        })
    }
}

/// Checks a request built from caller parameters before any of it reaches
/// curl (P0-002C5B): an `http`/`https` URL without whitespace or control
/// characters (so it can never begin with `-` or name a file), an allowlisted
/// method, and headers whose names are HTTP tokens (never `@file`) and whose
/// values carry no line breaks.
fn check_request(request: &HttpRequest) -> Result<(), ToolError> {
    let url = request.url.as_str();
    // The authority (up to the first `/`, `?` or `#`) must be a non-empty host
    // without userinfo.
    let host_follows = ["http://", "https://"].iter().any(|scheme| {
        url.get(..scheme.len())
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case(scheme))
            && url[scheme.len()..]
                .split(['/', '?', '#'])
                .next()
                .is_some_and(|authority| !authority.is_empty() && !authority.contains('@'))
    });
    if !host_follows
        || url
            .chars()
            .any(|c| c.is_whitespace() || c.is_control() || c == '\\')
    {
        return Err(ToolError::UrlBlocked(
            "only http and https URLs with a host are allowed".into(),
        ));
    }
    const METHODS: [&str; 7] = ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"];
    if !METHODS.contains(&request.method.as_str()) {
        return Err(ToolError::InvalidParameters(
            "unsupported HTTP method".into(),
        ));
    }
    for (key, value) in &request.headers {
        let token = !key.is_empty()
            && key
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b));
        if !token || value.chars().any(|c| matches!(c, '\r' | '\n' | '\0')) {
            return Err(ToolError::InvalidParameters("invalid HTTP header".into()));
        }
    }
    Ok(())
}

impl Default for HttpAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_adapter_timeout() {
        let adapter = HttpAdapter::new();
        assert_eq!(adapter.timeout_secs, 30);
    }

    fn request(url: &str, method: &str, headers: &[(&str, &str)]) -> HttpRequest {
        HttpRequest {
            url: url.into(),
            method: method.into(),
            headers: headers
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            body: Some("@/etc/passwd".into()),
            timeout_secs: Some(1),
        }
    }

    #[test]
    fn p0_002c5b_hostile_requests_are_refused_before_curl_runs() {
        // Every one of these fails validation, so no process is spawned.
        let adapter = HttpAdapter::new();
        for url in [
            "file:///etc/passwd",
            "FILE:/etc/passwd",
            "-K/etc/passwd",
            "--config=/etc/passwd",
            "@/etc/passwd",
            "/etc/passwd",
            "smtp://localhost:587",
            "gopher://example.com",
            "local://database",
            "http://",
            "http:///etc/passwd",
            "https://example.com /x",
            "https://example.com\r\nX: 1",
            "https://allowed.example@evil.example/",
            "https://allowed.example\\@evil.example/",
            "",
        ] {
            assert!(
                matches!(
                    adapter.execute(&request(url, "GET", &[])),
                    Err(ToolError::UrlBlocked(_))
                ),
                "{url:?}"
            );
        }
        for method in ["QUERY", "get", "-K", "TRACE", "GET /x HTTP/1.1", ""] {
            assert!(
                matches!(
                    adapter.execute(&request("https://example.com", method, &[])),
                    Err(ToolError::InvalidParameters(_))
                ),
                "{method:?}"
            );
        }
        for header in [
            ("@/etc/passwd", "x"),
            ("Bad Name", "x"),
            ("X", "a\r\nInjected: 1"),
            ("", "x"),
        ] {
            assert!(
                matches!(
                    adapter.execute(&request("https://example.com", "GET", &[header])),
                    Err(ToolError::InvalidParameters(_))
                ),
                "{header:?}"
            );
        }
        let ok = request(
            "HTTPS://example.com:8443/x",
            "POST",
            &[("Content-Type", "application/json")],
        );
        assert!(check_request(&ok).is_ok());
    }
}
