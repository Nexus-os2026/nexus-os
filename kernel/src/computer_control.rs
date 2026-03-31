//! Computer-Control Engine — agents control the real desktop in a governed way.
//!
//! The kernel exposes a small, platform-aware surface for:
//! - screenshot capture
//! - desktop input simulation
//! - Ollama vision-model analysis for screenshots
//! - emergency stop / rate limiting for input

use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const DEFAULT_OLLAMA_URL: &str = "http://localhost:11434";
const DEFAULT_INPUT_RATE_LIMIT: usize = 100;
#[cfg(not(test))]
const TYPE_DELAY_MS: u64 = 25;

static INPUT_KILL_SWITCH_ACTIVE: AtomicBool = AtomicBool::new(false);

// ── Types ───────────────────────────────────────────────────────────────

/// A rectangular screen region for targeted capture.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum InputAction {
    Click {
        x: u32,
        y: u32,
        button: MouseButton,
    },
    DoubleClick {
        x: u32,
        y: u32,
    },
    Drag {
        from_x: u32,
        from_y: u32,
        to_x: u32,
        to_y: u32,
    },
    Type {
        text: String,
    },
    KeyPress {
        key: String,
        modifiers: Vec<String>,
    },
    Shortcut {
        keys: Vec<String>,
    },
    MoveMouse {
        x: u32,
        y: u32,
    },
    Scroll {
        direction: String,
        amount: u32,
    },
    Wait {
        ms: u64,
    },
}

impl InputAction {
    /// A short human-readable label for audit logging.
    pub fn label(&self) -> String {
        match self {
            Self::Click { x, y, button } => format!("click({button:?} @ {x},{y})"),
            Self::DoubleClick { x, y } => format!("double_click({x},{y})"),
            Self::Drag {
                from_x,
                from_y,
                to_x,
                to_y,
            } => format!("drag({from_x},{from_y}->{to_x},{to_y})"),
            Self::Type { text } => {
                let preview: String = text.chars().take(20).collect();
                let suffix = if text.chars().count() > 20 { "..." } else { "" };
                format!("type(\"{preview}{suffix}\")")
            }
            Self::KeyPress { key, modifiers } => {
                if modifiers.is_empty() {
                    format!("key({key})")
                } else {
                    format!("key({}+{key})", modifiers.join("+"))
                }
            }
            Self::Shortcut { keys } => format!("shortcut({})", keys.join("+")),
            Self::MoveMouse { x, y } => format!("move({x},{y})"),
            Self::Scroll { direction, amount } => format!("scroll({direction}, {amount})"),
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InputControlStatus {
    pub enabled: bool,
    pub kill_switch_active: bool,
    pub actions_in_current_window: usize,
    pub max_actions_per_minute: usize,
    pub total_actions: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VisionAnalysis {
    pub model: String,
    pub output: String,
    pub screenshot_path: PathBuf,
}

// ── Kill switch ──────────────────────────────────────────────────────────

pub fn emergency_kill_switch_active() -> bool {
    INPUT_KILL_SWITCH_ACTIVE.load(Ordering::SeqCst)
}

pub fn activate_emergency_kill_switch() {
    INPUT_KILL_SWITCH_ACTIVE.store(true, Ordering::SeqCst);
}

pub fn reset_emergency_kill_switch() {
    INPUT_KILL_SWITCH_ACTIVE.store(false, Ordering::SeqCst);
}

// ── Shared helpers ───────────────────────────────────────────────────────

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn now_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn safe_slug(input: &str) -> String {
    let mut out = String::new();
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if (ch == '-' || ch == '_' || ch == ' ') && !out.ends_with('-') {
            out.push('-');
        }
    }
    let trimmed = out.trim_matches('-');
    if trimmed.is_empty() {
        "capture".to_string()
    } else {
        trimmed.to_string()
    }
}

fn run_command(program: &str, args: &[&str]) -> Result<std::process::Output, String> {
    std::process::Command::new(program)
        .args(args)
        .output()
        .map_err(|e| format!("failed to run {program}: {e}"))
}

fn ensure_parent_dir(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create {}: {e}", parent.display()))?;
    }
    Ok(())
}

pub fn control_artifacts_dir(workspace: &Path) -> PathBuf {
    workspace.join(".nexus").join("computer_control")
}

pub fn screenshot_dir(workspace: &Path) -> PathBuf {
    control_artifacts_dir(workspace).join("screenshots")
}

pub fn audit_log_path(workspace: &Path) -> PathBuf {
    control_artifacts_dir(workspace).join("audit.jsonl")
}

pub fn checkpoints_path(workspace: &Path) -> PathBuf {
    control_artifacts_dir(workspace).join("checkpoints.json")
}

pub fn append_audit_log(workspace: &Path, event_kind: &str, payload: Value) -> Result<(), String> {
    let path = audit_log_path(workspace);
    ensure_parent_dir(&path)?;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| format!("failed to open audit log {}: {e}", path.display()))?;
    let entry = json!({
        "timestamp": now_secs(),
        "event_kind": event_kind,
        "payload": payload,
    });
    use std::io::Write;
    writeln!(file, "{}", entry).map_err(|e| format!("failed to write audit log: {e}"))
}

pub fn write_bytes_to_workspace_file(
    workspace: &Path,
    prefix: &str,
    bytes: &[u8],
) -> Result<PathBuf, String> {
    let dir = screenshot_dir(workspace);
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("failed to create screenshot dir {}: {e}", dir.display()))?;
    let path = dir.join(format!("{}-{}.png", safe_slug(prefix), now_millis()));
    std::fs::write(&path, bytes).map_err(|e| format!("failed to write {}: {e}", path.display()))?;
    Ok(path)
}

