//! Cognitive planner — translates goals into executable step sequences via LLM.
//!
//! P3-HM-CONTRACT-02 §11.3, §11.4: every planning dispatch takes the unit's
//! `DispatchContext` and a prompt source the model adapter can call again
//! for the one rebuild (§12.7). Retained records enter a planning prompt
//! only as verbatim items (`PromptBuilder::include`), recalled route-first
//! for the unit by the caller's recall source; the reply is a derived item.

use super::types::{AgentGoal, AgentStep, PlannedAction, PlanningContext};
use crate::capabilities::has_capability;
use crate::errors::AgentError;
use crate::hm::{
    DerivedItem, DispatchContext, PromptBuilder, PromptEnvelope, PromptSource, RecalledRecord,
};
use regex::Regex;
use serde::Deserialize;
use serde_json::Value;

/// The planner's model (§11.4). A path without a context does not compile.
pub trait PlannerLlm: Send + Sync {
    /// Dispatches the envelope `prompt` builds for `ctx` (again after the
    /// one rebuild) and returns the reply as a derived item.
    fn plan_query(
        &self,
        ctx: &mut DispatchContext,
        prompt: &mut PromptSource<'_>,
    ) -> Result<DerivedItem, AgentError>;
}

/// The retained records a planning prompt includes (§12.3 item 4), each a
/// verbatim item recalled route-first for the unit.
#[derive(Debug, Default)]
pub struct PlanningRecall {
    /// RELEVANT MEMORIES.
    pub memories: Vec<RecalledRecord>,
    /// PREVIOUS OUTCOMES: the best strategy for the goal type.
    pub best_strategy: Option<RecalledRecord>,
    /// PREVIOUS OUTCOMES: the evolved planning prompt (`optimized_prompt`).
    pub optimized_prompt: Option<RecalledRecord>,
}

/// The recall of one planning envelope. It is called for each envelope,
/// so a rebuild recalls afresh under the rebuilt context (§12.7).
pub type PlanRecallSource<'a> = dyn FnMut(&mut DispatchContext) -> PlanningRecall + 'a;

/// A plan and the model reply it was parsed from. Every step is derived
/// from `reply`: text built from a step keeps its label.
#[derive(Debug)]
pub struct Plan {
    pub steps: Vec<AgentStep>,
    pub reply: DerivedItem,
}

/// Generates executable step plans from high-level goals.
pub struct CognitivePlanner {
    llm: Box<dyn PlannerLlm>,
}

impl CognitivePlanner {
    pub fn new(llm: Box<dyn PlannerLlm>) -> Self {
        Self { llm }
    }

    /// Plan steps for a goal of the unit `ctx` given the agent's context.
    /// `recall` supplies the retained records of each planning envelope.
    pub fn plan_goal(
        &self,
        ctx: &mut DispatchContext,
        goal: &AgentGoal,
        context: &PlanningContext,
        recall: &mut PlanRecallSource<'_>,
    ) -> Result<Plan, AgentError> {
        let (head, tail) = self.planning_text(goal, context);
        let retry = self.build_invalid_json_retry_prompt(goal, context);
        self.query_plan_with_retry(
            ctx,
            &mut |c: &mut DispatchContext| {
                let records = recall(c);
                Some(planning_envelope(c, &head, &tail, &records))
            },
            &mut |c: &mut DispatchContext| Some(text_envelope(c, &retry)),
            &goal.description,
            &goal.id,
            &context.agent_capabilities,
        )
    }

    /// Re-plan after a step failure, incorporating the error context. The
    /// failed and remaining steps come from `previous_plan`, so the text
    /// naming them is carried with its label.
    #[allow(clippy::too_many_arguments)]
    pub fn replan_after_failure(
        &self,
        ctx: &mut DispatchContext,
        goal: &AgentGoal,
        failed_step: &AgentStep,
        error: &str,
        remaining_steps: &[AgentStep],
        context: &PlanningContext,
        previous_plan: &DerivedItem,
    ) -> Result<Plan, AgentError> {
        let (head, plan_part, tail) =
            self.build_replan_prompt(goal, failed_step, error, remaining_steps, context);
        let from_plan = previous_plan.derive(plan_part);
        let retry = self.build_invalid_json_retry_prompt(goal, context);
        self.query_plan_with_retry(
            ctx,
            &mut |c: &mut DispatchContext| {
                let mut b = PromptBuilder::new(c);
                b.text(&head);
                b.carry(&from_plan);
                b.text(&tail);
                Some(b.build())
            },
            &mut |c: &mut DispatchContext| Some(text_envelope(c, &retry)),
            &goal.description,
            &goal.id,
            &context.agent_capabilities,
        )
    }

