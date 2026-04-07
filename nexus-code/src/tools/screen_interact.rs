//! ScreenInteractTool — governed screen interaction via xdotool.
//! Requires ComputerUse capability (not granted by default).

use super::{NxTool, ToolContext, ToolResult};
use async_trait::async_trait;
use serde_json::json;
use std::sync::atomic::{AtomicU64, Ordering};

/// Maximum actions per second (rate limiting).
const MAX_ACTIONS_PER_SECOND: u64 = 10;

/// Dead man switch timeout in seconds.
const DEAD_MAN_SWITCH_SECS: u64 = 60;

/// Key combos that are always blocked for safety.
const BLOCKED_COMBOS: &[&str] = &[
    "ctrl+alt+Delete",
    "ctrl+alt+delete",
    "ctrl+alt+Del",
    "ctrl+alt+del",
    "super+l",
    "super+L",
    "alt+F4",
    "alt+f4",
    "ctrl+alt+F1",
    "ctrl+alt+F2",
    "ctrl+alt+F3",
    "ctrl+alt+F4",
    "ctrl+alt+F5",
    "ctrl+alt+F6",
    "ctrl+alt+F7",
    "ctrl+alt+BackSpace",
];

// Simple rate limiter using atomic counter + timestamp
static LAST_ACTION_EPOCH_MS: AtomicU64 = AtomicU64::new(0);
static ACTION_COUNT_IN_WINDOW: AtomicU64 = AtomicU64::new(0);

/// Check if a key combo is in the blocked list.
pub fn is_blocked_combo(combo: &str) -> bool {
    BLOCKED_COMBOS.iter().any(|b| combo.eq_ignore_ascii_case(b))
}

/// Check rate limit: max MAX_ACTIONS_PER_SECOND in a 1-second window.
fn check_rate_limit() -> Result<(), String> {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let last = LAST_ACTION_EPOCH_MS.load(Ordering::Relaxed);
    if now_ms.saturating_sub(last) > 1000 {
        // New window
        LAST_ACTION_EPOCH_MS.store(now_ms, Ordering::Relaxed);
        ACTION_COUNT_IN_WINDOW.store(1, Ordering::Relaxed);
        Ok(())
    } else {
        let count = ACTION_COUNT_IN_WINDOW.fetch_add(1, Ordering::Relaxed) + 1;
        if count > MAX_ACTIONS_PER_SECOND {
            Err(format!(
                "Rate limited: {} actions/sec exceeds max {}",
                count, MAX_ACTIONS_PER_SECOND
            ))
        } else {
            Ok(())
        }
    }
}

/// Interact with the screen — click, type, scroll, key combos, move.
/// All actions are governed: rate limited, blocked combos, dead man switch, audit logged.
pub struct ScreenInteractTool;

#[async_trait]
impl NxTool for ScreenInteractTool {
    fn name(&self) -> &str {
        "screen_interact"
    }

    fn description(&self) -> &str {
        "Interact with the screen: click, type text, scroll, press key combos, or move the mouse. \
         All actions are governed — rate limited (10/sec), dangerous combos blocked, \
         60s dead man switch timeout. Requires ComputerUse capability."
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["click", "type", "scroll", "key", "move"],
                    "description": "The interaction type"
                },
                "x": {
                    "type": "integer",
                    "description": "X coordinate (for click/move)"
                },
                "y": {
                    "type": "integer",
                    "description": "Y coordinate (for click/move)"
                },
                "button": {
                    "type": "string",
                    "description": "Mouse button: 1=left, 2=middle, 3=right (default: 1)"
                },
                "text": {
                    "type": "string",
                    "description": "Text to type (for action=type)"
                },
                "direction": {
                    "type": "string",
                    "enum": ["up", "down"],
                    "description": "Scroll direction (for action=scroll)"
                },
                "amount": {
                    "type": "integer",
                    "description": "Scroll amount in clicks (default: 3)"
                },
                "combo": {
                    "type": "string",
                    "description": "Key combination (for action=key), e.g. 'ctrl+s'"
                }
            },
            "required": ["action"]
        })
    }

    fn estimated_fuel(&self, _input: &serde_json::Value) -> u64 {
        1
    }

    fn required_capability(
        &self,
        _input: &serde_json::Value,
    ) -> Option<crate::governance::Capability> {
        Some(crate::governance::Capability::ComputerUse)
    }

    async fn execute(&self, input: serde_json::Value, _ctx: &ToolContext) -> ToolResult {
        let action = match input.get("action").and_then(|v| v.as_str()) {
            Some(a) => a,
            None => return ToolResult::error("Missing required parameter: action"),
        };

        // Blocked combo check (before rate limiting)
        if action == "key" {
            let combo = match input.get("combo").and_then(|v| v.as_str()) {
                Some(c) => c,
                None => return ToolResult::error("Missing required parameter: combo"),
            };
            if is_blocked_combo(combo) {
                return ToolResult::error(format!(
                    "Blocked key combo: '{}' — this combination is forbidden for safety",
                    combo
                ));
            }
        }

        // Rate limit check
        if let Err(msg) = check_rate_limit() {
            return ToolResult::error(msg);
        }

        // Execute with dead man switch timeout
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(DEAD_MAN_SWITCH_SECS),
            execute_action(action, &input),
        )
        .await;

        match result {
            Ok(tool_result) => tool_result,
            Err(_) => ToolResult::error(format!(
                "Dead man switch: action '{}' timed out after {}s — aborted",
                action, DEAD_MAN_SWITCH_SECS
            )),
        }
    }
}

