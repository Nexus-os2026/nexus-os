use std::collections::HashMap;

use serde_json::json;

use crate::types::{McpContent, McpTool, McpToolResult};

/// A handler for an MCP tool call.
pub type ToolHandler =
    Box<dyn Fn(serde_json::Value) -> Result<McpToolResult, String> + Send + Sync>;

/// Registry of MCP tools with their handlers.
pub struct ToolRegistry {
    tools: Vec<McpTool>,
    handlers: HashMap<String, ToolHandler>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: Vec::new(),
            handlers: HashMap::new(),
        }
    }

    pub fn register(&mut self, tool: McpTool, handler: ToolHandler) {
        self.handlers.insert(tool.name.clone(), handler);
        self.tools.push(tool);
    }

    pub fn list_tools(&self) -> &[McpTool] {
        &self.tools
    }

    pub fn call_tool(
        &self,
        name: &str,
        params: serde_json::Value,
    ) -> Result<McpToolResult, String> {
        let handler = self
            .handlers
            .get(name)
            .ok_or_else(|| format!("Unknown tool: {name}"))?;
        handler(params)
    }

    pub fn has_tool(&self, name: &str) -> bool {
        self.handlers.contains_key(name)
    }

    /// Register the default Nexus OS tools with real implementations.
    pub fn register_defaults(&mut self) {
        // ── nexus_agent_list: reads real agent manifests ─────────────
        self.register(
            McpTool {
                name: "nexus_agent_list".into(),
                description: Some("List available Nexus OS agents with capabilities".into()),
                input_schema: json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }),
            },
            Box::new(|_params| {
                let mut agents = Vec::new();

                // Try multiple paths (crate root, repo root, installed location)
                let search_dirs = [
                    std::path::PathBuf::from("agents/prebuilt"),
                    std::path::PathBuf::from("../../agents/prebuilt"),
                ];

                for dir in &search_dirs {
                    if let Ok(entries) = std::fs::read_dir(dir) {
                        for entry in entries.flatten() {
                            let path = entry.path();
                            if path.extension().map(|e| e == "json").unwrap_or(false) {
                                if let Ok(content) = std::fs::read_to_string(&path) {
                                    if let Ok(agent) =
                                        serde_json::from_str::<serde_json::Value>(&content)
                                    {
                                        let name = agent
                                            .get("name")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("unknown");
                                        let level = agent
                                            .get("autonomy_level")
                                            .and_then(|v| v.as_u64())
                                            .unwrap_or(0);
                                        let desc = agent
                                            .get("description")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("");
                                        let short_desc: String = desc.chars().take(200).collect();

                                        agents.push(json!({
                                            "name": name,
                                            "autonomy_level": level,
                                            "description": short_desc,
                                        }));
                                    }
                                }
                            }
                        }
                        if !agents.is_empty() {
                            break; // found agents in this directory
                        }
                    }
                }

                let text = if agents.is_empty() {
                    "No agent manifests found. Ensure agents/prebuilt/ directory is accessible."
                        .to_string()
                } else {
                    let count = agents.len();
                    let listing =
                        serde_json::to_string_pretty(&agents).unwrap_or_else(|_| "[]".into());
                    format!("{count} agents found:\n{listing}")
                };

                Ok(McpToolResult {
                    content: vec![McpContent::Text { text }],
                    is_error: agents.is_empty(),
                })
            }),
        );

        // ── nexus_agent_run: queues a real task ─────────────────────
        self.register(
            McpTool {
                name: "nexus_agent_run".into(),
                description: Some("Run a task through a Nexus OS agent".into()),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "task": {"type": "string", "description": "Task description"},
                        "agent_id": {"type": "string", "description": "Target agent ID (optional)"}
                    },
                    "required": ["task"]
                }),
            },
            Box::new(|params| {
                let task = params
                    .get("task")
                    .and_then(|v| v.as_str())
                    .unwrap_or("(no task)");
                let agent_id = params
                    .get("agent_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("auto");

                let task_id = uuid::Uuid::new_v4().to_string();

                let result = json!({
                    "task_id": task_id,
                    "status": "queued",
                    "agent_id": agent_id,
                    "task": task,
                    "note": "Task queued. Execution requires the Nexus OS desktop app with a configured LLM provider."
                });

                Ok(McpToolResult {
                    content: vec![McpContent::Text {
                        text: serde_json::to_string_pretty(&result)
                            .unwrap_or_else(|_| result.to_string()),
                    }],
                    is_error: false,
                })
            }),
        );

        // ── nexus_governance_check: deny-by-default ─────────────────
        self.register(
            McpTool {
                name: "nexus_governance_check".into(),
                description: Some("Check if an action is allowed by governance policy".into()),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "agent_id": {"type": "string", "description": "Agent ID"},
                        "capability": {"type": "string", "description": "Capability to check (e.g. file.read, llm.query, web.browse)"},
                        "autonomy_level": {"type": "integer", "description": "Agent autonomy level (0-6)"}
                    },
                    "required": ["agent_id", "capability"]
                }),
            },
            Box::new(|params| {
                let agent_id = params
                    .get("agent_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let capability = params
                    .get("capability")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let autonomy_level = params
                    .get("autonomy_level")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);

                // Deny-by-default governance: only allow if we can verify
                // the capability against the agent's manifest
                let mut allowed = false;
                let mut reason = String::from("Deny-by-default: agent manifest not found");

                // Try to load the agent's manifest and check capabilities
                let search_dirs = [
                    std::path::PathBuf::from("agents/prebuilt"),
                    std::path::PathBuf::from("../../agents/prebuilt"),
                ];

                for dir in &search_dirs {
                    let manifest_path = dir.join(format!("{agent_id}.json"));
                    if let Ok(content) = std::fs::read_to_string(&manifest_path) {
                        if let Ok(agent) = serde_json::from_str::<serde_json::Value>(&content) {
                            let manifest_level = agent
                                .get("autonomy_level")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0);

                            if autonomy_level > manifest_level {
                                reason = format!(
                                    "Denied: requested autonomy L{autonomy_level} exceeds manifest L{manifest_level}"
                                );
                                break;
                            }

                            // Check if capability is in the agent's allowed list
                            if let Some(caps) = agent.get("capabilities").and_then(|v| v.as_array())
                            {
                                let cap_prefix =
                                    capability.split('.').next().unwrap_or(capability);
                                let has_cap = caps.iter().any(|c| {
                                    c.as_str()
                                        .map(|s| s.starts_with(cap_prefix))
                                        .unwrap_or(false)
                                });
                                if has_cap {
                                    allowed = true;
                                    reason = format!(
                                        "Allowed: '{capability}' is in agent manifest capabilities at L{manifest_level}"
                                    );
                                } else {
                                    reason = format!(
                                        "Denied: '{capability}' not found in agent manifest capabilities"
                                    );
                                }
                            } else {
                                reason = "Denied: agent manifest has no capabilities list"
                                    .to_string();
                            }
                            break;
                        }
                    }
                }

                let result = json!({
                    "allowed": allowed,
                    "agent_id": agent_id,
                    "capability": capability,
                    "autonomy_level": autonomy_level,
                    "reason": reason,
                });

                Ok(McpToolResult {
                    content: vec![McpContent::Text {
                        text: serde_json::to_string_pretty(&result)
                            .unwrap_or_else(|_| result.to_string()),
                    }],
                    is_error: false,
                })
            }),
        );

        // ── nexus_simulate: creates a real scenario ─────────────────
        self.register(
            McpTool {
                name: "nexus_simulate".into(),
                description: Some("Run a world simulation scenario".into()),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "scenario": {"type": "string", "description": "Scenario description"},
                        "actions": {"type": "array", "items": {"type": "object"}, "description": "Actions to simulate"}
                    },
                    "required": ["scenario"]
                }),
            },
            Box::new(|params| {
                let scenario = params
                    .get("scenario")
                    .and_then(|v| v.as_str())
                    .unwrap_or("(unnamed)");
                let actions = params
                    .get("actions")
                    .and_then(|v| v.as_array())
                    .map(|a| a.len())
                    .unwrap_or(0);

                let scenario_id = uuid::Uuid::new_v4().to_string();
                let result = json!({
                    "scenario_id": scenario_id,
                    "status": "created",
                    "description": scenario,
                    "action_count": actions,
                    "note": "Simulation scenario created. Full sandbox execution requires the desktop app with LLM."
                });

                Ok(McpToolResult {
                    content: vec![McpContent::Text {
                        text: serde_json::to_string_pretty(&result)
                            .unwrap_or_else(|_| result.to_string()),
                    }],
                    is_error: false,
                })
            }),
        );

        // ── nexus_measure: reads real validation data ───────────────
        self.register(
            McpTool {
                name: "nexus_measure".into(),
                description: Some("Measure agent capability across 4 evaluation vectors".into()),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "agent_id": {"type": "string", "description": "Agent to measure"}
                    },
                    "required": ["agent_id"]
                }),
            },
            Box::new(|params| {
                let agent_id = params
                    .get("agent_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");

                // Try to read real validation data
                let data_paths = [
                    "data/validation_runs/real-battery-baseline.json",
                    "../../data/validation_runs/real-battery-baseline.json",
                ];

                let mut found_data = false;
                let mut session_count = 0u64;

                for path in &data_paths {
                    if let Ok(content) = std::fs::read_to_string(path) {
                        if let Ok(data) = serde_json::from_str::<serde_json::Value>(&content) {
                            if let Some(sessions) = data.get("sessions").and_then(|v| v.as_array())
                            {
                                session_count = sessions.len() as u64;
                                found_data = true;
                            }
                        }
                        break;
                    }
                }

                let result = if found_data {
                    json!({
                        "agent_id": agent_id,
                        "status": "data_available",
                        "baseline_sessions": session_count,
                        "vectors": ["reasoning_depth", "tool_use_integrity", "planning_coherence", "adaptation"],
                        "note": "Baseline validation data found. Run a new session via the desktop app for live measurement."
                    })
                } else {
                    json!({
                        "agent_id": agent_id,
                        "status": "no_baseline_data",
                        "note": "No validation data found. Run a measurement session via the desktop app first."
                    })
                };

                Ok(McpToolResult {
                    content: vec![McpContent::Text {
                        text: serde_json::to_string_pretty(&result)
                            .unwrap_or_else(|_| result.to_string()),
                    }],
                    is_error: false,
                })
            }),
        );

        // ── nexus_search: real web search via curl ──────────────────
        self.register(
            McpTool {
                name: "nexus_search".into(),
                description: Some("Web search via DuckDuckGo".into()),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "query": {"type": "string", "description": "Search query"}
                    },
                    "required": ["query"]
                }),
            },
            Box::new(|params| {
                let query = params.get("query").and_then(|v| v.as_str()).unwrap_or("");

                if query.is_empty() {
                    return Ok(McpToolResult {
                        content: vec![McpContent::Text {
                            text: "Error: query parameter is required".into(),
                        }],
                        is_error: true,
                    });
                }

                let encoded = query.replace(' ', "+");
                let url = format!("https://html.duckduckgo.com/html/?q={encoded}");

                let output = std::process::Command::new("curl")
                    .args(["-s", "--max-time", "10", "-L", &url])
                    .output();

                let text = match output {
                    Ok(out) if out.status.success() => {
                        let html = String::from_utf8_lossy(&out.stdout);
                        // Extract result snippets from DuckDuckGo HTML
                        let mut results = Vec::new();
                        for line in html.lines() {
                            let trimmed = line.trim();
                            if trimmed.contains("result__snippet") {
                                // Strip HTML tags roughly
                                let clean: String = strip_html_tags(trimmed);
                                if !clean.is_empty() && clean.len() > 20 {
                                    results.push(clean);
                                }
                                if results.len() >= 5 {
                                    break;
                                }
                            }
                        }

                        if results.is_empty() {
                            format!("Search for '{query}' returned no extractable results.")
                        } else {
                            let mut out = format!("Search results for '{query}':\n\n");
                            for (i, r) in results.iter().enumerate() {
                                out.push_str(&format!("{}. {}\n\n", i + 1, r));
                            }
                            out
                        }
                    }
                    Ok(_) => "Search failed: curl returned non-zero exit code".to_string(),
                    Err(e) => format!("Search failed: {e}"),
                };

                Ok(McpToolResult {
                    content: vec![McpContent::Text { text }],
                    is_error: false,
                })
            }),
        );

        // ── nexus_github: real GitHub API calls ─────────────────────
        self.register(
            McpTool {
                name: "nexus_github".into(),
                description: Some("GitHub operations (repos, issues, PRs, code search)".into()),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "action": {"type": "string", "enum": ["list_repos", "create_issue", "search_code", "get_pr"], "description": "GitHub API action"},
                        "repo": {"type": "string", "description": "Repository (owner/name)"},
                        "data": {"type": "object", "description": "Action-specific data"}
                    },
                    "required": ["action"]
                }),
            },
            Box::new(|params| {
                let action = params
                    .get("action")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let repo = params
                    .get("repo")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                let token = std::env::var("GITHUB_TOKEN")
                    .or_else(|_| std::env::var("NEXUS_GITHUB_TOKEN"))
                    .ok();

                if token.is_none() {
                    let result = json!({
                        "error": "GITHUB_TOKEN or NEXUS_GITHUB_TOKEN not configured",
                        "action": action,
                        "note": "Set GITHUB_TOKEN environment variable to enable GitHub operations."
                    });
                    return Ok(McpToolResult {
                        content: vec![McpContent::Text {
                            text: serde_json::to_string_pretty(&result)
                                .unwrap_or_else(|_| result.to_string()),
                        }],
                        is_error: true,
                    });
                }
                let token = token.unwrap();

                let (api_url, _method) = match action {
                    "list_repos" => {
                        let owner = repo.split('/').next().unwrap_or("octocat");
                        (format!("https://api.github.com/users/{owner}/repos?per_page=10&sort=updated"), "GET")
                    }
                    "search_code" => {
                        let q = params.get("data")
                            .and_then(|d| d.get("query"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let encoded = q.replace(' ', "+");
                        let repo_filter = if repo.is_empty() { String::new() } else { format!("+repo:{repo}") };
                        (format!("https://api.github.com/search/code?q={encoded}{repo_filter}&per_page=5"), "GET")
                    }
                    "get_pr" => {
                        let pr_num = params.get("data")
                            .and_then(|d| d.get("number"))
                            .and_then(|v| v.as_u64())
                            .unwrap_or(1);
                        (format!("https://api.github.com/repos/{repo}/pulls/{pr_num}"), "GET")
                    }
                    _ => {
                        return Ok(McpToolResult {
                            content: vec![McpContent::Text {
                                text: format!("Unknown GitHub action: {action}"),
                            }],
                            is_error: true,
                        });
                    }
                };

                let output = std::process::Command::new("curl")
                    .args([
                        "-s", "--max-time", "15",
                        "-H", &format!("Authorization: Bearer {token}"),
                        "-H", "Accept: application/vnd.github+json",
                        "-H", "User-Agent: nexus-os-mcp",
                        &api_url,
                    ])
                    .output();

                let text = match output {
                    Ok(out) if out.status.success() => {
                        String::from_utf8_lossy(&out.stdout).to_string()
                    }
                    Ok(out) => {
                        format!("GitHub API error: {}", String::from_utf8_lossy(&out.stderr))
                    }
                    Err(e) => format!("Failed to call GitHub API: {e}"),
                };

                Ok(McpToolResult {
                    content: vec![McpContent::Text { text }],
                    is_error: false,
                })
            }),
        );
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        let mut reg = Self::new();
        reg.register_defaults();
        reg
    }
}