    /// The planning prompt's backend text: everything before RELEVANT
    /// MEMORIES and everything after PREVIOUS OUTCOMES. It carries no
    /// retained record: the goal is the unit's own input, the rest is the
    /// agent's manifest and constant instructions.
    fn planning_text(&self, goal: &AgentGoal, context: &PlanningContext) -> (String, String) {
        let agent_name = context.agent_name.as_deref().unwrap_or("unknown-agent");
        let agent_description = context.agent_description.as_deref().unwrap_or(
            "No additional role description was provided. Infer the role from the goal and capabilities.",
        );
        let capabilities_str = context.agent_capabilities.join(", ");

        let allowed_actions = self.allowed_actions_description(&context.agent_capabilities);
        // G8: splice in the real top-level cwd contents so the planner can
        // resolve phrases like "src/" against the actual layout rather than
        // hallucinating paths. Empty string when no listing is available.
        // G8b: added the inference hint sentence so small models (gemma4:e2b/e4b)
        // don't silently construct paths at the workspace root when the user
        // reference isn't visible at the top level.
        let cwd_block = match &context.directory_listing {
            Some(listing) => format!(
                "\n\nCurrent working directory contents (top-level, max 60 entries):\n{listing}\n\nIf the user mentions a directory or file not visible in the listing above, it is likely nested inside one of the listed directories — search for it there before assuming the path is at the workspace root.\n"
            ),
            None => String::new(),
        };
        // P0-002C5A: without a workspace the planner is told so, rather than
        // being handed an invented directory to read from and write to.
        let workspace_block = match context.working_directory.as_deref() {
            Some(workspace) => format!(
                "WORKSPACE DIRECTORY: {workspace}\n(Use ABSOLUTE paths starting from this directory when reading/writing files. Example: \"{workspace}/README.md\"){cwd_block}"
            ),
            None => "WORKSPACE DIRECTORY: none (no filesystem workspace is available to this agent)"
                .to_string(),
        };

        let head = format!(
            r#"CRITICAL: Output ONLY a JSON array. Do NOT think out loud. Do NOT use <think> tags. Do NOT explain. Start your response with `[` and end with `]`.

You are the planning subsystem for Nexus OS. Create a step-by-step plan to achieve the goal below.

AGENT NAME: {agent_name}
AGENT DESCRIPTION:
{agent_description}

GOAL: {goal_desc}
PRIORITY: {priority}

{workspace_block}

AGENT CAPABILITIES: [{capabilities}]
AVAILABLE FUEL: {fuel}
AUTONOMY LEVEL: L{autonomy}

"#,
            agent_name = agent_name,
            agent_description = agent_description,
            goal_desc = goal.description,
            priority = goal.priority,
            capabilities = capabilities_str,
            fuel = context.available_fuel,
            autonomy = context.autonomy_level,
        );
        let tail = format!(
            r#"ALLOWED ACTIONS (you MUST only use these):
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
            allowed_actions = allowed_actions,
            action_types = self.allowed_action_types(&context.agent_capabilities),
        );
        // G8b diagnostic — TEMPORARY, remove after C3 verified
        let cwd_present = head.contains("Current working directory contents");
        let cwd_idx = head
            .find("Current working directory contents")
            .map(|i| i.to_string())
            .unwrap_or_else(|| "NOT FOUND".to_string());
        eprintln!(
            "[g8-diag] build_planning_prompt assembled: text_len={}, cwd_block_present={}, cwd_block_position={}",
            head.len() + tail.len(),
            cwd_present,
            cwd_idx
        );
        (head, tail)
    }

