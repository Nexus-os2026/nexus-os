//! Tests for the agent module: tool protocol, agent config, agent events,
//! system prompt builder, and consent handler logic.

use nexus_code::agent::tool_protocol::*;
use nexus_code::agent::{AgentConfig, AgentEvent};
use nexus_code::tools::ToolRegistry;
use serde_json::json;

// ═══════════════════════════════════════════════════════
// Tool Protocol Tests (12)
// ═══════════════════════════════════════════════════════

#[test]
fn test_tool_definition_from_nxtool() {
    let tool = nexus_code::tools::file_read::FileReadTool;
    let def = ToolDefinition::from_tool(&tool);

    assert_eq!(def.name, "file_read");
    assert!(!def.description.is_empty());
    assert!(def.input_schema.is_object());
    assert!(def.input_schema.get("properties").is_some());
}

#[test]
fn test_format_tools_anthropic() {
    let defs = vec![ToolDefinition {
        name: "file_read".to_string(),
        description: "Read a file".to_string(),
        input_schema: json!({"type": "object", "properties": {"path": {"type": "string"}}}),
    }];

    let formatted = format_tools_anthropic(&defs);
    let arr = formatted.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["name"], "file_read");
    assert_eq!(arr[0]["description"], "Read a file");
    assert!(arr[0]["input_schema"].is_object());
}

#[test]
fn test_format_tools_openai() {
    let defs = vec![ToolDefinition {
        name: "bash".to_string(),
        description: "Run a command".to_string(),
        input_schema: json!({"type": "object", "properties": {"command": {"type": "string"}}}),
    }];

    let formatted = format_tools_openai(&defs);
    let arr = formatted.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["type"], "function");
    assert_eq!(arr[0]["function"]["name"], "bash");
    assert_eq!(arr[0]["function"]["description"], "Run a command");
    assert!(arr[0]["function"]["parameters"].is_object());
}

#[test]
fn test_format_tools_google() {
    let defs = vec![ToolDefinition {
        name: "search".to_string(),
        description: "Search files".to_string(),
        input_schema: json!({"type": "object", "properties": {"pattern": {"type": "string"}}}),
    }];

    let formatted = format_tools_google(&defs);
    let arr = formatted.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert!(arr[0]["functionDeclarations"].is_array());
    let decls = arr[0]["functionDeclarations"].as_array().unwrap();
    assert_eq!(decls[0]["name"], "search");
}

#[test]
fn test_parse_tool_calls_anthropic() {
    let content = vec![
        json!({"type": "text", "text": "Let me read that file."}),
        json!({
            "type": "tool_use",
            "id": "toolu_abc123",
            "name": "file_read",
            "input": {"path": "src/main.rs"}
        }),
    ];

    let calls = parse_tool_calls_anthropic(&content);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].id, "toolu_abc123");
    assert_eq!(calls[0].name, "file_read");
    assert_eq!(calls[0].input["path"], "src/main.rs");
}

#[test]
fn test_parse_tool_calls_openai() {
    let tool_calls = vec![json!({
        "id": "call_xyz789",
        "type": "function",
        "function": {
            "name": "bash",
            "arguments": "{\"command\": \"ls -la\"}"
        }
    })];

    let calls = parse_tool_calls_openai(&tool_calls);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].id, "call_xyz789");
    assert_eq!(calls[0].name, "bash");
    assert_eq!(calls[0].input["command"], "ls -la");
}

#[test]
fn test_parse_tool_calls_google() {
    let parts = vec![
        json!({"text": "I'll search for that."}),
        json!({
            "functionCall": {
                "name": "search",
                "args": {"pattern": "TODO"}
            }
        }),
    ];

    let calls = parse_tool_calls_google(&parts);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].name, "search");
    assert_eq!(calls[0].input["pattern"], "TODO");
    // Google generates UUIDs for IDs
    assert!(!calls[0].id.is_empty());
}

#[test]
fn test_format_tool_results_anthropic() {
    let results = vec![ToolResultMessage {
        tool_call_id: "toolu_abc123".to_string(),
        tool_name: "file_read".to_string(),
        content: "file content here".to_string(),
        is_error: false,
    }];

    let formatted = format_tool_results_anthropic(&results);
    assert_eq!(formatted["role"], "user");
    let content = formatted["content"].as_array().unwrap();
    assert_eq!(content.len(), 1);
    assert_eq!(content[0]["type"], "tool_result");
    assert_eq!(content[0]["tool_use_id"], "toolu_abc123");
    assert_eq!(content[0]["content"], "file content here");
    assert_eq!(content[0]["is_error"], false);
}

