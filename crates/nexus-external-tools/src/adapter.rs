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
        let start = std::time::Instant::now();
        let timeout = request.timeout_secs.unwrap_or(self.timeout_secs);

        let mut args = vec![
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
            args.push("-d".to_string());
            args.push(body.clone());
        }

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
}