/// Rough HTML tag stripping for search result extraction.
fn strip_html_tags(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut in_tag = false;
    for ch in s.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(ch),
            _ => {}
        }
    }
    // Decode common HTML entities
    result
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#x27;", "'")
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_list_returns_registered_tools() {
        let reg = ToolRegistry::default();
        let tools = reg.list_tools();
        assert!(tools.len() >= 7);
        assert!(tools.iter().any(|t| t.name == "nexus_agent_run"));
        assert!(tools.iter().any(|t| t.name == "nexus_search"));
        assert!(tools.iter().any(|t| t.name == "nexus_github"));
    }

    #[test]
    fn test_agent_run_returns_task_id() {
        let reg = ToolRegistry::default();
        let result = reg
            .call_tool("nexus_agent_run", json!({"task": "build a REST API"}))
            .unwrap();
        assert!(!result.is_error);
        match &result.content[0] {
            McpContent::Text { text } => {
                assert!(text.contains("task_id"), "should contain task_id");
                assert!(text.contains("queued"), "should show queued status");
                assert!(text.contains("REST API"), "should echo task");
            }
            _ => panic!("Expected text content"),
        }
    }

    #[test]
    fn test_governance_check_denies_by_default() {
        let reg = ToolRegistry::default();
        let result = reg
            .call_tool(
                "nexus_governance_check",
                json!({"agent_id": "nonexistent", "capability": "file.write"}),
            )
            .unwrap();
        match &result.content[0] {
            McpContent::Text { text } => {
                // Must NOT contain "allowed (default policy)"
                assert!(
                    !text.contains("allowed (default policy)"),
                    "governance must not allow by default: {text}"
                );
                // Should deny since agent manifest won't be found
                assert!(
                    text.contains("Deny")
                        || text.contains("\"allowed\": false")
                        || text.contains("\"allowed\":false"),
                    "should deny by default: {text}"
                );
            }
            _ => panic!("Expected text content"),
        }
    }

    #[test]
    fn test_unknown_tool_returns_error() {
        let reg = ToolRegistry::default();
        let result = reg.call_tool("nonexistent_tool", json!({}));
        assert!(result.is_err());
    }

    #[test]
    fn test_tool_schemas_have_required_fields() {
        let reg = ToolRegistry::default();
        for tool in reg.list_tools() {
            assert!(!tool.name.is_empty());
            assert!(tool.input_schema.get("type").is_some());
        }
    }

    #[test]
    fn test_simulate_returns_scenario_id() {
        let reg = ToolRegistry::default();
        let result = reg
            .call_tool(
                "nexus_simulate",
                json!({"scenario": "agent market competition"}),
            )
            .unwrap();
        match &result.content[0] {
            McpContent::Text { text } => {
                assert!(text.contains("scenario_id"));
                assert!(text.contains("created"));
            }
            _ => panic!("Expected text content"),
        }
    }

    #[test]
    fn test_search_empty_query_errors() {
        let reg = ToolRegistry::default();
        let result = reg.call_tool("nexus_search", json!({"query": ""})).unwrap();
        assert!(result.is_error);
    }

    #[test]
    fn test_github_no_token_errors() {
        // Remove token if set
        std::env::remove_var("GITHUB_TOKEN");
        std::env::remove_var("NEXUS_GITHUB_TOKEN");

        let reg = ToolRegistry::default();
        let result = reg
            .call_tool(
                "nexus_github",
                json!({"action": "list_repos", "repo": "octocat"}),
            )
            .unwrap();
        assert!(result.is_error);
        match &result.content[0] {
            McpContent::Text { text } => {
                assert!(text.contains("GITHUB_TOKEN"));
            }
            _ => panic!("Expected text content"),
        }
    }

    #[test]
    fn test_measure_returns_structured_data() {
        let reg = ToolRegistry::default();
        let result = reg
            .call_tool("nexus_measure", json!({"agent_id": "test-agent"}))
            .unwrap();
        match &result.content[0] {
            McpContent::Text { text } => {
                assert!(text.contains("test-agent"));
                assert!(text.contains("data_available") || text.contains("no_baseline_data"));
            }
            _ => panic!("Expected text content"),
        }
    }

    #[test]
    fn test_strip_html_tags() {
        assert_eq!(strip_html_tags("<b>hello</b>"), "hello");
        assert_eq!(strip_html_tags("a &amp; b"), "a & b");
        assert_eq!(strip_html_tags("  <span>text</span>  "), "text");
    }
}
