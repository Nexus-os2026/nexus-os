use serde_json::json;

use crate::tools::ToolRegistry;
use crate::types::*;

/// MCP Server — handles JSON-RPC 2.0 requests from external clients.
pub struct McpServer {
    tools: ToolRegistry,
    server_info: ServerInfo,
    resources: Vec<McpResource>,
    prompts: Vec<McpPrompt>,
}

impl McpServer {
    pub fn new() -> Self {
        Self {
            tools: ToolRegistry::default(),
            server_info: ServerInfo {
                name: "nexus-os".into(),
                version: "9.6.0".into(),
            },
            resources: vec![
                McpResource {
                    uri: "nexus://agents/status".into(),
                    name: "Agent Status".into(),
                    description: Some("Current status of all registered agents".into()),
                    mime_type: "application/json".into(),
                },
                McpResource {
                    uri: "nexus://governance/policy".into(),
                    name: "Governance Policy".into(),
                    description: Some("Current governance policy configuration".into()),
                    mime_type: "application/json".into(),
                },
                McpResource {
                    uri: "nexus://audit/recent".into(),
                    name: "Recent Audit Events".into(),
                    description: Some("Last 100 audit trail entries".into()),
                    mime_type: "application/json".into(),
                },
            ],
            prompts: vec![
                McpPrompt {
                    name: "nexus_task".into(),
                    description: Some("Submit a task to a Nexus OS agent".into()),
                    arguments: vec![
                        McpPromptArgument {
                            name: "task".into(),
                            description: Some("Task description".into()),
                            required: true,
                        },
                        McpPromptArgument {
                            name: "agent".into(),
                            description: Some("Target agent name".into()),
                            required: false,
                        },
                    ],
                },
                McpPrompt {
                    name: "nexus_review".into(),
                    description: Some("Start a governed code review collaboration".into()),
                    arguments: vec![McpPromptArgument {
                        name: "code".into(),
                        description: Some("Code to review".into()),
                        required: true,
                    }],
                },
            ],
        }
    }

    /// Handle a JSON-RPC request and return a response.
    pub fn handle_request(&self, request: &JsonRpcRequest) -> JsonRpcResponse {
        match request.method.as_str() {
            "initialize" => self.handle_initialize(&request.id),
            "tools/list" => self.handle_tools_list(&request.id),
            "tools/call" => self.handle_tools_call(&request.id, &request.params),
            "resources/list" => self.handle_resources_list(&request.id),
            "resources/read" => self.handle_resources_read(&request.id, &request.params),
            "prompts/list" => self.handle_prompts_list(&request.id),
            "prompts/get" => self.handle_prompts_get(&request.id, &request.params),
            "ping" => JsonRpcResponse::success(request.id.clone(), json!({})),
            _ => JsonRpcResponse::error(
                request.id.clone(),
                METHOD_NOT_FOUND,
                format!("Method not found: {}", request.method),
            ),
        }
    }

    /// Parse a raw JSON string into a request and handle it.
    pub fn handle_raw(&self, raw: &str) -> String {
        let request: Result<JsonRpcRequest, _> = serde_json::from_str(raw);
        let response = match request {
            Ok(req) => self.handle_request(&req),
            Err(e) => JsonRpcResponse::error(
                serde_json::Value::Null,
                PARSE_ERROR,
                format!("Parse error: {e}"),
            ),
        };
        serde_json::to_string(&response).unwrap_or_else(|_| {
            r#"{"jsonrpc":"2.0","id":null,"error":{"code":-32603,"message":"Internal error"}}"#
                .into()
        })
    }

    fn handle_initialize(&self, id: &serde_json::Value) -> JsonRpcResponse {
        JsonRpcResponse::success(
            id.clone(),
            json!({
                "protocolVersion": "2024-11-05",
                "serverInfo": self.server_info,
                "capabilities": ServerCapabilities {
                    tools: Some(json!({"listChanged": true})),
                    resources: Some(json!({"subscribe": false, "listChanged": true})),
                    prompts: Some(json!({"listChanged": true})),
                },
            }),
        )
    }

    fn handle_tools_list(&self, id: &serde_json::Value) -> JsonRpcResponse {
        let tools: Vec<serde_json::Value> = self
            .tools
            .list_tools()
            .iter()
            .map(|t| serde_json::to_value(t).unwrap_or_default())
            .collect();
        JsonRpcResponse::success(id.clone(), json!({ "tools": tools }))
    }

    fn handle_tools_call(
        &self,
        id: &serde_json::Value,
        params: &serde_json::Value,
    ) -> JsonRpcResponse {
        let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let arguments = params
            .get("arguments")
            .cloned()
            .unwrap_or(serde_json::json!({}));

        match self.tools.call_tool(name, arguments) {
            Ok(result) => JsonRpcResponse::success(
                id.clone(),
                serde_json::to_value(&result).unwrap_or_default(),
            ),
            Err(e) => JsonRpcResponse::error(id.clone(), INVALID_PARAMS, e),
        }
    }

    fn handle_resources_list(&self, id: &serde_json::Value) -> JsonRpcResponse {
        let resources: Vec<serde_json::Value> = self
            .resources
            .iter()
            .map(|r| serde_json::to_value(r).unwrap_or_default())
            .collect();
        JsonRpcResponse::success(id.clone(), json!({ "resources": resources }))
    }