pub fn text_looks_sensitive(text: &str) -> bool {
    let lowered = text.to_ascii_lowercase();
    let looks_like_secret = text.len() >= 24
        && text.chars().any(|ch| ch.is_ascii_lowercase())
        && text.chars().any(|ch| ch.is_ascii_uppercase())
        && text.chars().any(|ch| ch.is_ascii_digit());

    looks_like_secret
        || lowered.contains("password")
        || lowered.contains("passwd")
        || lowered.contains("passphrase")
        || lowered.contains("api_key")
        || lowered.contains("token")
        || lowered.contains("secret")
        || lowered.contains("private key")
}

pub fn screen_analysis_indicates_sensitive_field(analysis: &str) -> bool {
    let lowered = analysis.to_ascii_lowercase();
    lowered.contains("password field")
        || lowered.contains("password input")
        || lowered.contains("sign-in form")
        || lowered.contains("credential")
        || lowered.contains("secret")
}

// ── Screen capture ──────────────────────────────────────────────────────

/// Capture a screenshot. Returns PNG bytes.
pub fn capture_screen(region: Option<&ScreenRegion>) -> Result<Vec<u8>, String> {
    capture_screen_impl(region)
}

/// Capture a specific window by its title. Returns PNG bytes.
pub fn capture_window(window_title: &str) -> Result<Vec<u8>, String> {
    capture_window_impl(window_title)
}

#[cfg(test)]
fn capture_screen_impl(region: Option<&ScreenRegion>) -> Result<Vec<u8>, String> {
    let size = region
        .map(|r| (r.width.max(1) * r.height.max(1) * 4) as usize)
        .unwrap_or(128);
    Ok(vec![42; size])
}

