//! Core types for the cognitive agent runtime.

pub use crate::action_types::BrowserAction;
pub use crate::action_types::PlannedAction;
use serde::{Deserialize, Serialize};

/// Phase of the cognitive loop an agent is currently in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CognitivePhase {
    Perceive,
    Reason,
    Plan,
    Act,
    Reflect,
    Learn,
    Idle,
    Blocked,
}

impl std::fmt::Display for CognitivePhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            CognitivePhase::Perceive => "perceive",
            CognitivePhase::Reason => "reason",
            CognitivePhase::Plan => "plan",
            CognitivePhase::Act => "act",
            CognitivePhase::Reflect => "reflect",
            CognitivePhase::Learn => "learn",
            CognitivePhase::Idle => "idle",
            CognitivePhase::Blocked => "blocked",
        };
        write!(f, "{s}")
    }
}

/// Status of a goal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GoalStatus {
    Pending,
    Active,
    Completed,
    Failed,
    Blocked,
}

/// A high-level goal assigned to an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentGoal {
    pub id: String,
    pub description: String,
    /// Pristine user-provided goal text (before manifest concatenation).
    pub user_goal: String,
    pub priority: u8,
    pub deadline: Option<String>,
    pub parent_goal: Option<String>,
    pub status: GoalStatus,
    /// Per-goal LLM model override (BUG-DROP). When `Some`, forces LLM dispatch
    /// to use this model for the lifetime of the goal, bypassing the agent's
    /// manifest `llm_model` and the gateway auto-resolver. When `None`, the
    /// existing manifest/auto-resolver fallback applies unchanged. Sentinels
    /// like "mock" / "auto" / "" should be normalized to `None` by callers
    /// before setting this field.
    #[serde(default)]
    pub model_override: Option<String>,
}

impl AgentGoal {
    pub fn new(description: String, priority: u8) -> Self {
        let user_goal = description.clone();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            description,
            user_goal,
            priority: priority.clamp(1, 10),
            deadline: None,
            parent_goal: None,
            status: GoalStatus::Pending,
            model_override: None,
        }
    }
}

/// Status of a single step in a plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepStatus {
    Planned,
    Executing,
    Succeeded,
    Failed,
    Skipped,
}

/// A single step in an agent's plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStep {
    pub id: String,
    pub goal_id: String,
    pub action: PlannedAction,
    pub status: StepStatus,
    pub result: Option<String>,
    pub fuel_cost: f64,
    pub attempts: u32,
    pub max_retries: u32,
}

impl AgentStep {
    pub fn new(goal_id: String, action: PlannedAction) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            goal_id,
            action,
            status: StepStatus::Planned,
            result: None,
            fuel_cost: 0.0,
            attempts: 0,
            max_retries: 2,
        }
    }
}

/// Result of one cognitive cycle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CycleResult {
    pub phase: CognitivePhase,
    pub steps_executed: u32,
    pub fuel_consumed: f64,
    pub should_continue: bool,
    pub blocked_reason: Option<String>,
    /// Whether the cycle produced a real result. `false` when the cycle
    /// hit a silent-failure path (empty LLM response, all steps empty,
    /// planner fallback that produced nothing).
    #[serde(default = "default_cycle_success")]
    pub success: bool,
    /// Human-readable reason when `success == false`. `None` on the happy path.
    #[serde(default)]
    pub failure_reason: Option<String>,
}

fn default_cycle_success() -> bool {
    true
}

/// Configuration for the cognitive loop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopConfig {
    pub max_cycles_per_goal: u32,
    pub max_consecutive_failures: u32,
    pub cycle_delay_ms: u64,
    pub fuel_reserve_threshold: f64,
    pub reflection_interval: u32,
}

impl Default for LoopConfig {
    fn default() -> Self {
        Self {
            max_cycles_per_goal: 50,
            max_consecutive_failures: 3,
            cycle_delay_ms: 500,
            fuel_reserve_threshold: 0.1,
            reflection_interval: 5,
        }
    }
}

/// Context provided to the planner for generating steps. It carries no
/// retained record: memories and previous outcomes reach a planning prompt
/// only as verbatim items (`planner::PlanningRecall`, P3-HM-CONTRACT-02
/// §11.2, §11.3).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanningContext {
    pub agent_name: Option<String>,
    pub agent_description: Option<String>,
    pub agent_capabilities: Vec<String>,
    pub available_fuel: f64,
    pub working_directory: Option<String>,
    /// Top-level contents of `working_directory` at planning time.
    /// Helps the planner LLM disambiguate user phrases like "src/" or
    /// "config/" against the actual repo layout. `None` when the cwd
    /// cannot be read or is empty.
    #[serde(default)]
    pub directory_listing: Option<String>,
    pub autonomy_level: u8,
}

