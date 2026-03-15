//! Cognitive planner — translates goals into executable step sequences via LLM.

use super::types::{AgentGoal, AgentStep, PlannedAction, PlanningContext};
use crate::capabilities::has_capability;
use crate::errors::AgentError;
use serde::Deserialize;

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
        let response = self.llm.plan_query(&prompt)?;
        let steps = self.parse_plan_response(&response, &goal.id, &context.agent_capabilities)?;
        Ok(steps)
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
        let response = self.llm.plan_query(&prompt)?;
        let steps = self.parse_plan_response(&response, &goal.id, &context.agent_capabilities)?;
        Ok(steps)
    }

    fn build_planning_prompt(&self, goal: &AgentGoal, context: &PlanningContext) -> String {
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

        format!(
            r#"You are a planning agent. Create a step-by-step plan to achieve the goal below.

GOAL: {goal_desc}
PRIORITY: {priority}

AGENT CAPABILITIES: [{capabilities}]
AVAILABLE FUEL: {fuel}
AUTONOMY LEVEL: L{autonomy}

RELEVANT MEMORIES:
- {memories}

PREVIOUS OUTCOMES:
- {outcomes}

ALLOWED ACTIONS (you MUST only use these):
{allowed_actions}

Respond with a JSON array of steps. Each step has:
- "action": an object with "type" field matching one of the allowed actions above, plus action-specific fields
- "description": a short human-readable description of what the step does

Example response:
[
  {{"action": {{"type": "LlmQuery", "prompt": "...", "context": []}}, "description": "Ask LLM for analysis"}},
  {{"action": {{"type": "MemoryStore", "key": "result", "value": "...", "memory_type": "episodic"}}, "description": "Store result"}}
]

Respond ONLY with the JSON array, no other text."#,
            goal_desc = goal.description,
            priority = goal.priority,
            capabilities = capabilities_str,
            fuel = context.available_fuel,
            autonomy = context.autonomy_level,
            memories = memories_str,
            outcomes = outcomes_str,
            allowed_actions = allowed_actions,
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
            r#"You are a planning agent. A step in your plan failed. Create an adapted plan.

GOAL: {goal_desc}
FAILED STEP: {failed_action} (attempt {attempt}/{max})
ERROR: {error}

REMAINING ORIGINAL STEPS:
{remaining}

AGENT CAPABILITIES: [{capabilities}]
AVAILABLE FUEL: {fuel}

ALLOWED ACTIONS (you MUST only use these):
{allowed_actions}

Create an adapted JSON plan that works around the failure. Respond ONLY with a JSON array."#,
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

    fn allowed_actions_description(&self, capabilities: &[String]) -> String {
        let has =
            |required: &str| has_capability(capabilities.iter().map(String::as_str), required);

        // Always-allowed actions
        let mut actions = vec![
            r#"- MemoryStore: {"type": "MemoryStore", "key": "...", "value": "...", "memory_type": "episodic|semantic|procedural"}"#.to_string(),
            r#"- MemoryRecall: {"type": "MemoryRecall", "query": "...", "memory_type": null}"#.to_string(),
            r#"- HitlRequest: {"type": "HitlRequest", "question": "...", "options": ["yes","no"]}"#.to_string(),
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
        }
        if has("web.search") {
            actions.push(r#"- WebSearch: {"type": "WebSearch", "query": "..."}"#.to_string());
        }
        if has("web.read") {
            actions.push(r#"- WebFetch: {"type": "WebFetch", "url": "..."}"#.to_string());
        }
        if has("mcp.call") {
            actions.push(
                r#"- ApiCall: {"type": "ApiCall", "method": "GET", "url": "...", "body": null}"#
                    .to_string(),
            );
        }
        if capabilities.iter().any(|c| c == "agent.message") {
            actions.push(r#"- AgentMessage: {"type": "AgentMessage", "target_agent": "...", "message": "..."}"#.to_string());
        }

        actions.join("\n")
    }

    fn parse_plan_response(
        &self,
        response: &str,
        goal_id: &str,
        capabilities: &[String],
    ) -> Result<Vec<AgentStep>, AgentError> {
        // Extract JSON array from response (handle markdown code blocks)
        let json_str = extract_json_array(response).ok_or_else(|| {
            AgentError::SupervisorError(format!(
                "planner response did not contain a valid JSON array: {response}"
            ))
        })?;

        let raw_steps: Vec<RawStep> = serde_json::from_str(&json_str)
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
}

/// Raw step from LLM JSON response.
#[derive(Debug, Deserialize)]
struct RawStep {
    action: PlannedAction,
    #[allow(dead_code)]
    description: Option<String>,
}

/// Extract a JSON array from text that may contain markdown fences.
fn extract_json_array(text: &str) -> Option<String> {
    let trimmed = text.trim();

    // Try direct parse first
    if trimmed.starts_with('[') {
        if let Some(end) = find_matching_bracket(trimmed) {
            return Some(trimmed[..=end].to_string());
        }
    }

    // Try extracting from markdown code block
    if let Some(start) = trimmed.find("```json") {
        let after = &trimmed[start + 7..];
        if let Some(end) = after.find("```") {
            let inner = after[..end].trim();
            if inner.starts_with('[') {
                return Some(inner.to_string());
            }
        }
    }

    // Try extracting from generic code block
    if let Some(start) = trimmed.find("```") {
        let after = &trimmed[start + 3..];
        // Skip optional language tag on same line
        let after = if let Some(nl) = after.find('\n') {
            &after[nl + 1..]
        } else {
            after
        };
        if let Some(end) = after.find("```") {
            let inner = after[..end].trim();
            if inner.starts_with('[') {
                return Some(inner.to_string());
            }
        }
    }

    None
}

fn find_matching_bracket(s: &str) -> Option<usize> {
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escape = false;
    for (i, ch) in s.char_indices() {
        if escape {
            escape = false;
            continue;
        }
        if ch == '\\' && in_string {
            escape = true;
            continue;
        }
        if ch == '"' {
            in_string = !in_string;
            continue;
        }
        if in_string {
            continue;
        }
        if ch == '[' {
            depth += 1;
        } else if ch == ']' {
            depth -= 1;
            if depth == 0 {
                return Some(i);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::super::types::StepStatus;
    use super::*;

    struct MockLlm {
        response: String,
    }

    impl PlannerLlm for MockLlm {
        fn plan_query(&self, _prompt: &str) -> Result<String, AgentError> {
            Ok(self.response.clone())
        }
    }

    fn make_context(caps: Vec<&str>) -> PlanningContext {
        PlanningContext {
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
        assert!(extract_json_array("[1,2,3]").is_some());
        assert!(extract_json_array("no json here").is_none());
        assert!(extract_json_array("```json\n[1]\n```").is_some());
        assert!(extract_json_array("```\n[1]\n```").is_some());
    }

    #[test]
    fn test_invalid_json_returns_error() {
        let llm = MockLlm {
            response: "not json at all".to_string(),
        };
        let planner = CognitivePlanner::new(Box::new(llm));
        let goal = AgentGoal::new("test".into(), 5);
        let ctx = make_context(vec![]);
        assert!(planner.plan_goal(&goal, &ctx).is_err());
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
        // Should NOT contain FileWrite since fs.write not in capabilities
        assert!(!prompt.contains("FileWrite"));
    }
}
