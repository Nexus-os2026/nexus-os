//! Shared action data and codecs, extracted unchanged from the legacy runtime.

use crate::screen_region::ScreenRegion;
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