/// Response from the cognitive status query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CognitiveStatusResponse {
    pub phase: CognitivePhase,
    pub active_goal: Option<AgentGoal>,
    pub steps_completed: u32,
    pub steps_total: u32,
    pub fuel_remaining: f64,
    pub cycle_count: u32,
    /// UNIX epoch seconds when the goal was assigned (for elapsed-time display).
    pub started_at_secs: u64,
    /// Raw output of the most recently succeeded step (for UI display).
    pub last_step_result: Option<String>,
}

/// Events emitted during cognitive loop execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event_type")]
pub enum CognitiveEvent {
    PhaseChange {
        agent_id: String,
        phase: CognitivePhase,
        goal_id: String,
        timestamp: u64,
    },
    StepExecuted {
        agent_id: String,
        step_id: String,
        action_type: String,
        status: StepStatus,
        result_preview: Option<String>,
        fuel_cost: f64,
    },
    GoalCompleted {
        agent_id: String,
        goal_id: String,
        success: bool,
        steps_total: u32,
        fuel_consumed: f64,
    },
    AgentBlocked {
        agent_id: String,
        reason: String,
        consent_id: Option<String>,
    },
    AgentCooldown {
        agent_id: String,
        cycles_completed: u32,
    },
    /// Notification from an agent to the user (sent via SendNotification action).
    AgentNotification {
        agent_id: String,
        title: String,
        body: String,
        /// "info", "warning", "error", or "success"
        level: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cognitive_phase_display() {
        assert_eq!(CognitivePhase::Perceive.to_string(), "perceive");
        assert_eq!(CognitivePhase::Blocked.to_string(), "blocked");
    }

    #[test]
    fn test_agent_goal_new() {
        let goal = AgentGoal::new("test goal".into(), 5);
        assert!(!goal.id.is_empty());
        assert_eq!(goal.priority, 5);
        assert_eq!(goal.status, GoalStatus::Pending);
    }

    #[test]
    fn test_agent_goal_priority_clamping() {
        let goal = AgentGoal::new("test".into(), 15);
        assert_eq!(goal.priority, 10);
        let goal = AgentGoal::new("test".into(), 0);
        assert_eq!(goal.priority, 1);
    }

    #[test]
    fn test_agent_step_new() {
        let step = AgentStep::new("g1".into(), PlannedAction::Noop);
        assert_eq!(step.status, StepStatus::Planned);
        assert_eq!(step.attempts, 0);
        assert_eq!(step.max_retries, 2);
    }

    #[test]
    fn test_planned_action_capabilities() {
        assert_eq!(
            PlannedAction::LlmQuery {
                prompt: "hi".into(),
                context: vec![]
            }
            .required_capabilities(),
            vec!["llm.query"]
        );
        assert_eq!(
            PlannedAction::CaptureScreen { region: None }.required_capabilities(),
            vec!["screen.capture"]
        );
        assert_eq!(
            PlannedAction::KeyboardShortcut {
                keys: vec!["Ctrl".into(), "C".into()]
            }
            .required_capabilities(),
            vec!["input.keyboard"]
        );
        assert_eq!(
            PlannedAction::FileWrite {
                path: "/tmp/f".into(),
                content: "x".into()
            }
            .required_capabilities(),
            vec!["fs.write"]
        );
        assert!(PlannedAction::Noop.required_capabilities().is_empty());
        assert!(PlannedAction::HitlRequest {
            question: "ok?".into(),
            options: vec![]
        }
        .required_capabilities()
        .is_empty());
    }

    #[test]
    fn test_planned_action_type() {
        assert_eq!(
            PlannedAction::ShellCommand {
                command: "ls".into(),
                args: vec![]
            }
            .action_type(),
            "shell_command"
        );
        assert_eq!(PlannedAction::Noop.action_type(), "noop");
        assert_eq!(
            PlannedAction::ComputerAction {
                description: "Open Firefox".into(),
                max_steps: 20
            }
            .action_type(),
            "computer_action"
        );
        assert_eq!(
            PlannedAction::SelectAlgorithm {
                algorithm: "world_model".into(),
                config_json: "{}".into()
            }
            .action_type(),
            "select_algorithm"
        );
    }

    #[test]
    fn test_loop_config_defaults() {
        let cfg = LoopConfig::default();
        assert_eq!(cfg.max_cycles_per_goal, 50);
        assert_eq!(cfg.max_consecutive_failures, 3);
        assert_eq!(cfg.cycle_delay_ms, 500);
        assert!((cfg.fuel_reserve_threshold - 0.1).abs() < f64::EPSILON);
        assert_eq!(cfg.reflection_interval, 5);
    }

    #[test]
    fn test_cycle_result_serde() {
        let result = CycleResult {
            phase: CognitivePhase::Act,
            steps_executed: 3,
            fuel_consumed: 12.5,
            should_continue: true,
            blocked_reason: None,
            success: true,
            failure_reason: None,
        };
        let json = serde_json::to_string(&result).unwrap();
        let back: CycleResult = serde_json::from_str(&json).unwrap();
        assert_eq!(back.phase, CognitivePhase::Act);
        assert_eq!(back.steps_executed, 3);
    }

    #[test]
    fn test_cognitive_event_serde() {
        let event = CognitiveEvent::PhaseChange {
            agent_id: "a1".into(),
            phase: CognitivePhase::Plan,
            goal_id: "g1".into(),
            timestamp: 12345,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"event_type\":\"PhaseChange\""));
    }

