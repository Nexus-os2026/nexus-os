use std::process::Stdio;
use std::time::{Duration, Instant};

use chrono::Utc;
use sha2::{Digest, Sha256};
use tracing::debug;

use crate::error::ComputerUseError;
use crate::input::backend::{parse_mouse_location, InputBackend};

const INPUT_TIMEOUT_SECS: u64 = 5;

/// Mouse button identifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

impl MouseButton {
    /// Convert to xdotool button number
    fn to_xdotool_button(self) -> &'static str {
        match self {
            MouseButton::Left => "1",
            MouseButton::Right => "3",
            MouseButton::Middle => "2",
        }
    }
}

impl std::fmt::Display for MouseButton {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MouseButton::Left => write!(f, "left"),
            MouseButton::Right => write!(f, "right"),
            MouseButton::Middle => write!(f, "middle"),
        }
    }
}

/// Scroll direction
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ScrollDirection {
    Up,
    Down,
    Left,
    Right,
}

impl ScrollDirection {
    /// Convert to xdotool button number for scrolling
    fn to_xdotool_button(self) -> &'static str {
        match self {
            ScrollDirection::Up => "4",
            ScrollDirection::Down => "5",
            ScrollDirection::Left => "6",
            ScrollDirection::Right => "7",
        }
    }
}

impl std::fmt::Display for ScrollDirection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScrollDirection::Up => write!(f, "up"),
            ScrollDirection::Down => write!(f, "down"),
            ScrollDirection::Left => write!(f, "left"),
            ScrollDirection::Right => write!(f, "right"),
        }
    }
}

/// Mouse actions that can be performed
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MouseAction {
    Click {
        x: u32,
        y: u32,
        button: MouseButton,
    },
    DoubleClick {
        x: u32,
        y: u32,
        button: MouseButton,
    },
    Move {
        x: u32,
        y: u32,
    },
    Scroll {
        x: u32,
        y: u32,
        direction: ScrollDirection,
        amount: u32,
    },
    Drag {
        start_x: u32,
        start_y: u32,
        end_x: u32,
        end_y: u32,
    },
    GetPosition,
}

impl std::fmt::Display for MouseAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MouseAction::Click { x, y, button } => {
                write!(f, "click({button}, {x}, {y})")
            }
            MouseAction::DoubleClick { x, y, button } => {
                write!(f, "double_click({button}, {x}, {y})")
            }
            MouseAction::Move { x, y } => write!(f, "move({x}, {y})"),
            MouseAction::Scroll {
                x,
                y,
                direction,
                amount,
            } => write!(f, "scroll({direction}, {amount}, {x}, {y})"),
            MouseAction::Drag {
                start_x,
                start_y,
                end_x,
                end_y,
            } => write!(f, "drag({start_x},{start_y} -> {end_x},{end_y})"),
            MouseAction::GetPosition => write!(f, "get_position"),
        }
    }
}

/// Result of a mouse action
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MouseActionResult {
    pub action: String,
    pub timestamp: chrono::DateTime<Utc>,
    pub duration_ms: u64,
    pub audit_hash: String,
    /// For GetPosition, the current coordinates
    pub position: Option<(u32, u32)>,
}

/// Mouse controller that executes actions via the input backend
pub struct MouseController {
    backend: InputBackend,
}

impl MouseController {
    pub fn new(backend: InputBackend) -> Self {
        Self { backend }
    }

    /// Build the xdotool command arguments for a mouse action
    pub fn build_command_args(action: &MouseAction) -> Vec<String> {
        match action {
            MouseAction::Click { x, y, button } => vec![
                "mousemove".into(),
                x.to_string(),
                y.to_string(),
                "click".into(),
                button.to_xdotool_button().into(),
            ],
            MouseAction::DoubleClick { x, y, button } => vec![
                "mousemove".into(),
                x.to_string(),
                y.to_string(),
                "click".into(),
                "--repeat".into(),
                "2".into(),
                button.to_xdotool_button().into(),
            ],
            MouseAction::Move { x, y } => {
                vec!["mousemove".into(), x.to_string(), y.to_string()]
            }
            MouseAction::Scroll {
                x,
                y,
                direction,
                amount,
            } => vec![
                "mousemove".into(),
                x.to_string(),
                y.to_string(),
                "click".into(),
                "--repeat".into(),
                amount.to_string(),
                direction.to_xdotool_button().into(),
            ],
            MouseAction::Drag {
                start_x,
                start_y,
                end_x,
                end_y,
            } => vec![
                "mousemove".into(),
                start_x.to_string(),
                start_y.to_string(),
                "mousedown".into(),
                "1".into(),
                "mousemove".into(),
                end_x.to_string(),
                end_y.to_string(),
                "mouseup".into(),
                "1".into(),
            ],
            MouseAction::GetPosition => vec!["getmouselocation".into()],
        }
    }

    /// Execute a mouse action
    pub async fn execute(
        &self,
        action: &MouseAction,
    ) -> Result<MouseActionResult, ComputerUseError> {
        let start = Instant::now();
        let args = Self::build_command_args(action);

        debug!("Executing mouse action: {} {:?}", self.backend.kind, args);

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
        .map_err(|e| ComputerUseError::InputError(format!("Mouse action failed: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ComputerUseError::InputError(format!(
                "xdotool exited {}: {}",
                output.status, stderr
            )));
        }

        let duration_ms = start.elapsed().as_millis() as u64;
        let action_str = action.to_string();

        // Parse position for GetPosition
        let position = if matches!(action, MouseAction::GetPosition) {
            let stdout = String::from_utf8_lossy(&output.stdout);
            Some(parse_mouse_location(&stdout)?)
        } else {
            None
        };

        let audit_hash = compute_action_hash(&action_str);

