use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::ComputerUseError;

/// Actions the agent can perform on the computer
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum AgentAction {
    /// Click at screen coordinates
    Click {
        x: u32,
        y: u32,
        #[serde(default = "default_button")]
        button: String,
    },
    /// Double-click at screen coordinates
    DoubleClick { x: u32, y: u32 },
    /// Type text using keyboard
    Type { text: String },
    /// Press a key or key combination
    KeyPress { key: String },
    /// Scroll at coordinates
    Scroll {
        x: u32,
        y: u32,
        direction: String,
        #[serde(default = "default_scroll_amount")]
        amount: u32,
    },
    /// Drag from one point to another
    Drag {
        start_x: u32,
        start_y: u32,
        end_x: u32,
        end_y: u32,
    },
    /// Wait/pause for a number of milliseconds
    Wait {
        #[serde(default = "default_wait_ms")]
        ms: u64,
    },
    /// Take a screenshot (explicit request)
    Screenshot,
    /// Signal task completion
    Done { summary: String },
}

fn default_button() -> String {
    "left".to_string()
}

fn default_scroll_amount() -> u32 {
    3
}

fn default_wait_ms() -> u64 {
    500
}

impl std::fmt::Display for AgentAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentAction::Click { x, y, button } => write!(f, "click({button}, {x}, {y})"),
            AgentAction::DoubleClick { x, y } => write!(f, "double_click({x}, {y})"),
            AgentAction::Type { text } => {
                let preview = if text.len() > 30 {
                    format!("{}...", &text[..30])
                } else {
                    text.clone()
                };
                write!(f, "type({preview:?})")
            }
            AgentAction::KeyPress { key } => write!(f, "key_press({key})"),
            AgentAction::Scroll {
                x,
                y,
                direction,
                amount,
            } => write!(f, "scroll({direction}, {amount}, {x}, {y})"),
            AgentAction::Drag {
                start_x,
                start_y,
                end_x,
                end_y,
            } => write!(f, "drag({start_x},{start_y} -> {end_x},{end_y})"),
            AgentAction::Wait { ms } => write!(f, "wait({ms}ms)"),
            AgentAction::Screenshot => write!(f, "screenshot"),
            AgentAction::Done { summary } => write!(f, "done({summary})"),
        }
    }
}

/// A plan of actions returned by the vision model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionPlan {
    /// What the model observes on screen
    pub observation: String,
    /// Reasoning about what to do next
    pub reasoning: String,
    /// Ordered list of actions to take
    pub actions: Vec<AgentAction>,
    /// Model's confidence in the plan (0.0 to 1.0)
    #[serde(default = "default_confidence")]
    pub confidence: f64,
}

fn default_confidence() -> f64 {
    0.5
}

impl ActionPlan {
    /// Parse an ActionPlan from JSON string
    pub fn from_json(json: &str) -> Result<Self, ComputerUseError> {
        // Try to extract JSON from markdown code blocks if present
        let cleaned = extract_json_block(json);
        serde_json::from_str::<ActionPlan>(cleaned).map_err(|e| {
            ComputerUseError::ActionPlanParseError(format!("Failed to parse action plan: {e}"))
        })
    }
}

/// Extract JSON from a markdown code block if present, otherwise return as-is
fn extract_json_block(s: &str) -> &str {
    let trimmed = s.trim();
    if let Some(rest) = trimmed.strip_prefix("```json") {
        if let Some(json) = rest.strip_suffix("```") {
            return json.trim();
        }
    }
    if let Some(rest) = trimmed.strip_prefix("```") {
        if let Some(json) = rest.strip_suffix("```") {
            return json.trim();
        }
    }
    trimmed
}

/// Result of executing a single agent action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionResult {
    /// The action that was executed
    pub action: String,
    /// Whether the action succeeded
    pub success: bool,
    /// Error message if failed
    pub error: Option<String>,
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// SHA-256 audit hash
    pub audit_hash: String,
    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl ActionResult {
    /// Create a successful action result
    pub fn success(action: &str, duration_ms: u64) -> Self {
        Self {
            action: action.to_string(),
            success: true,
            error: None,
            duration_ms,
            audit_hash: compute_audit_hash(action),
            timestamp: chrono::Utc::now(),
        }
    }

    /// Create a failed action result
    pub fn failure(action: &str, error: &str, duration_ms: u64) -> Self {
        Self {
            action: action.to_string(),
            success: false,
            error: Some(error.to_string()),
            duration_ms,
            audit_hash: compute_audit_hash(action),
            timestamp: chrono::Utc::now(),
        }
    }
}

