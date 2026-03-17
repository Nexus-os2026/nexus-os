//! ScreenUnderstanding — continuous screen analysis and context capture.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::time::{SystemTime, UNIX_EPOCH};

/// Default capture interval in milliseconds.
const DEFAULT_CAPTURE_INTERVAL_MS: u64 = 5000;

/// Maximum number of contexts kept in the rolling buffer.
const DEFAULT_MAX_HISTORY: usize = 100;

// ── Types ───────────────────────────────────────────────────────────────

/// A UI element detected on screen.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UiElement {
    /// The type of element (e.g. "button", "text_field", "menu_item").
    pub element_type: String,
    /// Visible text content of the element.
    pub text: String,
    /// Bounding rectangle as (x, y, width, height).
    pub bounds: (u32, u32, u32, u32),
    /// Whether the element can be interacted with.
    pub interactable: bool,
}

/// A snapshot of the screen at a point in time.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScreenContext {
    /// Unix timestamp in milliseconds when this context was captured.
    pub timestamp: u64,
    /// Name of the currently active (foreground) application.
    pub active_app: String,
    /// Visible text extracted from the screen.
    pub visible_text: Vec<String>,
    /// UI elements detected on screen.
    pub ui_elements: Vec<UiElement>,
    /// The most recent user action, if any.
    pub user_action: Option<String>,
}

// ── ScreenUnderstanding ─────────────────────────────────────────────────

/// Continuous screen analysis engine.
///
/// Captures periodic screen snapshots, extracts text and UI elements,
/// and maintains a rolling context buffer for intent prediction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenUnderstanding {
    /// Rolling buffer of captured screen contexts.
    history: VecDeque<ScreenContext>,
    /// Maximum number of contexts to keep.
    max_history: usize,
    /// Capture interval in milliseconds.
    capture_interval_ms: u64,
    /// Whether the engine is actively capturing.
    active: bool,
}

impl ScreenUnderstanding {
    /// Create a new `ScreenUnderstanding` with default settings.
    pub fn new() -> Self {
        Self {
            history: VecDeque::new(),
            max_history: DEFAULT_MAX_HISTORY,
            capture_interval_ms: DEFAULT_CAPTURE_INTERVAL_MS,
            active: false,
        }
    }

    /// Create with a custom capture interval and history size.
    pub fn with_config(capture_interval_ms: u64, max_history: usize) -> Self {
        Self {
            history: VecDeque::new(),
            max_history,
            capture_interval_ms,
            active: false,
        }
    }

    /// Return the configured capture interval in milliseconds.
    pub fn capture_interval_ms(&self) -> u64 {
        self.capture_interval_ms
    }

    /// Set the capture interval in milliseconds.
    pub fn set_capture_interval_ms(&mut self, ms: u64) {
        self.capture_interval_ms = ms;
    }

    /// Start the capture engine.
    pub fn start(&mut self) {
        self.active = true;
    }

    /// Stop the capture engine.
    pub fn stop(&mut self) {
        self.active = false;
    }

    /// Whether the engine is actively capturing.
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Capture a new screen context and add it to the rolling buffer.
    ///
    /// In production this would invoke platform-specific screen capture;
    /// here we accept a pre-built `ScreenContext` for testability.
    pub fn capture_context(&mut self, context: ScreenContext) {
        if self.history.len() >= self.max_history {
            self.history.pop_front();
        }
        self.history.push_back(context);
    }

    /// Get the last `n` screen contexts (most recent last).
    pub fn get_rolling_context(&self, last_n: usize) -> Vec<&ScreenContext> {
        let len = self.history.len();
        let start = len.saturating_sub(last_n);
        self.history.iter().skip(start).collect()
    }

    /// Get all contexts currently in the rolling buffer.
    pub fn all_contexts(&self) -> Vec<&ScreenContext> {
        self.history.iter().collect()
    }

    /// Return the number of captured contexts.
    pub fn context_count(&self) -> usize {
        self.history.len()
    }

    /// Extract all visible text from the most recent context.
    pub fn extract_text_from_screen(&self) -> Vec<String> {
        self.history
            .back()
            .map(|ctx| ctx.visible_text.clone())
            .unwrap_or_default()
    }