#[cfg(all(not(test), target_os = "linux"))]
fn capture_screen_impl(region: Option<&ScreenRegion>) -> Result<Vec<u8>, String> {
    let tmp = format!("/tmp/nexus-screen-{}.png", now_millis());
    let mut command = std::process::Command::new("import");
    if let Some(r) = region {
        command
            .arg("-window")
            .arg("root")
            .arg("-crop")
            .arg(format!("{}x{}+{}+{}", r.width, r.height, r.x, r.y));
    } else {
        command.arg("-window").arg("root");
    }
    command.arg(&tmp);

    let output = command
        .output()
        .map_err(|e| format!("failed to run ImageMagick import: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("screen capture failed: {stderr}"));
    }

    let bytes = std::fs::read(&tmp).map_err(|e| format!("failed to read screenshot: {e}"))?;
    // Best-effort: temp screenshot file cleanup is housekeeping; captured bytes are already in memory
    let _ = std::fs::remove_file(&tmp);
    Ok(bytes)
}

#[cfg(all(not(test), target_os = "macos"))]
fn capture_screen_impl(region: Option<&ScreenRegion>) -> Result<Vec<u8>, String> {
    let tmp = format!("/tmp/nexus-screen-{}.png", now_millis());
    let mut command = std::process::Command::new("screencapture");
    if let Some(r) = region {
        command
            .arg("-R")
            .arg(format!("{},{},{}x{}", r.x, r.y, r.width, r.height));
    }
    command.arg(&tmp);

    let output = command
        .output()
        .map_err(|e| format!("failed to run screencapture: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("screen capture failed: {stderr}"));
    }

    let bytes = std::fs::read(&tmp).map_err(|e| format!("failed to read screenshot: {e}"))?;
    // Best-effort: temp screenshot file cleanup is housekeeping; captured bytes are already in memory
    let _ = std::fs::remove_file(&tmp);
    Ok(bytes)
}

#[cfg(all(not(test), not(any(target_os = "linux", target_os = "macos"))))]
fn capture_screen_impl(_region: Option<&ScreenRegion>) -> Result<Vec<u8>, String> {
    Err("screen capture is not supported on this platform".to_string())
}

#[cfg(test)]
fn capture_window_impl(window_title: &str) -> Result<Vec<u8>, String> {
    if window_title.trim().is_empty() {
        return Err("window title cannot be empty".to_string());
    }
    Ok(vec![24; 256])
}

#[cfg(all(not(test), target_os = "linux"))]
fn capture_window_impl(window_title: &str) -> Result<Vec<u8>, String> {
    let search = run_command("xdotool", &["search", "--name", window_title])?;
    if !search.status.success() {
        let stderr = String::from_utf8_lossy(&search.stderr);
        return Err(format!("unable to find window '{window_title}': {stderr}"));
    }
    let window_id = String::from_utf8_lossy(&search.stdout)
        .lines()
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("no window found with title '{window_title}'"))?
        .to_string();

    let tmp = format!("/tmp/nexus-window-{}.png", now_millis());
    let output = run_command("import", &["-window", &window_id, &tmp])?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("window capture failed: {stderr}"));
    }

    let bytes = std::fs::read(&tmp).map_err(|e| format!("failed to read screenshot: {e}"))?;
    // Best-effort: temp window capture file cleanup is housekeeping; captured bytes are already in memory
    let _ = std::fs::remove_file(&tmp);
    Ok(bytes)
}

#[cfg(all(not(test), target_os = "macos"))]
fn capture_window_impl(_window_title: &str) -> Result<Vec<u8>, String> {
    Err("window-title capture is not yet implemented on macOS".to_string())
}

#[cfg(all(not(test), not(any(target_os = "linux", target_os = "macos"))))]
fn capture_window_impl(_window_title: &str) -> Result<Vec<u8>, String> {
    Err("window capture is not supported on this platform".to_string())
}

pub fn capture_and_store_screen(
    workspace: &Path,
    region: Option<&ScreenRegion>,
    audit_label: &str,
) -> Result<PathBuf, String> {
    let bytes = capture_screen(region)?;
    let path = write_bytes_to_workspace_file(workspace, audit_label, &bytes)?;
    append_audit_log(
        workspace,
        "screen.capture",
        json!({
            "path": path,
            "region": region,
        }),
    )?;
    Ok(path)
}

pub fn capture_and_store_window(workspace: &Path, window_title: &str) -> Result<PathBuf, String> {
    let bytes = capture_window(window_title)?;
    let path = write_bytes_to_workspace_file(workspace, window_title, &bytes)?;
    append_audit_log(
        workspace,
        "screen.capture_window",
        json!({
            "path": path,
            "window_title": window_title,
        }),
    )?;
    Ok(path)
}

// ── Vision helpers ──────────────────────────────────────────────────────

fn ollama_url() -> String {
    std::env::var("OLLAMA_URL").unwrap_or_else(|_| DEFAULT_OLLAMA_URL.to_string())
}

pub fn list_ollama_models(base_url: Option<&str>) -> Result<Vec<String>, String> {
    let base = base_url.unwrap_or(DEFAULT_OLLAMA_URL).trim_end_matches('/');
    let output = run_command("curl", &["-sS", &format!("{base}/api/tags")])?;
    if !output.status.success() {
        return Err("failed to query Ollama model list".to_string());
    }
    let payload: Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("failed to parse Ollama model list: {e}"))?;
    Ok(payload
        .get("models")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|model| {
            model
                .get("name")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .collect())
}

pub fn detect_vision_model(base_url: Option<&str>) -> Result<String, String> {
    let models = list_ollama_models(base_url)?;
    models
        .into_iter()
        .find(|name| {
            let lowered = name.to_ascii_lowercase();
            lowered.contains("llava")
                || lowered.contains("vision")
                || lowered.contains("moondream")
                || lowered.contains("bakllava")
        })
        .ok_or_else(|| "No vision model available. Install one with: ollama pull llava".to_string())
}

