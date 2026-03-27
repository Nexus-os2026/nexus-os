use crate::adapter::{HttpRequest, ToolError};
use std::collections::HashMap;

pub struct WebSearchTool;

impl WebSearchTool {
    pub fn build_request(params: &serde_json::Value) -> Result<HttpRequest, ToolError> {
        let query = params
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParameters("query required".into()))?;

        let encoded = query
            .replace(' ', "+")
            .replace('&', "%26")
            .replace('=', "%3D");

        let mut headers = HashMap::new();
        headers.insert("User-Agent".into(), "NexusOS-Agent/1.0".into());

        Ok(HttpRequest {
            url: format!("https://html.duckduckgo.com/html/?q={encoded}"),
            method: "GET".into(),
            headers,
            body: None,
            timeout_secs: Some(15),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_web_search_request_build() {
        let req = WebSearchTool::build_request(&serde_json::json!({"query": "rust programming"}))
            .unwrap();
        assert!(req.url.contains("rust+programming"));
        assert_eq!(req.method, "GET");
    }
}