    /// Clear the history buffer.
    pub fn clear(&mut self) {
        self.history.clear();
    }
}

impl Default for ScreenUnderstanding {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper to get the current Unix timestamp in milliseconds.
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_context(app: &str, text: Vec<&str>, action: Option<&str>) -> ScreenContext {
        ScreenContext {
            timestamp: now_ms(),
            active_app: app.to_string(),
            visible_text: text.into_iter().map(String::from).collect(),
            ui_elements: vec![UiElement {
                element_type: "button".into(),
                text: "OK".into(),
                bounds: (10, 20, 80, 30),
                interactable: true,
            }],
            user_action: action.map(String::from),
        }
    }

    #[test]
    fn new_engine_defaults() {
        let engine = ScreenUnderstanding::new();
        assert_eq!(engine.capture_interval_ms(), DEFAULT_CAPTURE_INTERVAL_MS);
        assert_eq!(engine.context_count(), 0);
        assert!(!engine.is_active());
    }

    #[test]
    fn start_stop() {
        let mut engine = ScreenUnderstanding::new();
        engine.start();
        assert!(engine.is_active());
        engine.stop();
        assert!(!engine.is_active());
    }

    #[test]
    fn capture_and_retrieve() {
        let mut engine = ScreenUnderstanding::new();
        engine.capture_context(make_context("editor", vec!["hello"], None));
        engine.capture_context(make_context("browser", vec!["world"], Some("click")));

        assert_eq!(engine.context_count(), 2);
        let rolling = engine.get_rolling_context(1);
        assert_eq!(rolling.len(), 1);
        assert_eq!(rolling[0].active_app, "browser");
    }

    #[test]
    fn rolling_context_respects_n() {
        let mut engine = ScreenUnderstanding::new();
        for i in 0..5 {
            engine.capture_context(make_context(&format!("app{i}"), vec![], None));
        }
        let rolling = engine.get_rolling_context(3);
        assert_eq!(rolling.len(), 3);
        assert_eq!(rolling[0].active_app, "app2");
        assert_eq!(rolling[2].active_app, "app4");
    }

    #[test]
    fn history_eviction() {
        let mut engine = ScreenUnderstanding::with_config(1000, 3);
        for i in 0..5 {
            engine.capture_context(make_context(&format!("app{i}"), vec![], None));
        }
        assert_eq!(engine.context_count(), 3);
        let all = engine.all_contexts();
        assert_eq!(all[0].active_app, "app2");
    }

    #[test]
    fn extract_text_from_screen() {
        let mut engine = ScreenUnderstanding::new();
        assert!(engine.extract_text_from_screen().is_empty());

        engine.capture_context(make_context("editor", vec!["line1", "line2"], None));
        let text = engine.extract_text_from_screen();
        assert_eq!(text, vec!["line1", "line2"]);
    }

    #[test]
    fn clear_history() {
        let mut engine = ScreenUnderstanding::new();
        engine.capture_context(make_context("app", vec![], None));
        engine.clear();
        assert_eq!(engine.context_count(), 0);
    }

    #[test]
    fn set_capture_interval() {
        let mut engine = ScreenUnderstanding::new();
        engine.set_capture_interval_ms(1000);
        assert_eq!(engine.capture_interval_ms(), 1000);
    }

    #[test]
    fn screen_context_serialization() {
        let ctx = make_context("editor", vec!["hello"], Some("typing"));
        let json = serde_json::to_string(&ctx).unwrap();
        let deserialized: ScreenContext = serde_json::from_str(&json).unwrap();
        assert_eq!(ctx, deserialized);
    }

    #[test]
    fn ui_element_serialization() {
        let elem = UiElement {
            element_type: "text_field".into(),
            text: "Name".into(),
            bounds: (0, 0, 200, 30),
            interactable: true,
        };
        let json = serde_json::to_string(&elem).unwrap();
        let deserialized: UiElement = serde_json::from_str(&json).unwrap();
        assert_eq!(elem, deserialized);
    }
}
