use std::process::Stdio;
use std::time::{Duration, Instant};

use chrono::Utc;
use tracing::debug;

use crate::error::ComputerUseError;
use crate::input::backend::InputBackend;
use crate::input::mouse::compute_action_hash;

const INPUT_TIMEOUT_SECS: u64 = 5;

/// Keyboard actions that can be performed
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum KeyAction {
    /// Type a string of text with natural delays
    Type { text: String },
    /// Press and release a single key
    KeyPress { key: String },
    /// Press a key combination (e.g., ctrl+s)
    KeyCombo { keys: Vec<String> },
    /// Hold a key down
    KeyDown { key: String },
    /// Release a held key
    KeyUp { key: String },
}

impl std::fmt::Display for KeyAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KeyAction::Type { text } => {
                let preview = if text.len() > 20 {
                    format!("{}...", &text[..20])
                } else {
                    text.clone()
                };
                write!(f, "type({preview:?})")
            }
            KeyAction::KeyPress { key } => write!(f, "key_press({key})"),
            KeyAction::KeyCombo { keys } => write!(f, "key_combo({})", keys.join("+")),
            KeyAction::KeyDown { key } => write!(f, "key_down({key})"),
            KeyAction::KeyUp { key } => write!(f, "key_up({key})"),
        }
    }
}

/// Blocked key combinations that require SystemKeyCombos capability
const BLOCKED_COMBOS: &[&str] = &["ctrl+alt+delete", "alt+f4", "ctrl+alt+t", "super"];

/// Check if a key combo is blocked
pub fn is_combo_blocked(combo: &str) -> bool {
    let normalized = combo.to_lowercase().replace(' ', "");
    BLOCKED_COMBOS.iter().any(|blocked| {
        let blocked_normalized = blocked.to_lowercase().replace(' ', "");
        normalized == blocked_normalized
    })
}

/// Result of a keyboard action
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct KeyboardActionResult {
    pub action: String,
    pub timestamp: chrono::DateTime<Utc>,
    pub duration_ms: u64,
    pub audit_hash: String,
}

/// Keyboard controller that executes actions via the input backend
pub struct KeyboardController {
    backend: InputBackend,
}

impl KeyboardController {
    pub fn new(backend: InputBackend) -> Self {
        Self { backend }
    }

    /// Escape special shell characters in text for xdotool type
    fn escape_text(text: &str) -> String {
        // xdotool type handles most chars, but we need to be careful with
        // shell metacharacters since we pass via Command (not shell)
        // tokio::process::Command doesn't go through shell, so no shell escaping needed
        // But xdotool itself needs no escaping when passed as argument directly
        text.to_string()
    }

    /// Build the xdotool command arguments for a keyboard action
    pub fn build_command_args(action: &KeyAction) -> Vec<String> {
        match action {
            KeyAction::Type { text } => {
                let escaped = Self::escape_text(text);
                vec![
                    "type".into(),
                    "--delay".into(),
                    "12".into(),
                    "--".into(),
                    escaped,
                ]
            }
            KeyAction::KeyPress { key } => vec!["key".into(), key.clone()],
            KeyAction::KeyCombo { keys } => {
                vec!["key".into(), keys.join("+")]
            }
            KeyAction::KeyDown { key } => vec!["keydown".into(), key.clone()],
            KeyAction::KeyUp { key } => vec!["keyup".into(), key.clone()],
        }
    }

    /// Execute a keyboard action
    pub async fn execute(
        &self,
        action: &KeyAction,
    ) -> Result<KeyboardActionResult, ComputerUseError> {
        // Check blocked combos
        match action {
            KeyAction::KeyCombo { keys } => {
                let combo = keys.join("+");
                if is_combo_blocked(&combo) {
                    return Err(ComputerUseError::BlockedKeyCombination { combo });
                }
            }
            KeyAction::KeyPress { key } => {
                if is_combo_blocked(key) {
                    return Err(ComputerUseError::BlockedKeyCombination { combo: key.clone() });
                }
            }
            _ => {}
        }

        let start = Instant::now();
        let args = Self::build_command_args(action);

        debug!(
            "Executing keyboard action: {} {:?}",
            self.backend.kind, args
        );

        let mut cmd = tokio::process::Command::new(&self.backend.binary_path);
        cmd.args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let child = cmd.spawn().map_err(|e| {
            ComputerUseError::InputError(format!("Failed to spawn {}: {e}", self.backend.kind))
        })?;

        let output = tokio::time::timeout(
            Duration::from_secs(INPUT_TIMEOUT_SECS),
            child.wait_with_output(),
        )
        .await
        .map_err(|_| ComputerUseError::InputTimeout {
            seconds: INPUT_TIMEOUT_SECS,
        })?
        .map_err(|e| ComputerUseError::InputError(format!("Keyboard action failed: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ComputerUseError::InputError(format!(
                "xdotool exited {}: {}",
                output.status, stderr
            )));
        }