#[test]
fn test_format_tool_results_openai() {
    let results = vec![ToolResultMessage {
        tool_call_id: "call_xyz789".to_string(),
        tool_name: "bash".to_string(),
        content: "command output".to_string(),
        is_error: false,
    }];

    let formatted = format_tool_results_openai(&results);
    assert_eq!(formatted.len(), 1);
    assert_eq!(formatted[0]["role"], "tool");
    assert_eq!(formatted[0]["tool_call_id"], "call_xyz789");
    assert_eq!(formatted[0]["content"], "command output");
}

#[test]
fn test_format_tool_results_google() {
    let results = vec![ToolResultMessage {
        tool_call_id: "uuid-123".to_string(),
        tool_name: "search".to_string(),
        content: "search results".to_string(),
        is_error: false,
    }];

    let formatted = format_tool_results_google(&results);
    assert_eq!(formatted["role"], "user");
    let parts = formatted["parts"].as_array().unwrap();
    assert_eq!(parts.len(), 1);
    assert!(parts[0]["functionResponse"].is_object());
    assert_eq!(parts[0]["functionResponse"]["name"], "search");
    assert_eq!(
        parts[0]["functionResponse"]["response"]["content"],
        "search results"
    );
}

#[test]
fn test_protocol_for_provider() {
    assert_eq!(
        ToolProtocol::for_provider("anthropic"),
        ToolProtocol::Anthropic
    );
    assert_eq!(ToolProtocol::for_provider("google"), ToolProtocol::Google);
    assert_eq!(ToolProtocol::for_provider("openai"), ToolProtocol::OpenAi);
    assert_eq!(ToolProtocol::for_provider("groq"), ToolProtocol::OpenAi);
    assert_eq!(ToolProtocol::for_provider("ollama"), ToolProtocol::OpenAi);
    assert_eq!(
        ToolProtocol::for_provider("openrouter"),
        ToolProtocol::OpenAi
    );
    assert_eq!(ToolProtocol::for_provider("deepseek"), ToolProtocol::OpenAi);
}

#[test]
fn test_parse_empty_tool_calls() {
    // Anthropic: empty content array
    let calls = parse_tool_calls_anthropic(&[]);
    assert!(calls.is_empty());

    // Anthropic: only text blocks
    let calls = parse_tool_calls_anthropic(&[json!({"type": "text", "text": "just text"})]);
    assert!(calls.is_empty());

    // OpenAI: empty tool_calls array
    let calls = parse_tool_calls_openai(&[]);
    assert!(calls.is_empty());

    // Google: empty parts
    let calls = parse_tool_calls_google(&[]);
    assert!(calls.is_empty());

    // Google: only text parts
    let calls = parse_tool_calls_google(&[json!({"text": "just text"})]);
    assert!(calls.is_empty());
}

// ═══════════════════════════════════════════════════════
// Agent Config Tests (3)
// ═══════════════════════════════════════════════════════

#[test]
fn test_agent_config_defaults() {
    let config = AgentConfig::default();
    assert_eq!(config.max_turns, 10);
    assert!(!config.auto_approve_tier2);
    assert!(!config.auto_approve_tier3);
    assert!(config.system_prompt.is_empty());
}

#[test]
fn test_consent_handler_headless_deny_all() {
    // Simulate headless mode with both flags false
    let approve_t2 = false;
    let approve_t3 = false;
    let handler = move |request: &nexus_code::governance::ConsentRequest| -> bool {
        match request.tier {
            nexus_code::governance::ConsentTier::Tier1 => true,
            nexus_code::governance::ConsentTier::Tier2 => approve_t2,
            nexus_code::governance::ConsentTier::Tier3 => approve_t3,
        }
    };

    // Create Tier2 request
    let tier2_req = nexus_code::governance::ConsentRequest {
        id: "test-1".to_string(),
        action: "file_write".to_string(),
        tier: nexus_code::governance::ConsentTier::Tier2,
        details: "write test".to_string(),
        timestamp: chrono::Utc::now(),
    };
    assert!(!handler(&tier2_req));

    // Create Tier3 request
    let tier3_req = nexus_code::governance::ConsentRequest {
        id: "test-2".to_string(),
        action: "bash".to_string(),
        tier: nexus_code::governance::ConsentTier::Tier3,
        details: "exec test".to_string(),
        timestamp: chrono::Utc::now(),
    };
    assert!(!handler(&tier3_req));

    // Tier1 always approved
    let tier1_req = nexus_code::governance::ConsentRequest {
        id: "test-3".to_string(),
        action: "file_read".to_string(),
        tier: nexus_code::governance::ConsentTier::Tier1,
        details: "read test".to_string(),
        timestamp: chrono::Utc::now(),
    };
    assert!(handler(&tier1_req));
}

