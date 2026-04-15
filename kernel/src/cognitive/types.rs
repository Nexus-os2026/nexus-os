//! Core types for the cognitive agent runtime.

use crate::computer_control::ScreenRegion;
use serde::{Deserialize, Serialize};

/// Deserialize a `Vec<String>` that tolerates LLM output quirks:
/// - `["a","b"]` → normal array
/// - `"a"` → wraps into `["a"]`
/// - `""` or `null` → empty vec
/// - `123` or other scalars → `["123"]`
pub(crate) mod string_or_vec {
    use serde::{Deserialize, Deserializer};

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        Ok(match value {
            serde_json::Value::Array(arr) => arr
                .into_iter()
                .filter_map(|v| match v {
                    serde_json::Value::String(s) => Some(s),
                    serde_json::Value::Null => None,
                    other => Some(other.to_string()),
                })
                .collect(),
            serde_json::Value::String(s) if s.is_empty() => vec![],
            serde_json::Value::String(s) => vec![s],
            serde_json::Value::Null => vec![],
            other => vec![other.to_string()],
        })
    }
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BrowserAction {
    Navigate {
        url: String,
    },
    Click {
        selector: String,
    },
    Fill {
        selector: String,
        text: String,
    },
    Press {
        selector: String,
        key: String,
    },
    WaitFor {
        selector: Option<String>,
        timeout_ms: Option<u64>,
    },
    ExtractText {
        selector: String,
    },
}

/// An action the agent can plan to execute.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PlannedAction {
    LlmQuery {
        prompt: String,
        #[serde(default, deserialize_with = "string_or_vec::deserialize")]
        context: Vec<String>,
    },
    FileRead {
        path: String,
    },
    FileWrite {
        path: String,
        content: String,
    },
    ShellCommand {
        command: String,
        #[serde(default, deserialize_with = "string_or_vec::deserialize")]
        args: Vec<String>,
    },
    DockerCommand {
        subcommand: String,
        #[serde(default, deserialize_with = "string_or_vec::deserialize")]
        args: Vec<String>,
    },
    WebSearch {
        query: String,
    },
    WebFetch {
        url: String,
    },
    ApiCall {
        method: String,
        url: String,
        body: Option<String>,
        /// Optional HTTP headers as key-value pairs (e.g. `{"Authorization": "Bearer xxx"}`).
        #[serde(default)]
        headers: Option<std::collections::HashMap<String, String>>,
    },
    ImageGenerate {
        prompt: String,
        output_path: String,
        provider: Option<String>,
        model: Option<String>,
        size: Option<String>,
    },
    TextToSpeech {
        text: String,
        output_path: String,
        provider: Option<String>,
        voice: Option<String>,
        model: Option<String>,
    },
    KnowledgeGraphUpdate {
        #[serde(default, deserialize_with = "string_or_vec::deserialize")]
        entities: Vec<String>,
        #[serde(default, deserialize_with = "string_or_vec::deserialize")]
        relationships: Vec<String>,
    },
    KnowledgeGraphQuery {
        query: String,
    },
    BrowserAutomate {
        start_url: String,
        actions: Vec<BrowserAction>,
        screenshot_dir: Option<String>,
    },
    CaptureScreen {
        region: Option<ScreenRegion>,
    },
    CaptureWindow {
        window_title: String,
    },
    AnalyzeScreen {
        query: String,
    },
    MouseMove {
        x: u32,
        y: u32,
    },
    MouseClick {
        x: u32,
        y: u32,
        button: String,
    },
    MouseDoubleClick {
        x: u32,
        y: u32,
    },
    MouseDrag {
        from_x: u32,
        from_y: u32,
        to_x: u32,
        to_y: u32,
    },
    KeyboardType {
        text: String,
    },
    KeyboardPress {
        key: String,
    },
    KeyboardShortcut {
        keys: Vec<String>,
    },
    ScrollWheel {
        direction: String,
        amount: u32,
    },
    ComputerAction {
        description: String,
        max_steps: u32,
    },
    AgentMessage {
        target_agent: String,
        message: String,
    },
    HitlRequest {
        question: String,
        #[serde(default, deserialize_with = "string_or_vec::deserialize")]
        options: Vec<String>,
    },
    MemoryStore {
        key: String,
        value: String,
        memory_type: String,
    },
    MemoryRecall {
        query: String,
        memory_type: Option<String>,
    },
    /// Send a notification to the user (displayed in the UI).
    SendNotification {
        title: String,
        body: String,
        /// "info", "warning", "error", or "success"
        level: String,
    },
    /// Execute code in a sandboxed environment. No network access, no filesystem
    /// access outside the agent workspace. Captures stdout/stderr.
    CodeExecute {
        /// "python3", "node", or "bash"
        language: String,
        /// The code to execute.
        code: String,
        /// Timeout in seconds (max 30, default 10).
        #[serde(default)]
        timeout_secs: Option<u32>,
    },
    Noop,
    // ── L4/L5 Self-Evolution & Governance Actions ──
    SelfModifyDescription {
        new_description: String,
    },
    SelfModifyStrategy {
        strategy_key: String,
        new_strategy: String,
    },
    CreateSubAgent {
        manifest_json: String,
    },
    DestroySubAgent {
        agent_id: String,
    },
    RunEvolutionTournament {
        variants: Vec<String>,
        task: String,
        rounds: u32,
    },
    ModifyGovernancePolicy {
        policy_key: String,
        policy_value: String,
    },
    AllocateEcosystemFuel {
        agent_id: String,
        amount: f64,
    },
    ModifyCognitiveParams {
        param_key: String,
        param_value: String,
    },
    SelectLlmProvider {
        phase: String,
        provider: String,
        model: String,
    },
    SelectAlgorithm {
        algorithm: String,
        config_json: String,
    },
    DesignAgentEcosystem {
        ecosystem_json: String,
    },
    RunCounterfactual {
        decision_id: String,
        alternatives: Vec<String>,
    },
    TemporalPlan {
        immediate: String,
        short_term: String,
        medium_term: String,
        long_term: String,
    },
    /// Delegate a task to an external agent via A2A protocol.
    A2aDelegation {
        agent_url: String,
        message: String,
    },
}