        Ok(MouseActionResult {
            action: action_str,
            timestamp: Utc::now(),
            duration_ms,
            audit_hash,
            position,
        })
    }
}

/// Compute SHA-256 hash of an action description for audit trail
pub fn compute_action_hash(description: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(description.as_bytes());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mouse_click_command_left() {
        let action = MouseAction::Click {
            x: 100,
            y: 200,
            button: MouseButton::Left,
        };
        let args = MouseController::build_command_args(&action);
        assert_eq!(args, vec!["mousemove", "100", "200", "click", "1"]);
    }

    #[test]
    fn test_mouse_click_command_right() {
        let action = MouseAction::Click {
            x: 300,
            y: 400,
            button: MouseButton::Right,
        };
        let args = MouseController::build_command_args(&action);
        assert_eq!(args, vec!["mousemove", "300", "400", "click", "3"]);
    }

    #[test]
    fn test_mouse_click_command_middle() {
        let action = MouseAction::Click {
            x: 50,
            y: 75,
            button: MouseButton::Middle,
        };
        let args = MouseController::build_command_args(&action);
        assert_eq!(args, vec!["mousemove", "50", "75", "click", "2"]);
    }

    #[test]
    fn test_mouse_double_click_command() {
        let action = MouseAction::DoubleClick {
            x: 100,
            y: 200,
            button: MouseButton::Left,
        };
        let args = MouseController::build_command_args(&action);
        assert_eq!(
            args,
            vec!["mousemove", "100", "200", "click", "--repeat", "2", "1"]
        );
    }

    #[test]
    fn test_mouse_move_command() {
        let action = MouseAction::Move { x: 500, y: 600 };
        let args = MouseController::build_command_args(&action);
        assert_eq!(args, vec!["mousemove", "500", "600"]);
    }

    #[test]
    fn test_mouse_scroll_up_command() {
        let action = MouseAction::Scroll {
            x: 100,
            y: 200,
            direction: ScrollDirection::Up,
            amount: 3,
        };
        let args = MouseController::build_command_args(&action);
        assert_eq!(
            args,
            vec!["mousemove", "100", "200", "click", "--repeat", "3", "4"]
        );
    }

    #[test]
    fn test_mouse_scroll_down_command() {
        let action = MouseAction::Scroll {
            x: 100,
            y: 200,
            direction: ScrollDirection::Down,
            amount: 5,
        };
        let args = MouseController::build_command_args(&action);
        assert_eq!(
            args,
            vec!["mousemove", "100", "200", "click", "--repeat", "5", "5"]
        );
    }

    #[test]
    fn test_mouse_drag_command() {
        let action = MouseAction::Drag {
            start_x: 10,
            start_y: 20,
            end_x: 300,
            end_y: 400,
        };
        let args = MouseController::build_command_args(&action);
        assert_eq!(
            args,
            vec![
                "mousemove",
                "10",
                "20",
                "mousedown",
                "1",
                "mousemove",
                "300",
                "400",
                "mouseup",
                "1"
            ]
        );
    }

    #[test]
    fn test_mouse_get_position_parse() {
        let (x, y) =
            parse_mouse_location("x:1234 y:567 screen:0 window:8388613").expect("should parse");
        assert_eq!(x, 1234);
        assert_eq!(y, 567);
    }

    #[test]
    fn test_mouse_bounds_validation() {
        // Coordinates that exceed screen should be caught by safety guard
        // Here we just validate the action builds correctly even with large coords
        let action = MouseAction::Click {
            x: 9999,
            y: 9999,
            button: MouseButton::Left,
        };
        let args = MouseController::build_command_args(&action);
        assert_eq!(args[1], "9999");
        assert_eq!(args[2], "9999");
    }

    #[test]
    fn test_mouse_action_result_hash() {
        let hash1 = compute_action_hash("click(left, 100, 200)");
        let hash2 = compute_action_hash("click(left, 100, 200)");
        let hash3 = compute_action_hash("click(right, 100, 200)");

        // Deterministic
        assert_eq!(hash1, hash2);
        // Different input = different hash
        assert_ne!(hash1, hash3);
        // SHA-256 = 64 hex chars
        assert_eq!(hash1.len(), 64);
    }

    #[test]
    fn test_mouse_button_display() {
        assert_eq!(MouseButton::Left.to_string(), "left");
        assert_eq!(MouseButton::Right.to_string(), "right");
        assert_eq!(MouseButton::Middle.to_string(), "middle");
    }

    #[test]
    fn test_scroll_direction_display() {
        assert_eq!(ScrollDirection::Up.to_string(), "up");
        assert_eq!(ScrollDirection::Down.to_string(), "down");
        assert_eq!(ScrollDirection::Left.to_string(), "left");
        assert_eq!(ScrollDirection::Right.to_string(), "right");
    }

    #[test]
    fn test_mouse_action_display() {
        let action = MouseAction::Click {
            x: 10,
            y: 20,
            button: MouseButton::Left,
        };
        assert_eq!(action.to_string(), "click(left, 10, 20)");
    }

    #[ignore]
    #[tokio::test]
    async fn test_real_mouse_move() {
        let backend = crate::input::detect_input_backend().expect("need xdotool");
        let controller = MouseController::new(backend);
        let result = controller
            .execute(&MouseAction::Move { x: 500, y: 500 })
            .await
            .expect("move should succeed");
        assert!(result.duration_ms < 5000);
    }

    #[ignore]
    #[tokio::test]
    async fn test_real_get_position() {
        let backend = crate::input::detect_input_backend().expect("need xdotool");
        let controller = MouseController::new(backend);
        let result = controller
            .execute(&MouseAction::GetPosition)
            .await
            .expect("get_position should succeed");
        assert!(result.position.is_some());
    }
}
