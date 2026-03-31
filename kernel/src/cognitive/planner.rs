//! Cognitive planner — translates goals into executable step sequences via LLM.

use super::types::{AgentGoal, AgentStep, PlannedAction, PlanningContext};
use crate::capabilities::has_capability;
use crate::errors::AgentError;
use regex::Regex;
use serde::Deserialize;
use serde_json::Value;

/// Trait abstraction for the LLM call so the planner is testable with mocks.
pub trait PlannerLlm: Send + Sync {
    /// Send a prompt and return the LLM response text.
    fn plan_query(&self, prompt: &str) -> Result<String, AgentError>;
}

/// Generates executable step plans from high-level goals.
pub struct CognitivePlanner {
    llm: Box<dyn PlannerLlm>,
}

impl CognitivePlanner {
    pub fn new(llm: Box<dyn PlannerLlm>) -> Self {
        Self { llm }
    }

    /// Plan steps for a goal given the agent's context.
    pub fn plan_goal(
        &self,
        goal: &AgentGoal,
        context: &PlanningContext,
    ) -> Result<Vec<AgentStep>, AgentError> {
        let prompt = self.build_planning_prompt(goal, context);
        self.query_plan_with_retry(
            &prompt,
            &self.build_invalid_json_retry_prompt(goal, context),
            &goal.description,
            &goal.id,
            &context.agent_capabilities,
        )
    }

    /// Re-plan after a step failure, incorporating the error context.
    pub fn replan_after_failure(
        &self,
        goal: &AgentGoal,
        failed_step: &AgentStep,
        error: &str,
        remaining_steps: &[AgentStep],
        context: &PlanningContext,
    ) -> Result<Vec<AgentStep>, AgentError> {
        let prompt = self.build_replan_prompt(goal, failed_step, error, remaining_steps, context);
        self.query_plan_with_retry(
            &prompt,
            &self.build_invalid_json_retry_prompt(goal, context),
            &goal.description,
            &goal.id,
            &context.agent_capabilities,
        )
    }

    fn build_planning_prompt(&self, goal: &AgentGoal, context: &PlanningContext) -> String {
        let agent_name = context.agent_name.as_deref().unwrap_or("unknown-agent");
        let agent_description = context.agent_description.as_deref().unwrap_or(
            "No additional role description was provided. Infer the role from the goal and capabilities.",
        );
        let capabilities_str = context.agent_capabilities.join(", ");
        let memories_str = if context.relevant_memories.is_empty() {
            "None".to_string()
        } else {
            context.relevant_memories.join("\n- ")
        };
        let outcomes_str = if context.previous_outcomes.is_empty() {
            "None".to_string()
        } else {
            context.previous_outcomes.join("\n- ")
        };

        let allowed_actions = self.allowed_actions_description(&context.agent_capabilities);
        let workspace = context.working_directory.as_deref().unwrap_or("/home/user");

        format!(
            r#"You are the planning subsystem for Nexus OS. Create a step-by-step plan to achieve the goal below.

AGENT NAME: {agent_name}
AGENT DESCRIPTION:
{agent_description}

GOAL: {goal_desc}
PRIORITY: {priority}

WORKSPACE DIRECTORY: {workspace}
(Use ABSOLUTE paths starting from this directory when reading/writing files. Example: "{workspace}/README.md")

AGENT CAPABILITIES: [{capabilities}]
AVAILABLE FUEL: {fuel}
AUTONOMY LEVEL: L{autonomy}

RELEVANT MEMORIES:
- {memories}

PREVIOUS OUTCOMES:
- {outcomes}

ALLOWED ACTIONS (you MUST only use these):
{allowed_actions}

RULES:
1. Respond with ONLY a JSON array. No markdown, no prose, no explanations.
2. Use ABSOLUTE file paths based on the workspace directory above.
3. Keep the plan minimal — use the fewest steps possible.
4. Always end with an LlmQuery step that synthesizes the final answer from gathered data.
5. Do NOT repeat steps. Do NOT add unnecessary file reads.
6. NEVER hallucinate or fabricate data. Use tools to get REAL data. If you need system info, use ShellCommand. If you need web data, use WebSearch/WebFetch. NEVER make up numbers or pretend you executed something.
7. Do NOT include <think> tags or reasoning blocks. Output ONLY the JSON array.

Each step object MUST have exactly these top-level fields:
- "action": an object with a required "type" field plus action-specific fields
- "description": a short human-readable description of what the step does

EXACT JSON SCHEMA EXAMPLE:
[
  {{"action": {{"type": "ShellCommand", "command": "free", "args": ["-m"]}}, "description": "Check memory usage"}},
  {{"action": {{"type": "LlmQuery", "prompt": "Summarize the data above", "context": ["previous step output"]}}, "description": "Summarize the content"}}
]

IMPORTANT:
- "context" in LlmQuery MUST be an array of strings, e.g. ["some text"]. Never a bare string.
- "args" in ShellCommand MUST be an array of strings, e.g. ["-m", "-h"]. Never a bare string.
- "options" in HitlRequest MUST be an array, e.g. ["yes", "no"].

Valid action types for this agent: {action_types}
For each action type, use ONLY these fields:
- LlmQuery: type, prompt, context (array of strings)
- FileRead: type, path
- FileWrite: type, path, content
- ShellCommand: type, command, args (array of strings)
- CodeExecute: type, language, code, timeout_secs (optional)
- WebSearch: type, query
- WebFetch: type, url
- ApiCall: type, method, url, body, headers (optional object e.g. {{"Authorization": "Bearer xxx"}})
- MemoryStore: type, key, value, memory_type
- MemoryRecall: type, query, memory_type
- SendNotification: type, title, body, level
- HitlRequest: type, question, options
- Noop: type
Do NOT include duplicate fields.
Do NOT include any text outside the JSON array."#,
            agent_name = agent_name,
            agent_description = agent_description,
            goal_desc = goal.description,
            priority = goal.priority,
            workspace = workspace,
            capabilities = capabilities_str,
            fuel = context.available_fuel,
            autonomy = context.autonomy_level,
            memories = memories_str,
            outcomes = outcomes_str,
            allowed_actions = allowed_actions,
            action_types = self.allowed_action_types(&context.agent_capabilities),
        )
    }

