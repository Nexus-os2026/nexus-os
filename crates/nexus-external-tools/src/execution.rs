use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::adapter::{HttpAdapter, ToolError};
use crate::governance::ToolGovernancePolicy;
use crate::registry::{ExternalTool, ToolRegistry};

/// Tool execution engine — governance check → rate limit → execute → audit.
pub struct ToolExecutionEngine {
    registry: ToolRegistry,
    adapter: HttpAdapter,
    rate_limits: HashMap<String, (u64, u32)>,
    policy: ToolGovernancePolicy,
}

impl ToolExecutionEngine {
    pub fn new(registry: ToolRegistry, policy: ToolGovernancePolicy) -> Self {
        Self {
            registry,
            adapter: HttpAdapter::new(),
            rate_limits: HashMap::new(),
            policy,
        }
    }

    pub fn execute(
        &mut self,
        agent_id: &str,
        autonomy_level: u8,
        tool_id: &str,
        params: serde_json::Value,
    ) -> Result<ToolCallResult, ToolError> {
        let start = std::time::Instant::now();

        let tool = self
            .registry
            .get(tool_id)
            .ok_or_else(|| ToolError::NotFound(tool_id.into()))?
            .clone();

        if !tool.available {
            return Err(ToolError::NotAvailable(format!(
                "{} requires {} to be set",
                tool.name,
                tool.auth_env_var.as_deref().unwrap_or("authentication"),
            )));
        }

        if autonomy_level < tool.min_autonomy_level {
            return Err(ToolError::GovernanceDenied(format!(
                "{} requires L{}+, agent is L{}",
                tool.name, tool.min_autonomy_level, autonomy_level,
            )));
        }

        self.check_rate_limit(&tool)?;

        // URL denylist check for webhook/rest_api — fail-closed: missing URL is denied.
        if tool.id == "webhook" || tool.id == "rest_api" {
            let url = params.get("url").and_then(|v| v.as_str()).ok_or_else(|| {
                ToolError::GovernanceDenied(
                    "webhook/rest_api requires a 'url' parameter for denylist check".to_string(),
                )
            })?;
            self.check_url_allowed(url)?;
        }

        let auth_token = tool
            .auth_env_var
            .as_ref()
            .and_then(|var| std::env::var(var).ok())
            .unwrap_or_default();

        let action = params.get("action").and_then(|v| v.as_str()).unwrap_or("");

        let request = build_request(&tool, action, &params, &auth_token)?;
        let response = self.adapter.execute(&request)?;

        self.record_call(&tool.id);

        let duration_ms = start.elapsed().as_millis() as u64;

        Ok(ToolCallResult {
            tool_id: tool.id,
            tool_name: tool.name,
            agent_id: agent_id.into(),
            success: response.success,
            status_code: response.status_code,
            response_body: response.body,
            duration_ms,
            cost: tool.cost_per_call,
            has_side_effects: tool.has_side_effects,
            timestamp: epoch_now(),
        })
    }

    fn check_rate_limit(&self, tool: &ExternalTool) -> Result<(), ToolError> {
        if let Some((last_time, count)) = self.rate_limits.get(&tool.id) {
            let now = epoch_now();
            if now - last_time < 60 && *count >= tool.rate_limit {
                return Err(ToolError::RateLimited(format!(
                    "{}: {} calls/min (max {})",
                    tool.name, count, tool.rate_limit,
                )));
            }
        }
        Ok(())
    }

    fn check_url_allowed(&self, url: &str) -> Result<(), ToolError> {
        let lower = url.to_lowercase();
        for blocked in &self.policy.url_denylist {
            if lower.contains(blocked) {
                return Err(ToolError::UrlBlocked(format!(
                    "URL contains blocked pattern: {blocked}"
                )));
            }
        }
        Ok(())
    }