pub fn query_vision_model(
    prompt: &str,
    image_base64: &str,
    model: Option<&str>,
    base_url: Option<&str>,
) -> Result<String, String> {
    let base = base_url.unwrap_or(DEFAULT_OLLAMA_URL).trim_end_matches('/');
    let model_name = match model {
        Some(model) if !model.trim().is_empty() => model.to_string(),
        _ => detect_vision_model(Some(base))?,
    };
    let body = json!({
        "model": model_name,
        "stream": false,
        "messages": [{
            "role": "user",
            "content": prompt,
            "images": [image_base64],
        }]
    });
    let encoded = serde_json::to_string(&body)
        .map_err(|e| format!("failed to encode vision request: {e}"))?;
    let output = run_command(
        "curl",
        &[
            "-sS",
            "-L",
            "-X",
            "POST",
            "-H",
            "content-type: application/json",
            "-d",
            &encoded,
            &format!("{base}/api/chat"),
        ],
    )?;
    if !output.status.success() {
        return Err("vision query request failed".to_string());
    }
    let payload: Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("failed to parse vision response: {e}"))?;
    payload
        .pointer("/message/content")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            payload
                .get("response")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .filter(|text| !text.trim().is_empty())
        .ok_or_else(|| "vision response did not contain text output".to_string())
}

pub fn analyze_stored_screenshot(
    screenshot_path: &Path,
    query: &str,
    model: Option<&str>,
) -> Result<String, String> {
    let bytes = std::fs::read(screenshot_path)
        .map_err(|e| format!("failed to read {}: {e}", screenshot_path.display()))?;
    let prompt = format!("Analyze this screenshot. {query}");
    query_vision_model(
        &prompt,
        &BASE64_STANDARD.encode(bytes),
        model,
        Some(&ollama_url()),
    )
}

pub fn capture_and_analyze_screen(
    workspace: &Path,
    query: &str,
    model: Option<&str>,
) -> Result<VisionAnalysis, String> {
    let screenshot_path = capture_and_store_screen(workspace, None, "screen-analysis")?;
    let output = analyze_stored_screenshot(&screenshot_path, query, model)?;
    let model_name = match model {
        Some(value) if !value.trim().is_empty() => value.to_string(),
        _ => detect_vision_model(Some(&ollama_url()))?,
    };
    append_audit_log(
        workspace,
        "screen.analyze",
        json!({
            "path": screenshot_path,
            "query": query,
            "model": model_name,
        }),
    )?;
    Ok(VisionAnalysis {
        model: model_name,
        output,
        screenshot_path,
    })
}

// ── Input simulation ────────────────────────────────────────────────────

/// Execute a desktop input action using platform-native tools.
pub fn execute_input_action(action: &InputAction) -> Result<(), String> {
    if emergency_kill_switch_active() {
        return Err("input control blocked by emergency kill switch".to_string());
    }
    execute_input_action_impl(action)
}

#[cfg(test)]
fn execute_input_action_impl(_action: &InputAction) -> Result<(), String> {
    Ok(())
}

#[cfg(all(not(test), target_os = "linux"))]
fn execute_input_action_impl(action: &InputAction) -> Result<(), String> {
    match action {
        InputAction::Click { x, y, button } => {
            let btn = match button {
                MouseButton::Left => "1",
                MouseButton::Middle => "2",
                MouseButton::Right => "3",
            };
            run_xdotool(&["mousemove", &x.to_string(), &y.to_string()])?;
            run_xdotool(&["click", btn])
        }
        InputAction::DoubleClick { x, y } => {
            run_xdotool(&["mousemove", &x.to_string(), &y.to_string()])?;
            run_xdotool(&["click", "--repeat", "2", "1"])
        }
        InputAction::Drag {
            from_x,
            from_y,
            to_x,
            to_y,
        } => {
            run_xdotool(&["mousemove", &from_x.to_string(), &from_y.to_string()])?;
            run_xdotool(&["mousedown", "1"])?;
            run_xdotool(&["mousemove", "--sync", &to_x.to_string(), &to_y.to_string()])?;
            run_xdotool(&["mouseup", "1"])
        }
        InputAction::Type { text } => run_xdotool(&[
            "type",
            "--delay",
            &TYPE_DELAY_MS.to_string(),
            "--clearmodifiers",
            text,
        ]),
        InputAction::KeyPress { key, modifiers } => {
            let combo = if modifiers.is_empty() {
                key.clone()
            } else {
                format!("{}+{key}", modifiers.join("+"))
            };
            run_xdotool(&["key", &combo])
        }
        InputAction::Shortcut { keys } => {
            if keys.is_empty() {
                return Err("shortcut requires at least one key".to_string());
            }
            run_xdotool(&["key", &keys.join("+")])
        }
        InputAction::MoveMouse { x, y } => {
            run_xdotool(&["mousemove", &x.to_string(), &y.to_string()])
        }
        InputAction::Scroll { direction, amount } => {
            let button = match direction.to_ascii_lowercase().as_str() {
                "up" => "4",
                "down" => "5",
                other => return Err(format!("unsupported scroll direction '{other}'")),
            };
            let repeats = (*amount).max(1);
            run_xdotool(&["click", "--repeat", &repeats.to_string(), button])
        }
        InputAction::Wait { ms } => {
            std::thread::sleep(std::time::Duration::from_millis(*ms));
            Ok(())
        }
    }
}

