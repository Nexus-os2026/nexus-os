use crate::adapter::{HttpRequest, ToolError};
use std::collections::HashMap;

pub struct EmailTool;

impl EmailTool {
    pub fn build_request(
        params: &serde_json::Value,
        _auth_token: &str,
    ) -> Result<HttpRequest, ToolError> {
        let to = params
            .get("to")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParameters("to required".into()))?;
        let subject = params
            .get("subject")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParameters("subject required".into()))?;
        let body = params
            .get("body")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParameters("body required".into()))?;

        // Use a local sendmail/msmtp approach via curl to a mail relay
        let smtp_host = std::env::var("SMTP_HOST").unwrap_or_else(|_| "localhost:587".into());
        let from = std::env::var("SMTP_FROM").unwrap_or_else(|_| "nexus@localhost".into());

        let headers = HashMap::new();
        let mail_body = format!("From: {from}\r\nTo: {to}\r\nSubject: {subject}\r\n\r\n{body}");

        Ok(HttpRequest {
            url: format!("smtp://{smtp_host}"),
            method: "POST".into(),
            headers,
            body: Some(mail_body),
            timeout_secs: Some(15),
        })
    }
}