#[test]
fn test_consent_handler_headless_approve_tier2() {
    let approve_t2 = true;
    let approve_t3 = false;
    let handler = move |request: &nexus_code::governance::ConsentRequest| -> bool {
        match request.tier {
            nexus_code::governance::ConsentTier::Tier1 => true,
            nexus_code::governance::ConsentTier::Tier2 => approve_t2,
            nexus_code::governance::ConsentTier::Tier3 => approve_t3,
        }
    };

    let tier2_req = nexus_code::governance::ConsentRequest {
        id: "test-1".to_string(),
        action: "file_write".to_string(),
        tier: nexus_code::governance::ConsentTier::Tier2,
        details: "write test".to_string(),
        timestamp: chrono::Utc::now(),
    };
    assert!(handler(&tier2_req)); // Tier2 approved

    let tier3_req = nexus_code::governance::ConsentRequest {
        id: "test-2".to_string(),
        action: "bash".to_string(),
        tier: nexus_code::governance::ConsentTier::Tier3,
        details: "exec test".to_string(),
        timestamp: chrono::Utc::now(),
    };
    assert!(!handler(&tier3_req)); // Tier3 still denied
}

// ═══════════════════════════════════════════════════════
// Agent Event Tests (3)
// ═══════════════════════════════════════════════════════

#[test]
fn test_agent_event_text_delta() {
    let event = AgentEvent::TextDelta("hello world".to_string());
    match event {
        AgentEvent::TextDelta(text) => assert_eq!(text, "hello world"),
        _ => panic!("Expected TextDelta"),
    }
}

#[test]
fn test_agent_event_tool_complete() {
    let event = AgentEvent::ToolCallComplete {
        name: "file_read".to_string(),
        success: true,
        duration_ms: 42,
        summary: "OK: read 100 lines".to_string(),
    };
    match event {
        AgentEvent::ToolCallComplete {
            name,
            success,
            duration_ms,
            summary,
        } => {
            assert_eq!(name, "file_read");
            assert!(success);
            assert_eq!(duration_ms, 42);
            assert!(summary.contains("100 lines"));
        }
        _ => panic!("Expected ToolCallComplete"),
    }
}

#[test]
fn test_agent_event_done() {
    let event = AgentEvent::Done {
        reason: "end_turn".to_string(),
        total_turns: 3,
    };
    match event {
        AgentEvent::Done {
            reason,
            total_turns,
        } => {
            assert_eq!(reason, "end_turn");
            assert_eq!(total_turns, 3);
        }
        _ => panic!("Expected Done"),
    }
}

// ═══════════════════════════════════════════════════════
// System Prompt Tests (2)
// ═══════════════════════════════════════════════════════

#[test]
fn test_build_system_prompt_includes_tools() {
    let registry = ToolRegistry::with_defaults();
    let prompt = nexus_code::agent::build_system_prompt("You are a coding agent.", &registry);

    // Should contain all tool names
    assert!(prompt.contains("file_read"));
    assert!(prompt.contains("file_write"));
    assert!(prompt.contains("file_edit"));
    assert!(prompt.contains("bash"));
    assert!(prompt.contains("search"));
    assert!(prompt.contains("glob"));

    // Should contain the base prompt
    assert!(prompt.contains("You are a coding agent."));

    // Should contain JSON schemas
    assert!(prompt.contains("\"type\": \"object\""));
}

#[test]
fn test_build_system_prompt_includes_usage_rules() {
    let registry = ToolRegistry::with_defaults();
    let prompt = nexus_code::agent::build_system_prompt("Base prompt.", &registry);

    assert!(prompt.contains("Tool Usage Rules"));
    assert!(prompt.contains("file_read before file_edit"));
    assert!(prompt.contains("search and glob"));
}

// ═══════════════════════════════════════════════════════
// Integration Tests (2)
// ═══════════════════════════════════════════════════════

#[test]
fn test_tool_registry_prompt_includes_all_tools() {
    let registry = ToolRegistry::with_defaults();
    let prompt = registry.build_tool_prompt();

    assert!(prompt.contains("file_read"));
    assert!(prompt.contains("file_write"));
    assert!(prompt.contains("file_edit"));
    assert!(prompt.contains("bash"));
    assert!(prompt.contains("search"));
    assert!(prompt.contains("glob"));
    assert!(prompt.contains("Parameters:"));
}