    fn record_call(&mut self, tool_id: &str) {
        let now = epoch_now();
        let entry = self.rate_limits.entry(tool_id.into()).or_insert((now, 0));
        if now - entry.0 >= 60 {
            *entry = (now, 1);
        } else {
            entry.1 += 1;
        }
    }

    pub fn registry(&self) -> &ToolRegistry {
        &self.registry
    }

    pub fn registry_mut(&mut self) -> &mut ToolRegistry {
        &mut self.registry
    }
}

fn build_request(
    tool: &ExternalTool,
    action: &str,
    params: &serde_json::Value,
    auth_token: &str,
) -> Result<crate::adapter::HttpRequest, ToolError> {
    match tool.id.as_str() {
        "github" => crate::tools::github::GitHubTool::build_request(action, params, auth_token),
        "slack" => crate::tools::slack::SlackTool::build_request(action, params, auth_token),
        "jira" => crate::tools::jira::JiraTool::build_request(action, params, auth_token),
        "web_search" => crate::tools::web_search::WebSearchTool::build_request(params),
        "webhook" => crate::tools::webhook::WebhookTool::build_request(params),
        "rest_api" => crate::tools::rest_api::RestApiTool::build_request(params),
        "email" => crate::tools::email::EmailTool::build_request(params, auth_token),
        "database" => crate::tools::database::DatabaseTool::build_request(params, auth_token),
        "file_storage" => {
            crate::tools::file_storage::FileStorageTool::build_request(action, params, auth_token)
        }
        _ => Err(ToolError::NotFound(tool.id.clone())),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallResult {
    pub tool_id: String,
    pub tool_name: String,
    pub agent_id: String,
    pub success: bool,
    pub status_code: u16,
    pub response_body: String,
    pub duration_ms: u64,
    pub cost: u64,
    pub has_side_effects: bool,
    pub timestamp: u64,
}

fn epoch_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_engine() -> ToolExecutionEngine {
        ToolExecutionEngine::new(
            ToolRegistry::default_registry(),
            ToolGovernancePolicy::default(),
        )
    }

    #[test]
    fn test_governance_autonomy_check() {
        let mut engine = make_engine();
        // webhook is always available but requires L4+
        let result = engine.execute(
            "agent-1",
            2,
            "webhook",
            serde_json::json!({"url": "https://example.com"}),
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("requires L4+"));
    }

    #[test]
    fn test_governance_autonomy_allowed() {
        let mut engine = make_engine();
        // web_search requires L2+ and is always available
        let result = engine.execute(
            "agent-1",
            4,
            "web_search",
            serde_json::json!({"query": "test"}),
        );
        // Will fail with curl error in test env, but governance passes
        assert!(
            result.is_ok()
                || result.as_ref().unwrap_err().to_string().contains("curl")
                || result
                    .as_ref()
                    .unwrap_err()
                    .to_string()
                    .contains("Execution")
        );
    }

    #[test]
    fn test_rate_limit_enforcement() {
        let mut engine = make_engine();
        // Manually set rate limit to exhausted
        engine
            .rate_limits
            .insert("web_search".into(), (epoch_now(), 31));
        let result = engine.execute("a1", 5, "web_search", serde_json::json!({"query": "test"}));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Rate limit"));
    }

    #[test]
    fn test_rate_limit_reset() {
        let mut engine = make_engine();
        // Set old rate limit (61 seconds ago)
        engine
            .rate_limits
            .insert("web_search".into(), (epoch_now() - 61, 100));
        // Should not be rate limited
        let check = engine.check_rate_limit(engine.registry().get("web_search").unwrap());
        assert!(check.is_ok());
    }

    #[test]
    fn test_url_denylist() {
        let engine = make_engine();
        assert!(engine.check_url_allowed("https://example.com").is_ok());
        assert!(engine.check_url_allowed("http://localhost:8080").is_err());
        assert!(engine
            .check_url_allowed("http://169.254.169.254/metadata")
            .is_err());
    }
}
