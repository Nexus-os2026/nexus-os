use crate::adapter::{HttpRequest, ToolError};
use std::collections::HashMap;

pub struct GitHubTool;

impl GitHubTool {
    pub fn build_request(
        action: &str,
        params: &serde_json::Value,
        token: &str,
    ) -> Result<HttpRequest, ToolError> {
        let mut headers = HashMap::new();
        headers.insert("Authorization".into(), format!("Bearer {token}"));
        headers.insert("Accept".into(), "application/vnd.github.v3+json".into());
        headers.insert("User-Agent".into(), "NexusOS-Agent".into());

        match action {
            "list_repos" => {
                let user = params.get("user").and_then(|v| v.as_str()).unwrap_or("me");
                Ok(HttpRequest {
                    url: if user == "me" {
                        "https://api.github.com/user/repos".into()
                    } else {
                        format!("https://api.github.com/users/{user}/repos")
                    },
                    method: "GET".into(),
                    headers,
                    body: None,
                    timeout_secs: None,
                })
            }
            "create_issue" => {
                let repo = params
                    .get("repo")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ToolError::InvalidParameters("repo required".into()))?;
                let title = params
                    .get("title")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ToolError::InvalidParameters("title required".into()))?;
                let body_text = params.get("body").and_then(|v| v.as_str()).unwrap_or("");
                Ok(HttpRequest {
                    url: format!("https://api.github.com/repos/{repo}/issues"),
                    method: "POST".into(),
                    headers,
                    body: Some(serde_json::json!({"title": title, "body": body_text}).to_string()),
                    timeout_secs: None,
                })
            }
            "get_pr" => {
                let repo = params
                    .get("repo")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ToolError::InvalidParameters("repo required".into()))?;
                let number = params
                    .get("number")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| ToolError::InvalidParameters("number required".into()))?;
                Ok(HttpRequest {
                    url: format!("https://api.github.com/repos/{repo}/pulls/{number}"),
                    method: "GET".into(),
                    headers,
                    body: None,
                    timeout_secs: None,
                })
            }
            "search_code" => {
                let query = params
                    .get("query")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ToolError::InvalidParameters("query required".into()))?;
                Ok(HttpRequest {
                    url: format!("https://api.github.com/search/code?q={}", urlencoded(query)),
                    method: "GET".into(),
                    headers,
                    body: None,
                    timeout_secs: None,
                })
            }
            _ => Err(ToolError::InvalidParameters(format!(
                "Unknown GitHub action: {action}"
            ))),
        }
    }
}

fn urlencoded(s: &str) -> String {
    s.replace(' ', "+").replace('&', "%26").replace('=', "%3D")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_github_request_build() {
        let req = GitHubTool::build_request(
            "list_repos",
            &serde_json::json!({"user": "octocat"}),
            "test-token",
        )
        .unwrap();
        assert!(req.url.contains("octocat"));
        assert_eq!(req.method, "GET");
        assert!(req
            .headers
            .get("Authorization")
            .unwrap()
            .contains("test-token"));
    }
}