/// Compute SHA-256 audit hash for an action
pub fn compute_audit_hash(description: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(description.as_bytes());
    hasher.update(chrono::Utc::now().to_rfc3339().as_bytes());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_action_serialize_click() {
        let action = AgentAction::Click {
            x: 100,
            y: 200,
            button: "left".to_string(),
        };
        let json = serde_json::to_string(&action).expect("serialize");
        assert!(json.contains("\"action\":\"click\""));
        assert!(json.contains("\"x\":100"));
        assert!(json.contains("\"y\":200"));
    }

    #[test]
    fn test_agent_action_serialize_type() {
        let action = AgentAction::Type {
            text: "hello world".to_string(),
        };
        let json = serde_json::to_string(&action).expect("serialize");
        assert!(json.contains("\"action\":\"type\""));
        assert!(json.contains("hello world"));
    }

    #[test]
    fn test_agent_action_serialize_done() {
        let action = AgentAction::Done {
            summary: "Task completed".to_string(),
        };
        let json = serde_json::to_string(&action).expect("serialize");
        assert!(json.contains("\"action\":\"done\""));
        assert!(json.contains("Task completed"));
    }

    #[test]
    fn test_action_plan_deserialize_valid() {
        let json = r#"{
            "observation": "I see a terminal window",
            "reasoning": "I need to click on it",
            "actions": [
                {"action": "click", "x": 500, "y": 300, "button": "left"}
            ],
            "confidence": 0.95
        }"#;
        let plan = ActionPlan::from_json(json).expect("parse");
        assert_eq!(plan.observation, "I see a terminal window");
        assert_eq!(plan.reasoning, "I need to click on it");
        assert_eq!(plan.actions.len(), 1);
        assert!((plan.confidence - 0.95).abs() < f64::EPSILON);
    }

    #[test]
    fn test_action_plan_deserialize_missing_fields() {
        // confidence should default to 0.5
        let json = r#"{
            "observation": "screen",
            "reasoning": "do thing",
            "actions": []
        }"#;
        let plan = ActionPlan::from_json(json).expect("parse");
        assert!((plan.confidence - 0.5).abs() < f64::EPSILON);
        assert!(plan.actions.is_empty());
    }

    #[test]
    fn test_action_plan_deserialize_invalid() {
        let json = "not valid json at all";
        let result = ActionPlan::from_json(json);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ComputerUseError::ActionPlanParseError(_)
        ));
    }

    #[test]
    fn test_action_result_creation() {
        let result = ActionResult::success("click(left, 100, 200)", 42);
        assert!(result.success);
        assert!(result.error.is_none());
        assert_eq!(result.duration_ms, 42);
        assert_eq!(result.audit_hash.len(), 64);

        let result = ActionResult::failure("click(left, 100, 200)", "out of bounds", 10);
        assert!(!result.success);
        assert_eq!(result.error.as_deref(), Some("out of bounds"));
    }

    #[test]
    fn test_action_display_click() {
        let action = AgentAction::Click {
            x: 10,
            y: 20,
            button: "left".to_string(),
        };
        assert_eq!(action.to_string(), "click(left, 10, 20)");
    }

    #[test]
    fn test_action_display_type_truncates() {
        let action = AgentAction::Type {
            text: "a".repeat(50),
        };
        let display = action.to_string();
        assert!(display.contains("..."));
    }

    #[test]
    fn test_action_display_done() {
        let action = AgentAction::Done {
            summary: "finished".to_string(),
        };
        assert_eq!(action.to_string(), "done(finished)");
    }

    #[test]
    fn test_extract_json_from_code_block() {
        let input = "```json\n{\"observation\":\"x\",\"reasoning\":\"y\",\"actions\":[],\"confidence\":0.9}\n```";
        let plan = ActionPlan::from_json(input).expect("parse from code block");
        assert_eq!(plan.observation, "x");
    }

    #[test]
    fn test_agent_action_roundtrip() {
        let actions = vec![
            AgentAction::Click {
                x: 1,
                y: 2,
                button: "right".to_string(),
            },
            AgentAction::Type {
                text: "test".to_string(),
            },
            AgentAction::KeyPress {
                key: "Return".to_string(),
            },
            AgentAction::Scroll {
                x: 100,
                y: 200,
                direction: "down".to_string(),
                amount: 3,
            },
            AgentAction::Wait { ms: 1000 },
            AgentAction::Screenshot,
            AgentAction::Done {
                summary: "done".to_string(),
            },
        ];
        for action in &actions {
            let json = serde_json::to_string(action).expect("serialize");
            let back: AgentAction = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(*action, back);
        }
    }

    #[test]
    fn test_default_button_is_left() {
        let json = r#"{"action": "click", "x": 10, "y": 20}"#;
        let action: AgentAction = serde_json::from_str(json).expect("parse");
        match action {
            AgentAction::Click { button, .. } => assert_eq!(button, "left"),
            _ => panic!("expected Click"),
        }
    }

    #[test]
    fn test_default_scroll_amount() {
        let json = r#"{"action": "scroll", "x": 10, "y": 20, "direction": "up"}"#;
        let action: AgentAction = serde_json::from_str(json).expect("parse");
        match action {
            AgentAction::Scroll { amount, .. } => assert_eq!(amount, 3),
            _ => panic!("expected Scroll"),
        }
    }

    #[test]
    fn test_default_wait_ms() {
        let json = r#"{"action": "wait"}"#;
        let action: AgentAction = serde_json::from_str(json).expect("parse");
        match action {
            AgentAction::Wait { ms } => assert_eq!(ms, 500),
            _ => panic!("expected Wait"),
        }
    }
}
