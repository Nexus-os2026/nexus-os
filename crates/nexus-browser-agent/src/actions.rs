//! Typed browser actions available to Nexus OS agents.

use serde::{Deserialize, Serialize};

use crate::bridge::BridgeCommand;

/// All browser actions available to agents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BrowserAction {
    ExecuteTask {
        task: String,
        max_steps: Option<u32>,
    },
    Navigate {
        url: String,
    },
    Screenshot {
        output_path: Option<String>,
    },
    GetContent,
    Close,
}

/// Result of a browser action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserActionResult {
    pub success: bool,
    pub action: String,
    pub result: Option<String>,
    pub url: Option<String>,
    pub title: Option<String>,
    pub steps_taken: Option<usize>,
    pub error: Option<String>,
    pub estimated_tokens: u64,
}

impl BrowserAction {
    /// Convert to a bridge command.
    pub fn to_command(&self, model_id: &str) -> BridgeCommand {
        match self {
            BrowserAction::ExecuteTask { task, max_steps } => BridgeCommand {
                action: "execute_task".into(),
                params: serde_json::json!({
                    "task": task,
                    "model_id": model_id,
                    "max_steps": max_steps.unwrap_or(20),
                }),
            },
            BrowserAction::Navigate { url } => BridgeCommand {
                action: "navigate".into(),
                params: serde_json::json!({ "url": url }),
            },
            BrowserAction::Screenshot { output_path } => BridgeCommand {
                action: "screenshot".into(),
                params: serde_json::json!({
                    "output_path": output_path.clone().unwrap_or_else(|| "/tmp/nexus_screenshot.png".into()),
                }),
            },
            BrowserAction::GetContent => BridgeCommand {
                action: "get_content".into(),
                params: serde_json::json!({}),
            },
            BrowserAction::Close => BridgeCommand {
                action: "close".into(),
                params: serde_json::json!({}),
            },
        }
    }

    /// Estimate token usage for compute burn calculation.
    pub fn estimated_tokens(&self) -> u64 {
        match self {
            BrowserAction::ExecuteTask { max_steps, .. } => {
                let steps = max_steps.unwrap_or(20) as u64;
                steps * 700
            }
            BrowserAction::Navigate { .. } => 100,
            BrowserAction::Screenshot { .. } => 50,
            BrowserAction::GetContent => 200,
            BrowserAction::Close => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_browser_action_to_command() {
        let action = BrowserAction::ExecuteTask {
            task: "search for cats".into(),
            max_steps: Some(10),
        };
        let cmd = action.to_command("ollama-7b");
        assert_eq!(cmd.action, "execute_task");
        assert_eq!(cmd.params["task"], "search for cats");
        assert_eq!(cmd.params["model_id"], "ollama-7b");
        assert_eq!(cmd.params["max_steps"], 10);

        let nav = BrowserAction::Navigate {
            url: "https://example.com".into(),
        };
        let cmd = nav.to_command("");
        assert_eq!(cmd.action, "navigate");
        assert_eq!(cmd.params["url"], "https://example.com");
    }

    #[test]
    fn test_browser_action_token_estimation() {
        let task = BrowserAction::ExecuteTask {
            task: "test".into(),
            max_steps: Some(10),
        };
        let nav = BrowserAction::Navigate {
            url: "https://x.com".into(),
        };
        assert!(
            task.estimated_tokens() > nav.estimated_tokens(),
            "ExecuteTask should use more tokens than Navigate"
        );
        assert_eq!(BrowserAction::Close.estimated_tokens(), 0);
    }
}