#[test]
fn test_headless_consent_handler_tier_logic() {
    // Test the exact logic used in main.rs for headless mode
    use nexus_code::governance::{ConsentRequest, ConsentTier};

    let make_request = |tier: ConsentTier, action: &str| ConsentRequest {
        id: uuid::Uuid::new_v4().to_string(),
        action: action.to_string(),
        tier,
        details: "test".to_string(),
        timestamp: chrono::Utc::now(),
    };

    // Default headless: deny Tier2 and Tier3
    let handler = |approve_t2: bool, approve_t3: bool| {
        move |request: &ConsentRequest| -> bool {
            match request.tier {
                ConsentTier::Tier1 => true,
                ConsentTier::Tier2 => approve_t2,
                ConsentTier::Tier3 => approve_t3,
            }
        }
    };

    // Default (no flags): deny both
    let h = handler(false, false);
    assert!(h(&make_request(ConsentTier::Tier1, "file_read")));
    assert!(!h(&make_request(ConsentTier::Tier2, "file_write")));
    assert!(!h(&make_request(ConsentTier::Tier3, "bash")));

    // --auto-approve: approve Tier2, deny Tier3
    let h = handler(true, false);
    assert!(h(&make_request(ConsentTier::Tier1, "file_read")));
    assert!(h(&make_request(ConsentTier::Tier2, "file_write")));
    assert!(!h(&make_request(ConsentTier::Tier3, "bash")));

    // --dangerously-approve-all: approve everything
    let h = handler(true, true);
    assert!(h(&make_request(ConsentTier::Tier1, "file_read")));
    assert!(h(&make_request(ConsentTier::Tier2, "file_write")));
    assert!(h(&make_request(ConsentTier::Tier3, "bash")));
}

// ═══════════════════════════════════════════════════════
// OpenAI Streaming Tool Collection Tests (5)
// ═══════════════════════════════════════════════════════

/// Helper: simulate parsing OpenAI streaming tool call deltas by
/// replaying the accumulation logic used in collect_openai_stream().
fn accumulate_openai_tool_deltas(
    chunks: &[serde_json::Value],
) -> (String, Vec<serde_json::Value>, Option<String>) {
    let mut text = String::new();
    let mut tool_calls_in_progress: std::collections::HashMap<u64, (String, String, String)> =
        std::collections::HashMap::new();
    let mut stop_reason: Option<String> = None;

    for json in chunks {
        if let Some(choice) = json
            .get("choices")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
        {
            let delta = choice.get("delta").unwrap_or(&serde_json::Value::Null);

            if let Some(content) = delta.get("content").and_then(|c| c.as_str()) {
                text.push_str(content);
            }

            if let Some(tool_calls) = delta.get("tool_calls").and_then(|tc| tc.as_array()) {
                for tc in tool_calls {
                    let index = tc.get("index").and_then(|i| i.as_u64()).unwrap_or(0);
                    if let Some(id) = tc.get("id").and_then(|i| i.as_str()) {
                        let name = tc
                            .get("function")
                            .and_then(|f| f.get("name"))
                            .and_then(|n| n.as_str())
                            .unwrap_or("")
                            .to_string();
                        let initial_args = tc
                            .get("function")
                            .and_then(|f| f.get("arguments"))
                            .and_then(|a| a.as_str())
                            .unwrap_or("")
                            .to_string();
                        tool_calls_in_progress.insert(index, (id.to_string(), name, initial_args));
                    } else if let Some(args_delta) = tc
                        .get("function")
                        .and_then(|f| f.get("arguments"))
                        .and_then(|a| a.as_str())
                    {
                        if let Some(entry) = tool_calls_in_progress.get_mut(&index) {
                            entry.2.push_str(args_delta);
                        }
                    }
                }
            }

            if let Some(reason) = choice.get("finish_reason").and_then(|r| r.as_str()) {
                stop_reason = Some(reason.to_string());
            }
        }
    }

    let mut sorted: Vec<(u64, (String, String, String))> =
        tool_calls_in_progress.into_iter().collect();
    sorted.sort_by_key(|(idx, _)| *idx);

    let tool_blocks: Vec<serde_json::Value> = sorted
        .into_iter()
        .map(|(_, (id, name, args))| {
            json!({
                "id": id,
                "type": "function",
                "function": {
                    "name": name,
                    "arguments": args
                }
            })
        })
        .collect();

    (text, tool_blocks, stop_reason)
}

