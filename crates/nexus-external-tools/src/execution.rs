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
}

impl ToolExecutionEngine {
    /// `_policy` is the governance policy the interface displays. The engine
    /// consulted only its URL denylist, for the tools whose destination the
    /// caller chose; Phase Zero refuses those tools outright instead
    /// ([`phase0_refusal`]), so the engine keeps no copy of it.
    pub fn new(registry: ToolRegistry, _policy: ToolGovernancePolicy) -> Self {
        Self {
            registry,
            adapter: HttpAdapter::new(),
            rate_limits: HashMap::new(),
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

        if autonomy_level < tool.min_autonomy_level {
            return Err(ToolError::GovernanceDenied(format!(
                "{} requires L{}+, agent is L{}",
                tool.name, tool.min_autonomy_level, autonomy_level,
            )));
        }

        // Phase Zero refusals come before any credential is read and before
        // any request is built (see `phase0_refusal`), and before the
        // availability check, so a refused tool's answer is the same whether
        // or not its environment token is set.
        if let Some(reason) = phase0_refusal(&tool.id) {
            return Err(ToolError::GovernanceDenied(format!(
                "{}: {reason}",
                tool.id
            )));
        }

        if !tool.available {
            return Err(ToolError::NotAvailable(format!(
                "{} requires {} to be set",
                tool.name,
                tool.auth_env_var.as_deref().unwrap_or("authentication"),
            )));
        }

        self.check_rate_limit(&tool)?;

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

/// Why Phase Zero refuses a tool whatever the calling agent's level, or
/// `None` when the tool may run.
///
/// - `rest_api`, `webhook` and `file_storage` (Final Gate item B): the
///   request goes to a host the caller chooses: a URL, or an S3 bucket name
///   that becomes the host. A URL or host name is not an egress grant, and
///   the substring denylist these tools had did not bound it (`127.1` and
///   `[::1]` reached loopback).
/// - `github`, `slack` and `jira` (items C and G): the request carries an
///   operator token from the environment, which would be placed on curl's
///   command line, and performs an operation the interface chose (an issue,
///   a message, a ticket) on the operator's account, behind an autonomy
///   level the interface also chose.
///
/// `web_search` (a fixed host, no credential) stays. `email` and `database`
/// are not listed here: their requests are built (an `smtp://` and a
/// `local://` URL) and then refused by the adapter's `check_request`, which
/// accepts only http(s) URLs with a host, before any process runs.
pub fn phase0_refusal(tool_id: &str) -> Option<&'static str> {
    match tool_id {
        "rest_api" | "webhook" | "file_storage" => Some(
            "unavailable in Phase Zero: the request would go to a destination the caller chose, and a URL or host name is not egress authority",
        ),
        "github" | "slack" | "jira" => Some(
            "unavailable in Phase Zero: the request would put an operator credential on a process command line to act on the operator's account for an interface-chosen operation",
        ),
        _ => None,
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

    /// A loopback listener that must never be contacted, and its port.
    fn quiet_listener() -> (std::net::TcpListener, u16) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let port = listener.local_addr().unwrap().port();
        (listener, port)
    }

    fn assert_never_contacted(listener: &std::net::TcpListener) {
        assert!(matches!(
            listener.accept(),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock
        ));
    }

    /// An engine whose every tool counts as available, as when the operator
    /// has set each tool's environment token, without reading or setting any
    /// environment variable here.
    fn engine_with_every_tool_available() -> ToolExecutionEngine {
        let mut registry = ToolRegistry::new();
        for tool in ToolRegistry::default_registry().all_tools() {
            let mut tool = tool.clone();
            tool.available = true;
            registry.register(tool);
        }
        ToolExecutionEngine::new(registry, ToolGovernancePolicy::default())
    }

    /// Final Gate item B (replaces the substring-denylist test): the tools
    /// whose destination the caller chooses are refused at every level,
    /// before any request, for every address. That covers the loopback and
    /// metadata addresses the denylist blocked and the spellings it missed
    /// (`127.1`, `[::1]`, `0x7f000001`).
    #[test]
    fn p0_fg_caller_destination_tools_are_refused() {
        let (listener, port) = quiet_listener();
        let mut engine = engine_with_every_tool_available();
        for (tool, params) in [
            (
                "rest_api",
                serde_json::json!({"url": format!("http://127.1:{port}/"), "method": "GET"}),
            ),
            (
                "rest_api",
                serde_json::json!({"url": format!("http://0x7f000001:{port}/x"), "method": "POST"}),
            ),
            (
                "rest_api",
                serde_json::json!({"url": "http://169.254.169.254/metadata", "method": "GET"}),
            ),
            (
                "webhook",
                serde_json::json!({"url": format!("http://127.1:{port}/hook")}),
            ),
            (
                "webhook",
                serde_json::json!({"url": "https://example.com/hook"}),
            ),
            (
                "file_storage",
                serde_json::json!({"action": "list", "bucket": "fg-attacker-bucket"}),
            ),
        ] {
            let error = engine.execute("agent-1", 5, tool, params).unwrap_err();
            assert!(
                matches!(&error, ToolError::GovernanceDenied(reason)
                    if reason.contains("destination the caller chose")),
                "{tool}: {error}"
            );
        }
        assert_never_contacted(&listener);
        assert!(engine.rate_limits.is_empty(), "no call was recorded");
    }

    /// Final Gate items C and G: the tools that would carry an operator
    /// token on curl's command line, for an operation the interface chose,
    /// are refused at every level before the token is read.
    #[test]
    fn p0_fg_operator_credential_tools_are_refused() {
        let mut engine = engine_with_every_tool_available();
        for (tool, params) in [
            (
                "github",
                serde_json::json!({"action": "create_issue", "repo": "o/r", "title": "t"}),
            ),
            (
                "slack",
                serde_json::json!({"action": "send_message", "channel": "C1", "text": "t"}),
            ),
            (
                "jira",
                serde_json::json!({"action": "create_issue", "project": "P", "summary": "s"}),
            ),
        ] {
            let error = engine.execute("agent-1", 5, tool, params).unwrap_err();
            assert!(
                matches!(&error, ToolError::GovernanceDenied(reason)
                    if reason.contains("operator credential")),
                "{tool}: {error}"
            );
        }
        assert!(engine.rate_limits.is_empty(), "no call was recorded");
    }

    /// A refused tool's answer does not depend on whether its environment
    /// token is set: the refusal comes before the availability check, so the
    /// answer does not reveal the token's presence either.
    #[test]
    fn p0_fg_refused_tools_answer_the_same_with_or_without_their_token() {
        let mut unset = ToolExecutionEngine::new(
            {
                let mut registry = ToolRegistry::new();
                for tool in ToolRegistry::default_registry().all_tools() {
                    let mut tool = tool.clone();
                    tool.available = tool.auth_env_var.is_none();
                    registry.register(tool);
                }
                registry
            },
            ToolGovernancePolicy::default(),
        );
        let mut set = engine_with_every_tool_available();
        for tool in [
            "github",
            "slack",
            "jira",
            "file_storage",
            "rest_api",
            "webhook",
        ] {
            let params = serde_json::json!({"action": "list"});
            let without = unset
                .execute("agent-1", 5, tool, params.clone())
                .unwrap_err();
            let with = set.execute("agent-1", 5, tool, params).unwrap_err();
            assert!(
                matches!(&without, ToolError::GovernanceDenied(reason) if reason.contains("Phase Zero")),
                "{tool}: {without}"
            );
            assert_eq!(without.to_string(), with.to_string(), "{tool}");
        }
    }

    /// `email` and `database` build their requests and are then refused by
    /// the adapter's URL check (smtp:// and local:// are not http(s)), before
    /// any process runs.
    #[test]
    fn p0_fg_email_and_database_fail_in_the_adapter_check() {
        let mut engine = engine_with_every_tool_available();
        for (tool, params) in [
            (
                "email",
                serde_json::json!({"to": "a@example.com", "subject": "s", "body": "b"}),
            ),
            (
                "database",
                serde_json::json!({"query": "SELECT 1", "database": "d"}),
            ),
        ] {
            let error = engine.execute("agent-1", 5, tool, params).unwrap_err();
            assert!(matches!(error, ToolError::UrlBlocked(_)), "{tool}: {error}");
        }
        assert!(engine.rate_limits.is_empty(), "no call was recorded");
    }

    /// The level check still comes first (the desktop guard pins "requires
    /// L4+" for an L2 agent), and the fixed-host search tool is not refused.
    #[test]
    fn p0_fg_phase0_refusals_keep_the_level_check_first_and_spare_search() {
        let mut engine = engine_with_every_tool_available();
        let error = engine
            .execute(
                "agent-1",
                2,
                "webhook",
                serde_json::json!({"url": "https://example.com"}),
            )
            .unwrap_err();
        assert!(error.to_string().contains("requires L4+"), "{error}");
        assert_eq!(phase0_refusal("web_search"), None);
        for tool in ["email", "database"] {
            assert_eq!(phase0_refusal(tool), None, "{tool} fails closed on its own");
        }
    }
}