#[cfg(all(not(test), target_os = "linux"))]
fn run_xdotool(args: &[&str]) -> Result<(), String> {
    let output = std::process::Command::new("xdotool")
        .args(args)
        .output()
        .map_err(|e| format!("failed to run xdotool: {e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "xdotool error: {}",
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

#[cfg(all(not(test), target_os = "macos"))]
fn execute_input_action_impl(action: &InputAction) -> Result<(), String> {
    match action {
        InputAction::Wait { ms } => {
            std::thread::sleep(std::time::Duration::from_millis(*ms));
            Ok(())
        }
        InputAction::Type { text } => {
            let script = format!("tell application \"System Events\" to keystroke {:?}", text);
            run_osascript(&script)
        }
        InputAction::KeyPress { key, .. } => {
            let script = format!("tell application \"System Events\" to keystroke {:?}", key);
            run_osascript(&script)
        }
        InputAction::Shortcut { keys } => {
            let joined = keys.join("+");
            let script = format!(
                "tell application \"System Events\" to keystroke {:?}",
                joined
            );
            run_osascript(&script)
        }
        other => Err(format!("action not yet supported on macOS: {other:?}")),
    }
}

#[cfg(all(not(test), target_os = "macos"))]
fn run_osascript(script: &str) -> Result<(), String> {
    let output = std::process::Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .map_err(|e| format!("failed to run osascript: {e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "osascript error: {}",
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

#[cfg(all(not(test), not(any(target_os = "linux", target_os = "macos"))))]
fn execute_input_action_impl(_action: &InputAction) -> Result<(), String> {
    Err("input simulation is not supported on this platform".to_string())
}

// ── ComputerControlEngine ───────────────────────────────────────────────

/// Governed engine for desktop automation. Enforces rate limits and tracks
/// a full audit history of every action executed.
pub struct ComputerControlEngine {
    enabled: bool,
    action_history: Vec<ActionRecord>,
    max_actions_per_minute: usize,
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
            enabled: false,
            action_history: Vec::new(),
            max_actions_per_minute: DEFAULT_INPUT_RATE_LIMIT,
            recent_timestamps: VecDeque::new(),
        }
    }

    /// Execute an input action with governance checks.
    pub fn execute(&mut self, action: InputAction) -> Result<ActionRecord, String> {
        if !self.enabled {
            return Err("Computer control is disabled".to_string());
        }
        if emergency_kill_switch_active() {
            return Err("input control blocked by emergency kill switch".to_string());
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
        if let Some(error) = &record.error {
            Err(error.clone())
        } else {
            Ok(record)
        }
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

    /// Capture a screenshot summary.
    pub fn capture_screen(&self, region: Option<&ScreenRegion>) -> Result<CaptureResult, String> {
        if !self.enabled {
            return Err("Computer control is disabled".to_string());
        }
        let data = capture_screen(region)?;
        Ok(CaptureResult {
            width: region.map(|r| r.width).unwrap_or(0),
            height: region.map(|r| r.height).unwrap_or(0),
            format: "png".to_string(),
            size_bytes: data.len(),
        })
    }

    pub fn action_history(&self) -> &[ActionRecord] {
        &self.action_history
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn max_actions_per_minute(&self) -> usize {
        self.max_actions_per_minute
    }

    pub fn set_max_actions_per_minute(&mut self, limit: usize) {
        self.max_actions_per_minute = limit.max(1);
    }

    pub fn rate_limit_check(&mut self) -> Result<(), String> {
        self.evict_stale_timestamps();
        if self.recent_timestamps.len() >= self.max_actions_per_minute {
            return Err(format!(
                "Rate limit exceeded: {} actions in the last 60 seconds (max {})",
                self.recent_timestamps.len(),
                self.max_actions_per_minute
            ));
        }
        Ok(())
    }

    pub fn actions_in_window(&mut self) -> usize {
        self.evict_stale_timestamps();
        self.recent_timestamps.len()
    }

    pub fn total_actions(&self) -> usize {
        self.action_history.len()
    }

    pub fn status(&mut self) -> InputControlStatus {
        InputControlStatus {
            enabled: self.enabled,
            kill_switch_active: emergency_kill_switch_active(),
            actions_in_current_window: self.actions_in_window(),
            max_actions_per_minute: self.max_actions_per_minute,
            total_actions: self.total_actions(),
        }
    }

    fn evict_stale_timestamps(&mut self) {
        let window_start = now_secs().saturating_sub(60);
        while self
            .recent_timestamps
            .front()
            .is_some_and(|&ts| ts < window_start)
        {
            self.recent_timestamps.pop_front();
        }
    }

    #[cfg(test)]
    pub fn push_timestamp_for_testing(&mut self, timestamp: u64) {
        self.recent_timestamps.push_back(timestamp);
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

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
            InputAction::Drag {
                from_x: 1,
                from_y: 2,
                to_x: 3,
                to_y: 4
            }
            .label(),
            "drag(1,2->3,4)"
        );
        assert_eq!(
            InputAction::Shortcut {
                keys: vec!["Ctrl".into(), "Shift".into(), "T".into()]
            }
            .label(),
            "shortcut(Ctrl+Shift+T)"
        );
        assert_eq!(
            InputAction::Scroll {
                direction: "down".into(),
                amount: 3
            }
            .label(),
            "scroll(down, 3)"
        );
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
        let record = engine.record_dry_run(InputAction::Click {
            x: 100,
            y: 200,
            button: MouseButton::Left,
        });
        assert!(record.success);
        assert!(record.error.is_none());
        assert_eq!(engine.total_actions(), 1);
    }

    #[test]
    fn test_rate_limit_default_is_100() {
        let engine = ComputerControlEngine::new();
        assert_eq!(engine.max_actions_per_minute(), 100);
    }

    #[test]
    fn test_rate_limit_check_blocks_when_window_full() {
        let mut engine = ComputerControlEngine::new();
        for _ in 0..engine.max_actions_per_minute() {
            engine.push_timestamp_for_testing(now_secs());
        }
        let error = engine.rate_limit_check().unwrap_err();
        assert!(error.contains("Rate limit exceeded"));
    }

    #[test]
    fn test_status_reflects_kill_switch() {
        reset_emergency_kill_switch();
        let mut engine = ComputerControlEngine::new();
        engine.enable();
        activate_emergency_kill_switch();
        let status = engine.status();
        assert!(status.kill_switch_active);
        reset_emergency_kill_switch();
    }

    #[test]
    fn test_sensitive_text_detection() {
        assert!(text_looks_sensitive("password=supersecret"));
        assert!(text_looks_sensitive("sk-live-MyVeryLongApiKey123456"));
        assert!(!text_looks_sensitive("hello world"));
    }

    #[test]
    fn test_sensitive_field_detection_from_analysis() {
        assert!(screen_analysis_indicates_sensitive_field(
            "This screenshot shows a password field in a sign-in form."
        ));
        assert!(!screen_analysis_indicates_sensitive_field(
            "This screenshot shows a normal document editor."
        ));
    }

    #[test]
    fn test_audit_log_written() {
        let tmp = TempDir::new().unwrap();
        append_audit_log(
            tmp.path(),
            "screen.capture",
            json!({"path": "/tmp/example.png"}),
        )
        .unwrap();
        let contents = std::fs::read_to_string(audit_log_path(tmp.path())).unwrap();
        assert!(contents.contains("screen.capture"));
    }

    #[test]
    fn test_workspace_file_path_is_png() {
        let tmp = TempDir::new().unwrap();
        let path = write_bytes_to_workspace_file(tmp.path(), "sample", b"png").unwrap();
        assert!(path.extension().and_then(|ext| ext.to_str()) == Some("png"));
        assert!(path.exists());
    }

    #[test]
    fn test_detect_vision_model_errors_when_none_available() {
        let result = detect_vision_model(Some("http://127.0.0.1:9"));
        assert!(result.is_err());
    }
}
