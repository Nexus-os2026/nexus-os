use crate::adapter::{HttpRequest, ToolError};
use std::collections::HashMap;

pub struct SlackTool;

impl SlackTool {
    pub fn build_request(
        action: &str,
        params: &serde_json::Value,
        token: &str,
    ) -> Result<HttpRequest, ToolError> {
        let mut headers = HashMap::new();
        headers.insert("Authorization".into(), format!("Bearer {token}"));
        headers.insert("Content-Type".into(), "application/json".into());

        match action {
            "send_message" => {
                let channel = params
                    .get("channel")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ToolError::InvalidParameters("channel required".into()))?;
                let message = params
                    .get("message")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ToolError::InvalidParameters("message required".into()))?;
                Ok(HttpRequest {
                    url: "https://slack.com/api/chat.postMessage".into(),
                    method: "POST".into(),
                    headers,
                    body: Some(
                        serde_json::json!({"channel": channel, "text": message}).to_string(),
                    ),
                    timeout_secs: None,
                })
            }
            "list_channels" => Ok(HttpRequest {
                url: "https://slack.com/api/conversations.list".into(),
                method: "GET".into(),
                headers,
                body: None,
                timeout_secs: None,
            }),
            "get_history" => {
                let channel = params
                    .get("channel")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ToolError::InvalidParameters("channel required".into()))?;
                Ok(HttpRequest {
                    url: format!("https://slack.com/api/conversations.history?channel={channel}"),
                    method: "GET".into(),
                    headers,
                    body: None,
                    timeout_secs: None,
                })
            }
            _ => Err(ToolError::InvalidParameters(format!(
                "Unknown Slack action: {action}"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slack_request_build() {
        let req = SlackTool::build_request(
            "send_message",
            &serde_json::json!({"channel": "#general", "message": "hello"}),
            "xoxb-test",
        )
        .unwrap();
        assert!(req.url.contains("chat.postMessage"));
        assert_eq!(req.method, "POST");
        assert!(req.body.unwrap().contains("general"));
    }
}