impl PlannedAction {
    /// The capability string(s) required for this action.
    pub fn required_capabilities(&self) -> Vec<&'static str> {
        match self {
            PlannedAction::LlmQuery { .. } => vec!["llm.query"],
            PlannedAction::FileRead { .. } => vec!["fs.read"],
            PlannedAction::FileWrite { .. } => vec!["fs.write"],
            PlannedAction::ShellCommand { .. } => vec!["process.exec"],
            PlannedAction::DockerCommand { .. } => vec!["docker_run"],
            PlannedAction::WebSearch { .. } => vec!["web.search"],
            PlannedAction::WebFetch { .. } => vec!["web.read"],
            PlannedAction::ApiCall { .. } => vec!["mcp.call"],
            PlannedAction::ImageGenerate { .. } => vec!["image.generate"],
            PlannedAction::TextToSpeech { .. } => vec!["tts.generate"],
            PlannedAction::KnowledgeGraphUpdate { .. }
            | PlannedAction::KnowledgeGraphQuery { .. } => vec!["knowledge.graph"],
            PlannedAction::BrowserAutomate { .. } => vec!["browser.automate"],
            PlannedAction::CaptureScreen { .. } | PlannedAction::CaptureWindow { .. } => {
                vec!["screen.capture"]
            }
            PlannedAction::AnalyzeScreen { .. } => vec!["screen.analyze"],
            PlannedAction::MouseMove { .. }
            | PlannedAction::MouseClick { .. }
            | PlannedAction::MouseDoubleClick { .. }
            | PlannedAction::MouseDrag { .. }
            | PlannedAction::ScrollWheel { .. } => vec!["input.mouse"],
            PlannedAction::KeyboardType { .. }
            | PlannedAction::KeyboardPress { .. }
            | PlannedAction::KeyboardShortcut { .. } => vec!["input.keyboard"],
            PlannedAction::ComputerAction { .. } => vec!["computer.use"],
            PlannedAction::AgentMessage { .. } => vec!["agent.message"],
            PlannedAction::HitlRequest { .. } => vec![], // always allowed
            PlannedAction::MemoryStore { .. } => vec![], // always allowed
            PlannedAction::MemoryRecall { .. } => vec![], // always allowed
            PlannedAction::SendNotification { .. } => vec![], // always allowed
            PlannedAction::CodeExecute { .. } => vec!["process.exec"],
            PlannedAction::Noop => vec![],
            PlannedAction::SelfModifyDescription { .. } => vec!["self.modify"],
            PlannedAction::SelfModifyStrategy { .. } => vec!["self.modify"],
            PlannedAction::CreateSubAgent { .. } => vec!["self.modify"],
            PlannedAction::DestroySubAgent { .. } => vec!["self.modify"],
            PlannedAction::RunEvolutionTournament { .. } => vec!["self.modify"],
            PlannedAction::ModifyGovernancePolicy { .. } => vec!["self.modify"],
            PlannedAction::AllocateEcosystemFuel { .. } => vec!["self.modify"],
            PlannedAction::ModifyCognitiveParams { .. } => vec!["self.modify"],
            PlannedAction::SelectLlmProvider { .. } => vec!["self.modify"],
            PlannedAction::SelectAlgorithm { .. } => vec!["self.modify"],
            PlannedAction::DesignAgentEcosystem { .. } => vec!["self.modify"],
            PlannedAction::RunCounterfactual { .. } => vec!["self.modify"],
            PlannedAction::TemporalPlan { .. } => vec!["self.modify"],
            PlannedAction::A2aDelegation { .. } => vec!["a2a.delegate"],
        }
    }

    /// Short type label for audit/display.
    pub fn action_type(&self) -> &'static str {
        match self {
            PlannedAction::LlmQuery { .. } => "llm_query",
            PlannedAction::FileRead { .. } => "file_read",
            PlannedAction::FileWrite { .. } => "file_write",
            PlannedAction::ShellCommand { .. } => "shell_command",
            PlannedAction::DockerCommand { .. } => "docker_command",
            PlannedAction::WebSearch { .. } => "web_search",
            PlannedAction::WebFetch { .. } => "web_fetch",
            PlannedAction::ApiCall { .. } => "api_call",
            PlannedAction::ImageGenerate { .. } => "image_generate",
            PlannedAction::TextToSpeech { .. } => "text_to_speech",
            PlannedAction::KnowledgeGraphUpdate { .. } => "knowledge_graph_update",
            PlannedAction::KnowledgeGraphQuery { .. } => "knowledge_graph_query",
            PlannedAction::BrowserAutomate { .. } => "browser_automate",
            PlannedAction::CaptureScreen { .. } => "capture_screen",
            PlannedAction::CaptureWindow { .. } => "capture_window",
            PlannedAction::AnalyzeScreen { .. } => "analyze_screen",
            PlannedAction::MouseMove { .. } => "mouse_move",
            PlannedAction::MouseClick { .. } => "mouse_click",
            PlannedAction::MouseDoubleClick { .. } => "mouse_double_click",
            PlannedAction::MouseDrag { .. } => "mouse_drag",
            PlannedAction::KeyboardType { .. } => "keyboard_type",
            PlannedAction::KeyboardPress { .. } => "keyboard_press",
            PlannedAction::KeyboardShortcut { .. } => "keyboard_shortcut",
            PlannedAction::ScrollWheel { .. } => "scroll_wheel",
            PlannedAction::ComputerAction { .. } => "computer_action",
            PlannedAction::AgentMessage { .. } => "agent_message",
            PlannedAction::HitlRequest { .. } => "hitl_request",
            PlannedAction::MemoryStore { .. } => "memory_store",
            PlannedAction::MemoryRecall { .. } => "memory_recall",
            PlannedAction::SendNotification { .. } => "send_notification",
            PlannedAction::CodeExecute { .. } => "code_execute",
            PlannedAction::Noop => "noop",
            PlannedAction::SelfModifyDescription { .. } => "self_modify_description",
            PlannedAction::SelfModifyStrategy { .. } => "self_modify_strategy",
            PlannedAction::CreateSubAgent { .. } => "create_sub_agent",
            PlannedAction::DestroySubAgent { .. } => "destroy_sub_agent",
            PlannedAction::RunEvolutionTournament { .. } => "run_evolution_tournament",
            PlannedAction::ModifyGovernancePolicy { .. } => "modify_governance_policy",
            PlannedAction::AllocateEcosystemFuel { .. } => "allocate_ecosystem_fuel",
            PlannedAction::ModifyCognitiveParams { .. } => "modify_cognitive_params",
            PlannedAction::SelectLlmProvider { .. } => "select_llm_provider",
            PlannedAction::SelectAlgorithm { .. } => "select_algorithm",
            PlannedAction::DesignAgentEcosystem { .. } => "design_agent_ecosystem",
            PlannedAction::RunCounterfactual { .. } => "run_counterfactual",
            PlannedAction::TemporalPlan { .. } => "temporal_plan",
            PlannedAction::A2aDelegation { .. } => "a2a_delegation",
        }
    }
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

/// Context provided to the planner for generating steps.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanningContext {
    pub agent_name: Option<String>,
    pub agent_description: Option<String>,
    pub agent_capabilities: Vec<String>,
    pub available_fuel: f64,
    pub relevant_memories: Vec<String>,
    pub previous_outcomes: Vec<String>,
    pub working_directory: Option<String>,
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
            relevant_memories: vec![],
            previous_outcomes: vec![],
            working_directory: Some("/tmp".into()),
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