    /// The replan prompt as (text before, text from the previous plan,
    /// text after): the failed and remaining steps come from the plan.
    fn build_replan_prompt(
        &self,
        goal: &AgentGoal,
        failed_step: &AgentStep,
        error: &str,
        remaining_steps: &[AgentStep],
        context: &PlanningContext,
    ) -> (String, String, String) {
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

        let head = format!(
            r#"You are the planning subsystem for Nexus OS. A step in your plan failed. Create an adapted plan.

AGENT NAME: {agent_name}
AGENT DESCRIPTION:
{agent_description}

GOAL: {goal_desc}
"#,
            agent_name = agent_name,
            agent_description = agent_description,
            goal_desc = goal.description,
        );
        let from_plan = format!(
            r#"FAILED STEP: {failed_action} (attempt {attempt}/{max})
ERROR: {error}

REMAINING ORIGINAL STEPS:
{remaining}
"#,
            failed_action = failed_step.action.action_type(),
            attempt = failed_step.attempts,
            max = failed_step.max_retries,
            error = error,
            remaining = remaining_str,
        );
        let tail = format!(
            r#"
AGENT CAPABILITIES: [{capabilities}]
AVAILABLE FUEL: {fuel}

ALLOWED ACTIONS (you MUST only use these):
{allowed_actions}

Create an adapted JSON plan that works around the failure.
Respond with ONLY a valid JSON array of step objects.
Do NOT include duplicate fields.
Do NOT include any text outside the JSON array."#,
            capabilities = context.agent_capabilities.join(", "),
            fuel = context.available_fuel,
            allowed_actions = allowed_actions,
        );
        (head, from_plan, tail)
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
        ctx: &mut DispatchContext,
        primary: &mut PromptSource<'_>,
        retry: &mut PromptSource<'_>,
        goal_description: &str,
        goal_id: &str,
        capabilities: &[String],
    ) -> Result<Plan, AgentError> {
        let first = self.llm.plan_query(ctx, primary)?;
        let first_response = first.text();
        eprintln!(
            "[planner:{}] raw LLM response ({} chars): {}",
            goal_id,
            first_response.len(),
            &first_response[..first_response.floor_char_boundary(500)]
        );
        match self.parse_plan_response(first_response, goal_id, capabilities) {
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
                Ok(Plan {
                    steps,
                    reply: first,
                })
            }
            Err(first_error) => {
                eprintln!(
                    "[planner:{}] parse FAILED (attempt 1): {}",
                    goal_id, first_error
                );
                if !is_retriable_parse_error(&first_error) {
                    return Err(first_error);
                }
                let second = self.llm.plan_query(ctx, retry)?;
                let retry_response = second.text();
                eprintln!(
                    "[planner:{}] retry raw LLM response ({} chars): {}",
                    goal_id,
                    retry_response.len(),
                    &retry_response[..retry_response.floor_char_boundary(500)]
                );
                match self.parse_plan_response(retry_response, goal_id, capabilities) {
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
                        Ok(Plan {
                            steps,
                            reply: second,
                        })
                    }
                    Err(second_error) => {
                        eprintln!(
                            "[planner:{}] parse FAILED (attempt 2): {} — falling back to direct LLM query",
                            goal_id, second_error
                        );
                        let step = self.llm_query_fallback_step(
                            goal_id,
                            goal_description,
                            format!(
                                "Planner returned invalid JSON twice. First error: {first_error}. Second error: {second_error}"
                            ),
                        );
                        // The fallback plan is derived from both replies.
                        let reply = DerivedItem::combine(&[&first, &second], String::new())
                            .unwrap_or(second);
                        Ok(Plan {
                            steps: vec![step],
                            reply,
                        })
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

        let sanitized = escape_control_chars_in_strings(&json_str);
        let repaired = repair_common_json_issues(&sanitized);
        let raw_steps = parse_raw_steps(&sanitized)
            .or_else(|e| {
                eprintln!("[planner:{}] parse attempt 1 failed: {e}", goal_id);
                parse_raw_steps(&remove_trailing_commas(&sanitized))
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
        _message: String,
    ) -> AgentStep {
        AgentStep::new(
            goal_id.to_string(),
            PlannedAction::LlmQuery {
                prompt: format!(
                    "Answer this as best you can from general knowledge:\n\n{goal_description}"
                ),
                context: vec![],
            },
        )
    }
}

/// The planning envelope: the backend text, with each recalled record
/// included verbatim (§11.2). No retained content enters as text.
fn planning_envelope(
    ctx: &DispatchContext,
    head: &str,
    tail: &str,
    recall: &PlanningRecall,
) -> PromptEnvelope {
    let mut b = PromptBuilder::new(ctx);
    b.text(head);
    b.text("RELEVANT MEMORIES:\n");
    if recall.memories.is_empty() {
        b.text("- None\n");
    }
    for memory in &recall.memories {
        b.text("- ");
        b.include(memory);
    }
    b.text("\nPREVIOUS OUTCOMES:\n");
    if recall.best_strategy.is_none() && recall.optimized_prompt.is_none() {
        b.text("- None\n");
    }
    if let Some(strategy) = &recall.best_strategy {
        b.text("- Previously, the most successful approach for similar goals was:\n");
        b.include(strategy);
        b.text("  Consider using or adapting this approach.\n");
    }
    if let Some(guidance) = &recall.optimized_prompt {
        b.text("- Use this evolved planning prompt guidance:\n");
        b.include(guidance);
    }
    b.text("\n");
    b.text(tail);
    b.build()
}

/// An envelope of backend text only.
fn text_envelope(ctx: &DispatchContext, text: &str) -> PromptEnvelope {
    let mut b = PromptBuilder::new(ctx);
    b.text(text);
    b.build()
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

/// Escape literal control characters (newline, tab, etc.) that appear inside
/// JSON string values.  Small LLMs like Gemma4 emit literal `\n`/`\t` bytes
/// inside strings which are illegal per the JSON spec and cause serde_json to
/// reject the payload.  This function walks the input tracking whether we are
/// inside a quoted string and replaces only the offending bytes, leaving
/// everything outside strings untouched.
fn escape_control_chars_in_strings(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_string = false;
    let mut escape_next = false;

    for ch in input.chars() {
        if escape_next {
            out.push(ch);
            escape_next = false;
            continue;
        }

        if ch == '"' {
            in_string = !in_string;
            out.push(ch);
            continue;
        }

        if in_string && ch == '\\' {
            escape_next = true;
            out.push(ch);
            continue;
        }

        if in_string && ch.is_control() {
            match ch {
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                other => {
                    // \u00XX for other control chars
                    let code = other as u32;
                    out.push_str(&format!("\\u{code:04x}"));
                }
            }
            continue;
        }

        out.push(ch);
    }
    out
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
    use crate::hm::testing::{ScriptedLlm, TestHm};
    use crate::hm::RouteClass;
    use std::sync::OnceLock;

    /// The synthetic store and issuer of these tests.
    fn hm() -> &'static TestHm {
        static HM: OnceLock<TestHm> = OnceLock::new();
        HM.get_or_init(TestHm::new)
    }

    /// A goal unit of a registered (local-only) agent.
    fn unit() -> DispatchContext {
        let hm = hm();
        hm.goal(hm.agent("planner-test-agent"))
    }

    /// No retained record for these prompts.
    fn no_recall(_: &mut DispatchContext) -> PlanningRecall {
        PlanningRecall::default()
    }

    /// The model, replying `response` through the issuer on an attested
    /// local transport.
    #[allow(non_snake_case)]
    fn MockLlm(response: &str) -> ScriptedLlm {
        ScriptedLlm::replying(&hm().issuer, RouteClass::Local, response)
    }

    /// The model, replying each of `responses` in order.
    fn sequence(responses: &[&str]) -> ScriptedLlm {
        responses.iter().fold(
            ScriptedLlm::new(&hm().issuer, RouteClass::Local),
            |llm, r| llm.then(r),
        )
    }

    /// `plan_goal` in a fresh unit with no recalled record.
    fn plan(
        planner: &CognitivePlanner,
        goal: &AgentGoal,
        ctx: &PlanningContext,
    ) -> Result<Vec<AgentStep>, AgentError> {
        planner
            .plan_goal(&mut unit(), goal, ctx, &mut no_recall)
            .map(|plan| plan.steps)
    }

    /// The planning prompt's backend text (its records are included by
    /// the envelope, none here).
    fn prompt_text(planner: &CognitivePlanner, goal: &AgentGoal, ctx: &PlanningContext) -> String {
        let (head, tail) = planner.planning_text(goal, ctx);
        format!("{head}{tail}")
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
            working_directory: None,
            directory_listing: None,
            autonomy_level: 2,
        }
    }

    /// M0-FIX F3 oracle: while web search is closed (no governed search
    /// backend), the planner does not offer `WebSearch` or steer the model
    /// to it, even to an agent that holds `web.search`, in the planning
    /// prompt or the replan prompt. `WebFetch` stays offered to `web.read`.
    #[test]
    fn m0_f3_planner_does_not_offer_closed_web_search() {
        let planner = CognitivePlanner::new(Box::new(MockLlm("[]")));
        let goal = AgentGoal::new("find the latest rust release".into(), 5);
        let ctx = make_context(vec!["llm.query", "web.search", "web.read"]);
        let prompt = prompt_text(&planner, &goal, &ctx);
        assert!(!prompt.contains("WebSearch"), "{prompt}");
        assert!(!prompt.to_ascii_lowercase().contains("curl"), "{prompt}");
        assert!(prompt.contains("WebFetch"), "{prompt}");
        let failed = AgentStep::new(goal.id.clone(), PlannedAction::Noop);
        let (head, plan_part, tail) =
            planner.build_replan_prompt(&goal, &failed, "failed", &[], &ctx);
        let replan = format!("{head}{plan_part}{tail}");
        assert!(!replan.contains("WebSearch"), "{replan}");
        assert!(!planner
            .allowed_action_types(&ctx.agent_capabilities)
            .contains("WebSearch"));
        assert!(!planner
            .allowed_actions_description(&ctx.agent_capabilities)
            .contains("WebSearch"));
    }

    #[test]
    fn p0_002c5a_planning_prompt_without_workspace_invents_no_directory() {
        let planner = CognitivePlanner::new(Box::new(MockLlm("[]")));
        let goal = AgentGoal::new("list the files".into(), 5);
        let prompt = prompt_text(&planner, &goal, &make_context(vec!["llm.query"]));
        assert!(prompt.contains("WORKSPACE DIRECTORY: none"), "{prompt}");
        assert!(!prompt.contains("/home/user"));
        assert!(!prompt.contains("Current working directory contents"));
    }

    #[test]
    fn test_plan_goal_basic() {
        let llm = MockLlm(
            r#"[
                {"action": {"type": "LlmQuery", "prompt": "analyze code", "context": []}, "description": "analyze"},
                {"action": {"type": "MemoryStore", "key": "result", "value": "done", "memory_type": "episodic"}, "description": "store"}
            ]"#,
        );
        let planner = CognitivePlanner::new(Box::new(llm));
        let goal = AgentGoal::new("test goal".into(), 5);
        let ctx = make_context(vec!["llm.query"]);
        let steps = plan(&planner, &goal, &ctx).unwrap();
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0].action.action_type(), "llm_query");
        assert_eq!(steps[1].action.action_type(), "memory_store");
        assert_eq!(steps[0].status, StepStatus::Planned);
    }