#[test]
fn test_collect_openai_stream_text_only() {
    let chunks = vec![
        json!({"choices":[{"index":0,"delta":{"role":"assistant","content":""},"finish_reason":null}]}),
        json!({"choices":[{"index":0,"delta":{"content":"Hello"},"finish_reason":null}]}),
        json!({"choices":[{"index":0,"delta":{"content":" world"},"finish_reason":null}]}),
        json!({"choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}),
    ];

    let (text, tool_blocks, stop_reason) = accumulate_openai_tool_deltas(&chunks);
    assert_eq!(text, "Hello world");
    assert!(tool_blocks.is_empty());
    assert_eq!(stop_reason.as_deref(), Some("stop"));
}

#[test]
fn test_collect_openai_stream_single_tool_call() {
    let chunks = vec![
        json!({"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"call_abc123","type":"function","function":{"name":"file_read","arguments":""}}]},"finish_reason":null}]}),
        json!({"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"path\":"}}]},"finish_reason":null}]}),
        json!({"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"\"src/main.rs\"}"}}]},"finish_reason":null}]}),
        json!({"choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}),
    ];

    let (text, tool_blocks, stop_reason) = accumulate_openai_tool_deltas(&chunks);
    assert!(text.is_empty());
    assert_eq!(tool_blocks.len(), 1);
    assert_eq!(tool_blocks[0]["id"], "call_abc123");
    assert_eq!(tool_blocks[0]["function"]["name"], "file_read");
    assert_eq!(
        tool_blocks[0]["function"]["arguments"],
        "{\"path\":\"src/main.rs\"}"
    );
    assert_eq!(stop_reason.as_deref(), Some("tool_calls"));

    // Verify parse_tool_calls_openai can read the assembled blocks
    let calls = parse_tool_calls_openai(&tool_blocks);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].name, "file_read");
    assert_eq!(calls[0].input["path"], "src/main.rs");
}

#[test]
fn test_collect_openai_stream_multiple_tool_calls() {
    let chunks = vec![
        json!({"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"call_1","type":"function","function":{"name":"file_read","arguments":""}}]},"finish_reason":null}]}),
        json!({"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"path\":\"a.rs\"}"}}]},"finish_reason":null}]}),
        json!({"choices":[{"index":0,"delta":{"tool_calls":[{"index":1,"id":"call_2","type":"function","function":{"name":"file_read","arguments":""}}]},"finish_reason":null}]}),
        json!({"choices":[{"index":0,"delta":{"tool_calls":[{"index":1,"function":{"arguments":"{\"path\":\"b.rs\"}"}}]},"finish_reason":null}]}),
        json!({"choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}),
    ];

    let (_text, tool_blocks, _stop_reason) = accumulate_openai_tool_deltas(&chunks);
    assert_eq!(tool_blocks.len(), 2);

    // Verify sorted by index
    assert_eq!(tool_blocks[0]["id"], "call_1");
    assert_eq!(tool_blocks[1]["id"], "call_2");

    let calls = parse_tool_calls_openai(&tool_blocks);
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].input["path"], "a.rs");
    assert_eq!(calls[1].input["path"], "b.rs");
}

#[test]
fn test_collect_openai_stream_text_then_tool() {
    let chunks = vec![
        json!({"choices":[{"index":0,"delta":{"content":"I'll read that file."},"finish_reason":null}]}),
        json!({"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"call_xyz","type":"function","function":{"name":"file_read","arguments":"{\"path\":\"main.rs\"}"}}]},"finish_reason":null}]}),
        json!({"choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}),
    ];

    let (text, tool_blocks, stop_reason) = accumulate_openai_tool_deltas(&chunks);
    assert_eq!(text, "I'll read that file.");
    assert_eq!(tool_blocks.len(), 1);
    assert_eq!(tool_blocks[0]["function"]["name"], "file_read");
    assert_eq!(stop_reason.as_deref(), Some("tool_calls"));
}

#[test]
fn test_collect_openai_stream_partial_arguments() {
    // Arguments streamed across 4 chunks
    let chunks = vec![
        json!({"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"call_p","type":"function","function":{"name":"bash","arguments":""}}]},"finish_reason":null}]}),
        json!({"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"co"}}]},"finish_reason":null}]}),
        json!({"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"mman"}}]},"finish_reason":null}]}),
        json!({"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"d\": \"ls -la\"}"}}]},"finish_reason":null}]}),
        json!({"choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}),
    ];

    let (_text, tool_blocks, _stop_reason) = accumulate_openai_tool_deltas(&chunks);
    assert_eq!(tool_blocks.len(), 1);

    let calls = parse_tool_calls_openai(&tool_blocks);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].name, "bash");
    assert_eq!(calls[0].input["command"], "ls -la");
}
