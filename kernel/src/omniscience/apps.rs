//! AppIntegration — deep integration with common application types.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

// ── Types ───────────────────────────────────────────────────────────────

/// Known application categories for specialized context extraction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AppType {
    Editor,
    Browser,
    Terminal,
    FileManager,
    Other,
}

impl AppType {
    /// Classify an application name into a type.
    pub fn from_name(name: &str) -> Self {
        let lower = name.to_lowercase();
        if lower.contains("code")
            || lower.contains("vim")
            || lower.contains("emacs")
            || lower.contains("editor")
            || lower.contains("sublime")
            || lower.contains("atom")
            || lower.contains("intellij")
            || lower.contains("jetbrains")
        {
            Self::Editor
        } else if lower.contains("browser")
            || lower.contains("firefox")
            || lower.contains("chrome")
            || lower.contains("safari")
            || lower.contains("edge")
        {
            Self::Browser
        } else if lower.contains("terminal")
            || lower.contains("console")
            || lower.contains("iterm")
            || lower.contains("alacritty")
            || lower.contains("kitty")
            || lower.contains("wezterm")
        {
            Self::Terminal
        } else if lower.contains("finder")
            || lower.contains("nautilus")
            || lower.contains("explorer")
            || lower.contains("files")
            || lower.contains("dolphin")
            || lower.contains("thunar")
        {
            Self::FileManager
        } else {
            Self::Other
        }
    }
}

/// Context captured from a specific application.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppContext {
    /// Application name.
    pub app_name: String,
    /// Classified application type.
    pub app_type: AppType,
    /// Application-specific context data (varies by app type).
    pub context_data: Value,
    /// Unix timestamp in milliseconds when captured.
    pub captured_at: u64,
}

// ── AppIntegration ──────────────────────────────────────────────────────

/// Manages deep integration with known application types.
///
/// Tracks active applications, captures app-specific context,
/// and provides enriched data for the intent predictor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppIntegration {
    /// Known application contexts, keyed by app name.
    contexts: HashMap<String, AppContext>,
    /// Currently detected active application.
    active_app: Option<String>,
}

impl AppIntegration {
    /// Create a new `AppIntegration`.
    pub fn new() -> Self {
        Self {
            contexts: HashMap::new(),
            active_app: None,
        }
    }

    /// Detect and set the active application.
    ///
    /// In production this would query the window manager;
    /// here it accepts the app name directly for testability.
    pub fn detect_active_app(&mut self, app_name: &str) {
        self.active_app = Some(app_name.to_string());
    }

    /// Get the name of the currently active application, if known.
    pub fn active_app(&self) -> Option<&str> {
        self.active_app.as_deref()
    }

    /// Get the app type of the currently active application.
    pub fn active_app_type(&self) -> Option<AppType> {
        self.active_app.as_deref().map(AppType::from_name)
    }

    /// Capture and store context for a given application.
    ///
    /// The `context_data` is application-specific JSON. For example:
    /// - Editor: `{ "file": "main.rs", "line": 42, "language": "rust" }`
    /// - Browser: `{ "url": "https://...", "title": "..." }`
    /// - Terminal: `{ "cwd": "/home/user", "last_command": "cargo test" }`
    pub fn capture_app_context(&mut self, app_name: &str, context_data: Value) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let ctx = AppContext {
            app_name: app_name.to_string(),
            app_type: AppType::from_name(app_name),
            context_data,
            captured_at: now,
        };
        self.contexts.insert(app_name.to_string(), ctx);
    }

    /// Get the most recently captured context for a given application.
    pub fn get_app_context(&self, app_name: &str) -> Option<&AppContext> {
        self.contexts.get(app_name)
    }

    /// Get all captured application contexts.
    pub fn all_contexts(&self) -> Vec<&AppContext> {
        self.contexts.values().collect()
    }

    /// Remove context for a given application.
    pub fn remove_app_context(&mut self, app_name: &str) -> Option<AppContext> {
        self.contexts.remove(app_name)
    }

    /// Clear all application contexts.
    pub fn clear(&mut self) {
        self.contexts.clear();
        self.active_app = None;
    }

    /// Return the number of tracked applications.
    pub fn tracked_count(&self) -> usize {
        self.contexts.len()
    }
}