async fn execute_action(action: &str, input: &serde_json::Value) -> ToolResult {
    match action {
        "click" => {
            let x = match input.get("x").and_then(|v| v.as_i64()) {
                Some(v) => v,
                None => return ToolResult::error("Missing required parameter: x"),
            };
            let y = match input.get("y").and_then(|v| v.as_i64()) {
                Some(v) => v,
                None => return ToolResult::error("Missing required parameter: y"),
            };
            let button = input.get("button").and_then(|v| v.as_str()).unwrap_or("1");

            let move_result = std::process::Command::new("xdotool")
                .args(["mousemove", "--sync", &x.to_string(), &y.to_string()])
                .output();
            if let Err(e) = move_result {
                return ToolResult::error(format!("xdotool mousemove failed: {}", e));
            }

            let click_result = std::process::Command::new("xdotool")
                .args(["click", button])
                .output();
            if let Err(e) = click_result {
                return ToolResult::error(format!("xdotool click failed: {}", e));
            }

            ToolResult::success(format!("Clicked at ({}, {}) button={}", x, y, button))
        }
        "type" => {
            let text = match input.get("text").and_then(|v| v.as_str()) {
                Some(t) => t,
                None => return ToolResult::error("Missing required parameter: text"),
            };

            let result = std::process::Command::new("xdotool")
                .args(["type", "--clearmodifiers", "--delay", "12", text])
                .output();
            if let Err(e) = result {
                return ToolResult::error(format!("xdotool type failed: {}", e));
            }

            ToolResult::success(format!("Typed {} chars", text.len()))
        }
        "scroll" => {
            let direction = match input.get("direction").and_then(|v| v.as_str()) {
                Some(d) => d,
                None => return ToolResult::error("Missing required parameter: direction"),
            };
            let amount = input.get("amount").and_then(|v| v.as_i64()).unwrap_or(3);
            let button = if direction == "down" { "5" } else { "4" };

            for _ in 0..amount {
                if let Err(e) = std::process::Command::new("xdotool")
                    .args(["click", button])
                    .output()
                {
                    return ToolResult::error(format!("xdotool scroll failed: {}", e));
                }
            }

            ToolResult::success(format!("Scrolled {} {} clicks", direction, amount))
        }
        "key" => {
            let combo = match input.get("combo").and_then(|v| v.as_str()) {
                Some(c) => c,
                None => return ToolResult::error("Missing required parameter: combo"),
            };

            let result = std::process::Command::new("xdotool")
                .args(["key", "--clearmodifiers", combo])
                .output();
            if let Err(e) = result {
                return ToolResult::error(format!("xdotool key failed: {}", e));
            }

            ToolResult::success(format!("Pressed key combo: {}", combo))
        }
        "move" => {
            let x = match input.get("x").and_then(|v| v.as_i64()) {
                Some(v) => v,
                None => return ToolResult::error("Missing required parameter: x"),
            };
            let y = match input.get("y").and_then(|v| v.as_i64()) {
                Some(v) => v,
                None => return ToolResult::error("Missing required parameter: y"),
            };

            let result = std::process::Command::new("xdotool")
                .args(["mousemove", "--sync", &x.to_string(), &y.to_string()])
                .output();
            if let Err(e) = result {
                return ToolResult::error(format!("xdotool mousemove failed: {}", e));
            }

            ToolResult::success(format!("Moved mouse to ({}, {})", x, y))
        }
        _ => ToolResult::error(format!("Unknown action: '{}'", action)),
    }
}