    fn build_replan_prompt(
        &self,
        goal: &AgentGoal,
        failed_step: &AgentStep,
        error: &str,
        remaining_steps: &[AgentStep],
        context: &PlanningContext,
    ) -> String {
        let agent_name = context.agent_name.as_deref().unwrap_or("unknown-agent");
        let agent_description = context.agent_description.as_deref().unwrap_or(
            "No additional role description was provided. Infer the role from the goal and capabilities.",
        );
        let remaining_desc: Vec<String> = remaining_steps
            .iter()
            .map(|s| format!("  - {}", s.action.action_type()))
            .collect();
        let remaining_str = if remaining_desc.is_empty() {
            "None".to_string()
        } else {
            remaining_desc.join("\n")
        };

        let allowed_actions = self.allowed_actions_description(&context.agent_capabilities);

        format!(
            r#"You are the planning subsystem for Nexus OS. A step in your plan failed. Create an adapted plan.

AGENT NAME: {agent_name}
AGENT DESCRIPTION:
{agent_description}

GOAL: {goal_desc}
FAILED STEP: {failed_action} (attempt {attempt}/{max})
ERROR: {error}

REMAINING ORIGINAL STEPS:
{remaining}

AGENT CAPABILITIES: [{capabilities}]
AVAILABLE FUEL: {fuel}

ALLOWED ACTIONS (you MUST only use these):
{allowed_actions}

Create an adapted JSON plan that works around the failure.
Respond with ONLY a valid JSON array of step objects.
Do NOT include duplicate fields.
Do NOT include any text outside the JSON array."#,
            agent_name = agent_name,
            agent_description = agent_description,
            goal_desc = goal.description,
            failed_action = failed_step.action.action_type(),
            attempt = failed_step.attempts,
            max = failed_step.max_retries,
            error = error,
            remaining = remaining_str,
            capabilities = context.agent_capabilities.join(", "),
            fuel = context.available_fuel,
            allowed_actions = allowed_actions,
        )
    }

    fn build_invalid_json_retry_prompt(
        &self,
        goal: &AgentGoal,
        context: &PlanningContext,
    ) -> String {
        format!(
            r#"Your previous response had invalid JSON. Retry with ONLY a valid JSON array.
Do not include markdown fences.
Do not include commentary before or after the array.
Your response must start with `[` and end with `]`.
Do not repeat keys or include duplicate fields anywhere in the JSON.
CRITICAL: "args", "context", and "options" fields MUST be arrays of strings like ["a","b"], NEVER a bare string.

Goal: {goal}
Agent name: {agent_name}
Agent description: {agent_description}
Allowed action types: {action_types}
Each item must be:
{{"action": {{"type": "ShellCommand", "command": "free", "args": ["-m"]}}, "description": "Check memory"}}"#,
            goal = goal.description,
            agent_name = context.agent_name.as_deref().unwrap_or("unknown-agent"),
            agent_description = context
                .agent_description
                .as_deref()
                .unwrap_or("unavailable"),
            action_types = self.allowed_action_types(&context.agent_capabilities),
        )
    }

