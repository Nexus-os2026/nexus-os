use crate::adapter::{HttpRequest, ToolError};
use std::collections::HashMap;

pub struct WebhookTool;

impl WebhookTool {
    pub fn build_request(params: &serde_json::Value) -> Result<HttpRequest, ToolError> {
        let url = params
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParameters("url required".into()))?;

        let method = params
            .get("method")
            .and_then(|v| v.as_str())
            .unwrap_or("POST");

        let mut headers: HashMap<String, String> = HashMap::new();
        headers.insert("Content-Type".into(), "application/json".into());

        if let Some(h) = params.get("headers").and_then(|v| v.as_object()) {
            for (k, v) in h {
                if let Some(s) = v.as_str() {
                    headers.insert(k.clone(), s.into());
                }
            }
        }

        let body = params.get("body").map(|v| v.to_string());

        Ok(HttpRequest {
            url: url.into(),
            method: method.into(),
            headers,
            body,
            timeout_secs: Some(15),
        })
    }
}
