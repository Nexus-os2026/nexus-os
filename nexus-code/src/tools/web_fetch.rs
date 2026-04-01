//! WebFetchTool — HTTP GET with content extraction.
//! Requires NetworkAccess capability (not granted by default).

use super::{NxTool, ToolContext, ToolResult};
use async_trait::async_trait;
use serde_json::json;

const MAX_RESPONSE_BYTES: usize = 100_000;

/// Fetch a web page and extract text content.
pub struct WebFetchTool;

#[async_trait]
impl NxTool for WebFetchTool {
    fn name(&self) -> &str {
        "web_fetch"
    }

    fn description(&self) -> &str {
        "Fetch a web page and extract its text content. Useful for documentation, \
         error messages, API references. Requires NetworkAccess capability."
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The URL to fetch (must start with http:// or https://)"
                },
                "extract_text": {
                    "type": "boolean",
                    "description": "Strip HTML tags (default: true)"
                }
            },
            "required": ["url"]
        })
    }

    fn estimated_fuel(&self, _input: &serde_json::Value) -> u64 {
        15
    }

    fn required_capability(
        &self,
        _input: &serde_json::Value,
    ) -> Option<crate::governance::Capability> {
        Some(crate::governance::Capability::NetworkAccess)
    }

    async fn execute(&self, input: serde_json::Value, _ctx: &ToolContext) -> ToolResult {
        let url = match input.get("url").and_then(|v| v.as_str()) {
            Some(u) => u,
            None => return ToolResult::error("Missing required parameter: url"),
        };

        let extract_text = input
            .get("extract_text")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        if !url.starts_with("http://") && !url.starts_with("https://") {
            return ToolResult::error("URL must start with http:// or https://");
        }

        let client = match reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .user_agent("NexusCode/0.1")
            .build()
        {
            Ok(c) => c,
            Err(e) => return ToolResult::error(format!("HTTP client error: {}", e)),
        };

        let response = match client.get(url).send().await {
            Ok(r) => r,
            Err(e) => return ToolResult::error(format!("Failed to fetch '{}': {}", url, e)),
        };

        let status = response.status();
        if !status.is_success() {
            return ToolResult::error(format!("HTTP {}: {}", status.as_u16(), url));
        }

        let body = match response.text().await {
            Ok(t) => t,
            Err(e) => return ToolResult::error(format!("Failed to read response: {}", e)),
        };

        let content = if extract_text {
            strip_html_tags(&body)
        } else {
            body
        };

        let output = if content.len() > MAX_RESPONSE_BYTES {
            format!(
                "{}\n\n[TRUNCATED: {} bytes total]",
                &content[..MAX_RESPONSE_BYTES],
                content.len()
            )
        } else {
            content
        };

        ToolResult::success(output)
    }
}

/// Strip HTML tags and script/style content.
pub fn strip_html_tags(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut in_tag = false;
    let mut in_script = false;
    let mut in_style = false;

    let lower = html.to_lowercase();
    let chars: Vec<char> = html.chars().collect();
    let lower_chars: Vec<char> = lower.chars().collect();

    let mut i = 0;
    while i < chars.len() {
        if !in_tag && chars[i] == '<' {
            let remaining: String = lower_chars[i..].iter().take(10).collect();
            if remaining.starts_with("<script") {
                in_script = true;
            }
            if remaining.starts_with("<style") {
                in_style = true;
            }
            in_tag = true;
            i += 1;
            continue;
        }

        if in_tag && chars[i] == '>' {
            if i >= 8 {
                let lookback: String = lower_chars[i.saturating_sub(8)..=i].iter().collect();
                if lookback.contains("</script>") {
                    in_script = false;
                }
                if lookback.contains("</style>") {
                    in_style = false;
                }
            }
            in_tag = false;
            i += 1;
            continue;
        }

        if !in_tag && !in_script && !in_style {
            result.push(chars[i]);
        }

        i += 1;
    }

    // Collapse whitespace
    let mut clean = String::new();
    let mut last_ws = false;
    for ch in result.chars() {
        if ch.is_whitespace() {
            if !last_ws {
                clean.push(' ');
                last_ws = true;
            }
        } else {
            clean.push(ch);
            last_ws = false;
        }
    }

    clean.trim().to_string()
}
