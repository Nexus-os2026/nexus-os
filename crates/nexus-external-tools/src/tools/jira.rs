use crate::adapter::{HttpRequest, ToolError};
use std::collections::HashMap;

pub struct JiraTool;

impl JiraTool {
    pub fn build_request(
        action: &str,
        params: &serde_json::Value,
        token: &str,
    ) -> Result<HttpRequest, ToolError> {
        let base_url =
            std::env::var("JIRA_BASE_URL").unwrap_or_else(|_| "https://jira.atlassian.net".into());
        let email = std::env::var("JIRA_EMAIL").unwrap_or_default();
        let auth = base64_encode(&format!("{email}:{token}"));

        let mut headers = HashMap::new();
        headers.insert("Authorization".into(), format!("Basic {auth}"));
        headers.insert("Content-Type".into(), "application/json".into());

        match action {
            "list_issues" => {
                let project = params
                    .get("project")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ToolError::InvalidParameters("project required".into()))?;
                Ok(HttpRequest {
                    url: format!(
                        "{base_url}/rest/api/3/search?jql=project={project}&maxResults=50"
                    ),
                    method: "GET".into(),
                    headers,
                    body: None,
                    timeout_secs: None,
                })
            }
            "create_issue" => {
                let data = params
                    .get("data")
                    .ok_or_else(|| ToolError::InvalidParameters("data required".into()))?;
                Ok(HttpRequest {
                    url: format!("{base_url}/rest/api/3/issue"),
                    method: "POST".into(),
                    headers,
                    body: Some(data.to_string()),
                    timeout_secs: None,
                })
            }
            "update_issue" => {
                let issue_key = params
                    .get("issue_key")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ToolError::InvalidParameters("issue_key required".into()))?;
                let data = params
                    .get("data")
                    .ok_or_else(|| ToolError::InvalidParameters("data required".into()))?;
                Ok(HttpRequest {
                    url: format!("{base_url}/rest/api/3/issue/{issue_key}"),
                    method: "PUT".into(),
                    headers,
                    body: Some(data.to_string()),
                    timeout_secs: None,
                })
            }
            _ => Err(ToolError::InvalidParameters(format!(
                "Unknown Jira action: {action}"
            ))),
        }
    }
}

fn base64_encode(s: &str) -> String {
    use sha2::Digest;
    // Simple base64 without pulling in the base64 crate
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes = s.as_bytes();
    let mut result = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let n = (b0 << 16) | (b1 << 8) | b2;
        result.push(CHARS[((n >> 18) & 0x3f) as usize] as char);
        result.push(CHARS[((n >> 12) & 0x3f) as usize] as char);
        if chunk.len() > 1 {
            result.push(CHARS[((n >> 6) & 0x3f) as usize] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(CHARS[(n & 0x3f) as usize] as char);
        } else {
            result.push('=');
        }
    }
    let _ = sha2::Sha256::new(); // suppress unused import if sha2 Digest pulled in
    result
}