impl Default for AppIntegration {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn app_type_classification() {
        assert_eq!(AppType::from_name("Visual Studio Code"), AppType::Editor);
        assert_eq!(AppType::from_name("vim"), AppType::Editor);
        assert_eq!(AppType::from_name("Firefox"), AppType::Browser);
        assert_eq!(AppType::from_name("Google Chrome"), AppType::Browser);
        assert_eq!(AppType::from_name("Terminal"), AppType::Terminal);
        assert_eq!(AppType::from_name("Alacritty"), AppType::Terminal);
        assert_eq!(AppType::from_name("Nautilus"), AppType::FileManager);
        assert_eq!(AppType::from_name("Finder"), AppType::FileManager);
        assert_eq!(AppType::from_name("Spotify"), AppType::Other);
    }

    #[test]
    fn detect_active_app() {
        let mut integration = AppIntegration::new();
        assert!(integration.active_app().is_none());

        integration.detect_active_app("Firefox");
        assert_eq!(integration.active_app(), Some("Firefox"));
        assert_eq!(integration.active_app_type(), Some(AppType::Browser));
    }

    #[test]
    fn capture_and_get_context() {
        let mut integration = AppIntegration::new();
        integration.capture_app_context("VS Code", json!({"file": "main.rs", "line": 42}));

        let ctx = integration.get_app_context("VS Code").unwrap();
        assert_eq!(ctx.app_type, AppType::Editor);
        assert_eq!(ctx.context_data["file"], "main.rs");
    }

    #[test]
    fn context_overwrites_on_same_app() {
        let mut integration = AppIntegration::new();
        integration.capture_app_context("Terminal", json!({"cwd": "/home"}));
        integration.capture_app_context("Terminal", json!({"cwd": "/tmp"}));

        assert_eq!(integration.tracked_count(), 1);
        let ctx = integration.get_app_context("Terminal").unwrap();
        assert_eq!(ctx.context_data["cwd"], "/tmp");
    }

    #[test]
    fn multiple_apps() {
        let mut integration = AppIntegration::new();
        integration.capture_app_context("Firefox", json!({"url": "https://example.com"}));
        integration.capture_app_context("Terminal", json!({"cwd": "/home"}));

        assert_eq!(integration.tracked_count(), 2);
        assert_eq!(integration.all_contexts().len(), 2);
    }

    #[test]
    fn remove_context() {
        let mut integration = AppIntegration::new();
        integration.capture_app_context("Firefox", json!({}));
        let removed = integration.remove_app_context("Firefox");
        assert!(removed.is_some());
        assert_eq!(integration.tracked_count(), 0);
    }

    #[test]
    fn clear_all() {
        let mut integration = AppIntegration::new();
        integration.detect_active_app("Firefox");
        integration.capture_app_context("Firefox", json!({}));
        integration.clear();
        assert!(integration.active_app().is_none());
        assert_eq!(integration.tracked_count(), 0);
    }

    #[test]
    fn app_context_serialization() {
        let ctx = AppContext {
            app_name: "VS Code".into(),
            app_type: AppType::Editor,
            context_data: json!({"file": "test.rs"}),
            captured_at: 1000,
        };
        let json_str = serde_json::to_string(&ctx).unwrap();
        let deser: AppContext = serde_json::from_str(&json_str).unwrap();
        assert_eq!(deser.app_name, "VS Code");
        assert_eq!(deser.app_type, AppType::Editor);
    }

    #[test]
    fn app_type_serialization() {
        for app_type in [
            AppType::Editor,
            AppType::Browser,
            AppType::Terminal,
            AppType::FileManager,
            AppType::Other,
        ] {
            let json_str = serde_json::to_string(&app_type).unwrap();
            let deser: AppType = serde_json::from_str(&json_str).unwrap();
            assert_eq!(deser, app_type);
        }
    }

    #[test]
    fn get_nonexistent_app() {
        let integration = AppIntegration::new();
        assert!(integration.get_app_context("nonexistent").is_none());
    }
}