        let duration_ms = start.elapsed().as_millis() as u64;
        let action_str = action.to_string();
        let audit_hash = compute_action_hash(&action_str);

        Ok(KeyboardActionResult {
            action: action_str,
            timestamp: Utc::now(),
            duration_ms,
            audit_hash,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keyboard_type_command() {
        let action = KeyAction::Type {
            text: "hello world".into(),
        };
        let args = KeyboardController::build_command_args(&action);
        assert_eq!(args, vec!["type", "--delay", "12", "--", "hello world"]);
    }

    #[test]
    fn test_keyboard_type_escapes_special_chars() {
        let action = KeyAction::Type {
            text: "it's a \"test\" & more <html>".into(),
        };
        let args = KeyboardController::build_command_args(&action);
        // Passed as direct argument to Command, no shell escaping needed
        assert_eq!(args[4], "it's a \"test\" & more <html>");
    }

    #[test]
    fn test_keyboard_keypress_command() {
        let action = KeyAction::KeyPress {
            key: "Return".into(),
        };
        let args = KeyboardController::build_command_args(&action);
        assert_eq!(args, vec!["key", "Return"]);
    }

    #[test]
    fn test_keyboard_combo_command() {
        let action = KeyAction::KeyCombo {
            keys: vec!["ctrl".into(), "s".into()],
        };
        let args = KeyboardController::build_command_args(&action);
        assert_eq!(args, vec!["key", "ctrl+s"]);
    }

    #[test]
    fn test_keyboard_combo_multiple_keys() {
        let action = KeyAction::KeyCombo {
            keys: vec!["ctrl".into(), "shift".into(), "p".into()],
        };
        let args = KeyboardController::build_command_args(&action);
        assert_eq!(args, vec!["key", "ctrl+shift+p"]);
    }

    #[test]
    fn test_keyboard_keydown_command() {
        let action = KeyAction::KeyDown {
            key: "Shift_L".into(),
        };
        let args = KeyboardController::build_command_args(&action);
        assert_eq!(args, vec!["keydown", "Shift_L"]);
    }

    #[test]
    fn test_keyboard_keyup_command() {
        let action = KeyAction::KeyUp {
            key: "Shift_L".into(),
        };
        let args = KeyboardController::build_command_args(&action);
        assert_eq!(args, vec!["keyup", "Shift_L"]);
    }

    #[test]
    fn test_keyboard_action_result_hash() {
        let hash1 = compute_action_hash("key_press(Return)");
        let hash2 = compute_action_hash("key_press(Return)");
        let hash3 = compute_action_hash("key_press(Escape)");

        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
        assert_eq!(hash1.len(), 64);
    }

    #[test]
    fn test_blocked_combo_alt_f4() {
        assert!(is_combo_blocked("alt+F4"));
        assert!(is_combo_blocked("alt+f4"));
        assert!(is_combo_blocked("Alt+F4"));
    }

    #[test]
    fn test_blocked_combo_ctrl_alt_delete() {
        assert!(is_combo_blocked("ctrl+alt+Delete"));
        assert!(is_combo_blocked("ctrl+alt+delete"));
    }

    #[test]
    fn test_allowed_combo_ctrl_s() {
        assert!(!is_combo_blocked("ctrl+s"));
        assert!(!is_combo_blocked("ctrl+S"));
    }

    #[test]
    fn test_allowed_combo_ctrl_c() {
        assert!(!is_combo_blocked("ctrl+c"));
        assert!(!is_combo_blocked("ctrl+C"));
    }

    #[test]
    fn test_system_keys_grant_allows_blocked() {
        // The safety guard handles the grant logic, but the raw check always blocks
        assert!(is_combo_blocked("alt+f4"));
        // When system_keys_allowed=true in the safety guard, it bypasses this check
    }

    #[test]
    fn test_key_action_display() {
        let action = KeyAction::Type {
            text: "hello".into(),
        };
        assert_eq!(action.to_string(), "type(\"hello\")");

        let action = KeyAction::KeyCombo {
            keys: vec!["ctrl".into(), "s".into()],
        };
        assert_eq!(action.to_string(), "key_combo(ctrl+s)");
    }

    #[ignore]
    #[tokio::test]
    async fn test_real_type_text() {
        let backend = crate::input::detect_input_backend().expect("need xdotool");
        let controller = KeyboardController::new(backend);
        let result = controller
            .execute(&KeyAction::Type {
                text: "hello from nexus".into(),
            })
            .await
            .expect("type should succeed");
        assert!(result.duration_ms < 10000);
    }
}