    fn handle_resources_read(
        &self,
        id: &serde_json::Value,
        params: &serde_json::Value,
    ) -> JsonRpcResponse {
        let uri = params.get("uri").and_then(|v| v.as_str()).unwrap_or("");
        let content = match uri {
            "nexus://agents/status" => json!({"agents": [], "count": 0}),
            "nexus://governance/policy" => {
                json!({"min_autonomy": 2, "hitl_required_above": 3, "fuel_metering": true})
            }
            "nexus://audit/recent" => json!({"events": [], "count": 0}),
            _ => {
                return JsonRpcResponse::error(
                    id.clone(),
                    INVALID_PARAMS,
                    format!("Unknown resource: {uri}"),
                )
            }
        };
        JsonRpcResponse::success(
            id.clone(),
            json!({
                "contents": [{
                    "uri": uri,
                    "mimeType": "application/json",
                    "text": serde_json::to_string(&content).unwrap_or_default()
                }]
            }),
        )
    }

    fn handle_prompts_list(&self, id: &serde_json::Value) -> JsonRpcResponse {
        let prompts: Vec<serde_json::Value> = self
            .prompts
            .iter()
            .map(|p| serde_json::to_value(p).unwrap_or_default())
            .collect();
        JsonRpcResponse::success(id.clone(), json!({ "prompts": prompts }))
    }

    fn handle_prompts_get(
        &self,
        id: &serde_json::Value,
        params: &serde_json::Value,
    ) -> JsonRpcResponse {
        let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
        match self.prompts.iter().find(|p| p.name == name) {
            Some(prompt) => {
                let text = match name {
                    "nexus_task" => "You are a Nexus OS agent. Execute the following task with full governance compliance.",
                    "nexus_review" => "You are a Nexus OS code reviewer. Analyze the following code for quality, security, and correctness.",
                    _ => "Execute the requested operation.",
                };
                JsonRpcResponse::success(
                    id.clone(),
                    json!({
                        "description": prompt.description,
                        "messages": [{
                            "role": "user",
                            "content": {"type": "text", "text": text}
                        }]
                    }),
                )
            }
            None => JsonRpcResponse::error(
                id.clone(),
                INVALID_PARAMS,
                format!("Unknown prompt: {name}"),
            ),
        }
    }
}

impl Default for McpServer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server() -> McpServer {
        McpServer::new()
    }

    fn req(method: &str, params: serde_json::Value) -> JsonRpcRequest {
        JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: json!(1),
            method: method.into(),
            params,
        }
    }

    #[test]
    fn test_initialize_returns_capabilities() {
        let s = server();
        let resp = s.handle_request(&req("initialize", json!({})));
        let result = resp.result.unwrap();
        assert_eq!(result["protocolVersion"], "2024-11-05");
        assert!(result["capabilities"]["tools"].is_object());
        assert!(result["capabilities"]["resources"].is_object());
        assert!(result["capabilities"]["prompts"].is_object());
    }

    #[test]
    fn test_tools_list_returns_tools() {
        let s = server();
        let resp = s.handle_request(&req("tools/list", json!({})));
        let tools = resp.result.unwrap()["tools"].as_array().unwrap().clone();
        assert!(tools.len() >= 7);
    }

    #[test]
    fn test_tools_call_valid() {
        let s = server();
        let resp = s.handle_request(&req(
            "tools/call",
            json!({"name": "nexus_agent_run", "arguments": {"task": "hello"}}),
        ));
        assert!(resp.error.is_none());
        let content = &resp.result.unwrap()["content"];
        assert!(content[0]["text"].as_str().unwrap().contains("hello"));
    }

    #[test]
    fn test_tools_call_unknown_tool() {
        let s = server();
        let resp = s.handle_request(&req(
            "tools/call",
            json!({"name": "nonexistent", "arguments": {}}),
        ));
        assert!(resp.error.is_some());
    }

    #[test]
    fn test_resources_list() {
        let s = server();
        let resp = s.handle_request(&req("resources/list", json!({})));
        let resources = resp.result.unwrap()["resources"]
            .as_array()
            .unwrap()
            .clone();
        assert!(resources.len() >= 3);
    }

    #[test]
    fn test_resources_read_valid() {
        let s = server();
        let resp = s.handle_request(&req(
            "resources/read",
            json!({"uri": "nexus://agents/status"}),
        ));
        assert!(resp.error.is_none());
    }

    #[test]
    fn test_resources_read_unknown() {
        let s = server();
        let resp = s.handle_request(&req("resources/read", json!({"uri": "nexus://unknown"})));
        assert!(resp.error.is_some());
    }

    #[test]
    fn test_prompts_list() {
        let s = server();
        let resp = s.handle_request(&req("prompts/list", json!({})));
        let prompts = resp.result.unwrap()["prompts"].as_array().unwrap().clone();
        assert!(prompts.len() >= 2);
    }

    #[test]
    fn test_unknown_method() {
        let s = server();
        let resp = s.handle_request(&req("unknown/method", json!({})));
        assert_eq!(resp.error.unwrap().code, METHOD_NOT_FOUND);
    }

    #[test]
    fn test_malformed_json_handled() {
        let s = server();
        let resp = s.handle_raw("not valid json");
        let parsed: JsonRpcResponse = serde_json::from_str(&resp).unwrap();
        assert_eq!(parsed.error.unwrap().code, PARSE_ERROR);
    }

    #[test]
    fn test_ping() {
        let s = server();
        let resp = s.handle_request(&req("ping", json!({})));
        assert!(resp.error.is_none());
    }
}