    fn allowed_actions_description(&self, capabilities: &[String]) -> String {
        let has =
            |required: &str| has_capability(capabilities.iter().map(String::as_str), required);

        // Always-allowed actions
        let mut actions = vec![
            r#"- MemoryStore: {"type": "MemoryStore", "key": "...", "value": "...", "memory_type": "episodic|semantic|procedural"}"#.to_string(),
            r#"- MemoryRecall: {"type": "MemoryRecall", "query": "...", "memory_type": null}"#.to_string(),
            r#"- HitlRequest: {"type": "HitlRequest", "question": "...", "options": ["yes","no"]}"#.to_string(),
            r#"- SendNotification: {"type": "SendNotification", "title": "...", "body": "...", "level": "info|warning|error|success"}"#.to_string(),
            r#"- Noop: {"type": "Noop"}"#.to_string(),
        ];

        // Capability-gated actions
        if has("llm.query") {
            actions.push(
                r#"- LlmQuery: {"type": "LlmQuery", "prompt": "...", "context": ["..."]}"#
                    .to_string(),
            );
        }
        if has("fs.read") {
            actions.push(r#"- FileRead: {"type": "FileRead", "path": "..."}"#.to_string());
        }
        if has("fs.write") {
            actions.push(
                r#"- FileWrite: {"type": "FileWrite", "path": "...", "content": "..."}"#
                    .to_string(),
            );
        }
        if has("process.exec") {
            actions.push(
                r#"- ShellCommand: {"type": "ShellCommand", "command": "...", "args": ["..."]}"#
                    .to_string(),
            );
            actions.push(
                r#"- CodeExecute: {"type": "CodeExecute", "language": "python3|node|bash", "code": "...", "timeout_secs": 10}"#
                    .to_string(),
            );
        }
        if has("web.search") {
            actions.push(r#"- WebSearch: {"type": "WebSearch", "query": "..."}"#.to_string());
        }
        if has("web.read") {
            actions.push(r#"- WebFetch: {"type": "WebFetch", "url": "..."}"#.to_string());
        }
        if has("mcp.call") {
            actions.push(
                r#"- ApiCall: {"type": "ApiCall", "method": "GET|POST|PUT|DELETE|PATCH|HEAD", "url": "...", "body": null, "headers": null}"#
                    .to_string(),
            );
        }
        if capabilities.iter().any(|c| c == "agent.message") {
            actions.push(r#"- AgentMessage: {"type": "AgentMessage", "target_agent": "...", "message": "..."}"#.to_string());
        }

        actions.join("\n")
    }

    fn allowed_action_types(&self, capabilities: &[String]) -> String {
        let has =
            |required: &str| has_capability(capabilities.iter().map(String::as_str), required);

        let mut actions = vec![
            "MemoryStore",
            "MemoryRecall",
            "SendNotification",
            "HitlRequest",
            "Noop",
        ];
        if has("llm.query") {
            actions.push("LlmQuery");
        }
        if has("fs.read") {
            actions.push("FileRead");
        }
        if has("fs.write") {
            actions.push("FileWrite");
        }
        if has("process.exec") {
            actions.push("ShellCommand");
            actions.push("CodeExecute");
        }
        if has("web.search") {
            actions.push("WebSearch");
        }
        if has("web.read") {
            actions.push("WebFetch");
        }
        if has("mcp.call") {
            actions.push("ApiCall");
        }
        if capabilities.iter().any(|c| c == "agent.message") {
            actions.push("AgentMessage");
        }

        actions.join(", ")
    }

    fn query_plan_with_retry(
        &self,
        primary_prompt: &str,
        retry_prompt: &str,
        goal_description: &str,
        goal_id: &str,
        capabilities: &[String],
    ) -> Result<Vec<AgentStep>, AgentError> {
        let first_response = self.llm.plan_query(primary_prompt)?;
        eprintln!(
            "[planner:{}] raw LLM response ({} chars): {}",
            goal_id,
            first_response.len(),
            &first_response[..first_response.len().min(500)]
        );
        match self.parse_plan_response(&first_response, goal_id, capabilities) {
            Ok(steps) => {
                eprintln!(
                    "[planner:{}] parsed {} steps: {}",
                    goal_id,
                    steps.len(),
                    steps
                        .iter()
                        .map(|s| s.action.action_type())
                        .collect::<Vec<_>>()
                        .join(", ")
                );
                Ok(steps)
            }
            Err(first_error) => {
                eprintln!(
                    "[planner:{}] parse FAILED (attempt 1): {}",
                    goal_id, first_error
                );
                if !is_retriable_parse_error(&first_error) {
                    return Err(first_error);
                }
                let retry_response = self.llm.plan_query(retry_prompt)?;
                eprintln!(
                    "[planner:{}] retry raw LLM response ({} chars): {}",
                    goal_id,
                    retry_response.len(),
                    &retry_response[..retry_response.len().min(500)]
                );
                match self.parse_plan_response(&retry_response, goal_id, capabilities) {
                    Ok(steps) => {
                        eprintln!(
                            "[planner:{}] retry parsed {} steps: {}",
                            goal_id,
                            steps.len(),
                            steps
                                .iter()
                                .map(|s| s.action.action_type())
                                .collect::<Vec<_>>()
                                .join(", ")
                        );
                        Ok(steps)
                    }
                    Err(second_error) => {
                        eprintln!(
                            "[planner:{}] parse FAILED (attempt 2): {} — falling back to direct LLM query",
                            goal_id, second_error
                        );
                        Ok(vec![self.llm_query_fallback_step(
                            goal_id,
                            goal_description,
                            format!(
                                "Planner returned invalid JSON twice. First error: {first_error}. Second error: {second_error}"
                            ),
                        )])
                    }
                }
            }
        }
    }

    fn parse_plan_response(
        &self,
        response: &str,
        goal_id: &str,
        capabilities: &[String],
    ) -> Result<Vec<AgentStep>, AgentError> {
        let json_str = extract_json_array(response).ok_or_else(|| {
            AgentError::SupervisorError(format!(
                "planner response did not contain a valid JSON array (response: {} chars)",
                response.len()
            ))
        })?;

        eprintln!(
            "[planner:{}] extracted {} chars of JSON from {} chars of response",
            goal_id,
            json_str.len(),
            response.len()
        );

        let repaired = repair_common_json_issues(&json_str);
        let raw_steps = parse_raw_steps(&json_str)
            .or_else(|e| {
                eprintln!("[planner:{}] parse attempt 1 failed: {e}", goal_id);
                parse_raw_steps(&remove_trailing_commas(&json_str))
            })
            .or_else(|e| {
                eprintln!("[planner:{}] parse attempt 2 failed: {e}", goal_id);
                parse_raw_steps(&repaired)
            })
            .map_err(|e| AgentError::SupervisorError(format!("failed to parse plan JSON: {e}")))?;

        let mut steps = Vec::new();
        for raw in raw_steps {
            // Validate that the action's required capabilities are present
            let required = raw.action.required_capabilities();
            for cap in &required {
                if !has_capability(capabilities.iter().map(String::as_str), cap) {
                    return Err(AgentError::SupervisorError(format!(
                        "planner produced action '{}' requiring capability '{}' not in agent manifest",
                        raw.action.action_type(),
                        cap
                    )));
                }
            }
            steps.push(AgentStep::new(goal_id.to_string(), raw.action));
        }

        Ok(steps)
    }

    fn llm_query_fallback_step(
        &self,
        goal_id: &str,
        goal_description: &str,
        message: String,
    ) -> AgentStep {
        AgentStep::new(
            goal_id.to_string(),
            PlannedAction::LlmQuery {
                prompt: format!(
                    "IMPORTANT: The structured planner failed to produce a valid action plan. \
                     You MUST NOT hallucinate or fabricate data. If the goal requires running a command, \
                     reading a file, or fetching data, say exactly: \
                     'I was unable to create an execution plan. The goal was: {goal_description}. \
                     Error: {message}'. \
                     Only answer if you can do so from your training data alone, and clearly state \
                     that this is from general knowledge, not from executing any command."
                ),
                context: vec![message],
            },
        )
    }
}

/// Raw step from LLM JSON response.
#[derive(Debug, Deserialize)]
struct RawStep {
    action: PlannedAction,
    // Deserialized from LLM JSON response; kept for debug / logging.
    #[allow(dead_code)]
    description: Option<String>,
}

fn parse_raw_steps(json_str: &str) -> Result<Vec<RawStep>, String> {
    let value: Value =
        serde_json::from_str(json_str).map_err(|error| format!("invalid JSON: {error}"))?;
    let items = value
        .as_array()
        .ok_or_else(|| "planner JSON root was not an array".to_string())?;

    items
        .iter()
        .map(|item| {
            // Try nested format first: {"action": {...}, "description": "..."}
            serde_json::from_value::<RawStep>(item.clone())
                .or_else(|_| {
                    // Try flat format: {"type": "ShellCommand", ...}
                    // Wrap it into a RawStep
                    serde_json::from_value::<PlannedAction>(item.clone()).map(|action| RawStep {
                        action,
                        description: None,
                    })
                })
                .map_err(|error| format!("invalid planner step: {error}"))
        })
        .collect()
}

/// Extract a JSON array (or single object) from LLM text that may contain
/// preamble, trailing explanation, markdown fences, or chat template markers.
///
/// Uses bracket-depth tracking with string escape awareness so it finds the
/// correct closing `]` even when the JSON contains nested arrays like `"args": ["-m"]`.
fn extract_json_array(text: &str) -> Option<String> {
    let sanitized = sanitize_llm_response(text);
    let bytes = sanitized.as_bytes();

    // Strategy 1: Find the first `[` and its matching `]` via depth tracking
    if let Some(start) = sanitized.find('[') {
        if let Some(end) = find_matching_bracket(bytes, start, b'[', b']') {
            let candidate = sanitized[start..=end].trim().to_string();
            // Quick sanity: must contain at least one `{`
            if candidate.contains('{') {
                return Some(candidate);
            }
        }
    }

    // Strategy 2: Find the first `{` and its matching `}` (single action, wrap in array)
    if let Some(start) = sanitized.find('{') {
        if let Some(end) = find_matching_bracket(bytes, start, b'{', b'}') {
            let obj = &sanitized[start..=end];
            // Only wrap if it looks like a planner action (has "type" key)
            if obj.contains("\"type\"") {
                return Some(format!("[{obj}]"));
            }
        }
    }

    None
}

/// Find the index of the closing bracket that matches the opening bracket at `start`.
/// Tracks string escaping so brackets inside JSON strings are ignored.
fn find_matching_bracket(bytes: &[u8], start: usize, open: u8, close: u8) -> Option<usize> {
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escape_next = false;

    for (i, &byte) in bytes.iter().enumerate().skip(start) {
        if escape_next {
            escape_next = false;
            continue;
        }
        match byte {
            b'\\' if in_string => escape_next = true,
            b'"' => in_string = !in_string,
            b if b == open && !in_string => depth += 1,
            b if b == close && !in_string => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

fn sanitize_llm_response(input: &str) -> String {
    let no_think = strip_think_tags(input);
    strip_markdown_fences(&no_think).trim().to_string()
}

/// Strip `<think>...</think>` reasoning blocks emitted by Qwen3 and similar models.
/// These blocks contain the model's internal reasoning and must be removed before
/// parsing the actual JSON output.
fn strip_think_tags(input: &str) -> String {
    let mut result = input.to_string();
    // Remove all <think>...</think> blocks (may span multiple lines)
    while let Some(start) = result.find("<think>") {
        if let Some(end) = result[start..].find("</think>") {
            result = format!("{}{}", &result[..start], &result[start + end + 8..]);
        } else {
            // Unclosed <think> tag — remove everything from <think> to end
            result.truncate(start);
            break;
        }
    }
    result
}

fn strip_markdown_fences(input: &str) -> String {
    static FENCE_LINE_RE: std::sync::LazyLock<Regex> =
        std::sync::LazyLock::new(|| match Regex::new(r"(?m)^\s*```[a-zA-Z0-9_-]*\s*$") {
            Ok(re) => re,
            Err(e) => {
                eprintln!("Failed to compile markdown fence regex: {e}");
                match Regex::new("^$") {
                    Ok(re) => re,
                    Err(_) => std::process::abort(),
                }
            }
        });
    FENCE_LINE_RE.replace_all(input, "").to_string()
}

fn repair_common_json_issues(input: &str) -> String {
    let no_trailing_commas = remove_trailing_commas(input);
    remove_duplicate_json_fields(&no_trailing_commas)
}

fn remove_duplicate_json_fields(input: &str) -> String {
    let mut current = input.to_string();
    let candidate_keys = [
        "args",
        "context",
        "options",
        "body",
        "path",
        "content",
        "command",
        "method",
        "url",
        "query",
        "key",
        "value",
        "memory_type",
        "prompt",
        "description",
        "type",
    ];
    let value_pattern = r#"\[[^\[\]]*\]|"(?:[^"\\]|\\.)*"|-?\d+(?:\.\d+)?|true|false|null"#;

    loop {
        let mut updated = current.clone();
        for key in candidate_keys {
            let pattern = format!(
                r#",\s*"{key}"\s*:\s*(?:{value_pattern})\s*,\s*"{key}"\s*:\s*(?P<value>{value_pattern})"#
            );
            let duplicate_field_re = match Regex::new(&pattern) {
                Ok(re) => re,
                Err(_) => continue,
            };
            let replacement = format!(r#","{key}": $value"#);
            updated = duplicate_field_re
                .replace_all(&updated, replacement.as_str())
                .to_string();
        }
        if updated == current {
            return updated;
        }
        current = updated;
    }
}

fn remove_trailing_commas(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_string = false;
    let mut escape = false;
    let chars: Vec<char> = input.chars().collect();
    let mut index = 0usize;

    while index < chars.len() {
        let ch = chars[index];

        if escape {
            out.push(ch);
            escape = false;
            index += 1;
            continue;
        }

        if ch == '\\' && in_string {
            out.push(ch);
            escape = true;
            index += 1;
            continue;
        }

        if ch == '"' {
            in_string = !in_string;
            out.push(ch);
            index += 1;
            continue;
        }

        if !in_string && ch == ',' {
            let mut lookahead = index + 1;
            while lookahead < chars.len() && chars[lookahead].is_whitespace() {
                lookahead += 1;
            }
            if lookahead < chars.len() && matches!(chars[lookahead], ']' | '}') {
                index += 1;
                continue;
            }
        }

        out.push(ch);
        index += 1;
    }

    out
}

fn is_retriable_parse_error(error: &AgentError) -> bool {
    let message = error.to_string();
    message.contains("planner response did not contain a valid JSON array")
        || message.contains("failed to parse plan JSON")
}

#[cfg(test)]
mod tests {
    use super::super::types::StepStatus;
    use super::*;
    use std::sync::Mutex;

    struct MockLlm {
        response: String,
    }

    impl PlannerLlm for MockLlm {
        fn plan_query(&self, _prompt: &str) -> Result<String, AgentError> {
            Ok(self.response.clone())
        }
    }

    struct SequenceMockLlm {
        responses: Mutex<Vec<String>>,
    }

    impl PlannerLlm for SequenceMockLlm {
        fn plan_query(&self, _prompt: &str) -> Result<String, AgentError> {
            let mut responses = self.responses.lock().unwrap();
            if responses.is_empty() {
                return Err(AgentError::SupervisorError(
                    "no more mock planner responses".to_string(),
                ));
            }
            Ok(responses.remove(0))
        }
    }

    fn make_context(caps: Vec<&str>) -> PlanningContext {
        PlanningContext {
            agent_name: Some("planner-test-agent".to_string()),
            agent_description: Some(
                "A governed planning test agent focused on producing valid execution plans."
                    .to_string(),
            ),
            agent_capabilities: caps.into_iter().map(|s| s.to_string()).collect(),
            available_fuel: 1000.0,
            relevant_memories: vec![],
            previous_outcomes: vec![],
            working_directory: None,
            autonomy_level: 2,
        }
    }

    #[test]
    fn test_plan_goal_basic() {
        let llm = MockLlm {
            response: r#"[
                {"action": {"type": "LlmQuery", "prompt": "analyze code", "context": []}, "description": "analyze"},
                {"action": {"type": "MemoryStore", "key": "result", "value": "done", "memory_type": "episodic"}, "description": "store"}
            ]"#
            .to_string(),
        };
        let planner = CognitivePlanner::new(Box::new(llm));
        let goal = AgentGoal::new("test goal".into(), 5);
        let ctx = make_context(vec!["llm.query"]);
        let steps = planner.plan_goal(&goal, &ctx).unwrap();
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0].action.action_type(), "llm_query");
        assert_eq!(steps[1].action.action_type(), "memory_store");
        assert_eq!(steps[0].status, StepStatus::Planned);
    }

    #[test]
    fn test_plan_rejects_unauthorized_action() {
        let llm = MockLlm {
            response: r#"[
                {"action": {"type": "FileWrite", "path": "/tmp/x", "content": "hack"}, "description": "write"}
            ]"#
            .to_string(),
        };
        let planner = CognitivePlanner::new(Box::new(llm));
        let goal = AgentGoal::new("test".into(), 5);
        // Only has fs.read, not fs.write
        let ctx = make_context(vec!["fs.read"]);
        let result = planner.plan_goal(&goal, &ctx);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("fs.write"));
    }

    #[test]
    fn test_plan_allows_always_allowed_actions() {
        let llm = MockLlm {
            response: r#"[
                {"action": {"type": "MemoryStore", "key": "k", "value": "v", "memory_type": "episodic"}, "description": "store"},
                {"action": {"type": "MemoryRecall", "query": "q", "memory_type": null}, "description": "recall"},
                {"action": {"type": "HitlRequest", "question": "ok?", "options": ["yes"]}, "description": "ask"},
                {"action": {"type": "Noop"}, "description": "wait"}
            ]"#
            .to_string(),
        };
        let planner = CognitivePlanner::new(Box::new(llm));
        let goal = AgentGoal::new("test".into(), 5);
        // No capabilities at all
        let ctx = make_context(vec![]);
        let steps = planner.plan_goal(&goal, &ctx).unwrap();
        assert_eq!(steps.len(), 4);
    }

    #[test]
    fn test_replan_after_failure() {
        let llm = MockLlm {
            response: r#"[
                {"action": {"type": "LlmQuery", "prompt": "retry with different approach", "context": ["error context"]}, "description": "retry"}
            ]"#
            .to_string(),
        };
        let planner = CognitivePlanner::new(Box::new(llm));
        let goal = AgentGoal::new("test".into(), 5);
        let failed = AgentStep {
            id: "s1".into(),
            goal_id: goal.id.clone(),
            action: PlannedAction::LlmQuery {
                prompt: "original".into(),
                context: vec![],
            },
            status: StepStatus::Failed,
            result: None,
            fuel_cost: 5.0,
            attempts: 2,
            max_retries: 2,
        };
        let ctx = make_context(vec!["llm.query"]);
        let steps = planner
            .replan_after_failure(&goal, &failed, "timeout", &[], &ctx)
            .unwrap();
        assert_eq!(steps.len(), 1);
    }

    #[test]
    fn test_extract_json_from_markdown() {
        let text = r#"Here is the plan:

```json
[{"action": {"type": "Noop"}, "description": "wait"}]
```

Done."#;
        let llm = MockLlm {
            response: text.to_string(),
        };
        let planner = CognitivePlanner::new(Box::new(llm));
        let goal = AgentGoal::new("test".into(), 5);
        let ctx = make_context(vec![]);
        let steps = planner.plan_goal(&goal, &ctx).unwrap();
        assert_eq!(steps.len(), 1);
    }

    #[test]
    fn test_extract_json_array() {
        // Basic valid JSON arrays with objects
        assert!(extract_json_array(r#"[{"type":"Noop"}]"#).is_some());
        assert!(extract_json_array("no json here").is_none());

        // Markdown fences
        assert!(extract_json_array("```json\n[{\"type\":\"Noop\"}]\n```").is_some());
        assert!(extract_json_array("```\n[{\"type\":\"Noop\"}]\n```").is_some());

        // Preamble and trailing text
        assert!(extract_json_array("Here is the plan:\n[{\"type\":\"Noop\"}]\nDone.").is_some());
        assert_eq!(
            extract_json_array("preface\n```json\n[{\"type\":\"Noop\"}]\n```\ntrailer").as_deref(),
            Some(r#"[{"type":"Noop"}]"#)
        );

        // Trailing chat template markers (the real-world failure case)
        let with_trailing = r#"[{"action":{"type":"ShellCommand","command":"free","args":["-m"]},"description":"check"}]

<|im_start|>user
Your previous response had invalid JSON."#;
        let extracted = extract_json_array(with_trailing).unwrap();
        assert!(extracted.starts_with('['));
        assert!(extracted.ends_with(']'));
        assert!(!extracted.contains("<|im_start|>"));

        // Nested arrays in args don't confuse the bracket matcher
        let nested = r#"[{"type":"ShellCommand","command":"ls","args":["-la","/tmp"]}]"#;
        assert_eq!(extract_json_array(nested).as_deref(), Some(nested));

        // Single object → wrapped in array
        let single = r#"{"type":"ShellCommand","command":"free","args":["-m"]}"#;
        let wrapped = extract_json_array(single).unwrap();
        assert!(wrapped.starts_with('['));
        assert!(wrapped.ends_with(']'));
    }

    #[test]
    fn test_invalid_json_returns_llm_query_fallback() {
        let llm = MockLlm {
            response: "not json at all".to_string(),
        };
        let planner = CognitivePlanner::new(Box::new(llm));
        let goal = AgentGoal::new("test".into(), 5);
        let ctx = make_context(vec![]);
        let steps = planner.plan_goal(&goal, &ctx).unwrap();
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].action.action_type(), "llm_query");
    }

    #[test]
    fn test_build_planning_prompt_contains_goal() {
        let llm = MockLlm {
            response: "[]".to_string(),
        };
        let planner = CognitivePlanner::new(Box::new(llm));
        let goal = AgentGoal::new("build a widget".into(), 7);
        let ctx = make_context(vec!["llm.query", "fs.read"]);
        let prompt = planner.build_planning_prompt(&goal, &ctx);
        assert!(prompt.contains("build a widget"));
        assert!(prompt.contains("llm.query, fs.read"));
        assert!(prompt.contains("LlmQuery"));
        assert!(prompt.contains("FileRead"));
        assert!(prompt.contains("planner-test-agent"));
        assert!(prompt.contains("A governed planning test agent"));
        assert!(prompt.contains("Do NOT include duplicate fields"));
        assert!(prompt.contains("EXACT JSON SCHEMA EXAMPLE"));
    }

    #[test]
    fn test_duplicate_fields_are_tolerated_via_value_parsing() {
        let llm = MockLlm {
            response: r#"[
                {"action": {"type": "ShellCommand", "command": "ls", "args": ["-l"], "args": ["-la"]}, "description": "inspect"}
            ]"#
            .to_string(),
        };
        let planner = CognitivePlanner::new(Box::new(llm));
        let goal = AgentGoal::new("inspect workspace".into(), 5);
        let ctx = make_context(vec!["process.exec"]);

        let steps = planner.plan_goal(&goal, &ctx).unwrap();
        assert_eq!(steps.len(), 1);
        match &steps[0].action {
            PlannedAction::ShellCommand { command, args } => {
                assert_eq!(command, "ls");
                assert_eq!(args, &vec!["-la".to_string()]);
            }
            other => panic!("unexpected action: {other:?}"),
        }
    }

    #[test]
    fn test_invalid_json_retries_and_recovers() {
        let llm = SequenceMockLlm {
            responses: Mutex::new(vec![
                "```json\n[{\"action\": {\"type\": \"Noop\"}, \"description\": \"wait\",}]\n```"
                    .to_string(),
                r#"[{"action": {"type": "Noop"}, "description": "wait"}]"#.to_string(),
            ]),
        };
        let planner = CognitivePlanner::new(Box::new(llm));
        let goal = AgentGoal::new("recover from invalid json".into(), 5);
        let ctx = make_context(vec![]);

        let steps = planner.plan_goal(&goal, &ctx).unwrap();
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].action.action_type(), "noop");
    }

    #[test]
    fn test_double_invalid_json_returns_llm_query_step() {
        let llm = SequenceMockLlm {
            responses: Mutex::new(vec![
                "not valid json".to_string(),
                "still not valid json".to_string(),
            ]),
        };
        let planner = CognitivePlanner::new(Box::new(llm));
        let goal = AgentGoal::new("gracefully degrade".into(), 5);
        let ctx = make_context(vec![]);

        let steps = planner.plan_goal(&goal, &ctx).unwrap();
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].action.action_type(), "llm_query");
        // Fallback step should NOT pre-populate result (that caused hallucination).
        // Instead, the prompt itself warns the LLM not to fabricate data.
        assert!(steps[0].result.is_none());
        if let PlannedAction::LlmQuery { ref prompt, .. } = steps[0].action {
            assert!(prompt.contains("structured planner failed"));
        } else {
            panic!("expected LlmQuery fallback action");
        }
    }

    #[test]
    fn test_strip_markdown_fences_removes_embedded_fence_lines() {
        let stripped = strip_markdown_fences("before\n```json\n[{\"a\":1}]\n```\nafter");
        let normalized = stripped
            .lines()
            .filter(|line| !line.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(normalized, "before\n[{\"a\":1}]\nafter");
    }

    #[test]
    fn test_strip_think_tags() {
        // Basic think block removal
        let input = "<think>I need to check memory</think>[{\"action\":{\"type\":\"Noop\"},\"description\":\"test\"}]";
        let result = strip_think_tags(input);
        assert!(
            result.starts_with('['),
            "should start with JSON array, got: {result}"
        );
        assert!(!result.contains("<think>"));

        // Multi-line think block
        let input = "<think>\nLet me reason about this.\nI should use free -m.\n</think>\n[{\"action\":{\"type\":\"Noop\"},\"description\":\"x\"}]";
        let result = strip_think_tags(input);
        assert!(!result.contains("<think>"));
        assert!(result.contains("Noop"));

        // No think tags — passthrough
        let input = "[{\"action\":{\"type\":\"Noop\"},\"description\":\"x\"}]";
        assert_eq!(strip_think_tags(input), input);

        // Unclosed think tag — remove to end
        let input = "some text<think>endless reasoning";
        let result = strip_think_tags(input);
        assert_eq!(result, "some text");
    }

    #[test]
    fn test_sanitize_strips_think_before_extracting_json() {
        let input = "<think>reasoning here</think>\n```json\n[{\"action\":{\"type\":\"Noop\"},\"description\":\"ok\"}]\n```";
        let sanitized = sanitize_llm_response(input);
        assert!(!sanitized.contains("<think>"));
        assert!(sanitized.contains("Noop"));
    }
}