    #[test]
    fn test_plan_rejects_unauthorized_action() {
        let llm = MockLlm(
            r#"[
                {"action": {"type": "FileWrite", "path": "/tmp/x", "content": "hack"}, "description": "write"}
            ]"#,
        );
        let planner = CognitivePlanner::new(Box::new(llm));
        let goal = AgentGoal::new("test".into(), 5);
        // Only has fs.read, not fs.write
        let ctx = make_context(vec!["fs.read"]);
        let result = plan(&planner, &goal, &ctx);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("fs.write"));
    }

    #[test]
    fn test_plan_allows_always_allowed_actions() {
        let llm = MockLlm(
            r#"[
                {"action": {"type": "MemoryStore", "key": "k", "value": "v", "memory_type": "episodic"}, "description": "store"},
                {"action": {"type": "MemoryRecall", "query": "q", "memory_type": null}, "description": "recall"},
                {"action": {"type": "HitlRequest", "question": "ok?", "options": ["yes"]}, "description": "ask"},
                {"action": {"type": "Noop"}, "description": "wait"}
            ]"#,
        );
        let planner = CognitivePlanner::new(Box::new(llm));
        let goal = AgentGoal::new("test".into(), 5);
        // No capabilities at all
        let ctx = make_context(vec![]);
        let steps = plan(&planner, &goal, &ctx).unwrap();
        assert_eq!(steps.len(), 4);
    }

    #[test]
    fn test_replan_after_failure() {
        let llm = MockLlm(
            r#"[
                {"action": {"type": "LlmQuery", "prompt": "retry with different approach", "context": ["error context"]}, "description": "retry"}
            ]"#,
        );
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
        let mut unit = unit();
        let previous_plan = unit.produce("the previous plan");
        let steps = planner
            .replan_after_failure(
                &mut unit,
                &goal,
                &failed,
                "timeout",
                &[],
                &ctx,
                &previous_plan,
            )
            .unwrap()
            .steps;
        assert_eq!(steps.len(), 1);
    }

    #[test]
    fn test_extract_json_from_markdown() {
        let text = r#"Here is the plan:

```json
[{"action": {"type": "Noop"}, "description": "wait"}]
```

Done."#;
        let llm = MockLlm(text);
        let planner = CognitivePlanner::new(Box::new(llm));
        let goal = AgentGoal::new("test".into(), 5);
        let ctx = make_context(vec![]);
        let steps = plan(&planner, &goal, &ctx).unwrap();
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
        let llm = MockLlm("not json at all");
        let planner = CognitivePlanner::new(Box::new(llm));
        let goal = AgentGoal::new("test".into(), 5);
        let ctx = make_context(vec![]);
        let steps = plan(&planner, &goal, &ctx).unwrap();
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].action.action_type(), "llm_query");
    }

    #[test]
    fn test_build_planning_prompt_contains_goal() {
        let llm = MockLlm("[]");
        let planner = CognitivePlanner::new(Box::new(llm));
        let goal = AgentGoal::new("build a widget".into(), 7);
        let ctx = make_context(vec!["llm.query", "fs.read"]);
        let prompt = prompt_text(&planner, &goal, &ctx);
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
        let llm = MockLlm(
            r#"[
                {"action": {"type": "ShellCommand", "command": "ls", "args": ["-l"], "args": ["-la"]}, "description": "inspect"}
            ]"#,
        );
        let planner = CognitivePlanner::new(Box::new(llm));
        let goal = AgentGoal::new("inspect workspace".into(), 5);
        let ctx = make_context(vec!["process.exec"]);

        let steps = plan(&planner, &goal, &ctx).unwrap();
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
        let llm = sequence(&[
            "```json\n[{\"action\": {\"type\": \"Noop\"}, \"description\": \"wait\",}]\n```",
            r#"[{"action": {"type": "Noop"}, "description": "wait"}]"#,
        ]);
        let planner = CognitivePlanner::new(Box::new(llm));
        let goal = AgentGoal::new("recover from invalid json".into(), 5);
        let ctx = make_context(vec![]);

        let steps = plan(&planner, &goal, &ctx).unwrap();
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].action.action_type(), "noop");
    }

    #[test]
    fn test_double_invalid_json_returns_llm_query_step() {
        let llm = sequence(&["not valid json", "still not valid json"]);
        let planner = CognitivePlanner::new(Box::new(llm));
        let goal = AgentGoal::new("gracefully degrade".into(), 5);
        let ctx = make_context(vec![]);

        let steps = plan(&planner, &goal, &ctx).unwrap();
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].action.action_type(), "llm_query");
        // Fallback step should NOT pre-populate result (that caused hallucination).
        // The prompt sends the original goal so the LLM can still give a useful answer.
        assert!(steps[0].result.is_none());
        if let PlannedAction::LlmQuery { ref prompt, .. } = steps[0].action {
            assert!(
                prompt.contains("gracefully degrade"),
                "fallback prompt must contain the original goal, got: {prompt}"
            );
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

    // ── escape_control_chars_in_strings tests ──

    #[test]
    fn test_sanitizer_passes_valid_json_unchanged() {
        let input = r#"[{"action": {"type": "ShellCommand", "command": "ls -la"}}]"#;
        assert_eq!(escape_control_chars_in_strings(input), input);
    }

    #[test]
    fn test_sanitizer_escapes_literal_newline_in_string() {
        // Build JSON with a literal 0x0A byte inside a string value
        let input = format!(r#"{{"a":"x{}y"}}"#, '\n');
        let result = escape_control_chars_in_strings(&input);
        assert!(
            result.contains(r#""x\ny""#),
            "expected escaped newline, got: {result}"
        );
        // The result must now be valid JSON
        let parsed: Result<serde_json::Value, _> = serde_json::from_str(&result);
        assert!(parsed.is_ok(), "sanitized output should be valid JSON");
    }

    #[test]
    fn test_sanitizer_does_not_touch_literal_newline_outside_string() {
        let input = "[\n  {\"a\": \"b\"}\n]";
        assert_eq!(escape_control_chars_in_strings(input), input);
    }

    #[test]
    fn test_sanitizer_handles_escaped_quote() {
        // {"msg": "he said \"hi\""} with a literal newline after the last escaped quote
        let input = format!(r#"{{"msg": "he said \"hi\"{}end"}}"#, '\n');
        let result = escape_control_chars_in_strings(&input);
        // The escaped quotes must be preserved
        assert!(
            result.contains(r#"\"hi\""#),
            "escaped quotes lost: {result}"
        );
        // The literal newline must be escaped
        assert!(
            result.contains(r#"\n"#),
            "literal newline not escaped: {result}"
        );
        let parsed: Result<serde_json::Value, _> = serde_json::from_str(&result);
        assert!(parsed.is_ok(), "sanitized output should be valid JSON");
    }

    #[test]
    fn test_parse_plan_response_recovers_gemma_style_response() {
        // Simulate Gemma4 output: correct JSON shape but with a literal newline
        // inside the "command" string value (the real bug).
        let response = format!(
            r#"[
  {{"action": {{"type": "ShellCommand", "command": "cd /home/nexus{}&& ls -la", "args": []}}, "description": "List files"}},
  {{"action": {{"type": "LlmQuery", "prompt": "Summarize the file list", "context": ["output"]}}, "description": "Summarize"}}
]"#,
            '\n'
        );

        let llm = MockLlm(&response.clone());
        let planner = CognitivePlanner::new(Box::new(llm));
        let goal = AgentGoal::new("list files".into(), 5);
        let ctx = make_context(vec!["process.exec", "llm.query"]);
        let steps = plan(&planner, &goal, &ctx).unwrap();
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0].action.action_type(), "shell_command");
        assert_eq!(steps[1].action.action_type(), "llm_query");
    }

    // ── llm_query_fallback_step tests ──

    #[test]
    fn test_fallback_step_uses_original_goal() {
        let llm = MockLlm("[]");
        let planner = CognitivePlanner::new(Box::new(llm));
        let step = planner.llm_query_fallback_step(
            "goal-1",
            "list the files in the current directory",
            "some error".to_string(),
        );
        if let PlannedAction::LlmQuery { prompt, .. } = &step.action {
            assert!(
                prompt.contains("list the files in the current directory"),
                "fallback prompt must contain the original goal, got: {prompt}"
            );
            assert!(
                !prompt.contains("IMPORTANT"),
                "fallback prompt must not contain meta-instructions, got: {prompt}"
            );
            assert!(
                !prompt.contains("hallucinate"),
                "fallback prompt must not contain anti-hallucination lecture, got: {prompt}"
            );
            assert!(
                !prompt.contains("unable to create"),
                "fallback prompt must not mention planning failure, got: {prompt}"
            );
        } else {
            panic!("expected LlmQuery action");
        }
    }

    #[test]
    fn test_strip_think_tags_unclosed_at_start_returns_empty() {
        // Regression: if <think> starts at position 0 and never closes,
        // the entire response is truncated to "". This is the exact scenario
        // that causes the planner to see 0-char responses from reasoning models.
        let input = "<think>I am thinking but never close";
        let result = strip_think_tags(input);
        assert_eq!(
            result, "",
            "unclosed <think> at position 0 must return empty string"
        );
    }

    #[test]
    fn test_strip_think_tags_closed_returns_after_think() {
        let input = "<think>reasoning</think>[{\"action\":\"x\"}]";
        let result = strip_think_tags(input);
        assert_eq!(result, "[{\"action\":\"x\"}]");
    }

    #[test]
    fn test_strip_think_tags_no_think_passes_through() {
        let input = "[{\"action\":\"x\"}]";
        let result = strip_think_tags(input);
        assert_eq!(result, input);
    }

    #[test]
    fn test_planning_prompt_starts_with_critical_no_think() {
        let llm = MockLlm("[]");
        let planner = CognitivePlanner::new(Box::new(llm));
        let goal = AgentGoal::new("test goal".into(), 5);
        let ctx = make_context(vec!["llm.query"]);
        let prompt = prompt_text(&planner, &goal, &ctx);
        assert!(
            prompt.starts_with("CRITICAL: Output ONLY a JSON array. Do NOT think out loud."),
            "planning prompt must start with CRITICAL no-think prefix, got: {}",
            &prompt[..80.min(prompt.len())]
        );
    }
}