    #[test]
    fn test_planned_action_serde_roundtrip() {
        let action = PlannedAction::LlmQuery {
            prompt: "hello".into(),
            context: vec!["ctx1".into()],
        };
        let json = serde_json::to_string(&action).unwrap();
        let back: PlannedAction = serde_json::from_str(&json).unwrap();
        if let PlannedAction::LlmQuery { prompt, context } = back {
            assert_eq!(prompt, "hello");
            assert_eq!(context, vec!["ctx1"]);
        } else {
            panic!("wrong variant");
        }
    }

    #[test]
    fn test_planning_context_serde() {
        let ctx = PlanningContext {
            agent_name: Some("test-agent".into()),
            agent_description: Some("test agent description".into()),
            agent_capabilities: vec!["llm.query".into()],
            available_fuel: 100.0,
            working_directory: Some("/tmp".into()),
            directory_listing: None,
            autonomy_level: 2,
        };
        let json = serde_json::to_string(&ctx).unwrap();
        let back: PlanningContext = serde_json::from_str(&json).unwrap();
        assert_eq!(back.autonomy_level, 2);
    }

    #[test]
    fn test_cognitive_status_response() {
        let status = CognitiveStatusResponse {
            phase: CognitivePhase::Idle,
            active_goal: None,
            steps_completed: 0,
            steps_total: 0,
            fuel_remaining: 1000.0,
            cycle_count: 0,
            started_at_secs: 0,
            last_step_result: None,
        };
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("\"phase\":\"Idle\""));
    }

    // ── string_or_vec deserializer tests ──

    #[test]
    fn test_llm_query_context_as_array() {
        let json = r#"{"type": "LlmQuery", "prompt": "hello", "context": ["a", "b"]}"#;
        let action: PlannedAction = serde_json::from_str(json).unwrap();
        if let PlannedAction::LlmQuery { context, .. } = action {
            assert_eq!(context, vec!["a", "b"]);
        } else {
            panic!("expected LlmQuery");
        }
    }

    #[test]
    fn test_llm_query_context_as_string() {
        let json = r#"{"type": "LlmQuery", "prompt": "hello", "context": "previous output"}"#;
        let action: PlannedAction = serde_json::from_str(json).unwrap();
        if let PlannedAction::LlmQuery { context, .. } = action {
            assert_eq!(context, vec!["previous output"]);
        } else {
            panic!("expected LlmQuery");
        }
    }

    #[test]
    fn test_llm_query_context_as_empty_string() {
        let json = r#"{"type": "LlmQuery", "prompt": "hello", "context": ""}"#;
        let action: PlannedAction = serde_json::from_str(json).unwrap();
        if let PlannedAction::LlmQuery { context, .. } = action {
            assert!(context.is_empty());
        } else {
            panic!("expected LlmQuery");
        }
    }

    #[test]
    fn test_llm_query_context_as_null() {
        let json = r#"{"type": "LlmQuery", "prompt": "hello", "context": null}"#;
        let action: PlannedAction = serde_json::from_str(json).unwrap();
        if let PlannedAction::LlmQuery { context, .. } = action {
            assert!(context.is_empty());
        } else {
            panic!("expected LlmQuery");
        }
    }

    #[test]
    fn test_llm_query_context_missing() {
        let json = r#"{"type": "LlmQuery", "prompt": "hello"}"#;
        let action: PlannedAction = serde_json::from_str(json).unwrap();
        if let PlannedAction::LlmQuery { context, .. } = action {
            assert!(context.is_empty());
        } else {
            panic!("expected LlmQuery");
        }
    }

    #[test]
    fn test_shell_command_args_as_string() {
        let json = r#"{"type": "ShellCommand", "command": "free", "args": "-m"}"#;
        let action: PlannedAction = serde_json::from_str(json).unwrap();
        if let PlannedAction::ShellCommand { args, .. } = action {
            assert_eq!(args, vec!["-m"]);
        } else {
            panic!("expected ShellCommand");
        }
    }

    #[test]
    fn test_shell_command_args_missing() {
        let json = r#"{"type": "ShellCommand", "command": "ls"}"#;
        let action: PlannedAction = serde_json::from_str(json).unwrap();
        if let PlannedAction::ShellCommand { args, .. } = action {
            assert!(args.is_empty());
        } else {
            panic!("expected ShellCommand");
        }
    }
}
