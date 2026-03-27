use crate::adapter::{HttpRequest, ToolError};
use std::collections::HashMap;

pub struct FileStorageTool;

impl FileStorageTool {
    pub fn build_request(
        action: &str,
        params: &serde_json::Value,
        _auth_token: &str,
    ) -> Result<HttpRequest, ToolError> {
        let bucket = params
            .get("bucket")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParameters("bucket required".into()))?;

        let endpoint = std::env::var("S3_ENDPOINT")
            .unwrap_or_else(|_| format!("https://{bucket}.s3.amazonaws.com"));

        let headers = HashMap::new();

        match action {
            "list" => Ok(HttpRequest {
                url: format!("{endpoint}/?list-type=2"),
                method: "GET".into(),
                headers,
                body: None,
                timeout_secs: None,
            }),
            "download" => {
                let key = params
                    .get("key")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ToolError::InvalidParameters("key required".into()))?;
                Ok(HttpRequest {
                    url: format!("{endpoint}/{key}"),
                    method: "GET".into(),
                    headers,
                    body: None,
                    timeout_secs: None,
                })
            }
            "upload" | "delete" => {
                let key = params
                    .get("key")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ToolError::InvalidParameters("key required".into()))?;
                Ok(HttpRequest {
                    url: format!("{endpoint}/{key}"),
                    method: if action == "upload" { "PUT" } else { "DELETE" }.into(),
                    headers,
                    body: None,
                    timeout_secs: None,
                })
            }
            _ => Err(ToolError::InvalidParameters(format!(
                "Unknown storage action: {action}"
            ))),
        }
    }
}
