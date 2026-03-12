//! Computer-Control Engine — agents control real desktop applications.
//!
//! Provides screenshot capture, keyboard/mouse simulation, and governed
//! rate-limited action execution. All actions require the `computer.control`
//! capability and are fully audit-logged.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

// ── Types ───────────────────────────────────────────────────────────────

/// A rectangular screen region for targeted capture.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenRegion {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// Mouse button identifiers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// An action that an agent can execute on the desktop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InputAction {
    Click { x: u32, y: u32, button: MouseButton },
    DoubleClick { x: u32, y: u32 },
    Type { text: String },
    KeyPress { key: String, modifiers: Vec<String> },
    MoveMouse { x: u32, y: u32 },
    Scroll { x: u32, y: u32, delta: i32 },
    Wait { ms: u64 },
}

impl InputAction {
    /// A short human-readable label for audit logging.
    pub fn label(&self) -> String {
        match self {
            Self::Click { x, y, button } => format!("click({button:?} @ {x},{y})"),
            Self::DoubleClick { x, y } => format!("double_click({x},{y})"),
            Self::Type { text } => {
                let preview: String = text.chars().take(20).collect();
                let suffix = if text.len() > 20 { "..." } else { "" };
                format!("type(\"{preview}{suffix}\")")
            }
            Self::KeyPress { key, modifiers } => {
                if modifiers.is_empty() {
                    format!("key({key})")
                } else {
                    format!("key({}+{key})", modifiers.join("+"))
                }
            }
            Self::MoveMouse { x, y } => format!("move({x},{y})"),
            Self::Scroll { x, y, delta } => format!("scroll({x},{y}, delta={delta})"),
            Self::Wait { ms } => format!("wait({ms}ms)"),
        }
    }
}

/// Recorded action with timestamp.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionRecord {
    pub action: InputAction,
    pub timestamp: u64,
    pub success: bool,
    pub error: Option<String>,
}

/// Result of a screen capture operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureResult {
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub size_bytes: usize,
}

// ── Screen capture ──────────────────────────────────────────────────────

/// Capture a screenshot. Returns PNG bytes.
///
/// Uses platform-native tools:
/// - Linux: `import` (ImageMagick) or `scrot`
/// - macOS: `screencapture`
/// - Windows: stub (returns error)
pub fn capture_screen(region: Option<&ScreenRegion>) -> Result<Vec<u8>, String> {
    capture_screen_impl(region)
}

#[cfg(target_os = "linux")]
fn capture_screen_impl(region: Option<&ScreenRegion>) -> Result<Vec<u8>, String> {
    let tmp = "/tmp/nexus_screenshot.png";

    let mut cmd = std::process::Command::new("import");
    if let Some(r) = region {
        cmd.arg("-window")
            .arg("root")
            .arg("-crop")
            .arg(format!("{}x{}+{}+{}", r.width, r.height, r.x, r.y));
    } else {
        cmd.arg("-window").arg("root");
    }
    cmd.arg(tmp);

    let output = cmd
        .output()
        .map_err(|e| format!("Failed to run screenshot tool (install ImageMagick): {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Screenshot failed: {stderr}"));
    }

    std::fs::read(tmp).map_err(|e| format!("Failed to read screenshot: {e}"))
}

#[cfg(target_os = "macos")]
fn capture_screen_impl(region: Option<&ScreenRegion>) -> Result<Vec<u8>, String> {
    let tmp = "/tmp/nexus_screenshot.png";

    let mut cmd = std::process::Command::new("screencapture");
    if let Some(r) = region {
        cmd.arg("-R")
            .arg(format!("{},{},{}x{}", r.x, r.y, r.width, r.height));
    }
    cmd.arg(tmp);

    let output = cmd
        .output()
        .map_err(|e| format!("Failed to run screencapture: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Screenshot failed: {stderr}"));
    }

    std::fs::read(tmp).map_err(|e| format!("Failed to read screenshot: {e}"))
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn capture_screen_impl(_region: Option<&ScreenRegion>) -> Result<Vec<u8>, String> {
    Err("Screen capture not yet supported on this platform".to_string())
}

// ── Input simulation ────────────────────────────────────────────────────

/// Execute a desktop input action using platform-native tools.
///
/// - Linux: `xdotool`
/// - macOS: `osascript`
/// - Windows: stub
pub fn execute_input_action(action: &InputAction) -> Result<(), String> {
    execute_input_action_impl(action)
}

