use serde::{Deserialize, Serialize};

/// A registered external tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalTool {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: ToolCategory,
    pub parameters: Vec<ToolParameter>,
    pub return_type: String,
    pub requires_auth: bool,
    pub auth_env_var: Option<String>,
    pub available: bool,
    pub required_capability: String,
    pub min_autonomy_level: u8,
    pub cost_per_call: u64,
    pub has_side_effects: bool,
    pub rate_limit: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolCategory {
    CodeRepository,
    ProjectManagement,
    Communication,
    Search,
    Database,
    Storage,
    Webhook,
    RestApi,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolParameter {
    pub name: String,
    pub description: String,
    pub param_type: ParamType,
    pub required: bool,
    pub default: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ParamType {
    String,
    Integer,
    Boolean,
    Json,
    Url,
    FilePath,
}

/// The tool registry — manages all available external tools.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolRegistry {
    tools: Vec<ExternalTool>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self { tools: Vec::new() }
    }

    pub fn register(&mut self, tool: ExternalTool) {
        if let Some(existing) = self.tools.iter_mut().find(|t| t.id == tool.id) {
            *existing = tool;
        } else {
            self.tools.push(tool);
        }
    }

    pub fn available_tools(&self) -> Vec<&ExternalTool> {
        self.tools.iter().filter(|t| t.available).collect()
    }

    pub fn tools_by_category(&self, category: &ToolCategory) -> Vec<&ExternalTool> {
        self.tools
            .iter()
            .filter(|t| t.category == *category)
            .collect()
    }

    pub fn get(&self, tool_id: &str) -> Option<&ExternalTool> {
        self.tools.iter().find(|t| t.id == tool_id)
    }

    pub fn refresh_availability(&mut self) {
        for tool in &mut self.tools {
            tool.available = if let Some(ref env_var) = tool.auth_env_var {
                std::env::var(env_var).is_ok()
            } else {
                true
            };
        }
    }

    pub fn all_tools(&self) -> &[ExternalTool] {
        &self.tools
    }

    pub fn default_registry() -> Self {
        let mut registry = Self::new();

        let p = |name: &str, desc: &str, pt: ParamType, req: bool| ToolParameter {
            name: name.into(),
            description: desc.into(),
            param_type: pt,
            required: req,
            default: None,
        };

        registry.register(ExternalTool {
            id: "github".into(),
            name: "GitHub".into(),
            description: "GitHub API — repos, issues, PRs, code search".into(),
            category: ToolCategory::CodeRepository,
            parameters: vec![
                p(
                    "action",
                    "API action (list_repos, create_issue, search_code)",
                    ParamType::String,
                    true,
                ),
                p("repo", "Repository (owner/name)", ParamType::String, false),
                p("data", "Request body (JSON)", ParamType::Json, false),
            ],
            return_type: "JSON".into(),
            requires_auth: true,
            auth_env_var: Some("GITHUB_TOKEN".into()),
            available: false,
            required_capability: "external_tool.github".into(),
            min_autonomy_level: 3,
            cost_per_call: 2_000_000,
            has_side_effects: true,
            rate_limit: 30,
        });

        registry.register(ExternalTool {
            id: "slack".into(),
            name: "Slack".into(),
            description: "Slack API — send messages, list channels".into(),
            category: ToolCategory::Communication,
            parameters: vec![
                p(
                    "action",
                    "API action (send_message, list_channels)",
                    ParamType::String,
                    true,
                ),
                p("channel", "Channel name or ID", ParamType::String, false),
                p("message", "Message text", ParamType::String, false),
            ],
            return_type: "JSON".into(),
            requires_auth: true,
            auth_env_var: Some("SLACK_TOKEN".into()),
            available: false,
            required_capability: "external_tool.slack".into(),
            min_autonomy_level: 3,
            cost_per_call: 1_000_000,
            has_side_effects: true,
            rate_limit: 60,
        });

        registry.register(ExternalTool {
            id: "email".into(),
            name: "Email".into(),
            description: "Send email via SMTP".into(),
            category: ToolCategory::Communication,
            parameters: vec![
                p("to", "Recipient email", ParamType::String, true),
                p("subject", "Email subject", ParamType::String, true),
                p("body", "Email body", ParamType::String, true),
            ],
            return_type: "status".into(),
            requires_auth: true,
            auth_env_var: Some("SMTP_PASSWORD".into()),
            available: false,
            required_capability: "external_tool.email".into(),
            min_autonomy_level: 4,
            cost_per_call: 5_000_000,
            has_side_effects: true,
            rate_limit: 10,
        });

        registry.register(ExternalTool {
            id: "web_search".into(),
            name: "Web Search".into(),
            description: "Search the web via DuckDuckGo".into(),
            category: ToolCategory::Search,
            parameters: vec![
                p("query", "Search query", ParamType::String, true),
                ToolParameter {
                    name: "max_results".into(),
                    description: "Maximum results".into(),
                    param_type: ParamType::Integer,
                    required: false,
                    default: Some(serde_json::json!(10)),
                },
            ],
            return_type: "JSON array".into(),
            requires_auth: false,
            auth_env_var: None,
            available: true,
            required_capability: "external_tool.search".into(),
            min_autonomy_level: 2,
            cost_per_call: 500_000,
            has_side_effects: false,
            rate_limit: 30,
        });

        registry.register(ExternalTool {
            id: "database".into(),
            name: "Database".into(),
            description: "Execute SQL queries against PostgreSQL or SQLite".into(),
            category: ToolCategory::Database,
            parameters: vec![
                p("query", "SQL query", ParamType::String, true),
                p(
                    "database",
                    "Database connection string",
                    ParamType::String,
                    true,
                ),
                ToolParameter {
                    name: "read_only".into(),
                    description: "Enforce read-only".into(),
                    param_type: ParamType::Boolean,
                    required: false,
                    default: Some(serde_json::json!(true)),
                },
            ],
            return_type: "JSON rows".into(),
            requires_auth: true,
            auth_env_var: Some("DATABASE_URL".into()),
            available: false,
            required_capability: "external_tool.database".into(),
            min_autonomy_level: 4,
            cost_per_call: 3_000_000,
            has_side_effects: true,
            rate_limit: 20,
        });

        registry.register(ExternalTool {
            id: "jira".into(),
            name: "Jira".into(),
            description: "Jira API — issues, boards, sprints".into(),
            category: ToolCategory::ProjectManagement,
            parameters: vec![
                p(
                    "action",
                    "API action (list_issues, create_issue, update_issue)",
                    ParamType::String,
                    true,
                ),
                p("project", "Project key", ParamType::String, false),
                p("data", "Request body", ParamType::Json, false),
            ],
            return_type: "JSON".into(),
            requires_auth: true,
            auth_env_var: Some("JIRA_TOKEN".into()),
            available: false,
            required_capability: "external_tool.jira".into(),
            min_autonomy_level: 3,
            cost_per_call: 2_000_000,
            has_side_effects: true,
            rate_limit: 30,
        });

        registry.register(ExternalTool {
            id: "webhook".into(),
            name: "Webhook".into(),
            description: "Send HTTP requests to webhook endpoints".into(),
            category: ToolCategory::Webhook,
            parameters: vec![
                p("url", "Webhook URL", ParamType::Url, true),
                ToolParameter {
                    name: "method".into(),
                    description: "HTTP method".into(),
                    param_type: ParamType::String,
                    required: false,
                    default: Some(serde_json::json!("POST")),
                },
                p("headers", "HTTP headers", ParamType::Json, false),
                p("body", "Request body", ParamType::Json, false),
            ],
            return_type: "HTTP response".into(),
            requires_auth: false,
            auth_env_var: None,
            available: true,
            required_capability: "external_tool.webhook".into(),
            min_autonomy_level: 4,
            cost_per_call: 3_000_000,
            has_side_effects: true,
            rate_limit: 20,
        });

        registry.register(ExternalTool {
            id: "rest_api".into(),
            name: "REST API".into(),
            description: "Generic REST API caller".into(),
            category: ToolCategory::RestApi,
            parameters: vec![
                p("url", "API endpoint URL", ParamType::Url, true),
                p("method", "HTTP method", ParamType::String, true),
                p("headers", "HTTP headers", ParamType::Json, false),
                p("body", "Request body", ParamType::Json, false),
            ],
            return_type: "HTTP response".into(),
            requires_auth: false,
            auth_env_var: None,
            available: true,
            required_capability: "external_tool.rest_api".into(),
            min_autonomy_level: 4,
            cost_per_call: 2_000_000,
            has_side_effects: true,
            rate_limit: 30,
        });

        registry.register(ExternalTool {
            id: "file_storage".into(),
            name: "File Storage".into(),
            description: "S3-compatible file storage — upload, download, list".into(),
            category: ToolCategory::Storage,
            parameters: vec![
                p(
                    "action",
                    "Storage action (upload, download, list, delete)",
                    ParamType::String,
                    true,
                ),
                p("bucket", "Bucket name", ParamType::String, true),
                p("key", "Object key/path", ParamType::String, false),
                p("file_path", "Local file path", ParamType::FilePath, false),
            ],
            return_type: "JSON".into(),
            requires_auth: true,
            auth_env_var: Some("AWS_ACCESS_KEY_ID".into()),
            available: false,
            required_capability: "external_tool.storage".into(),
            min_autonomy_level: 4,
            cost_per_call: 1_000_000,
            has_side_effects: true,
            rate_limit: 60,
        });

        registry.refresh_availability();
        registry
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_registry_has_9_tools() {
        let reg = ToolRegistry::default_registry();
        assert_eq!(reg.all_tools().len(), 9);
    }

    #[test]
    fn test_web_search_always_available() {
        let reg = ToolRegistry::default_registry();
        let ws = reg.get("web_search").unwrap();
        assert!(ws.available);
        assert!(!ws.requires_auth);
    }

    #[test]
    fn test_tool_availability_without_env() {
        let reg = ToolRegistry::default_registry();
        let gh = reg.get("github").unwrap();
        // GITHUB_TOKEN unlikely to be set in test env
        if std::env::var("GITHUB_TOKEN").is_err() {
            assert!(!gh.available);
        }
    }

    #[test]
    fn test_tool_categories() {
        let reg = ToolRegistry::default_registry();
        assert_eq!(
            reg.get("github").unwrap().category,
            ToolCategory::CodeRepository
        );
        assert_eq!(
            reg.get("slack").unwrap().category,
            ToolCategory::Communication
        );
        assert_eq!(
            reg.get("web_search").unwrap().category,
            ToolCategory::Search
        );
        assert_eq!(
            reg.get("database").unwrap().category,
            ToolCategory::Database
        );
        assert_eq!(
            reg.get("jira").unwrap().category,
            ToolCategory::ProjectManagement
        );
        assert_eq!(reg.get("webhook").unwrap().category, ToolCategory::Webhook);
        assert_eq!(reg.get("rest_api").unwrap().category, ToolCategory::RestApi);
        assert_eq!(
            reg.get("file_storage").unwrap().category,
            ToolCategory::Storage
        );
    }
}