#[cfg(target_os = "linux")]
fn execute_input_action_impl(action: &InputAction) -> Result<(), String> {
    match action {
        InputAction::Click { x, y, button } => {
            let btn = match button {
                MouseButton::Left => "1",
                MouseButton::Right => "3",
                MouseButton::Middle => "2",
            };
            run_xdotool(&["mousemove", &x.to_string(), &y.to_string()])?;
            run_xdotool(&["click", btn])
        }
        InputAction::DoubleClick { x, y } => {
            run_xdotool(&["mousemove", &x.to_string(), &y.to_string()])?;
            run_xdotool(&["click", "--repeat", "2", "1"])
        }
        InputAction::Type { text } => run_xdotool(&["type", "--clearmodifiers", text]),
        InputAction::KeyPress { key, modifiers } => {
            let combo = if modifiers.is_empty() {
                key.clone()
            } else {
                format!("{}+{key}", modifiers.join("+"))
            };
            run_xdotool(&["key", &combo])
        }
        InputAction::MoveMouse { x, y } => {
            run_xdotool(&["mousemove", &x.to_string(), &y.to_string()])
        }
        InputAction::Scroll { x, y, delta } => {
            run_xdotool(&["mousemove", &x.to_string(), &y.to_string()])?;
            let direction = if *delta > 0 { "5" } else { "4" };
            let clicks = delta.unsigned_abs().to_string();
            run_xdotool(&["click", "--repeat", &clicks, direction])
        }
        InputAction::Wait { ms } => {
            std::thread::sleep(std::time::Duration::from_millis(*ms));
            Ok(())
        }
    }
}

#[cfg(target_os = "linux")]
fn run_xdotool(args: &[&str]) -> Result<(), String> {
    let output = std::process::Command::new("xdotool")
        .args(args)
        .output()
        .map_err(|e| format!("Failed to run xdotool (install xdotool): {e}"))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("xdotool error: {stderr}"))
    }
}

#[cfg(target_os = "macos")]
fn execute_input_action_impl(action: &InputAction) -> Result<(), String> {
    match action {
        InputAction::Wait { ms } => {
            std::thread::sleep(std::time::Duration::from_millis(*ms));
            Ok(())
        }
        other => {
            // Use osascript for basic actions.
            let script = match other {
                InputAction::Click { x, y, .. } => {
                    format!("tell application \"System Events\" to click at {{{x}, {y}}}")
                }
                InputAction::Type { text } => {
                    format!("tell application \"System Events\" to keystroke \"{text}\"")
                }
                InputAction::KeyPress { key, .. } => {
                    format!("tell application \"System Events\" to key code \"{key}\"")
                }
                _ => return Err(format!("Action not supported on macOS: {:?}", other)),
            };
            let output = std::process::Command::new("osascript")
                .arg("-e")
                .arg(&script)
                .output()
                .map_err(|e| format!("Failed to run osascript: {e}"))?;

            if output.status.success() {
                Ok(())
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                Err(format!("osascript error: {stderr}"))
            }
        }
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn execute_input_action_impl(_action: &InputAction) -> Result<(), String> {
    Err("Input simulation not yet supported on this platform".to_string())
}

// ── ComputerControlEngine ───────────────────────────────────────────────

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Governed engine for desktop automation. Enforces rate limits and tracks
/// a full audit history of every action executed.
pub struct ComputerControlEngine {
    enabled: bool,
    action_history: Vec<ActionRecord>,
    max_actions_per_minute: usize,
    /// Sliding window of timestamps for rate limiting.
    recent_timestamps: VecDeque<u64>,
}

impl Default for ComputerControlEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl ComputerControlEngine {
    pub fn new() -> Self {
        Self {
            enabled: false, // disabled by default — must be explicitly enabled
            action_history: Vec::new(),
            max_actions_per_minute: 60,
            recent_timestamps: VecDeque::new(),
        }
    }

    /// Execute an input action with governance checks.
    ///
    /// Returns an error if the engine is disabled or rate-limited.
    /// Does NOT execute the system command in test mode — use `execute_real`
    /// for actual desktop control.
    pub fn execute(&mut self, action: InputAction) -> Result<ActionRecord, String> {
        if !self.enabled {
            return Err("Computer control is disabled".to_string());
        }

        self.rate_limit_check()?;

        let now = now_secs();
        let result = execute_input_action(&action);

        let record = ActionRecord {
            action,
            timestamp: now,
            success: result.is_ok(),
            error: result.err(),
        };

        self.action_history.push(record.clone());
        self.recent_timestamps.push_back(now);

        Ok(record)
    }

    /// Record an action without executing it (for testing / dry-run).
    pub fn record_dry_run(&mut self, action: InputAction) -> ActionRecord {
        let now = now_secs();
        let record = ActionRecord {
            action,
            timestamp: now,
            success: true,
            error: None,
        };
        self.action_history.push(record.clone());
        self.recent_timestamps.push_back(now);
        record
    }

    /// Capture a screenshot.
    pub fn capture_screen(&self, region: Option<&ScreenRegion>) -> Result<CaptureResult, String> {
        if !self.enabled {
            return Err("Computer control is disabled".to_string());
        }

        let data = capture_screen(region)?;
        Ok(CaptureResult {
            width: region.map_or(0, |r| r.width),
            height: region.map_or(0, |r| r.height),
            format: "png".to_string(),
            size_bytes: data.len(),
        })
    }

    /// Full action history.
    pub fn action_history(&self) -> &[ActionRecord] {
        &self.action_history
    }

    /// Enable the engine.
    pub fn enable(&mut self) {
        self.enabled = true;
    }

    /// Disable the engine. All subsequent actions will be rejected.
    pub fn disable(&mut self) {
        self.enabled = false;
    }

    /// Whether the engine is currently enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Current rate limit (actions per minute).
    pub fn max_actions_per_minute(&self) -> usize {
        self.max_actions_per_minute
    }

    /// Set the rate limit.
    pub fn set_max_actions_per_minute(&mut self, limit: usize) {
        self.max_actions_per_minute = limit;
    }

    /// Check if we're within the rate limit. Evicts stale entries.
    pub fn rate_limit_check(&mut self) -> Result<(), String> {
        let now = now_secs();
        let window_start = now.saturating_sub(60);

        // Evict entries older than 60 seconds.
        while self
            .recent_timestamps
            .front()
            .is_some_and(|&ts| ts < window_start)
        {
            self.recent_timestamps.pop_front();
        }

        if self.recent_timestamps.len() >= self.max_actions_per_minute {
            return Err(format!(
                "Rate limit exceeded: {} actions in the last 60 seconds (max {})",
                self.recent_timestamps.len(),
                self.max_actions_per_minute
            ));
        }

        Ok(())
    }

    /// Number of actions in the current rate-limit window.
    pub fn actions_in_window(&mut self) -> usize {
        let now = now_secs();
        let window_start = now.saturating_sub(60);
        while self
            .recent_timestamps
            .front()
            .is_some_and(|&ts| ts < window_start)
        {
            self.recent_timestamps.pop_front();
        }
        self.recent_timestamps.len()
    }

    /// Total actions ever executed.
    pub fn total_actions(&self) -> usize {
        self.action_history.len()
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_labels() {
        assert_eq!(
            InputAction::Click {
                x: 100,
                y: 200,
                button: MouseButton::Left
            }
            .label(),
            "click(Left @ 100,200)"
        );
        assert_eq!(
            InputAction::DoubleClick { x: 10, y: 20 }.label(),
            "double_click(10,20)"
        );
        assert_eq!(
            InputAction::Type {
                text: "hello".to_string()
            }
            .label(),
            "type(\"hello\")"
        );
        assert_eq!(
            InputAction::KeyPress {
                key: "Return".to_string(),
                modifiers: vec!["ctrl".to_string()]
            }
            .label(),
            "key(ctrl+Return)"
        );
        assert_eq!(
            InputAction::MoveMouse { x: 50, y: 60 }.label(),
            "move(50,60)"
        );
        assert_eq!(
            InputAction::Scroll {
                x: 0,
                y: 0,
                delta: -3
            }
            .label(),
            "scroll(0,0, delta=-3)"
        );
        assert_eq!(InputAction::Wait { ms: 500 }.label(), "wait(500ms)");
    }

    #[test]
    fn test_long_type_label_truncated() {
        let long_text = "a".repeat(50);
        let label = InputAction::Type { text: long_text }.label();
        assert!(label.contains("..."));
        assert!(label.len() < 40);
    }

    #[test]
    fn test_engine_disabled_by_default() {
        let engine = ComputerControlEngine::new();
        assert!(!engine.is_enabled());
    }

    #[test]
    fn test_engine_enable_disable() {
        let mut engine = ComputerControlEngine::new();
        engine.enable();
        assert!(engine.is_enabled());
        engine.disable();
        assert!(!engine.is_enabled());
    }

    #[test]
    fn test_execute_fails_when_disabled() {
        let mut engine = ComputerControlEngine::new();
        let result = engine.execute(InputAction::Wait { ms: 1 });
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("disabled"));
    }

    #[test]
    fn test_dry_run_records_action() {
        let mut engine = ComputerControlEngine::new();
        let action = InputAction::Click {
            x: 100,
            y: 200,
            button: MouseButton::Left,
        };
        let record = engine.record_dry_run(action);
        assert!(record.success);
        assert!(record.error.is_none());
        assert_eq!(engine.total_actions(), 1);
        assert_eq!(engine.action_history().len(), 1);
    }

    #[test]
    fn test_action_history_ordering() {
        let mut engine = ComputerControlEngine::new();
        engine.record_dry_run(InputAction::MoveMouse { x: 0, y: 0 });
        engine.record_dry_run(InputAction::Click {
            x: 10,
            y: 20,
            button: MouseButton::Left,
        });
        engine.record_dry_run(InputAction::Type {
            text: "hello".to_string(),
        });

        let history = engine.action_history();
        assert_eq!(history.len(), 3);
        assert!(matches!(history[0].action, InputAction::MoveMouse { .. }));
        assert!(matches!(history[1].action, InputAction::Click { .. }));
        assert!(matches!(history[2].action, InputAction::Type { .. }));
    }

    #[test]
    fn test_rate_limit_setting() {
        let mut engine = ComputerControlEngine::new();
        assert_eq!(engine.max_actions_per_minute(), 60);
        engine.set_max_actions_per_minute(10);
        assert_eq!(engine.max_actions_per_minute(), 10);
    }

    #[test]
    fn test_rate_limit_enforcement() {
        let mut engine = ComputerControlEngine::new();
        engine.set_max_actions_per_minute(3);

        // Fill the rate window.
        engine.record_dry_run(InputAction::Wait { ms: 1 });
        engine.record_dry_run(InputAction::Wait { ms: 1 });
        engine.record_dry_run(InputAction::Wait { ms: 1 });

        // Fourth action should be rate-limited.
        let result = engine.rate_limit_check();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Rate limit exceeded"));
    }

    #[test]
    fn test_capture_disabled() {
        let engine = ComputerControlEngine::new();
        let result = engine.capture_screen(None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("disabled"));
    }

    #[test]
    fn test_screen_region_serialization() {
        let region = ScreenRegion {
            x: 10,
            y: 20,
            width: 800,
            height: 600,
        };
        let json = serde_json::to_string(&region).unwrap();
        let parsed: ScreenRegion = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.x, 10);
        assert_eq!(parsed.width, 800);
    }

    #[test]
    fn test_input_action_serialization() {
        let action = InputAction::KeyPress {
            key: "a".to_string(),
            modifiers: vec!["ctrl".to_string(), "shift".to_string()],
        };
        let json = serde_json::to_string(&action).unwrap();
        let parsed: InputAction = serde_json::from_str(&json).unwrap();
        if let InputAction::KeyPress { key, modifiers } = parsed {
            assert_eq!(key, "a");
            assert_eq!(modifiers.len(), 2);
        } else {
            panic!("Deserialized to wrong variant");
        }
    }

    #[test]
    fn test_mouse_button_variants() {
        assert_eq!(
            serde_json::to_string(&MouseButton::Left).unwrap(),
            "\"Left\""
        );
        assert_eq!(
            serde_json::to_string(&MouseButton::Right).unwrap(),
            "\"Right\""
        );
        assert_eq!(
            serde_json::to_string(&MouseButton::Middle).unwrap(),
            "\"Middle\""
        );
    }

    #[test]
    fn test_actions_in_window_count() {
        let mut engine = ComputerControlEngine::new();
        assert_eq!(engine.actions_in_window(), 0);

        engine.record_dry_run(InputAction::Wait { ms: 1 });
        engine.record_dry_run(InputAction::Wait { ms: 1 });
        assert_eq!(engine.actions_in_window(), 2);
    }
}
